$startup = [Environment]::GetFolderPath('Startup')
$linkPath = Join-Path $startup 'FedoraWin.lnk'
Remove-Item -LiteralPath $linkPath -Force -ErrorAction SilentlyContinue
Write-Host 'FedoraWin startup shortcut removed.'
