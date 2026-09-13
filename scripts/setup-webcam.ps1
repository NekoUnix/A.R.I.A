#requires -Version 5.1
[CmdletBinding()]
param([string]$Python = 'py', [string]$Destination = (Join-Path $env:LOCALAPPDATA 'ARIA\tracking'))
$ErrorActionPreference = 'Stop'
trap { Write-Output $_.Exception.Message; exit 1 }
$ariaTracking = Join-Path (Split-Path -Parent $PSScriptRoot) 'tracking'
if (-not (Get-Command $Python -ErrorAction SilentlyContinue)) {
    throw 'Install 64-bit Python 3.12 from python.org (include the Python launcher), then retry. You can also choose python.exe in ARIA.'
}
$ariaPrefix = @()
if ([IO.Path]::GetFileNameWithoutExtension($Python) -eq 'py') { $ariaPrefix = @('-3.12') }
$ariaVersion = & $Python @ariaPrefix -c 'import sys; print(str(sys.version_info.major)+"."+str(sys.version_info.minor))'
if ($LASTEXITCODE -ne 0 -or $ariaVersion -ne '3.12') { throw 'Select Python 3.12 x64 for this pinned webcam runtime.' }
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
& $Python @ariaPrefix -m venv $Destination
if ($LASTEXITCODE -ne 0) { throw 'Could not create webcam environment.' }
$ariaInterpreter = Join-Path $Destination 'Scripts\python.exe'
& $ariaInterpreter -m pip install --disable-pip-version-check -r (Join-Path $ariaTracking 'requirements-lock.txt')
if ($LASTEXITCODE -ne 0) { throw 'Webcam dependency install failed. Check your internet connection and retry.' }
# This is ARIA's dedicated virtual environment. MediaPipe 1 no longer needs
# these legacy packages; remove leftovers when repairing a v0.23 installation.
& $ariaInterpreter -m pip uninstall --disable-pip-version-check --yes protobuf jax jaxlib ml-dtypes scipy opt-einsum sentencepiece
if ($LASTEXITCODE -ne 0) { throw 'Could not remove obsolete camera packages.' }
& $ariaInterpreter -m pip check
if ($LASTEXITCODE -ne 0) { throw 'Camera dependencies conflict. Use a dedicated ARIA runtime folder.' }
$ariaModel = Join-Path $Destination 'face_landmarker.task'
Invoke-WebRequest -UseBasicParsing 'https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task' -OutFile ($ariaModel + '.download')
$ariaExpected = '64184E229B263107BC2B804C6625DB1341FF2BB731874B0BCC2FE6544E0BC9FF'
if ((Get-FileHash -LiteralPath ($ariaModel + '.download') -Algorithm SHA256).Hash -ne $ariaExpected) { throw 'Face model checksum mismatch; installation was not completed.' }
Move-Item -LiteralPath ($ariaModel + '.download') -Destination $ariaModel -Force
& $ariaInterpreter -c 'import mediapipe, cv2, cv2_enumerate_cameras; print("Webcam runtime ready")'
if ($LASTEXITCODE -ne 0) { throw 'Webcam import validation failed.' }
Write-Output "Ready: $ariaInterpreter"
