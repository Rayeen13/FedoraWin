param(
    [string]$Executable = 'src-tauri\target\release\fedorawin.exe',
    [int]$TargetIdleMb = 100,
    [int]$HardLimitMb = 300
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exe = if ([IO.Path]::IsPathRooted($Executable)) {
    $Executable
} else {
    Join-Path $repoRoot $Executable
}

if (-not (Test-Path -LiteralPath $exe)) {
    throw "FedoraWin executable not found at $exe"
}

function Get-ProcessTreeIds {
    param([Parameter(Mandatory)][int]$RootProcessId)

    $rows = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootProcessId)

    do {
        $before = $ids.Count
        foreach ($row in $rows) {
            if ($ids.Contains([int]$row.ParentProcessId)) {
                [void]$ids.Add([int]$row.ProcessId)
            }
        }
    } while ($ids.Count -ne $before)

    return @($ids)
}

function Get-TreeWorkingSetMb {
    param([Parameter(Mandatory)][int]$RootProcessId)

    $ids = @(Get-ProcessTreeIds -RootProcessId $RootProcessId)
    [int64]$bytes = 0
    [int]$alive = 0
    foreach ($id in $ids) {
        try {
            $process = Get-Process -Id $id -ErrorAction Stop
            $bytes += [int64]$process.WorkingSet64
            $alive++
        } catch {
            # A short-lived WebView2 utility process may disappear between snapshots.
        }
    }

    [pscustomobject]@{
        Megabytes = [Math]::Ceiling($bytes / 1MB)
        ProcessCount = $alive
        ProcessIds = $ids
    }
}

$env:FEDORAWIN_CAPTURE_VIEW = 'panel'
$env:FEDORAWIN_CAPTURE_MODE = ''
$env:FEDORAWIN_CAPTURE_THEME = 'dark'
$process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru

try {
    Start-Sleep -Seconds 3

    $samples = @()
    1..6 | ForEach-Object {
        if ($process.HasExited) {
            throw "FedoraWin exited during the memory smoke test with code $($process.ExitCode)."
        }
        $sample = Get-TreeWorkingSetMb -RootProcessId $process.Id
        $samples += $sample
        Write-Host ("MEMORY sample {0}: {1} MB across {2} processes" -f $_, $sample.Megabytes, $sample.ProcessCount)
        if ($_ -eq 1) {
            foreach ($id in $sample.ProcessIds) {
                try {
                    $item = Get-Process -Id $id -ErrorAction Stop
                    Write-Host ("MEMORY process: {0} pid={1} workingSet={2} MB" -f $item.ProcessName, $id, [Math]::Ceiling($item.WorkingSet64 / 1MB))
                } catch {}
            }
        }
        Start-Sleep -Milliseconds 350
    }

    $peak = ($samples | Measure-Object -Property Megabytes -Maximum).Maximum
    $last = $samples[-1].Megabytes
    $processCount = ($samples | Measure-Object -Property ProcessCount -Maximum).Maximum

    Write-Host "FedoraWin idle working set: $last MB"
    Write-Host "FedoraWin sampled peak: $peak MB"
    Write-Host "Target idle budget: ~$TargetIdleMb MB"
    Write-Host "Hard memory ceiling: $HardLimitMb MB"

    [ordered]@{
        idle_working_set_mb = $last
        sampled_peak_mb = $peak
        process_count_peak = $processCount
        target_idle_mb = $TargetIdleMb
        hard_limit_mb = $HardLimitMb
        within_hard_limit = ($peak -le $HardLimitMb)
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $repoRoot 'memory-budget.json') -Encoding UTF8

    if ($peak -gt $HardLimitMb) {
        throw "FedoraWin exceeded the $HardLimitMb MB hard memory ceiling (sampled peak: $peak MB)."
    }

    if ($last -gt ($TargetIdleMb + 50)) {
        Write-Warning "FedoraWin is above the ~$TargetIdleMb MB idle target ($last MB). The build is under the hard ceiling but needs further trimming."
    }
} finally {
    $treeIds = if ($process) { @(Get-ProcessTreeIds -RootProcessId $process.Id) } else { @() }
    foreach ($id in ($treeIds | Sort-Object -Descending)) {
        Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
    }
    Remove-Item Env:FEDORAWIN_CAPTURE_VIEW -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_MODE -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_THEME -ErrorAction SilentlyContinue
}
