[CmdletBinding()]
param([ValidateSet('amd64','arm64')][string]$Arch='amd64',[string]$Target='x86_64-pc-windows-msvc',[switch]$SkipBuild,[string]$Python='python')
$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'check-release.ps1') -Python $Python
$version=(Get-Content -LiteralPath (Join-Path $project 'VERSION') -Raw).Trim()
if(($Arch -eq 'amd64' -and $Target -ne 'x86_64-pc-windows-msvc') -or ($Arch -eq 'arm64' -and $Target -ne 'aarch64-pc-windows-msvc')){throw 'Architecture and Rust target disagree.'}
if(-not $SkipBuild){
    $cargoHome=if($env:CARGO_HOME){$env:CARGO_HOME}else{Join-Path $env:USERPROFILE '.cargo'}
    $rustupHome=if($env:RUSTUP_HOME){$env:RUSTUP_HOME}else{Join-Path $env:USERPROFILE '.rustup'}
    $oldFlags=$env:RUSTFLAGS
    try {
        $env:RUSTFLAGS="--remap-path-prefix=$project=/usr/src/urtw --remap-path-prefix=$cargoHome=/usr/src/cargo --remap-path-prefix=$rustupHome=/usr/src/rust"
        Push-Location $project;try{cargo build --release --locked --target $Target;if($LASTEXITCODE -ne 0){throw 'Release build failed.'}}finally{Pop-Location}
    }finally{$env:RUSTFLAGS=$oldFlags}
}
$binary=Join-Path $project ('target\'+$Target+'\release\URTW.exe')
if(-not(Test-Path -LiteralPath $binary)){throw 'Built release binary missing.'}
$dist=Join-Path $project 'dist';New-Item -ItemType Directory -Path $dist -Force | Out-Null
$stage=Join-Path $dist ('package-'+[guid]::NewGuid().ToString('N'))
$package=Join-Path $stage ('URTW-'+$Arch);New-Item -ItemType Directory -Path $package -Force | Out-Null
try {
    Copy-Item -LiteralPath $binary -Destination (Join-Path $package 'URTW.exe')
    foreach($file in @('routing.ps1','setup.ps1','URTW.bat','install.bat','portable.flag','components.lock.json','README.md','README.ru.md','LICENSE','THIRD_PARTY_NOTICES.md','SECURITY.md','CHANGELOG.md','VERSION','SIGNING_KEY_FINGERPRINT')){Copy-Item -LiteralPath (Join-Path $project $file) -Destination $package}
    foreach($dir in @('config','docs','modules')){Copy-Item -LiteralPath (Join-Path $project $dir) -Destination $package -Recurse}
    $zip=Join-Path $dist ("URTW-$version-windows-$Arch.zip")
    if(Test-Path -LiteralPath $zip){Remove-Item -LiteralPath $zip}
    Compress-Archive -LiteralPath $package -DestinationPath $zip -CompressionLevel Optimal
    & $Python (Join-Path $PSScriptRoot 'scan-secrets.py') --zip $zip
    if($LASTEXITCODE -ne 0){Remove-Item -LiteralPath $zip;throw 'Release package credential scan failed.'}
    Write-Host "Built portable package: $zip"
}finally{
    $resolved=[IO.Path]::GetFullPath($stage);$expected=[IO.Path]::GetFullPath($dist).TrimEnd('\')+'\'
    if(-not $resolved.StartsWith($expected,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notmatch '^package-[a-f0-9]{32}$'){throw 'Unsafe packaging cleanup path.'}
    if(Test-Path -LiteralPath $resolved){Remove-Item -LiteralPath $resolved -Recurse -Force}
}
