param([string]$OutputDirectory = 'artifacts\site-preview')

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class FedoraWinCaptureNative {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);

    public static IntPtr FindVisibleWindow(int processId, string exactTitle) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hwnd, _) => {
            uint pid;
            GetWindowThreadProcessId(hwnd, out pid);
            if (pid != (uint)processId || !IsWindowVisible(hwnd)) return true;
            var title = new StringBuilder(256);
            GetWindowText(hwnd, title, title.Capacity);
            if (String.Equals(title.ToString(), exactTitle, StringComparison.Ordinal)) {
                found = hwnd;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exe = Join-Path $repoRoot 'src-tauri\target\release\fedorawin.exe'
if (-not (Test-Path -LiteralPath $exe)) {
    throw "FedoraWin executable not found at $exe. Build release first."
}

$output = if ([IO.Path]::IsPathRooted($OutputDirectory)) {
    [IO.Path]::GetFullPath($OutputDirectory)
} else {
    [IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory))
}
New-Item -ItemType Directory -Path $output -Force | Out-Null

$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$sourceBranch = (& git -C $repoRoot rev-parse --abbrev-ref HEAD).Trim()
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$captures = [ordered]@{}

function Wait-Window {
    param(
        [Parameter(Mandatory)][int]$ProcessId,
        [Parameter(Mandatory)][string]$Title,
        [int]$TimeoutSeconds = 20
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $hwnd = [FedoraWinCaptureNative]::FindVisibleWindow($ProcessId, $Title)
        if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for window '$Title' in process $ProcessId."
}

function Save-WindowCapture {
    param(
        [Parameter(Mandatory)][IntPtr]$Hwnd,
        [Parameter(Mandatory)][string]$FileName
    )

    $rect = New-Object FedoraWinCaptureNative+RECT
    if (-not [FedoraWinCaptureNative]::GetWindowRect($Hwnd, [ref]$rect)) {
        throw "GetWindowRect failed for $FileName."
    }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -lt 24 -or $height -lt 24) {
        throw "Capture '$FileName' has invalid window bounds ${width}x${height}."
    }

    $path = Join-Path $output $FileName
    $bitmap = New-Object System.Drawing.Bitmap($width, $height, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($width,$height)), [System.Drawing.CopyPixelOperation]::SourceCopy)
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }

    $file = Get-Item -LiteralPath $path
    if ($file.Length -lt 2048) {
        throw "Capture '$FileName' is unexpectedly small ($($file.Length) bytes)."
    }
    return $path
}

function Invoke-Capture {
    param(
        [Parameter(Mandatory)][string]$Key,
        [Parameter(Mandatory)][string]$View,
        [Parameter(Mandatory)][string]$WindowLabel,
        [string]$Mode = '',
        [string]$Theme = 'dark',
        [int]$SettleMilliseconds = 1400
    )

    $env:FEDORAWIN_CAPTURE_VIEW = $View
    $env:FEDORAWIN_CAPTURE_MODE = $Mode
    $env:FEDORAWIN_CAPTURE_THEME = $Theme
    $process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
    try {
        $hwnd = Wait-Window -ProcessId $process.Id -Title "FedoraWin — $WindowLabel"
        Start-Sleep -Milliseconds $SettleMilliseconds
        $fileName = "$Key.png"
        [void](Save-WindowCapture -Hwnd $hwnd -FileName $fileName)
        $captures[$Key] = $fileName
    } finally {
        if ($process -and -not $process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 350
        }
        Remove-Item Env:FEDORAWIN_CAPTURE_VIEW -ErrorAction SilentlyContinue
        Remove-Item Env:FEDORAWIN_CAPTURE_MODE -ErrorAction SilentlyContinue
        Remove-Item Env:FEDORAWIN_CAPTURE_THEME -ErrorAction SilentlyContinue
    }
}

Invoke-Capture -Key 'panel' -View 'panel' -WindowLabel 'panel' -SettleMilliseconds 900
Invoke-Capture -Key 'activities' -View 'activities' -WindowLabel 'activities' -SettleMilliseconds 1900
Invoke-Capture -Key 'apps' -View 'activities' -WindowLabel 'activities' -Mode 'apps' -SettleMilliseconds 2100
Invoke-Capture -Key 'search_terminal' -View 'activities' -WindowLabel 'activities' -Mode 'search-terminal' -SettleMilliseconds 2100
Invoke-Capture -Key 'quick_settings_dark' -View 'quick-settings' -WindowLabel 'quick-settings' -Theme 'dark'
Invoke-Capture -Key 'quick_settings_light' -View 'quick-settings' -WindowLabel 'quick-settings' -Theme 'light'
Invoke-Capture -Key 'appearance' -View 'quick-settings' -WindowLabel 'quick-settings' -Mode 'appearance' -Theme 'dark'
Invoke-Capture -Key 'date_menu' -View 'date-menu' -WindowLabel 'date-menu' -Theme 'dark'

$hashes = [ordered]@{}
foreach ($entry in $captures.GetEnumerator()) {
    $hashes[$entry.Key] = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $output $entry.Value)).Hash.ToLowerInvariant()
}
if (($hashes.Values | Select-Object -Unique).Count -lt 7) {
    throw 'Runtime gallery captures are not sufficiently distinct.'
}
if ($hashes.quick_settings_dark -eq $hashes.quick_settings_light) {
    throw 'Quick Settings dark/light captures are identical.'
}

[ordered]@{
    source_branch = $sourceBranch
    source_sha = $sourceSha
    captured_utc = [DateTime]::UtcNow.ToString('o')
    runtime = 'Tauri + Rust + WebView2'
    launch_executable = 'fedorawin.exe'
    runner_os = $env:RUNNER_OS
    runner_name = $env:RUNNER_NAME
    runner_image = $env:ImageOS
    github_run_id = $env:GITHUB_RUN_ID
    github_run_number = $env:GITHUB_RUN_NUMBER
    screen = [ordered]@{ width = $screen.Width; height = $screen.Height }
    screenshots = $captures
    sha256 = $hashes
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'preview-metadata.json') -Encoding UTF8

Write-Host "Captured $($captures.Count) real FedoraWin.exe gallery surfaces to $output"
