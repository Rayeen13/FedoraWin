param(
    [string]$OutputDirectory = 'artifacts\site-preview'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$output = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory))
}
New-Item -ItemType Directory -Path $output -Force | Out-Null

$stdoutPath = Join-Path $output 'fedora-win.stdout.log'
$stderrPath = Join-Path $output 'fedora-win.stderr.log'
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$sourceBranch = (& git -C $repoRoot rev-parse --abbrev-ref HEAD).Trim()
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds

function Wait-AutomationElement {
    param(
        [Parameter(Mandatory)][int]$ProcessId,
        [Parameter(Mandatory)][string]$AutomationId,
        [int]$TimeoutSeconds = 15
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $pidCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $ProcessId
    )
    $idCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
        $AutomationId
    )
    $condition = New-Object System.Windows.Automation.AndCondition($pidCondition, $idCondition)

    do {
        $element = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if ($null -ne $element) { return $element }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)

    throw "Timed out waiting for automation element '$AutomationId' in process $ProcessId."
}

function Invoke-AutomationButton {
    param(
        [Parameter(Mandatory)][int]$ProcessId,
        [Parameter(Mandatory)][string]$AutomationId,
        [int]$TimeoutSeconds = 15
    )

    $element = Wait-AutomationElement -ProcessId $ProcessId -AutomationId $AutomationId -TimeoutSeconds $TimeoutSeconds
    $pattern = $null
    if (-not $element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        throw "Automation element '$AutomationId' does not support InvokePattern."
    }
    ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
}

function Save-DesktopCapture {
    param([Parameter(Mandatory)][string]$FileName)

    $path = Join-Path $output $FileName
    $bitmap = New-Object System.Drawing.Bitmap($screen.Width, $screen.Height, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $screen.Size, [System.Drawing.CopyPixelOperation]::SourceCopy)
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }

    $file = Get-Item -LiteralPath $path
    if ($file.Length -lt 4096) {
        throw "Capture '$FileName' is unexpectedly small ($($file.Length) bytes)."
    }
    return $path
}

$powerShellExe = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$arguments = @(
    '-NoLogo',
    '-NoProfile',
    '-ExecutionPolicy', 'Bypass',
    '-STA',
    '-File', (Join-Path $repoRoot 'FedoraWin.ps1'),
    '-SafeMode',
    '-Diagnostic'
)
$startArgs = @{
    FilePath = $powerShellExe
    ArgumentList = $arguments
    WorkingDirectory = $repoRoot
    RedirectStandardOutput = $stdoutPath
    RedirectStandardError = $stderrPath
    PassThru = $true
}

$process = Start-Process @startArgs
$captures = [ordered]@{}
$captureError = $null

try {
    [void](Wait-AutomationElement -ProcessId $process.Id -AutomationId 'ActivitiesButton' -TimeoutSeconds 20)
    Start-Sleep -Milliseconds 750

    $captures.panel = Split-Path -Leaf (Save-DesktopCapture -FileName 'panel-desktop.png')

    Invoke-AutomationButton -ProcessId $process.Id -AutomationId 'ActivitiesButton'
    [void](Wait-AutomationElement -ProcessId $process.Id -AutomationId 'ShowAppsButton' -TimeoutSeconds 15)
    Start-Sleep -Milliseconds 1000
    $captures.activities = Split-Path -Leaf (Save-DesktopCapture -FileName 'activities-overview.png')

    Invoke-AutomationButton -ProcessId $process.Id -AutomationId 'ShowAppsButton'
    Start-Sleep -Milliseconds 1000
    $captures.apps = Split-Path -Leaf (Save-DesktopCapture -FileName 'applications-grid.png')

    Invoke-AutomationButton -ProcessId $process.Id -AutomationId 'ActivitiesButton'
    Start-Sleep -Milliseconds 500
    Invoke-AutomationButton -ProcessId $process.Id -AutomationId 'QuickButton'
    [void](Wait-AutomationElement -ProcessId $process.Id -AutomationId 'DarkStyleButton' -TimeoutSeconds 15)
    Start-Sleep -Milliseconds 1000
    $captures.quick_settings_dark = Split-Path -Leaf (Save-DesktopCapture -FileName 'quick-settings-dark.png')

    Invoke-AutomationButton -ProcessId $process.Id -AutomationId 'DarkStyleButton'
    Start-Sleep -Milliseconds 750
    $captures.quick_settings_light = Split-Path -Leaf (Save-DesktopCapture -FileName 'quick-settings-light.png')

    $hashes = [ordered]@{}
    foreach ($entry in $captures.GetEnumerator()) {
        $hashes[$entry.Key] = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $output $entry.Value)).Hash.ToLowerInvariant()
    }
    if (($hashes.Values | Select-Object -Unique).Count -lt 4) {
        throw 'CI UI captures are not sufficiently distinct; the desktop session may not be rendering FedoraWin correctly.'
    }

    $metadata = [ordered]@{
        source_branch = $sourceBranch
        source_sha = $sourceSha
        captured_utc = [DateTime]::UtcNow.ToString('o')
        runner_os = $env:RUNNER_OS
        runner_name = $env:RUNNER_NAME
        runner_image = $env:ImageOS
        github_run_id = $env:GITHUB_RUN_ID
        github_run_number = $env:GITHUB_RUN_NUMBER
        screen = [ordered]@{ width = $screen.Width; height = $screen.Height }
        screenshots = $captures
        sha256 = $hashes
    }
    $metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'preview-metadata.json') -Encoding UTF8
} catch {
    $captureError = $_
} finally {
    if ($null -ne $process -and -not $process.HasExited) {
        try {
            [System.Windows.Forms.SendKeys]::SendWait('%{F4}')
            [void]$process.WaitForExit(2500)
        } catch { }

        if (-not $process.HasExited) {
            try { [void]$process.CloseMainWindow(); [void]$process.WaitForExit(1500) } catch { }
        }
        if (-not $process.HasExited) {
            try { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue } catch { }
            try { [void]$process.WaitForExit(1500) } catch { }
        }
    }

    try {
        & $powerShellExe -NoLogo -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'Restore-Windows.ps1') | Out-Null
    } catch { }
}

if ($null -ne $captureError) {
    throw $captureError
}

Write-Host "FedoraWin site preview captured to $output"
