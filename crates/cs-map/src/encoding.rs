//! Section 26.2's eight visual channels, each with its own value type.
//!
//! # Eight types, not one struct of eight strings
//!
//! Every channel carries a type nothing else carries. That is what makes
//! `eight_encodings_are_independently_variable` a property of the declarations
//! rather than of a rendering: a channel computed from another channel's input
//! would have to name that input's type, and the whole set of public signatures
//! is compared for exactly that.
//!
//! # Opacity is the one this crate exists to keep separate
//!
//! Section 26.2 ends its opacity bullet with `현재 lens relevance이지 mastery가
//! 아님`. That sentence is held here the way `P2-N3` held time decay and `P2-N7`
//! held coverage: **by the absence of a vocabulary**, not by a rule.
//!
//! * [`LensRelevance`] is the only thing [`EncodedNode::opacity`] can hold.
//! * The only producer of a [`LensRelevance`] is [`crate::lens::relevance_of`],
//!   whose parameters are a lens and a subject and contain no mastery type at
//!   all.
//! * There is no `From`, `Into`, `Deref`, `AsRef`, `TryFrom` or free function
//!   between [`LensRelevance`] and [`MasteryFill`] or
//!   [`academic_domain::MasteryLevel`] in either direction, and the whole set of
//!   `impl` headers this crate declares is pinned so that adding one is an edit
//!   to a reviewed inventory rather than a new file nobody reads.
//!
//! A rule saying "do not derive opacity from mastery" is broken by the function
//! that does it anyway. A missing conversion is broken by nothing.

use std::collections::BTreeSet;

use academic_domain::{
    ConfidencePermille, EntityId, EpistemicStatus, FreshnessBand, MasteryLevel,
    predicates::PredicateName, temporal::TimeCoordinates,
};
use serde::{Serialize, Serializer};

use crate::CsMapError;

/// Renders one of section 7.2's predicates under its registry name.
///
/// `PredicateName` derives no `Serialize` of its own, and this crate does not
/// add one to it: a foreign type's wire form is that crate's decision.
fn serialize_predicate<S: Serializer>(
    value: &PredicateName,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(value.as_str())
}

/// Section 26.2's eight channels, in the bullet list's own order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VisualChannel {
    /// `node fill: mastery band.`
    NodeFill,
    /// `outer ring: freshness.`
    OuterRing,
    /// `border pattern: estimate confidence/user confirmation.`
    BorderPattern,
    /// `glyph: project observed/required/beneficial, question, gap.`
    Glyph,
    /// `edge stroke: type; dash는 inferred/predicted, solid는 confirmed.`
    EdgeStroke,
    /// `opacity: 현재 lens relevance이지 mastery가 아님.`
    Opacity,
    /// `halo: active Critical Path.`
    Halo,
    /// `timestamp badge: view의 as of.`
    TimestampBadge,
}

/// The eight channels, in section 26.2's order.
pub const VISUAL_CHANNELS: [VisualChannel; 8] = [
    VisualChannel::NodeFill,
    VisualChannel::OuterRing,
    VisualChannel::BorderPattern,
    VisualChannel::Glyph,
    VisualChannel::EdgeStroke,
    VisualChannel::Opacity,
    VisualChannel::Halo,
    VisualChannel::TimestampBadge,
];

impl VisualChannel {
    /// The channel's own words at the head of its section 26.2 bullet.
    #[must_use]
    pub const fn spec_bullet_head(self) -> &'static str {
        match self {
            Self::NodeFill => "node fill",
            Self::OuterRing => "outer ring",
            Self::BorderPattern => "border pattern",
            Self::Glyph => "glyph",
            Self::EdgeStroke => "edge stroke",
            Self::Opacity => "opacity",
            Self::Halo => "halo",
            Self::TimestampBadge => "timestamp badge",
        }
    }

    /// The stable key this channel's value is reported under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::NodeFill => "nodeFill",
            Self::OuterRing => "outerRing",
            Self::BorderPattern => "borderPattern",
            Self::Glyph => "glyph",
            Self::EdgeStroke => "edgeStroke",
            Self::Opacity => "opacity",
            Self::Halo => "halo",
            Self::TimestampBadge => "timestampBadge",
        }
    }

    /// Whether this channel is carried by a node or by an edge.
    #[must_use]
    pub const fn subject(self) -> ChannelSubject {
        match self {
            Self::EdgeStroke => ChannelSubject::Edge,
            _ => ChannelSubject::Node,
        }
    }
}

/// What a channel is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChannelSubject {
    /// The channel decorates a node.
    Node,
    /// The channel decorates an edge.
    Edge,
}

// ---------------------------------------------------------------------------
// One value type per channel
// ---------------------------------------------------------------------------

