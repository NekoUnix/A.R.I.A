#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Folder, [Parameter(Mandatory=$true)][string]$Core)
$ErrorActionPreference = 'Stop'
$ariaFolder = (Resolve-Path -LiteralPath $Folder).Path
$ariaCore = (Resolve-Path -LiteralPath $Core).Path
$ariaOldFolder = $env:ARIA_TEST_MODEL_FOLDER
$ariaOldCore = $env:ARIA_CUBISM_CORE
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
try {
    $env:ARIA_TEST_MODEL_FOLDER = $ariaFolder
    $env:ARIA_CUBISM_CORE = $ariaCore
    & cargo test --locked --release -p aria-desktop nested_model_library_renders_full_geometry -- --ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Live2D model library verification failed' }
} finally {
    $env:ARIA_TEST_MODEL_FOLDER = $ariaOldFolder
    $env:ARIA_CUBISM_CORE = $ariaOldCore
    Pop-Location
}
