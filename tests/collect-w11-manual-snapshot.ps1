<#
Read-only Windows 11 evidence snapshot for a HUMAN manual test session.
This does not launch FedoraWin, change Explorer/DWM/taskbar state, or pass gates.
Capture before, during, and after each manual procedure; preserve original recordings too.
#>
param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)]
    [ValidateSet('before', 'during', 'after-clean-exit', 'after-force-kill', 'after-explorer-restart')]
    [string]$Stage,
    [string]$OutputDirectory = 'artifacts/manual-w11',
    [string]$ExpectedExecutableSha256 = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$exe = (Resolve-Path -LiteralPath $Executable -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw 'The selected executable must be an existing file.'
}
$hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
if ($ExpectedExecutableSha256) {
    if ($ExpectedExecutableSha256 -notmatch '^[0-9a-fA-F]{64}$') {
        throw 'ExpectedExecutableSha256 must be a 64-character hexadecimal SHA-256.'
    }
    if ($hash -ne $ExpectedExecutableSha256.ToLowerInvariant()) {
        throw 'Executable SHA-256 does not match the expected CI release candidate.'
    }
}

$os = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop
$build = 0
if (-not [int]::TryParse([string]$os.BuildNumber, [ref]$build) -or
    $os.Caption -notmatch '\bWindows 11\b' -or $build -lt 22000) {
    throw "An unmodified Windows 11 host is required; observed $($os.Caption), build $($os.BuildNumber)."
}

# Bind observations to one interactive logon session and one Windows boot.
# A reboot or new logon can conceal failed forced-kill rollback.
$sessionId = [System.Diagnostics.Process]::GetCurrentProcess().SessionId
if ($sessionId -le 0) {
    throw 'Manual snapshots require an interactive Windows session (not session 0).'
}
$bootTimeUtcTicks = ([DateTime]$os.LastBootUpTime).ToUniversalTime().Ticks.ToString([System.Globalization.CultureInfo]::InvariantCulture)

# Bind taskbar HWNDs to Explorer PID AND process creation time. The enumeration
# is observational only; it never changes taskbar visibility or window styles.
if (-not ('FedoraWinManualTaskbarSnapshot' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class FedoraWinManualTaskbarSnapshot {
    public sealed class Taskbar {
        public long Hwnd;
        public uint OwnerPid;
        public string ClassName;
        public bool Visible;
    }

    private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr data);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int capacity);
    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hwnd);

    public static Taskbar[] Capture() {
        var rows = new List<Taskbar>();
        EnumWindows((hwnd, _) => {
            var name = new StringBuilder(128);
            if (GetClassNameW(hwnd, name, name.Capacity) <= 0) return true;
            string cls = name.ToString();
            if (cls != "Shell_TrayWnd" && cls != "Shell_SecondaryTrayWnd") return true;
            uint pid;
            if (GetWindowThreadProcessId(hwnd, out pid) == 0 || pid == 0) return true;
            rows.Add(new Taskbar {
                Hwnd = hwnd.ToInt64(), OwnerPid = pid,
                ClassName = cls, Visible = IsWindowVisible(hwnd)
            });
            return true;
        }, IntPtr.Zero);
        return rows.ToArray();
    }
}
'@
}

$explorers = @(Get-Process -Name explorer -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $sessionId })
if ($explorers.Count -eq 0) { throw 'Explorer is not running; manual snapshot is invalid.' }
$explorerRows = @()
$explorerLifetime = @{}
foreach ($process in $explorers) {
    # JSON numbers lose Int64 precision in JavaScript; preserve .NET ticks as decimal text.
    $ticks = $process.StartTime.ToUniversalTime().Ticks.ToString([System.Globalization.CultureInfo]::InvariantCulture)
    $explorerLifetime[[int]$process.Id] = $ticks
    $explorerRows += [ordered]@{
        pid = [int]$process.Id
        sessionId = [int]$process.SessionId
        startTimeUtcTicks = $ticks
    }
}

