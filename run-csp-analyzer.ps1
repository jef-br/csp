<#
    Launcher for CSP-Analyzer, the dev-only replay/inspection tool (src/bin/csp_analyzer.rs).

    Reads images from Desktop\CSP-INPUT and writes per-stage debug snapshots to
    Desktop\CSP-ANALYZER-OUTPUT — a folder distinct from the app's own CSP-OUTPUT, since
    csp_analyzer never touches or deletes the source images (no Exporter involved).
#>
param(
    [switch]$Release
)

$ErrorActionPreference = 'Stop'
$repoRoot = $PSScriptRoot

$desktop = [Environment]::GetFolderPath('Desktop')
$inputDir = Join-Path $desktop 'CSP-INPUT'
$outputDir = Join-Path $desktop 'CSP-ANALYZER-OUTPUT'

if (-not (Test-Path $inputDir)) {
    New-Item -ItemType Directory -Path $inputDir -Force | Out-Null
    Write-Host "Created $inputDir - drop images in there and run this again."
    Read-Host "Press Enter to close"
    exit 0
}

$extensions = @('.jpg', '.jpeg', '.png', '.bmp', '.tif', '.tiff', '.webp')
$hasImages = @(Get-ChildItem -Path $inputDir -File | Where-Object { $extensions -contains $_.Extension.ToLower() }).Count -gt 0
if (-not $hasImages) {
    Write-Host "No images found in $inputDir - drop images in there and run this again."
    Read-Host "Press Enter to close"
    exit 0
}

$cargoArgs = @('run')
if ($Release) { $cargoArgs += '--release' }
$cargoArgs += @('--bin', 'csp_analyzer', '--', $inputDir, $outputDir)

Push-Location $repoRoot
try {
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
} catch {
    Write-Host "Failed to run cargo: $_"
    $exitCode = 1
} finally {
    Pop-Location
}

if ($exitCode -eq 0) {
    Write-Host "`nDone. Snapshots written to $outputDir"
} else {
    Write-Host "`ncsp_analyzer exited with code $exitCode"
}
Read-Host "Press Enter to close"
