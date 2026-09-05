//! The review gate a model-extracted rule candidate has to pass, and the type
//! that makes passing it the only route.
//!
//! Section 11.2: *LLM은 원문에서 rule 후보를 추출할 수 있으나 사람이 검토한
//! executable rule만 production audit에 사용한다.*
//!
//! # The gate is a type, not a check
//!
//! Three types and one function:
//!
//! ```text
//! RuleCandidate  --  what a model extracted. Carries the quoted source text.
//!                    No accessor produces a RuleBody.
//!        |
//!        |  ReviewGate::admit(candidate, first, second)  -- two attestations,
//!        |                                                  two reviewers
//!        v
//! ReviewedRule   --  private fields, and this crate's only constructor is the
//!                    line inside `admit`. No public `new`, no `From`.
//!        |
//!        |  RuleSetDraft::include(reviewed)
//!        v
//! ExecutableRule --  what a published RuleSet holds and what `evaluate` takes.
//! ```
//!
//! A caller holding a [`RuleCandidate`] cannot build a [`ReviewedRule`],
//! because [`ReviewedRule`]'s fields are private and no public constructor
//! takes anything but the gate's own output. It cannot build an
//! [`crate::publish::ExecutableRule`] either, for the same reason, and it
//! cannot hand a candidate to `RuleSetDraft::include`, which takes a
//! [`ReviewedRule`] by value. Those are three compiler diagnostics, not three
//! run-time refusals; `tests/compile_fail/` holds one case for each.
//!
//! `P2-C7`'s projected/actual isolation and `P2-U1`'s "a forbidden field has no
//! setter" are the two precedents. The rule they share is that an absence is
//! stronger than a check, because a check has a caller who might not run it and
//! an absence has no caller at all.
//!
//! # Why two, and why two people
//!
//! [`ReviewGate::admit`] takes the two attestations as **two parameters**, so a
//! single attestation cannot be passed at all -- the arity is the requirement.
//! What the body then adds is that the two must name different reviewers and
//! must both be `Actor::User`: two attestations from one person is one review
//! recorded twice, and a model attesting to its own candidate is the gate
//! reviewing itself. Both are [`crate::error::RequirementError`] variants, and
//! `rule_candidate_review_gate` drives each.
//!
//! # What the candidate carries and where it stops
//!
//! A candidate carries `quoted_source`, which is the span of official text a
//! model read. That is the one free-text value in this crate, and it is on the
//! one type that never reaches an evaluation. What travels past the gate is its
//! **digest** -- [`ReviewedRule::quoted_source_digest`] -- which is what makes
//! the crossing to `source_rule` checkable: the official document publishes a
//! digest per rule, and [`crate::publish::RuleSetDraft::include`] requires the
//! two to agree. A digest is not a sentence, so [`ReviewedRule`] still carries
//! no official text, [`crate::publish::ExecutableRule`] still has no field one
//! could sit in, and the production audit path still has no sentence in it at
//! all -- `production_audit_no_llm`'s structural half.

use academic_domain::{
    Actor, ContentDigest, EntityId, TimestampMillis, engines::RuleId as SourceRuleId,
};

use crate::{
    dsl::{RuleBody, RuleId},
    error::RequirementError,
};

/// A rule a model proposed, before any human has looked at it.
///
/// It holds a body, because a candidate that proposed nothing would be nothing
/// to review. What it does not hold is any way to get that body out as
/// something executable: [`RuleCandidate::proposed_body`] exists for a reviewer
/// to read and returns a borrow, and no function in this crate turns a borrowed
/// body plus a `RuleCandidate` into a published rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCandidate {
    id: RuleId,
    source_rule: SourceRuleId,
    body: RuleBody,
    extracted_by: Actor,
    quoted_source: String,
    source_digest: ContentDigest,
}

impl RuleCandidate {
    /// Records what a model extracted.
    ///
    /// `extracted_by` is the actor that produced it. It is not validated to be
    /// a model: a candidate a person typed is still a candidate and still needs
    /// two reviewers. What is validated is the reviewer side, at the gate.
    ///
    /// `source_rule` is the identifier the **official document** gives the rule
    /// this was extracted from, and `id` is the identifier the reviewer chooses
    /// for it inside the set. They are two different namespaces and this is
    /// where the crossing is recorded. Until it was, the graduation audit's
    /// conflict gate compared the reviewer's spelling with the document's and
    /// concluded from a coincidence: the same unresolved conflict blocked a set
    /// that happened to spell the rule the document's way and did not block one
    /// that did not.
    #[must_use]
    pub fn extracted(
        id: RuleId,
        source_rule: SourceRuleId,
        body: RuleBody,
        extracted_by: Actor,
        quoted_source: String,
        source_digest: ContentDigest,
    ) -> Self {
        Self {
            id,
            source_rule,
            body,
            extracted_by,
            quoted_source,
            source_digest,
        }
    }

