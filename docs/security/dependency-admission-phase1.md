# Phase 1 F0 dependency admission

This receipt admits only dependencies needed to freeze the Phase 1 crate, SQLite, local IPC, platform-adapter, code-generation, and test boundaries. It is not a production security approval. Exact machine-checkable versions, registry checksums, licenses, minimum Rust declarations, owners, targets, and feature uses are in `dependency-admission-phase1.json` and are checked against locked `cargo metadata` by `phase1-scaffold-policy.test.mjs`.

## Tool and source boundary

- Rust `1.98.0`, Node `24.19.0`, and pnpm `11.22.0` are exact.
- All new Rust versions use `=` requirements and the crates.io registry source. Git dependencies, HTTP archives, loadable SQLite extensions, and unreviewed sources remain rejected by the dependency-free preflight.
- No npm package was added. `pnpm-lock.yaml` therefore retains the previously reviewed graph and adds no lifecycle/install script.
- One Cargo resolution is authorized for F0. After that lock update, every install, metadata query, build, test, and documentation command uses `--locked` and, where Cargo accepts it, `--offline`.
- The S1 path-capability delta performs no registry resolution: all 173 incoming `(name, version, source, checksum)` tuples remain exact under the canonical tuple receipt (`SHA-256 4f370a5dd80938b0b6a00de809985f7ff32378a866ec570e13d9b650e7ce01c7`), and the only added lock package is the source-less/checksum-less workspace path package `academic-store-platform 0.1.0`.
- RustSec/crates.io advisories, GitHub security advisories, and upstream release notices are the review channels. An applicable advisory requires a pinned update or a recorded exploitability decision; silent suppression is forbidden.

The Phase 0 direct versions already accepted in the incoming lock are preserved exactly while their compatible-range requirements become exact pins: `anyhow 1.0.104`, `ciborium 0.2.2`, `clap 4.6.6`, `ed25519-dalek 2.2.0`, `hex 0.4.3`, `hmac 0.12.1`, `proptest 1.11.0`, `prost 0.14.1`, `serde 1.0.229`, `serde_json 1.0.151`, `sha2 0.10.9`, `thiserror 2.0.20`, and `uuid 1.25.0`. F0 does not replace those accepted versions with lower manifest minima.

## Direct admissions

| Dependency | Owner and scope | Exact features | License | Declared MSRV | Network/install posture |
|---|---|---|---|---:|---|
| `rusqlite 0.40.2` | `academic-store`, `academic-projections`, `academic-portability` | no defaults; `backup`, `hooks`, `limits`; `bundled` only through default `bundled-sqlite`; vendored SQLCipher only through non-default `sqlcipher-spike` | MIT | not declared upstream; compiled on Rust 1.98 | no network; native build receipt required; no load extension |
| `tokio 1.53.1` | `academic-rpc`, `academic-daemon` | `io-util`, `macros`, `net`, `rt-multi-thread`, `signal`, `sync`, `time` | MIT | 1.71 | `net` is restricted to named pipe/UDS local IPC; no HTTP/TCP/UDP/DNS behavior |
| `windows-sys 0.61.2` | Windows vault/daemon adapters and the isolated store path-capability boundary | explicit WDK Foundation/FileSystem/SystemServices plus Win32 Foundation, Security/Authorization, FileSystem, IO, Pipes, RemoteDesktop session identity, SystemServices, Threading, WindowsProgramming subsets | MIT OR Apache-2.0 | 1.71 | native path, session, volume, ACL, named-pipe, and local filesystem capability only; no WinHTTP/WinInet/WinSock feature admitted |
| `rustix 1.1.4` | Unix vault/daemon adapters and the isolated store path-capability boundary | no defaults; `fs`, `net`, `process`, `std` across admitted owners; store-platform omits `net` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 1.63 | local filesystem, peer identity, and UDS capability only |
| `getrandom 0.4.3` | synthetic vault nonce/locator-key seed and daemon session nonce | no defaults; `std`; no `wasm_js` or custom backend | MIT OR Apache-2.0 | 1.85 | OS entropy only; not a production key hierarchy |
| `prost-build 0.14.1` | RPC build tooling | no defaults | Apache-2.0 | 1.71.1 | build-time only; aligned with frozen `prost 0.14.1` |
| `protoc-bin-vendored 3.2.0` | RPC build tooling | no defaults/features | MIT | not declared upstream; compiled on Rust 1.98 | pinned vendored compiler packages; no runtime dependency |
| `tempfile 3.27.0` | test support and daemon integration tests only | default `getrandom` | MIT OR Apache-2.0 | 1.63 | no product inclusion |
| `assert_cmd 2.2.2` | test support only | no defaults | MIT OR Apache-2.0 | 1.85 | no product inclusion |
| `predicates 3.1.4` | test support only | no defaults | MIT OR Apache-2.0 | 1.74 | no regex/color/default expansion; no product inclusion |

