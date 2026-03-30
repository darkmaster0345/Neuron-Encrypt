# Neuron Encrypt Windows Installer

This directory contains the NSIS script for building a Windows installer for Neuron Encrypt.

## Prerequisites

You need **NSIS 3.x** (Nullsoft Scriptable Install System) installed on your Windows machine.

### Install NSIS

**Via Chocolatey (recommended):**
```
choco install nsis --yes
```

**Via winget:**
```
winget install NSIS.NSIS
```

**Direct download:**
Visit https://nsis.sourceforge.io/Download and grab the latest NSIS 3.x installer.

## Building the Installer Locally

1. **Build the release binary** from the repository root:

   ```
   cd neuron-encrypt
   cargo build --release --target x86_64-pc-windows-msvc
   ```

2. **Run the NSIS compiler** from the repository root:

   ```
   makensis installer/neuron-encrypt.nsi
   ```

3. The installer will be written to:

   ```
   installer/NeuronEncrypt-Windows-x64-Setup.exe
   ```

## Testing the .vx2 File Association

After installing with the file association option checked:

1. Create a test encrypted file using Neuron Encrypt (encrypt any small file).
2. Navigate to the resulting `.vx2` file in File Explorer.
3. The file should show the Neuron Encrypt icon.
4. Double-clicking the `.vx2` file should launch Neuron Encrypt.
5. You can also right-click the file and verify "Open with" shows Neuron Encrypt.

To verify the registry entries:
```
reg query HKCR\.vx2
reg query HKCR\NeuronEncrypt.vx2file
```

## Clean Uninstall for Testing

1. Open **Settings > Apps > Installed apps** (or **Control Panel > Programs and Features**).
2. Find "Neuron Encrypt" and click Uninstall.
3. The uninstaller will remove:
   - The application binary and license file
   - Start Menu and Desktop shortcuts
   - The `.vx2` file association registry keys
   - The Add/Remove Programs registry entry
   - The install directory (only if empty)
4. The uninstaller will **not** delete any user `.vx2` encrypted files.

To verify complete removal:
```
reg query HKCR\.vx2 2>nul && echo "WARN: .vx2 key still exists" || echo "OK: .vx2 key removed"
reg query HKCR\NeuronEncrypt.vx2file 2>nul && echo "WARN: class key still exists" || echo "OK: class key removed"
reg query HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\NeuronEncrypt 2>nul && echo "WARN: uninstall key still exists" || echo "OK: uninstall key removed"
```

## CI/CD

The GitHub Actions release workflow automatically builds the installer on `windows-latest` after the main build job completes. It uploads both:

- `NeuronEncrypt-Windows-x64-Portable.exe` -- the standalone portable binary
- `NeuronEncrypt-Windows-x64-Setup.exe` -- the NSIS installer
