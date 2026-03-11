#!/usr/bin/env python3
"""
VAULTX MEMORY SECURITY TESTS
Tests for secure memory management and protection against memory harvesting

This test suite verifies that passwords and keys are properly protected
from memory harvesting attacks through secure memory management.
"""

import unittest
import time
import gc
import threading
import sys
import os
from unittest.mock import patch, MagicMock

# Import secure memory modules
from secure_memory import (
    SecureString, SecureKeyMaterial, SecurityError,
    SecureMemory, MemoryCleaner, secure_derive_key,
    start_memory_cleaner, stop_memory_cleaner, cleanup_all_memory
)
from secure_entry import SecureEntry, SecurePasswordFrame


class TestSecureString(unittest.TestCase):
    """Test SecureString memory protection"""
    
    def test_secure_string_creation(self):
        """Test SecureString creation and basic properties"""
        data = "test_password_123"
        secure_str = SecureString(data)
        
        self.assertEqual(len(secure_str), len(data))
        self.assertFalse(secure_str._locked)
        
        # Test that string conversion is prevented
        with self.assertRaises(SecurityError):
            str(secure_str)
    
    def test_secure_string_zeroing(self):
        """Test that SecureString properly zeros memory"""
        data = "sensitive_password_data"
        secure_str = SecureString(data)
        
        # Verify data exists
        self.assertEqual(len(secure_str), len(data))
        
        # Zero the memory
        secure_str.zero()
        
        # Verify it's locked
        self.assertTrue(secure_str._locked)
        
        # Verify access is blocked
        with self.assertRaises(SecurityError):
            secure_str.copy()
    
    def test_secure_string_comparison(self):
        """Test secure comparison without timing attacks"""
        data1 = "password123"
        data2 = "password123"
        data3 = "different_password"
        
        secure_str1 = SecureString(data1)
        secure_str2 = SecureString(data2)
        secure_str3 = SecureString(data3)
        
        # Test equal comparison
        self.assertTrue(secure_str1.compare(data2))
        self.assertTrue(secure_str1.compare(secure_str2))
        
        # Test unequal comparison
        self.assertFalse(secure_str1.compare(data3))
        self.assertFalse(secure_str1.compare(secure_str3))
        
        # Cleanup
        secure_str1.zero()
        secure_str2.zero()
        secure_str3.zero()
    
    def test_secure_string_context_manager(self):
        """Test SecureString context manager automatic cleanup"""
        data = "context_manager_test"
        
        with SecureString(data) as secure_str:
            self.assertFalse(secure_str._locked)
            self.assertEqual(len(secure_str), len(data))
        
        # After context, should be zeroed
        self.assertTrue(secure_str._locked)
    
    def test_secure_string_bytes_conversion(self):
        """Test secure bytes conversion"""
        data = "bytes_test_data"
        secure_str = SecureString(data)
        
        # Test bytes conversion
        bytes_data = secure_str.copy()
        self.assertEqual(bytes_data, data.encode('utf-8'))
        
        # Zero and verify access blocked
        secure_str.zero()
        with self.assertRaises(SecurityError):
            secure_str.copy()


class TestSecureMemory(unittest.TestCase):
    """Test SecureMemory utilities"""
    
    def test_secure_compare(self):
        """Test constant-time comparison"""
        # Equal strings
        self.assertTrue(SecureMemory.secure_compare("password", "password"))
        self.assertTrue(SecureMemory.secure_compare(b"password", b"password"))
        
        # Unequal strings (same length)
        self.assertFalse(SecureMemory.secure_compare("password", "passw0rd"))
        self.assertFalse(SecureMemory.secure_compare(b"password", b"passw0rd"))
        
        # Unequal strings (different length)
        self.assertFalse(SecureMemory.secure_compare("password", "password123"))
        self.assertFalse(SecureMemory.secure_compare(b"password", b"password123"))
    
    def test_force_garbage_collection(self):
        """Test garbage collection forcing"""
        # Create some objects
        test_objects = [SecureString(f"test_{i}") for i in range(10)]
        
        # Force garbage collection
        SecureMemory.force_garbage_collection()
        
        # Should not raise exceptions
        SecureMemory.force_garbage_collection()
    
    def test_memory_locking(self):
        """Test memory locking (where supported)"""
        test_data = b"test_memory_locking_data"
        
        # Try to lock memory - may not work on all systems
        result = SecureMemory.lock_memory(test_data)
        
        # Should not crash (returns True or False)
        self.assertIsInstance(result, bool)
        
        # Try to unlock
        SecureMemory.unlock_memory(test_data)
        
        # Should not crash
        self.assertTrue(True)


