//! The import row and the confirmed row as two linked claims.
//!
//! This is the contract section 29.3 fixes, and it is enforced by primitives
//! this repository already has rather than by a new rule:
//!
//! - an import row is asserted by [`Actor::Importer`] or [`Actor::ModelRun`],
//!   and `Claim::validate_for_actor` permits neither of those to assert
//!   [`AuthorityClass::UserExplicit`]. So no import route can mint a claim
//!   that reads as user-confirmed, whatever the caller passes;
//! - a confirmed row is asserted by [`Actor::User`], which is permitted
//!   `UserExplicit` and nothing else;
//! - the two carry different [`ClaimId`]s and are joined by an explicit
//!   [`ClaimRelation`], so a projection can find one from the other without
//!   either replacing the other.
//!
//! Confirmed rows are built from a [`ReconciledTranscript`] and from nothing
//! else. A reconciliation that halted produces no such value, which is how
//! `IN03`'s "nothing confirmed" holds without a caller checking a flag.

use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ClaimRelation, ClaimRelationKind,
    ConfidencePermille, EntityId, EpistemicStatus, EvidenceId, PredicateId, ScopeId, ValidInterval,
};

use crate::{
    TranscriptError,
    reconcile::ReconciledTranscript,
    record::{TranscriptField, TranscriptRow},
    source::TranscriptFormat,
};

/// Predicate every transcript row claim is asserted under.
pub const TRANSCRIPT_ROW_PREDICATE: &str = "academic.transcript.row";

/// Importer name recorded on a deterministically parsed row.
pub const DETERMINISTIC_IMPORTER_NAME: &str = "academic-transcript-normalizer";
/// Importer version recorded on a deterministically parsed row.
pub const DETERMINISTIC_IMPORTER_VERSION: &str = "1";

/// The two claim identities one row needs.
///
/// Identifiers are supplied by the caller rather than generated here. UUIDv7
/// generation reads a clock, and this crate keeps origin order, acceptance
/// order, and valid time in the caller's hands, exactly as `CONTRIBUTING.md`
/// rule 3 requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowClaimIds {
    /// Identity of the import row claim.
    pub import_claim_id: ClaimId,
    /// Identity of the user-confirmed row claim.
    pub confirmed_claim_id: ClaimId,
}

/// Everything one row claim needs that is not the row itself.
#[derive(Debug, Clone)]
pub struct RowClaimContext {
    /// Entity the row is asserted about — the attempt this row records.
    pub subject_entity_id: EntityId,
    /// Scope the claim and its relation live in.
    pub scope_id: ScopeId,
    /// Domain-valid interval of the assertion.
    pub valid_time: ValidInterval,
    /// Evidence pointing at the sealed transcript original.
    pub import_evidence_ids: Vec<EvidenceId>,
    /// Evidence pointing at the user's confirmation.
    pub confirmation_evidence_ids: Vec<EvidenceId>,
}

/// An import row claim, with the actor that may assert it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRowClaim {
    claim: Claim,
    actor: Actor,
    format: TranscriptFormat,
    ordinal: u32,
}

impl ImportRowClaim {
    /// Returns the validated claim.
    #[must_use]
    pub const fn claim(&self) -> &Claim {
        &self.claim
    }

    /// Returns the actor permitted to assert it.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// Returns the import route that produced the row.
    #[must_use]
    pub const fn format(&self) -> TranscriptFormat {
        self.format
    }

    /// Returns the row's document-order position.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

/// A user-confirmed row claim, with the actor that may assert it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedRowClaim {
    claim: Claim,
    actor: Actor,
    ordinal: u32,
    import_claim_id: ClaimId,
}

impl ConfirmedRowClaim {
    /// Returns the validated claim.
    #[must_use]
    pub const fn claim(&self) -> &Claim {
        &self.claim
    }

    /// Returns the user actor that asserted it.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// Returns the row's document-order position.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the import claim this confirmation is linked to.
    #[must_use]
    pub const fn import_claim_id(&self) -> ClaimId {
        self.import_claim_id
    }
}

/// One row's import claim, confirmed claim, and the relation joining them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedRowClaims {
    /// The import row.
    pub import: ImportRowClaim,
    /// The user-confirmed row.
    pub confirmed: ConfirmedRowClaim,
    /// The append-only link from the import row to the confirmation.
    pub relation: ClaimRelation,
}