/// Channel 1. Section 13.1's ladder, displayed.
///
/// The band **is** [`MasteryLevel`]: this crate declares no second banding, so a
/// fill cannot disagree with the state that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MasteryFill(MasteryLevel);

impl MasteryFill {
    /// Displays one section 13.1 level.
    #[must_use]
    pub const fn of(level: MasteryLevel) -> Self {
        Self(level)
    }

    /// The level displayed.
    #[must_use]
    pub const fn level(self) -> MasteryLevel {
        self.0
    }
}

/// Channel 2. Section 13.3's six bands, displayed as the outer ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct FreshnessRing(FreshnessBand);

impl FreshnessRing {
    /// Displays one section 13.3 band.
    #[must_use]
    pub const fn of(band: FreshnessBand) -> Self {
        Self(band)
    }

    /// The band displayed.
    #[must_use]
    pub const fn band(self) -> FreshnessBand {
        self.0
    }
}

/// Channel 3. Estimate confidence, or the user's own confirmation.
///
/// Section 26.2's bullet names two things, and they are two arms rather than
/// two ends of one scale: a user confirmation is not a very confident estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BorderPattern {
    /// The user said so. Section 30.4's override, drawn.
    UserConfirmed,
    /// An estimate the system is confident in.
    ConfidentEstimate,
    /// An estimate the system is not confident in.
    TentativeEstimate,
    /// Nothing is estimated and nobody confirmed anything.
    Unknown,
}

/// At or above this permille an estimate draws as
/// [`BorderPattern::ConfidentEstimate`].
pub const CONFIDENT_AT_OR_ABOVE_PERMILLE: u16 = 600;

impl BorderPattern {
    /// Reads the border pattern off a claim's status and confidence.
    #[must_use]
    pub fn of(status: EpistemicStatus, confidence: Option<ConfidencePermille>) -> Self {
        match status {
            EpistemicStatus::UserConfirmed => Self::UserConfirmed,
            EpistemicStatus::Unknown => Self::Unknown,
            _ => match confidence {
                Some(value) if value.value() >= CONFIDENT_AT_OR_ABOVE_PERMILLE => {
                    Self::ConfidentEstimate
                }
                Some(_) => Self::TentativeEstimate,
                None => Self::Unknown,
            },
        }
    }
}

/// Channel 4. Section 26.2's five glyphs, which are section 19's five symbols.
///
/// Both places name the same five, and `glyph_marks_are_the_project_lens_symbols`
/// parses section 19's symbol block to say so. The symbol and the label travel
/// together because section 19 requires the shape and the label to carry the
/// meaning redundantly with the colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlyphMark {
    /// `★ OBSERVED`: actually used in code.
    ProjectObserved,
    /// `▲ REQUIRED`: needed to understand, maintain or complete this project.
    ProjectRequired,
    /// `◇ WOULD_BENEFIT`: a conditional next step.
    ProjectBeneficial,
    /// `?`: an open question exists.
    OpenQuestion,
    /// `!`: a prerequisite gap blocks the path.
    PrerequisiteGap,
}

/// The five glyphs, in section 19's own order.
pub const GLYPH_MARKS: [GlyphMark; 5] = [
    GlyphMark::ProjectObserved,
    GlyphMark::ProjectRequired,
    GlyphMark::ProjectBeneficial,
    GlyphMark::OpenQuestion,
    GlyphMark::PrerequisiteGap,
];

impl GlyphMark {
    /// Section 19's own symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::ProjectObserved => "★",
            Self::ProjectRequired => "▲",
            Self::ProjectBeneficial => "◇",
            Self::OpenQuestion => "?",
            Self::PrerequisiteGap => "!",
        }
    }

    /// Section 19's own label, which travels beside the symbol.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProjectObserved => "OBSERVED",
            Self::ProjectRequired => "REQUIRED",
            Self::ProjectBeneficial => "WOULD_BENEFIT",
            Self::OpenQuestion => "OPEN_QUESTION",
            Self::PrerequisiteGap => "PREREQUISITE_GAP",
        }
    }
}

/// Channel 5, half one. Section 26.2: dashed is inferred or predicted, solid is
/// confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DashPattern {
    /// The claim is confirmed.
    Solid,
    /// The claim is inferred or predicted.
    Dashed,
}

/// Channel 5. The stroke of one edge: which of section 7.2's twenty it is, and
/// whether the claim behind it is confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct EdgeStroke {
    /// Section 7.2's predicate, which is the stroke's *type*.
    #[serde(serialize_with = "serialize_predicate")]
    pub predicate: PredicateName,
    /// Dashed or solid.
    pub dash: DashPattern,
}

