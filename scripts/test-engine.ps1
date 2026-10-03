[CmdletBinding()]
param([switch]$Integration)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
$tempBase=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$runtime=Join-Path $tempBase ('URT-test-'+[guid]::NewGuid().ToString('N'))
$script:checks=0
function Assert-That([bool]$Condition,[string]$Message){if(-not $Condition){throw "FAILED: $Message"};$script:checks++;Write-Host "PASS: $Message"}
function Assert-Throws([scriptblock]$Block,[string]$Message){$failed=$false;try{& $Block | Out-Null}catch{$failed=$true};Assert-That $failed $Message}
function New-TestKey {$rng=[Security.Cryptography.RandomNumberGenerator]::Create();$bytes=New-Object byte[] 32;try{$rng.GetBytes($bytes)}finally{$rng.Dispose()};return [Convert]::ToBase64String($bytes)}
try {
    foreach($file in (Get-ChildItem -LiteralPath $project -Filter *.ps1 -File -Recurse | Where-Object {$_.FullName -notmatch '\\(target|dist|\.qa)\\'})) {
        $tokens=$null;$errors=$null;[System.Management.Automation.Language.Parser]::ParseFile($file.FullName,[ref]$tokens,[ref]$errors) | Out-Null
        Assert-That ($errors.Count -eq 0) ('PowerShell parser: '+$file.Name)
    }
    . (Join-Path $project 'routing.ps1') -Command None -Root $runtime
    Assert-That (-not(Test-Path -LiteralPath $runtime)) 'Loading the engine has no runtime writes'
    Initialize-Runtime
    Assert-Throws {Assert-RuntimePath (Join-Path $tempBase 'outside.json')} 'Writes outside the runtime are rejected'
    Assert-Throws {Assert-RuntimePath ($runtime+'-sibling\settings.json')} 'Sibling prefix cannot bypass the path check'
    $settings=Read-Settings;Save-Settings $settings
    Assert-That ((Get-Acl -LiteralPath (Join-Path $runtime 'settings.json')).AreAccessRulesProtected) 'Private settings have a protected ACL'
    Assert-That ((Read-Settings).schema_version -eq 1) 'Runtime settings round-trip'
    $applied=[pscustomobject]@{ProxyEnable=[pscustomobject]@{exists=$true;value=1};ProxyServer=[pscustomobject]@{exists=$true;value='127.0.0.1:17890'};ProxyOverride=[pscustomobject]@{exists=$true;value='<local>'};AutoConfigURL=[pscustomobject]@{exists=$false;value=$null}}
    $matching=[pscustomobject]@{ProxyEnable=1;ProxyServer='127.0.0.1:17890';ProxyOverride='<local>'}
    Assert-ProxyOwnership $matching $applied
    Assert-That $true 'Exact owned proxy values are recognized without changing the registry'
    $changed=[pscustomobject]@{ProxyEnable=1;ProxyServer='127.0.0.1:17890';ProxyOverride='<local>';AutoConfigURL='https://example.com/new.pac'}
    Assert-Throws {Assert-ProxyOwnership $changed $applied} 'An external PAC change prevents proxy restoration'
    $wg=Join-Path $runtime 'test.conf'
    $profile="[Interface]`nPrivateKey = $(New-TestKey)`nAddress = 10.0.0.2/32, fd00::2/128`nDNS = 1.1.1.1`n[Peer]`nPublicKey = $(New-TestKey)`nEndpoint = [2001:db8::1]:51820`nAllowedIPs = 0.0.0.0/0, ::/0`nPersistentKeepalive = 25`n"
    [IO.File]::WriteAllText($wg,$profile)
    $parsed=Read-WireGuard $wg;$yaml=New-WireGuardYaml $parsed
    Assert-That ($parsed.Server -eq '2001:db8::1' -and $parsed.Port -eq 51820) 'Bracketed IPv6 endpoint is parsed correctly'
    Assert-That ($yaml.Contains('  enable: false') -and $yaml.Contains('  - MATCH,DIRECT')) 'Imported defaults keep TUN off and catch-all direct'
    Assert-That ($yaml.IndexOf('DOMAIN-SUFFIX,chatgpt.com,WG') -lt $yaml.IndexOf('PROCESS-NAME,chrome.exe,DIRECT')) 'Browser domain rules precede browser process fallback'
    [IO.File]::WriteAllText($wg,$profile+"[Peer]`nPublicKey = $(New-TestKey)`n")
    Assert-Throws {Read-WireGuard $wg} 'Multiple peers are rejected'
    [IO.File]::WriteAllText($wg,$profile.Replace('Address = 10.0.0.2/32','Address = 10.0.0.2/99'))
    Assert-Throws {Read-WireGuard $wg} 'Invalid interface prefix is rejected'
    [IO.File]::WriteAllText($wg,$profile.Replace('[Peer]',"PostUp = echo unsafe`n[Peer]"))
    Assert-Throws {Read-WireGuard $wg} 'Executable WireGuard hooks are rejected'
    [IO.File]::WriteAllText($wg,$profile)
    $originalValidator=${function:Test-MihomoConfig}
    if($Integration){Install-Component 'mihomo'}else{function Test-MihomoConfig([string]$Configuration){if(-not(Test-Path -LiteralPath $Configuration)){throw 'Missing configuration'}}}
    Select-Configuration $wg
    $s=Read-Settings
    Assert-That (@($s.profiles).Count -eq 1) 'Single profile is stored as a JSON array'
    Assert-That ($s.active_config -ne $wg -and (Test-Path -LiteralPath $wg)) 'WireGuard source remains unchanged'
    Assert-That (-not(Get-OwnedProcess 'mihomo')) 'Selecting a profile does not start the core'
    $pac=Get-Content -LiteralPath (Join-Path $runtime 'browser-routing.pac') -Raw
    Assert-That ($pac.Contains('return "DIRECT";') -and $pac.Contains('chatgpt.com')) 'Browser PAC has selective proxy rules and a direct fallback'
    $cfg=Ensure-TgConfig
    Assert-That ($cfg.host -eq '127.0.0.1' -and $cfg.secret -match '^[a-f0-9]{32}$' -and -not $cfg.no_secure) 'Telegram default uses loopback, a generated secret and TLS'
    Assert-That ((Get-Acl -LiteralPath (Get-TgConfigPath)).AreAccessRulesProtected) 'Telegram secret file has a protected ACL'
    Assert-That ((Get-TelegramLink).StartsWith('tg://proxy?server=127.0.0.1&port=1443&secret=')) 'Telegram link uses the private configured secret'
    $self=Get-Process -Id $PID
    $record=@{pid=$PID;path=$self.Path;start_time=([DateTimeOffset]$self.StartTime.ToUniversalTime()).ToUnixTimeSeconds();config=''}
    Write-AtomicText (Join-Path $runtime 'mihomo.pid.json') ($record | ConvertTo-Json) -Private
    Assert-Throws {Stop-OwnedProcess 'mihomo'} 'A process outside this runtime cannot be stopped'
    Remove-Item -LiteralPath (Join-Path $runtime 'mihomo.pid.json')
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zipPath=Join-Path $runtime 'unsafe.zip';$zip=[IO.Compression.ZipFile]::Open($zipPath,'Create');try{$null=$zip.CreateEntry('../escape.txt')}finally{$zip.Dispose()}
    Assert-Throws {Expand-CheckedZip $zipPath (Join-Path $runtime 'unpack')} 'ZIP traversal is rejected before extraction'
    if($Integration){
        $listener=New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback,0);$listener.Start();$testPort=$listener.LocalEndpoint.Port;$listener.Stop()
        $direct=Join-Path $runtime 'profiles\direct.yaml';$text=(Get-Content -LiteralPath (Join-Path $project 'config\examples\mihomo-direct.yaml') -Raw).Replace('17890',[string]$testPort)
        Write-AtomicText $direct $text -Private
        $sourceHash=(Get-FileHash -LiteralPath $direct).Hash
        Select-Configuration $direct
        Assert-That ((Get-FileHash -LiteralPath $direct).Hash -eq $sourceHash) 'Selecting YAML does not rewrite the source'
        $proxyKey='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
        $before=Get-ItemProperty -LiteralPath $proxyKey | Select-Object ProxyEnable,ProxyServer,ProxyOverride,AutoConfigURL | ConvertTo-Json -Compress
        Start-Mihomo
        Assert-That ([bool](Get-OwnedProcess 'mihomo') -and (Test-Port $testPort)) 'Isolated proxy-only core starts on its own random loopback port'
        Stop-OwnedProcess 'mihomo'
        Assert-That (-not(Test-Port $testPort)) 'Isolated core stops and releases its listener'
        $after=Get-ItemProperty -LiteralPath $proxyKey | Select-Object ProxyEnable,ProxyServer,ProxyOverride,AutoConfigURL | ConvertTo-Json -Compress
        Assert-That ($before -eq $after) 'Isolated integration leaves Windows proxy settings unchanged'
    }
    ${function:Test-MihomoConfig}=$originalValidator
    Write-Host "$script:checks engine checks passed."
} finally {
    if(Test-Path -LiteralPath $runtime){
        if(Get-Command Get-OwnedProcess -ErrorAction SilentlyContinue){try{Stop-OwnedProcess 'mihomo'}catch{}}
        $resolved=[IO.Path]::GetFullPath($runtime)
        if(-not $resolved.StartsWith($tempBase,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notmatch '^URT-test-[a-f0-9]{32}$'){throw 'Unsafe test cleanup path.'}
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
