//! `P2-X5`: the CS map atlas, its eight encodings, its ten lenses and its
//! timeline — the surface whose whole job is to say no more than the record
//! says.
//!
//! A picture is the easiest place in this product for an inference to become a
//! fact. Nothing about a drawn node announces which of its marks came from the
//! user, which from a deterministic engine and which from a model; the reader
//! sees one shape. So the four things this crate refuses are refused by
//! **separated types and missing conversions**, not by rules:
//!
//! | The thing that must not happen | What stops it |
//! |---|---|
//! | opacity reading as mastery | [`encoding::LensRelevance`] and [`encoding::MasteryFill`] are two types, the only producer of the first is [`lens::relevance_of`] whose parameters name no mastery, and the crate's whole set of `impl` headers is pinned so no `From`, `Into`, `Deref` or `AsRef` can appear between them |
//! | an inference drawn as a confirmation | [`encoding::EdgeStroke::of`] is a total `match` over section 30.2's nine statuses with exactly two classes, and `Disputed` and `Superseded` are on the dashed side |
//! | `YOU` becoming a place on the map | [`anchor::YouAnchor`] has no [`academic_domain::EntityId`], so it cannot be a [`graph::MapNode`] and cannot be declared into a [`graph::MapGraph`] |
//! | a display setting recorded as history | [`scrubber::MapTransition::UserScopeChange`] returns `None` from [`scrubber::MapTransition::change_origin`], and no conversion to `ChangeOrigin` exists |
//! | a third overlay | [`lens::LensComposition::overlay`] takes `self` by value, so a refusal leaves no composition behind |
//! | a search result teleporting | [`search::SearchReveal`] holds a cluster, a path and a node, and [`search::reveal`] is the only producer |
//!
//! # What it is not evidence for
//!
//! **No Tauri runtime is linked and no window opens.** `P2-X1` recorded why —
//! linking the runtime resolves 388 new packages, six of which the workspace's
//! network policy forbids — and this task does not change that decision. Nothing
//! here draws a pixel, measures a frame or reads a stopwatch.
//!
//! What *is* checkable without a runtime is coordinates, sets and time, and that
//! is what the suite checks: a golden coordinate file, exact subgraph
//! comparisons in both directions, an independent oracle in another language for
//! the scrubber, and whole-set inventories over the crate's own declarations.
//! `docs/contracts/cs-map-atlas.md` states the boundary in full.
//!
//! # It opens nothing and persists nothing
//!
//! No file, no socket, no clock, no `academic-store` edge and no migration.
//! Every graph, reading, lens, coordinate and instant arrives as an argument,
//! and every function here is pure.
//!
//! # What this task does not decide
//!
//! * **Accessibility conformance.** `P2-X6` owns contrast, forced colours,
//!   colour-blind palettes, the `prefers-reduced-motion` diff list and keyboard
//!   reachability. What this crate provides is the non-colour half of every
//!   encoding as a *value* — a symbol, a label, a pattern, a screen-reader name
//!   — so that there is something for that task to audit.
//! * **What a critical path is.** `P2-N6` decides it; the halo displays whether
//!   a caller says a node is on the active one.
//! * **What a blind spot is.** `P2-N7` decides it; the gap glyph displays it.
//! * **What a state is.** `P2-N2` and `P2-N3` decide mastery and freshness.
//! * **§38.** This task leaves no gate open and closes none.

pub mod anchor;
pub mod atlas;
pub mod budget;
pub mod encoding;
pub mod focus;
pub mod graph;
pub mod lens;
pub mod scrubber;
pub mod search;

use academic_domain::EntityId;
use thiserror::Error;

pub use anchor::{YOU_REFERENCE_LABEL, YouAnchor};
pub use atlas::{
    Atlas, Coordinate, InitialView, LAYOUT_TOLERANCE_MILLI, Landmark, LandmarkDrift, LevelView,
    MAX_INITIAL_CLUSTERS, MIN_INITIAL_CLUSTERS, Placement, SEMANTIC_ZOOMS, SemanticZoom, lay_out,
};
pub use budget::{ATLAS_BUDGET, BUDGET_MEASURES, BudgetMeasure, BudgetReading, RenderBudget};
pub use encoding::{
    AsOfBadge, BorderPattern, ChannelFrame, ChannelSubject, ChannelValue, DashPattern, EdgeStroke,
    EncodedEdge, EncodedNode, FreshnessRing, GLYPH_MARKS, GlyphMark, HaloState, LENS_RELEVANCES,
    LensRelevance, MasteryFill, NodeReading, VISUAL_CHANNELS, VisualChannel, encode_edge,
    encode_node,
};
pub use focus::{FOCUS_KINDS, FocusKind, FocusMode, HopCount, MAX_HOPS, MIN_HOPS, Subgraph, focus};
pub use graph::{ClusterId, MapEdge, MapGraph, MapNode};
pub use lens::{
    LayerCollision, Legend, LegendEntry, LensComposition, LensSubject, MAP_LENSES, MAX_OVERLAYS,
    MapLens, relevance_of,
};
pub use scrubber::{
    Appearance, MAP_TRANSITIONS, MapDelta, MapEvent, MapProjection, MapTransition, SplitComparison,
    Timeline, TransitionPattern,
};
pub use search::{REVEAL_STAGES, RevealStage, SearchReveal, reveal};

