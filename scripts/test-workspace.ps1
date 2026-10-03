# Runs inside test-engine.ps1's isolated runtime. Never targets the live installation.
$workspace=Read-Workspace
Assert-That (@(Read-CoreCatalog).Count -eq 3) 'Built-in core catalog has Mihomo, sing-box and Xray with attribution'
Assert-That ($workspace.schema_version -eq 1 -and $workspace.profiles[0].id -eq 'legacy-0') 'Legacy selection migrates with stable identifiers'
Save-Workspace $workspace
Assert-That ((Get-Acl -LiteralPath (Join-Path $runtime 'workspace.json')).AreAccessRulesProtected) 'Endpoint and workspace data have a protected ACL'
$sourceHash=(Get-FileHash -LiteralPath $wg).Hash
$endpoint=New-WireGuardEndpoint $wg
Assert-That ($endpoint.type -eq 'wireguard' -and (Get-FileHash -LiteralPath $wg).Hash -eq $sourceHash) 'WireGuard endpoint import leaves the original unchanged'
$socks=New-LinkEndpoint 'socks5://user:sample-password@example.com:1080#Sample'
$vless=New-LinkEndpoint ('vless://'+[guid]::NewGuid().ToString()+'@example.com:443?security=tls&type=ws&sni=example.com&path=%2Fws#Sample')
$trojan=New-LinkEndpoint 'trojan://sample-password@example.com:443?security=tls&type=grpc&serviceName=sample#Sample'
$httpEndpoint=New-LinkEndpoint 'http://user:sample-password@example.com:8080#Sample'
$httpsEndpoint=New-LinkEndpoint 'https://user:sample-password@example.com:443#Sample'
$realityKey=(New-TestKey).TrimEnd('=').Replace('+','-').Replace('/','_')
$realityEndpoint=New-LinkEndpoint ('vless://'+[guid]::NewGuid().ToString()+'@example.com:443?security=reality&type=tcp&sni=example.com&pbk='+$realityKey+'&sid=0123456789abcdef&fp=chrome#Sample')
Assert-That ($socks.data.username -eq 'user' -and $vless.data.transport -eq 'ws') 'Share-link credentials and transport are parsed into private endpoint data'
Assert-Throws {New-LinkEndpoint 'file:///C:/example'} 'Unsupported endpoint protocols are rejected'
Assert-Throws {New-LinkEndpoint 'vless://invalid@example.com:443?security=tls'} 'Invalid VLESS UUID is rejected'
Assert-Throws {New-LinkEndpoint ('vless://'+[guid]::NewGuid()+'@example.com:443?security=none')} 'Unprotected generated VLESS links are rejected'
Invoke-WorkspaceRequest ([pscustomobject]@{action='AddStandard';name='Custom test'})
Invoke-WorkspaceRequest ([pscustomobject]@{action='AddRule';rule='domain example.com proxy'})
Invoke-WorkspaceRequest ([pscustomobject]@{action='AddRule';rule='process browser.exe direct'})
Invoke-WorkspaceRequest ([pscustomobject]@{action='MoveRule';index=1;delta=-1})
$workspace=Read-Workspace;$standard=Get-WorkspaceStandard $workspace
Assert-That ($standard.rules[0].kind -eq 'process' -and $standard.default -eq 'direct') 'Custom standards preserve ordered per-application rules'
Assert-Throws {Invoke-WorkspaceRequest ([pscustomobject]@{action='AddRule';rule='domain https://example.com proxy'})} 'Routing rejects URLs masquerading as domains'
Invoke-WorkspaceRequest ([pscustomobject]@{action='RemoveRule';index=0})
Invoke-WorkspaceRequest ([pscustomobject]@{action='BrowserDomains';domains='example.com, test.example.com'})
$workspace=Read-Workspace;$standard=Get-WorkspaceStandard $workspace
Assert-That (@($standard.rules).Count -eq 1 -and @($standard.browser_domains).Count -eq 2) 'Rule removal and independent browser split are saved'
$directStandard=[pscustomobject]@{id='direct-test';name='Direct';default='direct';rules=@();browser_domains=@()}
$proxyStandard=[pscustomobject]@{id='proxy-test';name='Proxy';default='direct';rules=@([pscustomobject]@{kind='domain';value='example.com';action='proxy'});browser_domains=@()}
$processStandard=[pscustomobject]@{id='process-test';name='Process';default='direct';rules=@([pscustomobject]@{kind='process';value='browser.exe';action='direct'});browser_domains=@()}
Assert-Throws {New-NativeProfile 'mihomo' $null $proxyStandard (Get-CoreOptions $workspace 'mihomo')} 'Proxy standards require a selected endpoint'
Assert-Throws {New-NativeProfile 'xray' $null $processStandard (Get-CoreOptions $workspace 'xray')} 'Xray process rules are rejected with a capability error'
foreach($coreId in @('mihomo','singbox','xray')){
    $options=Get-CoreOptions $workspace $coreId
    $options.dns=@('1.1.1.1','https://cloudflare-dns.com/dns-query')
    $options.tun=$false
    $native=New-NativeProfile $coreId $null $directStandard $options
    if($coreId -eq 'xray'){$json=$native | ConvertFrom-Json;Assert-That (($json.outbounds | Where-Object {$_.protocol -eq 'freedom'}).settings.domainStrategy -eq 'UseIP') 'Xray direct outbound resolves through its configured client DNS'}
    $core=Get-Core $coreId;$nativePath=Join-Path $runtime ('profiles\'+$coreId+'-workspace.'+$core.format)
    Write-AtomicText $nativePath $native -Private
    Assert-That ((Get-NativeProfilePort $coreId $nativePath) -eq $options.port) ($coreId+' generator emits a native loopback HTTP listener')
    if(-not $Integration -and -not(Test-Path -LiteralPath (Get-CoreExe $coreId))){Select-CoreProfile $coreId $nativePath;Assert-That ((Get-SelectedProfile (Read-Workspace) $coreId).validation -eq 'pending') ($coreId+' selection works before installing its binary')}
    if($Integration){
        if($coreId -ne 'mihomo'){Install-Component $coreId}
        Assert-That ((Test-CoreConfiguration $coreId $nativePath) -eq 'valid') ($coreId+' accepts generated direct DNS profile with the real pinned binary')
        foreach($ep in @($socks,$vless,$trojan,$endpoint,$httpEndpoint,$httpsEndpoint,$realityEndpoint)){
            Write-AtomicText $nativePath (New-NativeProfile $coreId $ep $proxyStandard $options) -Private
            Assert-That ((Test-CoreConfiguration $coreId $nativePath) -eq 'valid') ($coreId+' validates generated '+$ep.type+' endpoint without connecting it')
        }
        foreach($fallback in @('proxy','block')){$fallbackStandard=[pscustomobject]@{id='fallback';name='Fallback';default=$fallback;rules=@();browser_domains=@()};Write-AtomicText $nativePath (New-NativeProfile $coreId $socks $fallbackStandard $options) -Private;Assert-That ((Test-CoreConfiguration $coreId $nativePath) -eq 'valid') ($coreId+' validates '+$fallback+' fallback')}
        if($coreId -ne 'xray'){Write-AtomicText $nativePath (New-NativeProfile $coreId $null $processStandard $options) -Private;Assert-That ((Test-CoreConfiguration $coreId $nativePath) -eq 'valid') ($coreId+' validates process routing');$options.tun=$true;Write-AtomicText $nativePath (New-NativeProfile $coreId $null $directStandard $options) -Private;Assert-That ((Test-CoreConfiguration $coreId $nativePath) -eq 'valid') ($coreId+' validates TUN syntax without creating an adapter');$options.tun=$false}
        $listener=New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback,0);$listener.Start();$options.port=$listener.LocalEndpoint.Port;$listener.Stop()
        Write-AtomicText $nativePath (New-NativeProfile $coreId $null $directStandard $options) -Private
        Select-CoreProfile $coreId $nativePath;Start-SelectedCore $coreId
        Assert-That ([bool](Get-OwnedProcess $coreId) -and (Test-Port $options.port)) ($coreId+' starts only an isolated TUN-off listener')
        $httpListener=New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback,0);$httpListener.Start();$httpPort=$httpListener.LocalEndpoint.Port;$httpListener.Stop()
        $httpJob=Start-Job -ArgumentList $httpPort -ScriptBlock {
            param($LocalPort)
            $server=New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback,$LocalPort);$server.Start()
            try{while($true){$client=$server.AcceptTcpClient();try{$stream=$client.GetStream();$stream.ReadTimeout=5000;$buffer=New-Object byte[] 4096;$count=$stream.Read($buffer,0,$buffer.Length);if($count -eq 0){continue};$body='URTW-isolated-access';$response=[Text.Encoding]::ASCII.GetBytes("HTTP/1.1 200 OK`r`nContent-Length: $($body.Length)`r`nConnection: close`r`n`r`n$body");$stream.Write($response,0,$response.Length);break}finally{$client.Dispose()}}}finally{$server.Stop()}
        }
        try{
            for($attempt=0;$attempt -lt 50 -and -not(Test-Port $httpPort);$attempt++){Start-Sleep -Milliseconds 100}
            $reply=& curl.exe --silent --show-error --max-time 8 --noproxy does-not-match.invalid --proxy ('http://127.0.0.1:'+$options.port) ('http://127.0.0.1:'+$httpPort+'/')
            Assert-That ($LASTEXITCODE -eq 0 -and $reply -eq 'URTW-isolated-access') ($coreId+' forwards a real isolated HTTP request through its explicit proxy')
        }finally{Stop-Job -Job $httpJob;Remove-Job -Job $httpJob -Force}
        if($Dashboard){$uiSnapshot=Join-Path $runtime ($coreId+'-ui.json');& $dashboardFile --root $runtime --assets $project --snapshot $uiSnapshot --tab 1 --width 150 --height 36;if($LASTEXITCODE -ne 0){throw 'Dashboard snapshot failed.'};$screen=(((Get-Content -LiteralPath $uiSnapshot -Raw | ConvertFrom-Json).cells | ForEach-Object {$_.text}) -join '');Assert-That ($screen.Contains('PID '+(Get-OwnedProcess $coreId).Id) -and $screen.Contains('Running')) ($coreId+' is recognized as managed by the real Rust UI')}
        Stop-OwnedProcess $coreId
        Assert-That (-not(Test-Port $options.port)) ($coreId+' releases its isolated listener')
    }
}
if($Integration){$afterWorkspace=Get-ItemProperty -LiteralPath $proxyKey | Select-Object ProxyEnable,ProxyServer,ProxyOverride,AutoConfigURL | ConvertTo-Json -Compress;Assert-That ($before -eq $afterWorkspace) 'All three core integrations leave the Windows proxy unchanged'}
$workspace=Read-Workspace;$workspace.active_core='mihomo';$workspace.active_standard=$workspace.standards[0].id;Save-Workspace $workspace
New-WorkspaceBackup
$backup=Get-ChildItem -LiteralPath (Join-Path $runtime 'backups') -Directory | Sort-Object Name -Descending | Select-Object -First 1
Assert-That ((Get-Acl -LiteralPath $backup.FullName).AreAccessRulesProtected) 'Backup folder protects private native profiles'
$beforeRestore=Read-Workspace;$oldPaths=@($beforeRestore.profiles.path)
Restore-WorkspaceBackup $backup.FullName
$restored=Read-Workspace
Assert-That (@($restored.profiles).Count -eq @($beforeRestore.profiles).Count -and $restored.profiles[0].path -ne $oldPaths[0]) 'Restore creates new private files without overwriting originals'
Assert-Throws {Resolve-InstalledPath 'mihomo' 'unused' | Out-Null;Assert-RuntimePath 'C:\outside\core.exe'} 'Installation aliases cannot escape the runtime'
