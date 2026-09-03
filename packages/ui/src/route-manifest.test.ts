/**
 * `route_manifest_matches_ia_exactly` and `every_destination_opens`.
 *
 * Both sides of the first comparison are enumerated from independent sources:
 * section 25.1's drawn tree is parsed out of the specification, and the route
 * manifest is written by hand. The comparison is a set equality in both
 * directions and a parent-child equality on top of it, so a route the manifest
 * adds fails and a line the specification adds fails. No count appears in this
 * file.
 */

import assert from "node:assert/strict";
import test from "node:test";

import { allDestinations, breadcrumb, destinationKey, routeOf } from "./destinations.js";
import { EMPTY_DRAWER } from "./drawer.js";
import { parseIaTree, readIaTree, type IaNode } from "./ia.js";
import { ROUTE_MANIFEST, ROUTES_BY_ID } from "./routes.js";
import { openDestination, SHELL_CHROME, VIEW_BUILDERS } from "./views.js";

const specificationUrl = new URL(
  "../../../PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
  import.meta.url,
);

function labelsOf(nodes: readonly IaNode[]): readonly string[] {
  return nodes.map((node) => node.label);
}

void test("route_manifest_matches_ia_exactly", async () => {
  const ia = await readIaTree(specificationUrl);

  // Labels: neither side may hold one the other does not.
  const specificationLabels = new Set(labelsOf(ia));
  const manifestLabels = new Set(ROUTE_MANIFEST.map((route) => route.iaLabel));
  assert.deepEqual(
    [...specificationLabels].filter((label) => !manifestLabels.has(label)),
    [],
    "section 25.1 names a destination the route manifest does not",
  );
  assert.deepEqual(
    [...manifestLabels].filter((label) => !specificationLabels.has(label)),
    [],
    "the route manifest names a destination section 25.1 does not",
  );

  // One route per line, so a duplicated label cannot hide a missing one.
  assert.equal(
    manifestLabels.size,
    ROUTE_MANIFEST.length,
    "two routes claim the same section 25.1 label",
  );
  assert.equal(specificationLabels.size, ia.length, "section 25.1 draws the same label twice");

  // Structure: the parent of each label is the same on both sides.
  const manifestParentByLabel = new Map(
    ROUTE_MANIFEST.map((route) => {
      const parent = route.parentId === null ? null : ROUTES_BY_ID.get(route.parentId);
      if (route.parentId !== null && parent === undefined) {
        throw new Error(`${route.id} names a parent that is not in the manifest`);
      }
      return [route.iaLabel, parent?.iaLabel ?? null];
    }),
  );
  const specificationParentByLabel = new Map(ia.map((node) => [node.label, node.parentLabel]));
  assert.deepEqual(
    Object.fromEntries([...manifestParentByLabel].toSorted()),
    Object.fromEntries([...specificationParentByLabel].toSorted()),
    "the manifest's tree shape differs from section 25.1's",
  );

  // Order: the manifest is written in section 25.1's own reading order.
  assert.deepEqual(
    ROUTE_MANIFEST.map((route) => route.iaLabel),
    labelsOf(ia),
    "the manifest is not in section 25.1's order",
  );
});

void test("route_manifest_matches_ia_exactly rejects its violations", () => {
  const fence = [
    "Home / Today",
    "├─ Academic",
    "│  └─ Dashboard",
    "└─ Learn",
    "   └─ Questions",
  ].join("\n");
  const parsed = parseIaTree(fence);
  assert.deepEqual(labelsOf(parsed), [
    "Home / Today",
    "Academic",
    "Dashboard",
    "Learn",
    "Questions",
  ]);
  assert.deepEqual(
    parsed.map((node) => node.parentLabel),
    [null, "Home / Today", "Academic", "Home / Today", "Learn"],
  );

  // A removed line is a removed destination.
  const withoutOne = parseIaTree(
    ["Home / Today", "├─ Academic", "│  └─ Dashboard", "└─ Learn"].join("\n"),
  );
  assert.equal(
    labelsOf(withoutOne).includes("Questions"),
    false,
    "the parser did not notice a removed line",
  );

  // A line the parser cannot account for raises rather than being skipped.
  assert.throws(
    () => parseIaTree(["Home / Today", "   Dashboard"].join("\n")),
    /branch marker/u,
    "an indented line with no branch marker was skipped instead of raising",
  );
  assert.throws(
    () => parseIaTree(["Home / Today", "Second Root"].join("\n")),
    /second root/u,
    "a second root was accepted",
  );
  assert.throws(() => parseIaTree(""), /no nodes at all/u, "an empty fence parsed to a tree");
});

