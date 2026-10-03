param(
    [string]$OutputDirectory = 'native\\adwaita\\out\\preferences-runtime'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\\..')).Path
$outRoot = (Resolve-Path (Join-Path $PSScriptRoot 'out')).Path
$sourceExe = Join-Path $outRoot 'fedorawin-preferences.exe'
if (-not (Test-Path -LiteralPath $sourceExe)) {
    throw "Preferences executable is missing: $sourceExe"
}
if (-not $env:MSYS2_ROOT) {
    throw 'MSYS2_ROOT must come from setup-msys2 output.'
}

$ucrtRoot = Join-Path $env:MSYS2_ROOT 'ucrt64'
$ucrtBin = Join-Path $ucrtRoot 'bin'
$ntldd = Join-Path $ucrtBin 'ntldd.exe'
if (-not (Test-Path -LiteralPath $ntldd)) {
    throw "ntldd is unavailable: $ntldd"
}

$runtime = if ([IO.Path]::IsPathRooted($OutputDirectory)) {
    [IO.Path]::GetFullPath($OutputDirectory)
} else {
    [IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory))
}
if (Test-Path -LiteralPath $runtime) {
    Remove-Item -LiteralPath $runtime -Recurse -Force
}
New-Item -ItemType Directory -Path $runtime -Force | Out-Null
Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $runtime 'fedorawin-preferences.exe') -Force

function Convert-MsysDependencyPath {
    param([Parameter(Mandatory)][string]$Path)

    $candidate = $Path.Trim().Trim('"')
    if ($candidate -match '^/ucrt64/(.+)$') {
        return Join-Path $ucrtRoot ($Matches[1] -replace '/', '\\')
    }
    if ($candidate -match '^/([A-Za-z])/(.+)$') {
        return ('{0}:\\{1}' -f $Matches[1].ToUpperInvariant(), ($Matches[2] -replace '/', '\\'))
    }
    return $candidate
}

$dependencyOutput = @(& $ntldd -R $sourceExe 2>&1)
if ($LASTEXITCODE -ne 0) {
    throw "ntldd failed for Preferences with exit code $LASTEXITCODE. Output: $($dependencyOutput -join ' | ')"
}

$systemRoot = [IO.Path]::GetFullPath($env:SystemRoot).TrimEnd('\\')
$ucrtRootFull = [IO.Path]::GetFullPath($ucrtRoot).TrimEnd('\\')
$runtimeDlls = [ordered]@{}
$missing = [System.Collections.Generic.List[string]]::new()

foreach ($line in $dependencyOutput) {
    $text = [string]$line
    if ($text -match '<MODULE MISSING>|=>\s+not found') {
        $missing.Add($text.Trim())
        continue
    }
    if ($text -notmatch '=>\s+(.+?\.dll)(?:\s+\(0x[0-9A-Fa-f]+\))?\s*$') {
        continue
    }

    $resolved = Convert-MsysDependencyPath -Path $Matches[1]
    if (-not [IO.Path]::IsPathRooted($resolved)) {
        continue
    }
    $full = [IO.Path]::GetFullPath($resolved)
    if ($full.StartsWith($systemRoot, [StringComparison]::OrdinalIgnoreCase)) {
        continue
    }
    if (-not $full.StartsWith($ucrtRootFull, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Preferences depends on an unexpected non-system runtime outside UCRT64: $full"
    }
    if (-not (Test-Path -LiteralPath $full)) {
        $missing.Add($full)
        continue
    }
    $name = [IO.Path]::GetFileName($full)
    $runtimeDlls[$name.ToLowerInvariant()] = $full
}

if ($missing.Count) {
    throw "Portable Preferences dependency closure is incomplete: $($missing -join ' | ')"
}
if (-not $runtimeDlls.Contains('libadwaita-1-0.dll')) {
    throw 'Portable dependency scan did not include libadwaita-1-0.dll.'
}
if (-not $runtimeDlls.Contains('libgtk-4-1.dll')) {
    throw 'Portable dependency scan did not include libgtk-4-1.dll.'
}

foreach ($entry in $runtimeDlls.GetEnumerator()) {
    Copy-Item -LiteralPath $entry.Value -Destination (Join-Path $runtime ([IO.Path]::GetFileName($entry.Value))) -Force
}

$resourceCopies = @(
    @{ Source = Join-Path $ucrtRoot 'share\\glib-2.0\\schemas'; Destination = 'share\\glib-2.0\\schemas'; Required = $true },
    @{ Source = Join-Path $ucrtRoot 'share\\icons\\Adwaita'; Destination = 'share\\icons\\Adwaita'; Required = $true },
    @{ Source = Join-Path $ucrtRoot 'share\\icons\\hicolor'; Destination = 'share\\icons\\hicolor'; Required = $false }
)
foreach ($copy in $resourceCopies) {
    if (-not (Test-Path -LiteralPath $copy.Source)) {
        if ($copy.Required) { throw "Required GTK runtime data is missing: $($copy.Source)" }
        continue
    }
    $destination = Join-Path $runtime $copy.Destination
    New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
    Copy-Item -LiteralPath $copy.Source -Destination $destination -Recurse -Force
}

$schema = Join-Path $runtime 'share\\glib-2.0\\schemas\\gschemas.compiled'
if (-not (Test-Path -LiteralPath $schema)) {
    throw 'Portable Preferences runtime is missing gschemas.compiled.'
}

$files = @(Get-ChildItem -LiteralPath $runtime -File -Recurse)
$totalBytes = ($files | Measure-Object -Property Length -Sum).Sum
$manifest = [ordered]@{
    generated_utc = [DateTime]::UtcNow.ToString('o')
    source_executable = 'fedorawin-preferences.exe'
    layout = 'preferences-runtime'
    runtime_dll_count = $runtimeDlls.Count
    file_count = $files.Count
    total_bytes = [int64]$totalBytes
    total_mb = [Math]::Round($totalBytes / 1MB, 1)
    msys2_root_used_for_staging = $env:MSYS2_ROOT
    verification = 'CI staging only; not a beta distribution artifact.'
    dlls = @($runtimeDlls.Keys | Sort-Object)
}
$manifestPath = Join-Path $outRoot 'portable-runtime.json'
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
Write-Host "PORTABLE STAGE: $($runtimeDlls.Count) UCRT64 DLLs, $($files.Count) files, $($manifest.total_mb) MB."
