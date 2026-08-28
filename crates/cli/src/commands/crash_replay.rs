//! `academic crash-replay` — the Phase 1 fault matrix as machine-readable data.
//!
//! This command **cannot terminate anything**. Faults are compiled only under
//! the non-default `phase1-fault-injection` feature of the crates that own each
//! failpoint, and even there a fault fires solely when a test harness has set
//! the selection variable in a child process it owns. A production build has no
//! user-accessible crash switch, and adding one here would create exactly the
//! switch the execution contract forbids.
//!
//! What the command does is report, for every enumerated fault, which subsystem
//! owns it, where the process would stop, and the outcome a restart must
//! produce. A harness kills a real daemon by fault identifier and then uses
//! `academic doctor --deep` to check the resulting profile against these rows.

use serde_json::json;

use crate::output::{CliFailure, CommandResult, ExitClass};

/// Outcome a restart must produce after one fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartOutcome {
    /// No canonical reference, and a recoverable temp or orphan.
    NoReference,
    /// A complete sealed object plus a complete canonical transaction.
    Complete,
    /// Explicit quarantine or repair-required disposition.
    Quarantine,
    /// An idempotent retry returns the original receipt.
    IdempotentRetry,
}

impl RestartOutcome {
    /// Returns the stable single-letter code used by the Phase 1 fault matrix.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoReference => "N",
            Self::Complete => "C",
            Self::Quarantine => "Q",
            Self::IdempotentRetry => "R",
        }
    }
}

/// One row of the enumerated fault matrix.
#[derive(Debug, Clone, Copy)]
pub struct FaultRow {
    /// Stable fault identifier.
    pub id: &'static str,
    /// Subsystem that owns the failpoint.
    pub owner: &'static str,
    /// Where the process stops.
    pub termination_point: &'static str,
    /// Outcomes a restart must produce, in matrix order.
    pub outcomes: &'static [RestartOutcome],
    /// What a restart must be able to show.
    pub required_result: &'static str,
}

use RestartOutcome::{Complete, IdempotentRetry, NoReference, Quarantine};

