//! Reconciles a model run's recorded transmission against `egress_audit`.
//!
//! # Why this keys on `egress_consumption` and not on `egress_audit.grant_id`
//!
//! `egress_audit.grant_id` carries identifiers from two tables. `P2-G7` removed
//! the foreign key so process-activity rows could be written at all, and `T146`
//! measured that the typed `(process_class, capability)` pair does not
//! discriminate them: `EGRESS_PROXY` x `OPEN_OUTBOUND_SOCKET` is the cell where
//! the two namespaces overlap exactly, and it is the cell egress auditing cares
//! most about. Three consecutive allow rows with identical decision, class and
//! capability carried identifiers from both namespaces in the same 64-hex
//! shape.
//!
//! A reconciliation that keys on `grant_id` alone therefore matches a
//! process-capability token as readily as the egress grant it was looking for,
//! and reports agreement it never checked.
//!
//! `egress_consumption` is what resolves it, and no new column was needed.
//! `T149` measured the two foreign keys that table already carries:
//! `grant_id` references `egress_grant`, and `(egress_audit_seq, grant_id)`
//! references `egress_audit(audit_seq, grant_id)`. Together they mean a
//! consumption row names an audit row whose identifier is a real egress grant,
//! so coming through this join there is no polymorphism left to resolve.
//! `crates/policy/tests/consumption_join.rs` is the observation that both keys
//! hold; `an_audit_row_from_the_other_namespace_is_not_the_grant` is the
//! observation that it matters here, and it runs a
//! `grant_id`-only reconciliation beside this one so the difference between
//! them is executed rather than described.
//!
//! `execute` writes the consumption row in the same transaction as the allow
//! audit it names, so the row this reaches is the transmission and not the
//! decision that minted the grant.

use std::collections::BTreeSet;

use academic_policy::{AuditRow, ConsumptionRow, Decision};

use crate::record::{ModelRun, Transmission, TransmittedRange};

/// Why a recorded transmission and the egress audit do not agree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReconciliationError {
    /// Nothing consumed the grant the run named.
    ///
    /// This is the failure the join exists for. An identifier from the
    /// process-capability namespace cannot appear in `egress_consumption` at
    /// all, so a run naming one lands here rather than on a row that merely
    /// spells the same 64 hex characters.
    #[error("no consumption record spends grant {0}")]
    GrantNotConsumed(String),
    /// The consumption names an audit sequence the projection does not carry.
    #[error("consumption of grant {0} names an audit row that is not projected")]
    ConsumptionAuditMissing(String),
    /// More than one consumption claims the grant.
    #[error("egress grant {0} is claimed by more than one consumption record")]
    MultipleAuditRows(String),
    /// The grant's audit row is a denial, so no bytes were authorized.
    #[error("egress grant {0} was audited as a denial")]
    GrantDenied(String),
    /// The audited ranges are not the recorded ranges.
    #[error("recorded ranges and audited ranges differ for egress grant {0}")]
    RangesDiffer(String),
    /// The audited byte count is not the sum of the recorded ranges.
    #[error("recorded {recorded} bytes, audit records {audited}, for egress grant {grant_id}")]
    ByteCountDiffers {
        /// Grant the run named.
        grant_id: String,
        /// Bytes the run's own ranges cover.
        recorded: u64,
        /// Bytes the audit row records.
        audited: u64,
    },
    /// A run that declared itself local-only has input bytes in an egress audit.
    #[error("local-only run transmitted the bytes of artifact digest {0}")]
    LocalOnlyRunTransmitted(String),
}

/// What a successful reconciliation establishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    grant_id: Option<String>,
    audit_seq: Option<u64>,
    byte_count: u64,
}

impl Reconciliation {
    /// The egress grant the transfer spent, when one was.
    #[must_use]
    pub fn grant_id(&self) -> Option<&str> {
        self.grant_id.as_deref()
    }

    /// The audit row the run reconciled against, when one exists.
    #[must_use]
    pub const fn audit_seq(&self) -> Option<u64> {
        self.audit_seq
    }

