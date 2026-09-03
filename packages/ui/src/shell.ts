/**
 * The shell state machine.
 *
 * There is one transition, {@link navigate}, and it carries the evidence drawer
 * forward. The drawer is not a field a view may choose to keep; it is shell
 * state that a navigation cannot drop, which is what
 * `evidence_drawer_persists_across_views` enumerates the ordered pairs of
 * destinations to observe.
 */

import { allDestinations, destinationKey, type Destination } from "./destinations.js";
import { EMPTY_DRAWER, selectInDrawer, type DrawerState } from "./drawer.js";
import type { EntityRef } from "./entities.js";
import { openDestination, type RenderedView } from "./views.js";

/** Everything the shell holds between navigations. */
export interface ShellState {
  /** Where the shell is. */
  readonly destination: Destination;
  /** The right-hand evidence drawer. */
  readonly drawer: DrawerState;
  /** Destinations visited, oldest first. */
  readonly history: readonly string[];
}

/** The destination the shell starts at: the root of the section 25.1 tree. */
export function initialDestination(): Destination {
  const first = allDestinations()[0];
  if (first === undefined) {
    throw new Error("the route manifest yields no destinations");
  }
  return first;
}

/** A shell with nothing pinned, at the root. */
export function initialState(): ShellState {
  const destination = initialDestination();
  return { destination, drawer: EMPTY_DRAWER, history: [destinationKey(destination)] };
}

/**
 * Moves the shell to a destination.
 *
 * The drawer is copied across unchanged. Nothing here consults the destination
 * before deciding to keep it, so there is no route for which the drawer is
 * dropped.
 */
export function navigate(state: ShellState, destination: Destination): ShellState {
  return {
    destination,
    drawer: state.drawer,
    history: [...state.history, destinationKey(destination)],
  };
}

/** Pins an entity into the drawer without moving. */
export function select(state: ShellState, reference: EntityRef): ShellState {
  return { ...state, drawer: selectInDrawer(state.drawer, reference) };
}

/** Renders the current destination. */
export function render(state: ShellState): RenderedView {
  return openDestination(state.destination, state.drawer);
}
