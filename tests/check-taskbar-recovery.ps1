# Verify FedoraWin's independent desktop-presentation guardian restores Explorer
# taskbars after an abrupt FedoraWin termination. This uses the real Windows
# Explorer HWNDs on the CI desktop; it never terminates or restarts Explorer.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class FedoraWinTaskbarRecoveryProbe {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr data);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowW(string className, string windowName);

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
    throw 'Build the real fedorawin.exe before running taskbar recovery test.'
}

function Get-ClassName {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    $buffer = [Text.StringBuilder]::new(128)
    $length = [FedoraWinTaskbarRecoveryProbe]::GetClassNameW($Hwnd, $buffer, $buffer.Capacity)
    if ($length -le 0) { return '' }
    return $buffer.ToString()
}

function Test-ExplorerTaskbar {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    if ($Hwnd -eq [IntPtr]::Zero -or -not [FedoraWinTaskbarRecoveryProbe]::IsWindow($Hwnd)) {
        return $false
    }
    $class = Get-ClassName -Hwnd $Hwnd
    return $class -eq 'Shell_TrayWnd' -or $class -eq 'Shell_SecondaryTrayWnd'
}

function Get-VisibleTaskbars {
    $handles = [System.Collections.Generic.List[IntPtr]]::new()
    $primary = [FedoraWinTaskbarRecoveryProbe]::FindWindowW('Shell_TrayWnd', $null)
    if ((Test-ExplorerTaskbar -Hwnd $primary) -and [FedoraWinTaskbarRecoveryProbe]::IsWindowVisible($primary)) {
        $handles.Add($primary)
    }

    $callback = [FedoraWinTaskbarRecoveryProbe+EnumWindowsProc]{
        param([IntPtr]$hwnd, [IntPtr]$data)
        if ((Test-ExplorerTaskbar -Hwnd $hwnd) -and
            [FedoraWinTaskbarRecoveryProbe]::IsWindowVisible($hwnd) -and
            -not $handles.Contains($hwnd)) {
            $handles.Add($hwnd)
        }
        return $true
    }
    [void][FedoraWinTaskbarRecoveryProbe]::EnumWindows($callback, [IntPtr]::Zero)
    return @($handles)
}

function Get-TaskbarOwnerPid {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    [uint32]$ownerPid = 0
    $threadId = [FedoraWinTaskbarRecoveryProbe]::GetWindowThreadProcessId($Hwnd, [ref]$ownerPid)
    if ($threadId -eq 0) { return 0 }
    return [int]$ownerPid
}

# Pin both the original Explorer PID and its creation time. A reused PID or a
# recycled HWND must never authorize emergency ShowWindow against another app.
$explorerProcesses = @(Get-Process -Name explorer -ErrorAction SilentlyContinue)
$explorerBefore = @($explorerProcesses | Select-Object -ExpandProperty Id)
$script:explorerStartTicks = @{}
foreach ($process in $explorerProcesses) {
    # Failure to read process identity is a hard test failure before hiding anything.
    $script:explorerStartTicks[[int]$process.Id] = $process.StartTime.ToUniversalTime().Ticks
}

function Test-BaselineExplorerTaskbar {
    param([Parameter(Mandatory)][IntPtr]$Hwnd)
    if (-not (Test-ExplorerTaskbar -Hwnd $Hwnd)) { return $false }

    $ownerPid = Get-TaskbarOwnerPid -Hwnd $Hwnd
    if ($ownerPid -eq 0 -or -not $script:explorerStartTicks.ContainsKey([int]$ownerPid)) {
        return $false
    }
    $owner = Get-Process -Id $ownerPid -ErrorAction SilentlyContinue
    if ($null -eq $owner -or $owner.ProcessName -ne 'explorer') { return $false }
    try {
        return $owner.StartTime.ToUniversalTime().Ticks -eq $script:explorerStartTicks[[int]$ownerPid]
    } catch {
        return $false
    }
}

if ($explorerBefore.Count -eq 0) {
    throw 'Explorer is not running; taskbar recovery cannot be validated.'
}

$taskbars = @(Get-VisibleTaskbars)
if ($taskbars.Count -eq 0) {
    throw 'No visible Explorer taskbar HWND is available for the recovery test.'
}

