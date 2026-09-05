//! `P2-U3`: section 11.3's explainable proof tree, section 11.1's fail-closed
//! selector, and section 11.4's three-gate `DETERMINATE` rule.
//!
//! This is the engine that tells a user whether they can graduate, so every
//! contract in it is fail-closed and the same sentence is behind all of them:
//! **an absent fact is never a verdict**.
//!
//! # `DETERMINATE` is three values, not three checks
//!
//! Section 11.4: *rule coverage 100%, unresolved conflict 0, source freshness
//! 기준 충족 시에만 `DETERMINATE`*. [`verdict::DeterminateVerdict`] has private
//! fields and one constructor, and that constructor takes
//! [`verdict::CoverageWitness`], [`verdict::ConflictFreeWitness`] and
//! [`verdict::FreshnessWitness`] **by value**. Each has private fields and a
//! crate-private `establish` that returns `Option<Self>` from the evidence its
//! gate is about. There is no expression anywhere that produces a determinate
//! verdict from two of them.
//!
//! The default is not `DETERMINATE`. It is not even reachable: with no recorded
//! source-freshness criterion there is no third witness, and the specification
//! states no criterion, so an audit is `INDETERMINATE` until a user records
//! one.
//!
//! # `INDETERMINATE` always says what is outstanding, and `DETERMINATE` says
//! nothing is
//!
//! [`verdict::IndeterminateVerdict`] takes its first [`verdict::MissingCheck`]
//! as a **parameter**, so an indeterminate verdict with an empty list is not a
//! call that can be written. Every arm of `MissingCheck` names the exact
//! field, rule, attempt or source that is outstanding, and every one carries
//! the action that settles it.
//!
//! The other half is [`engine::DegreeAudit`]'s: the outstanding checks are read
//! first and an outstanding check is an indeterminate verdict whatever the
//! three gates say. The three gates were the whole of the condition and the
//! list was dropped on the determinate branch, so a determination could be
//! published beside a check nobody had settled.
//!
//! # A leaf that cannot cite itself is not a leaf
//!
//! Section 11.3 names four things on every leaf: the rule ID, the source
//! page/paragraph, the attempt used, and the equivalency decision.
//! [`leaf::ProofLeaf`] takes all four as constructor parameters, has private
//! fields, no setter and no builder, and the two that could be empty are enums
//! whose other arm states the reason rather than an empty list. A published
//! rule the source index does not place therefore cannot become a leaf: it is
//! left **unevaluated**, which is a partial failure and blocks coverage, rather
//! than evaluated into a verdict with no citation.
//!
//! A page recorded inside *another* document is refused the same way and is a
//! separate check. [`source::RuleSourceIndex::placement`] takes the rule whole
//! rather than its identifier, so the digest the span points inside and the
//! digest the rule rests on are compared in the one place a span is read, and
//! a leaf citing a document its rule was never read from is not a value this
//! crate can produce.
//!
//! # Planned work never satisfies anything, in four layers
//!
//! `P2-C7` sealed a projected value and this crate has no `academic-scenario`
//! edge, so a projection is not nameable here. `P2-U4` gave
//! `PlanScenarioChoice` no route to a `CourseAttempt` and
//! `AttemptStatus::Planned` no producer. `P2-U4`'s credit engine reports a
//! not-settled attempt as earning nothing, so a plan contributes no credit even
//! if one reached the ledger. And [`engine::DegreeAudit::evaluate`] has **no
//! plan parameter**: [`plan::PlanAnnotatedView`] borrows a finished audit and
//! produces labels, never a verdict.
//!
//! # Seven section 38 cells stay open
//!
//! `GATE-38-001`–`GATE-38-004`, `GATE-38-006`, `GATE-38-011` and
//! `GATE-38-012`. The first five are profile fields section 11.1's selector
//! reads and section 38.1 asks the user for; the last two are official facts a
//! rule verdict already carries. None has a default, no cohort is assumed from
//! a term or an attempt, and each appears as its own exact missing check. See
//! [`gate`].

pub mod engine;
pub mod error;
pub mod explain;
pub mod facts;
pub mod gate;
pub mod leaf;
pub mod plan;
pub mod profile;
pub mod select;
pub mod source;
pub mod transcript;
pub mod verdict;

pub use engine::{
    AuditInputBinding, AuditNode, DegreeAudit, GRADUATION_ENGINE_ID, GRADUATION_HARNESS_DIR,
    GraduationAuditEngine, RULE_DEGREE_AUDIT,
};
pub use error::AuditError;
pub use explain::{CreditExplanation, CreditLine, CreditVerdict};
pub use facts::{AuditFacts, decode, encode};
pub use gate::OpenGate;
pub use leaf::{AttemptUsage, EquivalencyDecision, NoAttemptReason, ProofLeaf};
pub use plan::{PlanAnnotatedView, PlanNote, PlannedCoursework};
pub use profile::{
    DegreeMode, ExchangeOrTransfer, GraduationStandard, InstitutionId, ProfileField, ProgrammeId,
    Recorded, SelectorDimension, StudentProfile,
};
pub use select::{
    CatalogEntry, CommonRuleExample, CommonRuleExamples, RuleSetCatalog, RuleSetScope,
    SelectedRuleSet, Selection, select,
};
pub use source::{Placement, RuleSourceIndex, RuleSourceSpan};
pub use transcript::{
    ALL_GPA_ELIGIBLE, CourseFactsIndex, CourseRequirementFacts, EntryAdmission, TranscriptEntry,
    TranscriptSnapshot,
};
pub use verdict::{
    ConflictFreeWitness, CoverageWitness, DegreeVerdict, DeterminateVerdict, FreshnessWitness,
    GraduationOutcome, IndeterminateVerdict, MissingCheck, SourceFreshnessPolicy,
};
