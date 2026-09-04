/**
 * The content `P2-X1` left the `Concepts / CS Map` route framed for.
 *
 * `P2-X1` fixed the frame: every route in the section 25.1 tree has a titled
 * view with a breadcrumb, at least one section, and the evidence drawer, and
 * each section names the task that fills it. This file is `P2-X5` filling the
 * one it named `Concepts and CS map`.
 *
 * # There is one model, and it is not here
 *
 * `academic-cs-map` owns what a zoom level is, what a lens claims, what the
 * eight encodings are and what a scrubber position means. This file is the
 * shell's side: which regions section 25.3 puts on the screen, in what order,
 * and which lenses the rail offers. It holds no coordinate, no node, no
 * relevance and no instant.
 *
 * `cs-map.test.ts` compares [`CS_MAP_LENSES`] against `academic_cs_map::MapLens`
 * read out of that crate's source, in both directions and in the crate's own
 * order, so a lens renamed in Rust fails here rather than drifting silently.
 * The five regions are compared against section 25.3's own five bullets the
 * same way.
 *
 * # No window opens
 *
 * No Tauri runtime is linked. What this file adds is that opening
 * `/learn/concepts` yields regions naming section 25.3's own content rather than
 * a promise that a later task will supply some. It is not evidence that anything
 * renders, and in particular it is not evidence that a graph is drawn.
 */

/** One of section 25.3's five screen regions, as the shell shows it. */
export interface CsMapRegionDefinition {
  /** The identifier the view registry frames this region under. */
  readonly id: string;
  /** Section 25.3's own position for this region, counting from one. */
  readonly position: number;
  /** The heading the shell renders. */
  readonly heading: string;
  /**
   * The words section 25.3's bullet opens with.
   *
   * Compared against the design document by `the_cs_map_regions_are_the_specifications`,
   * so a region invented here has no bullet to match and fails.
   */
  readonly specBulletHead: string;
}

/**
 * The five regions, in section 25.3's own bullet order.
 *
 * Written out rather than derived from anything, for the reason `P2-X1`'s view
 * registry is written out rather than derived from its route manifest: a derived
 * list would agree with the document because it *was* the document, and the
 * comparison would assert nothing.
 */
export const CS_MAP_REGIONS: readonly CsMapRegionDefinition[] = [
  { id: "LENS_RAIL", position: 1, heading: "Lenses", specBulletHead: "상단 lens" },
  { id: "FILTERS", position: 2, heading: "Filters", specBulletHead: "좌측" },
  { id: "ATLAS", position: 3, heading: "The map", specBulletHead: "중앙" },
  { id: "SELECTED_NODE", position: 4, heading: "Selected node", specBulletHead: "우측" },
  { id: "TIMELINE", position: 5, heading: "Timeline", specBulletHead: "하단 timeline" },
];

/**
 * The ten lenses the rail offers, in section 25.3's own order.
 *
 * The identifiers are `academic_cs_map::MapLens`'s wire discriminants, which
 * `the_cs_map_lenses_are_the_crates_own` compares against that crate's own
 * `as_str` arms. The shell holds the name and nothing else: what a lens *means*,
 * and which of section 26.2's channels it claims, is the crate's.
 */
export const CS_MAP_LENSES: readonly string[] = [
  "KNOWLEDGE",
  "FRESHNESS",
  "COURSEWORK",
  "CURRENT_SEMESTER",
  "PROJECT",
  "CAREER",
  "QUESTION",
  "CRITICAL_PATH",
  "BLIND_SPOT",
  "GRADUATION",
];

/** The route this file supplies content for. */
export const CS_MAP_ROUTE_ID = "learn.concepts";

/**
 * The five regions of the CS map screen, in order.
 *
 * There is no argument, so there is nothing a caller could pass that would put
 * a region before the first or add one after the last.
 */
export function csMapRegions(): readonly CsMapRegionDefinition[] {
  return CS_MAP_REGIONS;
}
