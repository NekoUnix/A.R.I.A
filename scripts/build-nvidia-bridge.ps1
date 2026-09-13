#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Sdk)
$ErrorActionPreference = 'Stop'
trap { Write-Output $_.Exception.Message; exit 1 }
$ariaSdk = (Resolve-Path -LiteralPath $Sdk).Path
$ariaSource = Join-Path (Split-Path -Parent $PSScriptRoot) 'tracking\nvidia'
$ariaBuild = Join-Path $env:LOCALAPPDATA 'ARIA\nvidia-build'
& cmake -S $ariaSource -B $ariaBuild -G 'Visual Studio 17 2022' -A x64 "-DARSDK_ROOT=$ariaSdk"
if ($LASTEXITCODE -ne 0) { throw 'NVIDIA bridge configuration failed. Install VS 2022 C++ Build Tools, CMake and matching SDK features.' }
& cmake --build $ariaBuild --config Release
if ($LASTEXITCODE -ne 0) { throw 'NVIDIA bridge build failed.' }
& cmake --install $ariaBuild --config Release --prefix $ariaSdk
if ($LASTEXITCODE -ne 0) { throw 'Cannot install bridge into the selected SDK folder.' }
Write-Output 'ARIA NVIDIA bridge ready. Choose this SDK folder in Tracking > Webcam · NVIDIA RTX.'
