@echo off
setlocal
set "ARIA_EXE=%~dp0target\release\aria-desktop.exe"
if not exist "%ARIA_EXE%" (
  echo The optimized ARIA Studio build is missing.
  echo Expected: "%ARIA_EXE%"
  echo Build it with: cargo build --release --locked -p aria-desktop
  echo See docs\windows.md for the Windows build instructions.
  pause
  exit /b 1
)
start "" /D "%~dp0" "%ARIA_EXE%"
if errorlevel 1 (
  echo Windows could not start ARIA Studio.
  pause
  exit /b 1
)
exit /b 0
