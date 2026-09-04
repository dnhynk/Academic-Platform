/**
 * `the_home_sections_are_the_crates_own`, and the shell half of
 * `no_gpa_or_streak_hero_component`.
 *
 * The shell and the crate hold the same eight groups, and neither is derived
 * from the other: `academic_home::HomeGroup` is written in Rust and
 * `HOME_GROUPS` is written here, so a group renamed on one side fails against
 * the other rather than drifting.
 *
 * The comparison reads the crate's source text, because there is no runtime
 * across which the two could be compared: this package links no Rust, and
 * `P2-X1` linked no Tauri runtime either. What is checked is therefore a
 * source-level agreement, and `docs/contracts/policy-source-scans.md` carries
 * this file's row for that reason.
 *
 * The second test is the shell half of an **absence claim**, and it is written
 * the way `P2-U8` writes one: whole sets compared in both directions, with no
 * list of forbidden names anywhere in the file. A ninth section fails as an
 * extra key whatever it is called, and a section that stopped being rendered
 * fails as a missing one.
 */

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { allDestinations, routeOf } from "./destinations.js";
import { EMPTY_DRAWER } from "./drawer.js";
import { HOME_GROUPS, HOME_ROUTE_ID, homeSections } from "./home.js";
import { ROUTE_MANIFEST } from "./routes.js";
import { openDestination } from "./views.js";

const crateSourceUrl = new URL("../../../crates/home/src/lib.rs", import.meta.url);
const specificationUrl = new URL(
  "../../../PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
  import.meta.url,
);

/**
 * The arms of one `match` in the crate, as `Self::Arm => "value"` pairs.
 *
 * Read out of the named function's body rather than out of the enum, so an arm
 * renamed with a stale value fails on the value.
 */
function armsOf(source: string, signature: string): ReadonlyMap<string, string> {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `the crate has no ${signature}`);
  const end = source.indexOf("\n    }\n", start);
  assert.notEqual(end, -1, `${signature} has no closing brace`);
  const body = source.slice(start, end);
  const arms = new Map<string, string>();
  for (const match of body.matchAll(/Self::([A-Za-z0-9_]+) => "([^"]*)",/gu)) {
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

/** The order the crate's `HomeGroup::ALL` lists its arms in. */
function allOrder(source: string): readonly string[] {
  const opener = "pub const ALL: [Self; 8] = [";
  const start = source.indexOf(opener);
  assert.notEqual(start, -1, "the crate has no HomeGroup::ALL");
  const end = source.indexOf("];", start);
  assert.notEqual(end, -1, "HomeGroup::ALL has no closing bracket");
  return [...source.slice(start + opener.length, end).matchAll(/Self::([A-Za-z0-9_]+),/gu)].map(
    (match) => match[1] ?? "",
  );
}

void test("the_home_sections_are_the_crates_own", async () => {
  const source = await readFile(crateSourceUrl, "utf8");

  const identifiers = armsOf(source, "pub const fn id(self) -> &'static str {");
  assert.ok(identifiers.size > 0, "no arm was read out of the crate");

  const fromCrate = new Set(identifiers.values());
  const fromShell = new Set(HOME_GROUPS.map((group) => group.id));
  assert.deepEqual(
    [...fromCrate].filter((id) => !fromShell.has(id)),
    [],
    "the crate names a home group the shell does not show",
  );
  assert.deepEqual(
    [...fromShell].filter((id) => !fromCrate.has(id)),
    [],
    "the shell shows a home group the crate does not name",
  );

  // And in the crate's own order, which is section 25.2's numbering. The arm
  // order comes from `HomeGroup::ALL` and the identifier from the `id` match,
  // so a group moved in one and not the other fails here.
  const order = allOrder(source);
  assert.deepEqual(
    order.map((arm) => identifiers.get(arm)),
    HOME_GROUPS.map((group) => group.id),
    "the shell's order is not the crate's",
  );

  // The shell's own numbering agrees with that order, and the crate's
  // `position` match agrees with both.
  assert.deepEqual(
    HOME_GROUPS.map((group) => group.position),
    HOME_GROUPS.map((_, index) => index + 1),
    "a shell group is numbered out of its own place",
  );
  const positions = numericArmsOf(source, "pub const fn position(self) -> usize {");
  assert.ok(positions.size > 0, "no position was read out of the crate");
  assert.deepEqual(
    order.map((arm) => positions.get(arm)),
    HOME_GROUPS.map((group) => group.position),
    "the crate's own numbering is not the shell's",
  );
  assert.deepEqual(
    [...positions.keys()].filter((arm) => !new Set(order).has(arm)),
    [],
    "the crate numbers an arm that is not in HomeGroup::ALL",
  );

  // The reader is not vacuous: an arm the crate does not have is not found.
  assert.equal(identifiers.get("SEMESTER_AVERAGE"), undefined);
});

