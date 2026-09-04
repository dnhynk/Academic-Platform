//! Section 26.5's timeline scrubber, split view and change-origin transitions.
//!
//! # The third transition kind lives here, and this is why
//!
//! Section 26.5 names three: `ontology change`, `evidence change`,
//! `user scope change`. `P2-C6`'s [`ChangeOrigin`] has four, and
//! `user scope change` is deliberately **not** one of them. Its contract records
//! the reason:
//!
//! > changing which scope is displayed changes what a viewer is shown, not what
//! > the record says. It belongs to the view that owns the scope filter, and
//! > putting it here would let a display setting be recorded as a change in
//! > canonical history.
//!
//! This crate is that view. So [`MapTransition`] has five arms: the four
//! [`ChangeOrigin`]s, each of which [`MapTransition::change_origin`] returns,
//! and [`MapTransition::UserScopeChange`], which returns `None`. There is no
//! `From<MapTransition> for ChangeOrigin` and no `TryFrom` either — the whole
//! set of `impl` headers this crate declares is pinned, so adding one is an edit
//! to a reviewed inventory. A total conversion would have to answer what
//! canonical origin a scope filter has, and it has none.
//!
//! # Both coordinates, always
//!
//! A scrubber position is a [`TimeCoordinates`], which carries `known_at` and
//! `valid_at` together. `P2-C6` gave that type no `Default`, no
//! single-coordinate constructor and no `now`, and nothing here adds one: a
//! projection this crate produces is always of a date somebody chose *and* a
//! knowledge state somebody chose.

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::{
    EntityId,
    temporal::{ChangeOrigin, TimeCoordinates},
};
use serde::Serialize;

use crate::CsMapError;

/// Why a node entered or left the view.
///
/// Four of the five are `P2-C6`'s canonical origins. The fifth is this view's
/// own and is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MapTransition {
    /// Evidence about the subject moved.
    EvidenceChange,
    /// An identity merge or split moved what the value attaches to.
    OntologyChange,
    /// The projector that computed the value changed version.
    AnalyzerUpgrade,
    /// An official source superseded an earlier official statement.
    OfficialSourceCorrection,
    /// The user changed which scope is displayed.
    ///
    /// Nothing in the record moved. This arm carries **no** [`ChangeOrigin`],
    /// and that is the whole point of it existing here rather than there.
    UserScopeChange,
}

/// The five transitions a viewer can be shown, in a stable order.
pub const MAP_TRANSITIONS: [MapTransition; 5] = [
    MapTransition::EvidenceChange,
    MapTransition::OntologyChange,
    MapTransition::AnalyzerUpgrade,
    MapTransition::OfficialSourceCorrection,
    MapTransition::UserScopeChange,
];

/// How a transition is drawn, without colour.
///
/// Section 26.2 requires shape, pattern, label and screen-reader text to carry
/// meaning redundantly with colour, so a transition's non-colour form is a value
/// rather than a stylesheet. `P2-X6` owns the contrast and forced-colours half;
/// what is fixed here is that the five forms are pairwise different.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransitionPattern {
    /// A solid sweep.
    Solid,
    /// A long dash.
    LongDash,
    /// A short dash.
    ShortDash,
    /// A dot-dash alternation.
    DotDash,
    /// A dotted outline.
    Dotted,
}

impl MapTransition {
    /// The canonical origin this transition is, when it is one.
    ///
    /// `Some` for exactly the four `P2-C6` arms and `None` for
    /// [`Self::UserScopeChange`]. `change_origin_transitions_are_distinguishable`
    /// compares the `Some` image against `academic_domain::temporal::CHANGE_ORIGINS`
    /// in both directions, so an origin added there and not here fails, and one
    /// added here that is not there fails too.
    #[must_use]
    pub const fn change_origin(self) -> Option<ChangeOrigin> {
        match self {
            Self::EvidenceChange => Some(ChangeOrigin::EvidenceChange),
            Self::OntologyChange => Some(ChangeOrigin::OntologyChange),
            Self::AnalyzerUpgrade => Some(ChangeOrigin::AnalyzerUpgrade),
            Self::OfficialSourceCorrection => Some(ChangeOrigin::OfficialSourceCorrection),
            Self::UserScopeChange => None,
        }
    }

