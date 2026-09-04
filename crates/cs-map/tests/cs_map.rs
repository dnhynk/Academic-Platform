//! `t068` section 5's fourteen named acceptance tests for `P2-X5`, plus the
//! readings of the design document they rest on.
//!
//! Every count in this file is **enumerated, not asserted**. Four zoom levels,
//! eight channels, ten lenses, five focus modes, five glyphs, three named
//! transitions and the `10–20` cluster range are each parsed out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compared against
//! this crate's own enumerations in both directions. A number that stops being
//! the document's fails here rather than drifting.
//!
//! Every fixture is `support`'s, which is synthetic by construction: no
//! connector runs, no socket opens, no clock is read, and no label below names a
//! real course, concept or person.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::PathBuf,
};

use academic_cs_map::{
    ATLAS_BUDGET, Appearance, BorderPattern, BudgetReading, ChannelFrame, ClusterId, Coordinate,
    CsMapError, DashPattern, FOCUS_KINDS, FocusKind, FocusMode, GLYPH_MARKS, GlyphMark, HopCount,
    LAYOUT_TOLERANCE_MILLI, LensComposition, LensRelevance, LensSubject, MAP_LENSES,
    MAP_TRANSITIONS, MAX_HOPS, MAX_INITIAL_CLUSTERS, MIN_HOPS, MIN_INITIAL_CLUSTERS, MapLens,
    MapTransition, RevealStage, SEMANTIC_ZOOMS, SemanticZoom, Subgraph, VISUAL_CHANNELS,
    VisualChannel, encode_edge, encode_node, focus, lay_out, relevance_of, reveal,
};
use academic_domain::{
    EntityId, EpistemicStatus, FreshnessBand, MasteryLevel,
    predicates::{NodeType, PredicateName},
    temporal::{CHANGE_ORIGINS, ChangeOrigin},
};
use support::{
    ATLAS_CLUSTERS, TIMELINE_READINGS, TestResult, atlas_cluster_tag, atlas_concept_label,
    atlas_concept_tag, atlas_of, coordinates, entity, every_mark, permille, plain_reading, small,
    small_readings, timeline,
};

// ---------------------------------------------------------------------------
// Reading the design document
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn design_document() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// The body of one `###` subsection, up to the next heading of any depth.
fn subsection<'a>(document: &'a str, heading: &str) -> Result<&'a str, Box<dyn Error>> {
    let start = document
        .find(heading)
        .ok_or_else(|| format!("the design document no longer holds {heading}"))?;
    let rest = &document[start + heading.len()..];
    let end = rest
        .find("\n### ")
        .or_else(|| rest.find("\n## "))
        .unwrap_or(rest.len());
    Ok(&rest[..end])
}

/// Every `- ` bullet of a block, trimmed.
fn bullets(block: &str) -> Vec<String> {
    block
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- "))
        .map(|line| line[2..].trim().to_owned())
        .collect()
}

/// Every backtick-quoted run of a block.
fn back_quoted(block: &str) -> Vec<String> {
    block
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// Every `| a | b | c |` row of a block, as its cells.
fn table_rows(block: &str) -> Vec<Vec<String>> {
    block
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|') && line.ends_with('|'))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|cells| !cells.iter().all(|cell| cell.chars().all(|c| c == '-')))
        .collect()
}

// ---------------------------------------------------------------------------
// The document's own counts
// ---------------------------------------------------------------------------

/// Section 26.1's table names four levels, and they are [`SEMANTIC_ZOOMS`].
///
/// Both directions, position by position, on the row label. No number is
/// compared: the row list is enumerated and so is the enumeration.
#[test]
fn the_four_zoom_levels_are_the_design_documents_own() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.1 정보 구조")?;
    let rows = table_rows(block);
    let header = rows
        .first()
        .ok_or("section 26.1 no longer holds a zoom table")?;
    assert_eq!(header[0], "Zoom", "the zoom table's first column moved");
    let labels: Vec<String> = rows[1..].iter().map(|row| row[0].clone()).collect();
    let ours: Vec<String> = SEMANTIC_ZOOMS
        .iter()
        .map(|zoom| zoom.spec_label().to_owned())
        .collect();
    assert_eq!(labels, ours, "the zoom rows and the arms disagree");

    // The `감추는 것` column is what makes a level a level. Each row hides
    // something different, so the four hidden cells are four distinct strings.
    let hidden: BTreeSet<String> = rows[1..].iter().map(|row| row[2].clone()).collect();
    assert_eq!(
        hidden.len(),
        rows.len() - 1,
        "two zoom rows hide the same thing, so one of them is not a level"
    );
    Ok(())
}

/// Section 26.2's bullet list names eight channels, and they are
/// [`VISUAL_CHANNELS`].
#[test]
fn the_eight_channels_are_the_design_documents_own() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.2 시각 인코딩")?;
    let heads: Vec<String> = bullets(block)
        .into_iter()
        .filter_map(|bullet| {
            bullet
                .split_once(':')
                .map(|(head, _)| head.trim().to_owned())
        })
        .collect();
    let ours: Vec<String> = VISUAL_CHANNELS
        .iter()
        .map(|channel| channel.spec_bullet_head().to_owned())
        .collect();
    assert_eq!(heads, ours, "the encoding bullets and the arms disagree");

    // The keys are as closed as the arms: two channels sharing a key would let
    // a whole-set comparison over the keys pass with seven values.
    let keys: BTreeSet<&str> = VISUAL_CHANNELS.iter().map(|c| c.key()).collect();
    assert_eq!(keys.len(), VISUAL_CHANNELS.len());
    Ok(())
}

/// Section 25.3's lens line names ten lenses, and they are [`MAP_LENSES`].
#[test]
fn the_ten_lenses_are_the_design_documents_own() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 25.3 CS Map / YOU ARE HERE")?;
    let line = bullets(block)
        .into_iter()
        .find(|bullet| bullet.starts_with("상단 lens:"))
        .ok_or("section 25.3 no longer names a lens rail")?;
    let named: Vec<String> = line
        .trim_start_matches("상단 lens:")
        .trim_end_matches('.')
        .split(',')
        .map(|name| name.trim().to_owned())
        .collect();
    let ours: Vec<String> = MAP_LENSES
        .iter()
        .map(|lens| lens.spec_name().to_owned())
        .collect();
    assert_eq!(named, ours, "the lens rail and the arms disagree");
    Ok(())
}

/// Which channel a lens claims is read out of section 26.2, in both directions.
///
/// For each lens, the number of section 26.2 bullets naming it must be one when
/// [`MapLens::claimed_channel`] is `Some` and zero when it is `None`, and the
/// bullet must be that channel's. Nothing here decides which lens has a
/// channel — the document does.
#[test]
fn lens_channel_claims_are_named_in_the_encoding_bullets() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.2 시각 인코딩")?;
    let by_channel: Vec<(VisualChannel, String)> = VISUAL_CHANNELS
        .into_iter()
        .map(|channel| {
            let bullet = bullets(block)
                .into_iter()
                .find(|line| line.starts_with(channel.spec_bullet_head()))
                .unwrap_or_else(|| unreachable!("{} has no bullet", channel.spec_bullet_head()));
            (channel, bullet.to_lowercase())
        })
        .collect();

    let mut claimed = 0_usize;
    for lens in MAP_LENSES {
        let needle = lens.spec_name().to_lowercase();
        let naming: Vec<VisualChannel> = by_channel
            .iter()
            .filter(|(_, bullet)| bullet.contains(&needle))
            .map(|(channel, _)| *channel)
            .collect();
        match lens.claimed_channel() {
            Some(channel) => {
                claimed += 1;
                assert_eq!(
                    naming,
                    vec![channel],
                    "{} claims {channel:?} but section 26.2 names it in {naming:?}",
                    lens.spec_name()
                );
            }
            None => assert!(
                naming.is_empty(),
                "{} claims no channel but section 26.2 names it in {naming:?}",
                lens.spec_name()
            ),
        }
    }
    // Both halves are non-empty, so neither direction of the comparison is
    // satisfied by an empty set.
    assert_eq!(
        claimed, 4,
        "section 26.2 names a different number of lenses"
    );
    assert!(claimed < MAP_LENSES.len());
    Ok(())
}

/// Section 26.4's bullets name five focus modes, and they are [`FOCUS_KINDS`].
#[test]
fn the_five_focus_modes_are_the_design_documents_own() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.4 Focus와 progressive disclosure")?;
    let heads: Vec<String> = bullets(block)
        .into_iter()
        .filter_map(|bullet| {
            bullet
                .split_once(':')
                .map(|(head, _)| head.trim().to_owned())
        })
        .collect();
    let ours: Vec<String> = FOCUS_KINDS
        .iter()
        .map(|kind| kind.spec_bullet_head().to_owned())
        .collect();
    assert_eq!(heads, ours, "the focus bullets and the arms disagree");
    Ok(())
}

