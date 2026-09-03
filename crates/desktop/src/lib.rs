//! The desktop surface's side of the local-core contract.
//!
//! ADR-001 constrains this surface: it may unlock, ingest, correct, review
//! evidence, back up, restore and approve policy, and it must not open the
//! database, hold a provider or root key, or take unrestricted filesystem or
//! network authority. Two of those are enforced here rather than described.
//!
//! **The command allowlist is typed.** [`DesktopCommand`] is a closed enum. It
//! is not constructible from a string, and every variant names one capability
//! from [`academic_rpc::PHASE1_CAPABILITY_IDS`]. The two lists are compared in
//! both directions by `tests/command_allowlist.rs`, so a capability the daemon
//! negotiates and the desktop cannot name is a failure, and so is a command the
//! desktop can name and the daemon does not negotiate.
//!
//! **An optimistic update is not canonical until a receipt says so.**
//! [`Optimistic<T>`](optimistic::Optimistic) has no accessor, no conversion and
//! no `Serialize`; the one exit is
//! [`Optimistic::confirm`](optimistic::Optimistic::confirm), which takes an
//! [`ImmutableReceipt`](academic_rpc::generated::ImmutableReceipt) and compares
//! every field the core bound the request to.
//!
//! **What this crate is not.** It links no Tauri runtime and opens no window.
//! `crates/desktop/tauri.conf.json` and `crates/desktop/capabilities/` are the
//! committed capability and CSP snapshot; `packages/ui` is the route manifest,
//! palette, backlinks and evidence drawer; `docs/contracts/desktop-shell.md`
//! states what each of those is and is not evidence for. This crate opens no
//! socket, declares no foreign function, reads no environment variable, spawns
//! no process and runs no build script, and it has no dependency edge of any
//! kind to `academic-store`, `academic-vault` or `academic-crypto` --
//! `desktop_cannot_open_the_database_or_read_keys` in
//! `tools/phase1-scaffold-policy.test.mjs` judges that from the Cargo graph,
//! the resolved link closure and the source text together.

pub mod command;
pub mod optimistic;

pub use command::{DesktopCommand, SyntheticFixtureId, capability_ids};
pub use optimistic::{Canonical, NotCanonical, Optimistic, SubmittedRequest};
