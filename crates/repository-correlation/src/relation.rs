//! Section 17.5's typed evidence relations, and the two authority lanes of
//! section 30.3 they answer for.
//!
//! ## The vocabulary is enumerated, never counted
//!
//! Section 17.5 writes the relations as a bullet list. [`EvidenceRelation`]
//! holds that list, in that order, with each variant's [`EvidenceRelation::as_str`]
//! spelled as the design document spells it. `seven_relation_types_are_distinct`
//! reads the bullet list back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares the two
//! sets in both directions, so the number of relations is a measurement of the
//! design document rather than a number restated here — a restated number is
//! how a miscount survives.
//!
//! ## Which lane a relation answers for
//!
//! Section 30.3's table has six rows and this task owns two of them, quoted
//! whole:
//!
//! | Claim 종류 | active view 우선순위 | 충돌 처리 |
//! |---|---|---|
//! | 현재 구현 | 같은 snapshot의 runtime/config/code direct evidence > user clarification > AI | spec은 intent lane에 보존 |
//! | project intent | 승인된 최신 spec/ADR > user clarification > AI | code와 drift 생성 |
//!
//! Row four's authority list is *runtime/config/code*, so the four relations
//! carrying a runtime, a configuration or a code observation answer it. Row
//! five's is *spec/ADR*, so the two carrying one of those answer that one.
//!
//! `PROJECT_DOC_EXPLAINS` answers **neither**, and that is not an omission.
//! Section 17.5 defines it as `문서가 현재 동작을 설명` — a description of what
//! the system does. A description is not an approval to build something, so it
//! is not row five's authority, and it does not make anything run, so it is not
//! row four's. Its absence is what section 17.5's second diagram turns into
//! `IMPLEMENTED_NOT_DOCUMENTED`, and putting it in the implementation lane
//! would make that absence weaken the implementation claim instead — the
//! opposite of what that diagram says.

/// Section 17.5's `주요 evidence relation`, in the design document's own order.
///
/// Every variant's [`Self::as_str`] is the spelling section 17.5 uses. The set
/// is compared against that section's bullet list at test time; see the module
/// documentation for why it is compared rather than counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceRelation {
    /// `규범적 의도`: a specification names the subject.
    SpecMentions,
    /// `실제 코드 구조에서 관찰`: the analysis observed a use.
    CodeUses,
    /// `architecture constraint로 필요`: an architecture decision requires it.
    ArchitectureRequires,
    /// `test가 동작/failure를 검증`: the observed use is test-scoped.
    TestExercises,
    /// `실행 구성에서 활성화`: a production configuration a trace agreed with.
    ConfigEnables,
    /// `incident가 failure mode를 드러냄`: an incident record named it.
    IncidentExposed,
    /// `문서가 현재 동작을 설명`: a document describes the current behaviour.
    DocExplains,
}

impl EvidenceRelation {
    /// Section 17.5's bullet list, in its order.
    pub const ALL: [Self; 7] = [
        Self::SpecMentions,
        Self::CodeUses,
        Self::ArchitectureRequires,
        Self::TestExercises,
        Self::ConfigEnables,
        Self::IncidentExposed,
        Self::DocExplains,
    ];

    /// The spelling section 17.5 uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpecMentions => "PROJECT_SPEC_MENTIONS",
            Self::CodeUses => "PROJECT_CODE_USES",
            Self::ArchitectureRequires => "PROJECT_ARCHITECTURE_REQUIRES",
            Self::TestExercises => "PROJECT_TEST_EXERCISES",
            Self::ConfigEnables => "PROJECT_CONFIG_ENABLES",
            Self::IncidentExposed => "PROJECT_INCIDENT_EXPOSED",
            Self::DocExplains => "PROJECT_DOC_EXPLAINS",
        }
    }

    /// Which of section 30.3's two rows this relation is authority for.
    ///
    /// Total, with no default arm: an eighth relation has to answer this
    /// question rather than inherit an answer from whichever arm it was
    /// written beside.
    #[must_use]
    pub const fn lane(self) -> AuthorityLane {
        match self {
            Self::SpecMentions | Self::ArchitectureRequires => AuthorityLane::Intent,
            Self::CodeUses | Self::TestExercises | Self::ConfigEnables | Self::IncidentExposed => {
                AuthorityLane::Implementation
            }
            Self::DocExplains => AuthorityLane::Description,
        }
    }
}

/// Which question a relation is an answer to.
///
/// Two of these are section 30.3's rows four and five. The third is neither of
/// them, and exists because section 17.5's seventh relation is neither: see the
/// module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthorityLane {
    /// Section 30.3 row five, `project intent`. `무엇을 만들기로 승인했는가`.
    Intent,
    /// Section 30.3 row four, `현재 구현`. `무엇이 현재 실행되는가`.
    Implementation,
    /// Neither row. A description of current behaviour approves nothing and
    /// runs nothing.
    Description,
}

impl AuthorityLane {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::Intent, Self::Implementation, Self::Description];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Intent => "INTENT",
            Self::Implementation => "IMPLEMENTATION",
            Self::Description => "DESCRIPTION",
        }
    }

    /// The section 30.3 row, as `academic-ledger` already names it.
    ///
    /// `P2-C3` implemented section 30.3's six rows as
    /// [`academic_ledger::ProductClaimType`] with a rank table each. This crate
    /// adds no rank and no ordering; what it adds is which authority class a
    /// piece of correlation evidence may enter that table as, which is
    /// [`crate::authority`].
    #[must_use]
    pub const fn claim_type(self) -> Option<academic_ledger::ProductClaimType> {
        match self {
            Self::Intent => Some(academic_ledger::ProductClaimType::ProjectIntent),
            Self::Implementation => Some(academic_ledger::ProductClaimType::CurrentImplementation),
            Self::Description => None,
        }
    }
}