/// Builds the import row claim for one row.
///
/// `confidence` is required for a model read and refused for a deterministic
/// one. A model read that carried no confidence would be indistinguishable
/// from a deterministic read in every projection that reads the claim, and
/// section 30.5 puts confidence on inference and nowhere else.
pub fn import_row_claim(
    row: &TranscriptRow,
    format: TranscriptFormat,
    confidence: Option<ConfidencePermille>,
    ids: RowClaimIds,
    context: &RowClaimContext,
) -> Result<ImportRowClaim, TranscriptError> {
    let (actor, authority_class, epistemic_status) = if format.is_model_read() {
        if confidence.is_none() {
            return Err(TranscriptError::ModelReadNeedsConfidence);
        }
        (
            Actor::ModelRun {
                run_id: context.subject_entity_id,
            },
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
        )
    } else {
        if confidence.is_some() {
            return Err(TranscriptError::DeterministicReadCarriesConfidence);
        }
        (
            Actor::Importer {
                name: DETERMINISTIC_IMPORTER_NAME.to_owned(),
                version: DETERMINISTIC_IMPORTER_VERSION.to_owned(),
            },
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
        )
    };
    let claim = Claim {
        id: ids.import_claim_id,
        subject_entity_id: context.subject_entity_id,
        predicate_id: predicate()?,
        object: ClaimObject::Text(row_object_text(row)),
        scope_id: context.scope_id,
        authority_class,
        epistemic_status,
        confidence,
        prediction_metadata: None,
        valid_time: context.valid_time,
        evidence_ids: context.import_evidence_ids.clone(),
    };
    claim.validate_for_actor(&actor)?;
    Ok(ImportRowClaim {
        claim,
        actor,
        format,
        ordinal: row.ordinal(),
    })
}

/// Builds the linked import and confirmed claims for every reconciled row.
///
/// The `ReconciledTranscript` argument is the gate: `reconcile` returns one
/// only when every row agreed, so a halted import has no value to pass here.
pub fn confirm_reconciled_rows(
    reconciled: &ReconciledTranscript,
    format: TranscriptFormat,
    confidence: Option<ConfidencePermille>,
    user_id: EntityId,
    ids: &[RowClaimIds],
    context: &RowClaimContext,
) -> Result<Vec<LinkedRowClaims>, TranscriptError> {
    let rows = reconciled.transcript().rows();
    if ids.len() != rows.len() {
        return Err(TranscriptError::ClaimIdCountMismatch {
            rows: rows.len(),
            ids: ids.len(),
        });
    }
    let mut linked = Vec::with_capacity(rows.len());
    for (row, row_ids) in rows.iter().zip(ids) {
        let import = import_row_claim(row, format, confidence, *row_ids, context)?;
        let actor = Actor::User { user_id };
        let claim = Claim {
            id: row_ids.confirmed_claim_id,
            subject_entity_id: context.subject_entity_id,
            predicate_id: predicate()?,
            object: ClaimObject::Text(row_object_text(row)),
            scope_id: context.scope_id,
            authority_class: AuthorityClass::UserExplicit,
            epistemic_status: EpistemicStatus::UserConfirmed,
            confidence: None,
            prediction_metadata: None,
            valid_time: context.valid_time,
            evidence_ids: context.confirmation_evidence_ids.clone(),
        };
        claim.validate_for_actor(&actor)?;
        if claim.id == import.claim.id {
            return Err(TranscriptError::ClaimIdsCollide(claim.id));
        }
        let relation = ClaimRelation {
            source_claim_id: import.claim.id,
            target_claim_id: claim.id,
            kind: ClaimRelationKind::Supports,
            scope_id: context.scope_id,
        };
        linked.push(LinkedRowClaims {
            import,
            confirmed: ConfirmedRowClaim {
                claim,
                actor,
                ordinal: row.ordinal(),
                import_claim_id: row_ids.import_claim_id,
            },
            relation,
        });
    }
    Ok(linked)
}

fn predicate() -> Result<PredicateId, TranscriptError> {
    Ok(PredicateId::parse(TRANSCRIPT_ROW_PREDICATE)?)
}

/// Renders the four reconciled fields as the claim's object text.
///
/// The identity header is absent by construction. A claim object is copied into
/// projections, proof trees and explanation snapshots, and putting the student
/// number there would reintroduce the value the redaction projection exists to
/// remove — one step outside the export path, where nothing in this task's
/// acceptance rows would have looked for it.
fn row_object_text(row: &TranscriptRow) -> String {
    TranscriptField::ALL
        .map(|field| format!("{}={}", field.as_str(), row.field(field)))
        .join(";")
}
