# Phase 2 P2-C7 dependency admission

This receipt admits the `trybuild` compile-fail harness and its transitive
closure, which the projected/actual type isolation is proved with. It is not a
production security approval and it does not admit real data:
`GATE-P2-ADMISSION` is still closed, ADR-002 is still unaccepted, and
`adr_002_accepted` remains `false`.

Exact versions, registry checksums, licenses, minimum Rust declarations,
repositories, and a per-crate trust-boundary justification are in
`dependency-admission-phase2-c7.json`, which `pnpm security` reads.

## What was added

| Group | Crates | Of those, linked into the binary | Build-time only |
|---|---:|---:|---:|
| Compile-fail harness (`trybuild`) | 8 | 0 | 8 |

`trybuild` is the one direct workspace dependency, pinned to `=1.0.120` as a
dev-dependency of `academic-scenario` alone. The other seven — `glob`,
`serde_spanned`, `target-triple`, `termcolor`, `toml`, `toml_writer`, and
`winapi-util` — are its transitive closure.

`toml_datetime`, `toml_parser`, `winnow`, and `serde_core` are also in that
closure and are **not** re-admitted here: they are already in the lock through
the `toml_edit` admitted by `P2-K1`, and `toml 1.1.4` resolves to the same
`toml-rs` generation, so no second TOML parser enters the graph.

One workspace path package is added: `academic-scenario`. It has no registry
source or checksum.

No npm package was added, so `pnpm-lock.yaml` keeps its reviewed graph and adds
no lifecycle or install script.

## Why this dependency belongs inside its trust boundary

The projected/actual isolation is a claim about what will *not* compile.
Asserting it in prose leaves nothing that fails when the seal opens, and
asserting it with a source grep passes for a crate that links the canonical
writer and simply has not named it yet. `trybuild` is what puts the claim in
front of a compiler and pins the resulting diagnostic, so an accidentally added
accessor on `Proposed<T>` becomes a red test rather than a silent regression.

The check has been observed to bite: adding an `into_inner` to `Proposed<T>`
turns `projected_mastery_is_not_readable` and `projected_workload_is_not_an_integer`
into `mismatch`, and removing it again returns the suite to green.

## Boundary

- **Never linked into a product binary.** Every admission is dev-only, owned by
  `academic-scenario`. `dependency_license_and_source_receipt_is_complete`
  asserts each one is absent from the default product package graph, so a later
  edit that promoted one to a shipping dependency fails the gate.
- **No network, no environment secret, no install script.** `trybuild` invokes
  the pinned `cargo` toolchain against a scratch project under
  `CARGO_TARGET_DIR`. CI populates the registry with `cargo fetch --locked`
  before the offline test run, so the harness performs no fetch of its own.
- **One unsafe-FFI leaf.** `winapi-util` is a Windows console-handle helper
  behind `termcolor`. It is the same reviewed FFI pattern already admitted for
  `windows-sys`, in a crate that never ships.
- Rust `1.98.0`, Node `24.19.0`, and pnpm `11.22.0` are unchanged and exact.
  Every crate is taken from `registry+https://github.com/rust-lang/crates.io-index`
  with a recorded checksum; no Git or HTTP source is used.

## Advisory path

RustSec and crates.io advisories, GitHub security advisories, and upstream
release notices, triaged as reviewed commits through this same admission
process. `.github/dependabot.yml` opens no version-update pull request, so no
automatic bump can bypass the review.
