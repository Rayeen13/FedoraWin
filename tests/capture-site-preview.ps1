param([string]$OutputDirectory = 'artifacts\site-preview')

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class FedoraWinCaptureNative {
    [DllImport("user32.dll", CharSet=CharSet.Auto)]
    public static extern IntPtr FindWindow(string className, string windowName);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);
}
'@

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$packageRoot = Join-Path $repoRoot 'artifacts\package'
$fedoraWinExe = Join-Path $packageRoot 'FedoraWin.exe'
if (-not (Test-Path -LiteralPath $fedoraWinExe)) { throw 'FedoraWin.exe package is missing. Run tests\build-exe.ps1 first.' }

$output = if ([IO.Path]::IsPathRooted($OutputDirectory)) { [IO.Path]::GetFullPath($OutputDirectory) } else { [IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory)) }
New-Item -ItemType Directory -Path $output -Force | Out-Null

$stdoutPath = Join-Path $output 'fedora-win.stdout.log'
$stderrPath = Join-Path $output 'fedora-win.stderr.log'
$sourceSha = (& git -C $repoRoot rev-parse HEAD).Trim()
$sourceBranch = (& git -C $repoRoot rev-parse --abbrev-ref HEAD).Trim()
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds

function Wait-ShellProcessId {
    param([Parameter(Mandatory)][int]$LauncherProcessId,[int]$TimeoutSeconds=15)
    $deadline=[DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $child=Get-CimInstance Win32_Process -Filter "ParentProcessId = $LauncherProcessId" -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -ieq 'powershell.exe' } | Select-Object -First 1
        if($child){return [int]$child.ProcessId}
        Start-Sleep -Milliseconds 200
    } while([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for FedoraWin.exe shell child process."
}

function Wait-AutomationElement {
    param([Parameter(Mandatory)][int]$ProcessId,[Parameter(Mandatory)][string]$AutomationId,[int]$TimeoutSeconds=15)
    $deadline=[DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $root=[System.Windows.Automation.AutomationElement]::RootElement
    $pidCondition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$ProcessId)
    $idCondition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty,$AutomationId)
    $condition=New-Object System.Windows.Automation.AndCondition($pidCondition,$idCondition)
    do {
        $element=$root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$condition)
        if($null -ne $element){return $element}
        Start-Sleep -Milliseconds 250
    } while([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for automation element '$AutomationId' in FedoraWin shell process $ProcessId."
}

function Invoke-AutomationButton {
    param([Parameter(Mandatory)][int]$ProcessId,[Parameter(Mandatory)][string]$AutomationId,[int]$TimeoutSeconds=15)
    $element=Wait-AutomationElement -ProcessId $ProcessId -AutomationId $AutomationId -TimeoutSeconds $TimeoutSeconds
    $pattern=$null
    if(-not $element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){throw "Automation element '$AutomationId' does not support InvokePattern."}
    ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
}

function Assert-WindowsTaskbarHidden {
    $taskbar=[FedoraWinCaptureNative]::FindWindow('Shell_TrayWnd',$null)
    if($taskbar -ne [IntPtr]::Zero -and [FedoraWinCaptureNative]::IsWindowVisible($taskbar)){
        throw 'Windows taskbar is visible while FedoraWin.exe is under visual validation.'
    }
}

function Wait-FedoraWinTheme {
    param([Parameter(Mandatory)][string]$Expected,[int]$TimeoutSeconds=8)
    $settings=Join-Path $env:LOCALAPPDATA 'FedoraWin\settings.json'
    $deadline=[DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        try {
            if(Test-Path -LiteralPath $settings){
                $saved=Get-Content -Raw -Encoding UTF8 -LiteralPath $settings | ConvertFrom-Json
                if([string]$saved.theme -eq $Expected){return}
            }
        } catch {}
        Start-Sleep -Milliseconds 200
    } while([DateTime]::UtcNow -lt $deadline)
    throw "FedoraWin theme did not persist as '$Expected' after the Quick Settings action."
}

function Copy-FedoraWinRuntimeLog {
    Copy-FedoraWinRuntimeLog
}

function Save-DesktopCapture {
    param([Parameter(Mandatory)][string]$FileName)
    Assert-WindowsTaskbarHidden
    $path=Join-Path $output $FileName
    $bitmap=New-Object System.Drawing.Bitmap($screen.Width,$screen.Height,[System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics=[System.Drawing.Graphics]::FromImage($bitmap)
    try{$graphics.CopyFromScreen($screen.Left,$screen.Top,0,0,$screen.Size,[System.Drawing.CopyPixelOperation]::SourceCopy); $bitmap.Save($path,[System.Drawing.Imaging.ImageFormat]::Png)}
    finally{$graphics.Dispose();$bitmap.Dispose()}
    $file=Get-Item -LiteralPath $path
    if($file.Length -lt 4096){throw "Capture '$FileName' is unexpectedly small ($($file.Length) bytes)."}
    return $path
}

$startArgs=@{
    FilePath=$fedoraWinExe
    ArgumentList=@('-Diagnostic')
    WorkingDirectory=$packageRoot
    RedirectStandardOutput=$stdoutPath
    RedirectStandardError=$stderrPath
    PassThru=$true
}
$launcher=Start-Process @startArgs
$shellPid=$null
$captures=[ordered]@{}
$captureError=$null

try {
    $shellPid=Wait-ShellProcessId -LauncherProcessId $launcher.Id -TimeoutSeconds 15
    [void](Wait-AutomationElement -ProcessId $shellPid -AutomationId 'ActivitiesButton' -TimeoutSeconds 20)
    Start-Sleep -Milliseconds 1000
    Assert-WindowsTaskbarHidden
    $captures.panel=Split-Path -Leaf (Save-DesktopCapture -FileName 'panel-desktop.png')

    Invoke-AutomationButton -ProcessId $shellPid -AutomationId 'ActivitiesButton'
    [void](Wait-AutomationElement -ProcessId $shellPid -AutomationId 'ShowAppsButton' -TimeoutSeconds 15)
    Start-Sleep -Milliseconds 900
    $captures.activities=Split-Path -Leaf (Save-DesktopCapture -FileName 'activities-overview.png')

    Invoke-AutomationButton -ProcessId $shellPid -AutomationId 'ShowAppsButton'
    Start-Sleep -Milliseconds 900
    $captures.apps=Split-Path -Leaf (Save-DesktopCapture -FileName 'applications-grid.png')

    Invoke-AutomationButton -ProcessId $shellPid -AutomationId 'ActivitiesButton'
    Start-Sleep -Milliseconds 450
    Invoke-AutomationButton -ProcessId $shellPid -AutomationId 'QuickButton'
    [void](Wait-AutomationElement -ProcessId $shellPid -AutomationId 'DarkStyleButton' -TimeoutSeconds 15)
    Start-Sleep -Milliseconds 800
    $captures.quick_settings_dark=Split-Path -Leaf (Save-DesktopCapture -FileName 'quick-settings-dark.png')

    Invoke-AutomationButton -ProcessId $shellPid -AutomationId 'DarkStyleButton'
    Wait-FedoraWinTheme -Expected 'light'
    Start-Sleep -Milliseconds 700
    $captures.quick_settings_light=Split-Path -Leaf (Save-DesktopCapture -FileName 'quick-settings-light.png')

    $hashes=[ordered]@{}
    foreach($entry in $captures.GetEnumerator()){$hashes[$entry.Key]=(Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $output $entry.Value)).Hash.ToLowerInvariant()}
    if(($hashes.Values | Select-Object -Unique).Count -lt 4){throw 'CI UI captures are not sufficiently distinct; the desktop session may not be rendering FedoraWin correctly.'}
    if($hashes.quick_settings_dark -eq $hashes.quick_settings_light){throw 'Quick Settings dark/light captures are identical; theme switching is not visually working.'}

    Copy-FedoraWinRuntimeLog
    [ordered]@{
        source_branch=$sourceBranch
        source_sha=$sourceSha
        captured_utc=[DateTime]::UtcNow.ToString('o')
        runner_os=$env:RUNNER_OS
        runner_name=$env:RUNNER_NAME
        runner_image=$env:ImageOS
        github_run_id=$env:GITHUB_RUN_ID
        github_run_number=$env:GITHUB_RUN_NUMBER
        launch_executable='FedoraWin.exe'
        launcher_pid=$launcher.Id
        shell_pid=$shellPid
        windows_taskbar_hidden=$true
        screen=[ordered]@{width=$screen.Width;height=$screen.Height}
        screenshots=$captures
        sha256=$hashes
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'preview-metadata.json') -Encoding UTF8
} catch {
    $captureError=$_
    try {
        $runtimeLog=Join-Path $env:LOCALAPPDATA 'FedoraWin\FedoraWin.log'
        if(Test-Path -LiteralPath $runtimeLog){Copy-Item -LiteralPath $runtimeLog -Destination (Join-Path $output 'fedora-win.runtime.log') -Force}
    } catch {}
} finally {
    try {
        $runtime=Join-Path $env:LOCALAPPDATA 'FedoraWin'
        New-Item -ItemType Directory -Force -Path $runtime | Out-Null
        Set-Content -LiteralPath (Join-Path $runtime 'restore.request') -Value 'ci-exit' -Encoding ASCII
    } catch {}
    if($launcher -and -not $launcher.HasExited){try{[void]$launcher.WaitForExit(6000)}catch{}}
    if($shellPid){
        $shell=Get-Process -Id $shellPid -ErrorAction SilentlyContinue
        if($shell){try{Stop-Process -Id $shellPid -Force -ErrorAction SilentlyContinue}catch{}}
    }
    if($launcher -and -not $launcher.HasExited){try{Stop-Process -Id $launcher.Id -Force -ErrorAction SilentlyContinue}catch{}}
    try{& powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File (Join-Path $packageRoot 'Restore-Windows.ps1') | Out-Null}catch{}
}

if($null -ne $captureError){throw $captureError}
Write-Host "FedoraWin.exe visual evidence captured to $output"
