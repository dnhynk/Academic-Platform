/**
 * The right-hand evidence drawer.
 *
 * Section 25.1 requires that the evidence drawer for the currently selected
 * entity stays open while the user moves between screens, so that reading
 * evidence does not cost a tab. The drawer is therefore modelled as shell
 * state, not as view state: `shell.ts` carries it across every navigation and
 * `views.ts` renders it into every view. `evidence_drawer_persists_across_views`
 * enumerates the ordered pairs of destinations and observes that the selection
 * survives each one.
 */

import { entityFor, type EntityRef } from "./entities.js";

/** The drawer's fixed side. Section 25.1 says right. */
export const DRAWER_SIDE = "right" as const;

/** One line of evidence shown in the drawer. */
export interface DrawerEvidenceLine {
  /** What produced the line. */
  readonly source: string;
  /** What the line says. */
  readonly statement: string;
}

/** The drawer's state: which entity is pinned, and nothing about the view. */
export interface DrawerState {
  /** The pinned entity, or `null` when nothing is selected. */
  readonly selected: EntityRef | null;
}

/** The drawer as it appears inside one rendered view. */
export interface DrawerPanel {
  readonly side: typeof DRAWER_SIDE;
  readonly selected: EntityRef | null;
  readonly title: string;
  readonly evidence: readonly DrawerEvidenceLine[];
}

/** A drawer with nothing pinned. */
export const EMPTY_DRAWER: DrawerState = { selected: null };

/** Pins an entity into the drawer. */
export function selectInDrawer(state: DrawerState, reference: EntityRef): DrawerState {
  void state;
  return { selected: reference };
}

/** Clears the drawer. */
export function clearDrawer(): DrawerState {
  return EMPTY_DRAWER;
}

/**
 * Renders the drawer for a view.
 *
 * The panel is produced for every view whether or not anything is pinned, so
 * that the drawer is a fixture of the shell rather than something a view opts
 * into.
 */
export function renderDrawer(state: DrawerState): DrawerPanel {
  if (state.selected === null) {
    return {
      side: DRAWER_SIDE,
      selected: null,
      title: "Evidence",
      evidence: [],
    };
  }
  const entity = entityFor(state.selected);
  if (entity === undefined) {
    throw new Error(`the drawer pins an entity the corpus does not hold: ${state.selected.id}`);
  }
  return {
    side: DRAWER_SIDE,
    selected: state.selected,
    title: `Evidence — ${entity.title}`,
    evidence: [
      {
        source: "synthetic corpus",
        statement: `${entity.ref.kind} ${entity.ref.id} is a fixture entity.`,
      },
    ],
  };
}
