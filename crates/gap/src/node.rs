//! Which ontology tiers can carry a gap at all, and the identity standing that
//! decides whether a state claim about one is readable.

use academic_domain::{EntityId, entity_registry::EntityKind};
use serde::{Deserialize, Serialize};

/// Whether a tier can be the subject of a gap.
///
/// `P2-C3` writes the answer in its own documentation:
/// `EntityKind::Field` is `A broad area that carries no independent
/// prerequisite of its own` and `EntityKind::Alias` is `A surface form that
/// names another entity; never carries evidence itself`. A gap is an
/// evidence-backed prerequisite deficit, so neither tier can bear one — and
/// that is why section 15.3's `데이터베이스를 더 공부하세요` is refused here
/// without this crate holding a list of broad phrases. `Database` is a `FIELD`.
///
/// Total over `EntityKind` with no wildcard arm, so a sixth tier is a compile
/// error rather than a silent admission.
#[must_use]
pub const fn gap_bearing(kind: EntityKind) -> bool {
    match kind {
        EntityKind::Concept | EntityKind::ConceptSense | EntityKind::Operation => true,
        EntityKind::Field | EntityKind::Alias => false,
    }
}

/// What `P2-C3`'s registry says about this node's identity right now.
///
/// Section 15.2's `ONTOLOGY_GAP` is `synonym/granularity 오류로 잘못 분리됨`, and
/// the three unsettled variants below are `P2-C3`'s own names for exactly that:
/// its `EquivalenceClass::SplitAmbiguous` is `The pre-change identity was split,
/// so its state cannot be attributed to any one successor until the
/// reclassification queue is decided`; section 7.4 splits a homonym into a
/// `ConceptSense`; and `identity.reclassification.pending` is a registry
/// predicate. This crate declares no fourth reason of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "standing", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityStanding {
    /// The registry's identity for this node is settled and its state is
    /// attributable to it.
    Settled,
    /// A split left this identity's evidence unattributable to any one
    /// successor. `EquivalenceClass::SplitAmbiguous`.
    SplitAmbiguous {
        /// Successors the reclassification queue offers, in identifier order.
        successors: Vec<EntityId>,
    },
    /// The label is a homonym and no `ConceptSense` has been chosen for this
    /// use. Section 7.4's `homonym은 ConceptSense로 분리한다`.
    SenseUnresolved {
        /// The candidate senses, in identifier order.
        senses: Vec<EntityId>,
    },
    /// A merge or split proposal is recorded and the curator has not decided.
    /// `identity.reclassification.pending`.
    ReclassificationPending,
}

impl IdentityStanding {
    /// Whether a personal state claim about this node may be read at all.
    ///
    /// Total with no wildcard arm.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        match self {
            Self::Settled => true,
            Self::SplitAmbiguous { .. }
            | Self::SenseUnresolved { .. }
            | Self::ReclassificationPending => false,
        }
    }

    /// Stable spelling of the standing.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Settled => "SETTLED",
            Self::SplitAmbiguous { .. } => "SPLIT_AMBIGUOUS",
            Self::SenseUnresolved { .. } => "SENSE_UNRESOLVED",
            Self::ReclassificationPending => "RECLASSIFICATION_PENDING",
        }
    }
}
