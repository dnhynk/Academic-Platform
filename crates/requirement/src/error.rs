//! Every way this crate refuses.
//!
//! Each variant is a refusal a caller can act on. There is no catch-all string
//! variant, so a new refusal has to be named here before it can be returned,
//! and nothing structured can be smuggled out through a message (section 2.3-3).

use thiserror::Error;

/// A refusal from rule authoring, review, publication or evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RequirementError {
    /// An identifier that is not one of section 11.2's rule types.
    #[error("`{identifier}` is not a section 11.2 rule type")]
    UnknownRuleType {
        /// What was offered.
        identifier: String,
    },

    /// A rule identifier that is empty or carries a character the canonical
    /// encodings separate fields with.
    #[error("`{value}` is not a valid {kind}")]
    InvalidIdentifier {
        /// Which identifier kind was being built.
        kind: &'static str,
        /// What was offered.
        value: String,
    },

    /// A rule body that cannot be evaluated as written.
    #[error("rule `{rule}` is malformed: {reason}")]
    MalformedRule {
        /// The rule that could not be compiled.
        rule: String,
        /// Why, from a fixed vocabulary.
        reason: &'static str,
    },

    /// Two rules in one set claim the same identifier.
    #[error("rule `{rule}` is declared twice in one rule set")]
    DuplicateRule {
        /// The repeated identifier.
        rule: String,
    },

    /// A reviewed rule named a document rule the official source did not
    /// publish.
    ///
    /// The identifier a reviewer chooses inside a set and the identifier the
    /// official document gives a rule are two namespaces. A rule set is bound
    /// to one published document, so the document identifier a rule claims has
    /// to be one that document really carries; otherwise the crossing is a
    /// string somebody typed, and anything downstream that compares the two --
    /// `academic-audit`'s source-conflict gate does -- is reading a
    /// coincidence.
    #[error(
        "rule `{rule}` names source rule `{source_rule}`, which this official source did not publish"
    )]
    SourceRuleNotPublished {
        /// The rule inside the set.
        rule: String,
        /// The document identifier it claimed.
        source_rule: String,
    },

    /// A reviewed rule whose quoted official text is not the text of the
    /// document rule it names.
    ///
    /// Membership says the document publishes that identifier. It does not say
    /// the body came from it, and the document publishes many: one
    /// credit-minimum body labelled with any of the twelve identifiers a
    /// fixture document carries was admitted twelve times over, so an
    /// unresolved conflict about the rule it was really read from blocked one
    /// of the twelve and left eleven `DETERMINATE`.
    ///
    /// The two reviewers attest which document rule they believe the candidate
    /// was read from. This is the half nobody can attest: the official document
    /// publishes a digest per rule, the candidate's quoted span has one, and if
    /// they differ then the claimed crossing is refuted rather than doubted.
    #[error(
        "rule `{rule}` quotes text that is not what source rule `{source_rule}` states in this official source"
    )]
    QuotedSourceIsNotTheNamedRule {
        /// The rule inside the set.
        rule: String,
        /// The document identifier it claimed to have been read from.
        source_rule: String,
    },

    /// A draft with no rule in it was offered for publication.
    ///
    /// Section 11.4 makes a graduation determination conditional on *rule
    /// coverage 100%*, and coverage over no rule is the vacuous witness
    /// `academic_audit::CoverageWitness` refuses. A set that requires nothing
    /// is not a lenient requirement set; it is the shape that answers 졸업
    /// 가능 from an empty tree, and it was also the one state in which an audit
    /// could reach `INDETERMINATE` with no outstanding check to name.
    #[error("requirement set `{set}` version {version} publishes no rule")]
    EmptyRuleSet {
        /// The set that would have been published.
        set: String,
        /// The version that would have been published.
        version: String,
    },

    /// A review gate call whose two attestations came from one reviewer.
    ///
    /// Two attestations by one person is one review recorded twice. The gate
    /// takes two parameters so a single attestation cannot be passed at all;
    /// this is the remaining case, where two values were supplied and named the
    /// same reviewer.
    #[error("a rule candidate needs two attestations from two reviewers")]
    OneReviewerTwice,

    /// An attestation filed by something other than a person.
    ///
    /// Section 11.2: *사람이 검토한 executable rule만 production audit에
    /// 사용한다*. A model run or an importer attesting to a model's own
    /// candidate is the gate reviewing itself.
    #[error("a review attestation must be filed by a user, not by {actor}")]
    ReviewerIsNotAUser {
        /// The actor kind that was offered.
        actor: &'static str,
    },

    /// An attestation that names a different candidate than the one under
    /// review.
    #[error("attestation names candidate `{named}`, not `{under_review}`")]
    AttestationNamesAnotherCandidate {
        /// The candidate the attestation names.
        named: String,
        /// The candidate the gate was called with.
        under_review: String,
    },

    /// An attestation that names a different document rule than the candidate
    /// claims to have been read from.
    ///
    /// The crossing between the reviewer's identifier and the document's is
    /// what `academic-audit`'s conflict gate decides applicability from, so it
    /// is part of what the two reviewers attest rather than a value a model
    /// supplies alone.
    #[error("attestation names source rule `{named}`, not `{under_review}`")]
    AttestationNamesAnotherSourceRule {
        /// The document rule the attestation names.
        named: String,
        /// The document rule the candidate claims.
        under_review: String,
    },

    /// A publication whose superseded predecessor is not the version it claims.
    #[error("rule set `{claimed}` does not supersede `{actual}`")]
    SupersedesTheWrongVersion {
        /// The version the publication says it follows.
        claimed: String,
        /// The version the ledger actually holds as current.
        actual: String,
    },

    /// A publication that would replace a version in place.
    #[error("rule set version `{version}` is already published and is immutable")]
    VersionAlreadyPublished {
        /// The version that already exists.
        version: String,
    },

    /// A release with no official example fixture, no synthetic transcript
    /// fixture, or neither.
    #[error("rule `{rule}` cannot be released: {missing}")]
    ReleaseFixturesMissing {
        /// The rule whose release was refused.
        rule: String,
        /// Which fixture class is absent.
        missing: &'static str,
    },

    /// An evaluation asked for a fact the frozen set does not declare.
    #[error("rule `{rule}` reads `{fact}`, which the frozen fact set does not declare")]
    UndeclaredFact {
        /// The rule that asked.
        rule: String,
        /// The fact key it asked for.
        fact: String,
    },

    /// An identifier or interval a domain type refused.
    #[error("domain refused a rule value: {0}")]
    Domain(String),
}
