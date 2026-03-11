#!/bin/bash
# VaultX macOS Build Script
# Builds a universal macOS executable (Intel + Apple Silicon) from vaultx.py

echo "=== VAULTX MACOS BUILD ==="

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo "ERROR: Python 3 not found. Please install Python 3."
    exit 1
fi

# Install dependencies
echo "Installing dependencies..."
pip3 install -r requirements.txt

# Check if PyInstaller is installed
if ! python3 -c "import PyInstaller" &> /dev/null; then
    echo "Installing PyInstaller..."
    pip3 install pyinstaller>=6.0.0
fi

# Create build directory if it doesn't exist
mkdir -p build
mkdir -p dist

# Build universal binary for both Intel and Apple Silicon
echo "Building VaultX universal executable..."
python3 -m PyInstaller --onefile --windowed --name VaultX --target-architecture universal2 vaultx.py

# Check if build succeeded
if [ -f "dist/VaultX" ]; then
    echo "SUCCESS: VaultX executable created at dist/VaultX"
    echo "File size: $(ls -lh dist/VaultX | awk '{print $5}')"
    echo "Architecture: $(file dist/VaultX | grep -o 'universal binary\|x86_64\|arm64')"
else
    echo "ERROR: Build failed. Check the output above for errors."
    exit 1
fi

echo "=== BUILD COMPLETE ==="
