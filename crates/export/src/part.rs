//! The six parts section 37 names, enumerated rather than counted.
//!
//! Section 37's closing list is the whole content contract of a graduation
//! bundle:
//!
//! ```text
//! 졸업 시 사용자는 다음을 export할 수 있다.
//!
//! - 공식 성적/요건과 계산 proof
//! - 원본을 포함하거나 제외할 수 있는 강의·질문 archive
//! - concept/competency evidence history
//! - repository snapshot과 architecture evolution
//! - role 관심 변화와 alternative paths
//! - machine-readable graph와 open formats
//! ```
//!
//! Each variant below carries its bullet **verbatim**, and
//! `graduation_bundle_contains_all_six_named_parts` parses the list back out of
//! the specification and compares it with [`BundlePart::ALL`] in both
//! directions. Nothing in this crate asserts the number six: a specification
//! edit that adds a seventh bullet, renames one, or drops one fails that
//! comparison rather than being folded into the nearest existing part.
//!
//! # Why the sixth part is total and the other five are selections
//!
//! The sixth bullet is *machine-readable graph와 open formats* — the graph,
//! not a topic within it. So [`BundlePart::MachineReadableGraph`] carries the
//! complete canonical state of the exported watermark, and the five topical
//! parts are views selected out of it by predicate namespace. That is what
//! makes the assignment total without inventing a seventh "everything else"
//! part the specification does not write: a claim whose predicate names no
//! section 37 topic is still exported, in the part whose subject is the whole
//! graph.

use crate::{ExportError, ExportResult};

/// One of the six parts section 37 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BundlePart {
    /// 공식 성적/요건과 계산 proof
    OfficialRecordAndProof,
    /// 원본을 포함하거나 제외할 수 있는 강의·질문 archive
    LectureAndQuestionArchive,
    /// concept/competency evidence history
    ConceptAndCompetencyEvidence,
    /// repository snapshot과 architecture evolution
    RepositorySnapshotAndEvolution,
    /// role 관심 변화와 alternative paths
    RoleInterestAndAlternativePaths,
    /// machine-readable graph와 open formats
    MachineReadableGraph,
}

impl BundlePart {
    /// Every part, in the order section 37 lists them.
    pub const ALL: [Self; 6] = [
        Self::OfficialRecordAndProof,
        Self::LectureAndQuestionArchive,
        Self::ConceptAndCompetencyEvidence,
        Self::RepositorySnapshotAndEvolution,
        Self::RoleInterestAndAlternativePaths,
        Self::MachineReadableGraph,
    ];

