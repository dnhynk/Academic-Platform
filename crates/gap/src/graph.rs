//! Section 15.2 step 2: `REQUIRES와 강한 BUILDS_ON subgraph를 확장한다`.
//!
//! ## The allowlist is not here
//!
//! `P2-C4`'s registry already carries a `prerequisite` column — `Whether a path
//! engine may traverse this edge as a prerequisite` — and exactly two of
//! section 7.2's twenty rows set it. So [`PrerequisiteEdge::admit`] calls
//! `academic_domain::predicates::prerequisite_descriptor`, which refuses the
//! other eighteen, and this crate holds **no list of admitted predicates**.
//! `RELATED_TO` is refused there rather than here, which is section 7.2's own
//! `path engine의 prerequisite로 사용 금지`.
//!
//! The registry also fixes which strengths each of the two admits.
//! `REQUIRES` admits `HARD` and `STRONG` and never `HELPFUL`; `BUILDS_ON`
//! admits `STRONG` and `HELPFUL` and never `HARD`. Both facts are read off the
//! descriptor, so `강한 BUILDS_ON` is `BuildsOn` at `Strong` because that is
//! the only strength above `Helpful` the registry lets that edge carry.
//!
//! ## `weak_builds_on_is_excluded_or_conditional`
//!
//! A `BUILDS_ON` at `HELPFUL` is section 7.2's `이해를 깊게 하지만 반드시
//! 선행해야 하는 것은 아닐 수 있음`. [`PrerequisiteEdge::blocks`] is `false` for
//! it, so the descent never crosses one and it can never produce a root
//! candidate — that is the *excluded* half, and it is a property of the value
//! rather than a filter a caller applies.
//!
//! The *conditional* half is section 15.2's `CONTEXT_GAP`, `목표나 구현 선택이
//! 불명확해 prerequisite가 갈림`. A node with **two or more** distinct helpful
//! `BUILDS_ON` objects that no success criterion names has a prerequisite set
//! that branches and a goal that has not chosen a branch — section 36.6's two
//! paths, where the design document's own answer is that the user picks. One
//! helpful edge is not a branch and stays excluded.

use crate::GapError;
use academic_domain::{
    EntityId, EvidenceId, MasteryLevel,
    predicates::{PredicateName, PrerequisiteStrength, prerequisite_descriptor},
};

/// The rung a prerequisite must reach before an edge of this strength stops
/// blocking.
///
/// This is the crate's one judgement and it is read off section 7.2's own
/// meaning cells against section 13.1's own rung glosses:
///
/// * `HARD` is `없으면 목표 수행이 신뢰성 있게 막히는`, so the goal is about
///   *performing*, and section 13.1's rung for `Used in a problem, an assignment
///   or an experiment` is `PRACTICED`.
/// * `STRONG` is the registry's `Near-hard: the goal is unreliable without it`,
///   and section 13.1's rung for `Explained in the user's own words,
///   distinguished, predicted` is `UNDERSTOOD`.
/// * `HELPFUL` is `Useful ordering, no blocking claim`, so there is no rung to
///   reach and the answer is `None` rather than a floor some caller could
///   compare against.
///
/// Total over `PrerequisiteStrength` with no wildcard arm, and pinned by
/// `the_gap_decisions_are_pinned` so it cannot drift without the pin moving.
#[must_use]
pub const fn blocking_floor(strength: PrerequisiteStrength) -> Option<MasteryLevel> {
    match strength {
        PrerequisiteStrength::Hard => Some(MasteryLevel::Practiced),
        PrerequisiteStrength::Strong => Some(MasteryLevel::Understood),
        PrerequisiteStrength::Helpful => None,
    }
}

