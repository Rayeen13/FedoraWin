$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path $PSScriptRoot '..\shell\AppCatalog.ps1')

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

$terminal = New-FedoraWinAppRecord -Name 'Windows Terminal' -AppId 'Microsoft.WindowsTerminal_8wekyb3d8bbwe!App' -TargetPath $null -Arguments $null -Source 'Test'
$notepad = New-FedoraWinAppRecord -Name 'Notepad' -AppId 'Microsoft.WindowsNotepad_8wekyb3d8bbwe!App' -TargetPath $null -Arguments $null -Source 'Test'
$explorer = New-FedoraWinAppRecord -Name 'File Explorer' -AppId $null -TargetPath 'C:\Windows\explorer.exe' -Arguments $null -Source 'Test'
$settings = New-FedoraWinAppRecord -Name 'Settings' -AppId 'windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel' -TargetPath $null -Arguments $null -Source 'Test'

Assert-True ($terminal.Aliases -contains 'terminal') 'Windows Terminal must expose the terminal alias.'
Assert-True ($terminal.Aliases -contains 'command line') 'Windows Terminal must expose the command-line alias.'
Assert-True ($explorer.Aliases -contains 'files') 'File Explorer must expose the GNOME Files alias.'
Assert-True ($settings.Aliases -contains 'preferences') 'Settings must expose preferences alias.'

$apps = @($notepad, $explorer, $terminal, $settings)
$terminalResult = @(Search-FedoraWinApps -Apps $apps -Query 'terminal' -Limit 10)
Assert-True ($terminalResult.Count -gt 0) 'Terminal search returned no result.'
Assert-True ($terminalResult[0].Name -eq 'Windows Terminal') 'Terminal search must rank Windows Terminal first.'

$filesResult = @(Search-FedoraWinApps -Apps $apps -Query 'files' -Limit 10)
Assert-True ($filesResult.Count -gt 0) 'Files search returned no result.'
Assert-True ($filesResult[0].Name -eq 'File Explorer') 'Files search must rank File Explorer first.'

$partial = @(Search-FedoraWinApps -Apps $apps -Query 'note' -Limit 10)
Assert-True ($partial.Count -gt 0 -and $partial[0].Name -eq 'Notepad') 'Prefix search must rank Notepad.'

$all = @(Search-FedoraWinApps -Apps $apps -Limit 3)
Assert-True ($all.Count -eq 3) 'Empty-query app drawer results must obey the limit.'

$source = Get-Content -Raw (Join-Path $PSScriptRoot '..\shell\AppCatalog.ps1')
Assert-True ($source -match 'Get-StartApps') 'Catalog must discover packaged Start apps.'
Assert-True ($source -match 'WScript\.Shell') 'Catalog must discover classic Start Menu shortcuts.'
Assert-True ($source -match 'shell:AppsFolder') 'Packaged apps must launch through AppsFolder.'

Write-Host 'App catalog smoke passed.'
