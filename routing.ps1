[CmdletBinding()]
param(
    [ValidateSet('Status','SelectConfig','Install','StartMihomo','StopMihomo','StartTG','StopTG','OpenZapret','CopyTGLink','OpenTelegram','EditTG','SetTGPort','EnableProxy','DisableProxy','Request','Boot','None')]
    [string]$Command='Status',
    [string]$Root=(Join-Path $env:LOCALAPPDATA 'URTW'),
    [string]$Path,
    [ValidatePattern('^[a-z][a-z0-9_-]{1,31}$')][string]$Component,
    [ValidateRange(1024,65535)][int]$Port=1443
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$Assets=$PSScriptRoot
$Root=[IO.Path]::GetFullPath($Root).TrimEnd('\')
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)

function Write-ProgressEvent([int]$Percent,[string]$Message) { [Console]::WriteLine("URT_PROGRESS|$Percent|$Message") }
function Assert-RuntimePath([string]$Candidate) {
    $full=[IO.Path]::GetFullPath($Candidate)
    if(-not $full.StartsWith($Root+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Refusing a write outside the selected runtime.'}
    return $full
}
function Protect-PrivateFile([string]$File) {
    $acl=New-Object Security.AccessControl.FileSecurity
    $acl.SetAccessRuleProtection($true,$false)
    foreach($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User.Value,'S-1-5-18','S-1-5-32-544')) {
        $rule=New-Object Security.AccessControl.FileSystemAccessRule((New-Object Security.Principal.SecurityIdentifier($sid)),'FullControl','Allow')
        $acl.AddAccessRule($rule)
    }
    [IO.File]::SetAccessControl($File,$acl)
}
function Protect-PrivateDirectory([string]$Directory) {
    $acl=New-Object Security.AccessControl.DirectorySecurity
    $acl.SetAccessRuleProtection($true,$false)
    foreach($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User.Value,'S-1-5-18','S-1-5-32-544')) {
        $rule=New-Object Security.AccessControl.FileSystemAccessRule((New-Object Security.Principal.SecurityIdentifier($sid)),'FullControl','ContainerInherit,ObjectInherit','None','Allow')
        $acl.AddAccessRule($rule)
    }
    [IO.Directory]::SetAccessControl($Directory,$acl)
}
function Initialize-Runtime {
    $protected=@([IO.Path]::GetPathRoot($Root).TrimEnd('\'),$env:USERPROFILE,$env:WINDIR,$Assets)
    if($protected -contains $Root){throw 'Choose a dedicated runtime folder, not a drive, user, Windows or project root.'}
    foreach($dir in @($Root,(Join-Path $Root 'tools'),(Join-Path $Root 'logs'),(Join-Path $Root 'profiles'),(Join-Path $Root 'mihomo-data'))) {
        if(-not (Test-Path -LiteralPath $dir)){New-Item -ItemType Directory -Path $dir -Force | Out-Null;Protect-PrivateDirectory $dir}
    }
}
function Write-AtomicText([string]$Destination,[string]$Text,[switch]$Private) {
    $Destination=Assert-RuntimePath $Destination
    $parent=Split-Path -Parent $Destination
    if(-not(Test-Path -LiteralPath $parent)){New-Item -ItemType Directory -Path $parent -Force | Out-Null}
    $temp=$Destination+'.'+[guid]::NewGuid().ToString('N')+'.tmp'
    try {
        [IO.File]::WriteAllText($temp,$Text,[Text.UTF8Encoding]::new($false))
        if($Private){Protect-PrivateFile $temp}
        if(Test-Path -LiteralPath $Destination){[IO.File]::Replace($temp,$Destination,[System.Management.Automation.Language.NullString]::Value)}else{[IO.File]::Move($temp,$Destination)}
        if($Private){Protect-PrivateFile $Destination}
    } finally {if(Test-Path -LiteralPath $temp){Remove-Item -LiteralPath $temp -Force}}
}
function Read-Settings {
    $file=Join-Path $Root 'settings.json'
    if(Test-Path -LiteralPath $file){
        $s=Get-Content -Encoding UTF8 -LiteralPath $file -Raw | ConvertFrom-Json
        if($s.schema_version -ne 1){throw 'Unsupported runtime settings schema.'}
        return $s
    }
    return [pscustomobject]@{schema_version=1;active_config=$null;profiles=@();mihomo_port=17890;tg_port=1443}
}
function Save-Settings($Settings) {Write-AtomicText (Join-Path $Root 'settings.json') ($Settings | ConvertTo-Json -Depth 8) -Private}
function Test-Port([int]$LocalPort) {
    $client=New-Object Net.Sockets.TcpClient
    try {$task=$client.ConnectAsync('127.0.0.1',$LocalPort); if($task.Wait(120)) {return $client.Connected}; return $false} catch {return $false} finally {$client.Dispose()}
}
function Get-OwnedProcess([string]$Name) {
    $file=Join-Path $Root ($Name+'.pid.json')
    if(-not(Test-Path -LiteralPath $file)){return $null}
    $r=Get-Content -Encoding UTF8 -LiteralPath $file -Raw | ConvertFrom-Json
    $process=Get-Process -Id $r.pid -ErrorAction SilentlyContinue
    if(-not $process){return $null}
    $identity=Get-CimInstance Win32_Process -Filter ('ProcessId='+$r.pid)
    if(-not $identity -or -not $identity.ExecutablePath){throw 'Cannot verify process identity; refusing to control it.'}
    $started=([DateTimeOffset]$identity.CreationDate.ToUniversalTime()).ToUnixTimeSeconds()
    if(-not [string]::Equals($identity.ExecutablePath,$r.path,[StringComparison]::OrdinalIgnoreCase) -or $started -ne $r.start_time){throw 'Process ownership changed; refusing to control it.'}
    $expected=if($Name -eq 'telegram'){Get-TgExe}else{Get-CoreExe $Name}
    if(-not[string]::Equals([IO.Path]::GetFullPath($identity.ExecutablePath),[IO.Path]::GetFullPath($expected),[StringComparison]::OrdinalIgnoreCase)){throw 'Recorded executable does not match this runtime adapter.'}
    return $process
}
function Save-Process([string]$Name,$Process,[string]$Configuration='',[int]$ListenPort=0) {
    $identity=Get-CimInstance Win32_Process -Filter ('ProcessId='+$Process.Id)
    if(-not $identity -or -not $identity.ExecutablePath){throw 'Cannot record the launched process identity.'}
    $record=@{pid=$Process.Id;path=$identity.ExecutablePath;start_time=([DateTimeOffset]$identity.CreationDate.ToUniversalTime()).ToUnixTimeSeconds();config=$Configuration;port=$ListenPort}
    Write-AtomicText (Join-Path $Root ($Name+'.pid.json')) ($record | ConvertTo-Json) -Private
}
function Stop-OwnedProcess([string]$Name) {
    $p=Get-OwnedProcess $Name
    if($p){
        if($Name -ne 'telegram'){
            $record=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $Root ($Name+'.pid.json')) -Raw | ConvertFrom-Json
            $current=Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
            Assert-ProxyListenerDetached $current ([int](Get-OptionalValue $record 'port' 0))
        }
        Stop-Process -Id $p.Id -ErrorAction Stop; $p.WaitForExit(5000) | Out-Null
    }
    $record=Assert-RuntimePath (Join-Path $Root ($Name+'.pid.json'))
    if(Test-Path -LiteralPath $record){Remove-Item -LiteralPath $record -Force}
}
function Assert-Key([string]$Value,[string]$Label) {
    try {if([Convert]::FromBase64String($Value).Length -ne 32){throw 'length'}} catch {throw "Invalid $Label; expected a base64-encoded 32-byte WireGuard key."}
}
function Read-WireGuard([string]$Source) {
    $profile=@{}; $section='';$peers=0
    foreach($raw in (Get-Content -Encoding UTF8 -LiteralPath $Source)) {
        $text=($raw -split '[#;]',2)[0].Trim()
        if(-not $text){continue}
        if($text -match '^\[(Interface|Peer)\]$'){$section=$matches[1];if($section -eq 'Peer'){$peers++};continue}
        if($text -notmatch '^([A-Za-z]+)\s*=\s*(.+)$' -or -not $section){throw 'Invalid WireGuard profile syntax.'}
        $key=$matches[1];$value=$matches[2].Trim()
        $allowed=if($section -eq 'Interface'){@('PrivateKey','Address','DNS','MTU')}else{@('PublicKey','PresharedKey','Endpoint','AllowedIPs','PersistentKeepalive')}
        if($allowed -notcontains $key){throw "Unsupported WireGuard directive: $key. Use a native Mihomo YAML for advanced profiles."}
        if($profile.ContainsKey($key)){throw 'Duplicate WireGuard directive.'}
        $profile[$key]=$value
    }
    if($peers -ne 1){throw 'Import requires exactly one WireGuard peer.'}
    foreach($required in @('PrivateKey','Address','PublicKey','Endpoint','AllowedIPs')){if(-not $profile.ContainsKey($required)){throw "WireGuard profile is missing $required."}}
    Assert-Key $profile.PrivateKey 'PrivateKey';Assert-Key $profile.PublicKey 'PublicKey'
    if($profile.ContainsKey('PresharedKey')){Assert-Key $profile.PresharedKey 'PresharedKey'}
    if($profile.Endpoint -notmatch '^(?:\[(?<host>[0-9a-fA-F:]+)\]|(?<host>[A-Za-z0-9.-]+)):(?<port>\d+)$'){throw 'Endpoint must be host:port or [IPv6]:port.'}
    $profile.Server=$matches.host;$profile.Port=[int]$matches.port
    if($profile.Port -lt 1 -or $profile.Port -gt 65535){throw 'Invalid WireGuard endpoint port.'}
    $profile.Ip=$null;$profile.Ipv6=$null
    foreach($address in ($profile.Address -split ',')) {
        $parts=$address.Trim() -split '/',2;$ip=$null
        if(-not [Net.IPAddress]::TryParse($parts[0],[ref]$ip)){throw 'Invalid WireGuard interface address.'}
        if($parts.Count -eq 2){$prefix=0;if(-not[int]::TryParse($parts[1],[ref]$prefix) -or $prefix -lt 0 -or $prefix -gt $(if($ip.AddressFamily -eq 'InterNetwork'){32}else{128})){throw 'Invalid WireGuard interface prefix.'}}
        if($ip.AddressFamily -eq 'InterNetwork'){$profile.Ip=$ip.ToString()}else{$profile.Ipv6=$ip.ToString()}
    }
    if(-not $profile.Ip){throw 'This importer requires an IPv4 interface address; use native YAML for IPv6-only profiles.'}
    foreach($cidr in ($profile.AllowedIPs -split ',')) {
        $parts=$cidr.Trim() -split '/',2;$ip=$null;$prefix=0
        if($parts.Count -ne 2 -or -not [Net.IPAddress]::TryParse($parts[0],[ref]$ip) -or -not [int]::TryParse($parts[1],[ref]$prefix) -or $prefix -lt 0 -or $prefix -gt $(if($ip.AddressFamily -eq 'InterNetwork'){32}else{128})){throw 'Invalid WireGuard AllowedIPs.'}
    }
    return $profile
}
function Read-RuleList([string]$Name) {
    $file=Join-Path $Root ('rules\'+$Name)
    if(-not(Test-Path -LiteralPath $file)){$file=Join-Path $Assets ('config\rules\'+$Name)}
    if(-not(Test-Path -LiteralPath $file)){return @()}
    return @(Get-Content -Encoding UTF8 -LiteralPath $file | ForEach-Object {$_.Trim().ToLowerInvariant()} | Where-Object {$_ -and -not $_.StartsWith('#')})
}
function New-WireGuardYaml($Profile) {
    $vpn=@(Read-RuleList 'vpn-domains.txt');$browser=@(Read-RuleList 'browser-vpn-domains.txt');$direct=@(Read-RuleList 'direct-domains.txt');$apps=@(Read-RuleList 'vpn-processes.txt')
    foreach($domain in @($vpn+$browser+$direct)){if($domain -notmatch '^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)*[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$'){throw 'Invalid domain in the routing lists.'}}
    foreach($process in $apps){if($process -notmatch '^[a-z0-9_.-]+\.exe$'){throw 'Invalid process in vpn-processes.txt.'}}
    $lines=New-Object 'Collections.Generic.List[string]'
    foreach($line in @('mixed-port: 17890','bind-address: 127.0.0.1','allow-lan: false','mode: rule','log-level: warning','ipv6: false','find-process-mode: strict','tun:','  enable: false','  stack: mixed','  auto-route: true','  auto-detect-interface: true','  strict-route: false','  dns-hijack: ["any:53"]','dns:','  enable: true','  listen: 127.0.0.1:1053','  enhanced-mode: fake-ip','  default-nameserver: [1.1.1.1, 8.8.8.8]','  nameserver: ["https://cloudflare-dns.com/dns-query"]','  proxy-server-nameserver: [1.1.1.1, 8.8.8.8]','proxies:','  - name: WG','    type: wireguard')){$lines.Add($line)}
    $fields=@{server=$Profile.Server;port=$Profile.Port;ip=$Profile.Ip;'private-key'=$Profile.PrivateKey;'public-key'=$Profile.PublicKey}
    if($Profile.Ipv6){$fields['ipv6']=$Profile.Ipv6}
    if($Profile.ContainsKey('PresharedKey')){$fields['pre-shared-key']=$Profile.PresharedKey}
    foreach($key in ($fields.Keys | Sort-Object)){$lines.Add('    '+$key+': '+($fields[$key] | ConvertTo-Json -Compress))}
    $ips=@($Profile.AllowedIPs -split ',' | ForEach-Object {$_.Trim()});$lines.Add('    allowed-ips: '+(ConvertTo-Json -InputObject $ips -Compress))
    $keepalive=if($Profile.ContainsKey('PersistentKeepalive')){[int]$Profile.PersistentKeepalive}else{25}
    if($keepalive -lt 0 -or $keepalive -gt 65535){throw 'Invalid WireGuard keepalive.'}
    $lines.Add("    persistent-keepalive: $keepalive");$lines.Add('    udp: true')
    if($Profile.ContainsKey('MTU')){$mtu=[int]$Profile.MTU;if($mtu -lt 1280 -or $mtu -gt 9000){throw 'Invalid WireGuard MTU.'};$lines.Add("    mtu: $mtu")}
    if($Profile.ContainsKey('DNS')){
        $dns=@($Profile.DNS -split ',' | ForEach-Object {$_.Trim()});foreach($address in $dns){$ip=$null;if(-not[Net.IPAddress]::TryParse($address,[ref]$ip)){throw 'WireGuard DNS import accepts IP addresses only.'}}
        $lines.Add('    remote-dns-resolve: true');$lines.Add('    dns: '+(ConvertTo-Json -InputObject $dns -Compress))
    }
    $lines.Add('rules:')
    foreach($cidr in @('127.0.0.0/8','10.0.0.0/8','172.16.0.0/12','192.168.0.0/16','169.254.0.0/16','224.0.0.0/4')){$lines.Add("  - IP-CIDR,$cidr,DIRECT,no-resolve")}
    $lines.Add('  - PROCESS-NAME,mihomo.exe,DIRECT')
    foreach($process in $apps){$lines.Add("  - PROCESS-NAME,$process,WG")}
    foreach($domain in $direct){$lines.Add("  - DOMAIN-SUFFIX,$domain,DIRECT")}
    foreach($domain in @($vpn+$browser | Sort-Object -Unique)){$lines.Add("  - DOMAIN-SUFFIX,$domain,WG")}
    foreach($process in @('chrome.exe','msedge.exe','firefox.exe','brave.exe','vivaldi.exe','opera.exe')){$lines.Add("  - PROCESS-NAME,$process,DIRECT")}
    $lines.Add('  - MATCH,DIRECT')
    return $lines -join "`n"
}
function Write-BrowserPac {
    $lines=New-Object 'Collections.Generic.List[string]';$lines.Add('function FindProxyForURL(url, host) {');$lines.Add('  host = host.toLowerCase();')
    foreach($domain in @(Read-RuleList 'browser-vpn-domains.txt')){
        if($domain -notmatch '^[a-z0-9.-]+$'){throw 'Invalid browser PAC domain.'}
        $lines.Add('  if (host === "'+$domain+'" || dnsDomainIs(host, ".'+$domain+'")) return "PROXY 127.0.0.1:17890; DIRECT";')
    }
    $lines.Add('  return "DIRECT";');$lines.Add('}')
    Write-AtomicText (Join-Path $Root 'browser-routing.pac') ($lines -join "`n") -Private
}
function Get-MihomoExe {return (Get-CoreExe 'mihomo')}
function Test-MihomoConfig([string]$Configuration) {
    $exe=Get-MihomoExe
    if(-not(Test-Path -LiteralPath $exe)){throw 'Install Mihomo before selecting and validating a configuration.'}
    Write-ProgressEvent 45 'Validating configuration with Mihomo'
    $output=& $exe -t -d (Join-Path $Root 'mihomo-data') -f $Configuration 2>&1
    if($LASTEXITCODE -ne 0){throw 'Mihomo rejected the configuration. Validate the selected YAML locally; credentials are not printed by URTW.'}
}
function Get-MihomoMixedPort([string]$Configuration) {
    $text=Get-Content -Encoding UTF8 -LiteralPath $Configuration -Raw
    if($text -notmatch '(?m)^mixed-port:\s*(\d+)\s*(?:#.*)?$'){throw 'URTW requires an explicit top-level mixed-port in the selected YAML.'}
    $mixedPort=[int]$matches[1];if($mixedPort -lt 1 -or $mixedPort -gt 65535){throw 'Invalid mixed-port.'}
    return $mixedPort
}
function Assert-RunningMihomoSelection($Settings,$Record) {
    $portProperty=$Record.PSObject.Properties['port']
    if(-not $portProperty -or $Record.port -ne $Settings.mihomo_port -or -not[string]::Equals($Record.config,$Settings.active_config,[StringComparison]::OrdinalIgnoreCase)){
        throw 'The selected profile differs from the running core. Stop and start managed Mihomo before enabling Windows Proxy.'
    }
}
function Select-Configuration([string]$Source) {
    if(-not(Test-Path -LiteralPath $Source -PathType Leaf)){throw 'Configuration file does not exist.'}
    $sourceFull=(Get-Item -LiteralPath $Source).FullName;$ext=[IO.Path]::GetExtension($sourceFull).ToLowerInvariant()
    $candidate=$sourceFull;$imported=$false
    if($ext -eq '.conf'){
        Write-ProgressEvent 10 'Reading WireGuard profile'
        $profile=Read-WireGuard $sourceFull
        $name=[IO.Path]::GetFileNameWithoutExtension($sourceFull) -replace '[^A-Za-z0-9_.-]','_'
        $candidate=Join-Path $Root ('profiles\'+$name+'-'+[guid]::NewGuid().ToString('N').Substring(0,8)+'.yaml')
        Write-AtomicText $candidate (New-WireGuardYaml $profile) -Private;$imported=$true
    } elseif($ext -notin @('.yaml','.yml')){throw 'Choose a Mihomo YAML or a single-peer WireGuard .conf file.'}
    try {Test-MihomoConfig $candidate} catch {if($imported){Remove-Item -LiteralPath (Assert-RuntimePath $candidate) -Force};throw}
    $mixedPort=Get-MihomoMixedPort $candidate
    $s=Read-Settings;$s.active_config=$candidate;$s.profiles=@(@($s.profiles)+@($candidate) | Select-Object -Unique);$s.mihomo_port=$mixedPort
    Save-Settings $s;if($imported){Write-BrowserPac}
    Write-ProgressEvent 100 'Selection saved; start or stop/start Mihomo to apply'
}
function Start-Mihomo {
    if(Get-OwnedProcess 'mihomo'){throw 'Managed Mihomo is already running. Stop it before applying a different selection.'}
    $s=Read-Settings;if(-not $s.active_config){throw 'Select a configuration first.'}
    if((Get-MihomoMixedPort $s.active_config) -ne $s.mihomo_port){throw 'The profile mixed-port changed; re-select it before starting Mihomo.'}
    Test-MihomoConfig $s.active_config
    if(Test-Port $s.mihomo_port){throw 'The selected Mihomo port is occupied by another process. URTW will not stop it.'}
    $text=Get-Content -Encoding UTF8 -LiteralPath $s.active_config -Raw
    $tunBlock=[regex]::Match($text,'(?ms)^tun:\s*(?:\{[^\r\n]*\}|\r?\n(?:[ \t]+[^\r\n]*\r?\n?)*)').Value
    if($tunBlock -match '(?i)enable:\s*true'){
        $admin=([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        if(-not $admin){throw 'This profile enables TUN. Open URTW in an Administrator terminal before starting it.'}
    }
    Write-ProgressEvent 65 'Starting the selected Mihomo profile'
    $p=Start-Process -FilePath (Get-MihomoExe) -ArgumentList @('-d',('"'+(Join-Path $Root 'mihomo-data')+'"'),'-f',('"'+$s.active_config+'"')) -WorkingDirectory (Join-Path $Root 'tools\mihomo') -WindowStyle Hidden -RedirectStandardOutput (Join-Path $Root 'logs\mihomo.out.log') -RedirectStandardError (Join-Path $Root 'logs\mihomo.err.log') -PassThru
    Save-Process 'mihomo' $p $s.active_config $s.mihomo_port
    for($i=0;$i -lt 50;$i++) {Start-Sleep -Milliseconds 200;$p.Refresh();if($p.HasExited){throw 'Mihomo exited during startup; inspect the runtime component log.'};if(Test-Port $s.mihomo_port){Write-ProgressEvent 100 'Mihomo listener is ready; connectivity is checked separately';return}}
    Stop-OwnedProcess 'mihomo';throw 'Mihomo did not open its listener within 10 seconds.'
}
function Get-TgExe {return (Resolve-InstalledPath 'telegram' (Join-Path $Root 'tools\telegram\TgWsProxy_windows.exe'))}
function Get-TgConfigPath {return (Join-Path (Split-Path -Parent (Get-TgExe)) 'TgWsProxy_data\config.json')}
function Ensure-TgConfig {
    $file=Get-TgConfigPath
    if(Test-Path -LiteralPath $file){return (Get-Content -Encoding UTF8 -LiteralPath $file -Raw | ConvertFrom-Json)}
    $rng=[Security.Cryptography.RandomNumberGenerator]::Create();$bytes=New-Object byte[] 16;try{$rng.GetBytes($bytes)}finally{$rng.Dispose()}
    $s=Read-Settings
    $cfg=[pscustomobject]@{host='127.0.0.1';port=$s.tg_port;secret=([BitConverter]::ToString($bytes).Replace('-','').ToLowerInvariant());dc_ip=@('2:149.154.167.220','4:149.154.167.220');verbose=$false;check_updates=$false;log_max_mb=5;buf_kb=256;pool_size=4;cfproxy=$true;cfproxy_user_domain_enabled=$false;cfproxy_user_domain=@();cfproxy_worker_enabled=$false;cfproxy_worker_domain=@();force_test_dc=$false;no_secure=$false;language='en';autostart=$false}
    Write-AtomicText $file ($cfg | ConvertTo-Json -Depth 8) -Private;return $cfg
}
function Start-Telegram {
    $exe=Get-TgExe
    if(-not(Test-Path -LiteralPath $exe)){throw 'Install TG WS Proxy first.'}
    if(Get-OwnedProcess 'telegram'){throw 'Managed TG WS Proxy is already running.'}
    if(Get-Process -Name 'TgWsProxy_windows','TgWsProxy_windows_arm64' -ErrorAction SilentlyContinue){throw 'An external TG WS Proxy instance is already running. Manage it in its tray; upstream allows one instance per user.'}
    $cfg=Ensure-TgConfig
    if($cfg.host -ne '127.0.0.1'){throw 'URTW managed Telegram profiles must listen on 127.0.0.1.'}
    if(Test-Port $cfg.port){throw 'The Telegram port is occupied by another process.'}
    $s=Read-Settings;$s.tg_port=[int]$cfg.port;Save-Settings $s
    Write-ProgressEvent 50 'Launching the official Telegram tray application'
    $p=Start-Process -FilePath $exe -ArgumentList '--portable' -WorkingDirectory (Split-Path -Parent $exe) -WindowStyle Hidden -PassThru
    Save-Process 'telegram' $p
    for($i=0;$i -lt 75;$i++){Start-Sleep -Milliseconds 200;$p.Refresh();if($p.HasExited){throw 'TG WS Proxy exited during startup; inspect its private log.'};if(Test-Port $cfg.port){Write-ProgressEvent 100 'Local Telegram proxy is listening';return}}
    Stop-OwnedProcess 'telegram';throw 'TG WS Proxy did not open its listener within 15 seconds.'
}
function Get-TelegramLink {
    $file=Get-TgConfigPath;if(-not(Test-Path -LiteralPath $file)){throw 'Start or configure the managed Telegram proxy first.'}
    $cfg=Get-Content -Encoding UTF8 -LiteralPath $file -Raw | ConvertFrom-Json
    if($cfg.host -ne '127.0.0.1' -or $cfg.secret -notmatch '^[a-fA-F0-9]{32}$'){throw 'Unsupported Telegram host or secret. Use the upstream tray connection link for advanced secret formats.'}
    return ('tg://proxy?server=127.0.0.1&port='+$cfg.port+'&secret='+$cfg.secret)
}
function Set-TelegramPort([int]$Value) {
    if(Get-OwnedProcess 'telegram'){throw 'Stop the managed Telegram proxy before changing its port.'}
    if(Test-Port (Read-Settings).tg_port){throw 'The Telegram listener is active; stop it in its tray before editing its port.'}
    if(Test-Port $Value){throw 'The requested local port is occupied.'}
    $cfg=Ensure-TgConfig;$cfg.port=$Value
    Write-AtomicText (Get-TgConfigPath) ($cfg | ConvertTo-Json -Depth 8) -Private
    $s=Read-Settings;$s.tg_port=$Value;Save-Settings $s
}
function Expand-CheckedZip([string]$Archive,[string]$Destination) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip=[IO.Compression.ZipFile]::OpenRead($Archive)
    try {foreach($entry in $zip.Entries){$full=[IO.Path]::GetFullPath((Join-Path $Destination $entry.FullName));if(-not $full.StartsWith($Destination.TrimEnd('\')+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Archive contains an unsafe path.'}}}finally{$zip.Dispose()}
    [IO.Compression.ZipFile]::ExtractToDirectory($Archive,$Destination)
}
function Download-Verified($Asset,[string]$Destination) {
    if($Asset.url -notmatch '^https://(?:github\.com|www\.wintun\.net)/' -or $Asset.sha256 -notmatch '^[a-f0-9]{64}$'){throw 'Invalid pinned download manifest.'}
    [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
    $oldProgress=$ProgressPreference;$ProgressPreference='SilentlyContinue'
    try {Invoke-WebRequest -Uri $Asset.url -OutFile $Destination -UseBasicParsing -TimeoutSec 120}finally{$ProgressPreference=$oldProgress}
    if((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Asset.sha256){throw 'Downloaded package failed SHA-256 verification.'}
}
function Install-Component([string]$Name) {
    if(Get-OwnedProcess $Name){throw 'Stop the managed component before replacing its binary.'}
    $arch=if($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64'){'arm64'}elseif([Environment]::Is64BitOperatingSystem){'amd64'}else{throw 'URTW supports x64 and ARM64 Windows only.'}
    $manifest=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $Assets 'components.lock.json') -Raw | ConvertFrom-Json
    if(-not $manifest.PSObject.Properties[$Name]){throw 'Custom adapters use B on Cores to install a trusted local binary.'}
    $item=$manifest.$Name;$property=$item.PSObject.Properties[$arch]
    if(-not $property){throw 'This component is unavailable for the current Windows architecture.'}
    $target=Join-Path $Root ('tools\'+$Name)
    $activeExe=if($Name -eq 'telegram'){Get-TgExe}elseif($Name -ne 'zapret'){Get-CoreExe $Name}else{Join-Path $target 'bin\winws.exe'}
    if(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and [string]::Equals($_.ExecutablePath,$activeExe,[StringComparison]::OrdinalIgnoreCase)}){throw 'The component executable is running; stop it explicitly before replacing binaries.'}
    $aliasesFile=Join-Path $Root 'installed-paths.json';if(Test-Path -LiteralPath $aliasesFile){$aliases=Get-Content -Encoding UTF8 -LiteralPath $aliasesFile -Raw | ConvertFrom-Json;if($aliases.PSObject.Properties[$Name]){throw 'An existing installation alias is present. Preserve its binaries or install in a new runtime folder.'}}
    if($Name -eq 'zapret' -and (Test-Path -LiteralPath (Join-Path $target 'service.bat'))){throw 'An existing Zapret package is present. Use its upstream manager for updates to preserve custom strategies and lists.'}
    $stage=Assert-RuntimePath (Join-Path $Root ('staging-'+[guid]::NewGuid().ToString('N')))
    New-Item -ItemType Directory -Path $stage | Out-Null
    $prepared=Join-Path $stage 'prepared';New-Item -ItemType Directory -Path $prepared | Out-Null
    try {
        Write-ProgressEvent 10 'Downloading pinned upstream package (size may vary)'
        $archive=Join-Path $stage $(if($Name -eq 'telegram'){'upstream.exe'}else{'upstream.zip'})
        Download-Verified $property.Value $archive
        Write-ProgressEvent 55 'SHA-256 verified; preparing component files'
        if($Name -eq 'telegram') {Copy-Item -LiteralPath $archive -Destination (Join-Path $prepared 'TgWsProxy_windows.exe')}
        else {
            $unpack=Join-Path $stage 'unpack';Expand-CheckedZip $archive $unpack
            if($Name -notin @('telegram','zapret')) {
                $definition=Get-Core $Name
                $exe=Get-ChildItem -LiteralPath $unpack -Recurse -File | Where-Object {$_.Name -match '^mihomo.*\.exe$'} | Select-Object -First 1
                if($Name -ne 'mihomo'){$exe=Get-ChildItem -LiteralPath $unpack -Recurse -File | Where-Object {$_.Name -eq $definition.executable} | Select-Object -First 1}
                if(-not $exe){throw 'Mihomo executable missing from the verified archive.'}
                Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $prepared $definition.executable)
                foreach($vendorFile in (Get-ChildItem -LiteralPath $exe.DirectoryName -File | Where-Object {$_.Name -match '^(LICENSE|COPYING|NOTICE)' -or $_.Extension -in @('.dat','.dll')})){Copy-Item -LiteralPath $vendorFile.FullName -Destination $prepared}
                if($Name -eq 'mihomo'){
                Write-ProgressEvent 70 'Downloading the pinned signed Wintun driver'
                $wzip=Join-Path $stage 'wintun.zip';Download-Verified $item.wintun $wzip
                $wdir=Join-Path $stage 'wintun';Expand-CheckedZip $wzip $wdir
                Copy-Item -LiteralPath (Join-Path $wdir ('wintun\bin\'+$arch+'\wintun.dll')) -Destination (Join-Path $prepared 'wintun.dll')
                $license=Join-Path $wdir 'wintun\LICENSE.txt';if(Test-Path -LiteralPath $license){Copy-Item -LiteralPath $license -Destination (Join-Path $prepared 'WINTUN-LICENSE.txt')}
                }
            } else {
                $manager=Get-ChildItem -LiteralPath $unpack -Recurse -Filter service.bat -File | Select-Object -First 1
                if(-not $manager){throw 'service.bat missing from the verified archive.'}
                Get-ChildItem -LiteralPath $manager.DirectoryName -Force | Copy-Item -Destination $prepared -Recurse
            }
        }
        Write-ProgressEvent 90 'All downloads verified; committing prepared component files'
        if(-not(Test-Path -LiteralPath $target)){New-Item -ItemType Directory -Path $target | Out-Null}
        foreach($entry in (Get-ChildItem -LiteralPath $prepared -Force)) {
            if($entry.PSIsContainer){Copy-Item -LiteralPath $entry.FullName -Destination $target -Recurse}
            else {
                $destination=Assert-RuntimePath (Join-Path $target $entry.Name);$new=$destination+'.new-'+[guid]::NewGuid().ToString('N')
                try {Copy-Item -LiteralPath $entry.FullName -Destination $new;if(Test-Path -LiteralPath $destination){[IO.File]::Replace($new,$destination,[System.Management.Automation.Language.NullString]::Value)}else{[IO.File]::Move($new,$destination)}}finally{if(Test-Path -LiteralPath $new){Remove-Item -LiteralPath $new -Force}}
            }
        }
        $versionsFile=Join-Path $Root 'installed.json';$versions=@{}
        if(Test-Path -LiteralPath $versionsFile){$v=Get-Content -Encoding UTF8 -LiteralPath $versionsFile -Raw | ConvertFrom-Json;foreach($p in $v.PSObject.Properties){$versions[$p.Name]=$p.Value}}
        $versions[$Name]=$item.version;Write-AtomicText $versionsFile ($versions | ConvertTo-Json) -Private
        Write-ProgressEvent 100 'Installed verified component; startup remains a separate action'
    } finally {if(Test-Path -LiteralPath $stage){$checked=Assert-RuntimePath $stage;Remove-Item -LiteralPath $checked -Recurse -Force}}
}
function Notify-ProxyChange {
    if(-not ('UrtWinInet' -as [type])) {Add-Type 'using System; using System.Runtime.InteropServices; public static class UrtWinInet { [DllImport("wininet.dll")] public static extern bool InternetSetOption(IntPtr h, int option, IntPtr buffer, int length); }'}
    [UrtWinInet]::InternetSetOption([IntPtr]::Zero,39,[IntPtr]::Zero,0) | Out-Null
    [UrtWinInet]::InternetSetOption([IntPtr]::Zero,37,[IntPtr]::Zero,0) | Out-Null
}
function Get-ProxyValues($RegistryProperties) {
    $values=@{}
    foreach($name in @('ProxyEnable','ProxyServer','ProxyOverride','AutoConfigURL')){$p=$RegistryProperties.PSObject.Properties[$name];$values[$name]=@{exists=[bool]$p;value=$(if($p){$p.Value}else{$null})}}
    return $values
}
function Assert-ProxyOwnership($Current,$Applied) {
    $now=Get-ProxyValues $Current
    foreach($property in $Applied.PSObject.Properties) {
        $expected=$property.Value;$actual=$now[$property.Name]
        if($actual.exists -ne $expected.exists -or ($expected.exists -and $actual.value -ne $expected.value)){throw 'Windows proxy was changed externally; refusing to overwrite the new configuration.'}
    }
}
function Assert-ProxyListenerDetached($Current,[int]$ListenPort) {
    $values=Get-ProxyValues $Current
    if($ListenPort -le 0 -or -not $values.ProxyEnable.exists -or $values.ProxyEnable.value -ne 1 -or -not $values.ProxyServer.exists){return}
    foreach($mapping in ([string]$values.ProxyServer.value -split '[;\s]+')){
        $address=($mapping -split '=')[-1]
        if($address -match '^(?:(?:https?|socks[45]?|ftp)://)?(?:127\.0\.0\.1|localhost|\[::1\]):([0-9]+)$' -and [int]$matches[1] -eq $ListenPort){
            throw 'Windows Proxy still uses this listener. Detach it in its original manager or Windows settings before stopping the core; the core remains running.'
        }
    }
}
function Stop-ManagedCore([string]$Name) {
    $backupFile=Join-Path $Root 'proxy-backup.json'
    if(Test-Path -LiteralPath $backupFile){
        $backup=Get-Content -Encoding UTF8 -LiteralPath $backupFile -Raw | ConvertFrom-Json
        if((Get-OptionalValue $backup 'core' 'mihomo') -eq $Name){Restore-SystemProxy}
    }
    Stop-OwnedProcess $Name
}
function Apply-ProxyValues([string]$Key,$Values) {
    foreach($p in $Values.PSObject.Properties){if($p.Value.exists){$type=if($p.Name -eq 'ProxyEnable'){'DWord'}else{'String'};New-ItemProperty -LiteralPath $Key -Name $p.Name -Value $p.Value.value -PropertyType $type -Force | Out-Null}else{Remove-ItemProperty -LiteralPath $Key -Name $p.Name -ErrorAction SilentlyContinue}}
}
function Enable-SystemProxy {
    $s=Read-Settings;if(-not(Get-OwnedProcess 'mihomo')){throw 'Start the managed Mihomo listener before enabling the Windows proxy.'}
    $record=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $Root 'mihomo.pid.json') -Raw | ConvertFrom-Json
    Assert-RunningMihomoSelection $s $record
    if(-not(Test-Port $record.port)){throw 'The managed Mihomo listener is unavailable.'}
    $key='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings';$backupFile=Join-Path $Root 'proxy-backup.json'
    if(Test-Path -LiteralPath $backupFile){throw 'A proxy restore snapshot already exists; restore it before enabling again.'}
    $old=Get-ItemProperty -LiteralPath $key;$values=Get-ProxyValues $old
    $applied=[pscustomobject]@{ProxyEnable=[pscustomobject]@{exists=$true;value=1};ProxyServer=[pscustomobject]@{exists=$true;value=('127.0.0.1:'+$s.mihomo_port)};ProxyOverride=[pscustomobject]@{exists=$true;value='<local>;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;*.local'};AutoConfigURL=[pscustomobject]@{exists=$false;value=$null}}
    $backup=[pscustomobject]@{values=[pscustomobject]$values;applied=$applied};Write-AtomicText $backupFile ($backup | ConvertTo-Json -Depth 8) -Private
    try {Apply-ProxyValues $key $applied;Notify-ProxyChange} catch {Apply-ProxyValues $key $backup.values;Notify-ProxyChange;Remove-Item -LiteralPath $backupFile -Force;throw}
}
function Restore-SystemProxy {
    $backupFile=Join-Path $Root 'proxy-backup.json';if(-not(Test-Path -LiteralPath $backupFile)){throw 'This runtime has no Windows proxy snapshot; external proxy settings are left to their owner.'}
    $backup=Get-Content -Encoding UTF8 -LiteralPath $backupFile -Raw | ConvertFrom-Json;$key='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
    $current=Get-ItemProperty -LiteralPath $key
    Assert-ProxyOwnership $current $backup.applied
    Apply-ProxyValues $key $backup.values
    Notify-ProxyChange;Remove-Item -LiteralPath (Assert-RuntimePath $backupFile) -Force
}

. (Join-Path $Assets 'modules\workspace.ps1')
if($Command -eq 'None'){return}
try {
    if($Command -ne 'Status'){Initialize-Runtime}
    switch($Command) {
        'Status' {Read-Settings | ConvertTo-Json -Depth 8}
        'SelectConfig' {Select-Configuration $Path}
        'Install' {if(-not $Component){throw 'Specify a component.'};Install-Component $Component}
        'StartMihomo' {Start-Mihomo}
        'StopMihomo' {Stop-ManagedCore 'mihomo'}
        'StartTG' {Start-Telegram}
        'StopTG' {Stop-OwnedProcess 'telegram'}
        'SetTGPort' {Set-TelegramPort $Port}
        'CopyTGLink' {Set-Clipboard -Value (Get-TelegramLink)}
        'OpenTelegram' {Start-Process -FilePath (Get-TelegramLink) | Out-Null}
        'EditTG' {$null=Ensure-TgConfig;Start-Process -FilePath notepad.exe -ArgumentList ('"'+(Get-TgConfigPath)+'"') | Out-Null}
        'OpenZapret' {
            $manager=Join-Path $Root 'tools\zapret\service.bat';if(-not(Test-Path -LiteralPath $manager)){throw 'Install Zapret first.'}
            Start-Process -FilePath cmd.exe -ArgumentList ('/d /c call "'+$manager+'"') -WorkingDirectory (Split-Path -Parent $manager) -WindowStyle Normal | Out-Null
        }
        'EnableProxy' {Enable-SystemProxy}
        'DisableProxy' {Restore-SystemProxy}
        'Boot' {$w=Read-Workspace;if(-not(Get-OwnedProcess $w.active_core)){Start-SelectedCore $w.active_core}}
        'Request' { $requestText=[Console]::In.ReadToEnd();if($requestText.Length -gt 1048576){throw 'Workspace request is too large.'};try{$request=$requestText | ConvertFrom-Json}catch{throw 'Invalid workspace request.'};Invoke-WorkspaceRequest $request }
    }
} catch {
    [Console]::Error.WriteLine('URT_ERROR|'+$_.Exception.Message)
    exit 1
}
