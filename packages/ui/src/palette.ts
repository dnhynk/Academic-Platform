/**
 * The command palette.
 *
 * Section 25.1: "Course, Concept, Project and Question are reached from any
 * screen by command palette and backlink." The palette is therefore built from
 * the corpus and the manifest rather than from the current screen, and it takes
 * the origin only so that the origin can be recorded on each command and so
 * that `palette_reaches_four_entity_types_from_every_route` can enumerate the
 * whole `origin × entity kind` product instead of sampling one screen.
 */

import { allDestinations, destinationForEntity, type Destination } from "./destinations.js";
import { ENTITIES, type EntityRef } from "./entities.js";
import { ROUTES_BY_ID, type EntityKind } from "./routes.js";

/** One entry of the palette. */
export interface PaletteCommand {
  /** Stable identifier, unique inside one palette. */
  readonly id: string;
  /** What the entry reads as. */
  readonly label: string;
  /** The entity kind this entry opens, or `null` for a plain navigation. */
  readonly entityKind: EntityKind | null;
  /** Where the entry goes. */
  readonly target: Destination;
  /** Where the palette was opened from. */
  readonly origin: Destination;
}

/** Case-insensitive substring match, or everything for an empty query. */
function matches(query: string, ...haystacks: readonly string[]): boolean {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) {
    return true;
  }
  return haystacks.some((text) => text.toLowerCase().includes(needle));
}

/**
 * The palette as opened from `origin`, filtered by `query`.
 *
 * Entity entries come first, because reaching the four entity types is what the
 * palette exists for; navigation entries follow.
 */
export function paletteFor(origin: Destination, query = ""): readonly PaletteCommand[] {
  const commands: PaletteCommand[] = [];
  for (const entity of ENTITIES) {
    if (!matches(query, entity.ref.kind, entity.ref.id, entity.title)) {
      continue;
    }
    commands.push({
      id: `entity:${entity.ref.kind}:${entity.ref.id}`,
      label: `${entity.ref.kind}: ${entity.title}`,
      entityKind: entity.ref.kind,
      target: destinationForEntity(entity.ref),
      origin,
    });
  }
  for (const destination of allDestinations()) {
    const route = ROUTES_BY_ID.get(destination.routeId);
    if (route === undefined) {
      throw new Error(`destination names a route that is not in the manifest: ${destination.routeId}`);
    }
    if (!matches(query, route.iaLabel, destination.path)) {
      continue;
    }
    commands.push({
      id: `goto:${destination.path}`,
      label: `Go to ${route.iaLabel}`,
      entityKind: null,
      target: destination,
      origin,
    });
  }
  return commands;
}

/**
 * The palette entries that open one entity kind, as opened from `origin`.
 *
 * Empty is the failure `palette_reaches_four_entity_types_from_every_route`
 * observes for a cell.
 */
export function paletteReach(origin: Destination, kind: EntityKind): readonly PaletteCommand[] {
  return paletteFor(origin).filter((command) => command.entityKind === kind);
}

/** The entity a palette command opens, or `null` for a plain navigation. */
export function commandEntity(command: PaletteCommand): EntityRef | null {
  if (command.entityKind === null || command.target.entityId === null) {
    return null;
  }
  return { kind: command.entityKind, id: command.target.entityId };
}
