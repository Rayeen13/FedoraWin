Set-StrictMode -Version Latest
$ErrorActionPreference = 'SilentlyContinue'

$runtimeDir = Join-Path $env:LOCALAPPDATA 'FedoraWin'
$statePath = Join-Path $runtimeDir 'state.json'
$restoreRequestPath = Join-Path $runtimeDir 'restore.request'
$pidPath = Join-Path $runtimeDir 'pid.txt'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class FedoraWinRestoreNative
{
    public const int SW_SHOW = 5;
    public const int SPI_SETDESKWALLPAPER = 20;
    public const int SPI_SETWORKAREA = 47;
    public const int SPIF_UPDATEINIFILE = 0x01;
    public const int SPIF_SENDWININICHANGE = 0x02;

    [DllImport("user32.dll", CharSet=CharSet.Auto)]
    public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
    [DllImport("user32.dll", CharSet=CharSet.Auto)]
    public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr childAfter, string className, string windowName);
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
    public static IntPtr GetDesktopListView()
    {
        IntPtr defView = IntPtr.Zero;
        IntPtr progman = FindWindow("Progman", null);
        if (progman != IntPtr.Zero) defView = FindWindowEx(progman, IntPtr.Zero, "SHELLDLL_DefView", null);
        if (defView == IntPtr.Zero)
        {
            EnumWindows(delegate(IntPtr hwnd, IntPtr lParam)
            {
                IntPtr child = FindWindowEx(hwnd, IntPtr.Zero, "SHELLDLL_DefView", null);
                if (child != IntPtr.Zero) { defView = child; return false; }
                return true;
            }, IntPtr.Zero);
        }
        if (defView == IntPtr.Zero) return IntPtr.Zero;
        return FindWindowEx(defView, IntPtr.Zero, "SysListView32", "FolderView");
    }
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Auto, EntryPoint="SystemParametersInfo")]
    public static extern bool SystemParametersInfoString(int uAction, int uParam, string lpvParam, int fuWinIni);
    [DllImport("user32.dll", EntryPoint="SystemParametersInfo")]
    public static extern bool SystemParametersInfoRect(int uAction, int uParam, ref RECT lpvParam, int fuWinIni);
    public static bool SetWorkArea(int left, int top, int right, int bottom)
    {
        RECT r = new RECT { Left = left, Top = top, Right = right, Bottom = bottom };
        return SystemParametersInfoRect(SPI_SETWORKAREA, 0, ref r, SPIF_SENDWININICHANGE);
    }
}
'@

New-Item -ItemType Directory -Force -Path $runtimeDir | Out-Null
Set-Content -LiteralPath $restoreRequestPath -Value 'restore' -Encoding ASCII

$taskbar = [FedoraWinRestoreNative]::FindWindow('Shell_TrayWnd', $null)
$desktopIcons = [FedoraWinRestoreNative]::GetDesktopListView()
$state = $null

if (Test-Path -LiteralPath $statePath) {
    $state = Get-Content -Raw -Encoding UTF8 -LiteralPath $statePath | ConvertFrom-Json
    if ($state.originalWorkArea -and $state.originalWorkArea.Count -eq 4) {
        [FedoraWinRestoreNative]::SetWorkArea(
            [int]$state.originalWorkArea[0],
            [int]$state.originalWorkArea[1],
            [int]$state.originalWorkArea[2],
            [int]$state.originalWorkArea[3]
        ) | Out-Null
    }
    if ($state.originalWallpaper -and (Test-Path -LiteralPath $state.originalWallpaper)) {
        [FedoraWinRestoreNative]::SystemParametersInfoString(
            [FedoraWinRestoreNative]::SPI_SETDESKWALLPAPER,
            0,
            [string]$state.originalWallpaper,
            [FedoraWinRestoreNative]::SPIF_UPDATEINIFILE -bor [FedoraWinRestoreNative]::SPIF_SENDWININICHANGE
        ) | Out-Null
    }
}

if ($taskbar -ne [IntPtr]::Zero) {
    $showTaskbar = $true
    if ($state -and $state.PSObject.Properties.Name -contains 'taskbarWasVisible') { $showTaskbar = [bool]$state.taskbarWasVisible }
    [FedoraWinRestoreNative]::ShowWindow($taskbar, $(if ($showTaskbar) { 5 } else { 0 })) | Out-Null
}
if ($desktopIcons -ne [IntPtr]::Zero) {
    $showIcons = $true
    if ($state -and $state.PSObject.Properties.Name -contains 'desktopIconsWereVisible') { $showIcons = [bool]$state.desktopIconsWereVisible }
    [FedoraWinRestoreNative]::ShowWindow($desktopIcons, $(if ($showIcons) { 5 } else { 0 })) | Out-Null
}

Start-Sleep -Milliseconds 900

if (Test-Path -LiteralPath $pidPath) {
    $candidate = 0
    if ([int]::TryParse((Get-Content -Raw -LiteralPath $pidPath).Trim(), [ref]$candidate)) {
        $proc = Get-CimInstance Win32_Process -Filter "ProcessId = $candidate"
        if ($proc -and $proc.CommandLine -like '*FedoraWin.ps1*') {
            Stop-Process -Id $candidate -Force
        }
    }
}

# If the process had to be killed, its AppBar HWND has only just disappeared.
# Apply the saved work area once more after that destruction so Windows cannot
# leave a stale top reservation behind.
if ($state -and $state.originalWorkArea -and $state.originalWorkArea.Count -eq 4) {
    [FedoraWinRestoreNative]::SetWorkArea(
        [int]$state.originalWorkArea[0],
        [int]$state.originalWorkArea[1],
        [int]$state.originalWorkArea[2],
        [int]$state.originalWorkArea[3]
    ) | Out-Null
}
if ($taskbar -ne [IntPtr]::Zero) {
    $showTaskbar = $true
    if ($state -and $state.PSObject.Properties.Name -contains 'taskbarWasVisible') { $showTaskbar = [bool]$state.taskbarWasVisible }
    [FedoraWinRestoreNative]::ShowWindow($taskbar, $(if ($showTaskbar) { 5 } else { 0 })) | Out-Null
}
if ($desktopIcons -ne [IntPtr]::Zero) {
    $showIcons = $true
    if ($state -and $state.PSObject.Properties.Name -contains 'desktopIconsWereVisible') { $showIcons = [bool]$state.desktopIconsWereVisible }
    [FedoraWinRestoreNative]::ShowWindow($desktopIcons, $(if ($showIcons) { 5 } else { 0 })) | Out-Null
}

Remove-Item -Force -ErrorAction SilentlyContinue $pidPath
Write-Host 'Windows taskbar, desktop icons, work area, and previous wallpaper restored.'
