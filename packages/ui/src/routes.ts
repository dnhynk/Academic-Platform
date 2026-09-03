/**
 * The desktop route manifest.
 *
 * This table is the contract. `route-manifest.test.ts` reads section 25.1 of
 * `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares the two in
 * both directions, so a line that appears here and not in the specification
 * fails, and a line that appears in the specification and not here fails. No
 * count is asserted anywhere; both sides are enumerated and the sets compared.
 *
 * One tree line is one route. Two of the specification's lines name a pair --
 * `Course Catalog & Course Detail` and `Concepts / CS Map` -- and neither is
 * split into two nodes here, because splitting on punctuation would make the
 * comparison depend on how a label is spelled rather than on what the tree
 * says. Where a route also addresses one entity, it carries a `detail`
 * parameter name instead, and `destinations.ts` opens the index form and the
 * detail form as two separate destinations.
 */

/** The four entity types section 25.1 requires from every screen. */
export type EntityKind = "Course" | "Concept" | "Project" | "Question";

/** Every entity kind, in the order section 25.1 names them. */
export const ENTITY_KINDS: readonly EntityKind[] = [
  "Course",
  "Concept",
  "Project",
  "Question",
];

/** One node of the section 25.1 tree. */
export interface RouteDefinition {
  /** Stable identifier. Never derived from the label at runtime. */
  readonly id: string;
  /** The label this route answers for in the section 25.1 tree. */
  readonly iaLabel: string;
  /** Parent route id, or `null` for the root. */
  readonly parentId: string | null;
  /** URL path of the index form of this destination. */
  readonly path: string;
  /**
   * Parameter name of the detail form, when this route addresses one entity.
   *
   * A route with a detail parameter has two destinations: `path` and
   * `path/:detailParam`.
   */
  readonly detailParam: string | null;
  /** The entity kind whose detail form this route opens, when it has one. */
  readonly entityKind: EntityKind | null;
}

/**
 * The route manifest, in section 25.1's own order.
 *
 * `P2-X1` owns this table; execution-plan section 4 names it a serialized
 * conflict hotspot.
 */
export const ROUTE_MANIFEST: readonly RouteDefinition[] = [
  {
    id: "home",
    iaLabel: "Home / Today",
    parentId: null,
    path: "/",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "academic",
    iaLabel: "Academic",
    parentId: "home",
    path: "/academic",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "academic.dashboard",
    iaLabel: "Dashboard",
    parentId: "academic",
    path: "/academic/dashboard",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "academic.semester-planner",
    iaLabel: "Semester Planner",
    parentId: "academic",
    path: "/academic/semester-planner",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "academic.courses",
    iaLabel: "Course Catalog & Course Detail",
    parentId: "academic",
    path: "/academic/courses",
    detailParam: "courseId",
    entityKind: "Course",
  },
  {
    id: "academic.graduation-audit",
    iaLabel: "Graduation Audit",
    parentId: "academic",
    path: "/academic/graduation-audit",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "learn",
    iaLabel: "Learn",
    parentId: "home",
    path: "/learn",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "learn.lectures",
    iaLabel: "Lectures",
    parentId: "learn",
    path: "/learn/lectures",
    detailParam: "lectureId",
    entityKind: null,
  },
  {
    id: "learn.concepts",
    iaLabel: "Concepts / CS Map",
    parentId: "learn",
    path: "/learn/concepts",
    detailParam: "conceptId",
    entityKind: "Concept",
  },
  {
    id: "learn.questions",
    iaLabel: "Questions",
    parentId: "learn",
    path: "/learn/questions",
    detailParam: "questionId",
    entityKind: "Question",
  },
  {
    id: "build",
    iaLabel: "Build",
    parentId: "home",
    path: "/build",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "build.projects",
    iaLabel: "Projects",
    parentId: "build",
    path: "/build/projects",
    detailParam: "projectId",
    entityKind: "Project",
  },
  {
    id: "build.repository-snapshots",
    iaLabel: "Repository Snapshots",
    parentId: "build",
    path: "/build/repository-snapshots",
    detailParam: "snapshotId",
    entityKind: null,
  },
  {
    id: "build.build-to-learn",
    iaLabel: "Build → Learn",
    parentId: "build",
    path: "/build/build-to-learn",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "explore",
    iaLabel: "Explore",
    parentId: "home",
    path: "/explore",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "explore.career",
    iaLabel: "Career",
    parentId: "explore",
    path: "/explore/career",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "explore.critical-paths",
    iaLabel: "Critical Paths",
    parentId: "explore",
    path: "/explore/critical-paths",
    detailParam: "pathId",
    entityKind: null,
  },
  {
    id: "explore.blind-spots",
    iaLabel: "Blind Spots",
    parentId: "explore",
    path: "/explore/blind-spots",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "evidence",
    iaLabel: "Evidence & Settings",
    parentId: "home",
    path: "/evidence",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "evidence.source-claim-review",
    iaLabel: "Source / Claim Review",
    parentId: "evidence",
    path: "/evidence/source-claim-review",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "evidence.permissions-consent",
    iaLabel: "Permissions & Consent",
    parentId: "evidence",
    path: "/evidence/permissions-consent",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "evidence.privacy-providers",
    iaLabel: "Privacy / Providers",
    parentId: "evidence",
    path: "/evidence/privacy-providers",
    detailParam: null,
    entityKind: null,
  },
  {
    id: "evidence.export-backup-audit",
    iaLabel: "Export / Backup / Audit",
    parentId: "evidence",
    path: "/evidence/export-backup-audit",
    detailParam: null,
    entityKind: null,
  },
];

/** The manifest keyed by route id. */
export const ROUTES_BY_ID: ReadonlyMap<string, RouteDefinition> = new Map(
  ROUTE_MANIFEST.map((route) => [route.id, route]),
);

/** The route that opens one entity kind's detail form. */
export function routeForEntityKind(kind: EntityKind): RouteDefinition {
  const route = ROUTE_MANIFEST.find((candidate) => candidate.entityKind === kind);
  if (route === undefined) {
    throw new Error(`no route in the manifest opens ${kind} detail`);
  }
  return route;
}
