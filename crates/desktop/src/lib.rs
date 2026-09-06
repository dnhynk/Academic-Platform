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
//! The optional `desktop-runtime` feature starts the bundled Tauri window and
//! sends the existing versioned RPC over local endpoints. Default builds keep
//! only these typed contracts. Neither lane links a store or key owner;
//! see `docs/contracts/desktop-shell.md` for the measured boundary.

pub mod command;
#[cfg(feature = "desktop-runtime")]
mod local_client;
pub mod optimistic;
#[cfg(feature = "desktop-runtime")]
pub mod runtime;

pub use command::{DesktopCommand, SyntheticFixtureId, capability_ids};
pub use optimistic::{Canonical, NotCanonical, Optimistic, SubmittedRequest};
