# CS map atlas, encodings, lens composition and time travel

`P2-X5` fixes the multiscale atlas of section 26.1, the eight visual channels of
section 26.2, the composition rule of section 26.3, the five focus modes and the
search reveal of section 26.4, the timeline scrubber and split view of section
26.5, and the first screen and lens rail of section 25.3.

Everything below lives in `academic-cs-map`. It links `academic-domain`, `serde`
and `thiserror` and nothing else; it opens no file, opens no socket, reads no
clock, runs no build script and adds no migration.

## What this is not evidence for

**No Tauri runtime is linked and no window opens.** `P2-X1` recorded the
measurement that decided it — `tauri 2.11.5` resolves 388 new packages into the
default product closure, six of which the workspace's network policy forbids —
and this task does not revisit it. Nothing here draws a pixel or measures a
frame.

In particular, `five_thousand_node_fixture_meets_the_budget` **does not measure
time.** Wall-clock on a shared machine measures the machine, and a ceiling loose
enough to survive one would be a number nothing could violate. What it measures
is how much the atlas materialises and how much work the layout does, both of
which are deterministic functions of the fixture. Whether a real renderer draws
five thousand nodes at an acceptable frame rate is not answered here and is not
claimed.

The other three things that stay open are named at the end of this page.

## The first screen

Section 25.3: `초기 화면은 수천 node가 아니라 10–20개 Field cluster와 현재 선택한
goal neighborhood다.`

`Atlas::initial_view` returns the field clusters and the goal's one-hop
neighbourhood and nothing else, and refuses a graph outside the range with
`ClusterCountOutOfRange` rather than trimming to twenty.

`initial_view_is_ten_to_twenty_clusters` reads the `10–20` out of the design
document, compares the returned cluster set against the graph's own `FIELD` set
in both directions, and compares the materialised identity set against the
clusters plus the neighbourhood in both directions. A count on its own would pass
for a screen showing twenty of the wrong things.

## `YOU` is a reference point

Section 25.3: `가운데의 YOU는 좌표 node가 아니라 사용자 state overlay의 기준점이다.`

`YouAnchor` has **no** `EntityId`. It carries the identities it is reckoned from
and the midpoint of their bounding box. It cannot be handed to `MapNode::declare`
and therefore cannot reach `MapGraph::declare`, which
`tests/compile_fail/a_you_anchor_is_not_a_node.rs` says with a committed
diagnostic.

The absence claim is checked by looking everywhere.
`you_is_not_an_ontology_node` sweeps twenty-one surfaces — the graph, the initial
view, all four zoom levels, all five focus subgraphs, seven scrubber projections,
both panes of a split view and a search reveal — and requires every one of them
to be non-empty before requiring that none of them holds a node under the
anchor's identity or carrying its label. The one scrubber reading that is
deliberately before every event is counted separately rather than swept, because
sweeping an empty set proves nothing.

## Four semantic zoom levels

| Zoom | Types shown | Horizon from the goal |
|---|---|---|
| `Z0 Ecosystem` | `FIELD` | none |
| `Z1 Domain` | `FIELD`, `CONCEPT` | none |
| `Z2 Concept` | `CONCEPT`, `CONCEPT_SENSE` | 2 hops |
| `Z3 Evidence` | `CONCEPT`, `CLAIM`, `EVIDENCE_ITEM`, `LECTURE`, `CODE_COMPONENT` | 1 hop |

The two coarse levels are scoped by **type**, because the section 26.1 table's
`감추는 것` column hides `개별 concept` at `Z0` and `세부 operation` at `Z1`. The
two fine levels are scoped by **distance**, because that column hides
`먼 주변 node` at `Z2` and the `전역 graph` at `Z3`. The four type sets are nested
in neither direction: `Z2` admits `CONCEPT_SENSE`, which `Z1` does not, and drops
`FIELD`, which `Z1` has.

`zoom_changes_semantic_level_not_only_scale` requires the four type sets to be
pairwise different, the four node sets to be pairwise different, and every node
present at two levels to sit at the **identical** coordinate in both — which is
the half a scale factor could not pass. The four row labels are parsed out of the
section 26.1 table and compared against `SEMANTIC_ZOOMS` position by position.

## Layout stability, as a golden coordinate test

The layout is a lattice, not a relaxation:

```text
cell   = digest(cluster identity) mod (GRID * GRID), probed forward on collision
column = cell mod GRID,  row = cell / GRID
pitch  = PITCH_BASE + growth_band(node count) * PITCH_STEP
anchor = ((column - GRID/2) * pitch, (row - GRID/2) * pitch)
member = anchor + offset(member identity)
```

