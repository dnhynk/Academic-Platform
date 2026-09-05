/**
 * `the_academic_sections_are_the_crates_own`, and the shell half of
 * `percentage_is_secondary_with_breakdown` and `coverage_tabs_are_non_overlapping`.
 *
 * The shell and the crate hold the same five enumerations, and neither is
 * derived from the other: `academic_dashboard`'s arms are written in Rust and
 * the lists in `academic.ts` are written here, so an arm renamed on one side
 * fails against the other rather than drifting.
 *
 * The comparison reads the crate's source text, because there is no runtime
 * across which the two could be compared: this package links no Rust, and
 * `P2-X1` linked no Tauri runtime either. What is checked is therefore a
 * source-level agreement, and `docs/contracts/policy-source-scans.md` carries
 * this file's row for that reason.
 *
 * Every comparison is a whole set in both directions **and** a positional
 * equality. There is no list of forbidden names anywhere in the file: a seventh
 * dashboard section, a fifth coverage tab or a fifth audit state fails as an
 * extra key whatever it is called.
 */

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  ACADEMIC_INDEX,
  ACADEMIC_ROUTE_IDS,
  AUDIT_STATES,
  COURSE_SECTIONS,
  COVERAGE_TABS,
  DASHBOARD_SECTIONS,
  PLANNER_SECTIONS,
  academicSections,
  type AcademicSectionDefinition,
} from "./academic.js";
import { allDestinations, routeOf } from "./destinations.js";
import { EMPTY_DRAWER } from "./drawer.js";
import { ROUTE_MANIFEST } from "./routes.js";
import { openDestination } from "./views.js";

const crateUrl = (file: string): URL =>
  new URL(`../../../crates/dashboard/src/${file}`, import.meta.url);

/**
 * The arms of one `match` in a crate module, as `Self::Arm => "value"` pairs.
 *
 * Read out of the named function's body rather than out of the enum, so an arm
 * renamed with a stale value fails on the value.
 */
function armsOf(source: string, signature: string): ReadonlyMap<string, string> {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `the crate has no ${signature}`);
  const end = source.indexOf("\n    }\n", start);
  assert.notEqual(end, -1, `${signature} has no closing brace`);
  const arms = new Map<string, string>();
  for (const match of source.slice(start, end).matchAll(/Self::([A-Za-z0-9_]+) => "([^"]*)",/gu)) {
    arms.set(match[1] ?? "", match[2] ?? "");
  }
  return arms;
}

/** The arms of a `match` whose values are numbers rather than strings. */
function numericArmsOf(source: string, signature: string): ReadonlyMap<string, number> {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `the crate has no ${signature}`);
  const end = source.indexOf("\n    }\n", start);
  assert.notEqual(end, -1, `${signature} has no closing brace`);
  const arms = new Map<string, number>();
  for (const match of source.slice(start, end).matchAll(/Self::([A-Za-z0-9_]+) => (\d+),/gu)) {
    arms.set(match[1] ?? "", Number(match[2]));
  }
  return arms;
}

/** The order one `ALL` constant lists its arms in. */
function allOrder(source: string, opener: string): readonly string[] {
  const start = source.indexOf(opener);
  assert.notEqual(start, -1, `the crate has no ${opener}`);
  const end = source.indexOf("];", start);
  assert.notEqual(end, -1, `${opener} has no closing bracket`);
  return [...source.slice(start + opener.length, end).matchAll(/Self::([A-Za-z0-9_]+),/gu)].map(
    (match) => match[1] ?? "",
  );
}

/** Compares one shell list against one crate enumeration, both ways and in order. */
function agrees(
  source: string,
  opener: string,
  identifierSignature: string,
  shell: readonly AcademicSectionDefinition[],
  what: string,
): void {
  const identifiers = armsOf(source, identifierSignature);
  assert.ok(identifiers.size > 0, `no arm was read out of the crate for ${what}`);
  const fromCrate = new Set(identifiers.values());
  const fromShell = new Set(shell.map((entry) => entry.id));
  assert.deepEqual(
    [...fromCrate].filter((id) => !fromShell.has(id)),
    [],
    `the crate names a ${what} the shell does not show`,
  );
  assert.deepEqual(
    [...fromShell].filter((id) => !fromCrate.has(id)),
    [],
    `the shell shows a ${what} the crate does not name`,
  );
  const order = allOrder(source, opener);
  assert.deepEqual(
    order.map((arm) => identifiers.get(arm)),
    shell.map((entry) => entry.id),
    `the shell's ${what} order is not the crate's`,
  );
  assert.deepEqual(
    shell.map((entry) => entry.position),
    shell.map((_, index) => index + 1),
    `a shell ${what} is numbered out of its own place`,
  );
}

