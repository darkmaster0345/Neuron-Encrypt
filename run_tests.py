#!/usr/bin/env python3
"""
VAULTX TEST RUNNER
Simple script to run all VaultX cryptographic tests
"""

import sys
import os

# Add current directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from test_vaultx_crypto import run_crypto_tests

if __name__ == "__main__":
    print("Starting VaultX cryptographic test suite...")
    success = run_crypto_tests()
    
    if success:
        print("\n🎉 All tests completed successfully!")
        print("The VaultX cryptographic implementation is mathematically correct.")
    else:
        print("\n⚠️  Some tests failed. Please review the implementation.")
    
    sys.exit(0 if success else 1)
