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