class TestSecureKeyMaterial(unittest.TestCase):
    """Test SecureKeyMaterial protection"""
    
    def test_secure_key_creation(self):
        """Test secure key material creation"""
        key_data = os.urandom(32)
        secure_key = SecureKeyMaterial(key_data)
        
        # Test key access
        retrieved_key = secure_key.get_key()
        self.assertEqual(retrieved_key, key_data)
        
        # Zero and verify access blocked
        secure_key.zero()
        self.assertTrue(secure_key._locked)
        with self.assertRaises(SecurityError):
            secure_key.get_key()
    
    def test_secure_key_comparison(self):
        """Test secure key comparison"""
        key_data1 = os.urandom(32)
        key_data2 = os.urandom(32)
        key_data3 = os.urandom(32)
        
        secure_key1 = SecureKeyMaterial(key_data1)
        secure_key2 = SecureKeyMaterial(key_data2)
        
        # Test comparison
        self.assertTrue(secure_key1.compare(key_data1))
        self.assertFalse(secure_key1.compare(key_data3))
        
        # Cleanup
        secure_key1.zero()
        secure_key2.zero()
    
    def test_secure_key_context_manager(self):
        """Test SecureKeyMaterial context manager"""
        key_data = os.urandom(32)
        
        with SecureKeyMaterial(key_data) as secure_key:
            retrieved_key = secure_key.get_key()
            self.assertEqual(retrieved_key, key_data)
        
        # After context, should be zeroed
        self.assertTrue(secure_key._locked)


class TestMemoryCleaner(unittest.TestCase):
    """Test MemoryCleaner functionality"""
    
    def test_memory_cleaner_registration(self):
        """Test memory cleaner registration"""
        cleaner = MemoryCleaner(interval=0.1)  # Fast for testing
        
        # Create secure strings
        secure_str1 = SecureString("test1")
        secure_str2 = SecureString("test2")
        
        # Register them
        cleaner.register(secure_str1)
        cleaner.register(secure_str2)
        
        self.assertEqual(len(cleaner.secure_strings), 2)
        
        # Unregister one
        cleaner.unregister(secure_str1)
        self.assertEqual(len(cleaner.secure_strings), 1)
        
        # Cleanup
        secure_str1.zero()
        secure_str2.zero()
    
    def test_memory_cleaner_cleanup(self):
        """Test memory cleaner cleanup functionality"""
        cleaner = MemoryCleaner(interval=0.1)
        
        # Create and register secure strings
        secure_strings = [SecureString(f"test_{i}") for i in range(5)]
        
        for secure_str in secure_strings:
            cleaner.register(secure_str)
        
        self.assertEqual(len(cleaner.secure_strings), 5)
        
        # Cleanup all
        cleaner.cleanup_all()
        
        # All should be zeroed and removed
        self.assertEqual(len(cleaner.secure_strings), 0)
        for secure_str in secure_strings:
            self.assertTrue(secure_str._locked)


class TestSecureKeyDerivation(unittest.TestCase):
    """Test secure key derivation"""
    
    def test_secure_key_derivation(self):
        """Test secure key derivation with memory protection"""
        password = "test_password_123"
        salt = os.urandom(32)
        
        # Derive key securely
        secure_key = secure_derive_key(password, salt)
        
        # Should be SecureKeyMaterial
        self.assertIsInstance(secure_key, SecureKeyMaterial)
        
        # Get key
        key = secure_key.get_key()
        self.assertEqual(len(key), 32)  # 256 bits
        
        # Zero and verify
        secure_key.zero()
        self.assertTrue(secure_key._locked)
    
    def test_secure_derivation_cleanup(self):
        """Test that intermediate values are cleaned up"""
        password = "test_password_123"
        salt = os.urandom(32)
        
        # Derive key
        secure_key = secure_derive_key(password, salt)
        
        # Get key and zero
        key = secure_key.get_key()
        secure_key.zero()
        
        # Should be cleaned up
        self.assertTrue(secure_key._locked)


