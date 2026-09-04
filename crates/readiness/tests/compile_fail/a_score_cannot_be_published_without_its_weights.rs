//! Four disclosures, and `disclose` takes all four by value.
//!
//! Leaving the weighting out is not a score published under a default
//! weighting. It is a call that does not compile, which is what
//! `score_without_full_disclosure_is_blocked` means by *blocked*.

use academic_competency::Competency;
use academic_readiness::{
    AuxiliaryScore, MissingDataDisclosure, ReadinessError, ReadinessMatrix, RubricDisclosure,
    SourceDisclosure, disclose,
};

fn shape(
    matrix: &ReadinessMatrix,
    competencies: &[&Competency],
    rubric: RubricDisclosure,
    sources: SourceDisclosure,
    missing: MissingDataDisclosure,
) -> Result<AuxiliaryScore, ReadinessError> {
    disclose(matrix, competencies, rubric, sources, missing)
}

fn main() {
    let _ = shape;
}