void test("the_academic_sections_are_the_crates_own", async () => {
  const screen = await readFile(crateUrl("screen.rs"), "utf8");
  const planner = await readFile(crateUrl("planner.rs"), "utf8");
  const course = await readFile(crateUrl("course.rs"), "utf8");
  const auditState = await readFile(crateUrl("audit_state.rs"), "utf8");

  agrees(
    screen,
    "pub const ALL: [Self; 6] = [",
    "pub const fn id(self) -> &'static str {",
    DASHBOARD_SECTIONS,
    "dashboard line",
  );
  agrees(
    planner,
    "pub const ALL: [Self; 6] = [",
    "pub const fn id(self) -> &'static str {",
    PLANNER_SECTIONS,
    "planner axis",
  );
  agrees(
    course,
    "pub const ALL: [Self; 6] = [",
    "pub const fn id(self) -> &'static str {",
    COURSE_SECTIONS,
    "course block",
  );
  agrees(
    course,
    "pub const ALL: [Self; 4] = [",
    "pub const fn spec_word(self) -> &'static str {",
    COVERAGE_TABS,
    "coverage tab",
  );
  agrees(
    auditState,
    "pub const ALL: [Self; 4] = [",
    "pub const fn spec_word(self) -> &'static str {",
    AUDIT_STATES,
    "audit state",
  );

  // The crate's own numbering agrees with the shell's, for the three
  // enumerations that carry one.
  for (const [source, shell] of [
    [screen, DASHBOARD_SECTIONS],
    [planner, PLANNER_SECTIONS],
    [course, COURSE_SECTIONS],
  ] as const) {
    const positions = numericArmsOf(source, "pub const fn position(self) -> usize {");
    assert.ok(positions.size > 0, "no position was read out of the crate");
    assert.deepEqual(
      [...positions.values()].toSorted((left, right) => left - right),
      shell.map((entry) => entry.position),
      "the crate's own numbering is not the shell's",
    );
  }

  // The reader is not vacuous: an arm the crate does not have is not found.
  assert.equal(armsOf(screen, "pub const fn id(self) -> &'static str {").get("GPA_HERO"), undefined);
});

void test("the_graduation_percentage_is_not_one_of_the_six_sections", async () => {
  // The shell half of `percentage_is_secondary_with_breakdown`. Section 25.4
  // calls the percentage a 보조 시각화, and the six sections are the whole of
  // the screen's sequence: a seventh entry here would be an extra key against
  // the crate's own six, which the test above compares in both directions.
  const screen = await readFile(crateUrl("screen.rs"), "utf8");
  const identifiers = armsOf(screen, "pub const fn id(self) -> &'static str {");
  assert.equal(identifiers.size, DASHBOARD_SECTIONS.length);
  for (const entry of DASHBOARD_SECTIONS) {
    assert.ok(
      !/percent|퍼센트|%/iu.test(entry.id) && !/%/u.test(entry.heading),
      `${entry.id} puts the graduation percentage in the screen's own sequence`,
    );
  }
  // And the crate really does keep it off that sequence: `secondary` is a field
  // of the screen and not an arm of `DashboardSection`.
  assert.ok(
    screen.includes("secondary: Option<SecondaryPercentage>"),
    "the crate no longer holds the percentage outside its six sections",
  );
  assert.ok(
    !screen.includes("SecondaryPercentage(") ,
    "the crate now has a section arm carrying a percentage",
  );
});

void test("every_academic_route_opens_its_own_sections", () => {
  // Both directions: every route this file answers for is in the manifest, and
  // every `Academic` route in the manifest is one this file answers for.
  const manifest = new Set(ROUTE_MANIFEST.map((route) => route.id));
  for (const routeId of ACADEMIC_ROUTE_IDS) {
    assert.ok(manifest.has(routeId), `${routeId} is not in the route manifest`);
  }
  const academic = ROUTE_MANIFEST.filter(
    (route) => route.id === "academic" || route.parentId === "academic",
  ).map((route) => route.id);
  assert.deepEqual(
    academic.toSorted(),
    [...ACADEMIC_ROUTE_IDS].toSorted(),
    "the Academic branch of the manifest and this file's routes have diverged",
  );

  // The index names the four children, in the tree's own order.
  assert.deepEqual(
    ACADEMIC_INDEX.map((entry) => entry.heading),
    ROUTE_MANIFEST.filter((route) => route.parentId === "academic").map((route) => route.iaLabel),
    "the Academic index is not the tree's own child order",
  );

  // Opening each destination yields exactly those sections, and the drawer.
  for (const destination of allDestinations()) {
    const route = routeOf(destination);
    if (!(ACADEMIC_ROUTE_IDS as readonly string[]).includes(route.id)) {
      continue;
    }
    const view = openDestination(destination, EMPTY_DRAWER);
    assert.deepEqual(
      view.sections.map((section) => section.heading),
      academicSections(route.id).map((entry) => entry.heading),
      `${route.id} does not open its own sections`,
    );
    for (const section of view.sections) {
      assert.equal(section.filledBy, "P2-X3", `${section.id} is not filled by P2-X3`);
    }
    assert.ok(view.chrome.includes("evidenceDrawer"), `${route.id} lost the evidence drawer`);
  }

  // A route this file does not answer for raises rather than returning nothing.
  assert.throws(() => academicSections("learn.lectures"), /is not an Academic route/u);
});