/// The complete enumerated Phase 1 fault matrix.
///
/// The order and spelling are frozen: `academic-test-support` holds the same
/// 26 identifiers for the process harness, and `tests/cli.rs` asserts this
/// catalog against that list so the two cannot drift apart.
pub const FAULT_MATRIX: &[FaultRow] = &[
    FaultRow {
        id: "V01",
        owner: "vault",
        termination_point: "temp object created, before first bytes",
        outcomes: &[NoReference],
        required_result: "expired empty temp removed",
    },
    FaultRow {
        id: "V02",
        owner: "vault",
        termination_point: "middle of stream write",
        outcomes: &[NoReference],
        required_result: "a partial never appears sealed",
    },
    FaultRow {
        id: "V03",
        owner: "vault",
        termination_point: "after temp file sync",
        outcomes: &[NoReference],
        required_result: "temp reusable only through a fresh verified ingest",
    },
    FaultRow {
        id: "V04",
        owner: "vault",
        termination_point: "after destination directory sync, before rename",
        outcomes: &[NoReference],
        required_result: "no canonical reference",
    },
    FaultRow {
        id: "V05",
        owner: "vault",
        termination_point: "after rename, before final directory sync",
        outcomes: &[NoReference],
        required_result: "no reference, or a valid orphan; never a database reference",
    },
    FaultRow {
        id: "V06",
        owner: "vault/store",
        termination_point: "after read-back and sealed receipt, before database begin",
        outcomes: &[Quarantine],
        required_result: "valid orphan adopted on retry, or quarantined after the grace window",
    },
    FaultRow {
        id: "DB01",
        owner: "store",
        termination_point: "immediately after BEGIN IMMEDIATE",
        outcomes: &[NoReference],
        required_result: "rollback with no sequence consumed",
    },
    FaultRow {
        id: "DB02",
        owner: "store",
        termination_point: "after batch and idempotency provisional inserts",
        outcomes: &[NoReference],
        required_result: "the entire transaction is absent",
    },
    FaultRow {
        id: "DB03",
        owner: "store",
        termination_point: "midway through event and normalized inserts",
        outcomes: &[NoReference],
        required_result: "no partial event range",
    },
    FaultRow {
        id: "DB04",
        owner: "store",
        termination_point: "after descriptor, evidence, and claim closure",
        outcomes: &[NoReference],
        required_result: "a sealed orphan is permitted; the reference is absent",
    },
    FaultRow {
        id: "DB05",
        owner: "store",
        termination_point: "after projection outbox insert",
        outcomes: &[NoReference],
        required_result: "the outbox cannot lead the canonical commit",
    },
    FaultRow {
        id: "DB06",
        owner: "store",
        termination_point: "after device head and revision update, before commit",
        outcomes: &[NoReference],
        required_result: "the old head and revision are retained",
    },
    FaultRow {
        id: "DB07",
        owner: "store/daemon",
        termination_point: "after commit, before response write",
        outcomes: &[Complete, IdempotentRetry],
        required_result: "a retry returns the exact stored receipt",
    },
    FaultRow {
        id: "PR01",
        owner: "projections",
        termination_point: "midway writing a BUILDING generation",
        outcomes: &[NoReference],
        required_result: "the old active generation remains and the partial is removable",
    },
    FaultRow {
        id: "PR02",
        owner: "projections",
        termination_point: "after checksum, before activation",
        outcomes: &[NoReference],
        required_result: "the old active remains; a verified inactive generation may be resumed or removed",
    },
    FaultRow {
        id: "PR03",
        owner: "projections",
        termination_point: "during the activation and cursor transaction",
        outcomes: &[NoReference],
        required_result: "the old or new pointer and cursor agree atomically",
    },
    FaultRow {
        id: "BK01",
        owner: "portability",
        termination_point: "midway through the SQLite Online Backup",
        outcomes: &[NoReference],
        required_result: "the unpublished backup temp is removed and the source is unchanged",
    },
    FaultRow {
        id: "BK02",
        owner: "portability",
        termination_point: "database snapshot complete, before object copy",
        outcomes: &[NoReference],
        required_result: "an incomplete unpublished backup is rejected",
    },
    FaultRow {
        id: "BK03",
        owner: "portability",
        termination_point: "midway through the reachable-object copy",
        outcomes: &[NoReference],
        required_result: "the manifest is absent and the incomplete backup is rejected",
    },
    FaultRow {
        id: "BK04",
        owner: "portability",
        termination_point: "manifest temp synced, before the publish rename",
        outcomes: &[NoReference],
        required_result: "only the old backup or a complete new backup exists",
    },
    FaultRow {
        id: "RS01",
        owner: "portability",
        termination_point: "empty destination and marker created",
        outcomes: &[NoReference],
        required_result: "an incomplete destination is recognizable and safely removable",
    },
    FaultRow {
        id: "RS02",
        owner: "portability",
        termination_point: "database copied, before integrity and ledger checks",
        outcomes: &[NoReference],
        required_result: "the destination is not publishable",
    },
    FaultRow {
        id: "RS03",
        owner: "portability",
        termination_point: "objects copied, before closure checks and projection rebuild",
        outcomes: &[NoReference],
        required_result: "the destination is not publishable and the source is untouched",
    },
    FaultRow {
        id: "RS04",
        owner: "portability",
        termination_point: "all checks pass, before the final directory publish",
        outcomes: &[NoReference],
        required_result: "an unpublished verified temp, or a complete published profile",
    },
    FaultRow {
        id: "IPC01",
        owner: "daemon",
        termination_point: "complete request read, before queue admission",
        outcomes: &[NoReference],
        required_result: "no write; a client retry is a new admission with the same key",
    },
    FaultRow {
        id: "IPC02",
        owner: "daemon/store",
        termination_point: "writer commit complete, before the IPC response",
        outcomes: &[Complete, IdempotentRetry],
        required_result: "the exact receipt is returned on reconnect",
    },
];

/// Returns whether this build compiled any fault-injection lane.
///
/// It is always `false` for a default product build. The CLI declares no
/// fault-injection feature of its own, so this can never become `true` by way
/// of a flag or an environment variable.
#[must_use]
pub const fn injection_available() -> bool {
    false
}

