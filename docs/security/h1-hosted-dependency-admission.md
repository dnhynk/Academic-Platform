# H1 unsigned hosted evidence dependency admission

Owner: encrypted-store/H1 preparation maintainers. Reviewed 2026-09-08 for the
synthetic, public CI evidence boundary only. No product dependency or locked
Cargo/pnpm resolution changes. Independent distribution/legal/security acceptance
and ADR-002 remain open.

## Artifact action

The sole new action is official `actions/upload-artifact` **v7.0.0**, commit
`bbbca2ddaa5d8feaa63e36b76fdaad77386f024f`. The official Git ref API resolves that
tag directly to this commit. Reviewed primary bytes: `action.yml`, `README.md`,
`package.json`, `package-lock.json`, and `LICENSE` at that commit, plus the
repository's inherited organization security policy. There is no root-level
`SECURITY.md` in that commit; the failed lookup was retained during review.
The action runs its packaged `dist/upload/index.js` with Node 24; CI does not
install or resolve its npm development dependencies.

| Primary byte reference | SHA-256 / review |
| --- | --- |
| [Action definition](https://github.com/actions/upload-artifact/blob/bbbca2ddaa5d8feaa63e36b76fdaad77386f024f/action.yml) | `c5979822866a72362e609844b6ebe77d4b7e759af68cc1c2c425dcf51481fab4` |
| [MIT licence](https://github.com/actions/upload-artifact/blob/bbbca2ddaa5d8feaa63e36b76fdaad77386f024f/LICENSE) | `3e855ffa704114a51628ef8f0bf3aeb41728adf9d9070e263bf58aa5640b0eb5` |
| [Version and immutable artifact behavior](https://github.com/actions/upload-artifact/tree/bbbca2ddaa5d8feaa63e36b76fdaad77386f024f) | Official GitHub implementation; archive ID/digest/URL outputs; archive input defaults true |
| [Advisory/reporting route](https://github.com/actions/upload-artifact/security/policy) | Inherited GitHub security policy; owner reviews advisories through dependency admission, never floating-tag updates |

Reason: preserve downloadable failed and successful native test observations,
which console-only CI currently loses. Trust boundary: the official action can
read the runner filesystem and upload via GitHub's job-scoped artifact service;
the reviewed workflow restricts the configured path to one fresh synthetic
evidence directory. Hidden files and overwrite are explicitly false. Each name
contains platform, run ID, attempt and exact checkout SHA; retention is 30 days.
Immutable content does not mean permanent retention: expiration, run/repository
deletion and authorized artifact deletion remain possible. Retrieve archives,
API metadata/digests and validation output before expiry. There is no release,
attestation, signing, OIDC, personal secret, private key or product upload.

Existing checkout and setup-node actions retain their admitted full SHAs and
checkout disables persisted credentials. Permissions remain `contents: read`.
Workflow policy rejects any extra job/step, reference drift, upload scope drift,
write privilege, hidden-file upload or overwrite. No new npm/Cargo dependency
is needed by the collector or validator; both use Node built-ins and existing
restricted YAML/source-policy parsers.

## Native prerequisites

Rust 1.98.0 and Node 24.19.0 retain the repository toolchain pins. Cargo fetch
runs only after source preflight, with `--locked`; all build/test invocations
then use `--locked --offline`, one build job and no incremental compilation.
Each job owns a new target/temp directory. There is no registry/build cache
shared across platforms and no default-SQLite substitution.

Windows uses the already admitted
[Strawberry Perl pin](../../tools/sqlcipher/windows-toolchain.json): archive
5.42.2.1, 304301401 bytes, SHA-256
`32d83be90cf04b807cfb9477482bc36302cdee6f5b04cf57e81adecbd8f07898`.
The collector retains setup stdout/stderr and exit status. Size and digest are
checked before extraction; identity/modules are checked against every existing
pin by `h1-windows-toolchain.mjs`. The original local verifier requires a fixed
user-machine `D:` path. The hosted variant deliberately uses
`RUNNER_TEMP/h1-perl` and `RUNNER_TEMP/h1-perl-download` on the runner's own
volume (the observed ARM workspace is on `C:`). It changes no archive, digest,
version, module, interpreter architecture or product toolchain pin, and never
creates a substitute drive or changes the existing local verifier.
Only `OPENSSL_SRC_PERL` points to the interpreter; its bundled C compiler and
NASM directories are not added to PATH. Upstream's
[`openssl-src` implementation](https://github.com/alexcrichton/openssl-src-rs/blob/300.6.1%2B3.6.3/src/lib.rs)
supports `OPENSSL_RUST_USE_NASM=0`, which the Windows collector sets to preserve
the existing no-assembly contract even if a hosted image supplies NASM. This
changes implementation optimization only and selects no different cipher,
key derivation, parameters or source dependency. Upstream's
[pinned OpenSSL Windows notes](https://github.com/openssl/openssl/blob/openssl-3.6.3/NOTES-WINDOWS.md)
document native Perl, MSVC and the ARM configuration. The
[Microsoft toolchain reference](https://learn.microsoft.com/en-us/cpp/build/building-on-the-command-line?view=msvc-170)
describes architecture-specific build environments. The hosted image supplies
MSVC, SDK and build tools; actual compiler/build errors are evidence, not waived.
The x64 Perl interpreter can run under Windows ARM's compatibility layer;
that is a build prerequisite observation, never evidence of a native product.
Node OS/architecture, runner architecture, rustc host triple, and retained probe
PE/ELF/Mach-O architecture must agree with the matrix target.

Linux/macOS use image-provided Perl and C/build toolchains; no new apt, brew or
third-party setup action is introduced. `perl -V`, Rust/Cargo versions, OS release,
image name/version and raw native compilation diagnostics are retained. The
[official hosted-runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
documents the five selected labels. Hosted image labels are moving provider
inputs; observed image versions are evidence, not a claim of a hermetic OS image.

The collector copies SQLCipher and OpenSSL licence bytes from the exact fetched
registry source directories and checks their existing admission SHA-256 values.
SQLCipher/SQLite versions are runtime probe observations; the OpenSSL version
is explicitly a **locked source version**, not an unobserved provider API query.
Missing notices fail the completed component bundle validation. Build tools
remain outside the product dependency graph. Their existing notices/admission
are referenced here, not replaced by a new distribution approval.
