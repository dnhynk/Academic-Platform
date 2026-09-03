//! What one concept may carry inside one goal scope, and what it may not.
//!
//! Section 18.4's first two bullets are a pair, and they say different things:
//!
//! * `OBSERVED와 REQUIRED는 동시에 가능하다. 사용 중이지만 이해 evidence가
//!   부족할 수 있다.`
//! * `REQUIRED와 WOULD_BENEFIT_FROM은 같은 goal/scope에서는 동시에 둘 수 없다.
//!   서로 다른 goal에는 가능하다.`
//!
//! So they are held by two different shapes, and the shapes are the reason
//! neither rule needs a check:
//!
//! * [`ConceptStance::observed`] is **its own field**, so it is present or
//!   absent independently of anything else. `OBSERVED` and `REQUIRED` coexist
//!   because they do not share a slot.
//! * [`ConceptStance::outlook`] is **one slot** holding one [`Outlook`], and
//!   `REQUIRED` and `WOULD_BENEFIT_FROM` are two variants of it. They cannot
//!   coexist because a slot holds one value.
//!
//! `P2-R3`'s `ImplementationDrift` argued the same distinction in the
//! other direction: its four scopes *can* hold at once, so they are four fields
//! and not one enumeration, `because one enumeration, which admits exactly one,
//! would drop two of them`. Here exactly one may hold, so it is one enumeration
//! and not two fields — two fields would admit both.
//!
//! `서로 다른 goal에는 가능하다` follows without a second mechanism:
//! [`crate::ClassificationKey`] carries the goal version, so two goals are two
//! keys and two stances.
//!
//! ## Neither classification exists without its proof
//!
//! [`Outlook`] has no payload-free variant. `Required` holds a
//! [`ProofChain`] and `Beneficial` holds a [`BenefitContract`], so a label with
//! nothing behind it has no representation — which is `REQ-34-095`'s
//! *prevention enforces REQUIRED proof schema and mandatory BENEFIT trigger*
//! written as a type. [`ObservedProof`] is the same for the third label: it is
//! built only from a `P2-R3` relation edge that a `P2-R2` finding at
//! [`EvidenceTier::Observed`] produced.

use academic_repository_analysis::{ArtifactScope, EvidenceTier, LadderRung, Locator};
use academic_repository_correlation::{EdgeEvidence, EvidenceRelation, RelationEdge};

use crate::{benefit::BenefitContract, chain::ProofChain, scope::ClassificationKey};

/// Section 18's three classifications, as the label a reader is shown.
///
/// A label alone, kept beside the proof rather than instead of it — the way
/// `P2-R3`'s `DriftScopeKind` sits beside `DriftScopes`. Section 19's legend
/// is over exactly these three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClassificationLabel {
    /// `★ OBSERVED`: section 19's `code에서 실제 사용`.
    Observed,
    /// `▲ REQUIRED`: section 19's `현 project를 이해·유지·완성하는 데 필요`.
    Required,
    /// `◇ WOULD_BENEFIT`: section 19's `조건부 다음 단계`.
    WouldBenefitFrom,
}

impl ClassificationLabel {
    /// Exhaustive order, in section 18's own section order.
    pub const ALL: [Self; 3] = [Self::Observed, Self::Required, Self::WouldBenefitFrom];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "OBSERVED",
            Self::Required => "REQUIRED",
            Self::WouldBenefitFrom => "WOULD_BENEFIT_FROM",
        }
    }

    /// Section 19's legend glyph, which is shown beside the label rather than
    /// instead of it: section 19's own sentence is `기호는 색과 함께
    /// shape/label로 중복 표현한다`.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Observed => '★',
            Self::Required => '▲',
            Self::WouldBenefitFrom => '◇',
        }
    }
}

/// Section 18.1's `OBSERVED`, carried as the evidence that produced it.
///
/// Built from a `P2-R3` relation edge and from nothing else. This crate runs no
/// second ladder: `P2-R2` decided what an observation is, `P2-R3` turned it
/// into `PROJECT_CODE_USES` or `PROJECT_TEST_EXERCISES`, and what is here is
/// that edge's own rung, tier, scope and locators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedProof {
    relation: EvidenceRelation,
    rung: LadderRung,
    artifact_scope: ArtifactScope,
    locators: Vec<Locator>,
}