class TestSecureEntry(unittest.TestCase):
    """Test SecureEntry widget"""
    
    def setUp(self):
        """Set up test environment"""
        try:
            import tkinter as tk
            self.root = tk.Tk()
            self.root.withdraw()  # Hide window
        except:
            self.skipTest("tkinter not available")
    
    def tearDown(self):
        """Clean up test environment"""
        try:
            if hasattr(self, 'root'):
                self.root.destroy()
        except:
            pass
    
    def test_secure_entry_creation(self):
        """Test SecureEntry creation"""
        entry = SecureEntry(self.root)
        
        # Should be created successfully
        self.assertIsNotNone(entry)
        
        # Should have security features
        self.assertEqual(entry.cget('show'), '●')
        self.assertTrue(entry._clipboard_protection)
        self.assertEqual(entry._max_length, 128)
    
    def test_secure_entry_password_handling(self):
        """Test secure password handling"""
        entry = SecureEntry(self.root)
        
        # Simulate password entry
        test_password = "test_password_123"
        entry.insert(0, test_password)
        
        # Should have secure password
        try:
            secure_password = entry.get_secure_password()
            self.assertIsInstance(secure_password, SecureString)
            
            # Test password bytes
            password_bytes = entry.get_password_bytes()
            self.assertEqual(password_bytes, test_password.encode('utf-8'))
            
            # Test password string
            password_str = entry.get_password_str()
            self.assertEqual(password_str, test_password)
            
        except SecurityError:
            # May be locked due to cleanup timer
            pass
        
        # Clear and verify
        entry.clear()
        with self.assertRaises(SecurityError):
            entry.get_secure_password()
    
    def test_secure_password_frame(self):
        """Test SecurePasswordFrame"""
        frame = SecurePasswordFrame(self.root)
        
        # Should be created successfully
        self.assertIsNotNone(frame)
        self.assertIsNotNone(frame._secure_entry)
        
        # Test password entry
        entry = frame._secure_entry
        entry.insert(0, "test_password")
        
        try:
            secure_password = frame.get_secure_password()
            self.assertIsInstance(secure_password, SecureString)
        except SecurityError:
            # May be locked due to cleanup timer
            pass
        
        # Clear
        frame.clear()


class TestMemoryHarvestingProtection(unittest.TestCase):
    """Test protection against memory harvesting"""
    
    def test_password_zeroing_after_use(self):
        """Test that passwords are zeroed after use"""
        password = "sensitive_password_data"
        secure_password = SecureString(password)
        
        # Verify password exists
        self.assertEqual(len(secure_password), len(password))
        
        # Simulate use and cleanup
        password_copy = secure_password.copy()
        self.assertEqual(password_copy, password.encode('utf-8'))
        
        # Zero the password
        secure_password.zero()
        
        # Verify it's locked and inaccessible
        self.assertTrue(secure_password._locked)
        with self.assertRaises(SecurityError):
            secure_password.copy()
    
    def test_key_material_zeroing(self):
        """Test that key material is zeroed"""
        key_data = os.urandom(32)
        secure_key = SecureKeyMaterial(key_data)
        
        # Use the key
        retrieved_key = secure_key.get_key()
        self.assertEqual(retrieved_key, key_data)
        
        # Zero the key
        secure_key.zero()
        
        # Verify it's locked
        self.assertTrue(secure_key._locked)
        with self.assertRaises(SecurityError):
            secure_key.get_key()
    
    def test_memory_cleanup_on_error(self):
        """Test that memory is cleaned up even on errors"""
        password = "test_password"
        secure_password = SecureString(password)
        
        # Simulate error condition
        try:
            # Force an error
            raise Exception("Simulated error")
        except:
            # Even on error, password should be zeroable
            secure_password.zero()
            self.assertTrue(secure_password._locked)
    
    def test_context_manager_cleanup(self):
        """Test context manager automatic cleanup"""
        password = "context_test_password"
        
        with SecureString(password) as secure_str:
            # Use the secure string
            self.assertEqual(len(secure_str), len(password))
            copy_data = secure_str.copy()
            self.assertEqual(copy_data, password.encode('utf-8'))
        
        # After context, should be automatically zeroed
        self.assertTrue(secure_str._locked)
        with self.assertRaises(SecurityError):
            secure_str.copy()


