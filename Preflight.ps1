Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$results = New-Object System.Collections.Generic.List[object]

function Add-Result {
    param([string]$Name, [bool]$Pass, [string]$Detail)
    $results.Add([pscustomobject]@{ Name=$Name; Pass=$Pass; Detail=$Detail }) | Out-Null
}

try {
    $v = [Environment]::OSVersion.Version
    Add-Result 'Windows 11 build' ($v.Major -ge 10 -and $v.Build -ge 22000) ("Detected {0}" -f $v)
} catch { Add-Result 'Windows 11 build' $false $_.Exception.Message }

try {
    Add-Result 'PowerShell 5.1+' ($PSVersionTable.PSVersion -ge [Version]'5.1') ("PowerShell {0}" -f $PSVersionTable.PSVersion)
} catch { Add-Result 'PowerShell 5.1+' $false $_.Exception.Message }

try {
    Add-Result 'STA apartment' ([Threading.Thread]::CurrentThread.ApartmentState -eq 'STA') ([Threading.Thread]::CurrentThread.ApartmentState.ToString())
} catch { Add-Result 'STA apartment' $false $_.Exception.Message }

try {
    Add-Type -AssemblyName PresentationFramework
    Add-Type -AssemblyName PresentationCore
    Add-Type -AssemblyName WindowsBase
    Add-Type -AssemblyName System.Xaml
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    Add-Result 'WPF assemblies' $true 'Loaded successfully'
} catch { Add-Result 'WPF assemblies' $false $_.Exception.Message }

try {
    $nativeUiPath = Join-Path $root 'native\FedoraWinNativeUi.cs'
    Add-Type -TypeDefinition (Get-Content -Raw -Encoding UTF8 -LiteralPath $nativeUiPath) -ReferencedAssemblies @('System.Windows.Forms.dll','System.Drawing.dll','System.dll')
    $probeManager = New-Object FedoraWinChromeManager -ArgumentList 'FedoraWinPreflightNeverMatches'
    $probeManager.SetTheme('dark', '#3584E4', $true)
    $probeManager.Dispose()
    Add-Result 'Native GNOME bridge' $true 'AppBar + reversible window chrome compiled successfully'
} catch { Add-Result 'Native GNOME bridge' $false $_.Exception.ToString() }

try {
    $cfg = Get-Content -Raw -LiteralPath (Join-Path $root 'config.json') | ConvertFrom-Json
    $valid = ($cfg.panelHeight -ge 24 -and $cfg.panelHeight -le 60 -and $cfg.maxLauncherApps -ge 10)
    Add-Result 'Configuration' $valid 'config.json parsed'
} catch { Add-Result 'Configuration' $false $_.Exception.Message }

try {
    $loaded = @{}
    foreach ($xamlPath in @('ui\Panel.xaml','ui\Activities.xaml','ui\Calendar.xaml','ui\QuickSettings.xaml')) {
        [xml]$xaml = Get-Content -Raw -LiteralPath (Join-Path $root $xamlPath)
        $reader = New-Object System.Xml.XmlNodeReader $xaml
        $window = [Windows.Markup.XamlReader]::Load($reader)
        $loaded[$xamlPath] = $window
    }
    $required = @(
        @('ui\Panel.xaml', 'ActivitiesButton','ClockButton','QuickButton','PanelBatteryFill'),
        @('ui\Activities.xaml', 'SearchBox','AppPanel','WorkspacePreview','WorkspaceCard','ShowAppsButton','SearchResultsPanel'),
        @('ui\Calendar.xaml', 'DateTitle','CalendarDaysPanel','PrevMonthButton','NextMonthButton','OpenCalendarButton'),
        @('ui\QuickSettings.xaml', 'WifiButton','BluetoothButton','VolumeSlider','BrightnessSlider','SettingsButton','BatteryText','ThemeLightButton','ThemeDarkButton','ThemeSystemButton','AccentBlueButton')
    )
    foreach ($entry in $required) {
        $w = $loaded[$entry[0]]
        foreach ($name in $entry[1..($entry.Count-1)]) {
            if ($null -eq $w.FindName($name)) { throw "Missing named UI control: $name in $($entry[0])" }
        }
    }
    foreach ($w in $loaded.Values) { $w.Close() }
    Add-Result 'WPF UI files' $true 'All four modern GNOME-style XAML windows loaded and required controls resolved'
} catch { Add-Result 'WPF UI files' $false $_.Exception.Message }

