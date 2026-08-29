# Native Windows toolchain for the encrypted store lane

Building `academic-store --features sqlcipher-store` on Windows compiles
SQLCipher against OpenSSL, and `openssl-src` configures OpenSSL by running
`perl Configure VC-WIN64A`. That step needs a **native Windows Perl**. This file
pins exactly which one, so the build depends on a recorded artefact rather than
on whatever happens to be on a developer's `PATH`.

The machine-readable form of every pin below is
[`windows-toolchain.json`](windows-toolchain.json), and
[`verify-windows-toolchain.mjs`](verify-windows-toolchain.mjs) enforces it.

## What this replaces

The E1 spike stopped here. The only Perl on the evidence machine was
Git-for-Windows' Cygwin build, whose trimmed core omits
`Locale::Maketext::Simple`; `Configure` reaches it through
`Params/Check.pm` → `IPC/Cmd.pm` → `OpenSSL/config.pm` → `Configure:23`:

```text
Can't locate Locale/Maketext/Simple.pm in @INC (@INC entries checked:
 /usr/lib/perl5/site_perl ... /usr/share/perl5/core_perl) at
 /usr/share/perl5/core_perl/Params/Check.pm line 6.
```

t068 section 8.2 requires that be resolved "with a documented, pinned toolchain
rather than an ad-hoc `PATH` change". Vendoring the missing pure-Perl modules
and pointing `PERL5LIB` at them is the ad-hoc route, and it does not work
anyway: a Cygwin Perl emits `/d/…` paths that `nmake` cannot consume.

## The pin

| Field | Value |
| --- | --- |
| Component | Strawberry Perl, 64-bit portable |
| Version | `5.42.2.1` (`v5.42.2`, `MSWin32-x64-multi-thread`) |
| Archive | `strawberry-perl-5.42.2.1-64bit-portable.zip` |
| Size | `304301401` bytes |
| SHA-256 | `32d83be90cf04b807cfb9477482bc36302cdee6f5b04cf57e81adecbd8f07898` |
| Install root | `D:\toolchains\strawberry-perl-5.42.2.1-64bit-portable` |
| Referenced by | `OPENSSL_SRC_PERL` only |
| On `PATH` | never |

The SHA-256 was taken from two independent publishers before the archive was
used, and both agree on the digest and the byte count:

- `https://strawberryperl.com/releases.json`, field `edition.portable.sha256`
- the GitHub release asset metadata for tag `SP_54221_64bit`, field
  `assets[].digest`

## Procedure

1. Read the pinned digest from both sources above and confirm they agree with
   each other and with this file.
2. Download the archive.
3. **Verify size and SHA-256 before extracting.** A mismatch means delete the
   archive and stop; it does not mean retry.
4. Extract to the pinned install root.
5. Set `OPENSSL_SRC_PERL` to `<install root>\perl\bin\perl.exe` for the build
   command only. Do not add anything to `PATH`, and do not set `PERL`.
6. Run `node tools/sqlcipher/verify-windows-toolchain.mjs`. It exits 0 with a
   note on a host that is not using the lane, and fails on any drift once
   `OPENSSL_SRC_PERL` is set.

```powershell
$env:OPENSSL_SRC_PERL = "D:\toolchains\strawberry-perl-5.42.2.1-64bit-portable\perl\bin\perl.exe"
node tools/sqlcipher/verify-windows-toolchain.mjs
cargo test -p academic-store --no-default-features --features sqlcipher-store --locked --offline
```

## OpenSSL is built without assembly, deliberately

`openssl-src` looks for `nasm.exe` with `where nasm` and passes `no-asm` to
`Configure` when it finds none. Nothing puts `nasm.exe` on `PATH` here, so the
Windows OpenSSL is a `no-asm` build.

This distribution does ship a `nasm.exe` in `c\bin`. Using it would mean putting
that directory on `PATH` — the ad-hoc change section 8.2 rejects — so it stays
unused. What changes is which implementation of each primitive runs, not which
primitives: the cipher, key derivation, iteration count, and HMAC are identical,
and the cipher settings are read back and asserted at every open regardless.
Performance figures from this build are therefore not comparable to the Linux
lane's.

## Boundary

This is a build toolchain, not a dependency. It appears in no `Cargo.lock`, no
`package.json` dependency set, and no product feature graph, and it changes
nothing about what the product links. Removing the install root removes the
ability to build the encrypted lane on Windows and nothing else.
