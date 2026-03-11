#!/usr/bin/env python3
"""
VAULTX INSTALLATION VERIFICATION
Verifies the integrity and security of VaultX installation
"""

import os
import hashlib
import sys
import importlib.util
from pathlib import Path

class VaultXVerifier:
    """Verifies VaultX installation integrity"""
    
    # Expected file hashes (update when files change)
    EXPECTED_HASHES = {
        'vaultx_secure.py': 'PLACEHOLDER_HASH',
        'requirements.txt': 'PLACEHOLDER_HASH',
        'test_vaultx_crypto.py': 'PLACEHOLDER_HASH',
        'run_tests.py': 'PLACEHOLDER_HASH'
    }
    
    def __init__(self):
        self.issues = []
        self.warnings = []
        self.success_count = 0
        
    def calculate_file_hash(self, file_path):
        """Calculate SHA-256 hash of a file"""
        try:
            hasher = hashlib.sha256()
            with open(file_path, 'rb') as f:
                for chunk in iter(lambda: f.read(4096), b""):
                    hasher.update(chunk)
            return hasher.hexdigest()
        except Exception as e:
            self.issues.append(f"Cannot hash {file_path}: {e}")
            return None
    
    def verify_file_exists(self, file_path):
        """Verify that required files exist"""
        if os.path.exists(file_path):
            self.success_count += 1
            return True
        else:
            self.issues.append(f"Missing required file: {file_path}")
            return False
    
    def verify_file_integrity(self, file_path):
        """Verify file integrity using hash"""
        if not os.path.exists(file_path):
            return False
        
        current_hash = self.calculate_file_hash(file_path)
        expected_hash = self.EXPECTED_HASHES.get(os.path.basename(file_path))
        
        # For development - skip hash verification
        if expected_hash == 'PLACEHOLDER_HASH':
            self.warnings.append(f"Hash verification disabled for {file_path} (development mode)")
            self.success_count += 1
            return True
        
        if current_hash == expected_hash:
            self.success_count += 1
            return True
        else:
            self.issues.append(f"Hash mismatch for {file_path}")
            self.issues.append(f"  Expected: {expected_hash}")
            self.issues.append(f"  Actual:   {current_hash}")
            return False
    
    def verify_python_version(self):
        """Verify Python version compatibility"""
        version = sys.version_info
        if version.major >= 3 and version.minor >= 8:
            self.success_count += 1
            return True
        else:
            self.issues.append(f"Python 3.8+ required, found {version.major}.{version.minor}")
            return False
    
    def verify_dependencies(self):
        """Verify required dependencies can be imported"""
        required_modules = [
            'tkinter',
            'cryptography',
            'secrets',
            'hashlib',
            'json',
            'threading'
        ]
        
        for module in required_modules:
            try:
                if module == 'tkinter':
                    import tkinter as tk
                    # Test tkinter functionality
                    root = tk.Tk()
                    root.destroy()
                elif module == 'cryptography':
                    from cryptography.hazmat.primitives.ciphers.aead import AESGCMSIV
                    from cryptography.hazmat.primitives.kdf.argon2 import Argon2id
                    from cryptography.hazmat.primitives.kdf.hkdf import HKDF
                    from cryptography.hazmat.primitives import hashes
                elif module == 'secrets':
                    import secrets
                    # Test secure random generation
                    test_bytes = secrets.token_bytes(32)
                    assert len(test_bytes) == 32
                else:
                    importlib.import_module(module)
                
                self.success_count += 1
            except ImportError as e:
                self.issues.append(f"Missing dependency: {module} - {e}")
            except Exception as e:
                self.issues.append(f"Error testing {module}: {e}")
    
    def verify_source_code_security(self):
        """Basic security checks on source code"""
        vaultx_file = 'vaultx_secure.py'
        if not os.path.exists(vaultx_file):
            return
        
        try:
            with open(vaultx_file, 'r', encoding='utf-8') as f:
                content = f.read()
            
            # Check for suspicious patterns
            suspicious_patterns = [
                'import requests',
                'import urllib',
                'import socket',
                'import http',
                'subprocess.call',
                'os.system',
                'eval(',
                'exec(',
                'input(',
                'raw_input(',
                'open(',
                'file(',
                'connect(',
                'send(',
                'post(',
                'get(',
                'http://',
                'https://',
                'ftp://'
            ]
            
            found_suspicious = False
            for pattern in suspicious_patterns:
                if pattern in content:
                    # Some patterns may be legitimate, check context
                    if pattern in ['import hashlib', 'import json', 'import threading']:
                        continue  # These are expected
                    self.warnings.append(f"Suspicious pattern found: {pattern}")
                    found_suspicious = True
            
            if not found_suspicious:
                self.success_count += 1
            
            # Check for expected security features
            security_features = [
                'AESGCMSIV',
                'Argon2id',
                'HKDF',
                'SHA512',
                'secrets.token_bytes',
                'verify_integrity',
                'detect_debugging'
            ]
            
            missing_features = []
            for feature in security_features:
                if feature not in content:
                    missing_features.append(feature)
            
            if missing_features:
                self.warnings.append(f"Missing security features: {', '.join(missing_features)}")
            else:
                self.success_count += 1
                
        except Exception as e:
            self.issues.append(f"Error analyzing source code: {e}")
    
    def verify_file_permissions(self):
        """Verify file permissions are reasonable"""
        vaultx_file = 'vaultx_secure.py'
        if not os.path.exists(vaultx_file):
            return
        
        try:
            # Check if file is readable
            if os.access(vaultx_file, os.R_OK):
                self.success_count += 1
            else:
                self.issues.append(f"Cannot read {vaultx_file}")
            
            # Check if directory is writable (for creating recent files)
            if os.access('.', os.W_OK):
                self.success_count += 1
            else:
                self.warnings.append("Directory not writable - recent files may not work")
                
        except Exception as e:
            self.issues.append(f"Error checking permissions: {e}")
    
    def run_verification(self):
        """Run complete verification suite"""
        print("VAULTX INSTALLATION VERIFICATION")
        print("=" * 40)
        
        # Required files
        required_files = [
            'vaultx_secure.py',
            'requirements.txt',
            'test_vaultx_crypto.py',
            'run_tests.py'
        ]
        
        print("Checking required files...")
        for file_path in required_files:
            self.verify_file_exists(file_path)
        
        print("\nVerifying file integrity...")
        for file_path in required_files:
            self.verify_file_integrity(file_path)
        
        print("\nChecking Python version...")
        self.verify_python_version()
        
        print("\nVerifying dependencies...")
        self.verify_dependencies()
        
        print("\nAnalyzing source code security...")
        self.verify_source_code_security()
        
        print("\nChecking file permissions...")
        self.verify_file_permissions()
        
        # Print results
        self.print_results()
        
        return len(self.issues) == 0
    
    def print_results(self):
        """Print verification results"""
        print("\n" + "=" * 40)
        print("VERIFICATION RESULTS")
        print("=" * 40)
        
        print(f"✅ Successful checks: {self.success_count}")
        print(f"⚠️  Warnings: {len(self.warnings)}")
        print(f"❌ Issues: {len(self.issues)}")
        
        if self.warnings:
            print("\n⚠️  WARNINGS:")
            for warning in self.warnings:
                print(f"  - {warning}")
        
        if self.issues:
            print("\n❌ ISSUES:")
            for issue in self.issues:
                print(f"  - {issue}")
        
        if len(self.issues) == 0:
            print("\n🎉 INSTALLATION VERIFICATION PASSED!")
            print("VaultX appears to be properly installed and secure.")
            
            if len(self.warnings) == 0:
                print("No warnings detected.")
            else:
                print("Some warnings were detected but installation is usable.")
        else:
            print("\n❌ INSTALLATION VERIFICATION FAILED!")
            print("Please resolve the issues above before using VaultX.")
        
        print("\n" + "=" * 40)


def main():
    """Main verification function"""
    verifier = VaultXVerifier()
    success = verifier.run_verification()
    
    if success:
        print("\nYou can now run VaultX with: python vaultx_secure.py")
        sys.exit(0)
    else:
        print("\nPlease fix the issues before running VaultX.")
        sys.exit(1)


if __name__ == "__main__":
    main()
