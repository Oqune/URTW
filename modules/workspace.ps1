# Private workspace and native core adapters. Loaded by routing.ps1.
function Read-CoreCatalog {
    $catalog=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $Assets 'config\cores.json') -Raw | ConvertFrom-Json
    $items=@($catalog.cores)
    $custom=Join-Path $Root 'core-adapters'
    if(Test-Path -LiteralPath $custom){foreach($file in (Get-ChildItem -LiteralPath $custom -Filter *.json -File)){
        $adapter=Get-Content -Encoding UTF8 -LiteralPath $file.FullName -Raw | ConvertFrom-Json
        Assert-CoreAdapter $adapter
        if($items.id -contains $adapter.id){throw 'A custom adapter cannot replace a built-in core.'}
        $items+=@($adapter)
    }}
    return $items
}
function Assert-CoreAdapter($Adapter) {
    if($Adapter.id -notmatch '^[a-z][a-z0-9_-]{1,31}$' -or $Adapter.executable -notmatch '^[A-Za-z0-9_.-]+\.exe$'){throw 'Invalid core identifier or executable filename.'}
    if($Adapter.format -notin @('json','yaml') -or $Adapter.default_port -lt 1024 -or $Adapter.default_port -gt 65535){throw 'Invalid core format or local port.'}
    foreach($name in @('name','author','license','source','start_args','validate_args','tun','process_rules')){if(-not $Adapter.PSObject.Properties[$name]){throw 'Core adapter is missing required metadata.'}}
    if($Adapter.source -notmatch '^https://'){throw 'Core source must be an HTTPS project link.'}
    foreach($argument in @($Adapter.start_args)+@($Adapter.validate_args)){if($argument -isnot [string] -or $argument.Length -gt 512 -or $argument -match '[\r\n\x00]'){throw 'Invalid direct-process argument in core adapter.'}}
}
function Get-Core([string]$Id) {
    $core=@(Read-CoreCatalog | Where-Object {$_.id -eq $Id})
    if($core.Count -ne 1){throw 'Unknown core. Select an installed catalog adapter.'}
    return $core[0]
}
function Resolve-InstalledPath([string]$Id,[string]$Default) { $file=Join-Path $Root 'installed-paths.json';if(Test-Path -LiteralPath $file){$aliases=Get-Content -Encoding UTF8 -LiteralPath $file -Raw | ConvertFrom-Json;$p=$aliases.PSObject.Properties[$Id];if($p){return (Assert-RuntimePath $p.Value)}};return $Default }
function Get-CoreExe([string]$Id) { $core=Get-Core $Id; return (Resolve-InstalledPath $Id (Join-Path $Root ('tools\'+$Id+'\'+$core.executable))) }
function Get-CoreData([string]$Id) { $file=Join-Path $Root 'installed-data.json';if(Test-Path -LiteralPath $file){$aliases=Get-Content -Encoding UTF8 -LiteralPath $file -Raw | ConvertFrom-Json;$p=$aliases.PSObject.Properties[$Id];if($p){return (Assert-RuntimePath $p.Value)}};return (Join-Path $Root ('core-data\'+$Id)) }
function Read-Workspace {
    $path=Join-Path $Root 'workspace.json'
    if(Test-Path -LiteralPath $path){$w=Get-Content -Encoding UTF8 -LiteralPath $path -Raw | ConvertFrom-Json;if($w.schema_version -ne 1){throw 'Unsupported workspace schema.'};return $w}
    $w=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $Assets 'config\workspace-defaults.json') -Raw | ConvertFrom-Json
    $legacy=Read-Settings
    $index=0;foreach($path in @($legacy.profiles)){
        $id='legacy-'+$index;$index++;$w.profiles+=@([pscustomobject]@{id=$id;name=[IO.Path]::GetFileName($path);core='mihomo';path=$path;port=$legacy.mihomo_port;validation='pending';generated=$false})
        if($path -eq $legacy.active_config){$w.selected_profiles | Add-Member -NotePropertyName mihomo -NotePropertyValue $id -Force}
    }
    return $w
}
function Save-Workspace($Workspace) {Write-AtomicText (Join-Path $Root 'workspace.json') ($Workspace | ConvertTo-Json -Depth 40) -Private}
function Get-CoreOptions($Workspace,[string]$Id) {
    $p=$Workspace.core_options.PSObject.Properties[$Id]
    if($p){return $p.Value}
    $core=Get-Core $Id
    return [pscustomobject]@{port=$core.default_port;dns=@('1.1.1.1');dns_strategy='prefer_ipv4';ipv6=$false;tun=$false;log_level='warning'}
}
function Get-SelectedProfile($Workspace,[string]$Id) {
    $selected=$Workspace.selected_profiles.PSObject.Properties[$Id]
    if(-not $selected){throw 'Choose or generate a profile for the selected core first.'}
    $profile=@($Workspace.profiles | Where-Object {$_.id -eq $selected.Value -and $_.core -eq $Id})
    if($profile.Count -ne 1){throw 'Selected profile is missing from the workspace.'}
    return $profile[0]
}
function Expand-CoreArguments($Arguments,[string]$Id,[string]$Configuration) {
    $data=Get-CoreData $Id
    return @($Arguments | ForEach-Object {$_.Replace('{data}',$data).Replace('{config}',$Configuration).Replace('{root}',$Root)})
}
function Test-CoreConfiguration([string]$Id,[string]$Configuration) {
    $core=Get-Core $Id;$exe=Get-CoreExe $Id
    if(-not(Test-Path -LiteralPath $exe)){return 'pending'}
    $data=Get-CoreData $Id;if(-not(Test-Path -LiteralPath $data)){New-Item -ItemType Directory -Path $data -Force | Out-Null;Protect-PrivateDirectory $data}
    $arguments=Expand-CoreArguments $core.validate_args $Id $Configuration
    Write-ProgressEvent 45 'Checking configuration with the selected native core'
    $savedPreference=$ErrorActionPreference
    try{$ErrorActionPreference='Continue';$output=& $exe @arguments 2>&1;$code=$LASTEXITCODE}finally{$ErrorActionPreference=$savedPreference}
    if($code -ne 0){throw 'Native core rejected this profile. Review its private configuration; URTW does not print credentials.'}
    return 'valid'
}
function Get-NativeProfilePort([string]$Id,[string]$Configuration) {
    $core=Get-Core $Id
    if($core.format -eq 'yaml'){return (Get-MihomoMixedPort $Configuration)}
    try{$native=Get-Content -Encoding UTF8 -LiteralPath $Configuration -Raw | ConvertFrom-Json}catch{throw 'Invalid JSON profile.'}
    $inbounds=@($native.inbounds | Where-Object {
        ($_.PSObject.Properties['type'] -and $_.type -in @('mixed','http')) -or ($_.PSObject.Properties['protocol'] -and $_.protocol -eq 'http')
    })
    if($inbounds.Count -eq 0){throw 'The profile requires a loopback HTTP or mixed inbound for diagnostics and Windows Proxy.'}
    $inbound=$inbounds[0]
    if($inbound.listen -notin @('127.0.0.1','localhost','::1')){throw 'Managed native HTTP inbounds must listen on loopback.'}
    $port=if($inbound.PSObject.Properties['listen_port']){[int]$inbound.listen_port}else{[int]$inbound.port}
    if($port -lt 1024 -or $port -gt 65535){throw 'Managed HTTP port must be between 1024 and 65535.'}
    return $port
}
function Select-CoreProfile([string]$Id,[string]$Source) {
    $core=Get-Core $Id;$file=Get-Item -LiteralPath $Source -ErrorAction Stop
    if($file.PSIsContainer){throw 'Select a configuration file.'}
    if($file.Extension -eq '.conf'){
        $w=Read-Workspace;$endpoint=New-WireGuardEndpoint $file.FullName;$w.endpoints+=@($endpoint);$w.active_endpoint=$endpoint.id;Save-Workspace $w
        New-GeneratedProfile $Id
        return
    }
    if(($core.format -eq 'json' -and $file.Extension -ne '.json') -or ($core.format -eq 'yaml' -and $file.Extension -notin @('.yaml','.yml'))){throw 'The selected file format does not match this core.'}
    $port=Get-NativeProfilePort $Id $file.FullName;$validation=Test-CoreConfiguration $Id $file.FullName
    $w=Read-Workspace;$profile=@($w.profiles | Where-Object {$_.core -eq $Id -and $_.path -eq $file.FullName}) | Select-Object -First 1
    if(-not $profile){$profile=[pscustomobject]@{id=[guid]::NewGuid().ToString('N');name=$file.Name;core=$Id;path=$file.FullName;port=$port;validation=$validation;generated=$false};$w.profiles+=@($profile)}else{$profile.port=$port;$profile.validation=$validation}
    $w.active_core=$Id;$w.selected_profiles | Add-Member -NotePropertyName $Id -NotePropertyValue $profile.id -Force;Save-Workspace $w
    Write-ProgressEvent 100 $(if($validation -eq 'pending'){'Selected; install the core before native validation and startup'}else{'Profile validated and selected; startup remains a separate action'})
}
function Quote-CoreArgument([string]$Argument) {
    if($Argument -notmatch '[\s"]'){return $Argument}
    return ('"'+[regex]::Replace([regex]::Replace($Argument,'(\\*)"','$1$1\"'),'(\\+)$','$1$1')+'"')
}
function Start-SelectedCore([string]$Id) {
    if(Get-OwnedProcess $Id){throw 'This managed core is already running. Stop it before applying another profile.'}
    $w=Read-Workspace;$profile=Get-SelectedProfile $w $Id;$core=Get-Core $Id;$exe=Get-CoreExe $Id
    if(-not(Test-Path -LiteralPath $exe)){throw 'Install the selected core first.'}
    if((Get-NativeProfilePort $Id $profile.path) -ne $profile.port){throw 'The profile port changed; select it again before starting.'}
    $profile.validation=Test-CoreConfiguration $Id $profile.path;Save-Workspace $w
    if(Test-Port $profile.port){throw 'The profile local port is already occupied.'}
    $text=Get-Content -Encoding UTF8 -LiteralPath $profile.path -Raw
    $hasTun=if($core.format -eq 'yaml'){[regex]::Match($text,'(?ms)^tun:\s*(?:\{[^\r\n]*\}|\r?\n(?:[ \t]+[^\r\n]*\r?\n?)*)').Value -match '(?i)enable:\s*true'}else{$text -match '"type"\s*:\s*"tun"|"protocol"\s*:\s*"tun"'}
    if($hasTun -and -not([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'This profile enables TUN. Start URTW in an administrator terminal.'}
    $args=Expand-CoreArguments $core.start_args $Id $profile.path;$commandLine=(@($args | ForEach-Object {Quote-CoreArgument $_}) -join ' ')
    Write-ProgressEvent 65 'Starting the selected native core'
    $p=Start-Process -FilePath $exe -ArgumentList $commandLine -WorkingDirectory (Split-Path -Parent $exe) -WindowStyle Hidden -RedirectStandardOutput (Join-Path $Root ('logs\'+$Id+'.out.log')) -RedirectStandardError (Join-Path $Root ('logs\'+$Id+'.err.log')) -PassThru
    Save-Process $Id $p $profile.path $profile.port
    for($i=0;$i -lt 75;$i++){Start-Sleep -Milliseconds 200;$p.Refresh();if($p.HasExited){throw 'Core exited during startup. Open its private runtime log.'};if(Test-Port $profile.port){Write-ProgressEvent 100 'Managed core is listening; network access is measured separately';return}}
    Stop-OwnedProcess $Id;throw 'Core listener was not ready within 15 seconds.'
}
function New-WireGuardEndpoint([string]$Source) {
    $wg=Read-WireGuard $Source
    return [pscustomobject]@{id=[guid]::NewGuid().ToString('N');name=[IO.Path]::GetFileNameWithoutExtension($Source);type='wireguard';server=$wg.Server;port=$wg.Port;data=[pscustomobject]$wg}
}
function Get-OptionalValue($Object,[string]$Name,$Default=$null) { $p=$Object.PSObject.Properties[$Name];if($p){return $p.Value};return $Default }
function New-LinkEndpoint([string]$Link) {
    $uri=$null;if(-not[Uri]::TryCreate($Link,[UriKind]::Absolute,[ref]$uri)){throw 'Enter a complete proxy share link.'}
    $type=$uri.Scheme.ToLowerInvariant();if($type -notin @('socks5','http','https','vless','trojan')){throw 'Supported links: SOCKS5, HTTP(S), VLESS and Trojan. Import WireGuard from a .conf file.'}
    if(-not $uri.Host -or $uri.Port -lt 1 -or $uri.Port -gt 65535){throw 'The link needs a server and explicit valid port.'}
    $query=@{};foreach($part in $uri.Query.TrimStart('?') -split '&'){if($part){$pair=$part -split '=',2;$query[[Uri]::UnescapeDataString($pair[0])]=if($pair.Count -eq 2){[Uri]::UnescapeDataString($pair[1])}else{''}}}
    $credentials=@($uri.UserInfo -split ':',2 | ForEach-Object {[Uri]::UnescapeDataString($_)})
    $username=if($credentials.Count){$credentials[0]}else{''};$password=if($credentials.Count -eq 2){$credentials[1]}else{''}
    if($type -eq 'vless'){$guid=[guid]::Empty;if(-not[guid]::TryParse($username,[ref]$guid)){throw 'VLESS links require a valid UUID.'}}
    if($type -eq 'trojan' -and -not $username){throw 'Trojan links require a password.'}
    $transport=if($query.ContainsKey('type')){$query.type}else{'tcp'};if($transport -notin @('tcp','ws','grpc')){throw 'This link transport needs a native profile; generated links support TCP, WS and gRPC.'}
    $security=if($query.ContainsKey('security')){$query.security}elseif($type -in @('trojan','https')){'tls'}else{'none'}
    if($security -notin @('none','tls','reality')){throw 'Unsupported link security mode.'}
    if($type -eq 'vless' -and $security -eq 'none'){throw 'Generated VLESS profiles require TLS or REALITY; use a trusted native profile for private plain links.'}
    if($security -eq 'reality' -and (-not $query.ContainsKey('pbk') -or $query.pbk -notmatch '^[A-Za-z0-9_-]{43}$')){throw 'REALITY requires a valid public key.'}
    $name=[Uri]::UnescapeDataString($uri.Fragment.TrimStart('#'));if(-not $name){$name=$type+' / '+$uri.Host}
    if($name.Length -gt 80 -or $name -match '[\x00-\x1f]'){throw 'Endpoint name is too long or contains control characters.'}
    return [pscustomobject]@{id=[guid]::NewGuid().ToString('N');name=$name;type=$type;server=$uri.Host;port=$uri.Port;data=[pscustomobject]@{username=$username;password=$password;query=[pscustomobject]$query;transport=$transport;security=$security}}
}
function Assert-Domain([string]$Value){if($Value -notmatch '^(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?\.)*[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?$'){throw 'Enter a plain domain suffix without a URL or wildcard.'}}
function Assert-Standard($Standard) {
    if($Standard.default -notin @('direct','proxy','block')){throw 'Unknown default route action.'}
    foreach($domain in @($Standard.browser_domains)){Assert-Domain $domain}
    foreach($rule in @($Standard.rules)){
        if($rule.action -notin @('direct','proxy','block')){throw 'Unknown route action.'}
        switch($rule.kind){
            domain {Assert-Domain $rule.value}
            process {if($rule.value -notmatch '^[A-Za-z0-9_.-]+\.exe$'){throw 'Process rules require an executable basename.'}}
            ip {$parts=$rule.value -split '/',2;$address=$null;$prefix=0;if($parts.Count -ne 2 -or -not[Net.IPAddress]::TryParse($parts[0],[ref]$address) -or -not[int]::TryParse($parts[1],[ref]$prefix) -or $prefix -lt 0 -or $prefix -gt $(if($address.AddressFamily -eq 'InterNetwork'){32}else{128})){throw 'Invalid IP/CIDR route rule.'}}
            default {throw 'Rule type must be domain, process or ip.'}
        }
    }
}
function Assert-CoreOptions($Options,$Core) {
    if($Options.port -lt 1024 -or $Options.port -gt 65535){throw 'Client port must be 1024..65535.'}
    if($Options.tun -and -not $Core.tun){throw 'This adapter has no generated TUN profile support.'}
    if($Options.dns_strategy -notin @('prefer_ipv4','prefer_ipv6','ipv4_only','ipv6_only')){throw 'Unsupported DNS strategy.'}
    if($Options.log_level -notin @('debug','info','warn','warning','error')){throw 'Unsupported client log level.'}
    if(@($Options.dns).Count -eq 0){throw 'At least one client DNS resolver is required.'}
    foreach($resolver in @($Options.dns)){
        $address=$null;if([Net.IPAddress]::TryParse($resolver,[ref]$address)){continue}
        $uri=$null;if(-not[Uri]::TryCreate($resolver,[UriKind]::Absolute,[ref]$uri) -or $uri.Scheme -notin @('https','tls') -or $uri.UserInfo -or -not $uri.Host){throw 'DNS entries must be IP addresses, HTTPS DoH or TLS DoT URLs.'}
    }
}
function Get-WorkspaceStandard($Workspace){$s=@($Workspace.standards | Where-Object {$_.id -eq $Workspace.active_standard});if($s.Count -ne 1){throw 'Select a routing standard.'};Assert-Standard $s[0];return $s[0]}
function Get-WorkspaceEndpoint($Workspace){$e=@($Workspace.endpoints | Where-Object {$_.id -eq $Workspace.active_endpoint});if($e.Count -ne 1){return $null};return $e[0]}
function Route-Tag([string]$Action){switch($Action){direct {'DIRECT'}proxy {'PROXY'}block {'BLOCK'}default {throw 'Unknown route action.'}}}
function Get-LinkTls($Endpoint) {
    $q=$Endpoint.data.query;$serverName=Get-OptionalValue $q 'sni' $Endpoint.server
    return @{enabled=($Endpoint.data.security -ne 'none');server_name=$serverName;insecure=$false}
}
function New-MihomoOutbound($Endpoint) {
    if(-not $Endpoint){return $null}
    if($Endpoint.type -eq 'wireguard'){
        $p=$Endpoint.data;$value=@{name='PROXY';type='wireguard';server=$Endpoint.server;port=$Endpoint.port;ip=$p.Ip;'private-key'=$p.PrivateKey;'public-key'=$p.PublicKey;udp=$true;'allowed-ips'=@($p.AllowedIPs -split ',' | ForEach-Object {$_.Trim()})}
        if($p.Ipv6){$value.ipv6=$p.Ipv6};$psk=Get-OptionalValue $p 'PresharedKey';if($psk){$value['pre-shared-key']=$psk};$value.mtu=[int](Get-OptionalValue $p 'MTU' 1420);$value['persistent-keepalive']=[int](Get-OptionalValue $p 'PersistentKeepalive' 25);return $value
    }
    $type=if($Endpoint.type -eq 'https'){'http'}else{$Endpoint.type};$v=@{name='PROXY';type=$type;server=$Endpoint.server;port=$Endpoint.port};$d=$Endpoint.data;$q=$d.query
    if($type -in @('socks5','http')){if($d.username){$v.username=$d.username;$v.password=$d.password};if($Endpoint.type -eq 'https'){$v.tls=$true};return $v}
    if($type -eq 'vless'){$v.uuid=$d.username;$v.udp=$true;$flow=Get-OptionalValue $q 'flow';if($flow){$v.flow=$flow}}else{$v.password=$d.username}
    $v.tls=$true;$v.servername=Get-OptionalValue $q 'sni' $Endpoint.server;$v['skip-cert-verify']=$false
    if($d.security -eq 'reality'){$v['reality-opts']=@{'public-key'=$q.pbk;'short-id'=(Get-OptionalValue $q 'sid' '')};$v['client-fingerprint']=Get-OptionalValue $q 'fp' 'chrome'}
    if($d.transport -eq 'ws'){$v.network='ws';$v['ws-opts']=@{path=(Get-OptionalValue $q 'path' '/');headers=@{Host=(Get-OptionalValue $q 'host' $Endpoint.server)}}}
    if($d.transport -eq 'grpc'){$v.network='grpc';$v['grpc-opts']=@{'grpc-service-name'=(Get-OptionalValue $q 'serviceName' '')}}
    return $v
}
function New-SingboxOutbound($Endpoint) {
    if(-not $Endpoint){return $null}
    if($Endpoint.type -eq 'wireguard'){
        $p=$Endpoint.data;$peer=@{address=$Endpoint.server;port=$Endpoint.port;public_key=$p.PublicKey;allowed_ips=@($p.AllowedIPs -split ',' | ForEach-Object {$_.Trim()});persistent_keepalive_interval=[int](Get-OptionalValue $p 'PersistentKeepalive' 25)}
        $psk=Get-OptionalValue $p 'PresharedKey';if($psk){$peer.pre_shared_key=$psk}
        return @{type='wireguard';tag='PROXY';system=$false;address=@($p.Address -split ',' | ForEach-Object {$_.Trim()});private_key=$p.PrivateKey;mtu=[int](Get-OptionalValue $p 'MTU' 1420);peers=@($peer)}
    }
    $type=switch($Endpoint.type){socks5 {'socks'}https {'http'}default {$Endpoint.type}};$v=@{type=$type;tag='PROXY';server=$Endpoint.server;server_port=$Endpoint.port};$d=$Endpoint.data;$q=$d.query
    if($type -in @('socks','http')){if($d.username){$v.username=$d.username;$v.password=$d.password};if($Endpoint.type -eq 'https'){$v.tls=Get-LinkTls $Endpoint};return $v}
    if($type -eq 'vless'){$v.uuid=$d.username;$flow=Get-OptionalValue $q 'flow';if($flow){$v.flow=$flow}}else{$v.password=$d.username}
    $v.tls=Get-LinkTls $Endpoint
    if($d.security -eq 'reality'){$v.tls.reality=@{enabled=$true;public_key=$q.pbk;short_id=(Get-OptionalValue $q 'sid' '')};$v.tls.utls=@{enabled=$true;fingerprint=(Get-OptionalValue $q 'fp' 'chrome')}}
    if($d.transport -eq 'ws'){$v.transport=@{type='ws';path=(Get-OptionalValue $q 'path' '/');headers=@{Host=(Get-OptionalValue $q 'host' $Endpoint.server)}}}
    if($d.transport -eq 'grpc'){$v.transport=@{type='grpc';service_name=(Get-OptionalValue $q 'serviceName' '')}}
    return $v
}
function New-XrayOutbound($Endpoint) {
    if(-not $Endpoint){return $null}
    if($Endpoint.type -eq 'wireguard'){
        $p=$Endpoint.data;$server=if($Endpoint.server.Contains(':')){'['+$Endpoint.server+']'}else{$Endpoint.server};$peer=@{endpoint=($server+':'+$Endpoint.port);publicKey=$p.PublicKey;allowedIPs=@($p.AllowedIPs -split ',' | ForEach-Object {$_.Trim()});keepAlive=[int](Get-OptionalValue $p 'PersistentKeepalive' 25)}
        $psk=Get-OptionalValue $p 'PresharedKey';if($psk){$peer.preSharedKey=$psk}
        return @{tag='PROXY';protocol='wireguard';settings=@{secretKey=$p.PrivateKey;address=@(@($p.Ip)+@($p.Ipv6) | Where-Object {$_});peers=@($peer);noKernelTun=$true;mtu=[int](Get-OptionalValue $p 'MTU' 1420)}}
    }
    $type=switch($Endpoint.type){socks5 {'socks'}https {'http'}default {$Endpoint.type}};$d=$Endpoint.data;$q=$d.query;$v=@{tag='PROXY';protocol=$type}
    if($type -in @('socks','http')){$server=@{address=$Endpoint.server;port=$Endpoint.port};if($d.username){$server.users=@(@{user=$d.username;pass=$d.password})};$v.settings=@{servers=@($server)};if($Endpoint.type -eq 'https'){$v.streamSettings=@{security='tls';tlsSettings=@{serverName=$Endpoint.server}}};return $v}
    if($type -eq 'vless'){$user=@{id=$d.username;encryption='none'};$flow=Get-OptionalValue $q 'flow';if($flow){$user.flow=$flow};$v.settings=@{vnext=@(@{address=$Endpoint.server;port=$Endpoint.port;users=@($user)})}}else{$v.settings=@{servers=@(@{address=$Endpoint.server;port=$Endpoint.port;password=$d.username})}}
    $v.streamSettings=@{network=$d.transport;security=$d.security}
    if($d.security -eq 'reality'){$v.streamSettings.realitySettings=@{serverName=(Get-OptionalValue $q 'sni' $Endpoint.server);fingerprint=(Get-OptionalValue $q 'fp' 'chrome');publicKey=$q.pbk;shortId=(Get-OptionalValue $q 'sid' '')}}else{$v.streamSettings.tlsSettings=@{serverName=(Get-OptionalValue $q 'sni' $Endpoint.server);allowInsecure=$false}}
    if($d.transport -eq 'ws'){$v.streamSettings.wsSettings=@{path=(Get-OptionalValue $q 'path' '/');headers=@{Host=(Get-OptionalValue $q 'host' $Endpoint.server)}}}
    if($d.transport -eq 'grpc'){$v.streamSettings.grpcSettings=@{serviceName=(Get-OptionalValue $q 'serviceName' '')}}
    return $v
}
function Convert-ToYaml($Object,[int]$Indent=0) {
    # JSON scalars and flow arrays are a strict YAML subset; maps retain nesting.
    $prefix=' ' * $Indent;$lines=New-Object 'Collections.Generic.List[string]'
    foreach($key in $Object.Keys){$value=$Object[$key];if($value -is [Collections.IDictionary]){$lines.Add($prefix+$key+':');$lines.Add((Convert-ToYaml $value ($Indent+2)))}else{$lines.Add($prefix+$key+': '+(ConvertTo-Json -InputObject $value -Depth 30 -Compress))}}
    return $lines -join "`n"
}
function New-ClientDns($Options,[string]$Id) {
    if(-not $Options.ipv6 -and $Options.dns_strategy -in @('prefer_ipv6','ipv6_only')){throw 'Enable IPv6 before choosing an IPv6 DNS strategy.'}
    if($Id -eq 'mihomo' -and $Options.dns_strategy -in @('prefer_ipv6','ipv6_only')){throw 'Mihomo generator supports IPv4-only or its native default family choice; advanced DNS policies use a native YAML.'}
    if($Id -eq 'mihomo'){return @{enable=$true;listen='127.0.0.1:0';'enhanced-mode'='redir-host';'default-nameserver'=@('1.1.1.1','8.8.8.8');nameserver=@($Options.dns);'proxy-server-nameserver'=@('1.1.1.1','8.8.8.8');ipv6=([bool]$Options.ipv6 -and $Options.dns_strategy -ne 'ipv4_only')}}
    if($Id -eq 'xray'){if($Options.dns_strategy -eq 'prefer_ipv6'){throw 'Xray supports UseIP, IPv4-only and IPv6-only; select ipv6_only or use a native DNS policy.'};if(@($Options.dns | Where-Object {$_ -match '^tls://'}).Count){throw 'Xray generator accepts DNS IP addresses and HTTPS DoH; use a native profile for other DNS transports.'};return @{servers=@($Options.dns);queryStrategy=$(if(-not $Options.ipv6){'UseIPv4'}else{switch($Options.dns_strategy){ipv4_only {'UseIPv4'}ipv6_only {'UseIPv6'}default {'UseIP'}}})}}
    $servers=@(@{type='udp';tag='bootstrap';server='1.1.1.1'})
    $index=0;foreach($resolver in @($Options.dns)){
        $address=$null;if([Net.IPAddress]::TryParse($resolver,[ref]$address)){$server=@{type='udp';tag=('dns-'+$index);server=$resolver}}
        else{$uri=[Uri]$resolver;$server=@{type=$(if($uri.Scheme -eq 'https'){'https'}else{'tls'});tag=('dns-'+$index);server=$uri.Host;server_port=$(if($uri.Port -gt 0){$uri.Port}elseif($uri.Scheme -eq 'https'){443}else{853});domain_resolver='bootstrap';tls=@{enabled=$true;server_name=$uri.Host}};if($uri.Scheme -eq 'https'){$server.path=$uri.AbsolutePath}}
        $servers+=@($server);$index++
    }
    return @{servers=$servers;final='dns-0';strategy=$(if($Options.ipv6){$Options.dns_strategy}else{'ipv4_only'})}
}
function New-NativeProfile([string]$Id,$Endpoint,$Standard,$Options) {
    $core=Get-Core $Id;Assert-Standard $Standard;Assert-CoreOptions $Options $core
    $needsProxy=$Standard.default -eq 'proxy' -or @($Standard.rules | Where-Object {$_.action -eq 'proxy'}).Count -gt 0 -or @($Standard.browser_domains).Count -gt 0
    if($needsProxy -and -not $Endpoint){throw 'This standard uses proxy routes. Select an endpoint first or choose Direct.'}
    if(-not $core.process_rules -and @($Standard.rules | Where-Object {$_.kind -eq 'process'}).Count){throw 'This core adapter cannot generate process rules. Select Mihomo/sing-box or use a native vendor profile.'}
    $rules=@($Standard.browser_domains | ForEach-Object {[pscustomobject]@{kind='domain';value=$_;action='proxy'}})+@($Standard.rules)
    if($Id -eq 'mihomo'){
        $native=[ordered]@{'mixed-port'=[int]$Options.port;'bind-address'='127.0.0.1';'allow-lan'=$false;mode='rule';'log-level'=$(if($Options.log_level -eq 'warn'){'warning'}else{$Options.log_level});ipv6=[bool]$Options.ipv6;'find-process-mode'='strict';dns=(New-ClientDns $Options $Id);tun=@{enable=[bool]$Options.tun;stack='mixed';'auto-route'=$true;'auto-detect-interface'=$true;'dns-hijack'=@('any:53')};proxies=@();rules=@()}
        $outbound=New-MihomoOutbound $Endpoint;if($outbound){$native.proxies=@($outbound)}
        foreach($cidr in @('127.0.0.0/8','10.0.0.0/8','172.16.0.0/12','192.168.0.0/16')){$native.rules+=@('IP-CIDR,'+$cidr+',DIRECT,no-resolve')}
        foreach($rule in $rules){$kind=switch($rule.kind){domain {'DOMAIN-SUFFIX'}process {'PROCESS-NAME'}ip {'IP-CIDR'}};$action=if($rule.action -eq 'block'){'REJECT'}else{Route-Tag $rule.action};$native.rules+=@($kind+','+$rule.value+','+$action)}
        $native.rules+=@('MATCH,'+$(if($Standard.default -eq 'block'){'REJECT'}else{Route-Tag $Standard.default}));return (Convert-ToYaml $native)
    }
    if($Id -eq 'singbox'){
        $native=@{log=@{level=$(if($Options.log_level -eq 'warning'){'warn'}else{$Options.log_level})};dns=(New-ClientDns $Options $Id);inbounds=@(@{type='mixed';tag='in';listen='127.0.0.1';listen_port=[int]$Options.port});outbounds=@(@{type='direct';tag='DIRECT'});route=@{auto_detect_interface=$true;default_domain_resolver='dns-0';rules=@(@{ip_is_private=$true;action='route';outbound='DIRECT'});final=(Route-Tag $Standard.default)}}
        $outbound=New-SingboxOutbound $Endpoint;if($outbound){if($Endpoint.type -eq 'wireguard'){$native.endpoints=@($outbound)}else{$native.outbounds+=@($outbound)}}
        foreach($rule in $rules){$r=@{action=$(if($rule.action -eq 'block'){'reject'}else{'route'})};if($rule.action -ne 'block'){$r.outbound=Route-Tag $rule.action};$field=switch($rule.kind){domain {'domain_suffix'}process {'process_name'}ip {'ip_cidr'}};$r[$field]=@($rule.value);$native.route.rules+=@($r)}
        if($Standard.default -eq 'block'){$native.route.final='DIRECT';$native.route.rules+=@(@{action='reject'})}
        if($Options.tun){$native.inbounds+=@(@{type='tun';tag='tun';address=@('172.19.0.1/30');auto_route=$true;strict_route=$true;stack='mixed'})}
        return ($native | ConvertTo-Json -Depth 40)
    }
    if($Id -eq 'xray'){
        $native=@{log=@{loglevel=$(if($Options.log_level -eq 'warn'){'warning'}else{$Options.log_level})};dns=(New-ClientDns $Options $Id);inbounds=@(@{tag='in';listen='127.0.0.1';port=[int]$Options.port;protocol='http';settings=@{};sniffing=@{enabled=$true;destOverride=@('http','tls')}});outbounds=@();routing=@{domainStrategy='AsIs';rules=@(@{type='field';ip=@('127.0.0.0/8','10.0.0.0/8','172.16.0.0/12','192.168.0.0/16');outboundTag='DIRECT'})}}
        $proxy=New-XrayOutbound $Endpoint;$direct=@{tag='DIRECT';protocol='freedom';settings=@{domainStrategy='UseIP'}};$block=@{tag='BLOCK';protocol='blackhole';settings=@{}}
        if($proxy -and $proxy.protocol -ne 'wireguard'){if(-not $proxy.ContainsKey('streamSettings')){$proxy.streamSettings=@{}};$proxy.streamSettings.sockopt=@{domainStrategy='UseIP'}}
        $all=@($direct,$block);if($proxy){$all+=@($proxy)};$default=Route-Tag $Standard.default;$native.outbounds=@($all | Sort-Object @{Expression={if($_.tag -eq $default){0}else{1}}})
        foreach($rule in $rules){$r=@{type='field';outboundTag=(Route-Tag $rule.action)};if($rule.kind -eq 'domain'){$r.domain=@('domain:'+$rule.value)}elseif($rule.kind -eq 'ip'){$r.ip=@($rule.value)};$native.routing.rules+=@($r)}
        return ($native | ConvertTo-Json -Depth 40)
    }
    throw 'Custom adapters use native profiles; no generic config generator is declared.'
}
function New-GeneratedProfile([string]$Id) {
    $w=Read-Workspace;$standard=Get-WorkspaceStandard $w;$endpoint=Get-WorkspaceEndpoint $w;$options=Get-CoreOptions $w $Id;$core=Get-Core $Id
    $text=New-NativeProfile $Id $endpoint $standard $options
    $id=[guid]::NewGuid().ToString('N');$path=Join-Path $Root ('profiles\'+$Id+'-'+$id+'.'+$core.format)
    Write-AtomicText $path $text -Private
    try{$validation=Test-CoreConfiguration $Id $path}catch{Remove-Item -LiteralPath (Assert-RuntimePath $path) -Force;throw}
    $name=$core.name+' / '+$standard.name+$(if($endpoint){' / '+$endpoint.name}else{' / Direct'})
    $profile=[pscustomobject]@{id=$id;name=$name;core=$Id;path=$path;port=[int]$options.port;validation=$validation;generated=$true}
    $w.profiles+=@($profile);$w.active_core=$Id;$w.selected_profiles | Add-Member -NotePropertyName $Id -NotePropertyValue $id -Force;Save-Workspace $w
    Write-WorkspacePac $standard $options.port
    Write-ProgressEvent 100 'New private profile generated and selected; running cores stay unchanged'
}
function Write-WorkspacePac($Standard,[int]$ProxyPort) {
    $lines=@('function FindProxyForURL(url, host) {','  host = host.toLowerCase();','  if (isPlainHostName(host)) return "DIRECT";')
    foreach($domain in @($Standard.browser_domains)){Assert-Domain $domain;$d=$domain.ToLowerInvariant();$lines+=@('  if (host === "'+$d+'" || dnsDomainIs(host, ".'+$d+'")) return "PROXY 127.0.0.1:'+$ProxyPort+'";')}
    $lines+=@('  return "DIRECT";','}')
    Write-AtomicText (Join-Path $Root 'browser-routing.pac') ($lines -join "`n") -Private
}
function Copy-PrivateProfile([string]$ProfileId) {
    $w=Read-Workspace;$source=@($w.profiles | Where-Object {$_.id -eq $ProfileId}) | Select-Object -First 1;if(-not $source){throw 'Unknown profile.'}
    $id=[guid]::NewGuid().ToString('N');$path=Join-Path $Root ('profiles\'+$id+[IO.Path]::GetExtension($source.path));Write-AtomicText $path (Get-Content -Encoding UTF8 -LiteralPath $source.path -Raw) -Private
    $copy=[pscustomobject]@{id=$id;name=($source.name+' / copy');core=$source.core;path=$path;port=$source.port;validation='pending';generated=$false};$w.profiles+=@($copy);$w.selected_profiles | Add-Member -NotePropertyName $source.core -NotePropertyValue $id -Force;Save-Workspace $w
    Start-Process -FilePath notepad.exe -ArgumentList (Quote-CoreArgument $path) | Out-Null
}
function Invoke-WorkspaceRequest($Request) {
    $w=Read-Workspace;$core=Get-OptionalValue $Request 'core' $w.active_core
    switch($Request.action){
        SelectCore {$null=Get-Core $Request.id;$w.active_core=$Request.id;Save-Workspace $w}
        SelectProfile {$profile=@($w.profiles | Where-Object {$_.id -eq $Request.id}) | Select-Object -First 1;if(-not $profile){throw 'Unknown profile.'};$w.active_core=$profile.core;$w.selected_profiles | Add-Member -NotePropertyName $profile.core -NotePropertyValue $profile.id -Force;Save-Workspace $w}
        SelectFile {Select-CoreProfile $core $Request.path}
        Generate {New-GeneratedProfile $core}
        Start {Start-SelectedCore $core}
        Stop {if(Test-Path -LiteralPath (Join-Path $Root 'proxy-backup.json')){$b=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $Root 'proxy-backup.json') -Raw | ConvertFrom-Json;if((Get-OptionalValue $b 'core' 'mihomo') -eq $core){Restore-SystemProxy}};Stop-OwnedProcess $core}
        Validate {$p=Get-SelectedProfile $w $core;$p.validation=Test-CoreConfiguration $core $p.path;Save-Workspace $w;Write-ProgressEvent 100 $(if($p.validation -eq 'pending'){'Install this core to run native validation'}else{'Selected profile passed native validation'})}
        Clone {Copy-PrivateProfile $Request.id}
        RenameProfile {Assert-DisplayName $Request.name;$p=@($w.profiles | Where-Object {$_.id -eq $Request.id}) | Select-Object -First 1;if(-not $p){throw 'Unknown profile.'};$p.name=$Request.name;Save-Workspace $w}
        RemoveProfile {$p=@($w.profiles | Where-Object {$_.id -eq $Request.id}) | Select-Object -First 1;if(-not $p){throw 'Unknown profile.'};$w.profiles=@($w.profiles | Where-Object {$_.id -ne $Request.id});$selected=$w.selected_profiles.PSObject.Properties[$p.core];if($selected -and $selected.Value -eq $Request.id){$w.selected_profiles.PSObject.Properties.Remove($p.core)};Save-Workspace $w}
        Install {Install-Component $core}
        InstallBinary {Install-LocalCore $core $Request.path}
        EnableProxy {Enable-CoreProxy $core}
        Backup {New-WorkspaceBackup}
        Restore {Restore-WorkspaceBackup $Request.path}
        Autostart {Set-WorkspaceAutostart}
        OpenPac {$pac=Join-Path $Root 'browser-routing.pac';if(-not(Test-Path -LiteralPath $pac)){throw 'Generate a profile first to create its browser PAC.'};Start-Process -FilePath notepad.exe -ArgumentList (Quote-CoreArgument $pac) | Out-Null}
        AddEndpoint {$endpoint=if($Request.PSObject.Properties['path']){New-WireGuardEndpoint $Request.path}else{New-LinkEndpoint $Request.link};$w.endpoints+=@($endpoint);$w.active_endpoint=$endpoint.id;Save-Workspace $w}
        SelectEndpoint {if($w.endpoints.id -notcontains $Request.id){throw 'Unknown endpoint.'};$w.active_endpoint=$Request.id;Save-Workspace $w}
        RenameEndpoint {Assert-DisplayName $Request.name;$e=@($w.endpoints | Where-Object {$_.id -eq $Request.id}) | Select-Object -First 1;if(-not $e){throw 'Unknown endpoint.'};$e.name=$Request.name;Save-Workspace $w}
        ReplaceEndpoint {$e=New-LinkEndpoint $Request.link;$old=@($w.endpoints | Where-Object {$_.id -eq $Request.id}) | Select-Object -First 1;if(-not $old){throw 'Unknown endpoint.'};$e.id=$old.id;$w.endpoints=@($w.endpoints | ForEach-Object {if($_.id -eq $old.id){$e}else{$_}});Save-Workspace $w}
        RemoveEndpoint {$w.endpoints=@($w.endpoints | Where-Object {$_.id -ne $Request.id});if($w.active_endpoint -eq $Request.id){$w.active_endpoint=$null};Save-Workspace $w}
        AddStandard {if(-not $Request.name -or $Request.name.Length -gt 80 -or $Request.name -match '[\x00-\x1f]'){throw 'Enter a short standard name.'};$s=[pscustomobject]@{id=[guid]::NewGuid().ToString('N');name=$Request.name;default='direct';rules=@();browser_domains=@()};$w.standards+=@($s);$w.active_standard=$s.id;Save-Workspace $w}
        SelectStandard {if($w.standards.id -notcontains $Request.id){throw 'Unknown routing standard.'};$w.active_standard=$Request.id;Save-Workspace $w}
        RenameStandard {Assert-DisplayName $Request.name;$s=@($w.standards | Where-Object {$_.id -eq $Request.id}) | Select-Object -First 1;if(-not $s){throw 'Unknown standard.'};$s.name=$Request.name;Save-Workspace $w}
        RemoveStandard {if(@($w.standards).Count -le 1){throw 'Keep at least one standard.'};$w.standards=@($w.standards | Where-Object {$_.id -ne $Request.id});if($w.active_standard -eq $Request.id){$w.active_standard=$w.standards[0].id};Save-Workspace $w}
        AddRule {$s=Get-WorkspaceStandard $w;if($Request.rule -notmatch '^(domain|process|ip)\s+([^\s]+)\s+(direct|proxy|block)$'){throw 'Rule format: domain example.com proxy / process app.exe direct / ip 192.0.2.0/24 block'};$s.rules+=@([pscustomobject]@{kind=$matches[1];value=$matches[2];action=$matches[3]});Assert-Standard $s;Save-Workspace $w}
        RemoveRule {$s=Get-WorkspaceStandard $w;$index=[int]$Request.index;if($index -lt 0 -or $index -ge @($s.rules).Count){throw 'Select a route rule.'};$s.rules=@(for($i=0;$i -lt @($s.rules).Count;$i++){if($i -ne $index){$s.rules[$i]}});Save-Workspace $w}
        MoveRule {$s=Get-WorkspaceStandard $w;$i=[int]$Request.index;$j=$i+[int]$Request.delta;if($i -ge 0 -and $i -lt @($s.rules).Count -and $j -ge 0 -and $j -lt @($s.rules).Count){$swap=$s.rules[$i];$s.rules[$i]=$s.rules[$j];$s.rules[$j]=$swap;Save-Workspace $w}}
        BrowserDomains {$s=Get-WorkspaceStandard $w;$s.browser_domains=@($Request.domains -split ',' | ForEach-Object {$_.Trim().ToLowerInvariant()} | Where-Object {$_} | Select-Object -Unique);Assert-Standard $s;Save-Workspace $w}
        DefaultRoute {$s=Get-WorkspaceStandard $w;$s.default=$Request.value;Assert-Standard $s;Save-Workspace $w}
        CoreOptions {$definition=Get-Core $core;Assert-CoreOptions $Request.options $definition;$w.core_options | Add-Member -NotePropertyName $core -NotePropertyValue $Request.options -Force;Save-Workspace $w}
        OpenLogs {$log=Join-Path $Root ('logs\'+$core+'.out.log');if(-not(Test-Path -LiteralPath $log)){throw 'Start this core to create its log.'};Start-Process -FilePath notepad.exe -ArgumentList (Quote-CoreArgument $log) | Out-Null}
        OpenProfile {$p=Get-SelectedProfile $w $core;Start-Process -FilePath notepad.exe -ArgumentList (Quote-CoreArgument $p.path) | Out-Null}
        OpenFolder {Start-Process -FilePath explorer.exe -ArgumentList (Quote-CoreArgument $Root) | Out-Null}
        OpenLegacy {$legacy=Join-Path $Root 'routing.ps1';if(-not(Test-Path -LiteralPath $legacy) -or [string]::Equals($Root,$Assets,[StringComparison]::OrdinalIgnoreCase)){throw 'No separate legacy routing manager exists in this runtime.'};Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList ('-NoProfile -ExecutionPolicy Bypass -File '+(Quote-CoreArgument $legacy)+' -Command Menu') -WorkingDirectory $Root -WindowStyle Normal | Out-Null}
        ImportAdapter {$adapter=Get-Content -Encoding UTF8 -LiteralPath $Request.path -Raw | ConvertFrom-Json;Assert-CoreAdapter $adapter;if(@(Read-CoreCatalog).id -contains $adapter.id){throw 'This adapter ID already exists.'};Write-AtomicText (Join-Path $Root ('core-adapters\'+$adapter.id+'.json')) ($adapter | ConvertTo-Json -Depth 20) -Private}
        default {throw 'Unknown workspace action.'}
    }
}
function Assert-DisplayName([string]$Name){if(-not $Name -or $Name.Length -gt 80 -or $Name -match '[\x00-\x1f]'){throw 'Enter a short name without control characters.'}}
function Install-LocalCore([string]$Id,[string]$Source){
    $core=Get-Core $Id;$exe=Get-CoreExe $Id;if(Test-Path -LiteralPath $exe){throw 'A binary already exists. Install custom binaries in a new private runtime.'}
    $sourceFile=Get-Item -LiteralPath $Source;if($sourceFile.Extension -ne '.exe' -or $sourceFile.PSIsContainer){throw 'Select a trusted Windows executable.'}
    $bytes=[IO.File]::ReadAllBytes($sourceFile.FullName);if($bytes.Length -lt 64 -or $bytes[0] -ne 77 -or $bytes[1] -ne 90){throw 'File is not a Windows executable.'}
    $destination=Assert-RuntimePath $exe;$parent=Split-Path -Parent $destination;New-Item -ItemType Directory -Path $parent -Force | Out-Null;Protect-PrivateDirectory $parent;Copy-Item -LiteralPath $sourceFile.FullName -Destination $destination
    Write-ProgressEvent 100 'Trusted local binary installed; copy vendor dependencies beside it. Startup is separate.'
}
function Enable-CoreProxy([string]$Id){
    $w=Read-Workspace;$profile=Get-SelectedProfile $w $Id;if(-not(Get-OwnedProcess $Id)){throw 'Start the managed selected core first.'}
    $record=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $Root ($Id+'.pid.json')) -Raw | ConvertFrom-Json
    if($record.port -ne $profile.port -or -not[string]::Equals($record.config,$profile.path,[StringComparison]::OrdinalIgnoreCase)){throw 'Selected profile differs from running profile; stop/start before enabling Windows Proxy.'}
    if(-not(Test-Port $record.port)){throw 'The managed HTTP listener is unavailable.'}
    $key='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings';$backupFile=Join-Path $Root 'proxy-backup.json';if(Test-Path -LiteralPath $backupFile){throw 'Restore the existing Windows proxy snapshot first.'}
    $values=Get-ProxyValues (Get-ItemProperty -LiteralPath $key)
    $applied=[pscustomobject]@{ProxyEnable=[pscustomobject]@{exists=$true;value=1};ProxyServer=[pscustomobject]@{exists=$true;value=('127.0.0.1:'+$record.port)};ProxyOverride=[pscustomobject]@{exists=$true;value='<local>;127.*;10.*;192.168.*;*.local'};AutoConfigURL=[pscustomobject]@{exists=$false;value=$null}}
    $backup=[pscustomobject]@{core=$Id;values=[pscustomobject]$values;applied=$applied};Write-AtomicText $backupFile ($backup | ConvertTo-Json -Depth 8) -Private
    try{Apply-ProxyValues $key $applied;Notify-ProxyChange}catch{Apply-ProxyValues $key $backup.values;Notify-ProxyChange;Remove-Item -LiteralPath $backupFile -Force;throw}
}
function New-WorkspaceBackup {
    $w=Read-Workspace;$id=[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')+'-'+[guid]::NewGuid().ToString('N').Substring(0,8);$folder=Assert-RuntimePath (Join-Path $Root ('backups\'+$id));New-Item -ItemType Directory -Path $folder -Force | Out-Null;Protect-PrivateDirectory $folder
    foreach($p in @($w.profiles)){if(-not(Test-Path -LiteralPath $p.path)){throw 'Backup requires all profile source files to exist.'};$name=$p.id+[IO.Path]::GetExtension($p.path);$target=Join-Path $folder $name;Write-AtomicText $target (Get-Content -Encoding UTF8 -LiteralPath $p.path -Raw) -Private;$p.path=$name}
    Write-AtomicText (Join-Path $folder 'workspace.json') ($w | ConvertTo-Json -Depth 40) -Private
    Write-AtomicText (Join-Path $folder 'settings.json') ((Read-Settings) | ConvertTo-Json -Depth 20) -Private
    $tg=Get-TgConfigPath;if(Test-Path -LiteralPath $tg){Write-AtomicText (Join-Path $folder 'telegram-private.json') (Get-Content -Encoding UTF8 -LiteralPath $tg -Raw) -Private}
    $custom=Join-Path $Root 'core-adapters';if(Test-Path -LiteralPath $custom){Copy-Item -LiteralPath $custom -Destination $folder -Recurse}
    Write-ProgressEvent 100 ('Private backup saved: '+$folder)
}
function Restore-WorkspaceBackup([string]$Folder){
    foreach($c in @(Read-CoreCatalog)){if(Get-OwnedProcess $c.id){throw 'Stop all managed cores before restoring a backup.'}};if(Get-OwnedProcess 'telegram'){throw 'Stop managed Telegram before restoring its settings.'}
    $folderFull=(Get-Item -LiteralPath $Folder).FullName;$source=Join-Path $folderFull 'workspace.json';$w=Get-Content -Encoding UTF8 -LiteralPath $source -Raw | ConvertFrom-Json;if($w.schema_version -ne 1){throw 'Unsupported backup schema.'}
    $contents=@{};foreach($p in @($w.profiles)){if([IO.Path]::GetFileName($p.path) -ne $p.path){throw 'Backup profiles must be relative filenames.'};$contents[$p.id]=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $folderFull $p.path) -Raw;$null=Get-Core $p.core}
    foreach($s in @($w.standards)){Assert-Standard $s}
    $tg=Join-Path $folderFull 'telegram-private.json';if(Test-Path -LiteralPath $tg){$cfg=Get-Content -Encoding UTF8 -LiteralPath $tg -Raw | ConvertFrom-Json;if(Test-Port $cfg.port){throw 'Telegram listener is active; stop it before restoring settings.'}}
    New-WorkspaceBackup
    foreach($p in @($w.profiles)){$path=Join-Path $Root ('profiles\restored-'+[guid]::NewGuid().ToString('N')+[IO.Path]::GetExtension($p.path));Write-AtomicText $path $contents[$p.id] -Private;$p.path=$path;$p.validation='pending'}
    Save-Workspace $w
    if(Test-Path -LiteralPath $tg){Write-AtomicText (Get-TgConfigPath) (Get-Content -Encoding UTF8 -LiteralPath $tg -Raw) -Private;$settings=Read-Settings;$settings.tg_port=[int]$cfg.port;Save-Settings $settings}
    Write-ProgressEvent 100 'Backup restored into new private profile copies; original files were retained'
}
function Set-WorkspaceAutostart {
    $suffix=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($Root))).Replace('-','').Substring(0,12)
    $name='URTW-Core-'+$suffix;$task=Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
    if($task){Unregister-ScheduledTask -TaskName $name -Confirm:$false;Write-ProgressEvent 100 'URTW core autostart disabled';return}
    $w=Read-Workspace;$p=Get-SelectedProfile $w $w.active_core;if((Test-CoreConfiguration $w.active_core $p.path) -ne 'valid'){throw 'Install and validate the core first.'}
    $arguments='-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File '+(Quote-CoreArgument (Join-Path $Assets 'routing.ps1'))+' -Root '+(Quote-CoreArgument $Root)+' -Command Boot'
    $user=[Security.Principal.WindowsIdentity]::GetCurrent().Name;$action=New-ScheduledTaskAction -Execute (Join-Path $PSHOME 'powershell.exe') -Argument $arguments;$trigger=New-ScheduledTaskTrigger -AtLogOn -User $user
    $admin=([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    $principal=New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel $(if($admin){'Highest'}else{'Limited'})
    $settings=New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero)
    Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description ('URTW selected core for '+$Root) | Out-Null
    Write-ProgressEvent 100 'Selected core will start at logon. TUN requires an administrator registration.'
}