void test("the_eight_headings_answer_section_25_2s_eight_lines", async () => {
  const specification = await readFile(specificationUrl, "utf8");
  const heading = "### 25.2 Home / Today";
  const start = specification.indexOf(heading);
  assert.notEqual(start, -1, "the specification has no section 25.2");
  const rest = specification.slice(start + heading.length);
  const end = rest.indexOf("\n### ");
  assert.notEqual(end, -1, "section 25.2 does not end at a following heading");

  const numbered = rest
    .slice(0, end)
    .split(/\r?\n/u)
    .map((line) => /^(\d+)\.\s+(.*)$/u.exec(line.trim()))
    .filter((match): match is RegExpExecArray => match !== null);

  // Both enumerations, positionally. No count is written: the document's own
  // numbering is compared against the shell's, in both directions.
  assert.deepEqual(
    numbered.map((match) => Number(match[1])),
    HOME_GROUPS.map((group) => group.position),
    "section 25.2 does not number the groups the way the shell does",
  );
  assert.ok(numbered.length > 0, "section 25.2 parsed to no numbered lines");
  for (const [index, group] of HOME_GROUPS.entries()) {
    const line = numbered[index]?.[2] ?? "";
    assert.ok(line.length > 0, `section 25.2 has no line ${String(group.position)}`);
    assert.ok(group.heading.trim().length > 0, `${group.id} has no heading`);
  }
});

void test("no_gpa_or_streak_hero_component", async () => {
  // The document still refuses one, so this is not guarding a withdrawn rule.
  const specification = await readFile(specificationUrl, "utf8");
  assert.ok(
    specification.includes("GPA나 streak를 hero metric으로 두지 않는다"),
    "section 25.2 no longer refuses a hero metric",
  );

  // The expectation comes from the **crate**, not from `HOME_GROUPS`.
  //
  // An earlier version of this test compared the rendered sections against
  // `homeSections()`, and an injected ninth entry in `HOME_GROUPS` passed it:
  // both sides of the comparison were the same list, so the comparison
  // asserted nothing about a section added on this side. The crate's arms are
  // an independent enumeration and a ninth shell section fails against them.
  const source = await readFile(crateSourceUrl, "utf8");
  const identifiers = armsOf(source, "pub const fn id(self) -> &'static str {");
  const order = allOrder(source);
  const read = order.map((arm) => identifiers.get(arm));
  assert.ok(read.length > 0, "no group was read out of the crate");
  assert.deepEqual(
    read.filter((id) => id === undefined),
    [],
    "an arm of HomeGroup::ALL has no identifier",
  );
  const expected = read.filter((id): id is string => id !== undefined);
  assert.equal(expected.length, read.length);

  // The home route opens, and what it renders is exactly those groups, in
  // order, with nothing before the first. Compared in both directions, so a
  // ninth section of any name fails as an extra key.
  const opened = allDestinations()
    .filter((destination) => routeOf(destination).id === HOME_ROUTE_ID)
    .map((destination) => openDestination(destination, EMPTY_DRAWER));
  assert.equal(opened.length, 1, "the home route did not open exactly once");
  const view = opened[0];
  assert.ok(view !== undefined, "the home route did not open");

  const rendered = view.sections.map((section) => section.id);
  assert.deepEqual(rendered, expected, "the home view does not render the crate's own groups");
  assert.deepEqual(
    rendered.filter((id) => !new Set(expected).has(id)),
    [],
    "the home view renders a section the crate does not name",
  );
  assert.deepEqual(
    expected.filter((id) => !new Set(rendered).has(id)),
    [],
    "a group the crate names is not rendered",
  );
  // And `homeSections` is what feeds the view, so it is held to the same list.
  assert.deepEqual(
    homeSections().map((group) => group.id),
    expected,
    "homeSections does not return the crate's own groups",
  );
  assert.equal(
    rendered[0],
    "TODAYS_SCHEDULE",
    "something other than today's schedule is rendered first",
  );
  for (const section of view.sections) {
    assert.equal(section.filledBy, "P2-X2");
  }

  // The home route is the only one this file fills, so a heading added to
  // another route is not this task quietly widening its own surface.
  assert.ok(
    ROUTE_MANIFEST.some((route) => route.id === HOME_ROUTE_ID),
    "the home route left the manifest",
  );

  // Every rendered heading is one of the eight's, so a section cannot arrive
  // with a group's identifier and a heading from somewhere else.
  const headings = new Map(HOME_GROUPS.map((group) => [group.id, group.heading] as const));
  assert.deepEqual(
    view.sections.map((section) => section.heading),
    view.sections.map((section) => headings.get(section.id)),
    "a rendered section carries a heading that is not its group's",
  );
});
