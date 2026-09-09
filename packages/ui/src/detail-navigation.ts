/** Shell links and pinned evidence derived from the accepted detail projection. */
import type { DetailState } from "./detail-client.js";
import { relationRejected } from "./detail-client.js";
import type { Relation } from "./details.js";
import { detailDestination, type Destination } from "./destinations.js";
import type { DrawerEvidenceLine } from "./drawer.js";
import type { EntityRef } from "./entities.js";
import type { PaletteCommand } from "./palette.js";
import { ROUTES_BY_ID, type EntityKind } from "./routes.js";

interface ProfileEntry { readonly id: string; readonly title: string; readonly routeId: string; readonly kind: EntityKind | null; readonly relations: readonly Relation[] }
function entries(state: DetailState): readonly ProfileEntry[] {
  return [
    ...state.corpus.lectures.map((lecture) => ({ id: lecture.id, title: lecture.title, routeId: "learn.lectures", kind: null, relations: Object.values(lecture.links).flat() })),
    ...state.corpus.concepts.map((concept) => ({ id: concept.id, title: concept.title, routeId: "learn.concepts", kind: "Concept" as const, relations: Object.values(concept.relations).flat() })),
    ...state.corpus.projects.map((project) => ({ id: project.id, title: project.title, routeId: "build.projects", kind: "Project" as const, relations: Object.values(project.relations).flat() })),
    ...state.corpus.questions.map((question) => ({ id: question.id, title: question.revisions.at(-1)?.text ?? question.id, routeId: "learn.questions", kind: "Question" as const, relations: [...question.evidence, ...question.revisions.flatMap((revision) => [...revision.concepts, ...revision.resolutionEvidence])] })),
  ];
}
export function profilePalette(state: DetailState | null, origin: Destination, query: string): readonly PaletteCommand[] {
  if (!state) return [];
  const needle = query.trim().toLowerCase();
  return entries(state).flatMap((entry) => {
    const route = ROUTES_BY_ID.get(entry.routeId); if (!route) return [];
    const target = detailDestination(route, entry.id);
    if (![entry.title, entry.kind ?? "Lecture", entry.id, target.path].some((value) => value.toLowerCase().includes(needle))) return [];
    return [{ id: `profile:${entry.routeId}:${entry.id}`, label: `${entry.kind ?? "Lecture"}: ${entry.title}`, entityKind: entry.kind, target, origin }];
  });
}
export function profileTitle(state: DetailState | null, destination: Destination): string | null {
  if (!state) return null;
  return entries(state).find((entry) => entry.routeId === destination.routeId && entry.id === destination.entityId)?.title ?? null;
}
export function profileBacklinks(state: DetailState | null, destination: Destination): readonly { readonly title: string; readonly target: Destination }[] {
  if (!state) return [];
  return entries(state).flatMap((entry) => {
    if (!entry.relations.some((relation) => !relationRejected(state, relation.id) && relation.target?.routeId === destination.routeId && relation.target.id === destination.entityId)) return [];
    const route = ROUTES_BY_ID.get(entry.routeId); return route ? [{ title: entry.title, target: detailDestination(route, entry.id) }] : [];
  });
}
export function profileEvidence(state: DetailState | null, reference: EntityRef): { readonly title: string; readonly evidence: readonly DrawerEvidenceLine[] } | null {
  if (!state) return null;
  const entry = entries(state).find((item) => item.kind === reference.kind && item.id === reference.id); if (!entry) return null;
  return { title: `Evidence — ${entry.title}`, evidence: [
    { source: `Profile ${state.profile_id} · ${state.projector_version}`, statement: `Accepted source watermark ${String(state.known_at_accept_seq)} · revision ${String(state.revision)} · digest ${state.source_digest}` },
    { source: "Imported detail metadata", statement: "Imported states and confidence do not establish user confirmation; accepted rejection and undo receipts are separate." },
    ...[...new Map(entry.relations.map((relation) => [relation.id, relation])).values()].map((relation) => ({ source: `${relation.source.title} · ${relation.source.locator}`, statement: `${relation.label} · ${relationRejected(state, relation.id) ? "REJECTED" : `Reported ${relation.status}`} · reported confidence ${relation.confidence}\n${relation.source.content}` })),
  ] };
}
