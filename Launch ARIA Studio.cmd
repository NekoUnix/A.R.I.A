@echo off
setlocal
set "ARIA_EXE=%~dp0target\debug\aria-desktop.exe"
if not exist "%ARIA_EXE%" (
  echo ARIA Studio has not been built in this folder.
  echo Expected: "%ARIA_EXE%"
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
