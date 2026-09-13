param([string]$OutputDirectory = 'artifacts\package')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$output = if ([IO.Path]::IsPathRooted($OutputDirectory)) { [IO.Path]::GetFullPath($OutputDirectory) } else { [IO.Path]::GetFullPath((Join-Path $root $OutputDirectory)) }
New-Item -ItemType Directory -Force -Path $output | Out-Null
$exe = Join-Path $output 'FedoraWin.exe'
if (Test-Path -LiteralPath $exe) { Remove-Item -LiteralPath $exe -Force }
$launcherSource = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $root 'native\FedoraWinLauncher.cs')
Add-Type -TypeDefinition $launcherSource -OutputAssembly $exe -OutputType WindowsApplication -ReferencedAssemblies @('System.dll','System.Core.dll')
foreach ($file in @('FedoraWin.ps1','config.json','Restore-Windows.ps1')) { Copy-Item -LiteralPath (Join-Path $root $file) -Destination (Join-Path $output $file) -Force }
foreach ($directory in @('ui','shell','native')) {
    $destination = Join-Path $output $directory
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -Path (Join-Path $root ($directory + '\*')) -Destination $destination -Recurse -Force
}
if (-not (Test-Path -LiteralPath $exe)) { throw 'FedoraWin.exe was not produced.' }
Write-Host "FedoraWin executable package: $exe"
