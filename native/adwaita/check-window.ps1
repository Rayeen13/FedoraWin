Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AdwaitaProbe {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr unused);
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr unused);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder title, int length);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int length);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern IntPtr FindWindowW(string className, string title);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hwnd);
}
'@
$out = Join-Path $PSScriptRoot 'out'
$exe = Join-Path $out 'fedorawin-adwaita.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "Native executable absent: $exe" }
if (-not $env:MSYS2_ROOT) { throw 'MSYS2_ROOT must come from setup-msys2 output.' }
$ucrtBin = Join-Path $env:MSYS2_ROOT 'ucrt64\bin'
if (-not (Test-Path -LiteralPath (Join-Path $ucrtBin 'libadwaita-1-0.dll'))) {
    throw "Missing UCRT64 libadwaita runtime: $ucrtBin"
}
$env:PATH = "$ucrtBin;$env:PATH"
$title = 'FedoraWin Adwaita Native Probe'
$env:GDK_BACKEND = 'win32'
$env:GSETTINGS_BACKEND = 'memory'

function Get-ProcessWindows {
    param([int]$OwnerProcessId)
    $windows = [System.Collections.Generic.List[object]]::new()
    $callback = [AdwaitaProbe+EnumWindowsProc] {
        param([IntPtr]$handle, [IntPtr]$unused)
        $owner = [uint32]0
        [void][AdwaitaProbe]::GetWindowThreadProcessId($handle, [ref]$owner)
        if ($owner -eq $OwnerProcessId) {
            $windowTitle = [Text.StringBuilder]::new(512)
            $className = [Text.StringBuilder]::new(256)
            [void][AdwaitaProbe]::GetWindowTextW($handle, $windowTitle, $windowTitle.Capacity)
            [void][AdwaitaProbe]::GetClassNameW($handle, $className, $className.Capacity)
            $windows.Add([pscustomobject]@{
                Handle = $handle
                Title = $windowTitle.ToString()
                Class = $className.ToString()
                Visible = [AdwaitaProbe]::IsWindowVisible($handle)
            })
        }
        return $true
    }
    [void][AdwaitaProbe]::EnumWindows($callback, [IntPtr]::Zero)
    return $windows.ToArray()
}

foreach ($theme in @('dark','light')) {
    $env:FEDORAWIN_ADWAITA_THEME = $theme
    $stdout = Join-Path $out "adwaita-$theme.stdout.txt"
    $stderr = Join-Path $out "adwaita-$theme.stderr.txt"
    $process = Start-Process -FilePath $exe -WorkingDirectory $out -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    try {
        $hwnd = [IntPtr]::Zero
        for ($attempt=0; $attempt -lt 150; $attempt++) {
            if ($process.HasExited) {
                $errors = try { Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop } catch { '<unavailable>' }
                throw "Adwaita $theme exited: $($process.ExitCode); stderr=$errors; MSYS2_ROOT=$env:MSYS2_ROOT"
            }
            $window = @(Get-ProcessWindows -OwnerProcessId $process.Id | Where-Object {
                $_.Visible -and ($_.Title -eq $title -or $_.Class -match "gdk|gtk")
            } | Select-Object -First 1)
            if ($window.Count) { $hwnd=$window[0].Handle; break }
            Start-Sleep -Milliseconds 200
        }
        if ($hwnd -eq [IntPtr]::Zero) {
            $windows = @(Get-ProcessWindows -OwnerProcessId $process.Id | ForEach-Object {
                "HWND=$($_.Handle) title=$($_.Title) class=$($_.Class) visible=$($_.Visible)"
            }) -join "; "
            $errors = try { (Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop) } catch { "<unavailable>" }
            $output = try { (Get-Content -LiteralPath $stdout -Raw -ErrorAction Stop) } catch { "<unavailable>" }
            throw "No visible real Adwaita HWND ($theme) pid=$($process.Id) windows=$windows stderr=$errors stdout=$output"
        }
        [void][AdwaitaProbe]::SetForegroundWindow($hwnd)
        Start-Sleep -Milliseconds 900
        $rect = New-Object AdwaitaProbe+RECT
        if (-not [AdwaitaProbe]::GetWindowRect($hwnd,[ref]$rect)) { throw 'Window bounds unavailable.' }
        $width=$rect.Right-$rect.Left
        $height=$rect.Bottom-$rect.Top
        if ($width -lt 300 -or $height -lt 250) { throw 'Invalid GTK4 window bounds.' }
        $bitmap=[Drawing.Bitmap]::new($width,$height)
        $graphics=[Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,[Drawing.Size]::new($width,$height))
            $path=Join-Path $out "adwaita-$theme.png"
            $bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $bitmap.Dispose()
        }
        if ((Get-Item -LiteralPath $path).Length -lt 4000) { throw "Capture uninitialized ($theme)." }
        Write-Host "ADWAITA: real $theme Win32 HWND captured ($width x $height)."
    } finally {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
}
Remove-Item Env:FEDORAWIN_ADWAITA_THEME -ErrorAction SilentlyContinue