void test("every_destination_opens", () => {
  // The view registry and the manifest are compared in both directions first,
  // so a destination that opens because its route was silently dropped from one
  // of the two lists is a failure rather than an absence.
  const registered = new Set(VIEW_BUILDERS.keys());
  const routed = new Set(ROUTE_MANIFEST.map((route) => route.id));
  assert.deepEqual(
    [...routed].filter((id) => !registered.has(id)),
    [],
    "a route in the manifest has no registered view",
  );
  assert.deepEqual(
    [...registered].filter((id) => !routed.has(id)),
    [],
    "a view is registered for a route that is not in the manifest",
  );

  const destinations = allDestinations();
  const seen = new Set<string>();
  for (const destination of destinations) {
    const key = destinationKey(destination);
    assert.equal(seen.has(key), false, `two destinations share the path ${key}`);
    seen.add(key);

    const view = openDestination(destination, EMPTY_DRAWER);
    assert.equal(view.destination, destination);
    assert.ok(view.title.length > 0, `${key} opened with no title`);
    assert.ok(view.sections.length > 0, `${key} opened with no sections`);
    assert.equal(
      new Set(view.sections.map((section) => section.id)).size,
      view.sections.length,
      `${key} has two sections with the same id`,
    );
    assert.equal(view.drawer.side, "right", `${key} did not carry the right-hand drawer`);
    assert.deepEqual(
      view.chrome.toSorted(),
      [...SHELL_CHROME].toSorted(),
      `${key} withholds a shell affordance section 25.1 requires on every screen`,
    );

    // The breadcrumb reaches the root of the tree from every destination. The
    // expectation is walked here, from the manifest, rather than taken from the
    // same helper the view used.
    const expectedTrail: string[] = [];
    for (
      let cursor = ROUTES_BY_ID.get(destination.routeId);
      cursor !== undefined;
      cursor = cursor.parentId === null ? undefined : ROUTES_BY_ID.get(cursor.parentId)
    ) {
      expectedTrail.unshift(cursor.id);
    }
    const trail = breadcrumb(destination);
    assert.deepEqual(trail, expectedTrail, `${key} breadcrumb does not match the manifest`);
    assert.equal(trail.at(-1), destination.routeId, `${key} breadcrumb does not end at its route`);
    assert.equal(trail[0], "home", `${key} breadcrumb does not start at the root`);
    assert.deepEqual(view.breadcrumb, trail);
  }

  // Every route contributed its index form, and a route with a detail parameter
  // contributed a detail form as well.
  for (const route of ROUTE_MANIFEST) {
    const own = destinations.filter((destination) => destination.routeId === route.id);
    assert.equal(
      own.length,
      route.detailParam === null ? 1 : 2,
      `${route.id} contributed ${String(own.length)} destinations`,
    );
    assert.equal(routeOf(own[0] as never).id, route.id);
  }
});

void test("every_destination_opens rejects its violations", () => {
  const destination = allDestinations()[0];
  assert.ok(destination !== undefined);

  // A destination naming a route with no view raises rather than blanking.
  assert.throws(
    () => openDestination({ ...destination, routeId: "not-a-route" }, EMPTY_DRAWER),
    /not in the manifest/u,
  );

  // A route present in the manifest and absent from the registry is the
  // asymmetry the equality above refuses; observed here against a copy so the
  // committed registry is not mutated.
  const shortened = new Map(VIEW_BUILDERS);
  shortened.delete("evidence.privacy-providers");
  assert.deepEqual(
    ROUTE_MANIFEST.map((route) => route.id).filter((id) => !shortened.has(id)),
    ["evidence.privacy-providers"],
  );
});
