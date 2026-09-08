<#
    Runs the BiRefNet shot classifier over a folder of images.

    Reads images from  <project>\test data\INPUT   (override with -InputDir)
    Writes results to   <project>\test data\OUTPUT  (override with -OutputDir)

    For every input image, OUTPUT gets:
      <stem>--EIX=<trbl>[--BGC=<hex>][--FGC=<hex>].<ext>        (step 0)
                              the input image, copied + renamed with the
                              shot code derived so far
      <stem>_1_segmask.png    BiRefNet foreground mask                  (step 1)
      <stem>_2_refine.png     white canvas the size of the segmask with (step 2)
                              each gated edge's refined region pasted back
      zzz_results.json        per-image verdict + shot code

    EIX = edge intersection (4-bit top-right-bottom-left). BGC = uniform
    background colour (>97.5% of the inverse mask, else omitted). FGC =
    weighted-median colour of the largest colour blob inside the mask.
    `=` stands in for the spec's `:` (illegal in Windows filenames). The
    _<n>_ infix keeps generated files sorted in pipeline order next to
    their source; zzz_results.json sorts last. OUTPUT's loose files are
    cleared each run.

    Uses the bundled model + ONNX Runtime next to this script:
      birefnet_lite_512.onnx , onnxruntime.dll
    (regenerate the model at another resolution with tools/export_birefnet_onnx.py)
#>
param(
    [string]$InputDir,
    [string]$OutputDir
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot

if (-not $InputDir)  { $InputDir  = Join-Path $root 'test data\INPUT' }
if (-not $OutputDir) { $OutputDir = Join-Path $root 'test data\OUTPUT' }

if (-not (Test-Path $InputDir)) {
    New-Item -ItemType Directory -Path $InputDir -Force | Out-Null
    Write-Host "Created $InputDir - drop images in there and run this again."
    exit 0
}

$env:ORT_DYLIB_PATH = Join-Path $root 'onnxruntime.dll'
$env:BIREFNET_ONNX   = Join-Path $root 'birefnet_lite_512.onnx'

foreach ($f in @($env:ORT_DYLIB_PATH, $env:BIREFNET_ONNX)) {
    if (-not (Test-Path $f)) { Write-Error "missing required file: $f"; exit 1 }
}

Push-Location $root
try {
    & cargo run --release --features birefnet --example run_dir -- $InputDir $OutputDir
    $code = $LASTEXITCODE
} finally {
    Pop-Location
}

if ($code -eq 0) { Write-Host "`nDone. Results in $OutputDir" }
else { Write-Host "`nrun_dir exited with code $code" }
exit $code
