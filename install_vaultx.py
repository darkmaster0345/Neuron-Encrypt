#!/usr/bin/env python3
"""
VAULTX INSTALLATION SCRIPT
Secure installation of VaultX with verification
"""

import os
import sys
import subprocess
import shutil
from pathlib import Path

class VaultXInstaller:
    """Secure installer for VaultX"""
    
    def __init__(self):
        self.install_dir = Path.cwd()
        self.python_executable = sys.executable
        
    def check_python_version(self):
        """Check if Python version is compatible"""
        version = sys.version_info
        if version.major >= 3 and version.minor >= 8:
            print(f"✅ Python {version.major}.{version.minor}.{version.micro} detected")
            return True
        else:
            print(f"❌ Python 3.8+ required, found {version.major}.{version.minor}")
            return False
    
    def check_pip(self):
        """Check if pip is available"""
        try:
            subprocess.run([self.python_executable, '-m', 'pip', '--version'], 
                         capture_output=True, check=True)
            print("✅ pip is available")
            return True
        except subprocess.CalledProcessError:
            print("❌ pip is not available")
            return False
    
    def install_dependencies(self):
        """Install required dependencies"""
        requirements_file = self.install_dir / 'requirements.txt'
        
        if not requirements_file.exists():
            print("❌ requirements.txt not found")
            return False
        
        print("Installing dependencies...")
        try:
            result = subprocess.run([
                self.python_executable, '-m', 'pip', 'install', '-r', str(requirements_file)
            ], capture_output=True, text=True, check=True)
            
            print("✅ Dependencies installed successfully")
            return True
        except subprocess.CalledProcessError as e:
            print(f"❌ Failed to install dependencies: {e}")
            print(f"Error output: {e.stderr}")
            return False
    
    def verify_installation(self):
        """Run installation verification"""
        verify_script = self.install_dir / 'verify_installation.py'
        
        if not verify_script.exists():
            print("❌ verify_installation.py not found")
            return False
        
        print("Running installation verification...")
        try:
            result = subprocess.run([
                self.python_executable, str(verify_script)
            ], capture_output=True, text=True)
            
            print(result.stdout)
            if result.stderr:
                print("Warnings/Errors:")
                print(result.stderr)
            
            return result.returncode == 0
        except Exception as e:
            print(f"❌ Verification failed: {e}")
            return False
    
    def run_crypto_tests(self):
        """Run cryptographic tests"""
        test_script = self.install_dir / 'run_tests.py'
        
        if not test_script.exists():
            print("❌ run_tests.py not found")
            return False
        
        print("Running cryptographic tests...")
        try:
            result = subprocess.run([
                self.python_executable, str(test_script)
            ], capture_output=True, text=True)
            
            print(result.stdout)
            if result.stderr:
                print("Test errors:")
                print(result.stderr)
            
            return result.returncode == 0
        except Exception as e:
            print(f"❌ Crypto tests failed: {e}")
            return False
    
    def create_desktop_shortcut(self):
        """Create desktop shortcut (Windows only)"""
        if sys.platform != 'win32':
            print("⚠️  Desktop shortcut creation only supported on Windows")
            return True
        
        try:
            import winshell
            from win32com.client import Dispatch
            
            desktop = winshell.desktop()
            path = os.path.join(desktop, "VaultX.lnk")
            target = str(self.install_dir / 'vaultx_secure.py')
            wDir = str(self.install_dir)
            icon = target
            
            shell = Dispatch('WScript.Shell')
            shortcut = shell.CreateShortCut(path)
            shortcut.Targetpath = self.python_executable
            shortcut.Arguments = f'"{target}"'
            shortcut.WorkingDirectory = wDir
            shortcut.IconLocation = icon
            shortcut.save()
            
            print("✅ Desktop shortcut created")
            return True
        except ImportError:
            print("⚠️  Cannot create desktop shortcut (missing winshell/win32com)")
            return True
        except Exception as e:
            print(f"⚠️  Failed to create desktop shortcut: {e}")
            return True
    
    def print_success_message(self):
        """Print success message with next steps"""
        print("\n" + "=" * 50)
        print("🎉 VAULTX INSTALLATION SUCCESSFUL!")
        print("=" * 50)
        
        print("\n📋 NEXT STEPS:")
        print(f"1. Run VaultX: python vaultx_secure.py")
        print(f"2. Or double-click the desktop shortcut (if created)")
        print(f"3. Review the security documentation")
        print(f"4. Test with sample files before important use")
        
        print("\n🔒 SECURITY REMINDERS:")
        print("• Always verify source code before use")
        print("• Use strong passwords (12+ characters)")
        print("• Never share your passwords")
        print("• Loss of password = permanent data loss")
        
        print("\n📚 DOCUMENTATION:")
        print("• README.md - Full documentation")
        print("• SECURITY_ANALYSIS.md - Security details")
        print("• CRYPTO_TEST_REPORT.md - Test results")
        
        print("\n" + "=" * 50)
    
    def install(self, create_shortcut=True):
        """Run complete installation"""
        print("VAULTX SECURE INSTALLATION")
        print("=" * 40)
        
        # Step 1: Check Python version
        if not self.check_python_version():
            return False
        
        # Step 2: Check pip
        if not self.check_pip():
            return False
        
        # Step 3: Install dependencies
        if not self.install_dependencies():
            return False
        
        # Step 4: Verify installation
        if not self.verify_installation():
            return False
        
        # Step 5: Run crypto tests
        if not self.run_crypto_tests():
            print("⚠️  Crypto tests failed, but installation may still work")
        
        # Step 6: Create desktop shortcut (optional)
        if create_shortcut:
            self.create_desktop_shortcut()
        
        # Success
        self.print_success_message()
        return True


def main():
    """Main installation function"""
    print("Starting VaultX secure installation...")
    
    # Check if running as administrator (Windows)
    if sys.platform == 'win32':
        try:
            import ctypes
            if ctypes.windll.shell32.IsUserAnAdmin():
                print("⚠️  Running as administrator - not recommended for security")
                response = input("Continue anyway? (y/N): ")
                if response.lower() != 'y':
                    print("Installation cancelled for security reasons")
                    return
        except:
            pass
    
    # Create installer
    installer = VaultXInstaller()
    
    # Ask about desktop shortcut
    create_shortcut = True
    if sys.platform == 'win32':
        response = input("Create desktop shortcut? (Y/n): ")
        create_shortcut = response.lower() != 'n'
    
    # Run installation
    success = installer.install(create_shortcut=create_shortcut)
    
    if success:
        print("\n✅ Installation completed successfully!")
    else:
        print("\n❌ Installation failed. Please check the errors above.")
        sys.exit(1)


if __name__ == "__main__":
    main()
