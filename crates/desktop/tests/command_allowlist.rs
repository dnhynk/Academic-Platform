//! The typed local-core command allowlist, compared against the wire contract.
//!
//! `academic_rpc` owns which capabilities the daemon negotiates and which of
//! them may carry a write. This suite compares the desktop's closed enum
//! against those tables in both directions, so neither side can grow a
//! capability the other has not seen.

use academic_desktop::{DesktopCommand, SyntheticFixtureId, capability_ids};
use academic_rpc::{
    PHASE1_CAPABILITY_IDS, READ_ONLY_CAPABILITY_IDS, WRITE_CAPABILITY_IDS,
    expected_capability_for_command,
};

fn sorted(names: &[&'static str]) -> Vec<&'static str> {
    let mut owned = names.to_vec();
    owned.sort_unstable();
    owned
}

/// The desktop can name every negotiated capability, and no other.
#[test]
fn desktop_command_allowlist_equals_the_negotiated_capabilities() {
    assert_eq!(
        capability_ids(),
        sorted(PHASE1_CAPABILITY_IDS),
        "the desktop allowlist and the negotiated capability set have diverged"
    );
}

/// Each variant is issued under the capability the daemon expects for it.
#[test]
fn every_write_command_binds_the_capability_the_daemon_expects() {
    for command in DesktopCommand::ALL {
        match command.mutable_command() {
            Some(wire) => {
                assert!(
                    command.is_write(),
                    "{command:?} carries a write arm and is not a write"
                );
                assert_eq!(
                    command.capability_id(),
                    expected_capability_for_command(&wire),
                    "{command:?} is issued under a capability the daemon does not bind to it"
                );
                assert!(
                    WRITE_CAPABILITY_IDS.contains(&command.capability_id()),
                    "{command:?} writes under a capability that is not a write capability"
                );
            }
            None => {
                assert!(
                    !command.is_write(),
                    "{command:?} is a write with no write arm"
                );
                assert!(
                    READ_ONLY_CAPABILITY_IDS.contains(&command.capability_id()),
                    "{command:?} carries no write arm under a write capability"
                );
            }
        }
    }
}

/// The read and write halves partition the allowlist exactly.
#[test]
fn the_allowlist_partitions_into_the_daemon_s_two_halves() {
    let mut reads: Vec<&'static str> = DesktopCommand::ALL
        .iter()
        .filter(|command| !command.is_write())
        .map(|command| command.capability_id())
        .collect();
    let mut writes: Vec<&'static str> = DesktopCommand::ALL
        .iter()
        .filter(|command| command.is_write())
        .map(|command| command.capability_id())
        .collect();
    reads.sort_unstable();
    reads.dedup();
    writes.sort_unstable();
    writes.dedup();

    assert_eq!(reads, sorted(READ_ONLY_CAPABILITY_IDS));
    assert_eq!(writes, sorted(WRITE_CAPABILITY_IDS));
    assert!(
        reads.iter().all(|read| !writes.contains(read)),
        "a capability is on both halves"
    );
}

/// Every variant is represented in `ALL`, so the enumeration above is complete.
///
/// The match is exhaustive by the compiler, so adding a variant without adding
/// it here stops compiling rather than silently shrinking every test that
/// iterates `ALL`.
#[test]
fn every_variant_is_enumerated() {
    for command in DesktopCommand::ALL {
        let named = match command {
            DesktopCommand::Diagnostics => "Diagnostics",
            DesktopCommand::SyntheticExport => "SyntheticExport",
            DesktopCommand::SyntheticIngest(_) => "SyntheticIngest",
            DesktopCommand::SyntheticBackup => "SyntheticBackup",
            DesktopCommand::SyntheticRestore { .. } => "SyntheticRestore",
        };
        assert!(!named.is_empty());
    }
    let distinct: std::collections::BTreeSet<&'static str> = DesktopCommand::ALL
        .iter()
        .map(|command| command.capability_id())
        .collect();
    assert_eq!(
        distinct.len(),
        DesktopCommand::ALL.len(),
        "two variants share a capability, so `ALL` does not enumerate the variants"
    );
}

/// The fixture the desktop can name is the one the ingest command carries.
#[test]
fn the_fixture_allowlist_is_closed() {
    assert_eq!(SyntheticFixtureId::ALL.len(), 1);
    for fixture in SyntheticFixtureId::ALL {
        let command = DesktopCommand::SyntheticIngest(*fixture);
        let built = command.mutable_command();
        let carried = match &built {
            Some(academic_rpc::generated::mutable_request::Command::SyntheticIngest(ingest)) => {
                Some(ingest.synthetic_fixture_id.as_str())
            }
            _ => None,
        };
        assert_eq!(
            carried,
            Some(fixture.as_str()),
            "ingest built {built:?} instead of naming {}",
            fixture.as_str()
        );
    }
}