/// `P2-C4`'s three strengths, as the wire spelling this crate writes them in.
///
/// `PrerequisiteStrength` is that crate's enumeration and this declares no
/// second one: what is here is a total match with no wildcard arm, so a fourth
/// strength added there is a compile error here rather than a value some
/// serializer quietly renames.
#[must_use]
pub const fn strength_token(strength: PrerequisiteStrength) -> &'static str {
    match strength {
        PrerequisiteStrength::Hard => "HARD",
        PrerequisiteStrength::Strong => "STRONG",
        PrerequisiteStrength::Helpful => "HELPFUL",
    }
}

/// Parses [`strength_token`]'s output back. Total over the three spellings.
pub fn strength_of_token(token: &str) -> Result<PrerequisiteStrength, GapError> {
    match token {
        "HARD" => Ok(PrerequisiteStrength::Hard),
        "STRONG" => Ok(PrerequisiteStrength::Strong),
        "HELPFUL" => Ok(PrerequisiteStrength::Helpful),
        other => Err(GapError::UnknownStrengthToken(other.to_owned())),
    }
}

/// `serde` for `P2-C4`'s `PrerequisiteStrength`, which is not itself
/// serialisable. Both halves go through [`strength_token`].
pub mod strength_serde {
    use academic_domain::predicates::PrerequisiteStrength;
    use serde::{Deserialize, Deserializer, Serializer, de};

    /// Writes the strength as its stable token.
    ///
    /// # Errors
    ///
    /// Whatever `serializer` raises.
    pub fn serialize<S>(value: &PrerequisiteStrength, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(super::strength_token(*value))
    }

    /// Reads the token back, refusing anything outside the three.
    ///
    /// # Errors
    ///
    /// A `de::Error` for a token outside the three.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<PrerequisiteStrength, D::Error>
    where
        D: Deserializer<'de>,
    {
        let token = String::deserialize(deserializer)?;
        super::strength_of_token(&token).map_err(de::Error::custom)
    }
}

/// Parses a `P2-C4` predicate name back from its own `as_str` spelling.
///
/// Scans `PredicateName::ALL`, so the registry is what admits a name and this
/// crate holds no second list of the twenty.
pub fn predicate_of_token(token: &str) -> Result<PredicateName, GapError> {
    PredicateName::ALL
        .into_iter()
        .find(|candidate| candidate.as_str() == token)
        .ok_or_else(|| GapError::UnknownPredicateToken(token.to_owned()))
}

/// `serde` for `P2-C4`'s `PredicateName`, which is not itself serialisable.
pub mod predicate_serde {
    use academic_domain::predicates::PredicateName;
    use serde::{Deserialize, Deserializer, Serializer, de};

    /// Writes the predicate as the registry's own spelling.
    ///
    /// # Errors
    ///
    /// Whatever `serializer` raises.
    pub fn serialize<S>(value: &PredicateName, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value.as_str())
    }

    /// Reads it back, refusing any name outside the registry's twenty.
    ///
    /// # Errors
    ///
    /// A `de::Error` for a name the registry does not hold.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<PredicateName, D::Error>
    where
        D: Deserializer<'de>,
    {
        let token = String::deserialize(deserializer)?;
        super::predicate_of_token(&token).map_err(de::Error::custom)
    }
}

/// One admitted section 7.2 prerequisite edge, with the evidence section 7.3
/// says every edge carries.
///
/// Private fields and one constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrerequisiteEdge {
    predicate: PredicateName,
    strength: PrerequisiteStrength,
    advanced: EntityId,
    prerequisite: EntityId,
    evidence: Vec<EvidenceId>,
}

