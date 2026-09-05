/**
 * The content `P2-X1` left the four `Academic` routes framed for.
 *
 * `P2-X1` fixed the frame: every route in the section 25.1 tree has a titled
 * view with a breadcrumb, at least one section, and the evidence drawer, and
 * each section names the task that fills it. This file is `P2-X3` filling the
 * four it named `P2-X3` — `Dashboard`, `Semester Planner`,
 * `Course Catalog & Course Detail` and `Graduation Audit` — plus the `Academic`
 * index above them.
 *
 * # There is one model, and it is not here
 *
 * `academic-dashboard` owns what an average, an audit reading, a timeline
 * facet, a planner axis, a coverage tab and a percentage are. This file is the
 * shell's side: which sections each route shows and in what order, and nothing
 * else. It holds no figure, no identifier, no instant and no status — a section
 * here is a heading and a position, and `academic.test.ts` compares each list
 * against the crate's own enumeration read out of its source, so an arm renamed
 * in Rust fails here rather than drifting silently.
 *
 * # The orders are the specification's
 *
 * Section 25.4 lists six lines, section 25.5 lists six axes after
 * `다음을 즉시 재평가한다`, and section 25.6's fenced block names six headings
 * and four coverage tabs. The crate parses each of those out of the design
 * document; this file carries the same orders, written out rather than derived,
 * for the reason `P2-X1`'s view registry is written out rather than derived
 * from its route manifest — a derived list would agree with the crate because
 * it *was* the crate, and the comparison would assert nothing.
 *
 * # The graduation percentage is not a section
 *
 * Section 25.4 calls `졸업 72%` a 보조 시각화. It has no entry in
 * `DASHBOARD_SECTIONS`, which is the shell half of
 * `percentage_is_secondary_with_breakdown`: the six sections are the whole of
 * the screen's sequence, compared in both directions, so a seventh cannot be
 * added on this side either.
 *
 * # No window opens
 *
 * No Tauri runtime is linked. What this file adds is that opening
 * `/academic/dashboard` yields sections naming section 25.4's own content
 * rather than a promise that a later task will supply some. It is not evidence
 * that anything renders.
 */

/** One region of an `Academic` route, as the shell shows it. */
export interface AcademicSectionDefinition {
  /**
   * The crate's own arm, in `SCREAMING_SNAKE_CASE`.
   *
   * Compared against the crate's enumeration by `academic.test.ts`.
   */
  readonly id: string;
  /** The specification's own position for this region, counting from one. */
  readonly position: number;
  /** The heading the shell renders. */
  readonly heading: string;
}

/** Section 25.4's six lines, in its own order. */
export const DASHBOARD_SECTIONS: readonly AcademicSectionDefinition[] = [
  { id: "AVERAGES", position: 1, heading: "Grade averages and their proofs" },
  { id: "CREDITS_BY_CATEGORY", position: 2, heading: "Credits earned, by category" },
  { id: "APPLIED_PROFILE", position: 3, heading: "The standard being applied" },
  { id: "AUDIT_STATES", position: 4, heading: "Graduation audit" },
  { id: "ATTEMPT_TIMELINE", position: 5, heading: "Every attempt" },
  { id: "SOURCE_FRESHNESS", position: 6, heading: "Official sources and last sync" },
];

/** Section 25.5's six re-evaluated axes, in its own order. */
export const PLANNER_SECTIONS: readonly AcademicSectionDefinition[] = [
  {
    id: "CREDITS_CONFLICTS_AND_PREREQUISITES",
    position: 1,
    heading: "Credits, conflicts and prerequisites",
  },
  { id: "GRADUATION_RULE_CONTRIBUTION", position: 2, heading: "What it contributes to graduation" },
  { id: "CONCEPT_COMPETENCY_EXPOSURE", position: 3, heading: "What you would be exposed to" },
  { id: "PROJECT_AND_ROLE_RELEVANCE", position: 4, heading: "Relevance to your projects and roles" },
  { id: "WORKLOAD_RANGE_BASIS_AND_BIAS", position: 5, heading: "Workload, and where it comes from" },
  { id: "FOLLOW_ON_UNLOCK", position: 6, heading: "What it unlocks next" },
];

/** Section 25.6's six blocks, in its own order. */
export const COURSE_SECTIONS: readonly AcademicSectionDefinition[] = [
  { id: "OFFICIAL_IDENTITY", position: 1, heading: "Official identity" },
  { id: "OFFERINGS", position: 2, heading: "Offerings" },
  { id: "COVERAGE", position: 3, heading: "Coverage" },
  { id: "MY_RECORD", position: 4, heading: "My record" },
  { id: "CONNECTIONS", position: 5, heading: "Connections" },
  { id: "REVIEWS", position: 6, heading: "Reviews" },
];

/** Section 25.6's four non-overlapping coverage tabs, in its own order. */
export const COVERAGE_TABS: readonly AcademicSectionDefinition[] = [
  { id: "DESIGNED", position: 1, heading: "Designed" },
  { id: "TAUGHT", position: 2, heading: "Taught" },
  { id: "PRACTICED", position: 3, heading: "Practiced" },
  { id: "ASSESSED", position: 4, heading: "Assessed" },
];

/**
 * Section 25.4's four graduation-audit display words, in its own order.
 *
 * Four, not the five the audit engine publishes. `REMAINING` is the word both
 * `NEEDS` and `NOT_SATISFIED` are shown as, and the crate keeps the engine's
 * own status beside the word so the difference is never lost — see
 * `crates/dashboard/src/audit_state.rs` and
 * `docs/contracts/academic-dashboard.md`.
 */
export const AUDIT_STATES: readonly AcademicSectionDefinition[] = [
  { id: "SATISFIED", position: 1, heading: "Satisfied" },
  { id: "REMAINING", position: 2, heading: "Remaining" },
  { id: "UNKNOWN", position: 3, heading: "Unknown" },
  { id: "CONFLICT", position: 4, heading: "Conflict" },
];

/** The routes this file supplies content for, in the section 25.1 tree's order. */
export const ACADEMIC_ROUTE_IDS = [
  "academic",
  "academic.dashboard",
  "academic.semester-planner",
  "academic.courses",
  "academic.graduation-audit",
] as const;

/** The `Academic` index: one entry per child route, in the tree's order. */
export const ACADEMIC_INDEX: readonly AcademicSectionDefinition[] = [
  { id: "DASHBOARD", position: 1, heading: "Dashboard" },
  { id: "SEMESTER_PLANNER", position: 2, heading: "Semester Planner" },
  { id: "COURSE_CATALOG", position: 3, heading: "Course Catalog & Course Detail" },
  { id: "GRADUATION_AUDIT", position: 4, heading: "Graduation Audit" },
];

/**
 * The sections one `Academic` route shows, in order.
 *
 * Raises for a route this file does not answer for, which is the failure
 * `every_destination_opens` observes rather than a silent blank.
 */
export function academicSections(routeId: string): readonly AcademicSectionDefinition[] {
  switch (routeId) {
    case "academic":
      return ACADEMIC_INDEX;
    case "academic.dashboard":
      return DASHBOARD_SECTIONS;
    case "academic.semester-planner":
      return PLANNER_SECTIONS;
    case "academic.courses":
      return COURSE_SECTIONS;
    case "academic.graduation-audit":
      return AUDIT_STATES;
    default:
      throw new Error(`${routeId} is not an Academic route`);
  }
}