    /// The identifier the official document gives this rule.
    #[must_use]
    pub const fn source_rule(&self) -> &SourceRuleId {
        &self.source_rule
    }

    /// The identifier the candidate proposes for the rule.
    #[must_use]
    pub const fn id(&self) -> &RuleId {
        &self.id
    }

    /// The body a reviewer is being asked to attest to.
    #[must_use]
    pub const fn proposed_body(&self) -> &RuleBody {
        &self.body
    }

    /// Which actor extracted it.
    #[must_use]
    pub const fn extracted_by(&self) -> &Actor {
        &self.extracted_by
    }

    /// The official text the model read.
    ///
    /// This is the crate's one free-text value and it stops here. Nothing
    /// downstream has a field it fits in.
    #[must_use]
    pub fn quoted_source(&self) -> &str {
        &self.quoted_source
    }

    /// The digest of the source snapshot the quotation came from.
    #[must_use]
    pub const fn source_digest(&self) -> ContentDigest {
        self.source_digest
    }

    /// The digest of the quoted span, addressed the way the official document
    /// addresses its own rules.
    ///
    /// `academic_ingestion::rule_text_digest` is the one place that
    /// normalisation is written and `ParsedRule` is its other caller, so this
    /// value is comparable with the digest publication carries per rule rather
    /// than merely computed alike.
    #[must_use]
    pub fn quoted_source_digest(&self) -> ContentDigest {
        academic_ingestion::rule_text_digest(&self.quoted_source)
    }
}

/// One reviewer's attestation that a candidate says what the source rule it
/// names says.
///
/// The attestation names **both** identifiers, because the crossing between
/// them is the thing being attested. `source_rule` is a free parameter of
/// [`RuleCandidate::extracted`] whose own documentation calls it *what a model
/// extracted*, and `academic-audit`'s conflict gate decides applicability from
/// it: an unresolved conflict about a document rule blocks a set that publishes
/// a rule read from that rule and does not block one that does not.
/// `RuleSetDraft::include` checks that the document published the identifier,
/// which is membership -- the document publishes many, and the extraction may
/// carry any of them. Naming only the set-local identifier here left the value
/// the gate reads asserted by a model and attested by nobody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewAttestation {
    reviewer: Actor,
    candidate: RuleId,
    source_rule: SourceRuleId,
    attested_at: TimestampMillis,
}

impl ReviewAttestation {
    /// Files an attestation against one named candidate and the document rule
    /// it was read from.
    ///
    /// Both are named at filing time, so an attestation cannot be filed once
    /// and reused for a different rule or for a different reading of the
    /// document -- the gate compares both against the candidate it was called
    /// with.
    #[must_use]
    pub fn file(
        reviewer: Actor,
        candidate: RuleId,
        source_rule: SourceRuleId,
        attested_at: TimestampMillis,
    ) -> Self {
        Self {
            reviewer,
            candidate,
            source_rule,
            attested_at,
        }
    }

    /// Who attested.
    #[must_use]
    pub const fn reviewer(&self) -> &Actor {
        &self.reviewer
    }

    /// Which candidate it is about.
    #[must_use]
    pub const fn candidate(&self) -> &RuleId {
        &self.candidate
    }

    /// Which document rule the reviewer attests the candidate was read from.
    #[must_use]
    pub const fn source_rule(&self) -> &SourceRuleId {
        &self.source_rule
    }

    /// When it was filed.
    #[must_use]
    pub const fn attested_at(&self) -> TimestampMillis {
        self.attested_at
    }

    /// The reviewer's identity, when the attestation was filed by a person.
    fn user_id(&self) -> Result<EntityId, RequirementError> {
        match &self.reviewer {
            Actor::User { user_id } => Ok(*user_id),
            other => Err(RequirementError::ReviewerIsNotAUser {
                actor: other.kind_name(),
            }),
        }
    }
}

