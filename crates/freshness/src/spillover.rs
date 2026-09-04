//! Section 13.3's fourth input: `관련 concept의 최근 사용에서 오는 약한
//! spillover`, and the sentence that bounds it —
//! `관련 concept 사용의 전파는 한 단계, 낮은 weight, 명시적 근거로 제한해
//! 연쇄적으로 전체 분야가 신선해지는 오류를 막는다`.
//!
//! Three limits, and each is a value rather than a check.
//!
//! ## `명시적 근거` — the edge is cited or there is no contribution
//!
//! [`CitedEdge`] has private fields and one constructor, which refuses an edge
//! carrying no evidence item, an edge joining a concept to itself, and any
//! predicate outside [`SPILLOVER_EDGES`]. Section 7.3 already says an edge is
//! itself a claim with evidence, so an edge nobody can cite is an edge this
//! engine cannot read.
//!
//! ## `한 단계` — the second hop needs evidence the second concept does not have
//!
//! [`NeighborUse`] is built from the **neighbour's own dated evidence** and from
//! nothing else. There is no constructor taking a
//! [`crate::projection::FreshnessProjection`], a [`Spillover`] or a band, so a
//! concept whose freshness came from a neighbour has nothing to offer a third
//! concept: `A → B → C` gives `C` no contribution because `B` has no evidence of
//! its own to date. That is `REQ-13-034`'s own case and
//! `spillover_is_one_hop_and_cited` drives it.
//!
//! **The route that survives every other limit is the one where the dated
//! evidence handed to a neighbour is not the neighbour's.** Cite the real edge
//! `B — C`, then offer `A`'s evidence as `B`'s recent use, and `A` reaches `C`
//! across two hops through a single edge with nothing malformed anywhere. That
//! is the same misattribution `P2-N2` found one layer up, where a history
//! accepted another concept's admitted evidence. [`NeighborUse::direct`] refuses
//! any dated item whose eligible evidence is linked to a concept other than the
//! neighbour, and [`crate::projection::project`] refuses the zero-hop form of
//! it.
//!
//! ## `낮은 weight` — expressed as a band comparison, not as a coefficient
//!
//! A weight is a number somebody has to check is smaller. A band is a value that
//! can be compared. [`Spillover::toward`] takes the neighbour's own band, steps
//! it down once, and takes the lower of that and [`SPILLOVER_CEILING`], so the
//! contribution is **strictly below the neighbour's own band** for every band
//! that contributes at all — and `spillover_is_one_hop_and_cited` checks exactly
//! that, over all six bands, rather than comparing two coefficients.
//!
//! A neighbour below `MODERATE` contributes nothing, because section 13.3's
//! bullet says `최근 사용` and a neighbour at `LOW` was not recently used.
//! Contributions do not accumulate either: they are combined with the higher of
//! the two rather than summed, so ten neighbours give a concept exactly what one
//! gives it. Accumulation is how `연쇄적으로 전체 분야가 신선해지는 오류` arrives
//! without any single hop being wrong.

use academic_domain::{
    EntityId, EvidenceId, FreshnessBand, TimestampMillis, predicates::PredicateName,
};

use crate::{
    band::{floor_of, rank, step_down},
    decay::decay,
    evidence::DatedEvidence,
    persistence::{RetentionPrior, elapsed_millis},
};

/// The highest band a neighbour's use can put a concept in.
///
/// `VERY_HIGH` and `HIGH` are what this concept's own evidence and the user's
/// own statement reach. A neighbour's use is not evidence about this concept at
/// all, so it may put one in the middle of the scale and no higher.
pub const SPILLOVER_CEILING: FreshnessBand = FreshnessBand::Moderate;

/// The lowest neighbour band that counts as `최근 사용`.
pub const SPILLOVER_SOURCE_FLOOR: FreshnessBand = FreshnessBand::Moderate;

/// The section 7.2 edges a spillover may be cited on.
///
/// The four whose two endpoints are both concepts. Every other section 7.2 row
/// names a lecture, an assignment, an assessment, a project snapshot, a code
/// component, a source segment, a question, an evidence item, a role profile, a
/// competency or a course revision on one side, and a concept's freshness says
/// nothing about any of those. `spillover_is_one_hop_and_cited` compares this
/// list against `PredicateName::ALL` in both directions, so a twenty-first
/// predicate is an extra key rather than a silent admission, and checks each of
/// these four against section 7.2's own table in the design document.
pub const SPILLOVER_EDGES: [PredicateName; 4] = [
    PredicateName::Requires,
    PredicateName::BuildsOn,
    PredicateName::RelatedTo,
    PredicateName::SpecialCaseOf,
];

/// A section 7.2 edge with the evidence section 7.3 says every edge carries.
///
/// Private fields and one constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedEdge {
    predicate: PredicateName,
    subject: EntityId,
    object: EntityId,
    evidence: Vec<EvidenceId>,
}

impl CitedEdge {
    /// Records one edge and the evidence it rests on.
    ///
    /// Returns `None` when the predicate is not in [`SPILLOVER_EDGES`], when the
    /// two endpoints are the same concept, or when no evidence item is cited.
    #[must_use]
    pub fn of(
        predicate: PredicateName,
        subject: EntityId,
        object: EntityId,
        evidence: Vec<EvidenceId>,
    ) -> Option<Self> {
        if !SPILLOVER_EDGES.contains(&predicate) || subject == object || evidence.is_empty() {
            return None;
        }
        Some(Self {
            predicate,
            subject,
            object,
            evidence,
        })
    }

