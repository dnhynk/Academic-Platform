//! A projected evidence opportunity does not carry an attained level.
//!
//! This is the section 22.3 rule in code: "TCP is likely to be covered in
//! lecture" must not be spellable as "TCP is Understood at 68%". The
//! opportunity type has no mastery accessor, and its likelihood band is not a
//! mastery level under another name.

use academic_domain::{ClaimObject, ConfidencePermille, EntityId, OfferingId};
use academic_scenario::{
    LikelihoodBand, OpportunityBasis, OpportunityKind, ProjectedEvidenceOpportunity,
};

fn opportunity() -> ProjectedEvidenceOpportunity {
    let offering_id: OfferingId = "01936f2a-0000-7000-8000-000000000002".parse().unwrap();
    let concept_entity_id: EntityId = "01936f2a-0000-7000-8000-000000000003".parse().unwrap();
    ProjectedEvidenceOpportunity {
        offering_id,
        concept_entity_id,
        kind: OpportunityKind::Exposure,
        likelihood: LikelihoodBand::High,
        basis: OpportunityBasis::Syllabus,
        confidence: ConfidencePermille::new(650).unwrap(),
    }
}

fn main() {
    let projected = opportunity();

    let _by_accessor = projected.mastery_level();
    let _by_band = ClaimObject::Mastery(projected.likelihood());
}