A cluster's own `FIELD` node sits exactly on the anchor with no scatter, and it
is the landmark. That is a decision about stability rather than a shortcut: a
landmark chosen from the members — highest degree, lowest identity, most recently
used — is replaced by a different node whenever the membership changes, and a
landmark that was replaced has not "stayed within tolerance".

`lay_out` takes a graph and nothing else. No lens, overlay, focus mode, zoom
level or instant is a parameter, which is section 19's
`layout이 lens마다 바뀌면 사용자의 spatial memory가 깨지므로` stated as a
signature. `layout_is_the_same_under_every_lens_focus_and_zoom` exercises 829
views and compares every placement for exact equality afterwards.

`LAYOUT_TOLERANCE_MILLI` is `(GRID / 2) * PITCH_STEP * MAX_GROWTH_BAND` — the
furthest a landmark of a fixed cluster set can move between **any** two graph
sizes, because the pitch has only `MAX_GROWTH_BAND` steps to range over.
`landmark_coordinates_stay_within_tolerance` measures three size pairs across
four growth bands, requires each drift to be non-zero and inside the bound, and
requires the widest pair to sit **exactly on** the tolerance, so the bound is
known to be attained rather than merely generous. A layout whose pitch followed
the raw node count would have no bound at all.

The coordinates themselves are pinned in `testdata/cs-map/landmarks.expected`, so
a change to the spreading function or to any of the four constants is a reviewed
diff rather than a silent redraw.

**The boundary of the stability claim.** Anchors are stable under node growth and
under every view setting. They are **not** stable under a change to the *cluster*
set: the collision probe walks forward in cluster-identity order, so a field
added or removed can move an anchor. That is recorded rather than hidden — a
field appearing or disappearing is an ontology change, which section 26.5 already
draws with its own transition — and the alternative, letting two clusters share a
cell, would put two fields in one place. `landmark_coordinates_stay_within_tolerance`
requires all sixteen anchors of the fixture to be pairwise distinct.

## The eight channels

| Channel | Value type | Reads |
|---|---|---|
| `node fill` | `MasteryFill` | section 13.1's `MasteryLevel` |
| `outer ring` | `FreshnessRing` | section 13.3's `FreshnessBand` |
| `border pattern` | `BorderPattern` | `EpistemicStatus` and `ConfidencePermille` |
| `glyph` | `GlyphMark` | section 19's five symbols |
| `edge stroke` | `EdgeStroke` | section 7.2's predicate and the claim's status |
| `opacity` | `LensRelevance` | the base lens, and nothing else |
| `halo` | `HaloState` | whether the node is on the active critical path |
| `timestamp badge` | `AsOfBadge` | `P2-C6`'s `TimeCoordinates`, **both** axes |

`eight_encodings_are_independently_variable` draws a baseline frame and then, for
each of the eight, one frame with exactly that channel's input moved, and
compares the whole eight-value array: the moved channel must differ and the other
seven must be identical. A channel computed from another's input moves two.

A frame holds one node and one edge, because section 26.2's list spans both
subjects; a frame whose two halves are about different subjects is refused, so
the eight values are always eight values about one thing.

### Opacity is not mastery

Section 26.2's own words: `opacity: 현재 lens relevance이지 mastery가 아님.`

Held four ways, none of them a rule:

1. **Two types.** `LensRelevance` and `MasteryFill` share no trait, no ordering
   and no arithmetic.
2. **No conversion, in either direction.**
   `every_impl_header_in_this_crate_is_in_the_inventory` pins the whole set of
   `impl` headers the package declares and requires none of them to mention
   `From`, `Into`, `Deref`, `AsRef`, `Borrow`, `TryFrom`, `FromStr` or
   `FromIterator` for **any** type pair. This is the bypass class `P2-Y3`
   measured: a trait implementation declares no `pub fn`, so a public-function
   sweep cannot see one. Two compile-fail cases carry the diagnostics.
3. **Signatures.** `the_producers_of_a_relevance_are_pinned` compares the whole
   set of public functions returning a `LensRelevance` by value against two and
   pins both; neither names a mastery type.
   `no_signature_names_both_a_relevance_and_a_mastery` compares the whole set
   naming both against the empty set, shows each half separately non-empty, and
   shows the predicate biting on a fragment that does map one to the other.
4. **Behaviour.** `opacity_tracks_relevance_not_mastery` draws all six mastery
   levels with everything else fixed and requires the opacity to be one value six
   times **and the fill to be six different ones**, so the comparison is not
   passing because nothing moved.

### Dashed is inferred, solid is confirmed

`EdgeStroke::of` is a total `match` over section 30.2's nine statuses with
exactly two classes. `dash_solid_maps_to_claim_status` runs all nine against all
twenty of section 7.2's predicates and checks that the stroke keeps its type as
well as its dash.

