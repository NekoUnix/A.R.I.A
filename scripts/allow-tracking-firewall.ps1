#requires -Version 5.1
#requires -RunAsAdministrator
[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$AppPath,
    [ValidateRange(1, 65535)][int]$Port = 11125,
    [switch]$Remove
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$ariaRule = "ARIA-Tracking-UDP-$Port"

if ($Remove) {
    if ($PSCmdlet.ShouldProcess($ariaRule, 'Remove A.R.I.A. tracking firewall rule')) {
        Get-NetFirewallRule -Name $ariaRule -ErrorAction SilentlyContinue | Remove-NetFirewallRule
    }
    return
}

if (-not $AppPath) { throw 'Specify -AppPath with the exact aria-desktop.exe or aria-cli.exe you will run.' }
$ariaExe = (Resolve-Path -LiteralPath $AppPath).Path
if ([System.IO.Path]::GetFileName($ariaExe) -notin @('aria-desktop.exe', 'aria-cli.exe')) {
    throw 'AppPath must point to aria-desktop.exe or aria-cli.exe.'
}
if (-not (Test-Path -LiteralPath $ariaExe -PathType Leaf)) { throw 'The application executable does not exist.' }
if (Get-NetFirewallRule -Name $ariaRule -ErrorAction SilentlyContinue) {
    throw "Rule $ariaRule already exists. Run this script with -Port $Port -Remove first if you intend to replace it."
}
if ($PSCmdlet.ShouldProcess("$ariaExe / UDP $Port / Private / LocalSubnet", 'Allow incoming A.R.I.A. tracking')) {
    New-NetFirewallRule -Name $ariaRule -DisplayName "A.R.I.A. Tracking UDP $Port" `
        -Direction Inbound -Action Allow -Enabled True -Profile Private `
        -Program $ariaExe -Protocol UDP -LocalPort $Port -RemoteAddress LocalSubnet | Out-Null
    Write-Output "Allowed UDP $Port for $ariaExe from the local subnet on Private networks."
}
