#!/usr/bin/env python3
"""
VAULTX ATOMIC FILE OPERATIONS
Prevents data loss during file operations through atomic writes

SECURITY FEATURES:
- Atomic file operations (write-then-move)
- Temporary file creation and verification
- Crash recovery mechanisms
- Rollback on failure
- Integrity verification
- Safe file replacement
"""

import os
import tempfile
import shutil
import hashlib
import time
import uuid
from pathlib import Path
from typing import Optional, Tuple, Callable
import threading
import lockfile
import json


class FileOperationError(Exception):
    """File operation related errors"""
    pass


class AtomicFileWriter:
    """Atomic file writer with verification and rollback"""
    
    def __init__(self, target_path: str, verify_callback: Optional[Callable] = None):
        """
        Initialize atomic file writer
        
        Args:
            target_path: Final file path
            verify_callback: Optional verification function
        """
        self.target_path = Path(target_path)
        self.verify_callback = verify_callback
        self.temp_path = None
        self.backup_path = None
        self.file_handle = None
        self.hash_calculator = hashlib.sha256()
        self.expected_size = None
        self.actual_size = 0
        self.magic_bytes = None
        self.operation_id = str(uuid.uuid4())
        
    def __enter__(self):
        """Context manager entry"""
        self._prepare_temp_file()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit with cleanup"""
        if exc_type is None:
            # Success - complete atomic operation
            self._complete_atomic_write()
        else:
            # Failure - cleanup and rollback
            self._rollback()
        
        self._cleanup()
    
    def _prepare_temp_file(self):
        """Prepare temporary file for writing"""
        target_dir = self.target_path.parent
        
        # Create temporary file in same directory (ensures same filesystem)
        temp_name = f".vaultx_temp_{self.operation_id}_{int(time.time())}"
        self.temp_path = target_dir / temp_name
        
        # Create backup if target exists
        if self.target_path.exists():
            backup_name = f".vaultx_backup_{self.operation_id}_{int(time.time())}"
            self.backup_path = target_dir / backup_name
            shutil.copy2(self.target_path, self.backup_path)
        
        # Open temporary file for writing
        try:
            self.file_handle = open(self.temp_path, 'wb')
        except Exception as e:
            raise FileOperationError(f"Failed to create temporary file: {e}")
    
    def write(self, data: bytes):
        """Write data to temporary file"""
        if self.file_handle is None:
            raise FileOperationError("File not opened for writing")
        
        try:
            self.file_handle.write(data)
            self.hash_calculator.update(data)
            self.actual_size += len(data)
        except Exception as e:
            raise FileOperationError(f"Write operation failed: {e}")
    
    def write_magic_bytes(self, magic: bytes):
        """Write magic bytes and store for verification"""
        self.write(magic)
        self.magic_bytes = magic
    
    def set_expected_size(self, size: int):
        """Set expected file size for verification"""
        self.expected_size = size
    
    def get_file_hash(self) -> str:
        """Get SHA-256 hash of written data"""
        return self.hash_calculator.hexdigest()
    
    def _verify_temp_file(self) -> bool:
        """Verify temporary file before moving to target"""
        if not self.temp_path.exists():
            raise FileOperationError("Temporary file not found")
        
        # Check file size
        actual_size = self.temp_path.stat().st_size
        if self.expected_size is not None and actual_size != self.expected_size:
            raise FileOperationError(
                f"Size mismatch: expected {self.expected_size}, got {actual_size}"
            )
        
        # Verify magic bytes if set
        if self.magic_bytes:
            try:
                with open(self.temp_path, 'rb') as f:
                    actual_magic = f.read(len(self.magic_bytes))
                    if actual_magic != self.magic_bytes:
                        raise FileOperationError("Magic bytes verification failed")
            except Exception as e:
                raise FileOperationError(f"Magic bytes verification error: {e}")
        
        # Run custom verification callback if provided
        if self.verify_callback:
            try:
                if not self.verify_callback(self.temp_path):
                    raise FileOperationError("Custom verification failed")
            except Exception as e:
                raise FileOperationError(f"Custom verification error: {e}")
        
        return True
    
    def _complete_atomic_write(self):
        """Complete atomic write by moving temp file to target"""
        try:
            # Flush and close file handle
            if self.file_handle:
                self.file_handle.flush()
                os.fsync(self.file_handle.fileno())
                self.file_handle.close()
                self.file_handle = None
            
            # Verify temporary file
            self._verify_temp_file()
            
            # Atomic move (rename is atomic on most filesystems)
            self.temp_path.rename(self.target_path)
            
            # Remove backup if successful
            if self.backup_path and self.backup_path.exists():
                self.backup_path.unlink()
                
        except Exception as e:
            raise FileOperationError(f"Failed to complete atomic write: {e}")
    
    def _rollback(self):
        """Rollback operation - cleanup temp file and restore backup"""
        try:
            # Close file handle
            if self.file_handle:
                self.file_handle.close()
                self.file_handle = None
            
            # Remove temporary file
            if self.temp_path and self.temp_path.exists():
                self.temp_path.unlink()
            
            # Restore backup if exists
            if self.backup_path and self.backup_path.exists():
                if self.target_path.exists():
                    self.target_path.unlink()
                self.backup_path.rename(self.target_path)
                
        except Exception:
            pass  # Best effort cleanup
    
    def _cleanup(self):
        """Final cleanup"""
        try:
            if self.file_handle:
                self.file_handle.close()
            
            if self.temp_path and self.temp_path.exists():
                self.temp_path.unlink()
            
            if self.backup_path and self.backup_path.exists():
                self.backup_path.unlink()
                
        except Exception:
            pass  # Best effort cleanup


class CrashRecoveryManager:
    """Manages crash recovery for interrupted operations"""
    
    def __init__(self):
        self.recovery_dir = Path(".vaultx_recovery")
        self.lock_file = self.recovery_dir / "recovery.lock"
        self.operations_file = self.recovery_dir / "operations.json"
        
    def _ensure_recovery_dir(self):
        """Ensure recovery directory exists"""
        self.recovery_dir.mkdir(exist_ok=True)
    
    def _acquire_lock(self) -> bool:
        """Acquire recovery lock"""
        try:
            self._ensure_recovery_dir()
            lock = lockfile.LockFile(str(self.lock_file))
            lock.acquire(timeout=1)
            self._lock = lock
            return True
        except:
            return False
    
    def _release_lock(self):
        """Release recovery lock"""
        if hasattr(self, '_lock'):
            try:
                self._lock.release()
                self._lock = None
            except:
                pass
    
    def register_operation(self, operation_id: str, target_path: str, temp_path: str, backup_path: Optional[str] = None):
        """Register a file operation for crash recovery"""
        if not self._acquire_lock():
            return False
        
        try:
            operations = self._load_operations()
            operations[operation_id] = {
                'target_path': target_path,
                'temp_path': temp_path,
                'backup_path': backup_path,
                'timestamp': time.time()
            }
            self._save_operations(operations)
            return True
        except Exception:
            return False
        finally:
            self._release_lock()
    
    def complete_operation(self, operation_id: str):
        """Mark operation as completed"""
        if not self._acquire_lock():
            return False
        
        try:
            operations = self._load_operations()
            if operation_id in operations:
                del operations[operation_id]
                self._save_operations(operations)
            return True
        except Exception:
            return False
        finally:
            self._release_lock()
    
    def _load_operations(self) -> dict:
        """Load operations from file"""
        if self.operations_file.exists():
            try:
                with open(self.operations_file, 'r') as f:
                    return json.load(f)
            except:
                pass
        return {}
    
    def _save_operations(self, operations: dict):
        """Save operations to file"""
        with open(self.operations_file, 'w') as f:
            json.dump(operations, f, indent=2)
    
    def scan_for_crashed_operations(self) -> list:
        """Scan for crashed operations and return recovery info"""
        crashed_operations = []
        
        if not self._acquire_lock():
            return crashed_operations
        
        try:
            operations = self._load_operations()
            current_time = time.time()
            
            for operation_id, op_info in operations.items():
                # Check if operation is old (likely crashed)
                if current_time - op_info['timestamp'] > 300:  # 5 minutes
                    target_path = Path(op_info['target_path'])
                    temp_path = Path(op_info['temp_path'])
                    backup_path = Path(op_info['backup_path']) if op_info['backup_path'] else None
                    
                    recovery_info = {
                        'operation_id': operation_id,
                        'target_path': target_path,
                        'temp_path': temp_path,
                        'backup_path': backup_path,
                        'status': self._analyze_operation_status(target_path, temp_path, backup_path)
                    }
                    crashed_operations.append(recovery_info)
            
            return crashed_operations
        except Exception:
            return []
        finally:
            self._release_lock()
    
    def _analyze_operation_status(self, target_path: Path, temp_path: Path, backup_path: Optional[Path]) -> str:
        """Analyze the status of a crashed operation"""
        target_exists = target_path.exists()
        temp_exists = temp_path.exists()
        backup_exists = backup_path.exists() if backup_path else False
        
        if not target_exists and not temp_exists:
            return "failed_no_files"
        elif not target_exists and temp_exists:
            return "failed_temp_only"
        elif target_exists and temp_exists:
            return "interrupted_both_exist"
        elif target_exists and not temp_exists:
            if backup_exists:
                return "possibly_complete"
            else:
                return "possibly_complete_no_backup"
        else:
            return "unknown"
    
    def recover_operation(self, operation_id: str, action: str) -> bool:
        """Recover from a crashed operation"""
        if not self._acquire_lock():
            return False
        
        try:
            operations = self._load_operations()
            if operation_id not in operations:
                return False
            
            op_info = operations[operation_id]
            target_path = Path(op_info['target_path'])
            temp_path = Path(op_info['temp_path'])
            backup_path = Path(op_info['backup_path']) if op_info['backup_path'] else None
            
            success = False
            
            if action == "restore_backup":
                success = self._restore_backup(target_path, backup_path)
            elif action == "complete_temp":
                success = self._complete_from_temp(target_path, temp_path)
            elif action == "cleanup":
                success = self._cleanup_crashed(target_path, temp_path, backup_path)
            
            if success:
                # Remove from operations tracking
                del operations[operation_id]
                self._save_operations(operations)
            
            return success
        except Exception:
            return False
        finally:
            self._release_lock()
    
    def _restore_backup(self, target_path: Path, backup_path: Optional[Path]) -> bool:
        """Restore from backup"""
        if not backup_path or not backup_path.exists():
            return False
        
        try:
            if target_path.exists():
                target_path.unlink()
            backup_path.rename(target_path)
            return True
        except:
            return False
    
    def _complete_from_temp(self, target_path: Path, temp_path: Path) -> bool:
        """Complete operation from temp file"""
        if not temp_path.exists():
            return False
        
        try:
            if target_path.exists():
                target_path.unlink()
            temp_path.rename(target_path)
            return True
        except:
            return False
    
    def _cleanup_crashed(self, target_path: Path, temp_path: Path, backup_path: Optional[Path]) -> bool:
        """Clean up crashed operation files"""
        try:
            if temp_path.exists():
                temp_path.unlink()
            if backup_path and backup_path.exists():
                backup_path.unlink()
            return True
        except:
            return False


class SafeFileOperations:
    """Safe file operations with crash recovery"""
    
    def __init__(self):
        self.recovery_manager = CrashRecoveryManager()
    
    def atomic_write(self, target_path: str, data: bytes, 
                    magic_bytes: Optional[bytes] = None,
                    verify_callback: Optional[Callable] = None) -> bool:
        """Atomically write data to file"""
        try:
            with AtomicFileWriter(target_path, verify_callback) as writer:
                if magic_bytes:
                    writer.write_magic_bytes(magic_bytes)
                    writer.set_expected_size(len(magic_bytes) + len(data))
                    writer.write(data)
                else:
                    writer.set_expected_size(len(data))
                    writer.write(data)
            
            return True
        except Exception:
            return False
    
    def atomic_write_with_recovery(self, target_path: str, data: bytes,
                                  magic_bytes: Optional[bytes] = None,
                                  verify_callback: Optional[Callable] = None) -> bool:
        """Atomically write data with crash recovery"""
        operation_id = str(uuid.uuid4())
        
        try:
            with AtomicFileWriter(target_path, verify_callback) as writer:
                # Register for crash recovery
                temp_path = str(writer.temp_path)
                backup_path = str(writer.backup_path) if writer.backup_path else None
                
                self.recovery_manager.register_operation(
                    operation_id, target_path, temp_path, backup_path
                )
                
                # Write data
                if magic_bytes:
                    writer.write_magic_bytes(magic_bytes)
                    writer.set_expected_size(len(magic_bytes) + len(data))
                    writer.write(data)
                else:
                    writer.set_expected_size(len(data))
                    writer.write(data)
            
            # Mark as complete
            self.recovery_manager.complete_operation(operation_id)
            return True
            
        except Exception:
            # Cleanup on failure
            try:
                self.recovery_manager.recover_operation(operation_id, "cleanup")
            except:
                pass
            return False
    
    def verify_file_integrity(self, file_path: str, expected_magic: Optional[bytes] = None,
                            expected_size: Optional[int] = None) -> bool:
        """Verify file integrity"""
        try:
            path = Path(file_path)
            if not path.exists():
                return False
            
            # Check file size
            actual_size = path.stat().st_size
            if expected_size is not None and actual_size != expected_size:
                return False
            
            # Check magic bytes
            if expected_magic:
                with open(path, 'rb') as f:
                    actual_magic = f.read(len(expected_magic))
                    if actual_magic != expected_magic:
                        return False
            
            return True
        except:
            return False
    
    def scan_for_crashed_operations(self) -> list:
        """Scan for crashed operations"""
        return self.recovery_manager.scan_for_crashed_operations()
    
    def recover_crashed_operation(self, operation_id: str, action: str) -> bool:
        """Recover from crashed operation"""
        return self.recovery_manager.recover_operation(operation_id, action)


# Global instance
_safe_ops = SafeFileOperations()


def atomic_write_file(target_path: str, data: bytes, 
                     magic_bytes: Optional[bytes] = None,
                     verify_callback: Optional[Callable] = None) -> bool:
    """Convenience function for atomic file writing"""
    return _safe_ops.atomic_write_with_recovery(target_path, data, magic_bytes, verify_callback)


def verify_file_integrity(file_path: str, expected_magic: Optional[bytes] = None,
                         expected_size: Optional[int] = None) -> bool:
    """Convenience function for file integrity verification"""
    return _safe_ops.verify_file_integrity(file_path, expected_magic, expected_size)


def scan_crashed_operations() -> list:
    """Convenience function for scanning crashed operations"""
    return _safe_ops.scan_for_crashed_operations()


def recover_crashed_operation(operation_id: str, action: str) -> bool:
    """Convenience function for recovering crashed operations"""
    return _safe_ops.recover_crashed_operation(operation_id, action)


# Cleanup on exit
import atexit
atexit.register(lambda: _safe_ops.recovery_manager._release_lock())
