/**
 * `palette_reaches_four_entity_types_from_every_route`,
 * `backlinks_resolve_for_four_entity_types` and
 * `evidence_drawer_persists_across_views`.
 *
 * The palette claim is the whole `destination × entity kind` product, and the
 * drawer claim is every ordered pair of destinations, following
 * `cross_capability_matrix_denies_every_disallowed_cell`: a claim about "every
 * screen" is enumerated rather than sampled. Neither test asserts a cell count;
 * both derive their grid from the route manifest.
 */

import assert from "node:assert/strict";
import test from "node:test";

import { backlinksOf, resolveBacklinks, traversesBack } from "./backlinks.js";
import { allDestinations, destinationForEntity, destinationKey } from "./destinations.js";
import { EMPTY_DRAWER, renderDrawer, selectInDrawer } from "./drawer.js";
import { ENTITIES, REPRESENTATIVE_BY_KIND, refKey, RELATIONS } from "./entities.js";
import { commandEntity, paletteFor, paletteReach } from "./palette.js";
import { ENTITY_KINDS, routeForEntityKind } from "./routes.js";
import { initialState, navigate, render, select } from "./shell.js";
import { openDestination } from "./views.js";

void test("palette_reaches_four_entity_types_from_every_route", () => {
  const destinations = allDestinations();
  const covered: string[] = [];
  for (const origin of destinations) {
    for (const kind of ENTITY_KINDS) {
      // The palette has to be there before it can reach anything. Without this
      // the enumeration below would hold for a screen that offers no palette at
      // all, because `paletteFor` is a pure function of the corpus.
      assert.ok(
        openDestination(origin, EMPTY_DRAWER).chrome.includes("commandPalette"),
        `${destinationKey(origin)} carries no command palette`,
      );
      const reach = paletteReach(origin, kind);
      assert.ok(
        reach.length > 0,
        `the palette opened from ${destinationKey(origin)} reaches no ${kind}`,
      );
      for (const command of reach) {
        assert.equal(command.origin, origin);
        const entity = commandEntity(command);
        assert.ok(entity !== null, `${command.id} names an entity kind and opens no entity`);
        assert.equal(entity.kind, kind);
        // The target is the route the manifest says opens this kind, and the
        // entity it binds is one the corpus holds. Both are read from sources
        // the palette did not produce.
        assert.equal(command.target.routeId, routeForEntityKind(kind).id);
        assert.ok(
          ENTITIES.some((candidate) => refKey(candidate.ref) === refKey(entity)),
          `${command.id} opens an entity the corpus does not hold`,
        );
        // The cell is only reached if its target actually opens.
        const view = openDestination(command.target, EMPTY_DRAWER);
        assert.ok(view.sections.length > 0, `${command.id} opened an empty view`);
        assert.equal(view.destination.entityId, entity.id);
      }
      covered.push(`${destinationKey(origin)}|${kind}`);
    }
  }
  // Every cell of the grid was visited exactly once, and the grid is the
  // manifest's destinations by the four entity kinds.
  assert.equal(new Set(covered).size, covered.length, "a cell was enumerated twice");
  assert.equal(covered.length, destinations.length * ENTITY_KINDS.length);
});

void test("palette_reaches_four_entity_types_from_every_route rejects its violations", () => {
  const origin = allDestinations()[0];
  assert.ok(origin !== undefined);

  // A query that matches nothing yields no reach, which is the empty cell the
  // assertion above refuses.
  for (const kind of ENTITY_KINDS) {
    assert.equal(
      paletteFor(origin, "zzz-no-such-entity").filter((command) => command.entityKind === kind)
        .length,
      0,
    );
  }
  // Filtering by a kind's own name still reaches it, so the reach above is not
  // an artefact of an unfiltered palette.
  for (const kind of ENTITY_KINDS) {
    assert.ok(
      paletteFor(origin, kind).some((command) => command.entityKind === kind),
      `filtering by ${kind} lost the ${kind} entries`,
    );
  }
});

