#requires -Version 5.1
[CmdletBinding()]
param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$ariaRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Cargo was not found. Install Rust and MSVC C++ Build Tools; see docs/windows.md.'
}
Push-Location -LiteralPath $ariaRoot
try {
    $ariaHost = (& rustc -vV | Select-String '^host: ').ToString().Substring(6).Trim()
    if ($LASTEXITCODE -ne 0 -or $ariaHost -notmatch '^x86_64-pc-windows-(msvc|gnu)$') {
        throw 'This packaging script targets Windows x64. See docs/windows.md for the MSVC setup.'
    }
    if (-not $SkipTests) {
        & cargo test --locked --workspace
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed; no package was produced.' }
    }
    & cargo build --locked --release --workspace
    if ($LASTEXITCODE -ne 0) { throw 'Rust build failed; no package was produced.' }

    $ariaVersion = '0.4.0'
    $ariaDist = Join-Path $ariaRoot 'dist'
    $ariaStage = Join-Path $ariaDist ('staging\' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force -Path $ariaStage | Out-Null
    Copy-Item -LiteralPath (Join-Path $ariaRoot 'target\release\aria-desktop.exe'), (Join-Path $ariaRoot 'target\release\aria-cli.exe') -Destination $ariaStage
    Copy-Item -LiteralPath (Join-Path $ariaRoot 'README.md'), (Join-Path $ariaRoot 'LICENSE'), (Join-Path $ariaRoot 'THIRD_PARTY.md') -Destination $ariaStage
    Copy-Item -LiteralPath (Join-Path $ariaRoot 'docs') -Destination $ariaStage -Recurse
    $ariaScripts = Join-Path $ariaStage 'scripts'
    New-Item -ItemType Directory -Force -Path $ariaScripts | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'allow-tracking-firewall.ps1') -Destination $ariaScripts
    # Preserve the license files published with the resolved dependency crates.
    $ariaMetadataJson = & cargo metadata --locked --format-version 1 --filter-platform $ariaHost
    if ($LASTEXITCODE -ne 0) { throw 'Dependency metadata failed; no package was produced.' }
    $ariaMetadata = $ariaMetadataJson | ConvertFrom-Json
    $ariaLicenses = Join-Path $ariaStage 'dependency-licenses'
    New-Item -ItemType Directory -Force -Path $ariaLicenses | Out-Null
    $ariaIndex = @('# Resolved dependency licenses', '', 'License identifiers are reported by each dependency in Cargo metadata.', '')
    foreach ($ariaDep in $ariaMetadata.packages) {
        if (-not $ariaDep.source) { continue }
        $ariaDepRoot = Split-Path -Parent $ariaDep.manifest_path
        $ariaLicenseFiles = @(Get-ChildItem -LiteralPath $ariaDepRoot -Recurse -File | Where-Object Name -Match '(^|[-_])(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)|^(OFL|UFL)\.txt$')
        $ariaDepFolder = Join-Path $ariaLicenses ($ariaDep.name + '-' + $ariaDep.version)
        New-Item -ItemType Directory -Force -Path $ariaDepFolder | Out-Null
        foreach ($ariaFile in $ariaLicenseFiles) {
            $ariaRelative = $ariaFile.FullName.Substring($ariaDepRoot.Length).TrimStart('\', '/')
            $ariaLicenseTarget = Join-Path $ariaDepFolder $ariaRelative
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $ariaLicenseTarget) | Out-Null
            Copy-Item -LiteralPath $ariaFile.FullName -Destination $ariaLicenseTarget
        }
        $ariaIndex += ('- ' + $ariaDep.name + ' ' + $ariaDep.version + ': ' + $ariaDep.license + ' — ' + $ariaDep.repository)
    }
    $ariaIndex | Set-Content -LiteralPath (Join-Path $ariaLicenses 'INDEX.md') -Encoding UTF8
    # ZIP timestamps start in 1980; normalize only the staged copies of old notices.
    Get-ChildItem -LiteralPath $ariaStage -Recurse -File | Where-Object LastWriteTime -LT ([datetime]'1980-01-01') | ForEach-Object {
        $_.LastWriteTime = [datetime]'1980-01-02'
    }
    $ariaZip = Join-Path $ariaDist "aria-$ariaVersion-windows-x64.zip"
    Compress-Archive -Path (Join-Path $ariaStage '*') -DestinationPath $ariaZip -Force
    Write-Output "Portable app: $ariaZip"
    Get-FileHash -LiteralPath $ariaZip -Algorithm SHA256 | Format-List
} finally {
    Pop-Location
}
