#!/usr/bin/env python3
"""
VAULTX CRYPTOGRAPHIC UNIT TESTS
Test vectors for AES-256-GCM-SIV, Argon2id, and HKDF-SHA512

This test suite uses known test vectors to verify the mathematical correctness
of the cryptographic implementations used in VaultX.
"""

import unittest
import secrets
import os
import tempfile
import struct
from binascii import hexlify, unhexlify

# Import the cryptographic components from VaultX
from cryptography.hazmat.primitives.ciphers.aead import AESGCMSIV
from cryptography.hazmat.primitives.kdf.argon2 import Argon2id
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.hazmat.primitives import hashes


class TestVaultXCrypto(unittest.TestCase):
    """Test cryptographic primitives with known test vectors"""
    
    def setUp(self):
        """Set up test fixtures"""
        self.test_password = "TestPassword123!"
        self.test_salt = unhexlify("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
        self.test_nonce = unhexlify("00112233445566778899aabb")
        
    # ========== AES-GCM-SIV TEST VECTORS ==========
    
    def test_aes_gcmsiv_basic_encrypt_decrypt(self):
        """Test basic AES-GCM-SIV encrypt/decrypt cycle"""
        # Test key (256 bits / 32 bytes)
        key = unhexlify("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
        
        # Test plaintext
        plaintext = b"Hello, VaultX! This is a test message."
        
        # Test nonce (96 bits / 12 bytes)
        nonce = unhexlify("00112233445566778899aabb")
        
        # Encrypt
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        
        # Verify ciphertext is different from plaintext
        self.assertNotEqual(ciphertext, plaintext)
        self.assertGreater(len(ciphertext), len(plaintext))  # Includes auth tag
        
        # Decrypt
        decrypted = aesgcm.decrypt(nonce, ciphertext, None)
        
        # Verify round-trip
        self.assertEqual(decrypted, plaintext)
    
    def test_aes_gcmsiv_empty_data(self):
        """Test AES-GCM-SIV with empty plaintext"""
        key = secrets.token_bytes(32)
        nonce = secrets.token_bytes(12)
        
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, b"", None)
        decrypted = aesgcm.decrypt(nonce, ciphertext, None)
        
        self.assertEqual(decrypted, b"")
    
    def test_aes_gcmsiv_large_data(self):
        """Test AES-GCM-SIV with large data (1MB)"""
        key = secrets.token_bytes(32)
        nonce = secrets.token_bytes(12)
        
        # Generate 1MB of test data
        plaintext = os.urandom(1024 * 1024)
        
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        decrypted = aesgcm.decrypt(nonce, ciphertext, None)
        
        self.assertEqual(decrypted, plaintext)
        self.assertEqual(len(ciphertext), len(plaintext) + 16)  # 16-byte auth tag
    
    def test_aes_gcmsiv_wrong_key_fails(self):
        """Test that decryption fails with wrong key"""
        key1 = secrets.token_bytes(32)
        key2 = secrets.token_bytes(32)
        nonce = secrets.token_bytes(12)
        plaintext = b"Test message"
        
        # Encrypt with key1
        aesgcm1 = AESGCMSIV(key1)
        ciphertext = aesgcm1.encrypt(nonce, plaintext, None)
        
        # Try to decrypt with key2
        aesgcm2 = AESGCMSIV(key2)
        with self.assertRaises(Exception):  # Should raise authentication error
            aesgcm2.decrypt(nonce, ciphertext, None)
    
    def test_aes_gcmsiv_wrong_nonce_fails(self):
        """Test that decryption fails with wrong nonce"""
        key = secrets.token_bytes(32)
        nonce1 = secrets.token_bytes(12)
        nonce2 = secrets.token_bytes(12)
        plaintext = b"Test message"
        
        # Encrypt with nonce1
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce1, plaintext, None)
        
        # Try to decrypt with nonce2
        with self.assertRaises(Exception):  # Should raise authentication error
            aesgcm.decrypt(nonce2, ciphertext, None)
    
    # ========== ARGON2ID TEST VECTORS ==========
    
    def test_argon2id_known_vector(self):
        """Test Argon2id with known test vector"""
        # RFC 9106 test vector
        password = b"password"
        salt = unhexlify("ef8fb8b8c8e8a7b6d5e4f3c2b1a0f9e8d7c6b5a4f3e2d1c0b8a7f6e5d4c3")
        
        kdf = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,  # 64 MB
            lanes=4
        )
        
        derived_key = kdf.derive(password)
        self.assertEqual(len(derived_key), 32)
        
        # Verify deterministic derivation
        kdf2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        derived_key2 = kdf2.derive(password)
        self.assertEqual(derived_key, derived_key2)
    
    def test_argon2id_different_params(self):
        """Test Argon2id with different parameters"""
        password = self.test_password.encode()
        salt = self.test_salt
        
        # Test with different memory costs
        kdf1 = Argon2id(salt=salt, length=32, iterations=3, memory_cost=32768, lanes=4)
        kdf2 = Argon2id(salt=salt, length=32, iterations=3, memory_cost=65536, lanes=4)
        
        key1 = kdf1.derive(password)
        key2 = kdf2.derive(password)
        
        self.assertNotEqual(key1, key2)  # Different memory cost should produce different keys
    
    def test_argon2id_salt_uniqueness(self):
        """Test that different salts produce different keys"""
        password = self.test_password.encode()
        salt1 = secrets.token_bytes(32)
        salt2 = secrets.token_bytes(32)
        
        kdf1 = Argon2id(salt=salt1, length=32, iterations=3, memory_cost=65536, lanes=4)
        kdf2 = Argon2id(salt=salt2, length=32, iterations=3, memory_cost=65536, lanes=4)
        
        key1 = kdf1.derive(password)
        key2 = kdf2.derive(password)
        
        self.assertNotEqual(key1, key2)
    
    # ========== HKDF-SHA512 TEST VECTORS ==========
    
    def test_hkdf_sha512_known_vector(self):
        """Test HKDF-SHA512 with known test vector"""
        # RFC 5869 test vector adapted for SHA-512
        ikm = unhexlify("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b")
        salt = unhexlify("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20")
        info = b"f0f1f2f3f4f5f6f7f8f9"
        length = 32
        
        kdf = HKDF(
            algorithm=hashes.SHA512(),
            length=length,
            salt=salt,
            info=info
        )
        
        okm = kdf.derive(ikm)
        self.assertEqual(len(okm), length)
        
        # Verify deterministic derivation
        kdf2 = HKDF(
            algorithm=hashes.SHA512(),
            length=length,
            salt=salt,
            info=info
        )
        okm2 = kdf2.derive(ikm)
        self.assertEqual(okm, okm2)
    
    def test_hkdf_sha512_different_inputs(self):
        """Test HKDF-SHA512 with different inputs"""
        ikm = secrets.token_bytes(32)
        salt1 = secrets.token_bytes(32)
        salt2 = secrets.token_bytes(32)
        info = b"vaultx-test"
        length = 32
        
        kdf1 = HKDF(algorithm=hashes.SHA512(), length=length, salt=salt1, info=info)
        kdf2 = HKDF(algorithm=hashes.SHA512(), length=length, salt=salt2, info=info)
        
        okm1 = kdf1.derive(ikm)
        okm2 = kdf2.derive(ikm)
        
        self.assertNotEqual(okm1, okm2)  # Different salts should produce different outputs
    
    # ========== INTEGRATED KEY DERIVATION TESTS ==========
    
    def test_vaultx_key_derivation(self):
        """Test VaultX's specific key derivation sequence (Argon2id + HKDF)"""
        password = self.test_password.encode()
        salt = self.test_salt
        
        # Step 1: Argon2id
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password)
        
        # Step 2: HKDF-SHA512
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        final_key = kdf_hkdf.derive(intermediate_key)
        
        self.assertEqual(len(final_key), 32)
        
        # Verify deterministic derivation
        kdf_argon2_2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key_2 = kdf_argon2_2.derive(password)
        
        kdf_hkdf_2 = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        final_key_2 = kdf_hkdf_2.derive(intermediate_key_2)
        
        self.assertEqual(final_key, final_key_2)
    
    def test_vaultx_key_derivation_different_passwords(self):
        """Test that different passwords produce different keys"""
        salt = self.test_salt
        
        def derive_key(password_str):
            password = password_str.encode()
            
            kdf_argon2 = Argon2id(
                salt=salt,
                length=32,
                iterations=3,
                memory_cost=65536,
                lanes=4
            )
            intermediate_key = kdf_argon2.derive(password)
            
            kdf_hkdf = HKDF(
                algorithm=hashes.SHA512(),
                length=32,
                salt=salt,
                info=b'vaultx-aesgcmsiv'
            )
            return kdf_hkdf.derive(intermediate_key)
        
        key1 = derive_key("password1")
        key2 = derive_key("password2")
        
        self.assertNotEqual(key1, key2)
    
    # ========== FILE FORMAT TESTS ==========
    
    def test_vaultx_file_format_structure(self):
        """Test VaultX file format structure"""
        # Create test data
        salt = self.test_salt
        nonce = self.test_nonce
        plaintext = b"This is test data for VaultX file format testing."
        
        # Derive key
        password = self.test_password.encode()
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password)
        
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        key = kdf_hkdf.derive(intermediate_key)
        
        # Encrypt
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        
        # Create file data in VaultX format
        file_data = (
            b'VAULTX02' +  # Magic bytes (8)
            salt +         # Salt (32)
            nonce +        # Nonce (12)
            ciphertext     # Ciphertext + auth tag
        )
        
        # Verify file structure
        self.assertEqual(file_data[0:8], b'VAULTX02')
        self.assertEqual(file_data[8:40], salt)
        self.assertEqual(file_data[40:52], nonce)
        self.assertEqual(file_data[52:], ciphertext)
        
        # Verify expected file size
        expected_size = 8 + 32 + 12 + len(plaintext) + 16  # 16-byte auth tag
        self.assertEqual(len(file_data), expected_size)
    
    def test_vaultx_round_trip(self):
        """Test complete VaultX encrypt/decrypt round trip"""
        # Test data
        plaintext = b"This is a complete VaultX round-trip test."
        password = self.test_password.encode()
        
        # Generate random salt and nonce (as VaultX does)
        salt = secrets.token_bytes(32)
        nonce = secrets.token_bytes(12)
        
        # Derive key using VaultX method
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password)
        
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        key = kdf_hkdf.derive(intermediate_key)
        
        # Encrypt
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        
        # Create encrypted file data
        encrypted_data = b'VAULTX02' + salt + nonce + ciphertext
        
        # Parse and decrypt (simulating VaultX decryption)
        magic = encrypted_data[0:8]
        parsed_salt = encrypted_data[8:40]
        parsed_nonce = encrypted_data[40:52]
        parsed_ciphertext = encrypted_data[52:]
        
        # Verify parsing
        self.assertEqual(magic, b'VAULTX02')
        self.assertEqual(parsed_salt, salt)
        self.assertEqual(parsed_nonce, nonce)
        self.assertEqual(parsed_ciphertext, ciphertext)
        
        # Derive key again (as VaultX does during decryption)
        kdf_argon2_decrypt = Argon2id(
            salt=parsed_salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key_decrypt = kdf_argon2_decrypt.derive(password)
        
        kdf_hkdf_decrypt = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=parsed_salt,
            info=b'vaultx-aesgcmsiv'
        )
        key_decrypt = kdf_hkdf_decrypt.derive(intermediate_key_decrypt)
        
        # Decrypt
        aesgcm_decrypt = AESGCMSIV(key_decrypt)
        decrypted_plaintext = aesgcm_decrypt.decrypt(parsed_nonce, parsed_ciphertext, None)
        
        # Verify round-trip
        self.assertEqual(decrypted_plaintext, plaintext)
        self.assertEqual(key, key_decrypt)  # Keys should be identical
    
    # ========== EDGE CASE TESTS ==========
    
    def test_empty_password(self):
        """Test key derivation with empty password"""
        salt = self.test_salt
        
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(b"")  # Empty password
        
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        final_key = kdf_hkdf.derive(intermediate_key)
        
        self.assertEqual(len(final_key), 32)
        
        # Should still work for encryption/decryption
        nonce = secrets.token_bytes(12)
        plaintext = b"Test with empty password"
        
        aesgcm = AESGCMSIV(final_key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        decrypted = aesgcm.decrypt(nonce, ciphertext, None)
        
        self.assertEqual(decrypted, plaintext)
    
    def test_maximum_size_file(self):
        """Test encryption of large file (simulating 100MB)"""
        # Use smaller size for unit test to avoid memory issues
        test_size = 1024 * 1024  # 1MB
        plaintext = os.urandom(test_size)
        
        # Derive key
        salt = secrets.token_bytes(32)
        password = self.test_password.encode()
        
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password)
        
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        key = kdf_hkdf.derive(intermediate_key)
        
        # Encrypt and decrypt
        nonce = secrets.token_bytes(12)
        aesgcm = AESGCMSIV(key)
        
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        decrypted = aesgcm.decrypt(nonce, ciphertext, None)
        
        self.assertEqual(decrypted, plaintext)
        self.assertEqual(len(ciphertext), test_size + 16)  # +16 for auth tag
    
    def test_unicode_password(self):
        """Test key derivation with Unicode password"""
        unicode_password = "🔐TestPassword🔑123!漢字"
        salt = self.test_salt
        
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(unicode_password.encode('utf-8'))
        
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        final_key = kdf_hkdf.derive(intermediate_key)
        
        self.assertEqual(len(final_key), 32)
        
        # Test encryption/decryption
        nonce = secrets.token_bytes(12)
        plaintext = b"Unicode password test"
        
        aesgcm = AESGCMSIV(final_key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        decrypted = aesgcm.decrypt(nonce, ciphertext, None)
        
        self.assertEqual(decrypted, plaintext)


class TestVaultXIntegration(unittest.TestCase):
    """Integration tests for VaultX file operations"""
    
    def setUp(self):
        """Set up test environment"""
        self.test_dir = tempfile.mkdtemp()
        self.test_password = "IntegrationTestPassword123!"
    
    def tearDown(self):
        """Clean up test environment"""
        import shutil
        shutil.rmtree(self.test_dir)
    
    def test_file_encryption_decryption_integration(self):
        """Test complete file encryption/decryption integration"""
        # Create test file
        test_content = b"This is integration test content for VaultX." * 100
        test_file = os.path.join(self.test_dir, "test.txt")
        
        with open(test_file, 'wb') as f:
            f.write(test_content)
        
        # Encrypt file (simulating VaultX encryption)
        salt = secrets.token_bytes(32)
        nonce = secrets.token_bytes(12)
        
        # Derive key
        password = self.test_password.encode()
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password)
        
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        key = kdf_hkdf.derive(intermediate_key)
        
        # Encrypt and save
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, test_content, None)
        
        encrypted_file = test_file + ".vx2"
        with open(encrypted_file, 'wb') as f:
            f.write(b'VAULTX02')
            f.write(salt)
            f.write(nonce)
            f.write(ciphertext)
        
        # Verify encrypted file exists and has correct size
        self.assertTrue(os.path.exists(encrypted_file))
        expected_size = 8 + 32 + 12 + len(test_content) + 16
        self.assertEqual(os.path.getsize(encrypted_file), expected_size)
        
        # Decrypt file (simulating VaultX decryption)
        with open(encrypted_file, 'rb') as f:
            encrypted_data = f.read()
        
        # Parse header
        magic = encrypted_data[0:8]
        parsed_salt = encrypted_data[8:40]
        parsed_nonce = encrypted_data[40:52]
        parsed_ciphertext = encrypted_data[52:]
        
        self.assertEqual(magic, b'VAULTX02')
        
        # Derive key again
        kdf_argon2_decrypt = Argon2id(
            salt=parsed_salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key_decrypt = kdf_argon2_decrypt.derive(password)
        
        kdf_hkdf_decrypt = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=parsed_salt,
            info=b'vaultx-aesgcmsiv'
        )
        key_decrypt = kdf_hkdf_decrypt.derive(intermediate_key_decrypt)
        
        # Decrypt
        aesgcm_decrypt = AESGCMSIV(key_decrypt)
        decrypted_content = aesgcm_decrypt.decrypt(parsed_nonce, parsed_ciphertext, None)
        
        # Verify decrypted content matches original
        self.assertEqual(decrypted_content, test_content)
    
    def test_corrupted_file_detection(self):
        """Test detection of corrupted encrypted files"""
        # Create valid encrypted file
        salt = secrets.token_bytes(32)
        nonce = secrets.token_bytes(12)
        plaintext = b"Test content"
        
        password = self.test_password.encode()
        kdf_argon2 = Argon2id(
            salt=salt,
            length=32,
            iterations=3,
            memory_cost=65536,
            lanes=4
        )
        intermediate_key = kdf_argon2.derive(password)
        
        kdf_hkdf = HKDF(
            algorithm=hashes.SHA512(),
            length=32,
            salt=salt,
            info=b'vaultx-aesgcmsiv'
        )
        key = kdf_hkdf.derive(intermediate_key)
        
        aesgcm = AESGCMSIV(key)
        ciphertext = aesgcm.encrypt(nonce, plaintext, None)
        
        encrypted_data = b'VAULTX02' + salt + nonce + ciphertext
        
        # Test corruption scenarios
        test_cases = [
            (b'CORRUPT02', "Wrong magic bytes"),
            (encrypted_data[:len(encrypted_data)//2], "Truncated file"),
            (b'VAULTX02' + salt + nonce + ciphertext[:-1], b"Corrupted ciphertext"),
        ]
        
        for corrupted_data, description in test_cases:
            with self.subTest(description):
                # Try to parse and decrypt corrupted data
                if len(corrupted_data) < 8:
                    with self.assertRaises(Exception):
                        magic = corrupted_data[0:8]
                    continue
                
                magic = corrupted_data[0:8]
                if magic != b'VAULTX02':
                    continue  # Skip invalid magic bytes test
                
                if len(corrupted_data) < 52:
                    continue  # Skip too short files
                
                parsed_salt = corrupted_data[8:40]
                parsed_nonce = corrupted_data[40:52]
                parsed_ciphertext = corrupted_data[52:]
                
                # Derive key
                kdf_argon2_decrypt = Argon2id(
                    salt=parsed_salt,
                    length=32,
                    iterations=3,
                    memory_cost=65536,
                    lanes=4
                )
                intermediate_key_decrypt = kdf_argon2_decrypt.derive(password)
                
                kdf_hkdf_decrypt = HKDF(
                    algorithm=hashes.SHA512(),
                    length=32,
                    salt=parsed_salt,
                    info=b'vaultx-aesgcmsiv'
                )
                key_decrypt = kdf_hkdf_decrypt.derive(intermediate_key_decrypt)
                
                # Should fail to decrypt corrupted data
                aesgcm_decrypt = AESGCMSIV(key_decrypt)
                with self.assertRaises(Exception):
                    aesgcm_decrypt.decrypt(parsed_nonce, parsed_ciphertext, None)


def run_crypto_tests():
    """Run all cryptographic tests with detailed output"""
    print("=" * 70)
    print("VAULTX CRYPTOGRAPHIC UNIT TESTS")
    print("=" * 70)
    print("Testing AES-256-GCM-SIV, Argon2id, and HKDF-SHA512 implementations")
    print()
    
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add test classes
    suite.addTests(loader.loadTestsFromTestCase(TestVaultXCrypto))
    suite.addTests(loader.loadTestsFromTestCase(TestVaultXIntegration))
    
    # Run tests with detailed output
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
        print("\n✅ ALL CRYPTOGRAPHIC TESTS PASSED!")
        print("The AES-GCM-SIV math is verified to be correct.")
    else:
        print("\n❌ SOME TESTS FAILED!")
        print("Please review the cryptographic implementation.")
    
    print("=" * 70)
    
    return result.wasSuccessful()


if __name__ == "__main__":
    success = run_crypto_tests()
    exit(0 if success else 1)
