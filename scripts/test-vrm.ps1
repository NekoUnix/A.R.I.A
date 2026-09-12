#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Vrm)
$ErrorActionPreference = 'Stop'
$ariaVrmPath = (Resolve-Path -LiteralPath $Vrm).Path
$ariaPreviousVrm = $env:ARIA_TEST_VRM
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
try {
    $env:ARIA_TEST_VRM = $ariaVrmPath
    & cargo test --locked --release -p aria-desktop vrm:: -- --include-ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'VRM verification failed' }
} finally {
    $env:ARIA_TEST_VRM = $ariaPreviousVrm
    Pop-Location
}