    /// The stable wire discriminant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceChange => "EVIDENCE_CHANGE",
            Self::OntologyChange => "ONTOLOGY_CHANGE",
            Self::AnalyzerUpgrade => "ANALYZER_UPGRADE",
            Self::OfficialSourceCorrection => "OFFICIAL_SOURCE_CORRECTION",
            Self::UserScopeChange => "USER_SCOPE_CHANGE",
        }
    }

    /// The badge a viewer sees.
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::EvidenceChange => "EV",
            Self::OntologyChange => "ON",
            Self::AnalyzerUpgrade => "AN",
            Self::OfficialSourceCorrection => "OF",
            Self::UserScopeChange => "SC",
        }
    }

    /// The non-colour pattern the transition is drawn with.
    #[must_use]
    pub const fn pattern(self) -> TransitionPattern {
        match self {
            Self::EvidenceChange => TransitionPattern::Solid,
            Self::OntologyChange => TransitionPattern::LongDash,
            Self::AnalyzerUpgrade => TransitionPattern::ShortDash,
            Self::OfficialSourceCorrection => TransitionPattern::DotDash,
            Self::UserScopeChange => TransitionPattern::Dotted,
        }
    }

    /// What a screen reader announces.
    #[must_use]
    pub const fn screen_reader_name(self) -> &'static str {
        match self {
            Self::EvidenceChange => "evidence about this changed",
            Self::OntologyChange => "what this attaches to changed",
            Self::AnalyzerUpgrade => "the analyzer that computed this changed",
            Self::OfficialSourceCorrection => "an official source corrected this",
            Self::UserScopeChange => "you changed what is displayed",
        }
    }

    /// Whether the record moved, as opposed to the way it is observed or shown.
    ///
    /// Only [`Self::EvidenceChange`] is `true`, which is `P2-C6`'s
    /// `is_observation_system_change` read the other way round, extended to say
    /// that a scope change is not a change in the record either.
    #[must_use]
    pub const fn record_moved(self) -> bool {
        matches!(self, Self::EvidenceChange)
    }
}

/// Whether a node came into the view or left it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Appearance {
    /// The node became visible.
    Appears,
    /// The node stopped being visible.
    Disappears,
}

/// One thing the scrubber can replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MapEvent {
    /// The coordinates at which this happened.
    #[serde(serialize_with = "serialize_coordinates")]
    pub at: TimeCoordinates,
    /// The node.
    pub subject: EntityId,
    /// Which way.
    pub appearance: Appearance,
    /// Why.
    pub transition: MapTransition,
}

/// Renders both coordinates. `TimeCoordinates` derives no `Serialize` of its
/// own and this crate does not add one to it: a foreign type's wire form is
/// that crate's decision. Emitting one axis would be worse than emitting none.
fn serialize_coordinates<S: serde::Serializer>(
    value: &TimeCoordinates,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_map([
        ("knownAtAcceptSeq", i128::from(value.known_at_accept_seq)),
        ("validAtMillis", i128::from(value.valid_at.value())),
    ])
}

/// An ordered, append-only list of what the scrubber replays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    events: Vec<MapEvent>,
}

impl Timeline {
    /// Declares the whole timeline at once.
    ///
    /// The events are sorted here rather than trusted in the caller's order, on
    /// `(known_at, valid_at, subject, appearance)`. A projection that depended
    /// on insertion order would give two callers holding the same facts two
    /// different maps.
    ///
    /// # Errors
    ///
    /// [`CsMapError::EmptyTimeline`] when there is nothing to replay: a scrubber
    /// over an empty timeline agrees with every oracle.
    pub fn declare(mut events: Vec<MapEvent>) -> Result<Self, CsMapError> {
        if events.is_empty() {
            return Err(CsMapError::EmptyTimeline);
        }
        events.sort_by_key(|event| {
            (
                event.at.known_at_accept_seq,
                event.at.valid_at.value(),
                event.subject,
                event.appearance,
            )
        });
        Ok(Self { events })
    }

