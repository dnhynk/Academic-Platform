//! `P2-U2`: section 11.2's typed requirement rule DSL, section 11.1's
//! `DegreeRequirementSet`, and the reviewer gate that stops a model-extracted
//! rule candidate from ever executing.
//!
//! # Fourteen rule types, enumerated and not counted
//!
//! `t068` says *thirteen* and lists fourteen. The specification's own two
//! readings -- five `type:` identifiers in section 11.2's yaml and twelve
//! categories in its prose sentence, with the prose's *course set* opening into
//! three yaml types -- give fourteen, and `t001`'s independently derived
//! `REQ-11-004`...`REQ-11-017` gives fourteen rows. See [`rule_type`]; nothing
//! in this crate asserts a count.
//!
//! # A candidate cannot execute, and that is a type error
//!
//! [`candidate::RuleCandidate`] is what a model extracted.
//! [`publish::ExecutableRule`] is what an audit runs. There is no function from
//! the first to the second: the only producer of the intermediate
//! [`candidate::ReviewedRule`] is [`candidate::ReviewGate::admit`], which takes
//! two attestations as two parameters, and the only producer of an
//! [`publish::ExecutableRule`] is [`publish::RuleSetDraft::include`], which
//! takes a [`candidate::ReviewedRule`] by value. Every field of both is
//! private and neither has a public constructor, so the three cases in
//! `tests/compile_fail/` are compiler diagnostics rather than assertions.
//!
//! # No free text on the audit path
//!
//! The candidate carries the official sentence a model read. Nothing
//! downstream has a field it fits in: [`candidate::ReviewedRule`] does not
//! forward it and [`publish::ExecutableRule`] has no such field. Every operand,
//! threshold and scope in [`dsl`] is a typed value or a validated identifier
//! that cannot hold a space. And this crate's dependency closure contains no
//! crate that runs, wraps or carries a model's output, so a production audit
//! cannot reach one -- `production_audit_no_llm`.
//!
//! # Absence is `UNKNOWN`
//!
//! `GATE-38-011`, `GATE-38-012`, `GATE-38-015` and `GATE-38-016` are open. A
//! rule scoped to a cohort nobody recorded, a thesis rule whose scope is
//! unconfirmed, a mutual exclusion with no confirmed ceiling and an external
//! recognition with no confirmed cap each evaluate to
//! `ProofStatus::Unknown` and name the cell that made them so. None is a
//! default standing in for a value. See [`gate`].
//!
//! # What this crate does not have
//!
//! **No store edge.** The canonical writer is not in the closure these rules
//! compile against, so a rule set cannot write itself. Migration `0015` holds
//! the typed rows and `crates/store` owns them.
//!
//! **No curriculum edge.** `academic-curriculum`'s `EquivalenceRelation` is a
//! catalogue fact about two courses. This crate's `EQUIVALENCY` rule type is a
//! substitution one published requirement set admits. A requirement set that
//! silently inherited the catalogue's equivalences would change meaning when
//! the catalogue changed, and section 11.4 forbids exactly that by making a
//! change a new version rather than an edit. [`dsl::Operand`] resolves against
//! the rules published beside it and against nothing else.
//!
//! **No audit engine.** The proof tree, the `INDETERMINATE` selector and the
//! three-gate `DETERMINATE` rule are `P2-U3`'s. What is here is one rule's
//! verdict with everything a leaf needs; composing them is that task's.

pub mod candidate;
pub mod dsl;
pub mod error;
pub mod evaluate;
pub mod facts;
pub mod gate;
pub mod publish;
pub mod rule_type;

pub use candidate::{ReviewAttestation, ReviewGate, ReviewedRule, RuleCandidate};
pub use dsl::{
    AdmissionYear, Applicability, ApprovalAuthority, ApprovalRequirement, AreaId, AreaRequirement,
    CoRequisiteTiming, CountConstraint, CreditAmount, CreditCategory, DoubleCountingPolicy,
    GpaScope, InstructionLanguage, Operand, ProgramId, RecognitionPolicy, RuleBody, RuleId,
    ThesisGrading,
};
pub use error::RequirementError;
pub use evaluate::{Measure, RuleOutcome, evaluate};
pub use facts::{
    AcademicFacts, ApprovalFact, AttemptFact, AttemptStatus, GpaReading, LanguageEvidence,
    TermOrdinal, TrainingFact,
};
pub use gate::{OpenGate, unknown_readings};
pub use publish::{
    ExecutableRule, FixtureCase, OfficialExampleFixtures, OfficialSourceBinding, RuleSet,
    RuleSetDraft, RuleSetLedger, RuleSetVersion, SyntheticTranscriptFixtures,
};
pub use rule_type::{RuleType, SPEC_PROSE_CATEGORIES, SPEC_YAML_TYPES, SpellingSource};
