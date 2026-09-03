/**
 * `the_shell_sections_are_the_crates_own`.
 *
 * The shell and the crate hold the same six sections, and neither is derived
 * from the other: `academic_evidence_center::CenterSection` is written in Rust
 * and `EVIDENCE_CENTER_SECTIONS` is written here, so a section renamed on one
 * side fails against the other rather than drifting.
 *
 * The comparison reads the crate's source text, because there is no runtime
 * across which the two could be compared: this package links no Rust, and
 * `P2-X1` linked no Tauri runtime either. What is checked is therefore a
 * source-level agreement, and `docs/contracts/policy-source-scans.md` carries
 * this file's row for that reason.
 */

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { allDestinations, routeOf } from "./destinations.js";
import { EMPTY_DRAWER } from "./drawer.js";
import {
  EVIDENCE_CENTER_ROUTES,
  EVIDENCE_CENTER_SECTIONS,
  sectionsForRoute,
} from "./evidence-center.js";
import { ROUTE_MANIFEST } from "./routes.js";
import { openDestination } from "./views.js";

const crateSourceUrl = new URL(
  "../../../crates/evidence-center/src/lib.rs",
  import.meta.url,
);

/** `ProposalInbox` -> `PROPOSAL_INBOX`. */
function screamingSnake(pascal: string): string {
  return pascal.replaceAll(/(?<!^)([A-Z])/gu, "_$1").toUpperCase();
}

void test("the_shell_sections_are_the_crates_own", async () => {
  const source = await readFile(crateSourceUrl, "utf8");

  // The crate's arms, read out of its `spec_words` match rather than out of the
  // enum body: the match is what binds each arm to the specification's words,
  // so reading it gets both halves in one pass and a renamed arm with a stale
  // string fails on the string.
  const armPattern = /Self::([A-Za-z]+) => "([^"]+)",/gu;
  const start = source.indexOf("pub const fn spec_words(self) -> &'static str {");
  assert.notEqual(start, -1, "the crate has no CenterSection::spec_words");
  const end = source.indexOf("\n    }\n", start);
  assert.notEqual(end, -1, "CenterSection::spec_words has no closing brace");
  const body = source.slice(start, end);

  const fromCrate = new Map<string, string>();
  for (const match of body.matchAll(armPattern)) {
    fromCrate.set(screamingSnake(match[1] ?? ""), match[2] ?? "");
  }
  assert.ok(fromCrate.size > 0, "no arm was read out of the crate");

  const fromShell = new Map(
    EVIDENCE_CENTER_SECTIONS.map((section) => [section.id, section.specWords] as const),
  );

  // Both directions, and the specification's words on each side.
  assert.deepEqual(
    [...fromCrate.keys()].filter((id) => !fromShell.has(id)),
    [],
    "the crate names a centre section the shell does not show",
  );
  assert.deepEqual(
    [...fromShell.keys()].filter((id) => !fromCrate.has(id)),
    [],
    "the shell shows a centre section the crate does not name",
  );
  for (const [id, words] of fromShell) {
    assert.equal(fromCrate.get(id), words, `${id} does not carry the crate's own words`);
  }

  // The reader is not vacuous: an arm the crate does not have is not found.
  assert.equal(fromCrate.get("EXPORT_BACKUP_AUDIT"), undefined);
});

void test("every_evidence_section_has_a_route_that_shows_it", () => {
  const routed = new Set(ROUTE_MANIFEST.map((route) => route.id));
  for (const section of EVIDENCE_CENTER_SECTIONS) {
    assert.ok(
      routed.has(section.routeId),
      `${section.id} points at ${section.routeId}, which is not a route`,
    );
  }

  // Every route this file claims to fill has at least one section, except the
  // index, which shows all six.
  for (const routeId of EVIDENCE_CENTER_ROUTES) {
    assert.ok(routed.has(routeId), `${routeId} is not a route`);
    if (routeId === "evidence") {
      continue;
    }
    assert.ok(
      sectionsForRoute(routeId).length > 0,
      `${routeId} is claimed as filled and has no section`,
    );
  }

  // The partition loses nothing: every section is shown by exactly one child.
  const shown = EVIDENCE_CENTER_ROUTES.filter((routeId) => routeId !== "evidence").flatMap(
    (routeId) => sectionsForRoute(routeId).map((section) => section.id),
  );
  assert.deepEqual(
    [...shown].sort(),
    EVIDENCE_CENTER_SECTIONS.map((section) => section.id).sort(),
    "a centre section is shown twice or not at all",
  );
});

void test("the_evidence_branch_opens_with_content_rather_than_a_frame", () => {
  const opened = allDestinations()
    .filter((destination) => routeOf(destination).id.startsWith("evidence"))
    .map((destination) => openDestination(destination, EMPTY_DRAWER));
  assert.ok(opened.length >= 5, `the evidence branch opened ${opened.length} destinations`);

  const byRoute = new Map(
    opened.map((view) => [routeOf(view.destination).id, view] as const),
  );

  // The index shows all six.
  const index = byRoute.get("evidence");
  assert.ok(index !== undefined, "the Evidence & Settings index did not open");
  assert.equal(index.sections.length, EVIDENCE_CENTER_SECTIONS.length);
  for (const section of index.sections) {
    assert.equal(section.filledBy, "P2-X7");
  }

  // Each filled child shows exactly its own sections, by identifier.
  for (const routeId of EVIDENCE_CENTER_ROUTES) {
    if (routeId === "evidence") {
      continue;
    }
    const view = byRoute.get(routeId);
    assert.ok(view !== undefined, `${routeId} did not open`);
    assert.deepEqual(
      view.sections.map((section) => section.id),
      sectionsForRoute(routeId).map((section) => section.id),
      `${routeId} does not show its own sections`,
    );
    for (const section of view.sections) {
      assert.equal(section.filledBy, "P2-X7");
    }
  }

  // And the one route in this branch that is not `P2-X7`'s says so, rather
  // than carrying a promise nobody owns.
  const exportView = byRoute.get("evidence.export-backup-audit");
  assert.ok(exportView !== undefined, "the export route did not open");
  assert.deepEqual(
    [...new Set(exportView.sections.map((section) => section.filledBy))],
    ["P2-P1"],
    "the export route still claims P2-X7 fills it",
  );
});
