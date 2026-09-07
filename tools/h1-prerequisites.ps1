$ErrorActionPreference = 'Stop'
$pin = Get-Content -Raw tools/sqlcipher/windows-toolchain.json | ConvertFrom-Json
if (-not $env:RUNNER_TEMP) { throw 'Hosted RUNNER_TEMP required' }
$downloadRoot = Join-Path $env:RUNNER_TEMP 'h1-perl-download'
$installRoot = Join-Path $env:RUNNER_TEMP 'h1-perl'
New-Item -ItemType Directory -Force -Path $downloadRoot | Out-Null
$archivePath = Join-Path $downloadRoot $pin.archive.name
Invoke-WebRequest -Uri $pin.archive.url -OutFile $archivePath
if ((Get-Item -LiteralPath $archivePath).Length -ne $pin.archive.size_bytes) { throw 'Pinned Perl archive size mismatch' }
if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $pin.archive.sha256) { throw 'Pinned Perl archive digest mismatch' }
if (Test-Path -LiteralPath $installRoot) { throw 'Perl extraction root already exists' }
Expand-Archive -LiteralPath $archivePath -DestinationPath $installRoot
$perlPath = Join-Path $installRoot $pin.perl_relative_path
"OPENSSL_SRC_PERL=$perlPath" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
$env:OPENSSL_SRC_PERL = $perlPath
node tools/h1-windows-toolchain.mjs
if ($LASTEXITCODE -ne 0) { throw 'Pinned Perl identity verification failed' }
# The x64 build-time interpreter may run under Windows ARM emulation. Cargo,
# the C compiler target, and executed test binaries must still be native ARM64.