/// A candidate two people have attested to.
///
/// Every field is private and this crate declares exactly one expression that
/// builds the struct: the tail of [`ReviewGate::admit`].
/// `the_only_route_to_an_executable_rule_is_the_gate` pins that function whole
/// and counts the struct literal at one, so a second construction site is a
/// failed count as well as a changed pin.
///
/// It implements no `From`, no `Deref`, no `AsRef` and no `Into`. There is no
/// public constructor. A caller can hold one, read it, and give it to
/// [`crate::publish::RuleSetDraft::include`]; that is the whole surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedRule {
    id: RuleId,
    source_rule: SourceRuleId,
    body: RuleBody,
    first: ReviewAttestation,
    second: ReviewAttestation,
    source_digest: ContentDigest,
    quoted_source_digest: ContentDigest,
}

impl ReviewedRule {
    /// The rule's identifier.
    #[must_use]
    pub const fn id(&self) -> &RuleId {
        &self.id
    }

    /// The identifier the official document gives this rule.
    #[must_use]
    pub const fn source_rule(&self) -> &SourceRuleId {
        &self.source_rule
    }

    /// The reviewed body.
    #[must_use]
    pub const fn body(&self) -> &RuleBody {
        &self.body
    }

    /// The two attestations, in the order the gate received them.
    #[must_use]
    pub const fn attestations(&self) -> (&ReviewAttestation, &ReviewAttestation) {
        (&self.first, &self.second)
    }

    /// The digest of the source snapshot the reviewed rule rests on.
    #[must_use]
    pub const fn source_digest(&self) -> ContentDigest {
        self.source_digest
    }

    /// The digest of the official span the two reviewers read.
    ///
    /// [`crate::publish::RuleSetDraft::include`] compares it against the digest
    /// the official document publishes for [`ReviewedRule::source_rule`]. That
    /// is the half the two attestations cannot supply: they say which document
    /// rule the reviewers believe this was read from, and this says which
    /// document rule the quoted text actually is.
    #[must_use]
    pub const fn quoted_source_digest(&self) -> ContentDigest {
        self.quoted_source_digest
    }
}

/// The one door from a model-extracted candidate to a reviewable rule.
///
/// It is a unit type with one associated function rather than a value with
/// state, because a gate that could be configured is a gate that could be
/// configured open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewGate;

impl ReviewGate {
    /// Admits a candidate that two different people have attested to.
    ///
    /// The two attestations are two parameters: there is no call that supplies
    /// one. Both must name this candidate **and the document rule it claims to
    /// have been read from**, both must be filed by an `Actor::User`, and the
    /// two users must differ.
    ///
    /// The second half is what puts a person behind the crossing. What makes
    /// that crossing **checkable** is one layer on:
    /// [`ReviewedRule::quoted_source_digest`] travels out of here and
    /// [`crate::publish::RuleSetDraft::include`] compares it against the digest
    /// the official document publishes for the named rule, so two reviewers who
    /// attest a document rule the quoted text is not from are refused rather
    /// than believed.
    pub fn admit(
        candidate: RuleCandidate,
        first: ReviewAttestation,
        second: ReviewAttestation,
    ) -> Result<ReviewedRule, RequirementError> {
        for attestation in [&first, &second] {
            if attestation.candidate() != candidate.id() {
                return Err(RequirementError::AttestationNamesAnotherCandidate {
                    named: attestation.candidate().as_str().to_owned(),
                    under_review: candidate.id().as_str().to_owned(),
                });
            }
            if attestation.source_rule() != candidate.source_rule() {
                return Err(RequirementError::AttestationNamesAnotherSourceRule {
                    named: attestation.source_rule().as_str().to_owned(),
                    under_review: candidate.source_rule().as_str().to_owned(),
                });
            }
        }
        let first_user = first.user_id()?;
        let second_user = second.user_id()?;
        if first_user == second_user {
            return Err(RequirementError::OneReviewerTwice);
        }
        candidate.body.compile(&candidate.id)?;
        let quoted_source_digest = candidate.quoted_source_digest();
        Ok(ReviewedRule {
            id: candidate.id,
            source_rule: candidate.source_rule,
            body: candidate.body,
            first,
            second,
            source_digest: candidate.source_digest,
            quoted_source_digest,
        })
    }
}