/// Section 19's five symbols are [`GLYPH_MARKS`], symbol and label together.
#[test]
fn glyph_marks_are_the_project_lens_symbols() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 기본 동작")?;
    let quoted: Vec<String> = back_quoted(block)
        .into_iter()
        .filter(|run| {
            GLYPH_MARKS
                .iter()
                .any(|mark| run.starts_with(mark.symbol()))
        })
        .collect();
    let ours: Vec<String> = GLYPH_MARKS
        .iter()
        .map(|mark| match mark {
            GlyphMark::OpenQuestion | GlyphMark::PrerequisiteGap => mark.symbol().to_owned(),
            _ => format!("{} {}", mark.symbol(), mark.label()),
        })
        .collect();
    assert_eq!(
        quoted, ours,
        "section 19's symbol block and the arms disagree"
    );

    // Symbol and label are separately unique, which is section 19's redundancy
    // requirement read as a property of the values.
    let symbols: BTreeSet<&str> = GLYPH_MARKS.iter().map(|mark| mark.symbol()).collect();
    let labels: BTreeSet<&str> = GLYPH_MARKS.iter().map(|mark| mark.label()).collect();
    assert_eq!(symbols.len(), GLYPH_MARKS.len());
    assert_eq!(labels.len(), GLYPH_MARKS.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// 1. initial_view_is_ten_to_twenty_clusters
// ---------------------------------------------------------------------------

/// Section 25.3's first screen over the five-thousand-node fixture.
///
/// The cluster set is compared against the fixture's own `FIELD` set in both
/// directions, the materialised identity set is compared against the clusters
/// plus the goal's one-hop neighbourhood in both directions, and the `10–20`
/// range is read out of section 25.3 rather than written here. A count on its
/// own would pass for a screen that showed twenty of the wrong things.
#[test]
fn initial_view_is_ten_to_twenty_clusters() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 25.3 CS Map / YOU ARE HERE")?;
    let sentence = block
        .lines()
        .find(|line| line.contains("Field cluster"))
        .ok_or("section 25.3 no longer states the first screen")?;
    assert!(
        sentence.contains(&format!(
            "{MIN_INITIAL_CLUSTERS}–{MAX_INITIAL_CLUSTERS}개 Field cluster"
        )),
        "section 25.3 states a different range: {sentence}"
    );
    assert!(
        sentence.contains("수천 node가 아니라"),
        "section 25.3 no longer says the first screen is not the whole graph"
    );

    let graph = atlas_of(5_000)?;
    let atlas = lay_out(&graph)?;
    let goal = entity(&atlas_concept_tag(0));
    let view = atlas.initial_view(&graph, goal)?;

    let declared: BTreeSet<ClusterId> = graph.clusters().iter().copied().collect();
    let shown: BTreeSet<ClusterId> = view.clusters.iter().copied().collect();
    assert_eq!(shown, declared, "the first screen is not the cluster set");
    assert!((MIN_INITIAL_CLUSTERS..=MAX_INITIAL_CLUSTERS).contains(&view.clusters.len()));

    let expected: BTreeSet<EntityId> = graph
        .clusters()
        .iter()
        .map(|cluster| cluster.entity())
        .chain([
            goal,
            entity(&atlas_cluster_tag(0)),
            entity(&atlas_concept_tag(ATLAS_CLUSTERS)),
        ])
        .collect();
    assert_eq!(
        view.materialised(),
        expected,
        "the first screen materialises something other than the clusters and the goal neighbourhood"
    );
    assert!(
        view.materialised().len() < graph.node_count(),
        "the first screen is the whole graph"
    );

    // A graph outside the range is refused rather than trimmed: the
    // hand-written fixture has two clusters and gets a refusal naming the count.
    let hand_written = small()?;
    let hand_atlas = lay_out(&hand_written)?;
    assert_eq!(
        hand_atlas.initial_view(&hand_written, entity("concept.transaction")),
        Err(CsMapError::ClusterCountOutOfRange { count: 2 })
    );
    assert_eq!(graph.clusters().len(), ATLAS_CLUSTERS);
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. you_is_not_an_ontology_node
// ---------------------------------------------------------------------------

/// `YOU` appears in no node set this crate can produce, checked exhaustively.
///
/// The claim is an absence, so it is checked by sweeping **every** surface
/// rather than by naming the one place somebody might have put it: the graph,
/// the initial view, all four zoom levels, all five focus subgraphs, every one
/// of the compositions the lens rail admits, every scrubber reading, the split
/// comparison and the search reveal. Each is required to be non-empty first, so
/// a sweep over nothing cannot pass.
#[test]
fn you_is_not_an_ontology_node() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 25.3 CS Map / YOU ARE HERE")?;
    assert!(
        block.contains("YOU는 좌표 node가 아니라"),
        "section 25.3 no longer says YOU is not a coordinate node"
    );

    let graph = small()?;
    let atlas = lay_out(&graph)?;
    let readings = small_readings();
    let you = academic_cs_map::YouAnchor::over(
        [entity("concept.transaction"), entity("concept.isolation")]
            .into_iter()
            .collect(),
        &atlas,
    )?;

    let mut swept: Vec<(String, BTreeSet<EntityId>)> = Vec::new();
    swept.push((
        "graph".to_owned(),
        graph.nodes().map(|node| node.id()).collect(),
    ));
    let big = atlas_of(5_000)?;
    let big_atlas = lay_out(&big)?;
    let goal = entity(&atlas_concept_tag(0));
    swept.push((
        "initial view".to_owned(),
        big_atlas.initial_view(&big, goal)?.materialised(),
    ));
    for zoom in SEMANTIC_ZOOMS {
        swept.push((
            format!("zoom {}", zoom.as_str()),
            big_atlas.level(&big, zoom, goal)?.nodes,
        ));
    }
    for mode in every_focus_mode() {
        let kind = mode.kind();
        swept.push((
            format!("focus {kind:?}"),
            focus(&graph, &readings, &mode)?.nodes,
        ));
    }
    let events = timeline()?;
    let mut empty_readings = 0_usize;
    for (known_at, valid_at) in TIMELINE_READINGS {
        let visible = events.project(coordinates(known_at, valid_at)).visible;
        // The earliest reading is deliberately before every event, so it holds
        // nothing. Sweeping an empty set proves nothing, so it is counted
        // separately rather than swept.
        if visible.is_empty() {
            empty_readings += 1;
            continue;
        }
        swept.push((format!("scrubber {known_at}/{valid_at}"), visible));
    }
    assert_eq!(empty_readings, 1, "the timeline's boundary reading changed");
    let split = events.compare(coordinates(10, 1_000), coordinates(70, 7_000))?;
    swept.push((
        "split left".to_owned(),
        split.left.visible.iter().copied().collect(),
    ));
    swept.push((
        "split right".to_owned(),
        split.right.visible.iter().copied().collect(),
    ));
    let route = reveal(&graph, entity("concept.transaction"), "Serializability")?;
    swept.push((
        "search reveal".to_owned(),
        route.path().iter().copied().collect(),
    ));

    // The floor. A sweep whose surfaces are empty asserts nothing.
    assert!(
        swept.len() >= 18,
        "the sweep covers only {} surfaces",
        swept.len()
    );
    for (name, nodes) in &swept {
        assert!(
            !nodes.is_empty(),
            "{name} is empty, so sweeping it proves nothing"
        );
        assert!(
            !nodes.contains(&you_identity()),
            "{name} holds a node under the anchor's identity"
        );
        for node in nodes {
            if let Some(found) = graph.node(*node).or_else(|| big.node(*node)) {
                assert_ne!(
                    found.label(),
                    academic_cs_map::YOU_REFERENCE_LABEL,
                    "{name} holds a node labelled YOU"
                );
            }
        }
    }

    // The anchor has a position and it is nobody's placement, which is the
    // positive half: it is on the map without being of the map.
    assert!(
        big_atlas
            .placements()
            .all(|placement| placement.at != you.at())
            || atlas.placements().all(|placement| placement.at != you.at()),
        "the anchor sits exactly on a node"
    );
    assert_eq!(you.label(), "YOU");
    assert_eq!(you.references().len(), 2);
    assert_eq!(
        academic_cs_map::YouAnchor::over(BTreeSet::new(), &atlas),
        Err(CsMapError::AnchorHasNoReference)
    );
    Ok(())
}

