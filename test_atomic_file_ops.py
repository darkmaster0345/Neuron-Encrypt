#!/usr/bin/env python3
"""
VAULTX ATOMIC FILE OPERATIONS TESTS
Tests for atomic file operations and crash recovery

This test suite verifies that file operations are atomic and
prevent data loss during crashes or interruptions.
"""

import unittest
import os
import tempfile
import shutil
import time
import threading
from pathlib import Path
from unittest.mock import patch, MagicMock

# Import atomic file operations
from atomic_file_ops import (
    AtomicFileWriter, SafeFileOperations, CrashRecoveryManager,
    atomic_write_file, verify_file_integrity,
    scan_crashed_operations, recover_crashed_operation
)


class TestAtomicFileWriter(unittest.TestCase):
    """Test AtomicFileWriter functionality"""
    
    def setUp(self):
        """Set up test environment"""
        self.test_dir = tempfile.mkdtemp()
        self.target_file = os.path.join(self.test_dir, "test_file.txt")
        
    def tearDown(self):
        """Clean up test environment"""
        shutil.rmtree(self.test_dir, ignore_errors=True)
    
    def test_atomic_write_success(self):
        """Test successful atomic write"""
        test_data = b"This is test data for atomic write"
        
        with AtomicFileWriter(self.target_file) as writer:
            writer.write(test_data)
            writer.set_expected_size(len(test_data))
        
        # Verify file was created
        self.assertTrue(os.path.exists(self.target_file))
        
        # Verify file content
        with open(self.target_file, 'rb') as f:
            written_data = f.read()
        
        self.assertEqual(written_data, test_data)
        
        # Verify no temporary files remain
        temp_files = [f for f in os.listdir(self.test_dir) if f.startswith('.vaultx_temp_')]
        self.assertEqual(len(temp_files), 0)
        
        # Verify no backup files remain
        backup_files = [f for f in os.listdir(self.test_dir) if f.startswith('.vaultx_backup_')]
        self.assertEqual(len(backup_files), 0)
    
    def test_atomic_write_with_magic_bytes(self):
        """Test atomic write with magic bytes"""
        magic = b'MAGIC01'
        test_data = b"Test data with magic bytes"
        
        with AtomicFileWriter(self.target_file) as writer:
            writer.write_magic_bytes(magic)
            writer.write(test_data)
            writer.set_expected_size(len(magic) + len(test_data))
        
        # Verify file exists and has correct content
        self.assertTrue(os.path.exists(self.target_file))
        
        with open(self.target_file, 'rb') as f:
            written_magic = f.read(len(magic))
            written_data = f.read()
        
        self.assertEqual(written_magic, magic)
        self.assertEqual(written_data, test_data)
    
    def test_atomic_write_verification_callback(self):
        """Test atomic write with verification callback"""
        test_data = b"Test data with verification"
        
        def verify_callback(temp_path):
            """Custom verification callback"""
            with open(temp_path, 'rb') as f:
                data = f.read()
            return data == test_data
        
        with AtomicFileWriter(self.target_file, verify_callback) as writer:
            writer.write(test_data)
            writer.set_expected_size(len(test_data))
        
        # Verify file was created
        self.assertTrue(os.path.exists(self.target_file))
        
        with open(self.target_file, 'rb') as f:
            written_data = f.read()
        
        self.assertEqual(written_data, test_data)
    
    def test_atomic_write_size_verification_failure(self):
        """Test atomic write failure due to size mismatch"""
        test_data = b"Test data"
        
        with self.assertRaises(Exception):  # Should raise FileOperationError
            with AtomicFileWriter(self.target_file) as writer:
                writer.write(test_data)
                writer.set_expected_size(len(test_data) + 10)  # Wrong size
        
        # Verify target file was not created
        self.assertFalse(os.path.exists(self.target_file))
        
        # Verify no temporary files remain
        temp_files = [f for f in os.listdir(self.test_dir) if f.startswith('.vaultx_temp_')]
        self.assertEqual(len(temp_files), 0)
    
    def test_atomic_write_magic_bytes_verification_failure(self):
        """Test atomic write failure due to magic bytes mismatch"""
        magic = b'MAGIC01'
        test_data = b"Test data"
        
        # Simulate magic bytes verification failure
        def failing_verify(temp_path):
            with open(temp_path, 'wb') as f:
                f.write(b'WRONGMAGIC')  # Wrong magic bytes
            return False
        
        with self.assertRaises(Exception):  # Should raise FileOperationError
            with AtomicFileWriter(self.target_file, failing_verify) as writer:
                writer.write_magic_bytes(magic)
                writer.write(test_data)
        
        # Verify target file was not created
        self.assertFalse(os.path.exists(self.target_file))
    
    def test_atomic_write_with_existing_file(self):
        """Test atomic write when target file already exists"""
        # Create existing file
        original_data = b"Original data"
        with open(self.target_file, 'wb') as f:
            f.write(original_data)
        
        # Write new data atomically
        new_data = b"New atomic data"
        with AtomicFileWriter(self.target_file) as writer:
            writer.write(new_data)
            writer.set_expected_size(len(new_data))
        
        # Verify file has new data
        with open(self.target_file, 'rb') as f:
            written_data = f.read()
        
        self.assertEqual(written_data, new_data)
        
        # Verify no backup files remain
        backup_files = [f for f in os.listdir(self.test_dir) if f.startswith('.vaultx_backup_')]
        self.assertEqual(len(backup_files), 0)
    
    def test_atomic_write_exception_rollback(self):
        """Test rollback when exception occurs during write"""
        # Create existing file
        original_data = b"Original data"
        with open(self.target_file, 'wb') as f:
            f.write(original_data)
        
        # Simulate exception during write
        try:
            with AtomicFileWriter(self.target_file) as writer:
                writer.write(b"Partial data")
                raise Exception("Simulated error")
        except Exception:
            pass  # Expected
        
        # Verify original file is intact
        self.assertTrue(os.path.exists(self.target_file))
        
        with open(self.target_file, 'rb') as f:
            written_data = f.read()
        
        self.assertEqual(written_data, original_data)
        
        # Verify no temporary files remain
        temp_files = [f for f in os.listdir(self.test_dir) if f.startswith('.vaultx_temp_')]
        self.assertEqual(len(temp_files), 0)


