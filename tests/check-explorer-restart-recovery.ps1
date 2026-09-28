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
    [void][FedoraWinExplorerRestartProbe]::GetWindowThreadProcessId($Hwnd, [ref]$ownerPid)
    return [int]$ownerPid
}

function Ensure-ExplorerRunning {
    $existing = @(Get-Process -Name explorer -ErrorAction SilentlyContinue)
    if ($existing.Count -eq 0) {
        Start-Process explorer.exe | Out-Null
    }
}

$baselineExplorer = @(Get-Process -Name explorer -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
if ($baselineExplorer.Count -eq 0) {
    throw 'Explorer is not running before the restart test.'
}
$baselineTaskbars = @(Get-Taskbars | Where-Object { [FedoraWinExplorerRestartProbe]::IsWindowVisible($_) })
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

try {
    $shell = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru

    $hidden = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($shell.HasExited) { throw "FedoraWin exited before baseline taskbar hide: $($shell.ExitCode)" }
        $visible = @($baselineTaskbars | Where-Object {
            [FedoraWinExplorerRestartProbe]::IsWindow($_) -and
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
    foreach ($pid in $baselineExplorer) {
        Stop-Process -Id $pid -Force -ErrorAction SilentlyContinue
    }

    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($shell.HasExited) { throw "FedoraWin exited when Explorer restarted: $($shell.ExitCode)" }

        if ($attempt -eq 30) {
            Ensure-ExplorerRunning
        }

        $explorerNow = @(Get-Process -Name explorer -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
        $newPids = @($explorerNow | Where-Object { $baselineExplorer -notcontains $_ })
        $taskbarsNow = @(Get-Taskbars)
        foreach ($hwnd in $taskbarsNow) {
            $handle = $hwnd.ToInt64()
            $ownerPid = Get-OwnerPid -Hwnd $hwnd
            if ($baselineHandles -notcontains $handle -and $newPids -contains $ownerPid) {
                $replacementHwnd = $hwnd
                $replacementExplorerPid = $ownerPid
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
        if (-not [FedoraWinExplorerRestartProbe]::IsWindow($replacementHwnd)) {
            throw 'Replacement Explorer taskbar disappeared before FedoraWin could adopt it.'
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

    if (-not (Get-Process -Id $replacementExplorerPid -ErrorAction SilentlyContinue)) {
        throw 'Replacement Explorer exited while FedoraWin remained active.'
    }
    Write-Host ("EXPLORER RESTART ADOPTED: new Explorer PID={0}; replacement taskbar HWND={1}; FedoraWin PID={2}" -f
        $replacementExplorerPid,
        $replacementHwnd.ToInt64(),
        $shell.Id)

    Stop-Process -Id $shell.Id -Force -ErrorAction Stop
    $shell.WaitForExit()

    $restored = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ([FedoraWinExplorerRestartProbe]::IsWindow($replacementHwnd) -and
            [FedoraWinExplorerRestartProbe]::IsWindowVisible($replacementHwnd)) {
            $restored = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $restored) {
        throw 'Replacement Explorer taskbar was not restored after force-killing FedoraWin.'
    }

    if (-not (Get-Process -Id $replacementExplorerPid -ErrorAction SilentlyContinue)) {
        throw 'FedoraWin recovery required or caused another Explorer restart.'
    }
    if ((Get-OwnerPid -Hwnd $replacementHwnd) -ne $replacementExplorerPid) {
        throw 'Replacement taskbar changed Explorer ownership during FedoraWin recovery.'
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
        if ((Test-Taskbar -Hwnd $hwnd) -and -not [FedoraWinExplorerRestartProbe]::IsWindowVisible($hwnd)) {
            [void][FedoraWinExplorerRestartProbe]::ShowWindow($hwnd, 5)
        }
    }

    Remove-Item Env:FEDORAWIN_CAPTURE_VIEW -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_MODE -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_THEME -ErrorAction SilentlyContinue
}