    /// The events, in replay order.
    #[must_use]
    pub fn events(&self) -> &[MapEvent] {
        &self.events
    }

    /// The graph projection at one scrubber position.
    ///
    /// An event counts when **both** coordinates admit it: its acceptance
    /// sequence is at or below the reading's, and its valid instant is at or
    /// below the reading's. `P2-C6`'s two SQL predicates, one layer up, and
    /// neither stands in for the other.
    #[must_use]
    pub fn project(&self, at: TimeCoordinates) -> MapProjection {
        let mut visible: BTreeSet<EntityId> = BTreeSet::new();
        let mut entered: BTreeMap<EntityId, MapTransition> = BTreeMap::new();
        for event in &self.events {
            if event.at.known_at_accept_seq > at.known_at_accept_seq
                || event.at.valid_at.value() > at.valid_at.value()
            {
                continue;
            }
            match event.appearance {
                Appearance::Appears => {
                    visible.insert(event.subject);
                    entered.insert(event.subject, event.transition);
                }
                Appearance::Disappears => {
                    visible.remove(&event.subject);
                    entered.remove(&event.subject);
                }
            }
        }
        MapProjection {
            known_at_accept_seq: at.known_at_accept_seq,
            valid_at_millis: at.valid_at.value(),
            visible,
            entered,
        }
    }

    /// Two positions side by side.
    ///
    /// # Errors
    ///
    /// [`CsMapError::PanesAreTheSameReading`] when both panes name the same
    /// coordinates. A split view of one reading has an empty delta list, which
    /// satisfies every comparison drawn over it.
    pub fn compare(
        &self,
        left: TimeCoordinates,
        right: TimeCoordinates,
    ) -> Result<SplitComparison, CsMapError> {
        if left == right {
            return Err(CsMapError::PanesAreTheSameReading);
        }
        let left_pane = self.project(left);
        let right_pane = self.project(right);
        let mut deltas = Vec::new();
        for node in right_pane.visible.difference(&left_pane.visible) {
            deltas.push(MapDelta {
                node: *node,
                appearance: Appearance::Appears,
                transition: recorded(&right_pane, *node)?,
            });
        }
        for node in left_pane.visible.difference(&right_pane.visible) {
            deltas.push(MapDelta {
                node: *node,
                appearance: Appearance::Disappears,
                transition: recorded(&left_pane, *node)?,
            });
        }
        deltas.sort_by_key(|delta| (delta.node, delta.appearance));
        Ok(SplitComparison {
            left: left_pane,
            right: right_pane,
            deltas,
        })
    }
}

/// The reason a pane holds a node.
///
/// Never a default: a delta with an invented cause would be this surface
/// telling a reader why something changed when it does not know.
fn recorded(pane: &MapProjection, node: EntityId) -> Result<MapTransition, CsMapError> {
    pane.entered
        .get(&node)
        .copied()
        .ok_or(CsMapError::TransitionNotRecorded { node })
}

/// The map as of one scrubber position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapProjection {
    /// The acceptance sequence this was read at.
    #[serde(rename = "knownAtAcceptSeq")]
    pub known_at_accept_seq: u64,
    /// The instant this was read at, in Unix epoch milliseconds.
    #[serde(rename = "validAtMillis")]
    pub valid_at_millis: i64,
    /// The nodes visible, in identity order.
    pub visible: BTreeSet<EntityId>,
    /// Why each visible node is here.
    pub entered: BTreeMap<EntityId, MapTransition>,
}

/// One difference between two panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MapDelta {
    /// The node that differs.
    pub node: EntityId,
    /// Which way it differs, read left to right.
    pub appearance: Appearance,
    /// Why, drawn distinguishably.
    pub transition: MapTransition,
}

/// Two independently labelled panes and the semantic difference between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SplitComparison {
    /// The left pane, carrying its own coordinates.
    pub left: MapProjection,
    /// The right pane, carrying its own coordinates.
    pub right: MapProjection,
    /// What differs, in identity order.
    pub deltas: Vec<MapDelta>,
}