void test("backlinks_resolve_for_four_entity_types", () => {
  // Every entity is walked, not one representative per kind: a kind whose
  // representative happens to be well connected would otherwise stand in for
  // every other entity of that kind.
  for (const entity of ENTITIES) {
    const subject = entity.ref;

    // The expectation is derived here, from the relation table, rather than
    // taken from `backlinksOf`. Comparing the shell's answer against the same
    // function that produced it would hold whatever that function did.
    const expected = RELATIONS.filter((edge) => refKey(edge.to) === refKey(subject))
      .map((edge) => refKey(edge.from))
      .toSorted();
    assert.deepEqual(backlinksOf(subject).map(refKey).toSorted(), expected);
    assert.deepEqual(
      resolveBacklinks(subject)
        .map((backlink) => refKey(backlink.from))
        .toSorted(),
      expected,
    );

    // The subject's own detail view shows that same set.
    const subjectView = openDestination(destinationForEntity(subject), EMPTY_DRAWER);
    assert.deepEqual(subjectView.backlinks.map(refKey).toSorted(), expected);

    for (const backlink of resolveBacklinks(subject)) {
      // Resolving means the referring entity has a destination that opens, and
      // that the destination is the referring entity's own detail form.
      const view = openDestination(backlink.destination, EMPTY_DRAWER);
      assert.ok(view.sections.length > 0, `the backlink into ${refKey(subject)} opened nothing`);
      assert.equal(
        backlink.destination.entityId,
        backlink.from.id,
        `the backlink into ${refKey(subject)} opened something other than ${refKey(backlink.from)}`,
      );
      // Traversable means the walk goes back the way it came.
      assert.ok(
        traversesBack(backlink.from, subject),
        `the backlink from ${refKey(backlink.from)} does not traverse back to ${refKey(subject)}`,
      );
    }
  }

  // And each of the four kinds actually has an entity something links to, so
  // the walk above is not satisfied by a corpus with no inbound edges at all.
  for (const kind of ENTITY_KINDS) {
    const linked = ENTITIES.filter(
      (entity) => entity.ref.kind === kind && resolveBacklinks(entity.ref).length > 0,
    );
    assert.ok(linked.length > 0, `nothing in the corpus links to any ${kind}`);
    const representative = REPRESENTATIVE_BY_KIND.get(kind);
    assert.ok(representative !== undefined, `no representative ${kind}`);
    assert.ok(
      resolveBacklinks(representative).length > 0,
      `nothing links to the representative ${kind}`,
    );
  }

  // No relation dangles: both ends of every edge are entities the corpus holds.
  const known = new Set(ENTITIES.map((entity) => refKey(entity.ref)));
  for (const edge of RELATIONS) {
    assert.ok(known.has(refKey(edge.from)), `relation from a missing entity: ${refKey(edge.from)}`);
    assert.ok(known.has(refKey(edge.to)), `relation into a missing entity: ${refKey(edge.to)}`);
  }
});

void test("backlinks_resolve_for_four_entity_types rejects its violations", () => {
  const subject = REPRESENTATIVE_BY_KIND.get("Concept");
  assert.ok(subject !== undefined);

  // An entity nothing refers to has no backlinks, which is the empty result the
  // assertion above refuses for each of the four kinds.
  assert.deepEqual(resolveBacklinks({ kind: "Concept", id: "not-in-the-corpus" }), []);

  // A one-way edge does not traverse back.
  const oneWay = RELATIONS.find((edge) => !traversesBack(edge.to, edge.from));
  assert.ok(oneWay !== undefined, "the corpus has no one-way edge to test the round trip against");
  assert.equal(traversesBack(oneWay.to, oneWay.from), false);
  assert.equal(traversesBack(oneWay.from, oneWay.to), true);
});

void test("evidence_drawer_persists_across_views", () => {
  const destinations = allDestinations();
  const pinned = REPRESENTATIVE_BY_KIND.get("Concept");
  assert.ok(pinned !== undefined);

  let pairs = 0;
  for (const from of destinations) {
    for (const to of destinations) {
      if (destinationKey(from) === destinationKey(to)) {
        continue;
      }
      const before = select(navigate(initialState(), from), pinned);
      assert.deepEqual(before.drawer.selected, pinned);
      assert.deepEqual(render(before).drawer.selected, pinned);

      const after = navigate(before, to);
      assert.deepEqual(
        after.drawer.selected,
        pinned,
        `navigating ${destinationKey(from)} -> ${destinationKey(to)} dropped the drawer selection`,
      );

      const view = render(after);
      assert.equal(view.destination, to);
      assert.ok(
        view.chrome.includes("evidenceDrawer"),
        `${destinationKey(to)} carries no evidence drawer`,
      );
      assert.equal(view.drawer.side, "right");
      assert.deepEqual(view.drawer.selected, pinned);
      assert.ok(view.drawer.evidence.length > 0, "the drawer rendered no evidence for a selection");
      pairs += 1;
    }
  }
  assert.equal(pairs, destinations.length * (destinations.length - 1));

  // The drawer also survives when nothing is pinned: every view has the panel,
  // and navigating never invents a selection. Without this half, a `navigate`
  // that simply set the drawer to a fixed entity would satisfy the loop above.
  for (const destination of destinations) {
    const view = openDestination(destination, EMPTY_DRAWER);
    assert.equal(view.drawer.side, "right");
    assert.equal(view.drawer.selected, null);

    const walked = navigate(initialState(), destination);
    assert.equal(
      walked.drawer.selected,
      null,
      `navigating to ${destinationKey(destination)} invented a drawer selection`,
    );
    assert.equal(render(walked).drawer.selected, null);
  }
});

void test("evidence_drawer_persists_across_views rejects its violations", () => {
  const pinned = REPRESENTATIVE_BY_KIND.get("Project");
  assert.ok(pinned !== undefined);
  const state = select(initialState(), pinned);

  // A navigation that rebuilt the drawer instead of carrying it would lose the
  // selection; this is that navigation, and it is not the one `navigate` runs.
  const dropped = { ...state, drawer: EMPTY_DRAWER };
  assert.equal(dropped.drawer.selected, null);
  assert.notDeepEqual(navigate(state, state.destination).drawer, dropped.drawer);

  // A drawer pinned to an entity the corpus does not hold raises rather than
  // rendering an empty panel that looks like "nothing selected".
  assert.throws(
    () => renderDrawer(selectInDrawer(EMPTY_DRAWER, { kind: "Course", id: "absent" })),
    /the corpus does not hold/u,
  );
});
