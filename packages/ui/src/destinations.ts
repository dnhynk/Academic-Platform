/**
 * Every place the shell can be.
 *
 * A destination is one route in its index form, or one route in its detail form
 * with an entity identifier bound. The enumeration is derived from the route
 * manifest rather than written out, so a route added to the manifest becomes a
 * destination that `every_destination_opens` must open and that the palette and
 * drawer enumerations must cover, without anyone remembering to extend a second
 * list.
 */

import { entitiesOfKind, refKey, type EntityRef } from "./entities.js";
import { ROUTES_BY_ID, ROUTE_MANIFEST, type RouteDefinition } from "./routes.js";

/** One concrete place in the shell. */
export interface Destination {
  /** Route this destination belongs to. */
  readonly routeId: string;
  /** Concrete path, with any detail parameter substituted. */
  readonly path: string;
  /** The bound entity identifier, or `null` for the index form. */
  readonly entityId: string | null;
}

/** The index destination of a route. */
export function indexDestination(route: RouteDefinition): Destination {
  return { routeId: route.id, path: route.path, entityId: null };
}

/** The detail destination of a route that addresses one entity. */
export function detailDestination(route: RouteDefinition, entityId: string): Destination {
  if (route.detailParam === null) {
    throw new Error(`${route.id} has no detail form`);
  }
  const separator = route.path.endsWith("/") ? "" : "/";
  return {
    routeId: route.id,
    path: `${route.path}${separator}${encodeURIComponent(entityId)}`,
    entityId,
  };
}

/**
 * A representative detail identifier for a route that has a detail form.
 *
 * A route bound to an entity kind uses the first entity of that kind in the
 * synthetic corpus. A route with a detail form and no entity kind -- lectures,
 * repository snapshots, critical paths -- uses a synthetic identifier built
 * from its own id, because the shell addresses those detail forms but the four
 * palette and backlink entity kinds do not include them.
 */
export function representativeDetailId(route: RouteDefinition): string {
  if (route.entityKind !== null) {
    const first = entitiesOfKind(route.entityKind)[0];
    if (first === undefined) {
      throw new Error(`the synthetic corpus holds no ${route.entityKind}`);
    }
    return first.ref.id;
  }
  return `synthetic-${route.id.replaceAll(".", "-")}-1`;
}

/**
 * Every destination the shell can open, in manifest order.
 *
 * Each route contributes its index form, and a route with a detail parameter
 * contributes one representative detail form as well.
 */
export function allDestinations(): readonly Destination[] {
  const destinations: Destination[] = [];
  for (const route of ROUTE_MANIFEST) {
    destinations.push(indexDestination(route));
    if (route.detailParam !== null) {
      destinations.push(detailDestination(route, representativeDetailId(route)));
    }
  }
  return destinations;
}

/** Canonical string form of a destination, for set and map keys. */
export function destinationKey(destination: Destination): string {
  return destination.path;
}

/** The route a destination belongs to. */
export function routeOf(destination: Destination): RouteDefinition {
  const route = ROUTES_BY_ID.get(destination.routeId);
  if (route === undefined) {
    throw new Error(`destination names a route that is not in the manifest: ${destination.routeId}`);
  }
  return route;
}

/** The destination that opens one entity's detail form. */
export function destinationForEntity(reference: EntityRef): Destination {
  const route = ROUTE_MANIFEST.find((candidate) => candidate.entityKind === reference.kind);
  if (route === undefined) {
    throw new Error(`no route in the manifest opens ${refKey(reference)}`);
  }
  return detailDestination(route, reference.id);
}

/** Route ids from the root down to this destination's own route. */
export function breadcrumb(destination: Destination): readonly string[] {
  const trail: string[] = [];
  let current: RouteDefinition | undefined = routeOf(destination);
  while (current !== undefined) {
    trail.unshift(current.id);
    current = current.parentId === null ? undefined : ROUTES_BY_ID.get(current.parentId);
  }
  return trail;
}
