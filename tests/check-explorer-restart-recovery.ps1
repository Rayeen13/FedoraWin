# Verify FedoraWin survives an Explorer restart and adopts/restores the replacement
# taskbar HWND without ever terminating Explorer in product code. This test itself
# restarts Explorer only inside the disposable Windows CI runner.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class FedoraWinExplorerRestartProbe {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr data);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr data);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int capacity);

    [DllImport("user32.dll")]
    public static extern bool IsWindow(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hwnd, int command);
}
'@

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exe = Join-Path $root 'src-tauri\target\release\fedorawin.exe'
if (-not (Test-Path -LiteralPath $exe)) {
    throw 'Build the real fedorawin.exe before running Explorer restart recovery test.'
}

function Get-ClassName {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    $buffer = [Text.StringBuilder]::new(128)
    $length = [FedoraWinExplorerRestartProbe]::GetClassNameW($Hwnd, $buffer, $buffer.Capacity)
    if ($length -le 0) { return '' }
    return $buffer.ToString()
}

function Test-Taskbar {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    if ($Hwnd -eq [IntPtr]::Zero -or -not [FedoraWinExplorerRestartProbe]::IsWindow($Hwnd)) {
        return $false
    }
    $class = Get-ClassName -Hwnd $Hwnd
    return $class -eq 'Shell_TrayWnd' -or $class -eq 'Shell_SecondaryTrayWnd'
}

function Get-Taskbars {
    $handles = [System.Collections.Generic.List[IntPtr]]::new()
    $callback = [FedoraWinExplorerRestartProbe+EnumWindowsProc]{
        param([IntPtr]$hwnd, [IntPtr]$data)
        if ((Test-Taskbar -Hwnd $hwnd) -and -not $handles.Contains($hwnd)) {
            $handles.Add($hwnd)
        }
        return $true
    }
    [void][FedoraWinExplorerRestartProbe]::EnumWindows($callback, [IntPtr]::Zero)
    return @($handles)
}

function Get-OwnerPid {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    [uint32]$ownerPid = 0
    $threadId = [FedoraWinExplorerRestartProbe]::GetWindowThreadProcessId($Hwnd, [ref]$ownerPid)
    if ($threadId -eq 0) { return 0 }
    return [int]$ownerPid
}

# Do not trust a taskbar-like class or a process name by itself.
$script:windowsExplorer = [IO.Path]::GetFullPath((Join-Path $env:SystemRoot 'explorer.exe'))

function Test-RealExplorerProcess {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Process)
    try {
        return [string]::Equals(
            $Process.Path,
            $script:windowsExplorer,
            [StringComparison]::OrdinalIgnoreCase
        )
    } catch {
        return $false
    }
}

function Get-RealExplorerProcesses {
    return @(Get-Process -Name explorer -ErrorAction SilentlyContinue |
        Where-Object { Test-RealExplorerProcess -Process $_ })
}

function Test-RealExplorerTaskbar {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    if (-not (Test-Taskbar -Hwnd $Hwnd)) { return $false }
    $ownerPid = Get-OwnerPid -Hwnd $Hwnd
    if ($ownerPid -eq 0) { return $false }
    try {
        $owner = Get-Process -Id $ownerPid -ErrorAction Stop
        $created = $owner.StartTime.ToUniversalTime().Ticks
        if (-not (Test-RealExplorerProcess -Process $owner)) { return $false }
        $current = Get-Process -Id $ownerPid -ErrorAction Stop
        return (Test-RealExplorerProcess -Process $current) -and
            $current.StartTime.ToUniversalTime().Ticks -eq $created -and
            (Get-OwnerPid -Hwnd $Hwnd) -eq $ownerPid
    } catch {
        return $false
    }
}

function Ensure-ExplorerRunning {
    if (@(Get-RealExplorerProcesses).Count -eq 0) {
        Start-Process -FilePath $script:windowsExplorer | Out-Null
    }
}