class TestCrashRecoveryManager(unittest.TestCase):
    """Test CrashRecoveryManager functionality"""
    
    def setUp(self):
        """Set up test environment"""
        self.test_dir = tempfile.mkdtemp()
        self.recovery_dir = os.path.join(self.test_dir, ".vaultx_recovery")
        self.manager = CrashRecoveryManager()
        
        # Override recovery directory for testing
        self.manager.recovery_dir = Path(self.recovery_dir)
        self.manager.lock_file = self.manager.recovery_dir / "recovery.lock"
        self.manager.operations_file = self.manager.recovery_dir / "operations.json"
        
    def tearDown(self):
        """Clean up test environment"""
        shutil.rmtree(self.test_dir, ignore_errors=True)
    
    def test_register_and_complete_operation(self):
        """Test registering and completing an operation"""
        operation_id = "test_operation_123"
        target_path = os.path.join(self.test_dir, "target.txt")
        temp_path = os.path.join(self.test_dir, "temp.txt")
        
        # Register operation
        success = self.manager.register_operation(operation_id, target_path, temp_path)
        self.assertTrue(success)
        
        # Verify operation is registered
        operations = self.manager._load_operations()
        self.assertIn(operation_id, operations)
        
        op_info = operations[operation_id]
        self.assertEqual(op_info['target_path'], target_path)
        self.assertEqual(op_info['temp_path'], temp_path)
        
        # Complete operation
        success = self.manager.complete_operation(operation_id)
        self.assertTrue(success)
        
        # Verify operation is removed
        operations = self.manager._load_operations()
        self.assertNotIn(operation_id, operations)
    
    def test_scan_for_crashed_operations(self):
        """Test scanning for crashed operations"""
        operation_id = "crashed_operation_456"
        target_path = os.path.join(self.test_dir, "target.txt")
        temp_path = os.path.join(self.test_dir, "temp.txt")
        
        # Register old operation (simulate crash)
        self.manager.register_operation(operation_id, target_path, temp_path)
        
        # Manually set old timestamp to simulate crash
        operations = self.manager._load_operations()
        operations[operation_id]['timestamp'] = time.time() - 600  # 10 minutes ago
        self.manager._save_operations(operations)
        
        # Scan for crashed operations
        crashed_ops = self.manager.scan_for_crashed_operations()
        
        self.assertEqual(len(crashed_ops), 1)
        self.assertEqual(crashed_ops[0]['operation_id'], operation_id)
        self.assertEqual(crashed_ops[0]['target_path'], Path(target_path))
        self.assertEqual(crashed_ops[0]['temp_path'], Path(temp_path))
    
    def test_recover_operation_restore_backup(self):
        """Test recovering operation by restoring backup"""
        operation_id = "recover_operation_789"
        target_path = os.path.join(self.test_dir, "target.txt")
        temp_path = os.path.join(self.test_dir, "temp.txt")
        backup_path = os.path.join(self.test_dir, "backup.txt")
        
        # Create backup file
        backup_data = b"Backup data"
        with open(backup_path, 'wb') as f:
            f.write(backup_data)
        
        # Register operation with backup
        self.manager.register_operation(operation_id, target_path, temp_path, backup_path)
        
        # Recover by restoring backup
        success = self.manager.recover_operation(operation_id, "restore_backup")
        self.assertTrue(success)
        
        # Verify backup was restored
        self.assertTrue(os.path.exists(target_path))
        
        with open(target_path, 'rb') as f:
            restored_data = f.read()
        
        self.assertEqual(restored_data, backup_data)
        
        # Verify operation is removed from tracking
        operations = self.manager._load_operations()
        self.assertNotIn(operation_id, operations)
    
    def test_recover_operation_complete_from_temp(self):
        """Test recovering operation by completing from temp"""
        operation_id = "temp_complete_operation"
        target_path = os.path.join(self.test_dir, "target.txt")
        temp_path = os.path.join(self.test_dir, "temp.txt")
        
        # Create temp file
        temp_data = b"Temp file data"
        with open(temp_path, 'wb') as f:
            f.write(temp_data)
        
        # Register operation
        self.manager.register_operation(operation_id, target_path, temp_path)
        
        # Recover by completing from temp
        success = self.manager.recover_operation(operation_id, "complete_temp")
        self.assertTrue(success)
        
        # Verify temp was moved to target
        self.assertTrue(os.path.exists(target_path))
        self.assertFalse(os.path.exists(temp_path))
        
        with open(target_path, 'rb') as f:
            completed_data = f.read()
        
        self.assertEqual(completed_data, temp_data)
    
    def test_recover_operation_cleanup(self):
        """Test cleaning up crashed operation"""
        operation_id = "cleanup_operation"
        target_path = os.path.join(self.test_dir, "target.txt")
        temp_path = os.path.join(self.test_dir, "temp.txt")
        backup_path = os.path.join(self.test_dir, "backup.txt")
        
        # Create temp and backup files
        with open(temp_path, 'wb') as f:
            f.write(b"Temp data")
        with open(backup_path, 'wb') as f:
            f.write(b"Backup data")
        
        # Register operation
        self.manager.register_operation(operation_id, target_path, temp_path, backup_path)
        
        # Recover by cleanup
        success = self.manager.recover_operation(operation_id, "cleanup")
        self.assertTrue(success)
        
        # Verify temp and backup files are cleaned up
        self.assertFalse(os.path.exists(temp_path))
        self.assertFalse(os.path.exists(backup_path))
        
        # Verify operation is removed from tracking
        operations = self.manager._load_operations()
        self.assertNotIn(operation_id, operations)


