#!/bin/bash
# VaultX Linux Build Script
# Builds a Linux executable from vaultx.py

echo "=== VAULTX LINUX BUILD ==="

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

# Build the executable
echo "Building VaultX executable..."
python3 -m PyInstaller --onefile --windowed --name VaultX vaultx.py

# Check if build succeeded
if [ -f "dist/VaultX" ]; then
    echo "SUCCESS: VaultX executable created at dist/VaultX"
    echo "File size: $(ls -lh dist/VaultX | awk '{print $5}')"
else
    echo "ERROR: Build failed. Check the output above for errors."
    exit 1
fi

echo "=== BUILD COMPLETE ==="