impl PrerequisiteEdge {
    /// Admits one edge against `P2-C4`'s registry.
    ///
    /// # Errors
    ///
    /// [`GapError::NotATraversablePredicate`] when the registry's `prerequisite`
    /// column is false for `predicate` — eighteen of section 7.2's twenty rows,
    /// `RELATED_TO` among them; [`GapError::StrengthNotAdmitted`] when the
    /// registry does not let that predicate carry that strength;
    /// [`GapError::SelfEdge`] for an edge joining a concept to itself; and
    /// [`GapError::UncitedEdge`] for an edge carrying no evidence item, which is
    /// section 7.3's `edge 자체도 Claim이다`.
    pub fn admit(
        predicate: PredicateName,
        strength: PrerequisiteStrength,
        advanced: EntityId,
        prerequisite: EntityId,
        evidence: Vec<EvidenceId>,
    ) -> Result<Self, GapError> {
        let descriptor = prerequisite_descriptor(predicate)
            .map_err(|_| GapError::NotATraversablePredicate(predicate.as_str()))?;
        if !descriptor.strengths.contains(&strength) {
            return Err(GapError::StrengthNotAdmitted {
                predicate: predicate.as_str(),
                strength,
            });
        }
        if advanced == prerequisite {
            return Err(GapError::SelfEdge(predicate.as_str()));
        }
        if evidence.is_empty() {
            return Err(GapError::UncitedEdge(predicate.as_str()));
        }
        Ok(Self {
            predicate,
            strength,
            advanced,
            prerequisite,
            evidence,
        })
    }

    /// Which section 7.2 edge.
    #[must_use]
    pub const fn predicate(&self) -> PredicateName {
        self.predicate
    }

    /// The asserted strength.
    #[must_use]
    pub const fn strength(&self) -> PrerequisiteStrength {
        self.strength
    }

    /// The advanced end. Section 7.2's subject.
    #[must_use]
    pub const fn advanced(&self) -> EntityId {
        self.advanced
    }

    /// The prerequisite end. Section 7.2's object.
    #[must_use]
    pub const fn prerequisite(&self) -> EntityId {
        self.prerequisite
    }

    /// The evidence section 7.3 requires.
    #[must_use]
    pub fn evidence(&self) -> &[EvidenceId] {
        &self.evidence
    }

    /// Whether the descent may cross this edge.
    ///
    /// True exactly when [`blocking_floor`] names a rung, which is every
    /// `REQUIRES` — the registry admits only `HARD` and `STRONG` there — and a
    /// `BUILDS_ON` at `STRONG`. A helpful `BUILDS_ON` is false.
    #[must_use]
    pub const fn blocks(&self) -> bool {
        blocking_floor(self.strength).is_some()
    }

    /// The rung this edge needs on its prerequisite end, when it blocks.
    #[must_use]
    pub const fn floor(&self) -> Option<MasteryLevel> {
        blocking_floor(self.strength)
    }
}

/// The section 7.2 subgraph step 2 expands over.
///
/// Holds admitted edges only: every member passed [`PrerequisiteEdge::admit`],
/// so an edge outside `P2-C4`'s two traversable predicates has no value of the
/// type this graph stores.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrerequisiteGraph {
    edges: Vec<PrerequisiteEdge>,
}

impl PrerequisiteGraph {
    /// An empty subgraph.
    #[must_use]
    pub const fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Adds one admitted edge.
    #[must_use]
    pub fn with(mut self, edge: PrerequisiteEdge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Every edge, in insertion order.
    #[must_use]
    pub fn edges(&self) -> &[PrerequisiteEdge] {
        &self.edges
    }

    /// The blocking edges out of `advanced`, in insertion order.
    #[must_use]
    pub fn blocking_out_of(&self, advanced: EntityId) -> Vec<&PrerequisiteEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.advanced() == advanced && edge.blocks())
            .collect()
    }

    /// The non-blocking helpful `BUILDS_ON` objects out of `advanced`, deduplicated
    /// and in identifier order.
    ///
    /// These are the edges the descent excludes. They are read only to decide
    /// whether the prerequisite set branches; nothing here can put one on a
    /// blocking path.
    #[must_use]
    pub fn helpful_out_of(&self, advanced: EntityId) -> Vec<EntityId> {
        let mut found: Vec<EntityId> = self
            .edges
            .iter()
            .filter(|edge| edge.advanced() == advanced && !edge.blocks())
            .map(PrerequisiteEdge::prerequisite)
            .collect();
        found.sort_by_key(|id| id.as_uuid());
        found.dedup();
        found
    }
}
