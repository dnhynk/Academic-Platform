/**
 * Backlink resolution and traversal.
 *
 * Section 25.1 requires that Course, Concept, Project and Question are reached
 * from any screen by the command palette *and by backlink*. A backlink is
 * traversable when following it lands on a destination that opens and whose own
 * outbound relations lead back, so `backlinks_resolve_for_four_entity_types`
 * checks the round trip rather than the presence of a list.
 */

import { destinationForEntity, type Destination } from "./destinations.js";
import { entityExists, refKey, RELATIONS, type EntityRef, type RelationEdge } from "./entities.js";

/** One traversable backlink. */
export interface Backlink {
  /** The entity that refers to the subject. */
  readonly from: EntityRef;
  /** How it refers to it. */
  readonly kind: RelationEdge["kind"];
  /** The destination that opens the referring entity. */
  readonly destination: Destination;
}

/** The entities that refer to `subject`, in relation order. */
export function backlinksOf(subject: EntityRef): readonly EntityRef[] {
  return RELATIONS.filter((edge) => refKey(edge.to) === refKey(subject)).map((edge) => edge.from);
}

/** The entities `subject` refers to, in relation order. */
export function outboundOf(subject: EntityRef): readonly EntityRef[] {
  return RELATIONS.filter((edge) => refKey(edge.from) === refKey(subject)).map((edge) => edge.to);
}

/**
 * The backlinks into `subject`, each resolved to the destination that opens it.
 *
 * Raises when a relation names an entity the corpus does not hold, so a
 * dangling relation is a failure rather than a silently shorter list.
 */
export function resolveBacklinks(subject: EntityRef): readonly Backlink[] {
  return RELATIONS.filter((edge) => refKey(edge.to) === refKey(subject)).map((edge) => {
    if (!entityExists(edge.from)) {
      throw new Error(`backlink into ${refKey(subject)} names a missing entity: ${refKey(edge.from)}`);
    }
    return { from: edge.from, kind: edge.kind, destination: destinationForEntity(edge.from) };
  });
}

/**
 * Whether the backlink from `origin` into `subject` can be walked back.
 *
 * Walking back means the referring entity's own outbound relations name the
 * subject. That is what makes a backlink a traversal rather than a dead label.
 */
export function traversesBack(origin: EntityRef, subject: EntityRef): boolean {
  return outboundOf(origin).some((target) => refKey(target) === refKey(subject));
}
