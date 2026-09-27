#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Folder)
$ErrorActionPreference = 'Stop'
$ariaFolder = (Resolve-Path -LiteralPath $Folder).Path
$ariaOldFolder = $env:ARIA_TEST_MODEL_FOLDER
$ariaOldHost = $env:ARIA_CUBISM_HOST
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
try {
    $env:ARIA_TEST_MODEL_FOLDER = $ariaFolder
    & cargo build --locked --release -p aria-live2d --bin aria-cubism-host
    if ($LASTEXITCODE -ne 0) { throw 'Bundled Purism host build failed' }
    $env:ARIA_CUBISM_HOST = (Resolve-Path -LiteralPath 'target/release/aria-cubism-host.exe').Path
    & cargo test --locked --release -p aria-desktop local_model_library_expressive_physics -- --ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Physics stability/frame-rate verification failed' }
    & cargo test --locked --release -p aria-desktop local_model_library_stress_physics_and_hosted_geometry -- --ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Physics stress/native-worker equivalence verification failed' }
    & cargo test --locked --release -p aria-desktop nested_model_library_renders_full_geometry -- --ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Live2D model library verification failed' }
} finally {
    $env:ARIA_TEST_MODEL_FOLDER = $ariaOldFolder
    $env:ARIA_CUBISM_HOST = $ariaOldHost
    Pop-Location
}