    /// Bytes both records agree left the machine.
    #[must_use]
    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }
}

/// Reconciles one model run's recorded transmission against the egress audit.
///
/// Total in both directions. An `EGRESSED` run must have exactly one
/// consumption record for the grant it named, and the allow audit row that
/// record points at must carry exactly its ranges and their byte count; a
/// `LOCAL_ONLY` run must have no allow row that transmitted the bytes of any
/// artifact it read.
pub fn reconcile_transmitted_ranges(
    run: &ModelRun,
    audit_rows: &[AuditRow],
    consumptions: &[ConsumptionRow],
) -> Result<Reconciliation, ReconciliationError> {
    match run.transmitted_byte_ranges() {
        Transmission::LocalOnly => reconcile_local_only(run, audit_rows),
        Transmission::Egressed { grant_id, ranges } => {
            reconcile_egressed(grant_id.as_str(), ranges, audit_rows, consumptions)
        }
    }
}

fn reconcile_local_only(
    run: &ModelRun,
    audit_rows: &[AuditRow],
) -> Result<Reconciliation, ReconciliationError> {
    let read_digests = run
        .input_artifact_refs()
        .as_slice()
        .iter()
        .map(|input| input.content_digest().to_lower_hex())
        .collect::<BTreeSet<_>>();
    for row in audit_rows {
        if row.decision != Decision::Allow || row.byte_count == 0 {
            continue;
        }
        for range in &row.artifact_ranges {
            let digest = range.content_digest().as_str();
            if read_digests.contains(digest) {
                return Err(ReconciliationError::LocalOnlyRunTransmitted(
                    digest.to_owned(),
                ));
            }
        }
    }
    Ok(Reconciliation {
        grant_id: None,
        audit_seq: None,
        byte_count: 0,
    })
}

fn reconcile_egressed(
    grant_id: &str,
    ranges: &[TransmittedRange],
    audit_rows: &[AuditRow],
    consumptions: &[ConsumptionRow],
) -> Result<Reconciliation, ReconciliationError> {
    // The key is the consumption, not the audit row's own `grant_id`. Both of
    // this table's foreign keys have to hold for a row to exist, so the audit
    // row reached here carries an identifier that is a real `egress_grant` --
    // which is the whole of what a discriminator column would have added.
    let mut spent = consumptions
        .iter()
        .filter(|consumption| consumption.grant_id == grant_id);
    let Some(consumption) = spent.next() else {
        return Err(ReconciliationError::GrantNotConsumed(grant_id.to_owned()));
    };
    if spent.next().is_some() {
        return Err(ReconciliationError::MultipleAuditRows(grant_id.to_owned()));
    }
    let row = audit_rows
        .iter()
        .find(|row| row.audit_seq == consumption.egress_audit_seq)
        .ok_or_else(|| ReconciliationError::ConsumptionAuditMissing(grant_id.to_owned()))?;
    if row.decision != Decision::Allow {
        return Err(ReconciliationError::GrantDenied(grant_id.to_owned()));
    }

    let recorded = ranges
        .iter()
        .map(|range| {
            (
                range.object_id().to_owned(),
                range.start(),
                range.end(),
                range.content_digest().to_lower_hex(),
            )
        })
        .collect::<BTreeSet<_>>();
    let audited = row
        .artifact_ranges
        .iter()
        .map(|range| {
            (
                range.object_id().to_owned(),
                range.start(),
                range.end(),
                range.content_digest().as_str().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    if recorded != audited {
        return Err(ReconciliationError::RangesDiffer(grant_id.to_owned()));
    }

    let recorded_bytes = ranges.iter().map(TransmittedRange::length).sum::<u64>();
    if recorded_bytes != row.byte_count {
        return Err(ReconciliationError::ByteCountDiffers {
            grant_id: grant_id.to_owned(),
            recorded: recorded_bytes,
            audited: row.byte_count,
        });
    }

    Ok(Reconciliation {
        grant_id: Some(grant_id.to_owned()),
        audit_seq: Some(row.audit_seq),
        byte_count: recorded_bytes,
    })
}