    /// Which section 7.2 edge.
    #[must_use]
    pub const fn predicate(&self) -> PredicateName {
        self.predicate
    }

    /// The edge's subject.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// The edge's object.
    #[must_use]
    pub const fn object(&self) -> EntityId {
        self.object
    }

    /// The evidence items section 7.3 requires.
    #[must_use]
    pub fn evidence(&self) -> &[EvidenceId] {
        &self.evidence
    }

    /// Whether `concept` is one of the two endpoints.
    #[must_use]
    pub fn joins(&self, concept: EntityId) -> bool {
        self.subject == concept || self.object == concept
    }

    /// The endpoint that is not `concept`, when `concept` is one of them.
    #[must_use]
    pub fn other_end(&self, concept: EntityId) -> Option<EntityId> {
        if self.subject == concept {
            Some(self.object)
        } else if self.object == concept {
            Some(self.subject)
        } else {
            None
        }
    }
}

/// One related concept's own recent use, with the edge that licenses reading it.
///
/// Private fields and one constructor, [`NeighborUse::direct`], which takes the
/// neighbour's **own** dated evidence. Nothing here can be built out of a band,
/// a projection or another spillover, which is what makes the hop count a
/// property of the type rather than of a loop bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeighborUse {
    edge: CitedEdge,
    neighbor: EntityId,
    band: FreshnessBand,
    last_use: TimestampMillis,
}

impl NeighborUse {
    /// Reads `neighbor`'s own recent use across `edge`.
    ///
    /// Returns `None` when the edge does not join `neighbor`, when any dated
    /// item is linked to a concept other than `neighbor`, when there is no dated
    /// evidence at all, when an item is dated after `as_of`, or when the
    /// neighbour's own band is below [`SPILLOVER_SOURCE_FLOOR`] and so is not
    /// `최근 사용`.
    #[must_use]
    pub fn direct(
        edge: CitedEdge,
        neighbor: EntityId,
        dated: &[DatedEvidence],
        prior: &RetentionPrior,
        as_of: TimestampMillis,
    ) -> Option<Self> {
        if !edge.joins(neighbor) || dated.is_empty() {
            return None;
        }
        // Every item has to be about the neighbour. Without this, an edge
        // `B — C` plus `A`'s evidence offered as `B`'s use is a two-hop path
        // through one well-formed edge.
        if dated.iter().any(|item| item.concept() != neighbor) {
            return None;
        }
        let mut best: Option<(FreshnessBand, TimestampMillis)> = None;
        for item in dated {
            let elapsed = elapsed_millis(item.occurred_at(), as_of)?;
            let band = decay(elapsed, item.window(prior));
            let better = best.is_none_or(|(held, _)| rank(band) > rank(held));
            if better {
                best = Some((band, item.occurred_at()));
            }
        }
        let (band, last_use) = best?;
        if rank(band) < rank(SPILLOVER_SOURCE_FLOOR) {
            return None;
        }
        Some(Self {
            edge,
            neighbor,
            band,
            last_use,
        })
    }

    /// Which concept.
    #[must_use]
    pub const fn neighbor(&self) -> EntityId {
        self.neighbor
    }

    /// The edge this use may be read across.
    #[must_use]
    pub const fn edge(&self) -> &CitedEdge {
        &self.edge
    }

    /// The band the neighbour's own evidence puts *the neighbour* in.
    #[must_use]
    pub const fn band(&self) -> FreshnessBand {
        self.band
    }

    /// The instant of the neighbour's most recent use.
    #[must_use]
    pub const fn last_use(&self) -> TimestampMillis {
        self.last_use
    }
}

/// What one neighbour's use contributes to this concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spillover {
    edge: CitedEdge,
    subject: EntityId,
    neighbor: EntityId,
    neighbor_band: FreshnessBand,
    band: FreshnessBand,
    at: TimestampMillis,
}

impl Spillover {
    /// The contribution `use_` makes to `subject`.
    ///
    /// Returns `None` when the cited edge does not join `subject`, or when
    /// `subject` is the neighbour itself.
    #[must_use]
    pub fn toward(subject: EntityId, use_: NeighborUse) -> Option<Self> {
        if subject == use_.neighbor() || use_.edge().other_end(subject) != Some(use_.neighbor()) {
            return None;
        }
        let stepped = step_down(use_.band())?;
        Some(Self {
            band: floor_of(stepped, SPILLOVER_CEILING),
            neighbor_band: use_.band(),
            subject,
            neighbor: use_.neighbor(),
            at: use_.last_use(),
            edge: use_.edge,
        })
    }

    /// The band this contribution offers, which is strictly below
    /// [`Spillover::neighbor_band`].
    #[must_use]
    pub const fn band(&self) -> FreshnessBand {
        self.band
    }

    /// The band the neighbour itself is in.
    #[must_use]
    pub const fn neighbor_band(&self) -> FreshnessBand {
        self.neighbor_band
    }

    /// Which concept it came from.
    #[must_use]
    pub const fn neighbor(&self) -> EntityId {
        self.neighbor
    }

    /// Which concept it was computed for.
    ///
    /// A contribution carries the concept it was aimed at, so
    /// [`crate::projection::project`] refuses one built toward a different one
    /// rather than crediting this concept with a neighbour it has no edge to.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// The edge it is cited on.
    #[must_use]
    pub const fn edge(&self) -> &CitedEdge {
        &self.edge
    }

    /// The instant of the neighbour's use this rests on.
    #[must_use]
    pub const fn at(&self) -> TimestampMillis {
        self.at
    }
}
