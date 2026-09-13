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

$firefoxPackaged = New-FedoraWinAppRecord -Name 'Firefox' -AppId 'Mozilla.Firefox_test!App' -TargetPath $null -Arguments $null -Source 'StartApps'
$firefoxClassic = New-FedoraWinAppRecord -Name 'Firefox' -AppId $null -TargetPath 'C:\Program Files\Mozilla Firefox\firefox.exe' -Arguments $null -Source 'StartMenu'
$preferredFirefox = @(Select-FedoraWinPreferredAppRecords -Apps @($firefoxPackaged,$firefoxClassic))
Assert-True ($preferredFirefox.Count -eq 1) 'Duplicate app names must reconcile to one preferred launcher record.'
Assert-True ($preferredFirefox[0].TargetPath -eq 'C:\Program Files\Mozilla Firefox\firefox.exe') 'Classic launcher with an executable/icon target must beat a duplicate AppId-only record.'

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
Assert-True ($source -match 'Select-FedoraWinPreferredAppRecords') 'Catalog must reconcile duplicate StartApps and Start Menu records.'

$discovered = @(Get-FedoraWinInstalledApps)
Assert-True ($discovered.Count -gt 0) 'Real Windows installed-app discovery returned no applications.'
$normalizedNames = @(
    $discovered |
        ForEach-Object { ConvertTo-FedoraWinSearchText ([string]$_.Name) } |
        Where-Object { $_ }
)
$uniqueNames = @($normalizedNames | Select-Object -Unique)
Assert-True ($normalizedNames.Count -eq $uniqueNames.Count) 'Installed-app reconciliation left duplicate normalized app names.'

Write-Host ("App catalog smoke passed ({0} real applications discovered)." -f $discovered.Count)