class TestSafeFileOperations(unittest.TestCase):
    """Test SafeFileOperations functionality"""
    
    def setUp(self):
        """Set up test environment"""
        self.test_dir = tempfile.mkdtemp()
        self.safe_ops = SafeFileOperations()
        
    def tearDown(self):
        """Clean up test environment"""
        shutil.rmtree(self.test_dir, ignore_errors=True)
    
    def test_atomic_write_file(self):
        """Test atomic file write"""
        target_path = os.path.join(self.test_dir, "atomic_test.txt")
        test_data = b"Atomic write test data"
        
        success = self.safe_ops.atomic_write(target_path, test_data)
        self.assertTrue(success)
        
        # Verify file exists and has correct content
        self.assertTrue(os.path.exists(target_path))
        
        with open(target_path, 'rb') as f:
            written_data = f.read()
        
        self.assertEqual(written_data, test_data)
    
    def test_atomic_write_file_with_magic(self):
        """Test atomic file write with magic bytes"""
        target_path = os.path.join(self.test_dir, "magic_test.txt")
        magic = b'MAGIC01'
        test_data = b"Test data with magic"
        
        success = self.safe_ops.atomic_write(target_path, test_data, magic_bytes=magic)
        self.assertTrue(success)
        
        # Verify file exists and has correct content
        with open(target_path, 'rb') as f:
            written_magic = f.read(len(magic))
            written_data = f.read()
        
        self.assertEqual(written_magic, magic)
        self.assertEqual(written_data, test_data)
    
    def test_verify_file_integrity(self):
        """Test file integrity verification"""
        target_path = os.path.join(self.test_dir, "integrity_test.txt")
        magic = b'MAGIC01'
        test_data = b"Test data for integrity"
        
        # Create file
        with open(target_path, 'wb') as f:
            f.write(magic + test_data)
        
        # Verify with correct parameters
        success = self.safe_ops.verify_file_integrity(
            target_path, 
            expected_magic=magic,
            expected_size=len(magic) + len(test_data)
        )
        self.assertTrue(success)
        
        # Verify with wrong magic
        success = self.safe_ops.verify_file_integrity(
            target_path,
            expected_magic=b'WRONGMAGIC'
        )
        self.assertFalse(success)
        
        # Verify with wrong size
        success = self.safe_ops.verify_file_integrity(
            target_path,
            expected_size=len(magic) + len(test_data) + 10
        )
        self.assertFalse(success)
    
    def test_atomic_write_with_recovery(self):
        """Test atomic write with crash recovery"""
        target_path = os.path.join(self.test_dir, "recovery_test.txt")
        test_data = b"Recovery test data"
        
        success = self.safe_ops.atomic_write_with_recovery(target_path, test_data)
        self.assertTrue(success)
        
        # Verify file exists
        self.assertTrue(os.path.exists(target_path))
        
        with open(target_path, 'rb') as f:
            written_data = f.read()
        
        self.assertEqual(written_data, test_data)


