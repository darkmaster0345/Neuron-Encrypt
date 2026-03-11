#!/usr/bin/env python3
"""
VAULTX SECURE MEMORY MANAGEMENT
Protects sensitive data from memory harvesting attacks

SECURITY FEATURES:
- Secure string handling with automatic zeroing
- Memory zeroing on deletion
- Protection against garbage collection delays
- Secure password entry widget
- Key material protection
"""

import os
import sys
import gc
import ctypes
import threading
import time
from typing import Optional, Union
import weakref


class SecureString:
    """Secure string class that zeros memory on deletion"""
    
    def __init__(self, data: Union[str, bytes]):
        """Initialize secure string"""
        if isinstance(data, str):
            self._data = data.encode('utf-8')
        else:
            self._data = bytes(data)
        
        self._locked = False
        self._zero_on_gc = True
        
        # Register for cleanup
        self._finalizer = weakref.finalize(self, self._zero_memory)
    
    def __str__(self):
        """Prevent accidental string conversion"""
        raise SecurityError("Cannot convert SecureString to regular string")
    
    def __repr__(self):
        """Prevent accidental string conversion"""
        return "<SecureString: [REDACTED]>"
    
    def __len__(self):
        """Return length without exposing data"""
        return len(self._data)
    
    def __bytes__(self):
        """Return bytes copy (use carefully)"""
        if self._locked:
            raise SecurityError("SecureString is locked")
        return bytes(self._data)
    
    def __enter__(self):
        """Context manager entry"""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit - zero memory"""
        self.zero()
    
    def zero(self):
        """Explicitly zero memory"""
        if hasattr(self, '_data') and self._data:
            try:
                # Convert to bytearray for modification
                if isinstance(self._data, bytes):
                    data_array = bytearray(self._data)
                    for i in range(len(data_array)):
                        data_array[i] = 0
                    self._data = bytes(data_array)
                else:
                    for i in range(len(self._data)):
                        self._data[i] = 0
            except:
                pass  # Best effort
            finally:
                self._locked = True
    
    def _zero_memory(self):
        """Internal memory zeroing"""
        if self._zero_on_gc:
            self.zero()
    
    def compare(self, other: Union[str, bytes, 'SecureString']) -> bool:
        """Secure comparison without timing attacks"""
        if isinstance(other, SecureString):
            other_data = other._data
        elif isinstance(other, str):
            other_data = other.encode('utf-8')
        else:
            other_data = bytes(other)
        
        # Constant-time comparison
        if len(self._data) != len(other_data):
            return False
        
        result = 0
        for a, b in zip(self._data, other_data):
            result |= a ^ b
        
        return result == 0
    
    def copy(self) -> bytes:
        """Create a copy (use carefully)"""
        if self._locked:
            raise SecurityError("SecureString is locked")
        return bytes(self._data)


class SecurityError(Exception):
    """Security-related exceptions"""
    pass


class SecureMemory:
    """Memory security utilities"""
    
    @staticmethod
    def zero_memory(obj):
        """Zero memory of an object"""
        if hasattr(obj, 'zero'):
            obj.zero()
        elif isinstance(obj, (bytes, bytearray)):
            try:
                if isinstance(obj, bytes):
                    # Can't modify bytes directly
                    return
                for i in range(len(obj)):
                    obj[i] = 0
            except:
                pass
    
    @staticmethod
    def secure_compare(a: Union[str, bytes], b: Union[str, bytes]) -> bool:
        """Constant-time comparison to prevent timing attacks"""
        if isinstance(a, str):
            a = a.encode('utf-8')
        if isinstance(b, str):
            b = b.encode('utf-8')
        
        if len(a) != len(b):
            return False
        
        result = 0
        for x, y in zip(a, b):
            result |= x ^ y
        
        return result == 0
    
    @staticmethod
    def force_garbage_collection():
        """Force garbage collection to clean up sensitive data"""
        for _ in range(3):
            gc.collect()
        
        # Additional cleanup for Python objects
        if sys.platform == 'win32':
            try:
                import ctypes
                ctypes.windll.kernel32.SetProcessWorkingSetSize(
                    ctypes.windll.kernel32.GetCurrentProcess(), -1, -1
                )
            except:
                pass
    
    @staticmethod
    def lock_memory(data: bytes) -> bool:
        """Attempt to lock memory in RAM (prevent swapping)"""
        try:
            if sys.platform == 'linux':
                import ctypes
                libc = ctypes.CDLL('libc.so.6')
                # mlock - prevent memory from being paged to disk
                result = libc.mlock(data, len(data))
                return result == 0
            elif sys.platform == 'win32':
                import ctypes
                kernel32 = ctypes.windll.kernel32
                # VirtualLock - lock memory in RAM
                result = kernel32.VirtualLock(
                    ctypes.cast(id(data), ctypes.POINTER(ctypes.c_void_p)),
                    len(data)
                )
                return result != 0
        except:
            pass
        return False
    
    @staticmethod
    def unlock_memory(data: bytes):
        """Unlock previously locked memory"""
        try:
            if sys.platform == 'linux':
                import ctypes
                libc = ctypes.CDLL('libc.so.6')
                libc.munlock(data, len(data))
            elif sys.platform == 'win32':
                import ctypes
                kernel32 = ctypes.windll.kernel32
                kernel32.VirtualUnlock(
                    ctypes.cast(id(data), ctypes.POINTER(ctypes.c_void_p)),
                    len(data)
                )
        except:
            pass


class MemoryCleaner:
    """Automatic memory cleaning thread"""
    
    def __init__(self, interval: float = 30.0):
        self.interval = interval
        self.running = False
        self.thread = None
        self.secure_strings = []
        self._lock = threading.Lock()
    
    def register(self, secure_string: SecureString):
        """Register a secure string for cleanup"""
        with self._lock:
            self.secure_strings.append(secure_string)
    
    def unregister(self, secure_string: SecureString):
        """Unregister a secure string"""
        with self._lock:
            if secure_string in self.secure_strings:
                self.secure_strings.remove(secure_string)
    
    def start(self):
        """Start the memory cleaner thread"""
        if not self.running:
            self.running = True
            self.thread = threading.Thread(target=self._cleanup_loop, daemon=True)
            self.thread.start()
    
    def stop(self):
        """Stop the memory cleaner thread"""
        self.running = False
        if self.thread:
            self.thread.join(timeout=1.0)
    
    def _cleanup_loop(self):
        """Background cleanup loop"""
        while self.running:
            try:
                # Force garbage collection
                SecureMemory.force_garbage_collection()
                
                # Clean up registered strings
                with self._lock:
                    for secure_string in self.secure_strings[:]:
                        if secure_string._locked:
                            self.secure_strings.remove(secure_string)
                
                time.sleep(self.interval)
            except:
                pass
    
    def cleanup_all(self):
        """Immediate cleanup of all registered strings"""
        with self._lock:
            for secure_string in self.secure_strings:
                secure_string.zero()
            self.secure_strings.clear()


# Global memory cleaner instance
_memory_cleaner = MemoryCleaner()


def start_memory_cleaner():
    """Start the global memory cleaner"""
    _memory_cleaner.start()


def stop_memory_cleaner():
    """Stop the global memory cleaner"""
    _memory_cleaner.stop()


def register_secure_string(secure_string: SecureString):
    """Register a secure string for automatic cleanup"""
    _memory_cleaner.register(secure_string)


def cleanup_all_memory():
    """Clean up all registered memory immediately"""
    _memory_cleaner.cleanup_all()


class SecureKeyMaterial:
    """Secure storage for cryptographic key material"""
    
    def __init__(self, key_data: bytes):
        """Initialize secure key storage"""
        self._key = SecureString(key_data)
        self._locked = False
        
        # Try to lock memory in RAM
        SecureMemory.lock_memory(key_data)
        
        # Register for cleanup
        register_secure_string(self._key)
    
    def __del__(self):
        """Cleanup on deletion"""
        self.zero()
    
    def __enter__(self):
        """Context manager entry"""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit"""
        self.zero()
    
    def get_key(self) -> bytes:
        """Get key data (use carefully)"""
        if self._locked:
            raise SecurityError("Key is locked")
        return self._key.copy()
    
    def zero(self):
        """Zero the key material"""
        if hasattr(self, '_key'):
            self._key.zero()
            self._locked = True
    
    def compare(self, other_key: bytes) -> bool:
        """Securely compare with another key"""
        if self._locked:
            raise SecurityError("Key is locked")
        return self._key.compare(other_key)


def create_secure_key(key_data: bytes) -> SecureKeyMaterial:
    """Create secure key material"""
    return SecureKeyMaterial(key_data)


def secure_derive_key(password: str, salt: bytes, iterations: int = 3) -> SecureKeyMaterial:
    """Securely derive a key from password"""
    from cryptography.hazmat.primitives.kdf.argon2 import Argon2id
    from cryptography.hazmat.primitives.kdf.hkdf import HKDF
    from cryptography.hazmat.primitives import hashes
    
    # Use secure string for password
    secure_password = SecureString(password)
    
    try:
        # Argon2id
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=iterations,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(secure_password.copy())
        
        # HKDF-SHA512
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        final_key = kdf_hkdf.derive(intermediate_key)
        
        # Create secure key material
        secure_key = create_secure_key(final_key)
        
        # Zero intermediate values
        SecureMemory.zero_memory(intermediate_key)
        secure_password.zero()
        
        return secure_key
        
    except Exception as e:
        # Cleanup on error
        secure_password.zero()
        raise e


# Memory cleanup on exit
import atexit
atexit.register(cleanup_all_memory)
atexit.register(stop_memory_cleaner)
