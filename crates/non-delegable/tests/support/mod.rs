//! Fixtures shared by the acceptance suite.
//!
//! Every value here is synthetic. The three automatic actors are the ones
//! `academic_domain::Actor` declares beside `Actor::User`, built once so a test
//! that walks them cannot walk a shorter list than the enum has.

use std::{error::Error, str::FromStr as _};

use academic_domain::{
    Actor, ArtifactId, AuthorityClass, Claim, ClaimId, ClaimObject, ContentDigest, EntityId,
    EpistemicStatus, EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength,
    PredicateId, ScopeId, TimestampMillis, ValidInterval,
};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The design document, from this crate's directory.
pub const DESIGN_DOCUMENT: &str = "../../PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md";

/// A synthetic identifier with a fixed shape.
pub fn entity(suffix: u32) -> Result<EntityId, Box<dyn Error>> {
    Ok(EntityId::from_str(&format!(
        "01920000-0000-7000-8000-{suffix:012x}"
    ))?)
}

/// A synthetic scope identifier.
pub fn scope(suffix: u32) -> Result<ScopeId, Box<dyn Error>> {
    Ok(ScopeId::from_str(&format!(
        "01920000-0000-7000-8000-{suffix:012x}"
    ))?)
}

/// A synthetic evidence identifier.
pub fn evidence_id(suffix: u32) -> Result<EvidenceId, Box<dyn Error>> {
    Ok(EvidenceId::from_str(&format!(
        "01920000-0000-7000-8000-{suffix:012x}"
    ))?)
}

/// A synthetic artifact identifier.
pub fn artifact_id(suffix: u32) -> Result<ArtifactId, Box<dyn Error>> {
    Ok(ArtifactId::from_str(&format!(
        "01920000-0000-7000-8000-{suffix:012x}"
    ))?)
}

/// The user actor every accepted decision in this suite carries.
pub fn user_actor() -> Result<Actor, Box<dyn Error>> {
    Ok(Actor::User {
        user_id: entity(13)?,
    })
}

/// Every actor `academic_domain::Actor` declares that is not a user.
///
/// Built from the enum's own three non-user variants. A test walks this rather
/// than a list of names, so the three the execution plan enumerates —
/// `MODEL_RUN`, `IMPORTER`, `DETERMINISTIC_ENGINE` — are the three that exist
/// rather than three a test happened to write down.
pub fn automatic_actors() -> Result<[Actor; 3], Box<dyn Error>> {
    Ok([
        Actor::ModelRun {
            run_id: entity(901)?,
        },
        Actor::Importer {
            name: "synthetic-importer".to_owned(),
            version: "1".to_owned(),
        },
        Actor::DeterministicEngine {
            name: "synthetic-engine".to_owned(),
            version: "1".to_owned(),
        },
    ])
}

/// A synthetic evidence item.
pub fn evidence(suffix: u32, digest: &[u8]) -> Result<EvidenceItem, Box<dyn Error>> {
    Ok(EvidenceItem {
        id: evidence_id(suffix)?,
        artifact_id: artifact_id(suffix)?,
        locator: EvidenceLocator::Page { page_number: 1 },
        excerpt_digest: ContentDigest::sha256(digest),
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "synthetic-non-delegable-fixture".to_owned(),
        extractor_version: "1".to_owned(),
    })
}

/// A user-explicit, user-confirmed claim over one subject.
///
/// The pairing is the one ADR-003's matrix reserves for `Actor::User`, so
/// offering it with an automatic actor is the exact forgery the two doors this
/// suite drives have to refuse.
pub fn user_confirmed_claim(
    suffix: u32,
    subject: EntityId,
    scope_id: ScopeId,
    predicate: &str,
    object: ClaimObject,
    evidence_ids: Vec<EvidenceId>,
) -> Result<Claim, Box<dyn Error>> {
    Ok(Claim {
        id: ClaimId::from_str(&format!("01920000-0000-7000-8000-{suffix:012x}"))?,
        subject_entity_id: subject,
        predicate_id: PredicateId::parse(predicate)?,
        object,
        scope_id,
        authority_class: AuthorityClass::UserExplicit,
        epistemic_status: EpistemicStatus::UserConfirmed,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(13)),
        evidence_ids,
    })
}

/// Reads one `### ` subsection of the design document, body only.
///
/// The body ends at the next heading of any level, so a subsection cannot
/// silently absorb the one after it.
pub fn design_subsection(heading: &str) -> Result<String, Box<dyn Error>> {
    let text = std::fs::read_to_string(DESIGN_DOCUMENT)?;
    let start = text
        .find(heading)
        .ok_or_else(|| format!("the design document no longer has {heading}"))?;
    let body = &text[start + heading.len()..];
    let end = body.find("\n#").unwrap_or(body.len());
    Ok(body[..end].to_owned())
}
