Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

Write-Host "Windows PowerShell $($PSVersionTable.PSVersion) / $([Threading.Thread]::CurrentThread.ApartmentState)"
if ($PSVersionTable.PSVersion.Major -ne 5) { throw 'This smoke test must exercise Windows PowerShell 5.1.' }
if ([Threading.Thread]::CurrentThread.ApartmentState -ne 'STA') { throw 'WPF smoke test requires STA.' }

$parseErrors = @()
Get-ChildItem -LiteralPath $root -Filter '*.ps1' -Recurse | ForEach-Object {
    $tokens = $null
    $errors = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile($_.FullName,[ref]$tokens,[ref]$errors)
    if ($errors) { $parseErrors += $errors }
}
if ($parseErrors.Count -gt 0) {
    $parseErrors | ForEach-Object { Write-Error $_.Message }
    throw 'PowerShell parser errors detected.'
}
Write-Host 'PowerShell parse: PASS'

Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase
Add-Type -AssemblyName System.Xaml

Get-ChildItem -LiteralPath (Join-Path $root 'ui') -Filter '*.xaml' | ForEach-Object {
    [xml]$xml = Get-Content -Raw -Encoding UTF8 -LiteralPath $_.FullName
    $reader = New-Object System.Xml.XmlNodeReader $xml
    $window = [System.Windows.Markup.XamlReader]::Load($reader)
    if ($null -eq $window) { throw "XAML returned null: $($_.Name)" }
    $window.Close()
    Write-Host "XAML load: PASS $($_.Name)"
}

$nativePath = Join-Path $root 'native\FedoraWinNativeUi.cs'
Add-Type -TypeDefinition (Get-Content -Raw -Encoding UTF8 -LiteralPath $nativePath) -ReferencedAssemblies @('System.dll')
foreach ($typeName in @('FedoraWinAppBar','FedoraWinDwmFrameManager','FedoraWinPowerMode','FedoraWinSession')) {
    if ($null -eq ($typeName -as [type])) { throw "Native type failed to load: $typeName" }
}
Write-Host 'Native C# compile/load: PASS'

Add-Type -AssemblyName System.Runtime.WindowsRuntime
[Windows.Devices.Radios.Radio,Windows.System.Devices,ContentType=WindowsRuntime] | Out-Null
[Windows.Devices.Radios.RadioAccessStatus,Windows.System.Devices,ContentType=WindowsRuntime] | Out-Null
if (-not (Get-Command Get-StartApps -ErrorAction SilentlyContinue)) { throw 'Get-StartApps is unavailable.' }
Write-Host 'WinRT radio types and Get-StartApps: PASS'
Write-Host 'WINDOWS SMOKE TESTS PASSED'
