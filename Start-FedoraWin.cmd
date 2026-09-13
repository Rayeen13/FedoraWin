@echo off
cd /d "%~dp0"
start "FedoraWin 0.3.0" powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -WindowStyle Hidden -File "%~dp0FedoraWin.ps1"
