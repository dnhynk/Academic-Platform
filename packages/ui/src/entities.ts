/**
 * The synthetic entity corpus the shell navigates.
 *
 * `CONTRIBUTING.md` permits synthetic fixtures only. Every identifier, title
 * and relation here is invented in this file and built in process; nothing is
 * read from a profile, a database or a network, and the desktop has no way to
 * reach any of those. The corpus exists so that opening a destination,
 * resolving a backlink, and traversing back are observable operations rather
 * than assertions about a shape nothing produces.
 */

import type { EntityKind } from "./routes.js";
import { ENTITY_KINDS } from "./routes.js";

/** A reference to one entity of one kind. */
export interface EntityRef {
  readonly kind: EntityKind;
  readonly id: string;
}

/** One entity in the synthetic corpus. */
export interface Entity {
  readonly ref: EntityRef;
  readonly title: string;
}

/** A directed relation between two entities. */
export interface RelationEdge {
  readonly from: EntityRef;
  readonly to: EntityRef;
  readonly kind: RelationKind;
}

/** The relation kinds the shell can traverse. */
export type RelationKind =
  | "COVERS"
  | "REQUIRES"
  | "EVIDENCED_BY"
  | "ASKS_ABOUT"
  | "ANSWERED_BY";

function ref(kind: EntityKind, id: string): EntityRef {
  return { kind, id };
}

/** The synthetic entities, one small connected corpus. */
export const ENTITIES: readonly Entity[] = [
  { ref: ref("Course", "4190.101"), title: "Synthetic Discrete Mathematics" },
  { ref: ref("Course", "4190.310"), title: "Synthetic Algorithms" },
  { ref: ref("Concept", "amortized-analysis"), title: "Amortized analysis" },
  { ref: ref("Concept", "union-find"), title: "Disjoint set union" },
  { ref: ref("Project", "synthetic-graph-lab"), title: "Synthetic graph lab" },
  { ref: ref("Question", "q-why-inverse-ackermann"), title: "Why does the bound involve α(n)?" },
];

/**
 * The relation edges.
 *
 * Every edge is traversed in both directions by `backlinks.ts`: forward as an
 * outbound relation and backward as a backlink. The corpus is arranged so that
 * each of the four entity kinds is the target of at least one edge, which is
 * what `backlinks_resolve_for_four_entity_types` enumerates over.
 */
export const RELATIONS: readonly RelationEdge[] = [
  { from: ref("Course", "4190.310"), to: ref("Concept", "amortized-analysis"), kind: "COVERS" },
  { from: ref("Course", "4190.310"), to: ref("Concept", "union-find"), kind: "COVERS" },
  { from: ref("Concept", "union-find"), to: ref("Concept", "amortized-analysis"), kind: "REQUIRES" },
  { from: ref("Concept", "union-find"), to: ref("Course", "4190.101"), kind: "REQUIRES" },
  {
    from: ref("Project", "synthetic-graph-lab"),
    to: ref("Concept", "union-find"),
    kind: "EVIDENCED_BY",
  },
  {
    from: ref("Question", "q-why-inverse-ackermann"),
    to: ref("Concept", "amortized-analysis"),
    kind: "ASKS_ABOUT",
  },
  {
    from: ref("Concept", "amortized-analysis"),
    to: ref("Question", "q-why-inverse-ackermann"),
    kind: "ANSWERED_BY",
  },
  {
    from: ref("Course", "4190.310"),
    to: ref("Project", "synthetic-graph-lab"),
    kind: "EVIDENCED_BY",
  },
];

/** Canonical string form of a reference, for set and map keys. */
export function refKey(reference: EntityRef): string {
  return `${reference.kind}:${reference.id}`;
}

/** Whether the corpus holds this entity. */
export function entityExists(reference: EntityRef): boolean {
  return ENTITIES.some((entity) => refKey(entity.ref) === refKey(reference));
}

/** The entity this reference names, or `undefined`. */
export function entityFor(reference: EntityRef): Entity | undefined {
  return ENTITIES.find((entity) => refKey(entity.ref) === refKey(reference));
}

/** Every entity of one kind. */
export function entitiesOfKind(kind: EntityKind): readonly Entity[] {
  return ENTITIES.filter((entity) => entity.ref.kind === kind);
}

/**
 * One representative entity per kind, chosen as the first in corpus order.
 *
 * Used wherever a test has to enumerate `route × entity kind` and needs a
 * concrete target for each kind.
 */
export const REPRESENTATIVE_BY_KIND: ReadonlyMap<EntityKind, EntityRef> = new Map(
  ENTITY_KINDS.map((kind) => {
    const first = entitiesOfKind(kind)[0];
    if (first === undefined) {
      throw new Error(`the synthetic corpus holds no ${kind}`);
    }
    return [kind, first.ref];
  }),
);
