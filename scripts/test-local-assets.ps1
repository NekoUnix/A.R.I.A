#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$GifDirectory,
    [Parameter(Mandatory=$true)][string]$Live2DModel,
    [Parameter(Mandatory=$true)][string]$CubismCore
)
$ErrorActionPreference = 'Stop'
$ariaPaths = @{
    ARIA_TEST_GIF_DIR = (Resolve-Path -LiteralPath $GifDirectory).Path
    ARIA_TEST_MODEL = (Resolve-Path -LiteralPath $Live2DModel).Path
    ARIA_CUBISM_CORE = (Resolve-Path -LiteralPath $CubismCore).Path
}
$ariaPrevious = @{}
foreach ($ariaName in $ariaPaths.Keys) {
    $ariaPrevious[$ariaName] = [Environment]::GetEnvironmentVariable($ariaName, 'Process')
}
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
try {
    foreach ($ariaName in $ariaPaths.Keys) {
        [Environment]::SetEnvironmentVariable($ariaName, $ariaPaths[$ariaName], 'Process')
    }
    # Assets are read in place. No model, GIF or SDK is copied into the repository.
    & cargo test --locked --release -p aria-desktop local_gif_set_imports_with_bounded_textures_and_original_timing -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Local GIF verification failed' }
    & cargo test --locked --release -p aria-desktop frozen_avatar_skips_core_and_gpu_work_but_edits_refresh_it -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Local Live2D verification failed' }
} finally {
    Pop-Location
    foreach ($ariaName in $ariaPrevious.Keys) {
        [Environment]::SetEnvironmentVariable($ariaName, $ariaPrevious[$ariaName], 'Process')
    }
}