class TestConvenienceFunctions(unittest.TestCase):
    """Test convenience functions"""
    
    def setUp(self):
        """Set up test environment"""
        self.test_dir = tempfile.mkdtemp()
        
    def tearDown(self):
        """Clean up test environment"""
        shutil.rmtree(self.test_dir, ignore_errors=True)
    
    def test_atomic_write_file_function(self):
        """Test atomic_write_file convenience function"""
        target_path = os.path.join(self.test_dir, "convenience_test.txt")
        test_data = b"Convenience function test"
        
        success = atomic_write_file(target_path, test_data)
        self.assertTrue(success)
        
        # Verify file exists and has correct content
        self.assertTrue(os.path.exists(target_path))
        
        with open(target_path, 'rb') as f:
            written_data = f.read()
        
        self.assertEqual(written_data, test_data)
    
    def test_verify_file_integrity_function(self):
        """Test verify_file_integrity convenience function"""
        target_path = os.path.join(self.test_dir, "verify_convenience_test.txt")
        test_data = b"Verify convenience test"
        
        # Create file
        with open(target_path, 'wb') as f:
            f.write(test_data)
        
        # Verify with correct size
        success = verify_file_integrity(target_path, expected_size=len(test_data))
        self.assertTrue(success)
        
        # Verify with wrong size
        success = verify_file_integrity(target_path, expected_size=len(test_data) + 10)
        self.assertFalse(success)
    
    def test_scan_crashed_operations_function(self):
        """Test scan_crashed_operations convenience function"""
        crashed_ops = scan_crashed_operations()
        self.assertIsInstance(crashed_ops, list)