`Disputed` and `Superseded` draw **dashed**. The specification's bullet names
`inferred/predicted` and `confirmed` and does not place those two, so this crate
places them and says so: a disputed relation is not a confirmed one, and drawing
it solid would be the most direct way for this surface to present a contested
claim as settled.

## Lens composition

Section 25.3 names ten lenses and section 26.2 names eight channels. The two
lists were written independently, and they overlap on **four** lenses:
`Freshness` appears in the outer-ring bullet, `Project` and `Question` in the
glyph bullet, and `Critical Path` in the halo bullet. The other six appear in no
bullet at all.

`MapLens::claimed_channel` returns `Some` for exactly those four.
`lens_channel_claims_are_named_in_the_encoding_bullets` derives both halves by
searching each lens's own spec name in each bullet of the design document, in
both directions, so nothing here decides that `Coursework` has no channel — the
document does, and the test fails if it changes its mind.

**Section 26.3's own example collides.** Its
`Base: Knowledge State / Overlay 1: Project A / Overlay 2: Open Questions` puts
`Project` and `Question` on the glyph, so `layer_collision_warns_and_pins_legend`
reads that block out of the document and runs it.

`LensComposition::overlay` takes `self` **by value**, so a refused third overlay
consumes the composition and leaves nothing to retry with — `P2-X1`'s
`Optimistic::confirm` shape, for the same reason. `third_overlay_is_rejected`
enumerates the whole `base × overlay × overlay × overlay` product — ten thousand
orderings, thirty thousand insertions — and derives the expected verdict of each
step in the test rather than reading it off the composition.

A lens already in the composition is refused too, with
`LensAlreadyComposed`. **That one is this crate's decision and not section
26.3's**: the specification says nothing about a repeat, and a composition that
spent one of its two overlay slots redrawing its base would report a collision
with itself.

The legend always lists all eight channels in section 26.2's order — hiding one
would defeat the redundancy section 26.2 requires — and names, per channel, which
composed lenses claim it. `Legend::is_pinned` is true exactly when the
composition collides. **The viewer that honours the pin is `P2-X6`'s**; what is
fixed here is which compositions pin it and that the row order never moves.
`layer_collision_warns_and_pins_legend` enumerates all 820 admissible
compositions and requires both classes to be non-empty.

## Five focus modes

| Mode | Nodes | Edges kept |
|---|---|---|
| goal | the goal, everything reachable from it along `REQUIRES`, and the one-hop `REQUIRES` predecessors | `REQUIRES` between two selected nodes |
| local neighbourhood | everything within `hops` of the centre along the caller's edge-type filter | a filtered predicate between two selected nodes |
| evidence | the node and everything its state assertion is `EVIDENCED_BY` | `EVIDENCED_BY` between two selected nodes |
| uncertainty | every node whose reading is disputed, AI-inferred, or below the confidence threshold | every edge between two selected nodes |
| course | the revision's `DESIGNED_TO_TEACH` targets and the offering lectures' `TAUGHT_IN` sources | `DESIGNED_TO_TEACH` or `TAUGHT_IN` between two selected nodes |

`five_focus_modes_return_exact_subgraphs` compares each node set **and each edge
set** against a set typed out by hand, in both directions.

Three decisions are worth stating because a reader could reasonably expect
otherwise:

* **`BUILDS_ON` is not a prerequisite.** Section 7.2 separates the two:
  `REQUIRES` is a hard or near-hard dependency and `BUILDS_ON`
  `반드시 선행해야 하는 것은 아닐 수 있음`. The fixture carries a `BUILDS_ON` edge
  between two nodes the goal focus already holds, so a focus that walked it would
  return the same node set and a bigger edge set — and only the edge comparison
  catches that.
* **A node with no reading is not uncertain.** It has no state to be uncertain
  about, and admitting it would make the uncertainty focus a list of everything
  the graph has not been told about.
* **An offering's lectures arrive as an argument.** An offering's containment of
  its lectures is a section 9 aggregate and not one of section 7.2's twenty
  edges, so the course focus takes the lecture set rather than inventing an edge
  for it.

`HopCount::new` refuses zero as well as four. Section 26.4 writes `1–3 hop`, and
a zero-hop neighbourhood is the node by itself.

## Search reveals in three stages

Section 26.4: `graph search result는 node를 화면 밖에서 순간이동시키지 않고
cluster → path → node 순으로 안내한다.`

`SearchReveal` holds a cluster, a path and a node, all three by value, with
private fields and one producer. `reveal` refuses a match it cannot walk to with
`NoPathToTarget` rather than producing a node from nowhere, refuses a blank query,
and refuses an ambiguous one rather than choosing.

