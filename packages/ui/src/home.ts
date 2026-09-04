/**
 * The content `P2-X1` left the `Home / Today` route framed for.
 *
 * `P2-X1` fixed the frame: every route in the section 25.1 tree has a titled
 * view with a breadcrumb, at least one section, and the evidence drawer, and
 * each section names the task that fills it. This file is `P2-X2` filling the
 * one it named `Today`.
 *
 * # There is one model, and it is not here
 *
 * `academic-home` owns what a home card is, what an upcoming use is, and what
 * the four permission words mean. This file is the shell's side: which sections
 * the `/` route shows and in what order, and nothing else. It holds no card, no
 * identifier, no instant and no permission — a section here is a heading and a
 * position, and `home.test.ts` compares the eight against the crate's own
 * `HomeGroup` enumeration read out of its source, so a group renamed in Rust
 * fails here rather than drifting silently.
 *
 * # The order is the specification's
 *
 * Section 25.2 numbers its eight priorities. The crate parses that numbering
 * out of the design document and this file carries the same order; the shell
 * half of `no_gpa_or_streak_hero_component` is that these eight are the whole
 * of the screen, compared in both directions, so a ninth section cannot be
 * added on this side either.
 *
 * # No window opens
 *
 * No Tauri runtime is linked. What this file adds is that opening `/` yields
 * sections naming section 25.2's own content rather than a promise that a later
 * task will supply some. It is not evidence that anything renders.
 */

/** One of section 25.2's eight priority groups, as the shell shows it. */
export interface HomeGroupDefinition {
  /**
   * The crate's `HomeGroup` arm, in `SCREAMING_SNAKE_CASE`.
   *
   * Compared against `academic_home::HomeGroup::id` by
   * `the_home_sections_are_the_crates_own`.
   */
  readonly id: string;
  /** Section 25.2's own number for this group, counting from one. */
  readonly position: number;
  /** The heading the shell renders. */
  readonly heading: string;
}

/**
 * The eight groups, in section 25.2's own numbered order.
 *
 * Written out rather than derived from anything, for the reason `P2-X1`'s view
 * registry is written out rather than derived from its route manifest: a
 * derived list would agree with the crate because it *was* the crate, and the
 * comparison would assert nothing.
 */
export const HOME_GROUPS: readonly HomeGroupDefinition[] = [
  { id: "TODAYS_SCHEDULE", position: 1, heading: "Today's schedule" },
  { id: "MINIMUM_PREREQUISITE", position: 2, heading: "Before your next class" },
  { id: "RECORDING_PERMISSION_STATUS", position: 3, heading: "Recording permission" },
  { id: "OPEN_QUESTION_AND_MARK_MOMENT", position: 4, heading: "Your open questions and marks" },
  { id: "PROJECT_BLOCKING_KNOWLEDGE_NEED", position: 5, heading: "What is blocking your project" },
  {
    id: "OFFICIAL_CONDITION_AND_STALE_WARNING",
    position: 6,
    heading: "Official conditions and source warnings",
  },
  { id: "CRITICAL_PATH_NEXT_STEP", position: 7, heading: "Your next step" },
  { id: "CONCEPT_FRESHNESS_ALERT", position: 8, heading: "Concepts you are about to need" },
];

/** The route this file supplies content for. */
export const HOME_ROUTE_ID = "home";

/**
 * The eight sections of the home screen, in order.
 *
 * There is no argument, so there is nothing a caller could pass that would put
 * a section before the first or add one after the last.
 */
export function homeSections(): readonly HomeGroupDefinition[] {
  return HOME_GROUPS;
}