impl ObservedProof {
    /// Reads an edge, and answers only for one that observed a use.
    ///
    /// [`None`] for every other edge, including one whose analysis evidence is
    /// at [`EvidenceTier::PresentOnly`] or [`EvidenceTier::Possible`]: section
    /// 18.1's own example is `package.json에 redis만 존재 ... Caching concept:
    /// NOT OBSERVED`.
    pub(crate) fn of_edge(edge: &RelationEdge) -> Option<Self> {
        let observing = matches!(
            edge.relation(),
            EvidenceRelation::CodeUses | EvidenceRelation::TestExercises
        );
        if !observing {
            return None;
        }
        match edge.evidence() {
            EdgeEvidence::Analysis {
                rung,
                tier,
                artifact_scope,
                locators,
            } if *tier == EvidenceTier::Observed => Some(Self {
                relation: edge.relation(),
                rung: *rung,
                artifact_scope: *artifact_scope,
                locators: locators.clone(),
            }),
            EdgeEvidence::Analysis { .. }
            | EdgeEvidence::Document { .. }
            | EdgeEvidence::Incident { .. } => None,
        }
    }

    /// Which of section 17.5's relations observed the use.
    #[must_use]
    pub const fn relation(&self) -> EvidenceRelation {
        self.relation
    }

    /// Which of section 17.3's five observations produced it.
    #[must_use]
    pub const fn rung(&self) -> LadderRung {
        self.rung
    }

    /// Section 18.1's scope of the use.
    #[must_use]
    pub const fn artifact_scope(&self) -> ArtifactScope {
        self.artifact_scope
    }

    /// Section 17.4's locators, carried through unchanged.
    #[must_use]
    pub fn locators(&self) -> &[Locator] {
        &self.locators
    }

    /// Always [`EvidenceTier::Observed`]; there is no other value this type is
    /// built for.
    #[must_use]
    pub const fn tier(&self) -> EvidenceTier {
        EvidenceTier::Observed
    }
}

/// The forward-looking half of a stance: exactly one of two, or none.
///
/// One slot is the whole of section 18.4's second bullet. See the module
/// documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outlook {
    /// Section 18.2, with its complete five-step chain.
    Required(ProofChain),
    /// Section 18.3, with its trigger, state, benefit and trade-offs.
    Beneficial(BenefitContract),
}

impl Outlook {
    /// Which label a reader is shown.
    #[must_use]
    pub const fn label(&self) -> ClassificationLabel {
        match self {
            Self::Required(_) => ClassificationLabel::Required,
            Self::Beneficial(_) => ClassificationLabel::WouldBenefitFrom,
        }
    }

    /// The chain, when this is a requirement.
    #[must_use]
    pub const fn chain(&self) -> Option<&ProofChain> {
        match self {
            Self::Required(chain) => Some(chain),
            Self::Beneficial(_) => None,
        }
    }

    /// The contract, when this is a conditional benefit.
    #[must_use]
    pub const fn contract(&self) -> Option<&BenefitContract> {
        match self {
            Self::Beneficial(contract) => Some(contract),
            Self::Required(_) => None,
        }
    }
}

/// Everything one concept carries under one snapshot and one goal version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptStance {
    key: ClassificationKey,
    observed: Option<ObservedProof>,
    outlook: Option<Outlook>,
}

impl ConceptStance {
    /// Builds a stance. Crate-private: [`crate::classify`] is the one producer.
    pub(crate) const fn seal(
        key: ClassificationKey,
        observed: Option<ObservedProof>,
        outlook: Option<Outlook>,
    ) -> Self {
        Self {
            key,
            observed,
            outlook,
        }
    }

    /// Snapshot, goal version and concept.
    #[must_use]
    pub const fn key(&self) -> &ClassificationKey {
        &self.key
    }

    /// The `OBSERVED` half, which coexists with either outlook.
    #[must_use]
    pub const fn observed(&self) -> Option<&ObservedProof> {
        self.observed.as_ref()
    }

    /// The forward-looking half, of which there is at most one.
    #[must_use]
    pub const fn outlook(&self) -> Option<&Outlook> {
        self.outlook.as_ref()
    }

    /// Every label this stance shows, in [`ClassificationLabel::ALL`] order.
    ///
    /// At most two, and never `REQUIRED` beside `WOULD_BENEFIT_FROM`, because
    /// the outlook contributes exactly one label or none.
    #[must_use]
    pub fn labels(&self) -> Vec<ClassificationLabel> {
        let mut labels = Vec::new();
        if self.observed.is_some() {
            labels.push(ClassificationLabel::Observed);
        }
        if let Some(outlook) = &self.outlook {
            labels.push(outlook.label());
        }
        labels
    }
}
