[CmdletBinding()]
param([string]$Version,[string]$Repository='Oqune/URT',[string]$Gpg='gpg',[string]$Python='python')
$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
if(-not $Version){$Version=(Get-Content -LiteralPath (Join-Path $project 'VERSION') -Raw).Trim()}
if($Version -notmatch '^\d+\.\d+\.\d+$' -or $Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'){throw 'Invalid release version or repository.'}
$tag='v'+$Version
& (Join-Path $PSScriptRoot 'check-release.ps1') -Tag $tag -Python $Python
$fingerprint=(Get-Content -LiteralPath (Join-Path $project 'SIGNING_KEY_FINGERPRINT') -Raw).Trim()
function Invoke-Verification([string]$Program,[string[]]$Arguments) {
    # PowerShell 5.1 turns merged native stderr into ErrorRecords even on success.
    $savedPreference=$ErrorActionPreference
    try {
        $ErrorActionPreference='Continue'
        $output=& $Program @Arguments 2>&1
        $code=$LASTEXITCODE
        return @{Code=$code;Text=($output -join "`n")}
    }finally{$ErrorActionPreference=$savedPreference}
}
$verify=Invoke-Verification 'git' @('-C',$project,'-c',"gpg.program=$Gpg",'verify-tag','--raw',$tag)
if($verify.Code -ne 0 -or ($verify.Text -notmatch ('\[GNUPG:\] VALIDSIG '+[regex]::Escape($fingerprint)+'\b'))){throw 'Release tag does not verify with the expected GPG key.'}
$commit=(& git -C $project rev-list -n 1 $tag).Trim()
$runs=& gh run list --repo $Repository --workflow build.yml --branch $tag --commit $commit --json status,conclusion,event,headSha | ConvertFrom-Json
if(-not @($runs | Where-Object {$_.status -eq 'completed' -and $_.conclusion -eq 'success' -and $_.event -eq 'push'})){throw 'No successful completed release workflow for this commit.'}
$release=& gh release view $tag --repo $Repository --json isDraft,tagName,assets | ConvertFrom-Json
if($LASTEXITCODE -ne 0 -or -not $release.isDraft -or $release.tagName -ne $tag){throw 'Publication requires an existing draft for this exact tag.'}
$tempBase=[IO.Path]::GetFullPath([IO.Path]::GetTempPath());$stage=Join-Path $tempBase ('URT-release-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    & gh release download $tag --repo $Repository --dir $stage --pattern "URT-$Version-windows-*.zip" --pattern SHA256SUMS
    if($LASTEXITCODE -ne 0){throw 'Could not download draft release assets.'}
    $sums=Join-Path $stage 'SHA256SUMS';$seen=@()
    foreach($line in (Get-Content -LiteralPath $sums)) {
        if($line -notmatch '^([a-f0-9]{64})\s+\*?(URT-[0-9.]+-windows-(?:amd64|arm64)\.zip)$'){throw 'Malformed release checksum manifest.'}
        $hash=$matches[1];$name=$matches[2]
        if($name -notin @("URT-$Version-windows-amd64.zip","URT-$Version-windows-arm64.zip") -or $seen -contains $name){throw 'Unexpected or duplicate release artifact.'}
        $seen+=$name
        if((Get-FileHash -LiteralPath (Join-Path $stage $name) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash){throw 'Release checksum mismatch.'}
    }
    if($seen.Count -ne 2){throw 'Both Windows architecture archives are required.'}
    $zips=@($seen | ForEach-Object {Join-Path $stage $_})
    & $Python (Join-Path $PSScriptRoot 'scan-secrets.py') --zip @zips
    if($LASTEXITCODE -ne 0){throw 'Release assets failed the private-data scan.'}
    $signature=Join-Path $stage 'SHA256SUMS.asc'
    & $Gpg --armor --local-user $fingerprint --output $signature --detach-sign $sums
    if($LASTEXITCODE -ne 0){throw 'GPG signing failed.'}
    $result=Invoke-Verification $Gpg @('--status-fd','1','--verify',$signature,$sums)
    if($result.Code -ne 0 -or ($result.Text -notmatch ('\[GNUPG:\] VALIDSIG '+[regex]::Escape($fingerprint)+'\b'))){throw 'New release signature failed verification.'}
    & gh release upload $tag $signature --repo $Repository --clobber
    if($LASTEXITCODE -ne 0){throw 'Signature upload failed; draft remains unpublished.'}
    & gh release edit $tag --repo $Repository --draft=false --latest
    if($LASTEXITCODE -ne 0){throw 'Publication failed.'}
    Write-Host "Published GPG-signed release: https://github.com/$Repository/releases/tag/$tag"
}finally{
    $resolved=[IO.Path]::GetFullPath($stage)
    if(-not $resolved.StartsWith($tempBase,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notmatch '^URT-release-[a-f0-9]{32}$'){throw 'Unsafe release cleanup path.'}
    if(Test-Path -LiteralPath $resolved){Remove-Item -LiteralPath $resolved -Recurse -Force}
}
