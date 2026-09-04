/**
 * `the_cs_map_lenses_are_the_crates_own` and
 * `the_cs_map_regions_are_the_specifications`.
 *
 * The shell and the crate hold the same ten lenses, and neither is derived from
 * the other: `academic_cs_map::MapLens` is written in Rust and `CS_MAP_LENSES`
 * is written here, so a lens renamed on one side fails against the other rather
 * than drifting.
 *
 * The comparison reads the crate's source text, because there is no runtime
 * across which the two could be compared: this package links no Rust, and
 * `P2-X1` linked no Tauri runtime either. What is checked is therefore a
 * source-level agreement, and `docs/contracts/policy-source-scans.md` carries
 * this file's row for that reason.
 *
 * Both tests are whole-set comparisons in both directions, with no list of
 * forbidden names anywhere in the file. An eleventh lens fails as an extra key
 * whatever it is called, and a region that stopped being rendered fails as a
 * missing one.
 */

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { CS_MAP_LENSES, CS_MAP_REGIONS, CS_MAP_ROUTE_ID, csMapRegions } from "./cs-map.js";
import { allDestinations, routeOf } from "./destinations.js";
import { EMPTY_DRAWER } from "./drawer.js";
import { openDestination } from "./views.js";

const lensSourceUrl = new URL("../../../crates/cs-map/src/lens.rs", import.meta.url);
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
  const arms = new Map<string, string>();
  for (const match of source.slice(start, end).matchAll(/Self::([A-Za-z0-9_]+) => "([^"]*)",/gu)) {
    arms.set(match[1] ?? "", match[2] ?? "");
  }
  return arms;
}

/** The order the crate's `MAP_LENSES` lists its arms in. */
function lensOrder(source: string): readonly string[] {
  const opener = "pub const MAP_LENSES: [MapLens; 10] = [";
  const start = source.indexOf(opener);
  assert.notEqual(start, -1, "the crate has no MAP_LENSES");
  const end = source.indexOf("];", start);
  assert.notEqual(end, -1, "MAP_LENSES has no closing bracket");
  return [
    ...source.slice(start + opener.length, end).matchAll(/MapLens::([A-Za-z0-9_]+),/gu),
  ].map((match) => match[1] ?? "");
}

/** Section 25.3's body, up to the next heading. */
function csMapSection(specification: string): string {
  const start = specification.indexOf("### 25.3 CS Map / YOU ARE HERE");
  assert.notEqual(start, -1, "the design document no longer holds section 25.3");
  const rest = specification.slice(start);
  const end = rest.indexOf("\n### ", 1);
  assert.notEqual(end, -1, "section 25.3 has no successor heading");
  return rest.slice(0, end);
}

void test("the_cs_map_lenses_are_the_crates_own", async () => {
  const source = await readFile(lensSourceUrl, "utf8");

  const identifiers = armsOf(source, "pub const fn as_str(self) -> &'static str {");
  assert.ok(identifiers.size > 0, "no arm was read out of the crate");

  const fromCrate = new Set(identifiers.values());
  const fromShell = new Set(CS_MAP_LENSES);
  assert.deepEqual(
    [...fromCrate].filter((id) => !fromShell.has(id)),
    [],
    "the crate names a lens the rail does not offer",
  );
  assert.deepEqual(
    [...fromShell].filter((id) => !fromCrate.has(id)),
    [],
    "the rail offers a lens the crate does not name",
  );

  // And in the crate's own order, which is section 25.3's. The arm order comes
  // from `MAP_LENSES` and the identifier from the `as_str` match, so a lens
  // moved in one and not the other fails here.
  assert.deepEqual(
    lensOrder(source).map((arm) => identifiers.get(arm)),
    [...CS_MAP_LENSES],
    "the rail's order is not the crate's",
  );

  // Negative control: an arm the crate does not have reads as absent, so the
  // comparison above is measuring the source rather than always agreeing.
  assert.equal(identifiers.get("Curriculum"), undefined);
});

void test("the_cs_map_regions_are_the_specifications", async () => {
  const specification = await readFile(specificationUrl, "utf8");
  const section = csMapSection(specification);

  const bullets = section
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith("- "))
    .map((line) => line.slice(2));
  assert.ok(bullets.length > 0, "section 25.3 no longer has a bullet list");

  const heads = bullets.map((bullet) => {
    const at = bullet.indexOf(":");
    assert.ok(at > 0, `section 25.3's bullet has no head: ${bullet}`);
    return bullet.slice(0, at).trim();
  });

  assert.deepEqual(
    heads,
    CS_MAP_REGIONS.map((region) => region.specBulletHead),
    "the shell's regions are not section 25.3's, in its order",
  );

  // The shell's own numbering agrees with that order.
  assert.deepEqual(
    CS_MAP_REGIONS.map((region) => region.position),
    CS_MAP_REGIONS.map((_, index) => index + 1),
    "a region is numbered out of its own place",
  );

  // Section 25.3's first sentence is the first screen's whole content, and the
  // shell's map region is where it lands. A shell that dropped the region would
  // leave the route with nothing to put it in.
  assert.ok(
    section.includes("10–20개 Field cluster"),
    "section 25.3 no longer states the first screen",
  );
  assert.ok(
    CS_MAP_REGIONS.some((region) => region.id === "ATLAS"),
    "the shell has no region for the map itself",
  );
});

void test("the_cs_map_route_opens_its_five_regions", () => {
  const destination = allDestinations().find(
    (candidate) => routeOf(candidate).id === CS_MAP_ROUTE_ID && candidate.entityId === null,
  );
  assert.ok(destination, "the manifest no longer holds the CS map index route");

  const view = openDestination(destination, EMPTY_DRAWER);
  assert.deepEqual(
    view.sections.map((section) => section.heading),
    csMapRegions().map((region) => region.heading),
    "the rendered view is not the five regions",
  );
  assert.equal(view.sections.length, CS_MAP_REGIONS.length);
  for (const section of view.sections) {
    assert.equal(section.filledBy, "P2-X5");
  }
});
