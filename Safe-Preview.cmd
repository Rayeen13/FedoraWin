@echo off
setlocal
cd /d "%~dp0"
title FedoraWin 0.3.0 Safe Preview

echo FedoraWin 0.3.0 Safe Preview
echo ----------------------------
echo Temporary preview: keeps the Windows taskbar, desktop icons and wallpaper.
echo The GNOME panel reserves its own top strip and reversible window-chrome overlays are enabled.
echo If FedoraWin exits, both disappear automatically.
echo.

powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0FedoraWin.ps1" -SafeMode -Diagnostic
set "FW_EXIT=%ERRORLEVEL%"

if not "%FW_EXIT%"=="0" (
  echo.
  echo [ERROR] FedoraWin exited with code %FW_EXIT%.
  echo Log: %LOCALAPPDATA%\FedoraWin\FedoraWin.log
  echo.
  echo Keep this window open or send the log back for diagnosis.
  pause
  exit /b %FW_EXIT%
)

exit /b 0
