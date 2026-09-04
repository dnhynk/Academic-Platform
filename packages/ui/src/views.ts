/**
 * The view registry and what opening a destination produces.
 *
 * The registry is written out route by route rather than derived from the route
 * manifest. A derived registry would make `every_destination_opens` vacuous:
 * every route would have a view because every route was a route. Written out,
 * the two enumerations are independent, and the test compares them in both
 * directions -- a route with no view fails, and a view with no route fails.
 *
 * What a view is here is a structure, not pixels. No Tauri runtime is linked
 * and no window opens; opening a destination builds a titled frame with a
 * breadcrumb, at least one section, and the right-hand evidence drawer. The
 * per-surface content of those sections is `P2-X2` through `P2-X7`; what this
 * task fixes is that every destination in the section 25.1 tree has a frame to
 * put content into, and that the frame carries the drawer.
 *
 * `P2-X7` has since filled the `Evidence & Settings` branch: its four routes
 * take their sections from `evidence-center.ts`, whose six identifiers are
 * compared against `academic_evidence_center::CenterSection`. `P2-X2` has
 * filled `Home / Today` the same way, from `home.ts`, whose eight identifiers
 * are compared against `academic_home::HomeGroup`. The rest of the tree is
 * still framed.
 */

import { breadcrumb, routeOf, type Destination } from "./destinations.js";
import { renderDrawer, type DrawerPanel, type DrawerState } from "./drawer.js";
import { entityFor, type EntityRef } from "./entities.js";
import { EVIDENCE_CENTER_SECTIONS, sectionsForRoute } from "./evidence-center.js";
import { csMapRegions } from "./cs-map.js";
import { homeSections } from "./home.js";
import { backlinksOf } from "./backlinks.js";
import { ROUTE_MANIFEST, type RouteDefinition } from "./routes.js";

/**
 * The shell affordances a view carries.
 *
 * Section 25.1 requires the command palette and the evidence drawer to be
 * present on every screen, so their presence is a property of the rendered view
 * rather than an assumption about the shell. `SUPPRESSED_CHROME` is empty and
 * is the seam a future full-screen or modal surface would have to declare
 * itself in; `palette_reaches_four_entity_types_from_every_route` and
 * `evidence_drawer_persists_across_views` both read this, so declaring one
 * would fail rather than quietly removing an affordance the specification
 * requires.
 */
export type ChromeAffordance = "commandPalette" | "evidenceDrawer" | "breadcrumb";

/** Every affordance the shell frame provides. */
export const SHELL_CHROME: readonly ChromeAffordance[] = [
  "commandPalette",
  "evidenceDrawer",
  "breadcrumb",
];

/** Affordances a route withholds. No route withholds any. */
export const SUPPRESSED_CHROME: ReadonlyMap<string, readonly ChromeAffordance[]> = new Map();

/** One labelled region of a view. */
export interface ViewSection {
  /** Stable identifier, unique inside one view. */
  readonly id: string;
  /** What the region is called. */
  readonly heading: string;
  /** The task that fills this region with product content. */
  readonly filledBy: string;
}

/** What opening a destination produces. */
export interface RenderedView {
  readonly destination: Destination;
  readonly title: string;
  readonly breadcrumb: readonly string[];
  readonly sections: readonly ViewSection[];
  readonly drawer: DrawerPanel;
  /** The shell affordances this view carries. */
  readonly chrome: readonly ChromeAffordance[];
  /** Backlinks into the bound entity, empty for an index form. */
  readonly backlinks: readonly EntityRef[];
}

/** Builds the sections of one route's view. */
export type ViewBuilder = (route: RouteDefinition, destination: Destination) => readonly ViewSection[];

function frame(id: string, heading: string, filledBy: string): ViewSection {
  return { id, heading, filledBy };
}

/**
 * A section list for a route whose product content a later task owns.
 *
 * The `filledBy` value names that task, so a reader of a rendered view can see
 * that the region is a frame rather than mistaking an empty frame for a
 * finished surface.
 */
function framed(heading: string, filledBy: string): ViewBuilder {
  return (route, destination) => {
    const sections = [frame(`${route.id}.primary`, heading, filledBy)];
    const children = ROUTE_MANIFEST.filter((candidate) => candidate.parentId === route.id);
    if (children.length > 0) {
      sections.push(frame(`${route.id}.children`, "Sections", "P2-X1"));
    }
    if (destination.entityId !== null) {
      sections.push(frame(`${route.id}.backlinks`, "Backlinks", "P2-X1"));
    }
    return sections;
  };
}

/**
 * The sections `P2-X7` supplies for one `Evidence & Settings` child.
 *
 * The heading and the identifier are `evidence-center.ts`'s, which
 * `the_shell_sections_are_the_crates_own` compares against
 * `academic_evidence_center::CenterSection`. A route with no section of its own
 * would render an empty frame, so this raises instead: `every_destination_opens`
 * is what observes it.
 */
function centerSections(routeId: string): ViewBuilder {
  return () => {
    const sections = sectionsForRoute(routeId);
    if (sections.length === 0) {
      throw new Error(`no evidence-centre section is assigned to ${routeId}`);
    }
    return sections.map((section) => frame(section.id, section.heading, "P2-X7"));
  };
}

/**
 * The eight sections `P2-X2` supplies for `Home / Today`.
 *
 * The heading and the identifier are `home.ts`'s, which
 * `the_home_sections_are_the_crates_own` compares against
 * `academic_home::HomeGroup`. Section 25.2 numbers its eight priorities, and
 * the order they are returned in is that numbering: nothing is inserted before
 * the first, which is the shell half of `no_gpa_or_streak_hero_component`.
 */