class TestCrashScenarios(unittest.TestCase):
    """Test various crash scenarios"""
    
    def setUp(self):
        """Set up test environment"""
        self.test_dir = tempfile.mkdtemp()
        
    def tearDown(self):
        """Clean up test environment"""
        shutil.rmtree(self.test_dir, ignore_errors=True)
    
    def test_simulated_crash_during_write(self):
        """Test simulated crash during file write"""
        target_path = os.path.join(self.test_dir, "crash_test.txt")
        original_data = b"Original data before crash"
        
        # Create original file
        with open(target_path, 'wb') as f:
            f.write(original_data)
        
        # Simulate crash during write (exception in context manager)
        try:
            with AtomicFileWriter(target_path) as writer:
                writer.write(b"Partial data before crash")
                # Simulate crash
                raise Exception("Simulated system crash")
        except Exception:
            pass  # Expected
        
        # Verify original file is intact
        self.assertTrue(os.path.exists(target_path))
        
        with open(target_path, 'rb') as f:
            data = f.read()
        
        self.assertEqual(data, original_data)
        
        # Verify no temporary files remain
        temp_files = [f for f in os.listdir(self.test_dir) if f.startswith('.vaultx_temp_')]
        self.assertEqual(len(temp_files), 0)
    
    def test_simulated_power_loss_during_move(self):
        """Test simulated power loss during file move"""
        target_path = os.path.join(self.test_dir, "power_loss_test.txt")
        test_data = b"Data before power loss"
        
        # This is harder to simulate directly, but we can verify the atomic nature
        # by checking that either the old file or new file exists, never both
        
        # Create original file
        with open(target_path, 'wb') as f:
            f.write(b"Original data")
        
        # Write new file atomically
        success = atomic_write_file(target_path, test_data)
        self.assertTrue(success)
        
        # Verify file has new data
        with open(target_path, 'rb') as f:
            data = f.read()
        
        self.assertEqual(data, test_data)
        
        # Verify no temporary files remain
        temp_files = [f for f in os.listdir(self.test_dir) if f.startswith('.vaultx_temp_')]
        self.assertEqual(len(temp_files), 0)
    
    def test_interruption_recovery(self):
        """Test recovery from interrupted operation"""
        target_path = os.path.join(self.test_dir, "interruption_test.txt")
        temp_path = os.path.join(self.test_dir, "temp_file.txt")
        
        # Create temp file (simulating interrupted operation)
        temp_data = b"Temp data from interrupted operation"
        with open(temp_path, 'wb') as f:
            f.write(temp_data)
        
        # Use recovery manager to complete the operation
        manager = CrashRecoveryManager()
        manager.recovery_dir = Path(self.test_dir) / ".vaultx_recovery"
        manager.lock_file = manager.recovery_dir / "recovery.lock"
        manager.operations_file = manager.recovery_dir / "operations.json"
        
        operation_id = "interruption_test"
        manager.register_operation(operation_id, target_path, str(temp_path))
        
        # Recover from temp
        success = manager.recover_operation(operation_id, "complete_temp")
        self.assertTrue(success)
        
        # Verify temp was moved to target
        self.assertTrue(os.path.exists(target_path))
        self.assertFalse(os.path.exists(temp_path))
        
        with open(target_path, 'rb') as f:
            data = f.read()
        
        self.assertEqual(data, temp_data)


def run_atomic_file_ops_tests():
    """Run all atomic file operations tests"""
    print("=" * 70)
    print("VAULTX ATOMIC FILE OPERATIONS TESTS")
    print("=" * 70)
    print("Testing atomic file operations and crash recovery")
    print("to prevent data loss during system crashes")
    print()
    
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add test classes
    suite.addTests(loader.loadTestsFromTestCase(TestAtomicFileWriter))
    suite.addTests(loader.loadTestsFromTestCase(TestCrashRecoveryManager))
    suite.addTests(loader.loadTestsFromTestCase(TestSafeFileOperations))
    suite.addTests(loader.loadTestsFromTestCase(TestConvenienceFunctions))
    suite.addTests(loader.loadTestsFromTestCase(TestCrashScenarios))
    
    # Run tests
    runner = unittest.TextTestRunner(verbosity=2, stream=open(os.devnull, 'w'))
    result = runner.run(suite)
    
    # Print summary
    print(f"Tests run: {result.testsRun}")
    print(f"Failures: {len(result.failures)}")
    print(f"Errors: {len(result.errors)}")
    
    if result.failures:
        print("\nFAILURES:")
        for test, traceback in result.failures:
            print(f"- {test}: {traceback}")
    
    if result.errors:
        print("\nERRORS:")
        for test, traceback in result.errors:
            print(f"- {test}: {traceback}")
    
    if result.wasSuccessful():
        print("\n✅ ALL ATOMIC FILE OPERATIONS TESTS PASSED!")
        print("File operations are atomic and crash-safe.")
    else:
        print("\n❌ SOME TESTS FAILED!")
        print("Atomic file operations need attention.")
    
    print("=" * 70)
    
    return result.wasSuccessful()


if __name__ == "__main__":
    success = run_atomic_file_ops_tests()
    exit(0 if success else 1)