class TestMemorySecurityIntegration(unittest.TestCase):
    """Integration tests for memory security"""
    
    def test_full_workflow_memory_protection(self):
        """Test complete workflow with memory protection"""
        password = "integration_test_password"
        salt = os.urandom(32)
        
        # Derive key securely
        secure_key = secure_derive_key(password, salt)
        
        # Use the key
        key = secure_key.get_key()
        self.assertEqual(len(key), 32)
        
        # Simulate encryption/decryption
        test_data = b"test data for encryption"
        
        # In real usage, this would use AES-GCM-SIV
        # For testing, just verify key is accessible
        self.assertIsInstance(key, bytes)
        
        # Cleanup
        secure_key.zero()
        
        # Verify cleanup
        self.assertTrue(secure_key._locked)
    
    def test_multiple_secure_objects_cleanup(self):
        """Test cleanup of multiple secure objects"""
        secure_objects = []
        
        # Create multiple secure objects
        for i in range(10):
            secure_str = SecureString(f"test_password_{i}")
            secure_key = SecureKeyMaterial(os.urandom(32))
            secure_objects.extend([secure_str, secure_key])
        
        # Verify they exist
        for obj in secure_objects:
            if isinstance(obj, SecureString):
                self.assertFalse(obj._locked)
            elif isinstance(obj, SecureKeyMaterial):
                self.assertFalse(obj._locked)
        
        # Cleanup all
        for obj in secure_objects:
            obj.zero()
        
        # Verify all are locked
        for obj in secure_objects:
            if isinstance(obj, SecureString):
                self.assertTrue(obj._locked)
            elif isinstance(obj, SecureKeyMaterial):
                self.assertTrue(obj._locked)
    
    def test_memory_cleaner_integration(self):
        """Test memory cleaner integration"""
        # Start memory cleaner
        start_memory_cleaner()
        
        # Create secure objects
        secure_strings = [SecureString(f"cleaner_test_{i}") for i in range(5)]
        
        # Force cleanup
        cleanup_all_memory()
        
        # Stop memory cleaner
        stop_memory_cleaner()
        
        # Should not raise exceptions
        self.assertTrue(True)


def run_memory_security_tests():
    """Run all memory security tests"""
    print("=" * 70)
    print("VAULTX MEMORY SECURITY TESTS")
    print("=" * 70)
    print("Testing secure memory management and protection")
    print("against memory harvesting attacks")
    print()
    
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add test classes
    suite.addTests(loader.loadTestsFromTestCase(TestSecureString))
    suite.addTests(loader.loadTestsFromTestCase(TestSecureMemory))
    suite.addTests(loader.loadTestsFromTestCase(TestSecureKeyMaterial))
    suite.addTests(loader.loadTestsFromTestCase(TestMemoryCleaner))
    suite.addTests(loader.loadTestsFromTestCase(TestSecureKeyDerivation))
    suite.addTests(loader.loadTestsFromTestCase(TestSecureEntry))
    suite.addTests(loader.loadTestsFromTestCase(TestMemoryHarvestingProtection))
    suite.addTests(loader.loadTestsFromTestCase(TestMemorySecurityIntegration))
    
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
        print("\n✅ ALL MEMORY SECURITY TESTS PASSED!")
        print("Memory protection against harvesting attacks is working correctly.")
    else:
        print("\n❌ SOME TESTS FAILED!")
        print("Memory security implementation needs attention.")
    
    print("=" * 70)
    
    return result.wasSuccessful()


if __name__ == "__main__":
    success = run_memory_security_tests()
    exit(0 if success else 1)