function todaySections(): ViewBuilder {
  return () => {
    const sections = homeSections();
    if (sections.length === 0) {
      throw new Error("no home section is assigned to the Home / Today route");
    }
    return sections.map((section) => frame(section.id, section.heading, "P2-X2"));
  };
}

/**
 * The five regions `P2-X5` supplies for `Concepts / CS Map`.
 *
 * The heading and the identifier are `cs-map.ts`'s, whose lens rail
 * `the_cs_map_lenses_are_the_crates_own` compares against
 * `academic_cs_map::MapLens` and whose region order
 * `the_cs_map_regions_are_the_specifications` compares against section 25.3's
 * own bullets.
 */
function csMapSections(): ViewBuilder {
  return () => {
    const regions = csMapRegions();
    if (regions.length === 0) {
      throw new Error("no region is assigned to the Concepts / CS Map route");
    }
    return regions.map((region) => frame(region.id, region.heading, "P2-X5"));
  };
}

/**
 * The `Evidence & Settings` index: all six sections, each pointing at the child
 * route that shows it.
 *
 * Section 25.13 calls this one screen, so the index enumerates the whole of it
 * rather than the three children that happen to have content. A section whose
 * route is missing from the manifest fails here.
 */
function centerIndex(): ViewBuilder {
  return (route) => {
    const children = ROUTE_MANIFEST.filter((candidate) => candidate.parentId === route.id);
    const known = new Set(children.map((child) => child.id));
    return EVIDENCE_CENTER_SECTIONS.map((section) => {
      if (!known.has(section.routeId)) {
        throw new Error(`${section.id} points at ${section.routeId}, which is not a child route`);
      }
      return frame(`${route.id}.${section.id}`, section.heading, "P2-X7");
    });
  };
}

/**
 * One view builder per route in the section 25.1 tree.
 *
 * This map is the second enumeration `every_destination_opens` compares against
 * the manifest. Adding a route without adding a builder fails; adding a builder
 * without adding a route fails.
 */
export const VIEW_BUILDERS: ReadonlyMap<string, ViewBuilder> = new Map<string, ViewBuilder>([
  ["home", todaySections()],
  ["academic", framed("Academic", "P2-X3")],
  ["academic.dashboard", framed("Academic dashboard", "P2-X3")],
  ["academic.semester-planner", framed("Semester planner", "P2-X3")],
  ["academic.courses", framed("Course catalog", "P2-X3")],
  ["academic.graduation-audit", framed("Graduation audit", "P2-X3")],
  ["learn", framed("Learn", "P2-X4")],
  ["learn.lectures", framed("Lectures", "P2-X4")],
  ["learn.concepts", csMapSections()],
  ["learn.questions", framed("Questions", "P2-X4")],
  ["build", framed("Build", "P2-X4")],
  ["build.projects", framed("Projects", "P2-X4")],
  ["build.repository-snapshots", framed("Repository snapshots", "P2-X4")],
  ["build.build-to-learn", framed("Build to learn", "P2-X4")],
  ["explore", framed("Explore", "P2-X5")],
  ["explore.career", framed("Career", "P2-Y4")],
  ["explore.critical-paths", framed("Critical paths", "P2-X5")],
  ["explore.blind-spots", framed("Blind spots", "P2-X5")],
  ["evidence", centerIndex()],
  ["evidence.source-claim-review", centerSections("evidence.source-claim-review")],
  ["evidence.permissions-consent", centerSections("evidence.permissions-consent")],
  ["evidence.privacy-providers", centerSections("evidence.privacy-providers")],
  // Not `P2-X7`. Section 25.13 names six sections and none of them is export,
  // backup or audit: those are section 32.10's, and the plan gives them to
  // `P2-P1` (export and vendor-free restore) and `P2-P2` (the deletion and
  // retention flow). `P2-X1` assigned this route to `P2-X7` before either was
  // written; `P2-X7` cannot fill it, and says so here rather than leaving a
  // promise nobody owns. The deletion *receipt* half that is `P2-X7`'s is on
  // `Privacy / Providers` above, where the transmission it belongs to is.
  ["evidence.export-backup-audit", framed("Export, backup and audit", "P2-P1")],
]);

/** The title of one destination's view. */
function titleOf(route: RouteDefinition, destination: Destination): string {
  if (destination.entityId === null) {
    return route.iaLabel;
  }
  if (route.entityKind !== null) {
    const entity = entityFor({ kind: route.entityKind, id: destination.entityId });
    if (entity !== undefined) {
      return `${route.iaLabel} — ${entity.title}`;
    }
  }
  return `${route.iaLabel} — ${destination.entityId}`;
}

/**
 * Opens a destination.
 *
 * Raises when the destination names a route with no registered view, which is
 * the failure `every_destination_opens` observes rather than a silent blank.
 */
export function openDestination(destination: Destination, drawer: DrawerState): RenderedView {
  const route = routeOf(destination);
  const builder = VIEW_BUILDERS.get(route.id);
  if (builder === undefined) {
    throw new Error(`no view is registered for route ${route.id}`);
  }
  const sections = builder(route, destination);
  if (sections.length === 0) {
    throw new Error(`the view for route ${route.id} renders no sections`);
  }
  const bound: EntityRef | null =
    route.entityKind !== null && destination.entityId !== null
      ? { kind: route.entityKind, id: destination.entityId }
      : null;
  return {
    destination,
    title: titleOf(route, destination),
    breadcrumb: breadcrumb(destination),
    sections,
    drawer: renderDrawer(drawer),
    chrome: SHELL_CHROME.filter(
      (affordance) => !(SUPPRESSED_CHROME.get(route.id) ?? []).includes(affordance),
    ),
    backlinks: bound === null ? [] : backlinksOf(bound),
  };
}
