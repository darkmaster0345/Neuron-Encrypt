@echo off
echo ================================================================
echo   NEURON ENCRYPT — Release Build
echo   Target: Host
echo ================================================================
echo.

cargo build --release

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo BUILD FAILED. See errors above.
    pause
    exit /b 1
)

echo.
echo ================================================================
echo   BUILD SUCCESSFUL
echo   EXE: target\release\neuron-encrypt.exe
echo ================================================================
pause
