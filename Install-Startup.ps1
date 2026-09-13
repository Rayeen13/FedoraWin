Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$startup = [Environment]::GetFolderPath('Startup')
$linkPath = Join-Path $startup 'FedoraWin.lnk'
$target = Join-Path $root 'Start-FedoraWin.cmd'
$ws = New-Object -ComObject WScript.Shell
$shortcut = $ws.CreateShortcut($linkPath)
$shortcut.TargetPath = $target
$shortcut.WorkingDirectory = $root
$shortcut.Description = 'Start FedoraWin at sign-in'
$shortcut.Save()
Write-Host "Startup shortcut created: $linkPath"
