@echo off
echo ================================================================
echo   NEURON ENCRYPT — Release Build
echo   Target: x86_64-pc-windows-msvc
echo ================================================================
echo.

cargo build --release --target x86_64-pc-windows-msvc

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo BUILD FAILED. See errors above.
    pause
    exit /b 1
)

echo.
echo ================================================================
echo   BUILD SUCCESSFUL
echo   EXE: target\x86_64-pc-windows-msvc\release\neuron-encrypt.exe
echo ================================================================
pause
