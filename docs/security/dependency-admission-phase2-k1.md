# Phase 2 P2-K1 dependency admission

This receipt admits the dependencies the ADR-005 key hierarchy and its two
operating-system key brokers need. It is not a production security approval and
it does not admit real data: `GATE-P2-ADMISSION` is still closed, ADR-002 is
still unaccepted, and `adr_002_accepted` remains `false`.

Exact versions, registry checksums, licenses, minimum Rust declarations,
repositories, and a per-crate trust-boundary justification are in
`dependency-admission-phase2-k1.json`, which `pnpm security` reads.

## What was added

| Group | Crates | Of those, linked into the binary | Build-time only |
|---|---:|---:|---:|
| Key schedule and wrap (RustCrypto) | 13 | 12 | 1 |
| Linux Secret Service broker (`zbus`) | 30 | 17 | 13 |
| **Total** | **43** | **29** | **14** |

Two workspace path packages are also added: `academic-crypto` and
`academic-keystore-platform`. Neither has a registry source or checksum.

No npm package was added, so `pnpm-lock.yaml` keeps its reviewed graph and adds
no lifecycle or install script.

### Why 43 and not 42

The count quoted while the Secret Service decision was being taken was 42. The
final number is 43 because `argon2`'s `hash_password_into` — the raw-output KDF
entry point this task needs, as opposed to a PHC string API — is gated behind
that crate's `alloc` feature, and `alloc` pulls `password-hash`. This is a
consequence of the crate's feature layout, not a later change of approach.

### Second async runtime: none

`zbus` is taken with `default-features = false` and `features = ["tokio"]`. The
resolved Linux graph contains no `async-io`, `smol`, `async-global-executor`,
`async-executor`, `async-std`, `blocking`, `polling`, or `async-task`. The only
reactor in the workspace remains the already-admitted `tokio`. `futures-lite`,
`parking`, and `event-listener` do appear; they are synchronisation primitives
and combinators, not executors.

## Tool and source boundary

- Rust `1.98.0`, Node `24.19.0`, and pnpm `11.22.0` are unchanged and exact.
- Every new Rust requirement uses an `=` pin and the crates.io registry source.
  Git dependencies and insecure package tarballs stay rejected by the
  dependency-free `node tools/source-preflight.mjs` before any fetch, install,
  build, or test, and are rechecked by `pnpm security`.
- One Cargo resolution is authorized for this task. After that lock update every
  build, test, and metadata command uses `--locked` and, where Cargo accepts it,
  `--offline`.
- Licenses are `MIT`, `Apache-2.0`, or the dual `MIT OR Apache-2.0` across all
  43 crates. Nothing copyleft and nothing unlicensed was admitted.
- Advisories are triaged as reviewed commits through this same process.
  `.github/dependabot.yml` opens no version-update pull request, so no automatic
  bump can bypass the review.

## One trait generation, not two

`hkdf 0.12.4`, `argon2 0.5.3`, `blake2 0.10.6`, and `chacha20poly1305 0.10.1`
were pinned to the versions that share the `digest 0.10` / `crypto-common 0.1`
generation the already-admitted `sha2 0.10.9` and `hmac 0.12.1` use. The newer
`hkdf 0.13` / `blake2 0.11` line would have brought a second copy of the whole
RustCrypto trait stack into the graph. The resolved lock contains one.

## Network posture

No dependency admitted here opens an outbound socket. `zbus` speaks only to the
session D-Bus over a local Unix-domain socket owned by the user, which is the
same local-IPC posture already admitted for `tokio` and `rustix`. The product
graph still has no crate that can reach the network.

`tracing` enters the graph as `zbus`'s logging facade. This workspace installs
no subscriber, so its records go nowhere; independently of that, the key broker
passes no key material to it, which `no_key_bytes_in_logs_audit_or_export`
asserts against every rendered error and key type.

## Feature review of already-admitted packages

| Package | Added | Why |
|---|---|---|
| `windows-sys 0.61.2` | `Win32_Security_Cryptography` | `NCryptCreateProtectionDescriptor`, `NCryptProtectSecret`, `NCryptUnprotectSecret`, `NCryptFreeBuffer` for the DPAPI-CNG broker. No networking API is reachable through this feature. |
| `tokio 1.53.1` | `rt`, on `cfg(target_os = "linux")` only | The keystore leaf runs each Secret Service conversation on a dedicated thread with its own current-thread runtime, so the facade stays callable both from a synchronous caller and from inside the daemon's runtime. No new socket. |
| `zeroize 1.9.0` | `alloc`, `derive` (promoted from transitive to direct) | The zeroization boundary ADR-005 requires; `derive` supplies the `ZeroizeOnDrop` bound the key types assert. |
| `subtle 2.6.1` | promoted from transitive to direct | Constant-time recipient-record MAC comparison, required by `KY06`. |

## Where the new code sits

`academic-keystore-platform` is the second reviewed native FFI boundary after
`academic-store-platform` and follows the same pattern: the crate overrides the
workspace's `unsafe_code = "forbid"` to `deny`, every `unsafe` block sits in a
small private function carrying `#[allow(unsafe_code)]` and a concrete safety
argument, and the public facade exposes no raw handle, pointer, descriptor, or
D-Bus object. `keystore_leaf_public_facade_exposes_no_raw_handle` asserts that
last property against the crate's own source.

The Linux half of that crate contains no `unsafe` at all: `zbus` is safe Rust.
All six `unsafe` blocks are on the Windows side in `windows.rs`, each inside its
own private function carrying `#[allow(unsafe_code)]` — six blocks, six
allow sites, six safety arguments.

`academic-crypto` inherits the workspace lints unchanged, including
`unsafe_code = "forbid"`.
