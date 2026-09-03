//! `a_candidate_cannot_be_published`.
//!
//! Section 11.2: a model may extract a rule candidate, and only a rule a person
//! reviewed goes into a production audit.
//!
//! `RuleSetDraft::include` takes a `ReviewedRule`, whose only producer is
//! `ReviewGate::admit`. There is no run-time check that refuses an unreviewed
//! candidate here, because there is no call that offers one.

use academic_domain::{Actor, ContentDigest, CurriculumVersionId, EntityId, RequirementSetId};
use academic_requirement::{
    CreditAmount, CreditCategory, RuleBody, RuleCandidate, RuleId, RuleSetDraft, RuleSetVersion,
};

fn candidate() -> RuleCandidate {
    let run_id: EntityId = "01900000-0000-7000-8000-000000000002".parse().unwrap();
    RuleCandidate::extracted(
        RuleId::new("total_credits").unwrap(),
        RuleBody::CreditMinimum {
            category: CreditCategory::new("ALL_RECOGNIZED").unwrap(),
            threshold: CreditAmount::new(130).unwrap(),
        },
        Actor::ModelRun { run_id },
        "the page says at least 130 credits".to_owned(),
        ContentDigest::sha256(b"official"),
    )
}

fn main() {
    let candidate = candidate();

    // The draft takes a `ReviewedRule`. A candidate is not one.
    let _published = draft().include(candidate.clone(), fixtures().0, fixtures().1);

    // `ReviewedRule` has private fields and no public constructor, so a caller
    // cannot build the value the draft wants.
    let _forged = academic_requirement::ReviewedRule::new(candidate.clone());

    // And there is no coercion in either direction.
    let _into: academic_requirement::ReviewedRule = candidate.clone().into();
    let _from = academic_requirement::ReviewedRule::from(candidate);
}

// Never reached: the calls above do not compile. Declared so the diagnostics
// above are about the routes and not about missing names.
fn draft() -> RuleSetDraft {
    let _ = (RequirementSetId::try_from_uuid, CurriculumVersionId::try_from_uuid);
    unimplemented!()
}

fn fixtures() -> (
    &'static academic_requirement::OfficialExampleFixtures,
    &'static academic_requirement::SyntheticTranscriptFixtures,
) {
    unimplemented!()
}

fn _version() -> RuleSetVersion {
    RuleSetVersion::FIRST
}
