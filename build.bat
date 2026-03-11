@echo off
echo Building VaultX...
echo.

pip install cryptography pyinstaller
echo.

python -m PyInstaller --onefile --windowed --name VaultX vaultx.py
echo.

echo Done. Your exe is in the dist/ folder.
pause
