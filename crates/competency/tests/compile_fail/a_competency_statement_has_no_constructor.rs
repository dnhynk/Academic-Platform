//! Section 7.1, as an argument that does not exist.
//!
//! `declare` takes a situation, criteria, enabling concepts and a rubric, and
//! there is no place among them for a sentence. A caller who wants to write
//! `knows X` has nowhere to put it: the statement is rendered from the parts,
//! and `CompetencyStatement` has no constructor of its own.

use academic_competency::{
    CompetencyId, ContributionImportance, CriterionId, EnablingConcept, EvidenceRubric,
    EvidenceStage, Necessity, PerformanceCriterion, RubricRow, Situation, declare,
};
use academic_domain::EntityId;

fn concept() -> academic_competency::ConceptRef {
    academic_competency::ConceptRef::ontology(
        EntityId::try_from_uuid(uuid::Uuid::nil()).unwrap_or_else(|_| unreachable!()),
    )
}

fn main() {
    let criterion = CriterionId::new("c-1").unwrap_or_else(|_| unreachable!());
    let _ = declare(
        CompetencyId::new("knows_b_plus_tree").unwrap_or_else(|_| unreachable!()),
        Situation::new("a database course").unwrap_or_else(|_| unreachable!()),
        vec![
            PerformanceCriterion::of(criterion.clone(), "reads a page", vec![concept()])
                .unwrap_or_else(|_| unreachable!()),
        ],
        vec![EnablingConcept::of(
            concept(),
            ContributionImportance::Critical,
            Necessity::Necessary,
        )],
        EvidenceRubric::of(vec![
            RubricRow::of(criterion, EvidenceStage::Used, "the code")
                .unwrap_or_else(|_| unreachable!()),
        ]),
        "B+ Tree를 안다",
    );
}
