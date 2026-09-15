#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Glb)
$ErrorActionPreference = 'Stop'
$ariaGlbPath = (Resolve-Path -LiteralPath $Glb).Path
$ariaPreviousGlb = $env:ARIA_TEST_GLB
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
try {
    $env:ARIA_TEST_GLB = $ariaGlbPath
    & cargo test --locked --release -p aria-desktop local_glb_tracks_expressions_gestures_and_freezes -- --ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'GLB avatar verification failed' }
} finally {
    $env:ARIA_TEST_GLB = $ariaPreviousGlb
    Pop-Location
}
