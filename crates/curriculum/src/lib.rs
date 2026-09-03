//! `P2-U1`: section 8.2's curriculum, course, revision and offering aggregates,
//! with the boundaries section 9 draws between them.
//!
//! # The boundaries are absences, not checks
//!
//! Section 9's table gives each aggregate one row of what it *does not*
//! contain. A `Course` has no instructor and no term, a `CourseRevision` has no
//! section reality, and a `CourseOffering` has no per-session utterance. Here
//! those are fields that do not exist and setters that were never written, so
//! the three `*_boundary_rejects_*` cases in `tests/compile_fail/` are compiler
//! diagnostics rather than assertions: there is nothing to reject.
//!
//! The list of what is absent is enumerated in
//! `tests/curriculum_scans.rs`, read back out of the specification's own
//! section 8.2 blocks and section 9 table, and compared. Dropping a name from
//! the Rust side fails against the specification. No count is asserted.
//!
//! # Four independent relations
//!
//! Section 11.4: *동일·대체·폐지·경과조치는 독립 rule이며 양방향 동일성으로
//! 단순화하지 않는다*. Three of the four are course-level and live in
//! [`relation`]; 경과조치 moves an admission cohort between curriculum
//! versions and lives in [`version`]. There is no `From`, no conversion, and no
//! query whose answer reads more than one of the four sets, so recording a
//! replacement moves no identity answer and an equivalence asserted `A → B`
//! says nothing about `B → A`.
//!
//! # Absence is `UNKNOWN`
//!
//! `GATE-38-013`, `GATE-38-014` and `GATE-38-018` are open. A revision whose
//! official category has not been confirmed reads
//! [`revision::CurriculumCategory::Unknown`]; two course rows with no recorded
//! identity decision read [`relation::CourseCodeReuse::Unknown`]; a cohort with
//! no recorded arrangement reads [`version::CohortTransition::Unknown`]. None
//! of those is a default that stands in for a value: each is the absence of a
//! record, and nothing here infers past it. See [`gate`].
//!
//! # What this crate does not have
//!
//! **No store edge.** The canonical writer is not in this crate's dependency
//! closure, so a curriculum aggregate cannot write itself. Migration `0014`
//! holds the typed rows and `crates/store/src/curriculum_tests.rs` is where the
//! database half of the boundaries is enforced against a writer that never came
//! through here.
//!
//! **No prediction.** Section 8.3's four statuses are a field on an offering
//! ([`offering::OfferingStatus`]); the calibrated probability, the feature
//! families, the observation window and the per-term evaluation are `P2-U5`'s.
//! Nothing here computes one or promotes one into `Confirmed`.
//!
//! **No rule engine.** A `DegreeRequirementSet`, its `rules` and its
//! `transitionRules` (section 11.1) are `P2-U2`'s aggregate.
//!
//! **No transport and no parser.** The official bytes stay behind
//! `academic_ingestion::RawSnapshot`'s sealed route. What arrives here is a
//! `PublishedRules`: identifiers, dates and a parser version.

pub mod course;
pub mod error;
pub mod fault;
pub mod gate;
pub mod offering;
pub mod publish;
pub mod relation;
pub mod revision;
pub mod text;
pub mod version;

pub use course::{Course, CourseDraft};
pub use error::CurriculumError;
pub use fault::{NoFault, PublishCheckpoint, PublishFaultInjector};
pub use gate::{OpenGate, unknown_readings};
pub use offering::{
    Capacity, CourseOffering, CourseOfferingDraft, GradingMode, Meeting, OfferingStatus, Weekday,
};
pub use publish::{
    CurriculumLedger, CurriculumPublication, CurriculumPublisher, OfficialSourceBinding,
    PublishReceipt,
};
pub use relation::{
    CourseCodeReuse, CourseRelationKind, CourseRelations, EquivalenceRelation, IdentityDecision,
    ReplacementRelation, RetirementRelation,
};
pub use revision::{
    CourseRevision, CourseRevisionDraft, Credits, CurriculumCategory, OfficialPrerequisite,
    RecommendedPrerequisite,
};
pub use text::{AdmissionCohort, CourseCode, CourseTitle, InstructorName, SectionCode, TermCode};
pub use version::{
    CohortTransition, CurriculumVersion, CurriculumVersionDraft, PublicationStatus,
    TransitionArrangement,
};
