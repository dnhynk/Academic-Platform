# Developer bootstrap

## Supported baseline

| Tool | Pin | Purpose |
|---|---:|---|
| Rust | 1.98.0 | Core/domain/ledger/contracts/CLI |
| Rust host on Windows | `x86_64-pc-windows-msvc` | Native first-class Windows build |
| Node | 24.19.0 | Cross-platform tooling and TypeScript contract |
| pnpm | 11.22.0 | Single workspace lockfile |

Docker, a database server, a cloud account, microphone access, and network services are not prerequisites.

## Windows

The official rustup distribution can be installed non-interactively with a checksum check:

```powershell
$rustupDir = Join-Path ([System.IO.Path]::GetTempPath()) ('academic-rustup-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $rustupDir | Out-Null
$rustupExe = Join-Path $rustupDir 'rustup-init.exe'
$rustupSha = Join-Path $rustupDir 'rustup-init.exe.sha256'
Invoke-WebRequest https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile $rustupExe
Invoke-WebRequest https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe.sha256 -OutFile $rustupSha
$expected = ((Get-Content -Raw $rustupSha).Trim() -split '\s+')[0]
$actual = (Get-FileHash -Algorithm SHA256 $rustupExe).Hash
if ($actual -ne $expected) { throw 'rustup-init checksum mismatch' }
& $rustupExe -y --default-host x86_64-pc-windows-msvc --default-toolchain none --profile minimal
```

Then install repository pins:

```powershell
rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy
nvm use 24.19.0
npm install --global pnpm@11.22.0
pnpm install --frozen-lockfile
pnpm run doctor
```

If `cargo` is not visible in the current shell after first installing rustup, restart the shell or temporarily prepend `%USERPROFILE%\.cargo\bin` to `PATH`. MSVC linking additionally needs Visual Studio Build Tools with the Desktop C++ workload and a Windows SDK; Phase 0's pure Rust crates otherwise have no external native library.

## Linux

Use the official rustup bootstrap, then the same exact repository pin:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy
```

Install Node 24.19.0 with the organization's approved version manager, then:

```bash
npm install --global pnpm@11.22.0
pnpm install --frozen-lockfile
pnpm run doctor
```

## Reproducible bootstrap script

After the pinned tools are present, `pnpm bootstrap` validates exact versions, runs `pnpm install --frozen-lockfile`, and runs `cargo fetch --locked`. It does not collect data or start a service.

## Data warning

Only `schemas/fixtures/signed-batch-v1.json` and similarly labeled synthetic fixtures may be processed. The doctor reports `SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED`; treating that message as a production-readiness statement is a defect.