$baselineExplorerProcesses = @(Get-RealExplorerProcesses)
$baselineExplorer = @($baselineExplorerProcesses | Select-Object -ExpandProperty Id)
$baselineStartTicks = @{}
foreach ($process in $baselineExplorerProcesses) {
    $baselineStartTicks[[int]$process.Id] = $process.StartTime.ToUniversalTime().Ticks
}
if ($baselineExplorer.Count -eq 0) {
    throw 'Explorer is not running before the restart test.'
}
$baselineTaskbars = @(Get-Taskbars | Where-Object {
    (Test-RealExplorerTaskbar -Hwnd $_) -and [FedoraWinExplorerRestartProbe]::IsWindowVisible($_)
})
if ($baselineTaskbars.Count -eq 0) {
    throw 'No visible Explorer taskbar exists before the restart test.'
}
$baselineHandles = @($baselineTaskbars | ForEach-Object { $_.ToInt64() })

$env:FEDORAWIN_CAPTURE_VIEW = 'panel'
$env:FEDORAWIN_CAPTURE_MODE = ''
$env:FEDORAWIN_CAPTURE_THEME = 'dark'
Remove-Item Env:FEDORAWIN_KEEP_WINDOWS_TASKBAR -ErrorAction SilentlyContinue
$shell = $null
$replacementHwnd = [IntPtr]::Zero
$replacementExplorerPid = 0
$replacementExplorerStartTicks = 0

function Test-ReplacementTaskbar {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    if (-not (Test-RealExplorerTaskbar -Hwnd $Hwnd) -or
        (Get-OwnerPid -Hwnd $Hwnd) -ne $replacementExplorerPid) {
        return $false
    }
    try {
        $owner = Get-Process -Id $replacementExplorerPid -ErrorAction Stop
        return $owner.StartTime.ToUniversalTime().Ticks -eq $replacementExplorerStartTicks
    } catch {
        return $false
    }
}

