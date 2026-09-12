Set-StrictMode -Version 2.0

function ConvertTo-FedoraWinSearchText {
    param([AllowNull()][string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) { return '' }
    $normalized = $Text.ToLowerInvariant() -replace '[^a-z0-9]+', ' '
    return ($normalized -replace '\s+', ' ').Trim()
}

function Get-FedoraWinAppAliases {
    param(
        [AllowNull()][string]$Name,
        [AllowNull()][string]$AppId,
        [AllowNull()][string]$TargetPath
    )

    $probe = ConvertTo-FedoraWinSearchText ("$Name $AppId $TargetPath")
    $aliases = New-Object System.Collections.Generic.List[string]

    function Add-Alias([string]$Value) {
        if ([string]::IsNullOrWhiteSpace($Value)) { return }
        $value = ConvertTo-FedoraWinSearchText $Value
        if ($value -and -not $aliases.Contains($value)) { [void]$aliases.Add($value) }
    }

    if ($probe -match 'windows terminal|windowsterminal|(^| )wt( |$)') {
        foreach ($alias in @('terminal','console','command line','shell','cmd','powershell','pwsh')) { Add-Alias $alias }
    }
    if ($probe -match 'powershell') {
        foreach ($alias in @('terminal','shell','console','pwsh')) { Add-Alias $alias }
    }
    if ($probe -match 'command prompt|cmd exe') {
        foreach ($alias in @('terminal','console','shell','cmd')) { Add-Alias $alias }
    }
    if ($probe -match 'file explorer|explorer exe') {
        foreach ($alias in @('files','file manager','folders','explorer')) { Add-Alias $alias }
    }
    if ($probe -match 'microsoft store|windows store') {
        foreach ($alias in @('software','apps','app store','store')) { Add-Alias $alias }
    }
    if ($probe -match 'settings|systemsettings') {
        foreach ($alias in @('preferences','control center','system settings')) { Add-Alias $alias }
    }
    if ($probe -match 'snipping tool|screenclip|snippingtool') {
        foreach ($alias in @('screenshot','screen capture','snip')) { Add-Alias $alias }
    }
    if ($probe -match 'notepad') { Add-Alias 'text editor' }
    if ($probe -match 'calculator') { Add-Alias 'calc' }

    return @($aliases)
}

function New-FedoraWinAppRecord {
    param(
        [Parameter(Mandatory=$true)][string]$Name,
        [AllowNull()][string]$AppId,
        [AllowNull()][string]$TargetPath,
        [AllowNull()][string]$Arguments,
        [Parameter(Mandatory=$true)][string]$Source
    )

    $aliases = @(Get-FedoraWinAppAliases -Name $Name -AppId $AppId -TargetPath $TargetPath)
    $searchParts = @($Name, $AppId, $TargetPath) + $aliases
    [pscustomobject]@{
        Name       = $Name
        AppId      = $AppId
        TargetPath = $TargetPath
        Arguments  = $Arguments
        Source     = $Source
        Aliases    = $aliases
        SearchText = ConvertTo-FedoraWinSearchText ($searchParts -join ' ')
    }
}

function Get-FedoraWinInstalledApps {
    [CmdletBinding()]
    param()

    $records = New-Object System.Collections.Generic.List[object]
    $seen = @{}

    try {
        if (Get-Command Get-StartApps -ErrorAction SilentlyContinue) {
            foreach ($app in @(Get-StartApps -ErrorAction Stop)) {
                if ([string]::IsNullOrWhiteSpace([string]$app.Name)) { continue }
                $key = ('appid:' + ([string]$app.AppID).ToLowerInvariant())
                if ($seen.ContainsKey($key)) { continue }
                $seen[$key] = $true
                [void]$records.Add((New-FedoraWinAppRecord -Name ([string]$app.Name) -AppId ([string]$app.AppID) -TargetPath $null -Arguments $null -Source 'StartApps'))
            }
        }
    } catch { }

    $roots = @(
        [Environment]::GetFolderPath('StartMenu'),
        [Environment]::GetFolderPath('CommonStartMenu')
    ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -Unique

    $shell = $null
    try { $shell = New-Object -ComObject WScript.Shell } catch { }

    if ($shell) {
        foreach ($root in $roots) {
            foreach ($shortcut in @(Get-ChildItem -LiteralPath $root -Filter '*.lnk' -File -Recurse -ErrorAction SilentlyContinue)) {
                try {
                    $link = $shell.CreateShortcut($shortcut.FullName)
                    $target = [string]$link.TargetPath
                    $args = [string]$link.Arguments
                    $name = [IO.Path]::GetFileNameWithoutExtension($shortcut.Name)
                    if ([string]::IsNullOrWhiteSpace($name)) { continue }

                    $keyMaterial = if ($target) { "$target|$args" } else { $shortcut.FullName }
                    $key = 'lnk:' + (ConvertTo-FedoraWinSearchText $keyMaterial)
                    if ($seen.ContainsKey($key)) { continue }
                    $seen[$key] = $true
                    [void]$records.Add((New-FedoraWinAppRecord -Name $name -AppId $null -TargetPath $target -Arguments $args -Source 'StartMenu'))
                } catch { }
            }
        }
    }

    return @($records | Sort-Object Name, Source)
}

function Get-FedoraWinAppSearchScore {
    param(
        [Parameter(Mandatory=$true)]$App,
        [Parameter(Mandatory=$true)][string]$Query
    )

    $q = ConvertTo-FedoraWinSearchText $Query
    if (-not $q) { return 1 }

    $name = ConvertTo-FedoraWinSearchText ([string]$App.Name)
    $aliases = @($App.Aliases | ForEach-Object { ConvertTo-FedoraWinSearchText ([string]$_) })
    $search = ConvertTo-FedoraWinSearchText ([string]$App.SearchText)

    if ($name -eq $q) { return 1000 }
    if ($aliases -contains $q) { return 950 }
    if ($name.StartsWith($q, [StringComparison]::OrdinalIgnoreCase)) { return 800 }
    foreach ($alias in $aliases) {
        if ($alias.StartsWith($q, [StringComparison]::OrdinalIgnoreCase)) { return 750 }
    }
    if ($name.IndexOf($q, [StringComparison]::OrdinalIgnoreCase) -ge 0) { return 600 }
    if ($search.IndexOf($q, [StringComparison]::OrdinalIgnoreCase) -ge 0) { return 400 }
    return 0
}

function Search-FedoraWinApps {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)][object[]]$Apps,
        [AllowEmptyString()][string]$Query = '',
        [int]$Limit = 24
    )

    $matches = foreach ($app in $Apps) {
        $score = Get-FedoraWinAppSearchScore -App $app -Query $Query
        if ($score -gt 0) {
            [pscustomobject]@{ App = $app; Score = $score }
        }
    }

    return @($matches | Sort-Object @{Expression='Score';Descending=$true}, @{Expression={$_.App.Name};Descending=$false} | Select-Object -First $Limit | ForEach-Object { $_.App })
}

function Start-FedoraWinApp {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)]$App)

    if ($App.AppId) {
        Start-Process -FilePath 'explorer.exe' -ArgumentList ("shell:AppsFolder\{0}" -f [string]$App.AppId)
        return
    }
    if ($App.TargetPath) {
        Start-Process -FilePath ([string]$App.TargetPath) -ArgumentList ([string]$App.Arguments)
        return
    }
    throw "No launch target is available for '$($App.Name)'."
}