The existing Phase 0 cryptographic and contract dependencies retain their prior owners and features. F0 changes their Cargo requirements from compatible ranges to exact requirements without changing versions.

## Portability use of the already-admitted `backup` feature

`academic-portability` is the third owner of the already-admitted `rusqlite 0.40.2` package and adds no new dependency, feature, or resolution. It selects the same `backup`, `hooks`, and `limits` feature set with `default-features = false` and inherits the bundled plaintext SQLite lane from `academic-store`.

The `backup` feature was admitted in F0 and is now exercised: the Phase 1 backup copies the canonical database with the SQLite Online Backup API into a temporary directory at a fixed commit watermark, instead of copying a live file whose pages may be mid-transaction. Portability also opens its own read-only SQLite connection so it can enumerate canonical rows in canonical-identifier order; the guarded store reader admits the schema first, and the portability connection is immediately constrained to `query_only`. No archive, compression, cloud, or network dependency is added, and the resulting backup remains plaintext and synthetic-only.

`academic-portability` additionally takes an ordinary workspace edge on `academic-contracts` so restore can re-verify every stored signed envelope against an independent device authorization instead of trusting the signing key carried inside the restored bytes. The Phase 0 direct packages `serde`, `serde_json`, `sha2`, and the dev-only `ed25519-dalek` keep their accepted exact versions.

## Reviewed store path-capability boundary

`academic-store-platform` is a private workspace leaf used only by `academic-store`. It reuses the exact admitted `windows-sys 0.61.2` and `rustix 1.1.4` packages and adds no registry tuple, build script, network feature, or process/shell probe. The crate reproduces every workspace Rust/Clippy deny, changes only `unsafe_code` from workspace `forbid` to crate-default `deny`, and permits reviewed unsafe blocks solely on the smallest private Windows FFI functions; its public facade is safe and exposes no raw handle.

The Windows creation DACL is protected and explicit. It grants full access only to the current logon SID and LocalSystem, with LocalSystem narrowly retained for OS backup/recovery and security services; Administrators, Users, Everyone, creator-owner expansion, and inherited parent grants are not admitted. Native verification rejects any different owner, unprotected DACL, inherited ACE, extra trustee, ACE type, or access mask.

## Native and vendored transitive receipt

The machine receipt also pins the native closure selected by the default and spike lanes. Both lanes use `libsqlite3-sys 0.38.2`; default selects its bundled SQLite features and embeds SQLite `3.53.2`, while the explicit spike selects SQLCipher Community `4.14.0` over SQLite `3.51.3` plus `openssl-sys 0.9.117` and `openssl-src 300.6.1+3.6.3` (OpenSSL `3.6.3`). Their registry checksums, licenses, declared MSRVs, and exact per-lane feature sets are checked by the F0 policy test. The SQLCipher attribution license and OpenSSL license bytes are SHA-256 receipted, and both remain unaccepted by ADR-002.

`protoc-bin-vendored 3.2.0` resolves only its eight exact platform crates for Linux aarch64/ppc64le/s390x/x86/x86_64, macOS aarch64/x86_64, and Windows. Every platform crate is registry-checksummed and MIT-licensed in the machine receipt. These build-time packages generate no F0 code and have no product runtime edge.

## Default and SQLCipher lanes

`academic-store` has exactly these features:

```toml
default = ["bundled-sqlite"]
bundled-sqlite = ["rusqlite/bundled"]
sqlcipher-spike = ["rusqlite/bundled-sqlcipher-vendored-openssl"]
```

The default workspace graph must not contain SQLCipher or OpenSSL. The explicit SQLCipher compile check is run for `academic-store` with `--no-default-features --features sqlcipher-spike`; even a pass means only that a spike input compiled on that host. It cannot set `adr_002_accepted=true`, enable a daemon profile, or permit production data.

## Build provenance and limitations

Bundled plaintext SQLite and the explicit SQLCipher spike necessarily use Cargo native build scripts in `libsqlite3-sys` and its selected native toolchain. Those are registry-locked Cargo builds, not npm lifecycle/install scripts. The final F0 report records their exact locked versions, source checksums, selected feature trees, compiler/tool versions, default-vs-spike graphs, and binary checks. SQLCipher Community attribution, five-platform packaging, leakage, crash, rekey, wrong-key, backup, and independent restore evidence remain open under ADR-002.

No dependency adds a cloud client, HTTP stack, TLS stack, DNS resolver, generic RPC framework, database server, dynamic SQLite extension loader, telemetry, recorder, or install-time network behavior. The dependency test fails if a prohibited package enters the default product graph or if source contains product HTTP/TCP/UDP/DNS behavior.
