@echo off
set "LOG=%LOCALAPPDATA%\FedoraWin\FedoraWin.log"
if exist "%LOG%" (
  notepad.exe "%LOG%"
) else (
  echo No FedoraWin log exists yet.
  pause
)
