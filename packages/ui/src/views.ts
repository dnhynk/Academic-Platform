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
 */

import { breadcrumb, routeOf, type Destination } from "./destinations.js";
import { renderDrawer, type DrawerPanel, type DrawerState } from "./drawer.js";
import { entityFor, type EntityRef } from "./entities.js";
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
 * One view builder per route in the section 25.1 tree.
 *
 * This map is the second enumeration `every_destination_opens` compares against
 * the manifest. Adding a route without adding a builder fails; adding a builder
 * without adding a route fails.
 */
export const VIEW_BUILDERS: ReadonlyMap<string, ViewBuilder> = new Map<string, ViewBuilder>([
  ["home", framed("Today", "P2-X2")],
  ["academic", framed("Academic", "P2-X3")],
  ["academic.dashboard", framed("Academic dashboard", "P2-X3")],
  ["academic.semester-planner", framed("Semester planner", "P2-X3")],
  ["academic.courses", framed("Course catalog", "P2-X3")],
  ["academic.graduation-audit", framed("Graduation audit", "P2-X3")],
  ["learn", framed("Learn", "P2-X4")],
  ["learn.lectures", framed("Lectures", "P2-X4")],
  ["learn.concepts", framed("Concepts and CS map", "P2-X5")],
  ["learn.questions", framed("Questions", "P2-X4")],
  ["build", framed("Build", "P2-X4")],
  ["build.projects", framed("Projects", "P2-X4")],
  ["build.repository-snapshots", framed("Repository snapshots", "P2-X4")],
  ["build.build-to-learn", framed("Build to learn", "P2-X4")],
  ["explore", framed("Explore", "P2-X5")],
  ["explore.career", framed("Career", "P2-Y4")],
  ["explore.critical-paths", framed("Critical paths", "P2-X5")],
  ["explore.blind-spots", framed("Blind spots", "P2-X5")],
  ["evidence", framed("Evidence and settings", "P2-X7")],
  ["evidence.source-claim-review", framed("Source and claim review", "P2-X7")],
  ["evidence.permissions-consent", framed("Permissions and consent", "P2-X7")],
  ["evidence.privacy-providers", framed("Privacy and providers", "P2-X7")],
  ["evidence.export-backup-audit", framed("Export, backup and audit", "P2-X7")],
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
