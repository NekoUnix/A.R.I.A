param(
    [Parameter(Mandatory = $true)][string]$ApiKey,
    [ValidateRange(1024, 65535)][int]$Port = 39421,
    [UInt64]$EffectId = 1,
    [switch]$List
)
$ErrorActionPreference = 'Stop'
$ariaHeaders = @{ Authorization = "Bearer $ApiKey" }
$ariaBase = "http://127.0.0.1:$Port/v1/effects"
if ($List) {
    Invoke-RestMethod -Uri $ariaBase -Headers $ariaHeaders -TimeoutSec 3
} else {
    $ariaBody = @{ version = 1; id = $EffectId } | ConvertTo-Json -Compress
    Invoke-RestMethod -Uri "$ariaBase/trigger" -Method Post -Headers $ariaHeaders -ContentType 'application/json' -Body $ariaBody -TimeoutSec 3
}