$bars = @([FedoraWinManualTaskbarSnapshot]::Capture())
if ($bars.Count -eq 0) { throw 'No Explorer taskbar HWND found; snapshot is invalid.' }
$taskbarRows = @()
foreach ($bar in $bars) {
    $ownerPid = [int]$bar.OwnerPid
    if (-not $explorerLifetime.ContainsKey($ownerPid)) {
        throw "Taskbar HWND $($bar.Hwnd) is not owned by an observed Explorer process."
    }
    $taskbarRows += [ordered]@{
        hwnd = $bar.Hwnd.ToString([System.Globalization.CultureInfo]::InvariantCulture)
        className = $bar.ClassName
        visible = $bar.Visible
        ownerPid = $ownerPid
        ownerStartTimeUtcTicks = $explorerLifetime[$ownerPid]
    }
}

# Observe every live process using this *exact* candidate image, including
# independent frame/taskbar guardians. A "force-kill" snapshot taken while
# FedoraWin is still running must never look like clean recovery evidence.
# Do not terminate processes here: a human controls the lifecycle test.
$binaryName = [IO.Path]::GetFileName($exe).Replace("'", "''")
$possibleProcesses = @(Get-CimInstance -ClassName Win32_Process -Filter "Name = '$binaryName'" -ErrorAction Stop)
$candidateProcesses = @()
foreach ($candidate in $possibleProcesses) {
    # Other logon sessions do not demonstrate a runtime on this desktop.
    if ([int]$candidate.SessionId -ne $sessionId) { continue }
    if ([string]::IsNullOrWhiteSpace([string]$candidate.ExecutablePath)) {
        throw "Cannot read executable path for matching-name PID $($candidate.ProcessId); process identity is ambiguous."
    }
    if (-not [string]::Equals(
        [IO.Path]::GetFullPath([string]$candidate.ExecutablePath),
        [IO.Path]::GetFullPath($exe),
        [StringComparison]::OrdinalIgnoreCase)) {
        continue
    }
    if ([string]::IsNullOrWhiteSpace([string]$candidate.CommandLine) -or -not $candidate.CreationDate) {
        throw "Cannot inspect command line and creation lifetime for candidate PID $($candidate.ProcessId)."
    }
    $role = if ([string]$candidate.CommandLine -match '(?i)(?:^|\s)--restore-(?:frames|taskbar)-after(?:\s|$)') {
        'guardian'
    } else {
        'runtime'
    }
    $candidateProcesses += [ordered]@{
        pid = [int]$candidate.ProcessId
        sessionId = [int]$candidate.SessionId
        startTimeUtcTicks = ([DateTime]$candidate.CreationDate).ToUniversalTime().Ticks.ToString([System.Globalization.CultureInfo]::InvariantCulture)
        role = $role
    }
}

$timestamp = [DateTime]::UtcNow
$record = [ordered]@{
    schemaVersion = 4
    evidenceType = 'read-only-environment-snapshot'
    stage = $Stage
    capturedAtUtc = $timestamp.ToString('o')
    windows11Build = "$($os.Caption) (OS Build $($os.BuildNumber))"
    sessionId = $sessionId
    bootTimeUtcTicks = $bootTimeUtcTicks
    executableSha256 = $hash
    executablePath = $exe
    explorerProcesses = $explorerRows
    taskbars = $taskbarRows
    candidateProcesses = $candidateProcesses
    manualGateStatus = 'NOT RUN'
    betaApproved = $false
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$filename = "w11-$Stage-$($timestamp.ToString('yyyyMMddTHHmmssfffZ')).json"
$path = Join-Path $OutputDirectory $filename
$record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $path -Encoding UTF8 -NoNewline
Write-Host "Read-only Windows 11 snapshot saved: $path"
Write-Host 'This snapshot is NOT a manual PASS or beta approval. Attach original video and human gate results separately.'
