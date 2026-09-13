# Enable Settings > Developer API and set $env:ARIA_API_KEY locally first.
$ErrorActionPreference = 'Stop'
if (-not $env:ARIA_API_KEY) { throw 'Set ARIA_API_KEY to the key copied from ARIA.' }
$ariaBase = 'http://127.0.0.1:39421'
$ariaHeaders = @{ Authorization = "Bearer $env:ARIA_API_KEY" }
$ariaState = Invoke-RestMethod "$ariaBase/v1/state" -Headers $ariaHeaders
$ariaBody = @{ version=1; generation=$ariaState.generation; action=@{ type='theme'; name='Sonoma Dark' } } | ConvertTo-Json -Depth 5
$ariaQueued = Invoke-RestMethod "$ariaBase/v1/commands" -Method Post -Headers $ariaHeaders -ContentType 'application/json' -Body $ariaBody
for ($ariaAttempt=0; $ariaAttempt -lt 25; $ariaAttempt++) {
    Start-Sleep -Milliseconds 200
    $ariaResult = Invoke-RestMethod "$ariaBase/v1/commands/$($ariaQueued.ticket)" -Headers $ariaHeaders
    if ($ariaResult.status -eq 'applied') { Write-Output 'Theme applied'; return }
    if ($ariaResult.status -eq 'rejected') { throw $ariaResult.error }
}
throw 'Command still pending; inspect its ticket before retrying.'
