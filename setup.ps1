[CmdletBinding()]
param([string[]]$Components,[string]$Root=(Join-Path $env:LOCALAPPDATA 'URT'),[string]$WgConfig)
$ErrorActionPreference='Stop'
$engine=Join-Path $PSScriptRoot 'routing.ps1'
if(-not $Components){
    Write-Host 'URT installs selected components. Starting them is a separate action.'
    Write-Host '1: Mihomo   2: TG WS Proxy   3: Zapret (x64)'
    $answer=Read-Host 'Enter component numbers separated by commas (empty cancels)'
    $Components=@($answer -split ',' | ForEach-Object {switch($_.Trim()){'1'{'mihomo'}'2'{'telegram'}'3'{'zapret'}}})
}
$Components=@($Components | ForEach-Object {$_ -split ','} | ForEach-Object {$_.Trim().ToLowerInvariant()} | Where-Object {$_} | Select-Object -Unique)
foreach($component in $Components){
    if($component -notin @('mihomo','telegram','zapret')){throw "Unknown component: $component"}
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $engine -Root $Root -Command Install -Component $component
    if($LASTEXITCODE -ne 0){throw "Installation failed: $component"}
}
if($WgConfig){
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $engine -Root $Root -Command SelectConfig -Path $WgConfig
    if($LASTEXITCODE -ne 0){throw 'Profile selection failed.'}
}
if($Components.Count -gt 0){Write-Host 'Components installed. Open URT.exe to select a profile and start the components you need.'}