impl EdgeStroke {
    /// Reads a stroke off an edge's predicate and its claim's status.
    ///
    /// The partition is total over section 30.2's nine statuses and has exactly
    /// two classes, which is what `dash_solid_maps_to_claim_status` compares.
    /// `Disputed` and `Superseded` draw **dashed**: a disputed relation is not a
    /// confirmed one, and drawing it solid would be the most direct way for this
    /// surface to present a contested claim as settled.
    #[must_use]
    pub const fn of(predicate: PredicateName, status: EpistemicStatus) -> Self {
        let dash = match status {
            EpistemicStatus::OfficialConfirmed
            | EpistemicStatus::UserConfirmed
            | EpistemicStatus::CodeObserved
            | EpistemicStatus::DeterministicDerived => DashPattern::Solid,
            EpistemicStatus::AiInferred
            | EpistemicStatus::Prediction
            | EpistemicStatus::Disputed
            | EpistemicStatus::Superseded
            | EpistemicStatus::Unknown => DashPattern::Dashed,
        };
        Self { predicate, dash }
    }
}

/// Channel 6. How relevant a node is to the base lens, and nothing else.
///
/// Four steps rather than a percentage: a continuous opacity invites a reader to
/// compare two nodes' *amounts*, and there is no amount here to compare. This
/// type has no arithmetic and no conversion to or from a mastery value in
/// either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LensRelevance {
    /// The lens is about this node.
    Central,
    /// The lens reaches this node.
    Related,
    /// The lens does not reach this node, but it is on the map.
    Peripheral,
    /// The lens has nothing to say about this node.
    Outside,
}

/// The four relevance steps, from most to least relevant.
pub const LENS_RELEVANCES: [LensRelevance; 4] = [
    LensRelevance::Central,
    LensRelevance::Related,
    LensRelevance::Peripheral,
    LensRelevance::Outside,
];

/// Channel 7. Whether this node is on the active critical path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HaloState {
    /// The node is a step of the path the user has active.
    OnActiveCriticalPath,
    /// It is not.
    Off,
}

/// Channel 8. The `as of` of the whole view.
///
/// It carries `P2-C6`'s [`TimeCoordinates`], which is **both** coordinates:
/// as-known-at and valid-at. A badge that showed one would let a reader mistake
/// a past audit for a present understanding, which is the confusion that
/// contract exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AsOfBadge {
    /// The acceptance sequence the view is known at.
    #[serde(rename = "knownAtAcceptSeq")]
    pub known_at_accept_seq: u64,
    /// The instant the view is valid at, in Unix epoch milliseconds.
    #[serde(rename = "validAtMillis")]
    pub valid_at_millis: i64,
}

impl AsOfBadge {
    /// Draws the badge for one pair of coordinates.
    #[must_use]
    pub const fn at(coordinates: TimeCoordinates) -> Self {
        Self {
            known_at_accept_seq: coordinates.known_at_accept_seq,
            valid_at_millis: coordinates.valid_at.value(),
        }
    }
}

// ---------------------------------------------------------------------------
// The encoded values
// ---------------------------------------------------------------------------

/// What a caller knows about one node, before any channel is drawn.
///
/// Every field is an input a caller already holds. Relevance is **not** among
/// them: it is computed by [`crate::lens::relevance_of`] from the base lens, and
/// [`encode_node`] takes it as a separate argument so that a caller cannot
/// supply an opacity of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeReading {
    /// The node this is a reading of.
    pub node: EntityId,
    /// Section 13.1's level.
    pub mastery: MasteryLevel,
    /// Section 13.3's band.
    pub freshness: FreshnessBand,
    /// The status of the claim behind the displayed state.
    pub status: EpistemicStatus,
    /// That claim's confidence, when it has one.
    pub confidence: Option<ConfidencePermille>,
    /// Which of section 19's five marks apply.
    pub marks: BTreeSet<GlyphMark>,
    /// Whether the node is a step of the active critical path.
    pub on_active_critical_path: bool,
}

/// One node with all seven of its node channels drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncodedNode {
    /// The node drawn.
    pub node: EntityId,
    /// Channel 1.
    #[serde(rename = "nodeFill")]
    pub node_fill: MasteryFill,
    /// Channel 2.
    #[serde(rename = "outerRing")]
    pub outer_ring: FreshnessRing,
    /// Channel 3.
    #[serde(rename = "borderPattern")]
    pub border_pattern: BorderPattern,
    /// Channel 4.
    pub glyph: Vec<GlyphMark>,
    /// Channel 6.
    pub opacity: LensRelevance,
    /// Channel 7.
    pub halo: HaloState,
    /// Channel 8.
    #[serde(rename = "timestampBadge")]
    pub timestamp_badge: AsOfBadge,
}

/// One edge with its channel drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EncodedEdge {
    /// The subject end.
    pub from: EntityId,
    /// The object end.
    pub to: EntityId,
    /// Channel 5.
    #[serde(rename = "edgeStroke")]
    pub edge_stroke: EdgeStroke,
}

