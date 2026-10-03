Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class FedoraWinPortableProbe {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr unused);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr unused);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder title, int length);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);
}
'@

$out = Join-Path $PSScriptRoot 'out'
$runtime = Join-Path $out 'preferences-runtime'
$exe = Join-Path $runtime 'fedorawin-preferences.exe'
$manifestPath = Join-Path $out 'portable-runtime.json'
foreach ($required in @($exe, $manifestPath)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Portable runtime staging output is missing: $required"
    }
}
if (-not $env:MSYS2_ROOT) {
    throw 'MSYS2_ROOT must come from setup-msys2 output.'
}

function Find-PreferencesWindow {
    param([int]$ProcessId)
    $found = [IntPtr]::Zero
    $callback = [FedoraWinPortableProbe+EnumWindowsProc] {
        param([IntPtr]$hwnd, [IntPtr]$unused)
        $owner = [uint32]0
        [void][FedoraWinPortableProbe]::GetWindowThreadProcessId($hwnd, [ref]$owner)
        if ($owner -ne $ProcessId -or -not [FedoraWinPortableProbe]::IsWindowVisible($hwnd)) {
            return $true
        }
        $title = [Text.StringBuilder]::new(256)
        [void][FedoraWinPortableProbe]::GetWindowTextW($hwnd, $title, $title.Capacity)
        if ($title.ToString() -eq 'FedoraWin Preferences') {
            $script:portableWindow = $hwnd
            return $false
        }
        return $true
    }
    $script:portableWindow = [IntPtr]::Zero
    [void][FedoraWinPortableProbe]::EnumWindows($callback, [IntPtr]::Zero)
    return $script:portableWindow
}

$old = @{
    PATH = $env:PATH
    GDK_BACKEND = $env:GDK_BACKEND
    GSETTINGS_BACKEND = $env:GSETTINGS_BACKEND
    GSETTINGS_SCHEMA_DIR = $env:GSETTINGS_SCHEMA_DIR
    XDG_DATA_DIRS = $env:XDG_DATA_DIRS
    FEDORAWIN_ADWAITA_THEME = $env:FEDORAWIN_ADWAITA_THEME
}
$sandbox = Join-Path $env:RUNNER_TEMP 'fedorawin-portable-preferences'
New-Item -ItemType Directory -Path $sandbox -Force | Out-Null
$stdout = Join-Path $sandbox 'stdout.txt'
$stderr = Join-Path $sandbox 'stderr.txt'
$process = $null

try {
    $env:PATH = "$runtime;$env:SystemRoot\\System32;$env:SystemRoot"
    $env:GDK_BACKEND = 'win32'
    $env:GSETTINGS_BACKEND = 'memory'
    $env:GSETTINGS_SCHEMA_DIR = Join-Path $runtime 'share\\glib-2.0\\schemas'
    $env:XDG_DATA_DIRS = Join-Path $runtime 'share'
    $env:FEDORAWIN_ADWAITA_THEME = 'dark'

    $process = Start-Process -FilePath $exe -WorkingDirectory $sandbox -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $hwnd = [IntPtr]::Zero
    for ($attempt = 0; $attempt -lt 150; $attempt++) {
        $process.Refresh()
        if ($process.HasExited) {
            $errors = try { Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop } catch { '<unavailable>' }
            throw "Portable Preferences exited before creating its HWND: exit=$($process.ExitCode); stderr=$errors"
        }
        $hwnd = Find-PreferencesWindow -ProcessId $process.Id
        if ($hwnd -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 200
    }
    if ($hwnd -eq [IntPtr]::Zero) {
        $errors = try { Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop } catch { '<unavailable>' }
        throw "Portable Preferences did not create a visible HWND. stderr=$errors"
    }

    Start-Sleep -Milliseconds 500
    $process.Refresh()
    $modules = @(Get-Process -Id $process.Id -Module -ErrorAction Stop)
    $runtimeRoot = [IO.Path]::GetFullPath($runtime).TrimEnd('\\')
    $windowsRoot = [IO.Path]::GetFullPath($env:SystemRoot).TrimEnd('\\')
    $msysRoot = [IO.Path]::GetFullPath($env:MSYS2_ROOT).TrimEnd('\\')
    $unexpected = [System.Collections.Generic.List[string]]::new()
    foreach ($module in $modules) {
        $path = $module.FileName
        if (-not $path) { continue }
        $full = [IO.Path]::GetFullPath($path)
        if ($full.StartsWith($runtimeRoot, [StringComparison]::OrdinalIgnoreCase)) { continue }
        if ($full.StartsWith($windowsRoot, [StringComparison]::OrdinalIgnoreCase)) { continue }
        $unexpected.Add($full)
    }

    if ($unexpected.Count) {
        throw "Portable Preferences loaded modules outside its staged runtime or Windows: $($unexpected -join ' | ')"
    }
    if ($modules.FileName | Where-Object { $_ -and ([IO.Path]::GetFullPath($_)).StartsWith($msysRoot, [StringComparison]::OrdinalIgnoreCase) }) {
        throw 'Portable Preferences still loaded a module directly from the MSYS2 installation.'
    }

    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    Write-Host "PORTABLE VERIFIED: FedoraWin Preferences HWND launched with MSYS2 removed from PATH; staged size=$($manifest.total_mb) MB; loaded modules=$($modules.Count)."
} finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        [void]$process.WaitForExit(3000)
    }
    foreach ($name in $old.Keys) {
        $value = $old[$name]
        if ($null -eq $value) {
            Remove-Item "Env:$name" -ErrorAction SilentlyContinue
        } else {
            Set-Item "Env:$name" $value
        }
    }
}
