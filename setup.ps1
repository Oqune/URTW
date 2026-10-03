[CmdletBinding()]
param([string[]]$Components,[string]$Root=$(if(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'portable.flag')){Join-Path $PSScriptRoot 'data'}else{Join-Path $env:LOCALAPPDATA 'URTW'}),[string]$WgConfig)
$ErrorActionPreference='Stop'
$engine=Join-Path $PSScriptRoot 'routing.ps1'
if(-not $Components){
    Write-Host 'URTW installs selected components. Starting them is a separate action.'
    if(-not $PSBoundParameters.ContainsKey('Root')){$chosen=Read-Host ('Data folder [Enter: '+$Root+']');if($chosen){$Root=$chosen}}
    Write-Host '1: Mihomo   2: TG WS Proxy   3: Zapret (x64)   4: sing-box   5: Xray'
    $answer=Read-Host 'Enter component numbers separated by commas (empty cancels)'
    $Components=@($answer -split ',' | ForEach-Object {switch($_.Trim()){'1'{'mihomo'}'2'{'telegram'}'3'{'zapret'}'4'{'singbox'}'5'{'xray'}}})
}
$Components=@($Components | ForEach-Object {$_ -split ','} | ForEach-Object {$_.Trim().ToLowerInvariant()} | Where-Object {$_} | Select-Object -Unique)
foreach($component in $Components){
    if($component -notin @('mihomo','singbox','xray','telegram','zapret')){throw "Unknown component: $component"}
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $engine -Root $Root -Command Install -Component $component
    if($LASTEXITCODE -ne 0){throw "Installation failed: $component"}
}
if($WgConfig){
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $engine -Root $Root -Command SelectConfig -Path $WgConfig
    if($LASTEXITCODE -ne 0){throw 'Profile selection failed.'}
}
if($Components.Count -gt 0){Write-Host 'Components installed. Open URTW.exe to select a profile and start the components you need.'}
