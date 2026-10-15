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
$probeExe = Join-Path $out 'fedorawin-adwaita.exe'
$preferencesExe = Join-Path $out 'fedorawin-preferences.exe'
foreach ($candidate in @($probeExe,$preferencesExe)) {
    if (-not (Test-Path -LiteralPath $candidate)) {
        throw "Native executable absent: $candidate"
    }
}
if (-not $env:MSYS2_ROOT) { throw 'MSYS2_ROOT must come from setup-msys2 output.' }

$ucrtBin = Join-Path $env:MSYS2_ROOT 'ucrt64\bin'
if (-not (Test-Path -LiteralPath (Join-Path $ucrtBin 'libadwaita-1-0.dll'))) {
    throw "Missing UCRT64 libadwaita runtime: $ucrtBin"
}
$env:PATH = "$ucrtBin;$env:PATH"
$env:GDK_BACKEND = 'win32'
$env:GSETTINGS_BACKEND = 'memory'
$memoryCeilingMb = 300

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

function Capture-AdwaitaSurface {
    param(
        [string]$Executable,
        [string]$ExpectedTitle,
        [string]$Theme,
        [string]$CapturePrefix,
        [string]$Surface
    )

    $env:FEDORAWIN_ADWAITA_THEME = $Theme
    $stdout = Join-Path $out "$CapturePrefix-$Theme.stdout.txt"
    $stderr = Join-Path $out "$CapturePrefix-$Theme.stderr.txt"
    $process = Start-Process -FilePath $Executable -WorkingDirectory $out -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr

    try {
        $hwnd = [IntPtr]::Zero
        for ($attempt=0; $attempt -lt 150; $attempt++) {
            $process.Refresh()
            if ($process.HasExited) {
                $errors = try { Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop } catch { '<unavailable>' }
                throw "$Surface $Theme exited: $($process.ExitCode); stderr=$errors"
            }
            $window = @(Get-ProcessWindows -OwnerProcessId $process.Id | Where-Object {
                $_.Visible -and $_.Title -eq $ExpectedTitle
            } | Select-Object -First 1)
            if ($window.Count) { $hwnd=$window[0].Handle; break }
            Start-Sleep -Milliseconds 200
        }

        if ($hwnd -eq [IntPtr]::Zero) {
            $windows = @(Get-ProcessWindows -OwnerProcessId $process.Id | ForEach-Object {
                "HWND=$($_.Handle) title=$($_.Title) class=$($_.Class) visible=$($_.Visible)"
            }) -join "; "
            $errors = try { Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop } catch { '<unavailable>' }
            throw "No visible $Surface HWND ($Theme) pid=$($process.Id) windows=$windows stderr=$errors"
        }

        [void][AdwaitaProbe]::SetForegroundWindow($hwnd)
        Start-Sleep -Milliseconds 900
        $rect = New-Object AdwaitaProbe+RECT
        if (-not [AdwaitaProbe]::GetWindowRect($hwnd,[ref]$rect)) { throw "$Surface bounds unavailable." }
        $width=$rect.Right-$rect.Left
        $height=$rect.Bottom-$rect.Top
        if ($width -lt 300 -or $height -lt 250) { throw "Invalid $Surface GTK4 bounds: $width x $height." }

        $bitmap=[Drawing.Bitmap]::new($width,$height)
        $graphics=[Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,[Drawing.Size]::new($width,$height))
            $path=Join-Path $out "$CapturePrefix-$Theme.png"
            $bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $bitmap.Dispose()
        }
        if ((Get-Item -LiteralPath $path).Length -lt 4000) { throw "Capture uninitialized ($Surface / $Theme)." }

        $process.Refresh()
        $workingSetMb=[Math]::Round($process.WorkingSet64 / 1MB, 1)
        if ($workingSetMb -ge $memoryCeilingMb) {
            throw "$Surface exceeded temporary GTK surface memory ceiling: $workingSetMb MB >= $memoryCeilingMb MB."
        }
        Write-Host "ADWAITA: real $Surface $Theme HWND captured ($width x $height), working set=$workingSetMb MB."
        return [pscustomobject]@{
            surface=$Surface
            theme=$Theme
            pid=$process.Id
            width=$width
            height=$height
            working_set_mb=$workingSetMb
            capture=(Split-Path -Leaf $path)
        }
    } finally {
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            [void]$process.WaitForExit(3000)
        }
    }
}

$results = [System.Collections.Generic.List[object]]::new()
foreach ($theme in @('dark','light')) {
    $results.Add((Capture-AdwaitaSurface -Executable $probeExe -ExpectedTitle 'FedoraWin Adwaita Native Probe' -Theme $theme -CapturePrefix 'adwaita' -Surface 'baseline-proof'))
    $results.Add((Capture-AdwaitaSurface -Executable $preferencesExe -ExpectedTitle 'FedoraWin Preferences' -Theme $theme -CapturePrefix 'adwaita-preferences' -Surface 'preferences'))
}

$memoryEvidence = [ordered]@{
    captured_utc = [DateTime]::UtcNow.ToString('o')
    ceiling_mb = $memoryCeilingMb
    note = 'Per-process working set for temporary GTK4/libadwaita surfaces; this does not replace the separate idle shell process-tree budget.'
    samples = $results
}
$memoryEvidence | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $out 'adwaita-memory.json') -Encoding UTF8
Remove-Item Env:FEDORAWIN_ADWAITA_THEME -ErrorAction SilentlyContinue