`search_reveals_in_three_stages` checks that the route is real — consecutive
steps are edges of the graph, it starts where the viewer stood and ends at the
match — and `every_query_producer_returns_a_reveal` compares the whole set of
public functions taking a query against one, so a convenience wrapper returning a
bare identity would be a new entry rather than a quiet addition.

## Time travel

A scrubber position is a `TimeCoordinates`, which is **both** of `P2-C6`'s axes.
An event counts when its acceptance sequence is at or below the reading's **and**
its valid instant is at or below the reading's; neither stands in for the other,
and `scrubber_matches_the_temporal_oracle` includes readings where the two axes
disagree so that a reader collapsing them fails.

Events are sorted on `(known_at, valid_at, subject, appearance)` and `Appears`
sorts before `Disappears`, so a node that appears and disappears at identical
coordinates ends invisible. No row of the committed fixture ties, so the
tie-break is recorded here rather than exercised.

`Timeline::compare` refuses two identical readings with
`PanesAreTheSameReading`: a split view of one reading has an empty delta list,
which satisfies every comparison drawn over it. Each pane carries its own
coordinates, and a delta whose reason the projection did not record is
`TransitionNotRecorded` rather than a default — a delta with an invented cause
would be this surface telling a reader why something changed when it does not
know.

### The oracle is a second transcription in another language

`tools/cs-map-scrubber-oracle.mjs` renders `testdata/cs-map/scrubber.expected`.
Four things in it are independent of the Rust implementation: the event table is
typed from this page rather than read from the fixture module; the identities are
derived with `node:crypto` rather than through `academic_domain`; the algorithm
groups admitted events **by subject** and asks what the last one did, rather than
folding a running set; and the admission rule is restated from this page rather
than copied. `P2-U3` set this precedent and `P2-U5` recorded why: a value checked
against the engine that produced it proves only that the engine is deterministic.

`node tools/cs-map-scrubber-oracle.mjs --check` fails if the committed file
differs from a fresh render, and `tools/cs-map-scrubber-oracle.test.mjs` runs
that check plus the two negative controls.

### The third transition kind lives in the view

Section 26.5 names three: `ontology change`, `evidence change`,
`user scope change`. `P2-C6`'s `ChangeOrigin` has four, and `user scope change`
is deliberately not one of them; [that contract](bitemporal-time-travel.md)
records the reason:

> changing which scope is displayed changes what a viewer is shown, not what the
> record says. It belongs to the view that owns the scope filter, and putting it
> here would let a display setting be recorded as a change in canonical history.

This crate is that view. `MapTransition` has five arms: the four `ChangeOrigin`s,
each of which `change_origin` returns, and `UserScopeChange`, which returns
`None`. `change_origin_transitions_are_distinguishable` compares the `Some` image
against `CHANGE_ORIGINS` in both directions, requires `UserScopeChange` to be the
only `None`, requires all five to be pairwise different on all four non-colour
attributes — wire name, badge, pattern, screen-reader text — and requires every
arm to actually reach a viewer through the fixture's own timeline.

`no_function_returns_a_bare_change_origin` compares the whole set of public
signatures returning a bare `ChangeOrigin` against the empty set, which is what
would catch a total conversion added later; a compile-fail case carries the
diagnostic.

## The budget

| Ceiling | Value | What it bounds |
|---|---|---|
| `initial_view_nodes` | 64 | section 25.3's first screen |
| `goal_near_nodes` | 256 | `Z2 Concept` |
| `evidence_nodes` | 64 | `Z3 Evidence` |
| `layout_work_units_per_node` | 3 | the layout, **per node**, so it stays a linearity claim at any size |
| `search_path_hops` | 12 | one navigation |

`five_thousand_node_fixture_meets_the_budget` counts each measure off values the
crate produced, then moves each one past its ceiling on its own and requires a
refusal naming that measure, so no ceiling is a number nothing could exceed. The
measured numbers for the shipped fixture are in the task report rather than here,
because they are a measurement and this page is a contract.

`Z1 Domain` has no ceiling, and that is deliberate: the section 26.1 table scopes
it by type rather than by distance, so it legitimately shows every concept. The
budget's teeth are the first screen and the two fine levels.

## What stays open

* **The Tauri runtime binding**, with the 388-package admission it implies.
  Unchanged by this task.
* **Accessibility conformance.** `P2-X6` owns contrast, forced colours, the
  colour-blind palette, the `prefers-reduced-motion` diff list and keyboard
  reachability. What this crate provides is the non-colour half of every encoding
  as a value — a symbol, a label, a pattern, a screen-reader name — so that there
  is something to audit.
* **Anchor stability across a cluster-set change**, described above.
* **§38.** This task leaves no gate open and closes none.