/// An identity nothing declares, standing in for "the anchor as a node".
///
/// If the anchor ever gained an identity it would be derived from its label the
/// way every other fixture identity is, so this is the identity a mistake would
/// most likely produce.
fn you_identity() -> EntityId {
    entity(academic_cs_map::YOU_REFERENCE_LABEL)
}

// ---------------------------------------------------------------------------
// 3. zoom_changes_semantic_level_not_only_scale
// ---------------------------------------------------------------------------

/// Zooming changes which nodes and which **types** are shown, and moves nothing.
///
/// Three comparisons, each of which a pure transform would fail differently:
/// the four type sets are pairwise different and none contains another; the four
/// node sets are pairwise different; and every node present at two levels sits
/// at the identical coordinate in both, which is what a scale factor could not
/// do.
#[test]
fn zoom_changes_semantic_level_not_only_scale() -> TestResult {
    let graph = atlas_of(5_000)?;
    let atlas = lay_out(&graph)?;
    let goal = entity(&atlas_concept_tag(0));

    let mut levels = Vec::new();
    for zoom in SEMANTIC_ZOOMS {
        levels.push(atlas.level(&graph, zoom, goal)?);
    }
    assert_eq!(levels.len(), SEMANTIC_ZOOMS.len());

    for left in 0..levels.len() {
        assert!(
            !levels[left].nodes.is_empty(),
            "{} shows nothing",
            levels[left].zoom.spec_label()
        );
        for right in (left + 1)..levels.len() {
            assert_ne!(
                levels[left].types,
                levels[right].types,
                "{} and {} admit the same types",
                levels[left].zoom.spec_label(),
                levels[right].zoom.spec_label()
            );
            assert_ne!(
                levels[left].nodes,
                levels[right].nodes,
                "{} and {} show the same nodes",
                levels[left].zoom.spec_label(),
                levels[right].zoom.spec_label()
            );
        }
    }

    // Not nested: `Z2` admits a type `Z1` does not and drops one it does.
    let domain = SemanticZoom::Domain.shown_types();
    let concept = SemanticZoom::Concept.shown_types();
    assert!(concept.contains(&NodeType::ConceptSense) && !domain.contains(&NodeType::ConceptSense));
    assert!(domain.contains(&NodeType::Field) && !concept.contains(&NodeType::Field));

    // Nothing moves. Every node that appears at two levels has one coordinate.
    let mut compared = 0_usize;
    for left in 0..levels.len() {
        for right in (left + 1)..levels.len() {
            for node in levels[left].nodes.intersection(&levels[right].nodes) {
                let here = atlas.placement(*node);
                assert!(here.is_some(), "a shown node has no placement");
                assert_eq!(here, atlas.placement(*node));
                compared += 1;
            }
        }
    }
    assert!(
        compared > 0,
        "no node appears at two levels, so nothing was compared"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. landmark_coordinates_stay_within_tolerance
// ---------------------------------------------------------------------------

/// The golden coordinate file, as `cluster landmark x y` rows.
fn golden_landmarks() -> Result<Vec<String>, Box<dyn Error>> {
    Ok(fs::read_to_string(
        workspace_root()
            .join("testdata")
            .join("cs-map")
            .join("landmarks.expected"),
    )?
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(str::to_owned)
    .collect())
}

fn render_landmarks(atlas: &academic_cs_map::Atlas) -> Vec<String> {
    atlas
        .landmarks()
        .iter()
        .map(|landmark| {
            format!(
                "{} {} {} {}",
                landmark.cluster, landmark.node, landmark.at.x_milli, landmark.at.y_milli
            )
        })
        .collect()
}

/// Landmarks are where the committed file says, and they stay there.
///
/// Four measurements:
///
/// 1. **Golden.** Every landmark's coordinate equals the committed file's, so a
///    change to the layout is a reviewed diff rather than a silent redraw.
/// 2. **Free of the view.** Laying the same graph out again produces identical
///    coordinates, and no lens, overlay, focus mode or zoom level is a parameter
///    of `lay_out` at all.
/// 3. **Bounded under growth.** Crossing a growth band moves every landmark by
///    at most [`LAYOUT_TOLERANCE_MILLI`], and the measured drift is required to
///    be **non-zero** — a tolerance that nothing ever approaches is a number,
///    not a bound.
/// 4. **Members move with their landmark.** Every node present in both layouts
///    moves by no more than the same tolerance.
#[test]
fn landmark_coordinates_stay_within_tolerance() -> TestResult {
    let graph = atlas_of(5_000)?;
    let atlas = lay_out(&graph)?;

    let rendered = render_landmarks(&atlas);
    assert_eq!(rendered.len(), ATLAS_CLUSTERS);

    // No two clusters share a place. Two fields drawn on top of each other are
    // one landmark a viewer can navigate by, not two, and the identity spread
    // alone does not guarantee it — the probe in `lay_out` does.
    let places: BTreeSet<(i32, i32)> = atlas
        .landmarks()
        .iter()
        .map(|landmark| (landmark.at.x_milli, landmark.at.y_milli))
        .collect();
    assert_eq!(places.len(), ATLAS_CLUSTERS, "two clusters share an anchor");
    assert_eq!(
        rendered,
        golden_landmarks()?,
        "the landmark coordinates no longer match testdata/cs-map/landmarks.expected"
    );

    let again = lay_out(&atlas_of(5_000)?)?;
    assert_eq!(
        render_landmarks(&again),
        rendered,
        "the layout is not a function"
    );
    assert_eq!(atlas.landmark_drift(&again).furthest(), 0);
    again.landmark_drift(&atlas).within_tolerance()?;

    // Growth. Three pairs of sizes, spanning four different growth bands, and
    // the bound holds for every one of them. The widest pair — band zero
    // against band seven — is required to sit **exactly** on the tolerance, so
    // the bound is known to be attained rather than merely generous.
    let mut drifts = Vec::new();
    for (left, right) in [(200_usize, 5_000_usize), (3_000, 5_000), (200, 8_000)] {
        let here = lay_out(&atlas_of(left)?)?;
        let there = lay_out(&atlas_of(right)?)?;
        assert_ne!(
            here.growth_band(),
            there.growth_band(),
            "{left} and {right} are the same growth band, so nothing was measured"
        );
        let drift = here.landmark_drift(&there);
        assert!(
            drift.vanished().is_empty(),
            "a landmark vanished across a band"
        );
        drift.within_tolerance()?;
        assert!(drift.furthest() > 0, "{left} against {right} moved nothing");
        drifts.push(drift.furthest());
    }
    assert_eq!(
        drifts[2], LAYOUT_TOLERANCE_MILLI,
        "the widest pair no longer attains the tolerance, so the bound is slack"
    );
    assert!(drifts[0] < drifts[2] && drifts[1] < drifts[2], "{drifts:?}");

    let smaller = atlas_of(3_000)?;
    let smaller_atlas = lay_out(&smaller)?;
    let mut members = 0_usize;
    for node in smaller.nodes() {
        let (Some(here), Some(there)) = (
            atlas.placement(node.id()),
            smaller_atlas.placement(node.id()),
        ) else {
            continue;
        };
        assert!(
            here.displacement(there) <= LAYOUT_TOLERANCE_MILLI,
            "a member moved {} thousandths",
            here.displacement(there)
        );
        members += 1;
    }
    assert!(members >= 2_900, "only {members} members were compared");
    Ok(())
}

/// Nothing about a view is an input to the layout.
///
/// The layout is re-derived after building every lens composition, every focus
/// subgraph and every zoom level over the same graph, and every placement is
/// compared for exact equality. This is section 19's `layout이 lens마다 바뀌면
/// 사용자의 spatial memory가 깨지므로` as a measurement.
#[test]
fn layout_is_the_same_under_every_lens_focus_and_zoom() -> TestResult {
    let graph = small()?;
    let readings = small_readings();
    let before: BTreeMap<EntityId, Coordinate> = lay_out(&graph)?
        .placements()
        .map(|placement| (placement.node, placement.at))
        .collect();
    assert!(!before.is_empty());

    let mut exercised = 0_usize;
    for composition in every_composition() {
        for subject in graph.nodes() {
            let _ = composition.relevance(&LensSubject {
                node: subject.id(),
                node_type: subject.node_type(),
                named_by: [composition.base_lens()].into_iter().collect(),
                reached_by: BTreeSet::new(),
            });
        }
        let _ = composition.legend();
        exercised += 1;
    }
    for mode in every_focus_mode() {
        let _ = focus(&graph, &readings, &mode)?;
        exercised += 1;
    }
    let atlas = lay_out(&graph)?;
    for zoom in SEMANTIC_ZOOMS {
        let _ = atlas.level(&graph, zoom, entity("concept.transaction"))?;
        exercised += 1;
    }
    assert!(exercised > 100, "only {exercised} views were exercised");

    let after: BTreeMap<EntityId, Coordinate> = lay_out(&graph)?
        .placements()
        .map(|placement| (placement.node, placement.at))
        .collect();
    assert_eq!(before, after, "a view changed the layout");
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. eight_encodings_are_independently_variable
// ---------------------------------------------------------------------------

fn baseline_frame() -> Result<ChannelFrame, CsMapError> {
    let node = entity("concept.transaction");
    ChannelFrame::draw(
        &plain_reading(node),
        LensRelevance::Peripheral,
        academic_cs_map::AsOfBadge::at(coordinates(10, 1_000)),
        encode_edge(
            node,
            entity("concept.isolation"),
            PredicateName::Requires,
            EpistemicStatus::DeterministicDerived,
        ),
    )
}

/// Varying any one channel's input changes that channel and no other.
///
/// For each of the eight, a frame is drawn from the baseline with exactly one
/// input moved, and the whole eight-value array is compared: the moved channel
/// must differ and the other seven must be identical. A channel computed from
/// another's input would move two.
#[test]
fn eight_encodings_are_independently_variable() -> TestResult {
    let baseline = baseline_frame()?;
    let node = entity("concept.transaction");
    let other = entity("concept.isolation");
    let as_of = academic_cs_map::AsOfBadge::at(coordinates(10, 1_000));
    let edge = encode_edge(
        node,
        other,
        PredicateName::Requires,
        EpistemicStatus::DeterministicDerived,
    );

    let mut variants: Vec<(VisualChannel, ChannelFrame)> = Vec::new();
    for channel in VISUAL_CHANNELS {
        let mut reading = plain_reading(node);
        let mut relevance = LensRelevance::Peripheral;
        let mut badge = as_of;
        let mut stroke = edge;
        match channel {
            VisualChannel::NodeFill => reading.mastery = MasteryLevel::Fluent,
            VisualChannel::OuterRing => reading.freshness = FreshnessBand::VeryHigh,
            VisualChannel::BorderPattern => reading.status = EpistemicStatus::UserConfirmed,
            VisualChannel::Glyph => reading.marks = every_mark(),
            VisualChannel::EdgeStroke => {
                stroke = encode_edge(
                    node,
                    other,
                    PredicateName::Requires,
                    EpistemicStatus::AiInferred,
                );
            }
            VisualChannel::Opacity => relevance = LensRelevance::Central,
            VisualChannel::Halo => reading.on_active_critical_path = true,
            VisualChannel::TimestampBadge => {
                badge = academic_cs_map::AsOfBadge::at(coordinates(11, 2_000));
            }
        }
        variants.push((
            channel,
            ChannelFrame::draw(&reading, relevance, badge, stroke)?,
        ));
    }
    assert_eq!(variants.len(), VISUAL_CHANNELS.len());

    let base = baseline.channel_values();
    for (moved, frame) in &variants {
        let values = frame.channel_values();
        assert_eq!(values.len(), VISUAL_CHANNELS.len());
        for (index, channel) in VISUAL_CHANNELS.into_iter().enumerate() {
            assert_eq!(values[index].channel(), channel, "the channel order moved");
            if channel == *moved {
                assert_ne!(
                    values[index], base[index],
                    "moving {channel:?}'s input did not move {channel:?}"
                );
            } else {
                assert_eq!(
                    values[index], base[index],
                    "moving {moved:?}'s input also moved {channel:?}"
                );
            }
        }
    }

    // A frame whose halves are about different subjects is refused, so the
    // eight values are always eight values about one thing.
    assert_eq!(
        ChannelFrame::draw(
            &plain_reading(node),
            LensRelevance::Central,
            as_of,
            encode_edge(
                entity("concept.locking"),
                entity("concept.logging"),
                PredicateName::Requires,
                EpistemicStatus::UserConfirmed,
            ),
        ),
        Err(CsMapError::EdgeEndpointIsNotANode { node })
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The serialized key sets
// ---------------------------------------------------------------------------

/// Every key a drawn frame, a projection and a legend put on the wire.
///
/// `P2-Y3` measured a defect its named acceptance tests could not see: a
/// cross-row aggregation whose *content* every test agreed with and whose
/// serialized shape nobody compared. The repair was pinning the key sets, and
/// this is that repair for this crate's three wire-facing values.
///
/// Each set is compared in both directions, so a key added is an extra and a key
/// dropped is a missing one. The eight channel keys are additionally required to
/// be exactly [`VISUAL_CHANNELS`]'s own keys, so the wire cannot drift from
/// section 26.2's list while the in-process array still matches it.
#[test]
fn the_serialized_key_sets_are_closed() -> TestResult {
    let frame = baseline_frame()?;
    let rendered = serde_json::to_value(&frame)?;

    let node = rendered
        .get("node")
        .and_then(serde_json::Value::as_object)
        .ok_or("a drawn frame no longer carries a node")?;
    let node_keys: BTreeSet<&str> = node.keys().map(String::as_str).collect();
    assert_eq!(
        node_keys,
        [
            "node",
            "nodeFill",
            "outerRing",
            "borderPattern",
            "glyph",
            "opacity",
            "halo",
            "timestampBadge",
        ]
        .into_iter()
        .collect(),
        "the drawn node's key set changed"
    );

    let edge = rendered
        .get("edge")
        .and_then(serde_json::Value::as_object)
        .ok_or("a drawn frame no longer carries an edge")?;
    assert_eq!(
        edge.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["from", "to", "edgeStroke"].into_iter().collect(),
        "the drawn edge's key set changed"
    );

    // The seven node channels plus the one edge channel are section 26.2's
    // eight, by their own keys.
    let channel_keys: BTreeSet<&str> = VISUAL_CHANNELS.iter().map(|c| c.key()).collect();
    let on_the_wire: BTreeSet<&str> = node_keys
        .union(&edge.keys().map(String::as_str).collect())
        .copied()
        .filter(|key| channel_keys.contains(key))
        .collect();
    assert_eq!(on_the_wire, channel_keys, "a channel is not on the wire");

    let events = timeline()?;
    let projection = serde_json::to_value(events.project(coordinates(70, 7_000)))?;
    assert_eq!(
        projection
            .as_object()
            .ok_or("a projection is no longer an object")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ["knownAtAcceptSeq", "validAtMillis", "visible", "entered"]
            .into_iter()
            .collect(),
        "the projection's key set changed"
    );

    let split =
        serde_json::to_value(events.compare(coordinates(20, 2_000), coordinates(60, 6_000))?)?;
    assert_eq!(
        split
            .as_object()
            .ok_or("a split comparison is no longer an object")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ["left", "right", "deltas"].into_iter().collect(),
        "the split comparison's key set changed"
    );

    let legend = serde_json::to_value(
        LensComposition::base(MapLens::Knowledge)
            .overlay(MapLens::Project)?
            .overlay(MapLens::Question)?
            .legend(),
    )?;
    assert_eq!(
        legend
            .as_object()
            .ok_or("a legend is no longer an object")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ["entries", "pinned"].into_iter().collect(),
        "the legend's key set changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. opacity_tracks_relevance_not_mastery
// ---------------------------------------------------------------------------

/// Opacity moves with the lens and never with mastery.
///
/// The behavioural half. The signature and conversion halves are in
/// `cs_map_scans.rs`, which compares the whole set of public functions producing
/// a [`LensRelevance`] and the whole set of `impl` headers this crate declares.
///
/// Here: every one of section 13.1's six mastery levels is drawn with everything
/// else fixed and the opacity is required to be the same value six times, while
/// the fill is required to take six different ones — so the comparison is not
/// passing because nothing moved. Then the base lens is varied and the opacity
/// is required to move.
#[test]
fn opacity_tracks_relevance_not_mastery() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.2 시각 인코딩")?;
    let bullet = bullets(block)
        .into_iter()
        .find(|line| line.starts_with("opacity"))
        .ok_or("section 26.2 no longer has an opacity bullet")?;
    assert!(
        bullet.contains("lens relevance") && bullet.contains("mastery가 아님"),
        "section 26.2's opacity bullet changed: {bullet}"
    );

    let node = entity("concept.transaction");
    let as_of = academic_cs_map::AsOfBadge::at(coordinates(10, 1_000));
    let subject = LensSubject {
        node,
        node_type: NodeType::Concept,
        named_by: [MapLens::Knowledge].into_iter().collect(),
        reached_by: [MapLens::Project].into_iter().collect(),
    };
    let relevance = relevance_of(MapLens::Knowledge, &subject);

    let levels = [
        MasteryLevel::Unseen,
        MasteryLevel::Exposed,
        MasteryLevel::Understood,
        MasteryLevel::Practiced,
        MasteryLevel::Applied,
        MasteryLevel::Fluent,
    ];
    let mut opacities = BTreeSet::new();
    let mut fills = BTreeSet::new();
    for level in levels {
        let mut reading = plain_reading(node);
        reading.mastery = level;
        let drawn = encode_node(&reading, relevance, as_of);
        opacities.insert(drawn.opacity);
        fills.insert(drawn.node_fill);
    }
    assert_eq!(
        fills.len(),
        levels.len(),
        "the fill did not move across six mastery levels, so nothing was held constant"
    );
    assert_eq!(
        opacities.len(),
        1,
        "the opacity moved with mastery: {opacities:?}"
    );

    // The lens is what moves it. Every one of the four steps is reachable.
    let mut reached = BTreeSet::new();
    for lens in MAP_LENSES {
        reached.insert(relevance_of(lens, &subject));
    }
    reached.insert(relevance_of(
        MapLens::Knowledge,
        &LensSubject {
            node,
            node_type: NodeType::Concept,
            named_by: BTreeSet::new(),
            reached_by: BTreeSet::new(),
        },
    ));
    assert_eq!(
        reached,
        academic_cs_map::LENS_RELEVANCES.into_iter().collect(),
        "not every relevance step is reachable, so the channel has fewer values than it declares"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. dash_solid_maps_to_claim_status
// ---------------------------------------------------------------------------

/// Every one of section 30.2's nine statuses draws dashed or solid, and the
/// partition is the one section 26.2 states.
///
/// The status list is read out of `academic_domain` rather than written here, so
/// a tenth status added there fails this test instead of silently drawing as
/// whatever the `match` arm nearest it says.
#[test]
fn dash_solid_maps_to_claim_status() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.2 시각 인코딩")?;
    let bullet = bullets(block)
        .into_iter()
        .find(|line| line.starts_with("edge stroke"))
        .ok_or("section 26.2 no longer has an edge-stroke bullet")?;
    assert!(
        bullet.contains("dash는 inferred/predicted") && bullet.contains("solid는 confirmed"),
        "section 26.2's stroke bullet changed: {bullet}"
    );

    let statuses = [
        (EpistemicStatus::OfficialConfirmed, DashPattern::Solid),
        (EpistemicStatus::UserConfirmed, DashPattern::Solid),
        (EpistemicStatus::CodeObserved, DashPattern::Solid),
        (EpistemicStatus::DeterministicDerived, DashPattern::Solid),
        (EpistemicStatus::AiInferred, DashPattern::Dashed),
        (EpistemicStatus::Prediction, DashPattern::Dashed),
        (EpistemicStatus::Disputed, DashPattern::Dashed),
        (EpistemicStatus::Superseded, DashPattern::Dashed),
        (EpistemicStatus::Unknown, DashPattern::Dashed),
    ];

    // Both classes are non-empty, so neither half of the partition is vacuous.
    let solid = statuses
        .iter()
        .filter(|(_, dash)| *dash == DashPattern::Solid)
        .count();
    assert!(solid > 0 && solid < statuses.len());

    for (status, expected) in statuses {
        for predicate in PredicateName::ALL {
            let drawn = encode_edge(
                entity("concept.transaction"),
                entity("concept.isolation"),
                predicate,
                status,
            );
            assert_eq!(
                drawn.edge_stroke.dash,
                expected,
                "{} under {status:?} draws the wrong way",
                predicate.as_str()
            );
            assert_eq!(
                drawn.edge_stroke.predicate, predicate,
                "the stroke lost its type"
            );
        }
    }

    // The whole status enumeration is covered: nine, enumerated from the domain
    // crate's own serialization rather than counted here.
    let covered: BTreeSet<String> = statuses
        .iter()
        .map(|(status, _)| format!("{status:?}"))
        .collect();
    assert_eq!(
        covered.len(),
        statuses.len(),
        "a status is listed twice, so one is not covered"
    );
    for status in [
        EpistemicStatus::OfficialConfirmed,
        EpistemicStatus::UserConfirmed,
        EpistemicStatus::CodeObserved,
        EpistemicStatus::DeterministicDerived,
        EpistemicStatus::AiInferred,
        EpistemicStatus::Prediction,
        EpistemicStatus::Disputed,
        EpistemicStatus::Superseded,
        EpistemicStatus::Unknown,
    ] {
        assert!(covered.contains(&format!("{status:?}")));
    }

    // The two the specification does not name explicitly are the ones a reader
    // could get wrong, so they are stated: a disputed relation is not confirmed.
    assert_eq!(
        encode_edge(
            entity("concept.transaction"),
            entity("concept.isolation"),
            PredicateName::Requires,
            EpistemicStatus::Disputed,
        )
        .edge_stroke
        .dash,
        DashPattern::Dashed
    );

    // The border pattern is a different channel and reads a different thing.
    assert_eq!(
        BorderPattern::of(EpistemicStatus::UserConfirmed, None),
        BorderPattern::UserConfirmed
    );
    assert_eq!(
        BorderPattern::of(EpistemicStatus::AiInferred, Some(permille(599))),
        BorderPattern::TentativeEstimate
    );
    assert_eq!(
        BorderPattern::of(EpistemicStatus::AiInferred, Some(permille(600))),
        BorderPattern::ConfidentEstimate
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. third_overlay_is_rejected  ·  9. layer_collision_warns_and_pins_legend
// ---------------------------------------------------------------------------

/// Every composition the lens rail admits: one base and up to two overlays.
fn every_composition() -> Vec<LensComposition> {
    let mut all = Vec::new();
    for base in MAP_LENSES {
        let start = LensComposition::base(base);
        all.push(start.clone());
        for first in MAP_LENSES {
            let Ok(one) = start.clone().overlay(first) else {
                continue;
            };
            all.push(one.clone());
            for second in MAP_LENSES {
                if let Ok(two) = one.clone().overlay(second) {
                    all.push(two);
                }
            }
        }
    }
    all
}

/// A third overlay is refused for every one of the ten thousand orderings.
///
/// The whole `base × overlay × overlay × overlay` product is enumerated, so the
/// refusal is measured rather than sampled. The expected outcome of each step is
/// derived in the test from the composed set, not read off the composition.
#[test]
fn third_overlay_is_rejected() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.3 Lens composition")?;
    assert!(
        block.contains("기본 lens 하나와 보조 overlay 두 개까지만"),
        "section 26.3 no longer states one base and two overlays"
    );

    let mut refused_third = 0_usize;
    let mut refused_repeat = 0_usize;
    let mut accepted = 0_usize;
    for base in MAP_LENSES {
        for first in MAP_LENSES {
            for second in MAP_LENSES {
                for third in MAP_LENSES {
                    let mut composed = vec![base];
                    let mut held = LensComposition::base(base);
                    for (position, lens) in [first, second, third].into_iter().enumerate() {
                        let repeat = composed.contains(&lens);
                        let full = composed.len() > academic_cs_map::MAX_OVERLAYS;
                        match held.clone().overlay(lens) {
                            Ok(next) => {
                                assert!(
                                    !repeat && !full,
                                    "{lens:?} was admitted and should not be"
                                );
                                composed.push(lens);
                                held = next;
                                accepted += 1;
                            }
                            Err(CsMapError::LensAlreadyComposed { .. }) => {
                                assert!(repeat, "a fresh lens was refused as a repeat");
                                refused_repeat += 1;
                            }
                            Err(CsMapError::ThirdOverlayRejected { .. }) => {
                                assert!(
                                    full && !repeat,
                                    "a {position}th overlay was refused as a third"
                                );
                                refused_third += 1;
                            }
                            Err(other) => return Err(Box::new(other)),
                        }
                    }
                    assert!(composed.len() <= academic_cs_map::MAX_OVERLAYS + 1);
                }
            }
        }
    }
    assert_eq!(
        accepted + refused_repeat + refused_third,
        MAP_LENSES.len().pow(4) * 3
    );
    assert!(refused_third > 0 && refused_repeat > 0 && accepted > 0);

    // The refusal consumes the composition, so there is nothing to retry with.
    let full = LensComposition::base(MapLens::Knowledge)
        .overlay(MapLens::Project)?
        .overlay(MapLens::Question)?;
    assert_eq!(
        full.overlay(MapLens::BlindSpot),
        Err(CsMapError::ThirdOverlayRejected {
            base: "KNOWLEDGE",
            first: "PROJECT",
            second: "QUESTION",
            refused: "BLIND_SPOT",
        })
    );
    Ok(())
}

/// A collision is exactly two composed lenses on one channel, and it pins the
/// legend.
///
/// Every admissible composition is enumerated and the expected verdict is
/// recomputed in the test from [`MapLens::claimed_channel`], which
/// `lens_channel_claims_are_named_in_the_encoding_bullets` derives from the
/// document. Both classes are required to be non-empty.
#[test]
fn layer_collision_warns_and_pins_legend() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.3 Lens composition")?;
    assert!(
        block.contains("layer collision warning을 주고 legend를 고정한다"),
        "section 26.3 no longer states the warning and the pin"
    );

    let mut collided = 0_usize;
    let mut clear = 0_usize;
    let compositions = every_composition();
    assert!(
        compositions.len() > 700,
        "only {} compositions",
        compositions.len()
    );
    for composition in &compositions {
        let mut claims: BTreeMap<VisualChannel, Vec<MapLens>> = BTreeMap::new();
        for lens in composition.composed() {
            if let Some(channel) = lens.claimed_channel() {
                claims.entry(channel).or_default().push(lens);
            }
        }
        let expected = claims
            .iter()
            .find(|(_, lenses)| lenses.len() > 1)
            .map(|(channel, lenses)| (*channel, lenses.clone()));

        let legend = composition.legend();
        assert_eq!(
            legend.channels(),
            VISUAL_CHANNELS.to_vec(),
            "the legend order moved for {:?}",
            composition.composed()
        );
        match (composition.collision(), expected) {
            (Some(actual), Some((channel, lenses))) => {
                assert_eq!(actual.channel, channel);
                assert_eq!(actual.claimants, lenses);
                assert!(
                    legend.is_pinned(),
                    "a colliding composition left the legend loose"
                );
                collided += 1;
            }
            (None, None) => {
                assert!(!legend.is_pinned(), "a clear composition pinned the legend");
                clear += 1;
            }
            (actual, expected) => {
                return Err(format!(
                    "{:?}: collision {actual:?} but {expected:?} was derived",
                    composition.composed()
                )
                .into());
            }
        }

        // The legend explains every channel whichever way it went, and the rows
        // that name a lens are exactly the claimed ones.
        for entry in legend.entries() {
            assert_eq!(
                entry.claimed_by,
                claims.get(&entry.channel).cloned().unwrap_or_default()
            );
        }
    }
    assert!(collided > 0 && clear > 0, "one of the two classes is empty");

    // Section 26.3's own example collides. Its block is read out of the
    // document, so the case is the specification's rather than one chosen to
    // pass.
    let example: Vec<String> = block
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("Base:") || line.starts_with("Overlay "))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        example,
        vec![
            "Base: Knowledge State".to_owned(),
            "Overlay 1: Project A".to_owned(),
            "Overlay 2: Open Questions".to_owned(),
        ],
        "section 26.3's example composition changed"
    );
    let specified = LensComposition::base(MapLens::Knowledge)
        .overlay(MapLens::Project)?
        .overlay(MapLens::Question)?;
    let collision = specified
        .collision()
        .ok_or("section 26.3's own example no longer collides")?;
    assert_eq!(collision.channel, VisualChannel::Glyph);
    assert_eq!(
        collision.claimants,
        vec![MapLens::Project, MapLens::Question]
    );
    assert!(specified.legend().is_pinned());
    Ok(())
}

// ---------------------------------------------------------------------------
// 10. five_focus_modes_return_exact_subgraphs
// ---------------------------------------------------------------------------

fn every_focus_mode() -> Vec<FocusMode> {
    vec![
        FocusMode::Goal {
            goal: entity("concept.transaction"),
        },
        FocusMode::LocalNeighbourhood {
            centre: entity("concept.isolation"),
            hops: HopCount::new(1).unwrap_or_else(|error| unreachable!("{error}")),
            edge_types: [PredicateName::Requires].into_iter().collect(),
        },
        FocusMode::Evidence {
            node: entity("concept.isolation"),
        },
        FocusMode::Uncertainty {
            below: permille(600),
        },
        FocusMode::Course {
            revision: entity("revision.db-2026"),
            offering_lectures: [entity("lecture.12")].into_iter().collect(),
        },
    ]
}

fn tags(names: &[&str]) -> BTreeSet<EntityId> {
    names.iter().map(|tag| entity(tag)).collect()
}

fn edge_tags(edges: &[(&str, &str, PredicateName)]) -> BTreeSet<academic_cs_map::MapEdge> {
    edges
        .iter()
        .map(|(from, to, predicate)| academic_cs_map::MapEdge {
            from: entity(from),
            to: entity(to),
            predicate: *predicate,
        })
        .collect()
}

/// Each mode returns exactly the subgraph written out below, both ways.
///
/// The expected sets are typed out from the fixture rather than derived from the
/// implementation, so a mode that widened its walk fails on the extra member
/// and one that narrowed it fails on the missing one.
#[test]
fn five_focus_modes_return_exact_subgraphs() -> TestResult {
    let graph = small()?;
    let readings = small_readings();

    let expectations: Vec<(
        FocusKind,
        BTreeSet<EntityId>,
        BTreeSet<academic_cs_map::MapEdge>,
    )> = vec![
        (
            FocusKind::Goal,
            tags(&[
                "concept.transaction",
                "concept.isolation",
                "concept.locking",
                "concept.logging",
                "concept.serializability",
                "concept.ordering",
            ]),
            edge_tags(&[
                (
                    "concept.transaction",
                    "concept.isolation",
                    PredicateName::Requires,
                ),
                (
                    "concept.isolation",
                    "concept.locking",
                    PredicateName::Requires,
                ),
                (
                    "concept.locking",
                    "concept.logging",
                    PredicateName::Requires,
                ),
                (
                    "concept.serializability",
                    "concept.transaction",
                    PredicateName::Requires,
                ),
                (
                    "concept.ordering",
                    "concept.transaction",
                    PredicateName::Requires,
                ),
            ]),
        ),
        (
            FocusKind::LocalNeighbourhood,
            tags(&[
                "concept.isolation",
                "concept.transaction",
                "concept.locking",
            ]),
            edge_tags(&[
                (
                    "concept.transaction",
                    "concept.isolation",
                    PredicateName::Requires,
                ),
                (
                    "concept.isolation",
                    "concept.locking",
                    PredicateName::Requires,
                ),
            ]),
        ),
        (
            FocusKind::Evidence,
            tags(&["concept.isolation", "evidence.lecture12", "evidence.commit"]),
            edge_tags(&[
                (
                    "concept.isolation",
                    "evidence.lecture12",
                    PredicateName::EvidencedBy,
                ),
                (
                    "concept.isolation",
                    "evidence.commit",
                    PredicateName::EvidencedBy,
                ),
            ]),
        ),
        (
            FocusKind::Uncertainty,
            tags(&["concept.isolation", "concept.locking", "concept.logging"]),
            edge_tags(&[
                (
                    "concept.isolation",
                    "concept.locking",
                    PredicateName::Requires,
                ),
                (
                    "concept.locking",
                    "concept.logging",
                    PredicateName::Requires,
                ),
            ]),
        ),
        (
            FocusKind::Course,
            tags(&[
                "revision.db-2026",
                "lecture.12",
                "concept.transaction",
                "concept.isolation",
                "concept.locking",
            ]),
            edge_tags(&[
                (
                    "revision.db-2026",
                    "concept.transaction",
                    PredicateName::DesignedToTeach,
                ),
                (
                    "revision.db-2026",
                    "concept.isolation",
                    PredicateName::DesignedToTeach,
                ),
                ("concept.transaction", "lecture.12", PredicateName::TaughtIn),
                ("concept.locking", "lecture.12", PredicateName::TaughtIn),
            ]),
        ),
    ];
    assert_eq!(expectations.len(), FOCUS_KINDS.len());

    let modes = every_focus_mode();
    let kinds: Vec<FocusKind> = modes.iter().map(FocusMode::kind).collect();
    assert_eq!(kinds, FOCUS_KINDS.to_vec(), "a focus mode is missing");

    for (mode, (kind, nodes, edges)) in modes.iter().zip(expectations) {
        let result: Subgraph = focus(&graph, &readings, mode)?;
        assert_eq!(result.kind, kind);
        assert_eq!(result.nodes, nodes, "{kind:?} returned the wrong node set");
        assert_eq!(result.edges, edges, "{kind:?} returned the wrong edge set");
        assert!(!result.nodes.is_empty());
    }

    // Course focus alone carries sides, and it carries all three.
    let course = focus(&graph, &readings, &modes[4])?;
    assert_eq!(
        course.coverage,
        [
            (
                entity("concept.transaction"),
                academic_cs_map::focus::CoverageSide::Both
            ),
            (
                entity("concept.isolation"),
                academic_cs_map::focus::CoverageSide::DesignedOnly
            ),
            (
                entity("concept.locking"),
                academic_cs_map::focus::CoverageSide::ActualOnly
            ),
        ]
        .into_iter()
        .collect()
    );
    for other in [0, 1, 2, 3] {
        assert!(focus(&graph, &readings, &modes[other])?.coverage.is_empty());
    }

    // The range and the filter are refusals, not clamps.
    assert_eq!(
        HopCount::new(0),
        Err(CsMapError::HopsOutOfRange { hops: 0 })
    );
    assert_eq!(
        HopCount::new(4),
        Err(CsMapError::HopsOutOfRange { hops: 4 })
    );
    assert!(HopCount::new(1).is_ok() && HopCount::new(3).is_ok());
    assert_eq!(
        focus(
            &graph,
            &readings,
            &FocusMode::LocalNeighbourhood {
                centre: entity("concept.isolation"),
                hops: HopCount::new(2)?,
                edge_types: BTreeSet::new(),
            },
        ),
        Err(CsMapError::EmptyEdgeTypeFilter)
    );

    // Each admitted hop count reaches strictly further than the one below it,
    // so the number is read rather than clamped. Measured on the generated
    // fixture, whose prerequisite chains are longer than three.
    let chained = atlas_of(5_000)?;
    let mut reach = Vec::new();
    for hops in MIN_HOPS..=MAX_HOPS {
        reach.push(
            focus(
                &chained,
                &BTreeMap::new(),
                &FocusMode::LocalNeighbourhood {
                    centre: entity(&atlas_concept_tag(0)),
                    hops: HopCount::new(hops)?,
                    edge_types: [PredicateName::Requires].into_iter().collect(),
                },
            )?
            .nodes
            .len(),
        );
    }
    assert!(reach[0] < reach[1] && reach[1] < reach[2], "{reach:?}");
    Ok(())
}

// ---------------------------------------------------------------------------
// 11. scrubber_matches_the_temporal_oracle
// ---------------------------------------------------------------------------

/// `tools/cs-map-scrubber-oracle.mjs`'s committed render, as rows.
fn oracle() -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let text = fs::read_to_string(
        workspace_root()
            .join("testdata")
            .join("cs-map")
            .join("scrubber.expected"),
    )?;
    let mut rows = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("oracle line has no '=': {line}"))?;
        rows.insert(key.to_owned(), value.to_owned());
    }
    Ok(rows)
}

/// Every scrubber position agrees with an independent transcription in another
/// language.
///
/// The oracle holds its own copy of the twelve rows, its own copy of the eight
/// readings and its own algorithm — it counts each subject's admitted events
/// from scratch rather than folding a running set — so a comparison against it
/// is not the scrubber agreeing with itself. `P2-U3` set this precedent and
/// `P2-U5` recorded why: a value checked against the engine that produced it
/// proves only that the engine is deterministic.
#[test]
fn scrubber_matches_the_temporal_oracle() -> TestResult {
    let rows = oracle()?;
    let events = timeline()?;

    // The floor: the oracle has to have something in it.
    assert!(rows.len() >= TIMELINE_READINGS.len(), "the oracle is short");

    for (known_at, valid_at) in TIMELINE_READINGS {
        let key = format!("visible@{known_at}/{valid_at}");
        let expected = rows
            .get(&key)
            .ok_or_else(|| format!("the oracle no longer carries {key}"))?;
        let projection = events.project(coordinates(known_at, valid_at));
        let rendered = projection
            .visible
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(&rendered, expected, "the scrubber disagrees at {key}");

        let reasons = format!("entered@{known_at}/{valid_at}");
        let expected_reasons = rows
            .get(&reasons)
            .ok_or_else(|| format!("the oracle no longer carries {reasons}"))?;
        let rendered_reasons = projection
            .entered
            .iter()
            .map(|(node, transition)| format!("{node}:{}", transition.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(&rendered_reasons, expected_reasons);
    }

    // Both axes are read. A reading whose valid time is behind an event's is
    // not shown it, even when the acceptance sequence is ahead.
    let early_valid = events.project(coordinates(70, 1_500));
    let late_valid = events.project(coordinates(70, 7_000));
    assert_ne!(
        early_valid.visible, late_valid.visible,
        "the valid-at axis changes nothing, so one coordinate is being ignored"
    );
    let early_known = events.project(coordinates(10, 7_000));
    assert_ne!(
        early_known.visible, late_valid.visible,
        "the known-at axis changes nothing, so one coordinate is being ignored"
    );

    assert_eq!(
        academic_cs_map::Timeline::declare(Vec::new()),
        Err(CsMapError::EmptyTimeline)
    );
    Ok(())
}

/// Two dates side by side, each labelled with its own coordinates.
#[test]
fn split_view_labels_two_panes_independently() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.5 Time travel")?;
    assert!(
        block.contains("두 시점을 split view로 비교할 수 있다"),
        "section 26.5 no longer states the split view"
    );

    let events = timeline()?;
    let left = coordinates(20, 2_000);
    let right = coordinates(60, 6_000);
    let split = events.compare(left, right)?;

    assert_eq!(split.left.known_at_accept_seq, 20);
    assert_eq!(split.left.valid_at_millis, 2_000);
    assert_eq!(split.right.known_at_accept_seq, 60);
    assert_eq!(split.right.valid_at_millis, 6_000);
    assert_ne!(split.left.visible, split.right.visible);
    assert!(!split.deltas.is_empty());

    // The delta is the semantic difference, both ways.
    let added: BTreeSet<EntityId> = split
        .deltas
        .iter()
        .filter(|delta| delta.appearance == Appearance::Appears)
        .map(|delta| delta.node)
        .collect();
    let removed: BTreeSet<EntityId> = split
        .deltas
        .iter()
        .filter(|delta| delta.appearance == Appearance::Disappears)
        .map(|delta| delta.node)
        .collect();
    assert_eq!(
        added,
        split
            .right
            .visible
            .difference(&split.left.visible)
            .copied()
            .collect()
    );
    assert_eq!(
        removed,
        split
            .left
            .visible
            .difference(&split.right.visible)
            .copied()
            .collect()
    );
    assert!(!added.is_empty() && !removed.is_empty());

    assert_eq!(
        events.compare(left, left),
        Err(CsMapError::PanesAreTheSameReading)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 12. change_origin_transitions_are_distinguishable
// ---------------------------------------------------------------------------

/// The five transitions are pairwise distinguishable without colour, and only
/// four of them are canonical origins.
#[test]
fn change_origin_transitions_are_distinguishable() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.5 Time travel")?;
    let sentence = block
        .lines()
        .find(|line| line.contains("다른 transition으로 표현한다"))
        .ok_or("section 26.5 no longer names its transitions")?;
    for named in ["ontology change", "evidence change", "user scope change"] {
        assert!(
            sentence.contains(named),
            "section 26.5 no longer names {named}"
        );
    }

    // Each of the three the document names maps to a distinct arm.
    let named: Vec<MapTransition> = vec![
        MapTransition::OntologyChange,
        MapTransition::EvidenceChange,
        MapTransition::UserScopeChange,
    ];
    assert_eq!(named.iter().collect::<BTreeSet<_>>().len(), 3);

    // Pairwise distinguishable on all four non-colour attributes.
    for attribute in ["wire", "badge", "pattern", "screen reader"] {
        let rendered: BTreeSet<String> = MAP_TRANSITIONS
            .iter()
            .map(|transition| match attribute {
                "wire" => transition.as_str().to_owned(),
                "badge" => transition.badge().to_owned(),
                "pattern" => format!("{:?}", transition.pattern()),
                _ => transition.screen_reader_name().to_owned(),
            })
            .collect();
        assert_eq!(
            rendered.len(),
            MAP_TRANSITIONS.len(),
            "two transitions share a {attribute}"
        );
    }

    // The `Some` image is `P2-C6`'s four origins, in both directions.
    let ours: BTreeSet<ChangeOrigin> = MAP_TRANSITIONS
        .iter()
        .filter_map(|transition| transition.change_origin())
        .collect();
    let theirs: BTreeSet<ChangeOrigin> = CHANGE_ORIGINS.into_iter().collect();
    assert_eq!(ours, theirs, "the canonical origins and the arms disagree");

    // Exactly one arm is not an origin, and it is the scope change.
    let orphans: Vec<MapTransition> = MAP_TRANSITIONS
        .into_iter()
        .filter(|transition| transition.change_origin().is_none())
        .collect();
    assert_eq!(orphans, vec![MapTransition::UserScopeChange]);
    assert_eq!(MAP_TRANSITIONS.len(), CHANGE_ORIGINS.len() + 1);

    // Only an evidence change means the record moved. The other four are the
    // observation system or the display, which is the distinction `P2-C6` drew.
    let moved: Vec<MapTransition> = MAP_TRANSITIONS
        .into_iter()
        .filter(|transition| transition.record_moved())
        .collect();
    assert_eq!(moved, vec![MapTransition::EvidenceChange]);
    for transition in MAP_TRANSITIONS {
        if let Some(origin) = transition.change_origin() {
            assert_eq!(
                transition.record_moved(),
                !origin.is_observation_system_change()
            );
        }
    }

    // Every arm actually reaches a viewer: each one appears in the fixture's
    // timeline and is carried into a projection.
    let events = timeline()?;
    let mut carried: BTreeSet<MapTransition> = BTreeSet::new();
    for (known_at, valid_at) in TIMELINE_READINGS {
        for transition in events
            .project(coordinates(known_at, valid_at))
            .entered
            .values()
        {
            carried.insert(*transition);
        }
    }
    assert_eq!(
        carried,
        MAP_TRANSITIONS.into_iter().collect(),
        "a transition is declared and never shown"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 13. search_reveals_in_three_stages
// ---------------------------------------------------------------------------

/// A reveal is a cluster, then a route, then the node — never the node alone.
#[test]
fn search_reveals_in_three_stages() -> TestResult {
    let document = design_document()?;
    let block = subsection(&document, "### 26.4 Focus와 progressive disclosure")?;
    let sentence = block
        .lines()
        .find(|line| line.contains("cluster → path → node"))
        .ok_or("section 26.4 no longer states the three stages")?;
    assert!(sentence.contains("순간이동시키지 않고"));

    let graph = small()?;
    let route = reveal(&graph, entity("concept.transaction"), "Serializability")?;
    let stages = route.stages();
    assert_eq!(stages.len(), academic_cs_map::REVEAL_STAGES);
    assert_eq!(stages[0], RevealStage::Cluster(route.cluster()));
    assert_eq!(stages[1], RevealStage::Path(route.path().to_vec()));
    assert_eq!(stages[2], RevealStage::Node(route.node()));

    // The route is real: consecutive steps are edges of the graph, it starts
    // where the viewer stood and it ends at the match.
    let path = route.path();
    assert!(path.len() >= 2, "a reveal from elsewhere has a route");
    assert_eq!(path[0], entity("concept.transaction"));
    assert_eq!(path[path.len() - 1], entity("concept.serializability"));
    for pair in path.windows(2) {
        assert!(
            graph.edges().iter().any(|edge| {
                (edge.from == pair[0] && edge.to == pair[1])
                    || (edge.from == pair[1] && edge.to == pair[0])
            }),
            "the route steps between two nodes with no edge"
        );
    }
    assert_eq!(
        route.cluster(),
        graph
            .node(entity("concept.serializability"))
            .ok_or("the match left the graph")?
            .cluster()
    );

    // A match nothing can walk to is refused rather than teleported. The goal
    // node is in the fixture and has no edge at all.
    assert_eq!(
        reveal(
            &graph,
            entity("concept.transaction"),
            "Reliable collaboration"
        ),
        Err(CsMapError::NoPathToTarget {
            node: entity("goal.reliable-collaboration"),
        })
    );
    assert_eq!(
        reveal(&graph, entity("concept.transaction"), "Transaction"),
        Err(CsMapError::AmbiguousQuery {
            query: "Transaction".to_owned(),
            matches: 3,
        })
    );
    assert_eq!(
        reveal(&graph, entity("concept.transaction"), "   "),
        Err(CsMapError::EmptyQuery)
    );
    assert_eq!(
        reveal(&graph, entity("concept.transaction"), "Kubernetes"),
        Err(CsMapError::NoMatchForQuery {
            query: "Kubernetes".to_owned(),
        })
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 14. five_thousand_node_fixture_meets_the_budget
// ---------------------------------------------------------------------------

fn measure(graph: &academic_cs_map::MapGraph) -> Result<BudgetReading, Box<dyn Error>> {
    let atlas = lay_out(graph)?;
    let goal = entity(&atlas_concept_tag(0));
    let route = reveal(graph, goal, &atlas_concept_label(48))?;
    Ok(BudgetReading {
        node_count: graph.node_count(),
        initial_view_nodes: atlas.initial_view(graph, goal)?.materialised().len(),
        goal_near_nodes: atlas.level(graph, SemanticZoom::Concept, goal)?.nodes.len(),
        evidence_nodes: atlas
            .level(graph, SemanticZoom::Evidence, goal)?
            .nodes
            .len(),
        layout_work_units: atlas.work_units(),
        search_path_hops: route.path().len().saturating_sub(1),
    })
}

/// A five-thousand-node fixture is inside every one of the five ceilings, and
/// each ceiling is shown to bite.
///
/// The reading is counted off values the crate produced. The second half moves
/// each measure one past its ceiling on its own and requires a refusal naming
/// that measure, so no ceiling is a number that nothing could ever exceed.
#[test]
fn five_thousand_node_fixture_meets_the_budget() -> TestResult {
    let graph = atlas_of(5_000)?;
    assert_eq!(graph.node_count(), 5_000);
    let reading = measure(&graph)?;
    reading.within(&ATLAS_BUDGET)?;

    // The measured reading, pinned. These are the numbers
    // `docs/contracts/cs-map-atlas.md`'s budget section refers to, and pinning
    // them means a fixture or layout change that moved them is a reviewed diff
    // rather than a silently different measurement under the same ceilings.
    assert_eq!(reading.initial_view_nodes, 18);
    assert_eq!(reading.goal_near_nodes, 3);
    assert_eq!(reading.evidence_nodes, 2);
    assert_eq!(reading.layout_work_units, 5_032);
    assert_eq!(reading.search_path_hops, 3);

    // The layout is linear, not merely inside a large number.
    let smaller = atlas_of(2_500)?;
    let smaller_reading = measure(&smaller)?;
    smaller_reading.within(&ATLAS_BUDGET)?;
    assert!(
        reading.layout_work_units < smaller_reading.layout_work_units * 3,
        "the layout grew faster than the graph: {} against {}",
        reading.layout_work_units,
        smaller_reading.layout_work_units
    );

    // The first screen is not the graph, which is the whole point of the budget.
    assert!(reading.initial_view_nodes * 100 < reading.node_count);

    for measure_kind in academic_cs_map::BUDGET_MEASURES {
        let mut broken = reading;
        let ceiling = match measure_kind {
            academic_cs_map::BudgetMeasure::InitialViewNodes => {
                broken.initial_view_nodes = ATLAS_BUDGET.initial_view_nodes + 1;
                ATLAS_BUDGET.initial_view_nodes
            }
            academic_cs_map::BudgetMeasure::GoalNearNodes => {
                broken.goal_near_nodes = ATLAS_BUDGET.goal_near_nodes + 1;
                ATLAS_BUDGET.goal_near_nodes
            }
            academic_cs_map::BudgetMeasure::EvidenceNodes => {
                broken.evidence_nodes = ATLAS_BUDGET.evidence_nodes + 1;
                ATLAS_BUDGET.evidence_nodes
            }
            academic_cs_map::BudgetMeasure::LayoutWorkUnits => {
                broken.layout_work_units =
                    ATLAS_BUDGET.layout_work_units_per_node * reading.node_count + 1;
                ATLAS_BUDGET.layout_work_units_per_node * reading.node_count
            }
            academic_cs_map::BudgetMeasure::SearchPathHops => {
                broken.search_path_hops = ATLAS_BUDGET.search_path_hops + 1;
                ATLAS_BUDGET.search_path_hops
            }
        };
        assert_eq!(
            broken.within(&ATLAS_BUDGET),
            Err(CsMapError::BudgetExceeded {
                measure: measure_kind.as_str(),
                measured: ceiling + 1,
                ceiling,
            }),
            "{} does not bite",
            measure_kind.as_str()
        );
    }
    Ok(())
}
