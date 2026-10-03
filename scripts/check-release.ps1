[CmdletBinding()]
param([string]$Tag,[string]$Python='python')
$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
$version=(Get-Content -LiteralPath (Join-Path $project 'VERSION') -Raw).Trim()
if($version -notmatch '^\d+\.\d+\.\d+$'){throw 'VERSION must use X.Y.Z.'}
$cargo=Get-Content -LiteralPath (Join-Path $project 'Cargo.toml') -Raw
if($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"' -or $matches[1] -ne $version){throw 'Cargo.toml and VERSION disagree.'}
if($Tag -and $Tag -ne ('v'+$version)){throw 'Git tag and VERSION disagree.'}
$notes=Join-Path $project ('docs\releases\v'+$version+'.md')
if(-not(Test-Path -LiteralPath $notes)){throw 'Bilingual release notes are missing.'}
$fingerprint=(Get-Content -LiteralPath (Join-Path $project 'SIGNING_KEY_FINGERPRINT') -Raw).Trim()
if($fingerprint -notmatch '^[A-F0-9]{40}$'){throw 'Invalid signing key fingerprint.'}
$manifest=Get-Content -LiteralPath (Join-Path $project 'components.lock.json') -Raw | ConvertFrom-Json
if($manifest.schema_version -ne 1){throw 'Unsupported download manifest schema.'}
foreach($name in @('mihomo','telegram','zapret')){
    $component=$manifest.$name
    foreach($arch in @('amd64','arm64','wintun')){
        $p=$component.PSObject.Properties[$arch]
        if($p -and ($p.Value.sha256 -notmatch '^[a-f0-9]{64}$' -or $p.Value.url -notmatch '^https://(?:github\.com|www\.wintun\.net)/')){throw 'Invalid pinned component URL or digest.'}
    }
}
& $Python (Join-Path $PSScriptRoot 'scan-secrets.py') --repo $project
if($LASTEXITCODE -ne 0){throw 'Credential scan failed.'}
Write-Host "Release metadata checks passed: v$version"