/// Draws a node's seven channels.
///
/// The signature is the contract: `relevance` arrives already computed and
/// `reading.mastery` reaches only [`MasteryFill`]. There is no path from the
/// first parameter to the sixth channel or from the fifth channel to the sixth.
#[must_use]
pub fn encode_node(
    reading: &NodeReading,
    relevance: LensRelevance,
    as_of: AsOfBadge,
) -> EncodedNode {
    EncodedNode {
        node: reading.node,
        node_fill: MasteryFill::of(reading.mastery),
        outer_ring: FreshnessRing::of(reading.freshness),
        border_pattern: BorderPattern::of(reading.status, reading.confidence),
        glyph: reading.marks.iter().copied().collect(),
        opacity: relevance,
        halo: if reading.on_active_critical_path {
            HaloState::OnActiveCriticalPath
        } else {
            HaloState::Off
        },
        timestamp_badge: as_of,
    }
}

/// Draws an edge's channel.
#[must_use]
pub const fn encode_edge(
    from: EntityId,
    to: EntityId,
    predicate: PredicateName,
    status: EpistemicStatus,
) -> EncodedEdge {
    EncodedEdge {
        from,
        to,
        edge_stroke: EdgeStroke::of(predicate, status),
    }
}

/// One channel's drawn value.
///
/// A closed sum with one arm per [`VisualChannel`], so a comparison of two
/// frames is a comparison of typed values rather than of rendered strings and
/// no arm can quietly hold another arm's type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "channel",
    content = "value",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ChannelValue {
    /// Channel 1's value.
    NodeFill(MasteryFill),
    /// Channel 2's value.
    OuterRing(FreshnessRing),
    /// Channel 3's value.
    BorderPattern(BorderPattern),
    /// Channel 4's value.
    Glyph(Vec<GlyphMark>),
    /// Channel 5's value.
    EdgeStroke(EdgeStroke),
    /// Channel 6's value.
    Opacity(LensRelevance),
    /// Channel 7's value.
    Halo(HaloState),
    /// Channel 8's value.
    TimestampBadge(AsOfBadge),
}

impl ChannelValue {
    /// Which channel this is a value of.
    #[must_use]
    pub const fn channel(&self) -> VisualChannel {
        match self {
            Self::NodeFill(_) => VisualChannel::NodeFill,
            Self::OuterRing(_) => VisualChannel::OuterRing,
            Self::BorderPattern(_) => VisualChannel::BorderPattern,
            Self::Glyph(_) => VisualChannel::Glyph,
            Self::EdgeStroke(_) => VisualChannel::EdgeStroke,
            Self::Opacity(_) => VisualChannel::Opacity,
            Self::Halo(_) => VisualChannel::Halo,
            Self::TimestampBadge(_) => VisualChannel::TimestampBadge,
        }
    }
}

/// One node and one edge drawn together, so that all eight channels have a
/// value at once.
///
/// Section 26.2's list spans both subjects, so an independence comparison that
/// looked only at a node would silently exclude the stroke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChannelFrame {
    /// The node's seven.
    pub node: EncodedNode,
    /// The edge's one.
    pub edge: EncodedEdge,
}

impl ChannelFrame {
    /// Draws all eight.
    ///
    /// # Errors
    ///
    /// [`CsMapError::EdgeEndpointIsNotANode`] when the edge does not touch the
    /// node being drawn: a frame whose two halves are about different subjects
    /// would let an independence comparison pass by comparing unrelated values.
    pub fn draw(
        reading: &NodeReading,
        relevance: LensRelevance,
        as_of: AsOfBadge,
        edge: EncodedEdge,
    ) -> Result<Self, CsMapError> {
        if edge.from != reading.node && edge.to != reading.node {
            return Err(CsMapError::EdgeEndpointIsNotANode { node: reading.node });
        }
        Ok(Self {
            node: encode_node(reading, relevance, as_of),
            edge,
        })
    }

    /// Every channel's drawn value, in section 26.2's order.
    ///
    /// The array is fixed at eight, so a channel that stopped being drawn is a
    /// type error rather than a shorter list nobody counted.
    #[must_use]
    pub fn channel_values(&self) -> [ChannelValue; VISUAL_CHANNELS.len()] {
        [
            ChannelValue::NodeFill(self.node.node_fill),
            ChannelValue::OuterRing(self.node.outer_ring),
            ChannelValue::BorderPattern(self.node.border_pattern),
            ChannelValue::Glyph(self.node.glyph.clone()),
            ChannelValue::EdgeStroke(self.edge.edge_stroke),
            ChannelValue::Opacity(self.node.opacity),
            ChannelValue::Halo(self.node.halo),
            ChannelValue::TimestampBadge(self.node.timestamp_badge),
        ]
    }
}
