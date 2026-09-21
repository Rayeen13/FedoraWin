# Assert that the independently spawned frame guardian survives Stop-Process -Force.
# Uses a real WinForms top-level HWND, not a mocked DWM call.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class FedoraWinFrameProbe {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowW(string className, string title);
    [DllImport("user32.dll")]
    public static extern int IsWindow(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern int IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("dwmapi.dll")]
    public static extern int DwmGetWindowAttribute(IntPtr hwnd, uint attribute, out int value, uint size);
}
'@

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exe = Join-Path $root 'src-tauri\target\release\fedorawin.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw 'Build the real fedorawin.exe before running frame recovery test.' }

$title = "FedoraWin frame restoration probe $PID"
# Report the real WinForms HWND instead of depending on a global title search.
$probeReady = Join-Path ([IO.Path]::GetTempPath()) "fedorawin-frame-probe-$PID-$([guid]::NewGuid().ToString('N')).ready"
$probeStdout = "$probeReady.stdout"
$probeStderr = "$probeReady.stderr"
$escapedReadyPath = $probeReady.Replace("'", "''")
$probeCode = @"
Add-Type -AssemblyName System.Windows.Forms
if (-not [System.Windows.Forms.SystemInformation]::UserInteractive) {
    throw 'The Windows runner has no interactive desktop for a real WinForms HWND.'
}
`$form = New-Object System.Windows.Forms.Form
`$form.Text = '$title'
`$form.Width = 700
`$form.Height = 440
`$form.StartPosition = 'CenterScreen'
`$form.Show()
[IO.File]::WriteAllText('$escapedReadyPath', [string]`$form.Handle.ToInt64())
[System.Windows.Forms.Application]::Run(`$form)
"@
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($probeCode))
$probe = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-STA','-EncodedCommand', $encoded) -RedirectStandardOutput $probeStdout -RedirectStandardError $probeStderr -PassThru
$shell = $null

function Get-ProbeDiagnostics {
    $exit = if ($probe.HasExited) { $probe.ExitCode } else { 'running' }
    $errors = if (Test-Path -LiteralPath $probeStderr) { (Get-Content -LiteralPath $probeStderr -Raw).Trim() } else { '' }
    $output = if (Test-Path -LiteralPath $probeStdout) { (Get-Content -LiteralPath $probeStdout -Raw).Trim() } else { '' }
    return "exit=$exit; stderr=$errors; stdout=$output"
}

function Read-DwmAttributes {
    param([IntPtr]$Hwnd)
    $attributes = [ordered]@{}
    foreach ($attribute in @(20,33,34,35,36)) {
        $value = 0
        $result = [FedoraWinFrameProbe]::DwmGetWindowAttribute($Hwnd, [uint32]$attribute, [ref]$value, 4)
        if ($result -eq 0) { $attributes[[string]$attribute] = $value }
    }
    return $attributes
}

try {
    $hwnd = [IntPtr]::Zero
    for ($attempt = 0; $attempt -lt 300; $attempt++) {
        if ($probe.HasExited) { throw "WinForms probe exited: $(Get-ProbeDiagnostics)" }
        if (Test-Path -LiteralPath $probeReady) {
            $reported = (Get-Content -LiteralPath $probeReady -Raw).Trim()
            $candidate = [IntPtr]::new([long]::Parse($reported))
            $ownerPid = [uint32]0
            [void][FedoraWinFrameProbe]::GetWindowThreadProcessId($candidate, [ref]$ownerPid)
            if ([FedoraWinFrameProbe]::IsWindow($candidate) -ne 0 -and
                [FedoraWinFrameProbe]::IsWindowVisible($candidate) -ne 0 -and
                $ownerPid -eq $probe.Id) {
                $hwnd = $candidate
                break
            }
        }
        Start-Sleep -Milliseconds 100
    }
    if ($hwnd -eq [IntPtr]::Zero) {
        throw "Real WinForms probe HWND is not visible in the runner window station: $(Get-ProbeDiagnostics)"
    }
    $original = Read-DwmAttributes $hwnd
    if ($original.Count -lt 2) { throw 'DWM did not expose enough native attributes for a recovery test.' }

    $env:FEDORAWIN_KEEP_WINDOWS_TASKBAR = '1'
    $env:FEDORAWIN_CAPTURE_VIEW = 'panel'
    $env:FEDORAWIN_CAPTURE_THEME = 'dark'
    $shell = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru

    $changed = @()
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        if ($shell.HasExited) { throw "FedoraWin exited before styling: $($shell.ExitCode)" }
        $current = Read-DwmAttributes $hwnd
        $changed = @($original.Keys | Where-Object {
            $current.Contains($_) -and $current[$_] -ne $original[$_]
        })
        if ($changed.Count -gt 0) { break }
        Start-Sleep -Milliseconds 100
    }
    if ($changed.Count -eq 0) { throw 'Native frame was never styled: cannot validate recovery.' }
    Write-Host "FRAME STYLED: DWM attributes $($changed -join ',') changed on a real HWND."

    Stop-Process -Id $shell.Id -Force -ErrorAction Stop
    $shell.WaitForExit()
    $restored = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        $current = Read-DwmAttributes $hwnd
        $pending = @($changed | Where-Object {
            -not $current.Contains($_) -or $current[$_] -ne $original[$_]
        })
        if ($pending.Count -eq 0) { $restored = $true; break }
        Start-Sleep -Milliseconds 100
    }
    if (-not $restored) {
        throw "FRAME RECOVERY FAILED: original DWM attributes were not restored: $($pending -join ',')"
    }
    Write-Host "FRAME RECOVERY PASSED: independently restored $($changed.Count) changed DWM attributes after force-killing FedoraWin.exe."
} finally {
    if ($shell -and -not $shell.HasExited) { Stop-Process -Id $shell.Id -Force -ErrorAction SilentlyContinue }
    if ($probe -and -not $probe.HasExited) { Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:FEDORAWIN_KEEP_WINDOWS_TASKBAR -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_VIEW -ErrorAction SilentlyContinue
    Remove-Item Env:FEDORAWIN_CAPTURE_THEME -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $probeReady, $probeStdout, $probeStderr -ErrorAction SilentlyContinue
}