foreach ($hwnd in $taskbars) {
    if (-not (Test-BaselineExplorerTaskbar -Hwnd $hwnd)) {
        throw "Taskbar HWND $($hwnd.ToInt64()) is not owned by the original Explorer process lifetime."
    }
}
Write-Host ("TASKBAR BASELINE: Explorer PID(s)={0}; HWND(s)={1}" -f
    ($explorerBefore -join ','),
    (($taskbars | ForEach-Object { $_.ToInt64() }) -join ','))

$env:FEDORAWIN_CAPTURE_VIEW = 'panel'
$env:FEDORAWIN_CAPTURE_MODE = ''
$env:FEDORAWIN_CAPTURE_THEME = 'dark'
Remove-Item Env:FEDORAWIN_KEEP_WINDOWS_TASKBAR -ErrorAction SilentlyContinue
$shell = $null
$restored = $false

try {
    $shell = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru

    $hidden = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($shell.HasExited) {
            throw "FedoraWin exited before hiding Explorer taskbars: $($shell.ExitCode)"
        }
        foreach ($hwnd in $taskbars) {
            if (-not (Test-BaselineExplorerTaskbar -Hwnd $hwnd)) {
                throw "Baseline Explorer HWND $($hwnd.ToInt64()) changed identity before taskbar hide."
            }
        }
        $stillVisible = @($taskbars | Where-Object {
            [FedoraWinTaskbarRecoveryProbe]::IsWindowVisible($_)
        })
        if ($stillVisible.Count -eq 0) {
            $hidden = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $hidden) {
        throw 'FedoraWin did not hide all baseline Explorer taskbars.'
    }

    $explorerWhileHidden = @(Get-Process -Name explorer -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    $preservedWhileHidden = @($explorerBefore | Where-Object { $explorerWhileHidden -contains $_ })
    if ($preservedWhileHidden.Count -eq 0) {
        throw 'FedoraWin hid the taskbar but did not preserve the baseline Explorer process.'
    }
    Write-Host ("TASKBAR HIDDEN: Explorer preserved PID(s)={0}" -f ($preservedWhileHidden -join ','))

    Stop-Process -Id $shell.Id -Force -ErrorAction Stop
    $shell.WaitForExit()

    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        $pending = @($taskbars | Where-Object {
            -not (Test-BaselineExplorerTaskbar -Hwnd $_) -or
            -not [FedoraWinTaskbarRecoveryProbe]::IsWindowVisible($_)
        })
        if ($pending.Count -eq 0) {
            $restored = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $restored) {
        throw "TASKBAR RECOVERY FAILED: baseline Explorer taskbar HWND(s) remained hidden or disappeared: $(
            ($pending | ForEach-Object { $_.ToInt64() }) -join ','
        )"
    }

    $explorerAfter = @(Get-Process -Name explorer -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    $preservedAfter = @($explorerBefore | Where-Object { $explorerAfter -contains $_ })
    if ($preservedAfter.Count -eq 0) {
        throw 'Explorer was not preserved after forced FedoraWin termination.'
    }

    foreach ($hwnd in $taskbars) {
        $ownerPid = Get-TaskbarOwnerPid -Hwnd $hwnd
        if ($preservedAfter -notcontains [int]$ownerPid -or
            -not (Test-BaselineExplorerTaskbar -Hwnd $hwnd)) {
            throw "Restored taskbar HWND $($hwnd.ToInt64()) changed Explorer ownership or lifetime."
        }
    }

    Write-Host ("TASKBAR RECOVERY PASSED: force-kill restored {0} real Explorer taskbar HWND(s); Explorer PID(s) {1} remained intact." -f
        $taskbars.Count,
        ($preservedAfter -join ','))
} finally {
    if ($shell -and -not $shell.HasExited) {
        Stop-Process -Id $shell.Id -Force -ErrorAction SilentlyContinue
    }

    # Never show a recycled HWND or a taskbar-like window owned by another
    # process. Emergency cleanup may touch only the original Explorer lifetime.
    foreach ($hwnd in $taskbars) {
        if ((Test-BaselineExplorerTaskbar -Hwnd $hwnd) -and
            -not [FedoraWinTaskbarRecoveryProbe]::IsWindowVisible($hwnd)) {
            [void][FedoraWinTaskbarRecoveryProbe]::ShowWindow($hwnd, 5)
        }
    }

    Remove-Item Env:FEDORAWIN_CAPTURE_VIEW -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_MODE -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_THEME -ErrorAction SilentlyContinue
}