try {
    $uiFiles = Get-ChildItem -LiteralPath (Join-Path $root 'ui') -Filter '*.xaml' -File
    $badUnicode = @()
    foreach ($ui in $uiFiles) {
        $raw = Get-Content -Raw -Encoding UTF8 -LiteralPath $ui.FullName
        if ($raw -match '[^\x00-\x7F]') { $badUnicode += $ui.Name }
    }
    $calendarRaw = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $root 'ui\Calendar.xaml')
    $noLegacyCalendar = ($calendarRaw -notmatch '<Calendar(?:\s|>)')
    Add-Result 'Modern UI regression guards' ($badUnicode.Count -eq 0 -and $noLegacyCalendar) ('ASCII-safe vector UI; custom calendar={0}' -f $noLegacyCalendar)
} catch { Add-Result 'Modern UI regression guards' $false $_.Exception.Message }

try {
    $screenWidth = [System.Windows.SystemParameters]::PrimaryScreenWidth
    $screenHeight = [System.Windows.SystemParameters]::PrimaryScreenHeight
    $validScreen = ($screenWidth -gt 0 -and $screenHeight -gt 0)
    Add-Result 'WPF screen metrics' $validScreen ("{0} x {1} DIPs" -f $screenWidth, $screenHeight)
} catch { Add-Result 'WPF screen metrics' $false $_.Exception.Message }

try {
    $runtime = Join-Path $env:LOCALAPPDATA 'FedoraWin'
    New-Item -ItemType Directory -Force -Path $runtime | Out-Null
    $probe = Join-Path $runtime 'preflight.tmp'
    Set-Content -LiteralPath $probe -Value 'ok' -Encoding ASCII
    Remove-Item -LiteralPath $probe -Force
    Add-Result 'Recovery storage' $true $runtime
} catch { Add-Result 'Recovery storage' $false $_.Exception.Message }

try {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class FedoraWinPreflightNative {
    [DllImport("user32.dll", CharSet=CharSet.Auto)]
    public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
}
'@
    $taskbar = [FedoraWinPreflightNative]::FindWindow('Shell_TrayWnd', $null)
    Add-Result 'Windows taskbar handle' ($taskbar -ne [IntPtr]::Zero) ("HWND 0x{0:X}" -f $taskbar.ToInt64())
} catch { Add-Result 'Windows taskbar handle' $false $_.Exception.Message }

try {
    $screenCount = [System.Windows.Forms.Screen]::AllScreens.Count
    Add-Result 'Display detection' ($screenCount -ge 1) ("{0} display(s); v0.3 themes the primary display" -f $screenCount)
} catch { Add-Result 'Display detection' $false $_.Exception.Message }

try {
    $wallpaper = (Get-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name WallPaper -ErrorAction Stop).WallPaper
    if ($wallpaper -and (Test-Path -LiteralPath $wallpaper)) {
        Add-Result 'Wallpaper recovery path' $true $wallpaper
    } else {
        Add-Result 'Wallpaper recovery path' $true 'Current wallpaper is not a normal file; FedoraWin will leave it unchanged.'
    }
} catch {
    Add-Result 'Wallpaper recovery path' $true 'Wallpaper path unavailable; FedoraWin will leave it unchanged.'
}

Write-Host ''
Write-Host 'FedoraWin preflight' -ForegroundColor Cyan
Write-Host '-------------------' -ForegroundColor DarkGray
foreach ($r in $results) {
    if ($r.Pass) {
        Write-Host ('[PASS] {0}: {1}' -f $r.Name, $r.Detail) -ForegroundColor Green
    } else {
        Write-Host ('[FAIL] {0}: {1}' -f $r.Name, $r.Detail) -ForegroundColor Red
    }
}

$failed = @($results | Where-Object { -not $_.Pass })
Write-Host ''
if ($failed.Count -eq 0) {
    Write-Host 'Preflight passed. Run Safe-Preview.cmd next.' -ForegroundColor Green
    exit 0
}

Write-Host ("Preflight failed: {0} check(s). Do NOT run full mode yet." -f $failed.Count) -ForegroundColor Red
exit 1