    /// The contract spelling recorded in a manifest.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfficialRecordAndProof => "OFFICIAL_RECORD_AND_PROOF",
            Self::LectureAndQuestionArchive => "LECTURE_AND_QUESTION_ARCHIVE",
            Self::ConceptAndCompetencyEvidence => "CONCEPT_AND_COMPETENCY_EVIDENCE",
            Self::RepositorySnapshotAndEvolution => "REPOSITORY_SNAPSHOT_AND_EVOLUTION",
            Self::RoleInterestAndAlternativePaths => "ROLE_INTEREST_AND_ALTERNATIVE_PATHS",
            Self::MachineReadableGraph => "MACHINE_READABLE_GRAPH",
        }
    }

    /// Parses the contract spelling.
    pub fn parse(value: &str) -> ExportResult<Self> {
        Self::ALL
            .into_iter()
            .find(|part| part.as_str() == value)
            .ok_or_else(|| ExportError::Malformed {
                item: "bundle part",
                value: value.to_owned(),
            })
    }

    /// The directory this part occupies, below `parts/`.
    #[must_use]
    pub const fn directory(self) -> &'static str {
        match self {
            Self::OfficialRecordAndProof => "official-record-and-proof",
            Self::LectureAndQuestionArchive => "lecture-and-question-archive",
            Self::ConceptAndCompetencyEvidence => "concept-and-competency-evidence",
            Self::RepositorySnapshotAndEvolution => "repository-snapshot-and-evolution",
            Self::RoleInterestAndAlternativePaths => "role-interest-and-alternative-paths",
            Self::MachineReadableGraph => "machine-readable-graph",
        }
    }

    /// Section 37's own bullet for this part, verbatim.
    #[must_use]
    pub const fn specification_sentence(self) -> &'static str {
        match self {
            Self::OfficialRecordAndProof => "공식 성적/요건과 계산 proof",
            Self::LectureAndQuestionArchive => "원본을 포함하거나 제외할 수 있는 강의·질문 archive",
            Self::ConceptAndCompetencyEvidence => "concept/competency evidence history",
            Self::RepositorySnapshotAndEvolution => "repository snapshot과 architecture evolution",
            Self::RoleInterestAndAlternativePaths => "role 관심 변화와 alternative paths",
            Self::MachineReadableGraph => "machine-readable graph와 open formats",
        }
    }

    /// The predicate namespaces whose claims this part selects.
    ///
    /// Empty for [`Self::MachineReadableGraph`], which is not a selection: it
    /// carries the canonical state whole. A namespace appears under exactly one
    /// part, and `no_predicate_namespace_belongs_to_two_parts` compares the six
    /// lists pairwise, so a claim cannot be routed by whichever part is
    /// consulted first.
    #[must_use]
    pub const fn predicate_namespaces(self) -> &'static [&'static str] {
        match self {
            Self::OfficialRecordAndProof => &["course", "credit", "grade", "requirement"],
            Self::LectureAndQuestionArchive => &["lecture", "note", "question"],
            Self::ConceptAndCompetencyEvidence => {
                &["competency", "concept", "freshness", "mastery"]
            }
            Self::RepositorySnapshotAndEvolution => &["architecture", "code", "repository"],
            Self::RoleInterestAndAlternativePaths => &["career", "path", "role"],
            Self::MachineReadableGraph => &[],
        }
    }

    /// The part a predicate identifier's first segment routes a claim to.
    ///
    /// `None` means no section 37 topic names it, which is not an error: the
    /// claim is still exported under [`Self::MachineReadableGraph`], which
    /// carries the graph whole.
    #[must_use]
    pub fn for_predicate(predicate_id: &str) -> Option<Self> {
        let namespace = predicate_id.split('.').next().unwrap_or(predicate_id);
        Self::ALL
            .into_iter()
            .find(|part| part.predicate_namespaces().contains(&namespace))
    }
}

#[cfg(test)]
mod tests {
    use super::BundlePart;

    #[test]
    fn every_part_has_a_distinct_directory_name_and_spelling() {
        let mut directories: Vec<&str> = BundlePart::ALL
            .into_iter()
            .map(BundlePart::directory)
            .collect();
        directories.sort_unstable();
        directories.dedup();
        assert_eq!(directories.len(), BundlePart::ALL.len());

        let mut spellings: Vec<&str> = BundlePart::ALL
            .into_iter()
            .map(BundlePart::as_str)
            .collect();
        spellings.sort_unstable();
        spellings.dedup();
        assert_eq!(spellings.len(), BundlePart::ALL.len());
    }

    #[test]
    fn no_predicate_namespace_belongs_to_two_parts() {
        let mut seen: Vec<&str> = Vec::new();
        for part in BundlePart::ALL {
            for namespace in part.predicate_namespaces() {
                assert!(
                    !seen.contains(namespace),
                    "{namespace} is claimed by more than one section 37 part"
                );
                seen.push(namespace);
            }
        }
        assert!(!seen.is_empty());
    }

    #[test]
    fn the_graph_part_selects_nothing_and_every_other_part_selects_something() {
        for part in BundlePart::ALL {
            if part == BundlePart::MachineReadableGraph {
                assert!(part.predicate_namespaces().is_empty());
            } else {
                assert!(!part.predicate_namespaces().is_empty());
            }
        }
    }

    #[test]
    fn parse_round_trips_every_part_and_refuses_anything_else() {
        for part in BundlePart::ALL {
            assert_eq!(BundlePart::parse(part.as_str()).ok(), Some(part));
        }
        assert!(BundlePart::parse("OFFICIAL_RECORD").is_err());
    }
}