fn row_json(row: &FaultRow) -> serde_json::Value {
    json!({
        "id": row.id,
        "owner": row.owner,
        "termination_point": row.termination_point,
        "required_restart_outcomes": row
            .outcomes
            .iter()
            .map(|outcome| outcome.code())
            .collect::<Vec<_>>(),
        "required_result": row.required_result,
        "injectable_by_this_build": injection_available(),
    })
}

/// Reports the whole matrix, or one row selected by `--fault`.
pub fn run(fault: Option<&str>, all: bool) -> CommandResult {
    let rows: Vec<&FaultRow> = match (fault, all) {
        (Some(id), false) => {
            let id = id.to_ascii_uppercase();
            let Some(row) = FAULT_MATRIX.iter().find(|row| row.id == id) else {
                return Err(CliFailure::new(
                    ExitClass::Incompatible,
                    "UNKNOWN_FAULT_ID",
                    format!("{id} is not an enumerated Phase 1 fault identifier"),
                ));
            };
            vec![row]
        }
        (None, true) => FAULT_MATRIX.iter().collect(),
        _ => {
            return Err(CliFailure::new(
                ExitClass::Internal,
                "FAULT_SELECTION_INVALID",
                "exactly one of --fault or --all must be supplied",
            ));
        }
    };

    Ok(json!({
        "selection": if all { "ALL" } else { "SINGLE" },
        "fault_count": rows.len(),
        "matrix_size": FAULT_MATRIX.len(),
        "injection_available": injection_available(),
        "injection_note": "faults compile only under the non-default phase1-fault-injection \
                           feature of the owning crate and fire only for a harness-owned child \
                           process; this binary contains no crash switch",
        "faults": rows.iter().map(|row| row_json(row)).collect::<Vec<_>>(),
    }))
}

/// Renders the human lines for `crash-replay`.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    let mut lines = vec![
        "Academic OS Phase 1 fault matrix".to_owned(),
        format!(
            "faults reported: {} of {}",
            value["fault_count"], value["matrix_size"]
        ),
        format!(
            "fault injection available in this build: {}",
            value["injection_available"]
        ),
    ];
    if let Some(faults) = value["faults"].as_array() {
        for fault in faults {
            lines.push(format!(
                "- {} [{}] {} -> {} ({})",
                fault["id"].as_str().unwrap_or(""),
                fault["owner"].as_str().unwrap_or(""),
                fault["termination_point"].as_str().unwrap_or(""),
                fault["required_restart_outcomes"]
                    .as_array()
                    .map(|codes| codes
                        .iter()
                        .filter_map(|code| code.as_str())
                        .collect::<Vec<_>>()
                        .join("+"))
                    .unwrap_or_default(),
                fault["required_result"].as_str().unwrap_or("")
            ));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn the_matrix_matches_the_enumerated_phase_one_fault_contract() {
        let ordered = FAULT_MATRIX.iter().map(|row| row.id).collect::<Vec<_>>();
        assert_eq!(
            ordered,
            [
                "V01", "V02", "V03", "V04", "V05", "V06", "DB01", "DB02", "DB03", "DB04", "DB05",
                "DB06", "DB07", "PR01", "PR02", "PR03", "BK01", "BK02", "BK03", "BK04", "RS01",
                "RS02", "RS03", "RS04", "IPC01", "IPC02",
            ]
        );
        assert_eq!(ordered.iter().collect::<BTreeSet<_>>().len(), 26);
    }

    #[test]
    fn every_row_states_an_owner_a_point_and_an_outcome() {
        for row in FAULT_MATRIX {
            assert!(!row.owner.is_empty(), "{}", row.id);
            assert!(!row.termination_point.is_empty(), "{}", row.id);
            assert!(!row.required_result.is_empty(), "{}", row.id);
            assert!(!row.outcomes.is_empty(), "{}", row.id);
        }
    }

    #[test]
    fn the_two_post_commit_faults_require_an_idempotent_retry() {
        for id in ["DB07", "IPC02"] {
            let row = FAULT_MATRIX
                .iter()
                .find(|row| row.id == id)
                .map(|row| row.outcomes);
            assert_eq!(row, Some([Complete, IdempotentRetry].as_slice()), "{id}");
        }
    }

    #[test]
    fn this_build_can_never_inject_a_fault() {
        assert!(!injection_available());
    }
}