/// Everything this crate refuses.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CsMapError {
    /// A node was declared with a blank label, which no viewer could read.
    #[error("node {node} has a blank label")]
    EmptyLabel {
        /// The node.
        node: EntityId,
    },
    /// A node claims to be its own cluster without being a section 7.1 `FIELD`.
    #[error("node {node} is a {node_type} and cannot be a field cluster")]
    ClusterIsNotAField {
        /// The node.
        node: EntityId,
        /// Its section 7.1 type.
        node_type: &'static str,
    },
    /// Two nodes were declared under one identity.
    #[error("node {node} is declared twice")]
    DuplicateNode {
        /// The identity.
        node: EntityId,
    },
    /// A node names a cluster the graph does not declare.
    #[error("node {node} names cluster {cluster}, which is not declared")]
    NodeOutsideEveryCluster {
        /// The node.
        node: EntityId,
        /// The cluster it named.
        cluster: ClusterId,
    },
    /// An edge names an identity the node set does not hold.
    #[error("edge endpoint {node} is not a node of this graph")]
    EdgeEndpointIsNotANode {
        /// The endpoint.
        node: EntityId,
    },
    /// A layout was asked of a graph with no cluster.
    #[error("an atlas with no field cluster has nothing to lay out")]
    EmptyAtlas,
    /// The graph declares a number of clusters section 25.3 does not admit.
    #[error(
        "the first screen holds {count} field clusters; section 25.3 admits 10 through 20 \
         and this is not a number to round"
    )]
    ClusterCountOutOfRange {
        /// How many were declared.
        count: usize,
    },
    /// The selected goal is not a node of the graph.
    #[error("goal {node} is not a node of this graph")]
    GoalIsNotANode {
        /// The goal.
        node: EntityId,
    },
    /// A landmark moved further than the layout tolerance admits.
    #[error("a landmark moved {moved} thousandths, past the {tolerance} tolerance")]
    LandmarkMoved {
        /// The furthest displacement measured.
        moved: i64,
        /// What was admitted.
        tolerance: i64,
    },
    /// A landmark is present in one layout and not the other.
    #[error("cluster {cluster} has a landmark in one layout and not the other")]
    LandmarkVanished {
        /// The cluster.
        cluster: ClusterId,
    },
    /// A third overlay was offered to a full composition.
    #[error(
        "composition {base} + {first} + {second} refuses a third overlay ({refused}); \
         section 26.3 admits one base lens and at most two"
    )]
    ThirdOverlayRejected {
        /// The base lens.
        base: &'static str,
        /// The first overlay.
        first: &'static str,
        /// The second overlay.
        second: &'static str,
        /// The lens refused.
        refused: &'static str,
    },
    /// A lens already in the composition was offered again.
    #[error("{lens} is already in this composition")]
    LensAlreadyComposed {
        /// The lens.
        lens: &'static str,
    },
    /// A hop count outside section 26.4's `1–3 hop`.
    #[error("{hops} hops is outside section 26.4's 1 through 3")]
    HopsOutOfRange {
        /// What was asked for.
        hops: u8,
    },
    /// A local neighbourhood with nothing to walk along.
    #[error("a local neighbourhood needs at least one edge type to walk")]
    EmptyEdgeTypeFilter,
    /// A focus, anchor or reveal named a node the graph does not hold.
    #[error("node {node} is not on this map")]
    NodeNotOnTheMap {
        /// The node.
        node: EntityId,
    },
    /// A `YOU` anchor was placed over nothing.
    #[error("the YOU anchor is a reference point and needs something to be reckoned from")]
    AnchorHasNoReference,
    /// A blank search query, which would match every label.
    #[error("a blank query matches everything and reveals nothing")]
    EmptyQuery,
    /// Nothing matched.
    #[error("nothing on the map matches {query:?}")]
    NoMatchForQuery {
        /// The query.
        query: String,
    },
    /// More than one thing matched.
    #[error("{query:?} matches {matches} nodes; a reveal guides to one and does not choose")]
    AmbiguousQuery {
        /// The query.
        query: String,
        /// How many matched.
        matches: usize,
    },
    /// The match exists and cannot be walked to.
    #[error("node {node} matches but no path reaches it; a reveal never teleports")]
    NoPathToTarget {
        /// The match.
        node: EntityId,
    },
    /// A timeline with nothing to replay.
    #[error("a scrubber over an empty timeline agrees with everything")]
    EmptyTimeline,
    /// A split view of one reading against itself.
    #[error("both panes name the same reading; a split view compares two")]
    PanesAreTheSameReading,
    /// A visible node with no recorded reason for being visible.
    #[error("node {node} is visible with no recorded transition")]
    TransitionNotRecorded {
        /// The node.
        node: EntityId,
    },
    /// A reading broke one of the five ceilings.
    #[error("{measure} measured {measured}, past the ceiling of {ceiling}")]
    BudgetExceeded {
        /// Which ceiling.
        measure: &'static str,
        /// What was measured.
        measured: usize,
        /// What was admitted.
        ceiling: usize,
    },
}