try {
    $shell = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru

    $hidden = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($shell.HasExited) { throw "FedoraWin exited before baseline taskbar hide: $($shell.ExitCode)" }
        foreach ($hwnd in $baselineTaskbars) {
            if (-not (Test-RealExplorerTaskbar -Hwnd $hwnd) -or
                $baselineStartTicks[(Get-OwnerPid -Hwnd $hwnd)] -ne
                    (Get-Process -Id (Get-OwnerPid -Hwnd $hwnd)).StartTime.ToUniversalTime().Ticks) {
                throw 'Baseline Explorer taskbar changed owner lifetime before restart.'
            }
        }
        $visible = @($baselineTaskbars | Where-Object {
            [FedoraWinExplorerRestartProbe]::IsWindowVisible($_)
        })
        if ($visible.Count -eq 0) { $hidden = $true; break }
        Start-Sleep -Milliseconds 100
    }
    if (-not $hidden) {
        throw 'FedoraWin did not hide the baseline Explorer taskbar before restart.'
    }

    Write-Host ("EXPLORER RESTART BASELINE: PID(s)={0}; taskbar HWND(s)={1}" -f
        ($baselineExplorer -join ','),
        ($baselineHandles -join ','))

    # CI fault injection only: product code never calls Stop-Process on Explorer.
    foreach ($explorerPid in $baselineExplorer) {
        $owner = Get-Process -Id $explorerPid -ErrorAction Stop
        if (-not (Test-RealExplorerProcess -Process $owner) -or
            $owner.StartTime.ToUniversalTime().Ticks -ne $baselineStartTicks[$explorerPid]) {
            throw "Refusing to terminate a recycled or non-Explorer PID $explorerPid."
        }
        Stop-Process -Id $explorerPid -Force -ErrorAction Stop
    }

    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($shell.HasExited) { throw "FedoraWin exited when Explorer restarted: $($shell.ExitCode)" }

        if ($attempt -eq 30) {
            Ensure-ExplorerRunning
        }

        $explorerNow = @(Get-RealExplorerProcesses)
        $newProcesses = @($explorerNow | Where-Object {
            -not $baselineStartTicks.ContainsKey([int]$_.Id) -or
            $_.StartTime.ToUniversalTime().Ticks -ne $baselineStartTicks[[int]$_.Id]
        })
        $newPids = @($newProcesses | Select-Object -ExpandProperty Id)
        foreach ($hwnd in @(Get-Taskbars)) {
            $ownerPid = Get-OwnerPid -Hwnd $hwnd
            # Win32 may recycle a numerical HWND: identify the new taskbar by
            # Explorer process lifetime, not by comparing old/new HWND numbers.
            if ($newPids -contains $ownerPid -and (Test-RealExplorerTaskbar -Hwnd $hwnd)) {
                $replacementHwnd = $hwnd
                $replacementExplorerPid = $ownerPid
                $replacementExplorerStartTicks = (
                    $newProcesses | Where-Object { $_.Id -eq $ownerPid } | Select-Object -First 1
                ).StartTime.ToUniversalTime().Ticks
                break
            }
        }
        if ($replacementHwnd -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 100
    }

    if ($replacementHwnd -eq [IntPtr]::Zero) {
        throw 'Explorer did not create a replacement taskbar HWND after restart.'
    }

    $adopted = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($shell.HasExited) { throw "FedoraWin exited while adopting replacement taskbar: $($shell.ExitCode)" }
        if (-not (Test-ReplacementTaskbar -Hwnd $replacementHwnd)) {
            throw 'Replacement Explorer taskbar changed identity before FedoraWin could adopt it.'
        }
        if (-not [FedoraWinExplorerRestartProbe]::IsWindowVisible($replacementHwnd)) {
            $adopted = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $adopted) {
        throw 'FedoraWin did not hide the replacement Explorer taskbar after Explorer restart.'
    }

    if (-not (Test-ReplacementTaskbar -Hwnd $replacementHwnd)) {
        throw 'Replacement Explorer process lifetime changed while FedoraWin remained active.'
    }
    Write-Host ("EXPLORER RESTART ADOPTED: new Explorer PID={0}; replacement taskbar HWND={1}; FedoraWin PID={2}" -f
        $replacementExplorerPid,
        $replacementHwnd.ToInt64(),
        $shell.Id)

    Stop-Process -Id $shell.Id -Force -ErrorAction Stop
    $shell.WaitForExit()

    $restored = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ((Test-ReplacementTaskbar -Hwnd $replacementHwnd) -and
            [FedoraWinExplorerRestartProbe]::IsWindowVisible($replacementHwnd)) {
            $restored = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $restored) {
        throw 'Replacement Explorer taskbar was not restored after force-killing FedoraWin.'
    }

    if (-not (Test-ReplacementTaskbar -Hwnd $replacementHwnd)) {
        throw 'FedoraWin recovery required another Explorer lifetime or changed taskbar ownership.'
    }

    Write-Host ("EXPLORER RESTART RECOVERY PASSED: FedoraWin stayed alive across Explorer restart, adopted replacement HWND {0}, then restored it while Explorer PID {1} remained intact." -f
        $replacementHwnd.ToInt64(),
        $replacementExplorerPid)
} finally {
    if ($shell -and -not $shell.HasExited) {
        Stop-Process -Id $shell.Id -Force -ErrorAction SilentlyContinue
    }

    Ensure-ExplorerRunning
    Start-Sleep -Milliseconds 500
    foreach ($hwnd in @(Get-Taskbars)) {
        # Never show a spoofed taskbar-class window or recycled HWND owned by
        # a different process. Check the genuine Explorer lifetime at cleanup.
        if ((Test-RealExplorerTaskbar -Hwnd $hwnd) -and
            -not [FedoraWinExplorerRestartProbe]::IsWindowVisible($hwnd)) {
            [void][FedoraWinExplorerRestartProbe]::ShowWindow($hwnd, 5)
        }
    }

    Remove-Item Env:FEDORAWIN_CAPTURE_VIEW -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_MODE -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_THEME -ErrorAction SilentlyContinue
}
