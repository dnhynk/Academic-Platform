/** The view changes accepted decisions only after a matching native response. */
import { allRelations, coverage, selectCapture, type DetailCorpus, type RelationDecision } from "./details.js";
import type { NativeInvoke } from "./runtime-request.js";

export interface DetailState {
  readonly corpus: DetailCorpus; readonly decisions: readonly RelationDecision[]; readonly revision: number;
  readonly profile_id: string; readonly known_at_accept_seq: number; readonly valid_at_ms: number;
  readonly projector_version: string; readonly source_digest: string;
}
export interface DetailClient {
  read(selector?: DetailSelector): Promise<DetailState>;
  decide(relationId: string, action: "reject" | "undo", expectedRevision: number): Promise<DetailState>;
  pendingDecisions(): readonly DetailDecisionRequest[];
  audio(lectureId: string, signal?: AbortSignal): Promise<OriginalAudio>;
}
/** Local retry identities only; these records never establish acceptance. */
export interface DetailRetryStorage {
  keys(): readonly string[];
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}
export interface OriginalAudio { readonly lectureId: string; readonly bytes: Uint8Array; readonly digest: string; readonly mediaType: "audio/wav" }
export const MAX_AUDIO_BYTES = 4 * 1024 * 1024;
export interface DetailSelector { readonly view: "detail_workspace"; readonly known_at_accept_seq: number | null; readonly valid_at_ms: number | null }
export interface DetailDecisionRequest {
  readonly relation_id: string; readonly action: "reject" | "undo"; readonly expected_revision: number;
  readonly expected_profile_id: string; readonly selector: DetailSelector;
  readonly request_id: readonly number[]; readonly client_instance_id: readonly number[]; readonly idempotency_key: readonly number[];
}
interface DecisionReceipt {
  readonly sequence: number; readonly relation_id: string; readonly relation_claim_id: string;
  readonly action: "reject" | "undo"; readonly undoes: number | null; readonly actor: string;
}
const UUID_V7 = /^[a-f0-9]{8}-[a-f0-9]{4}-7[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/u;
class DecisionRefused extends Error {}
/** Exact Rust DetailDecisionRequest struct field order; the command wrapper is excluded. */
export async function detailDecisionDigest(request: DetailDecisionRequest): Promise<readonly number[]> {
  const bytes = new TextEncoder().encode(`academic.details-decision.v1\0${JSON.stringify({
    relation_id: request.relation_id, action: request.action, expected_revision: request.expected_revision,
    expected_profile_id: request.expected_profile_id,
    selector: { view: request.selector.view, known_at_accept_seq: request.selector.known_at_accept_seq, valid_at_ms: request.selector.valid_at_ms },
    request_id: request.request_id, client_instance_id: request.client_instance_id, idempotency_key: request.idempotency_key,
  })}`);
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))];
}
const CURRENT_SELECTOR: DetailSelector = Object.freeze({ view: "detail_workspace", known_at_accept_seq: null, valid_at_ms: null });
function immutable<T>(value: T): T {
  if (typeof value === "object" && value !== null) { for (const child of Object.values(value)) immutable(child); Object.freeze(value); }
  return value;
}
function selectorCopy(value: unknown): DetailSelector {
  shape(value, { view: "string" });
  const selector = value as { readonly view: string; readonly known_at_accept_seq: number | null; readonly valid_at_ms: number | null };
  if (selector.view !== "detail_workspace" || Object.keys(selector).length !== 3 || [selector.known_at_accept_seq, selector.valid_at_ms].some((coordinate) => coordinate !== null && (!Number.isSafeInteger(coordinate) || coordinate < 0))) throw new Error("Invalid detail selector");
  return Object.freeze({ ...selector, view: "detail_workspace" });
}
export function relationRejected(state: DetailState, relationId: string): boolean {
  return state.decisions.filter((event) => event.relationId === relationId).at(-1)?.action === "REJECT";
}
type Shape = "string" | "number" | "boolean" | null | readonly [Shape] | { readonly [key: string]: Shape };
const target = { routeId: "string", id: "string" } as const;
const source = { id: "string", title: "string", locator: "string", content: "string" } as const;
const relation = { id: "string", label: "string", source, status: "string", confidence: "string" } as const;
/** Validate every accessed field before a native value can reach a renderer. */
function shape(value: unknown, expected: Shape): void {
  if (expected === null) { if (value !== null) throw new Error("Expected null"); return; }
  if (typeof expected === "string") {
    if (typeof value !== expected || (typeof value === "number" && (!Number.isSafeInteger(value) || value < 0))) throw new Error("Invalid detail field");
    return;
  }
  if (Array.isArray(expected)) {
    if (!Array.isArray(value) || value.length > 4096) throw new Error("Invalid detail list");
    for (const item of value) shape(item, expected[0] as Shape);
    return;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("Invalid detail object");
  for (const [key, field] of Object.entries(expected)) {
    if (!(key in value)) throw new Error(`Missing detail field ${key}`);
    shape(Reflect.get(value, key), field);
  }
}
function relationGroups(value: unknown): void {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("Invalid relation groups");
  for (const relations of Object.values(value)) shape(relations, [relation]);
}
function closed(value: object, required: readonly string[], optional: readonly string[] = []): void {
  if (required.some((key) => !Object.hasOwn(value, key)) || Object.keys(value).some((key) => !required.includes(key) && !optional.includes(key))) throw new Error("Unsupported detail fields");
}
function wireBounds(value: unknown, depth = 0): void {
  if (depth > 24) throw new Error("Detail nesting limit exceeded");
  if (typeof value === "string" && new TextEncoder().encode(value).length > 65536) throw new Error("Detail string limit exceeded");
  if (Array.isArray(value)) { if (value.length > 4096) throw new Error("Detail list limit exceeded"); for (const item of value) wireBounds(item, depth + 1); }
  else if (typeof value === "object" && value !== null) { if (Object.keys(value).length > 128) throw new Error("Detail field limit exceeded"); for (const item of Object.values(value)) wireBounds(item, depth + 1); }
  else if (typeof value === "number" && (!Number.isSafeInteger(value) || value < 0)) throw new Error("Invalid detail number");
  if (depth === 0 && new TextEncoder().encode(JSON.stringify(value)).length > 1048576) throw new Error("Detail frame limit exceeded");
}
function decodeDecisionReceipt(value: unknown): DecisionReceipt {
  shape(value, { sequence: "number", relation_id: "string", relation_claim_id: "string", action: "string", actor: "string" });
  const receipt = value as DecisionReceipt;
  if (!['reject', 'undo'].includes(receipt.action) || Object.keys(receipt).length !== 6 || !UUID_V7.test(receipt.relation_claim_id) || !UUID_V7.test(receipt.actor) || receipt.sequence < 1
    || (receipt.action === "reject" ? receipt.undoes !== null : !Number.isSafeInteger(receipt.undoes) || receipt.undoes === null || receipt.undoes < 1 || receipt.undoes >= receipt.sequence)) throw new Error("Invalid original decision receipt");
  return receipt;
}
function refuseReadReceipt(value: unknown): void {
  if (typeof value === "object" && value !== null && ["receipt_id", "decision_sequence", "receipt_decision"].some((key) => Reflect.get(value, key) != null)) throw new Error("Read or nonaccepted reply carries a decision receipt");
}
export function decodeDetailState(value: unknown): DetailState {
  wireBounds(value);
  shape(value, { revision: "number", profile_id: "string", known_at_accept_seq: "number", valid_at_ms: "number", projector_version: "string", source_digest: "string", decisions: [{ sequence: "number", relationId: "string", action: "string", actor: "string" }], corpus: {
    lectures: [{ id: "string", title: "string", explanation: "string", segments: [{ id: "string", startMs: "number", endMs: "number", raw: "string", corrected: "string" }], paragraphs: [{ id: "string", text: "string", segmentIds: ["string"] }], captures: [{ id: "string", title: "string", text: "string", atMs: "number", segmentId: "string" }], review: [{ id: "string", kind: "string", segmentId: "string", note: "string" }] }],
    concepts: [{ id: "string", title: "string", state: "string", confidence: "string", freshness: "string", lastStrongEvidence: "string", explanation: "string" }],
    questions: [{ id: "string", origin: "string", isNew: "boolean", status: "string", goalRelevance: "string", goalRank: "number", nextContext: "string", ageDays: "number", explanation: "string", evidence: [relation], revisions: [{ at: "string", text: "string", concepts: [relation], resolutionEvidence: [relation] }] }],
    projects: [{ id: "string", title: "string", goal: "string", successCriteria: ["string"], repository: "string", branch: "string", snapshot: "string", currentSnapshot: "string", capturedAt: "string", currentAt: "string", dirty: "boolean", explanation: "string", files: [{ path: "string", text: "string" }] }],
  } });
  // The structural check above precedes this narrowing; remaining unions and cross-references follow.
  const state = value as DetailState;
  closed(state, ["revision", "profile_id", "known_at_accept_seq", "valid_at_ms", "projector_version", "source_digest", "decisions", "corpus"]);
  closed(state.corpus, ["lectures", "concepts", "questions", "projects"]);
  if (!state.profile_id.trim() || !["academic.details.v1", "academic.details.v2"].includes(state.projector_version) || !/^[a-f0-9]{64}$/u.test(state.source_digest)) throw new Error("Invalid or unsupported detail source identity");
  for (const entries of [state.corpus.lectures, state.corpus.concepts, state.corpus.questions, state.corpus.projects]) if (new Set(entries.map((item) => item.id)).size !== entries.length || entries.some((item) => !item.id.trim())) throw new Error("Invalid or duplicate detail identity");
  for (const lecture of state.corpus.lectures) {
    closed(lecture, ["id", "title", "explanation", "segments", "paragraphs", "captures", "review", "links"]);
    relationGroups(lecture.links);
    for (const segment of lecture.segments) {
      closed(segment, ["id", "startMs", "endMs", "raw", "corrected", "disposition", "reason"]);
      if (segment.disposition !== null && !["UNMAPPED", "EXCLUDED_NON_SPEECH", "REDACTED_WITH_POLICY", "UNTRANSCRIBED_FAILURE"].includes(segment.disposition)) throw new Error("Unknown segment disposition");
      if (segment.reason !== null && typeof segment.reason !== "string") throw new Error("Invalid disposition evidence");
      if (segment.endMs <= segment.startMs) throw new Error("Invalid audio interval");
    }
    coverage(lecture);
    for (const paragraph of lecture.paragraphs) closed(paragraph, ["id", "text", "segmentIds"]);
    for (const capture of lecture.captures) closed(capture, ["id", "title", "text", "atMs", "segmentId"]);
    for (const review of lecture.review) closed(review, ["id", "kind", "segmentId", "note"]);
    for (const records of [lecture.paragraphs, lecture.captures, lecture.review]) if (new Set(records.map((item) => item.id)).size !== records.length) throw new Error("Duplicate lecture source identity");
    for (const capture of lecture.captures) selectCapture(lecture, capture.id);
    for (const review of lecture.review) if (!["Mark Moment", "Low confidence", "Equation", "Code"].includes(review.kind) || !lecture.segments.some((segment) => segment.id === review.segmentId)) throw new Error("Unresolved review segment or kind");
  }
  for (const concept of state.corpus.concepts) { closed(concept, ["id", "title", "state", "confidence", "freshness", "lastStrongEvidence", "explanation", "relations"]); relationGroups(concept.relations); }
  for (const project of state.corpus.projects) { closed(project, ["id", "title", "goal", "successCriteria", "repository", "branch", "snapshot", "currentSnapshot", "capturedAt", "currentAt", "dirty", "explanation", "files", "relations"]); relationGroups(project.relations); for (const file of project.files) closed(file, ["path", "text"]); }
  for (const question of state.corpus.questions) {
    closed(question, ["id", "origin", "isNew", "status", "goalRelevance", "goalRank", "nextContext", "ageDays", "explanation", "evidence", "revisions"]);
    if (!question.revisions.length || !["OPEN", "PARTIAL", "RESOLVED", "REFRAMED"].includes(question.status)) throw new Error("Invalid question history");
    for (const revision of question.revisions) closed(revision, ["at", "text", "concepts", "resolutionEvidence"]);
  }
  const relations = allRelations(state.corpus);
  for (const item of relations) {
    closed(item, ["id", "label", "source", "status", "confidence"], ["target"]); closed(item.source, ["id", "title", "locator", "content"], ["href"]);
    if (!["PROPOSED", "CONFIRMED", "CONTESTED"].includes(item.status)) throw new Error("Invalid relation status");
    if (item.target !== undefined) { shape(item.target, target); closed(item.target, ["routeId", "id"]); }
    if (item.source.href !== undefined && (typeof item.source.href !== "string" || !item.source.href.startsWith("#"))) throw new Error("Nonlocal source link");
  }
  const rejected = new Map<string, number>();
  let sequence = 0;
  for (const event of state.decisions) {
    closed(event, ["sequence", "relationId", "action", "actor", "undoes"]);
    if (event.sequence <= sequence || event.sequence > state.known_at_accept_seq || !UUID_V7.test(event.actor) || !relations.some((item) => item.id === event.relationId)) throw new Error("Invalid decision ordering, source watermark or relation");
    if (event.action === "REJECT" && event.undoes === null && !rejected.has(event.relationId)) rejected.set(event.relationId, event.sequence);
    else if (event.action === "UNDO" && rejected.has(event.relationId) && event.undoes === rejected.get(event.relationId)) rejected.delete(event.relationId);
    else throw new Error("Invalid append-only decision history");
    sequence = event.sequence;
  }
  return immutable(structuredClone(state));
}
export function detailClient(invoke: NativeInvoke, storage?: DetailRetryStorage): DetailClient {
  const bytes = (length: number): readonly number[] => [...crypto.getRandomValues(new Uint8Array(length))];
  const clientId = bytes(16);
  let current: DetailState | null = null;
  let selected = CURRENT_SELECTOR;
  let selectionGeneration = 0;
  let selectionReady = false;
  const pending = new Map<string, { readonly request: DetailDecisionRequest; readonly undoes: number | null }>();
  const active = new Map<string, Promise<DetailState>>();
  const keyOf = (request: DetailDecisionRequest): string => JSON.stringify([request.expected_profile_id, request.relation_id, request.action, request.expected_revision]);
  const journalPrefix = "academic.imported-detail-retry.v1:";
  const storageKey = (request: DetailDecisionRequest): string => journalPrefix + request.request_id.map((byte) => byte.toString(16).padStart(2, "0")).join("");
  function loadRetries(): void {
    if (!storage) return;
    const keys = storage.keys().filter((key) => key.startsWith(journalPrefix));
    if (keys.length > 64) throw new Error("Invalid saved detail retries");
    const loaded = new Map<string, { readonly request: DetailDecisionRequest; readonly undoes: number | null }>();
    for (const key of keys) {
      const stored = storage.getItem(key); if (stored === null) continue;
      if (stored.length > 65536) throw new Error("Saved detail retry exceeds its limit");
      const record: unknown = JSON.parse(stored);
      shape(record, { request: { relation_id: "string", action: "string", expected_revision: "number", expected_profile_id: "string", request_id: ["number"], client_instance_id: ["number"], idempotency_key: ["number"] } });
      const saved = record as { readonly request: DetailDecisionRequest; readonly undoes: number | null };
      const request = saved.request;
      selectorCopy(request.selector);
      if (Object.keys(saved).length !== 2 || Object.keys(request).length !== 8 || !request.relation_id.trim() || !request.expected_profile_id.trim() || !["reject", "undo"].includes(request.action)
        || request.selector.known_at_accept_seq !== null || request.selector.valid_at_ms !== null
        || [request.request_id, request.client_instance_id, request.idempotency_key].some((value, index) => value.length !== (index === 2 ? 32 : 16) || value.some((byte) => byte > 255))
        || (request.action === "reject" ? saved.undoes !== null : saved.undoes === null || !Number.isSafeInteger(saved.undoes) || saved.undoes < 1)) throw new Error("Invalid saved detail retry identity");
      if (key !== storageKey(request) || loaded.has(keyOf(request))) throw new Error("Duplicate or mismatched saved detail retry");
      loaded.set(keyOf(request), immutable(saved));
    }
    pending.clear(); for (const [key, saved] of loaded) pending.set(key, saved);
  }
  async function request(operation: object, accepted: boolean, identity?: { readonly request_id: readonly number[]; readonly client_instance_id: readonly number[]; readonly idempotency_key: readonly number[]; readonly request_digest: readonly number[] }): Promise<{ readonly state: DetailState; readonly receipt: DecisionReceipt | null }> {
    const reply = await invoke("desktop_request_v1", { request: { version: 1, operation } });
    validateReply(reply);
    if (!accepted || (typeof reply === "object" && reply !== null && Reflect.get(reply, "state") !== "accepted")) refuseReadReceipt(reply);
    if (identity && typeof reply === "object" && reply !== null && Reflect.get(reply, "version") === 1 && Reflect.get(reply, "state") === "rejected"
      && Reflect.get(reply, "audio") == null && Reflect.get(reply, "details") == null
      && ["REVISION_CONFLICT", "CLOCK_BEFORE_HISTORY", "RELATION_NOT_FOUND", "ALREADY_REJECTED", "NOTHING_TO_UNDO", "HISTORY_BUDGET_EXCEEDED"].includes(String(Reflect.get(reply, "reason")))
      && Object.entries(identity).every(([key, expected]) => { const actual: unknown = Reflect.get(reply, key); return Array.isArray(actual) && actual.length === expected.length && actual.every((byte: unknown, index) => byte === expected[index]); })) throw new DecisionRefused("The core refused this request without recording a decision. Reload the current evidence before deciding again.");
    if (typeof reply !== "object" || reply === null || !("version" in reply) || reply.version !== 1 || !("state" in reply) || reply.state !== (accepted ? "accepted" : "ready") || !("details" in reply)) {
      throw new Error("The local service could not confirm the detail request. Check the connection and reload.");
    }
    if (Reflect.get(reply, "audio") != null) throw new Error("Detail reply carries an unrelated audio result");
    if (!accepted && ["request_id", "client_instance_id", "idempotency_key", "request_digest"].some((key) => Reflect.get(reply, key) != null)) throw new Error("Read reply carries write identity");
    if (accepted && (!("receipt_id" in reply) || !Array.isArray(reply.receipt_id) || reply.receipt_id.length !== 16 || !reply.receipt_id.every((byte: unknown) => typeof byte === "number" && Number.isInteger(byte) && byte >= 0 && byte <= 255))) throw new Error("Missing core decision receipt");
    if (identity) for (const [key, expected] of Object.entries(identity)) {
      const actual: unknown = Reflect.get(reply, key);
      if (!Array.isArray(actual) || actual.length !== expected.length || actual.some((byte: unknown, index) => byte !== expected[index])) throw new Error("Core receipt belongs to another request");
    }
    const decisionSequence: unknown = Reflect.get(reply, "decision_sequence");
    if (accepted && (typeof decisionSequence !== "number" || !Number.isSafeInteger(decisionSequence) || decisionSequence < 1)) throw new Error("Missing receipt-bound decision sequence");
    const receipt = accepted ? decodeDecisionReceipt(Reflect.get(reply, "receipt_decision")) : null;
    const state = decodeDetailState(reply.details);
    if (receipt && (receipt.sequence !== decisionSequence || receipt.sequence > state.known_at_accept_seq)) throw new Error("Decision receipt does not match the source watermark");
    return { state, receipt };
  }
  return {
    pendingDecisions: () => Object.freeze([...pending.values()].map((saved) => saved.request).filter((request) => request.expected_profile_id === current?.profile_id)),
    read: async (selector = CURRENT_SELECTOR) => {
      loadRetries();
      const selection = selectorCopy(selector);
      const generation = ++selectionGeneration;
      selectionReady = false;
      const { state } = await request({ command: "details_read", selector: selection }, false);
      if (generation !== selectionGeneration) throw new Error("Detail read was superseded by a newer selection");
      if (current && state.profile_id !== current.profile_id) throw new Error("Profile identity changed unexpectedly");
      if (selection.known_at_accept_seq !== null && state.known_at_accept_seq !== selection.known_at_accept_seq) throw new Error("Read did not match selected known time");
      if (selection.valid_at_ms !== null && state.valid_at_ms !== selection.valid_at_ms) throw new Error("Read did not match selected valid time");
      if (current && selection.known_at_accept_seq === null && selected.known_at_accept_seq === null && (state.revision < current.revision || state.known_at_accept_seq < current.known_at_accept_seq)) throw new Error("Read watermark moved backwards");
      current = state; selected = selection; selectionReady = true; return state;
    },
    decide: async (relationId, action, expectedRevision) => {
      if (!current || !selectionReady) throw new Error("Reload the selected profile before deciding");
      if (selected.known_at_accept_seq !== null || selected.valid_at_ms !== null) throw new Error("Historical views are read-only");
      const profileId = current.profile_id;
      const generation = selectionGeneration;
      const key = JSON.stringify([profileId, relationId, action, expectedRevision]);
      const inFlight = active.get(key); if (inFlight) return inFlight;
      loadRetries();
      let saved = pending.get(key);
      if (!saved) {
        if ([...pending.values()].some((item) => item.request.expected_profile_id === profileId)) throw new Error("Confirm the original pending request before starting another decision");
        if (current.revision !== expectedRevision) throw new Error("Reload the selected profile before deciding");
        if (!allRelations(current.corpus).some((relation) => relation.id === relationId)) throw new Error("Relation is absent from the selected profile");
        const prior = current.decisions.filter((event) => event.relationId === relationId).at(-1);
        if (action === "undo" && prior?.action !== "REJECT") throw new Error("No accepted rejection is available to undo");
        if (action === "reject" && prior?.action === "REJECT") throw new Error("This relation already has an accepted rejection");
        if (pending.size >= 64) throw new Error("Saved detail retry limit reached");
        saved = immutable({ request: { relation_id: relationId, action, expected_revision: expectedRevision, expected_profile_id: profileId, selector: { ...selected }, request_id: bytes(16), client_instance_id: clientId, idempotency_key: bytes(32) }, undoes: action === "undo" ? prior?.sequence ?? null : null });
        pending.set(key, saved);
      }
      // Write-ahead identity: failure here must prevent the native write.
      storage?.setItem(storageKey(saved.request), JSON.stringify(saved));
      const decision = saved.request;
      const undoTarget = saved.undoes;
      const completion = (async (): Promise<DetailState> => {
        const identity = { request_id: decision.request_id, client_instance_id: decision.client_instance_id, idempotency_key: decision.idempotency_key, request_digest: await detailDecisionDigest(decision) };
        const { state, receipt } = await request({ command: "details_decide", ...decision }, true, identity);
        if (state.profile_id !== profileId || state.revision <= expectedRevision || receipt?.relation_id !== relationId || receipt.action !== action || receipt.undoes !== undoTarget) throw new Error("Decision receipt does not match the requested change");
        const visible = state.decisions.find((event) => event.sequence === receipt.sequence);
        if (visible && (visible.relationId !== receipt.relation_id || visible.action !== receipt.action.toUpperCase() || visible.undoes !== receipt.undoes || visible.actor !== receipt.actor)) throw new Error("Visible decision disagrees with its original receipt");
        // Receipt confirmation is independent of current source visibility and selection.
        storage?.removeItem(storageKey(decision)); pending.delete(key);
        if (generation === selectionGeneration && state.revision >= current.revision && state.known_at_accept_seq >= current.known_at_accept_seq) current = state;
        return current;
      })();
      active.set(key, completion);
      try { return await completion; }
      catch (error) { if (error instanceof DecisionRefused) { storage?.removeItem(storageKey(decision)); pending.delete(key); } throw error; }
      finally { active.delete(key); }
    },
    audio: async (lectureId, signal) => {
      if (!current || !selectionReady || !current.corpus.lectures.some((lecture) => lecture.id === lectureId)) throw new Error("Lecture is absent from the selected profile");
      const profile = current; const selector = selected; const generation = selectionGeneration;
      const checkLifetime = (): void => { signal?.throwIfAborted(); if (generation !== selectionGeneration || current !== profile) throw new Error("The selected audio snapshot changed. Reload detail evidence."); };
      let output: Uint8Array | null = null; let digest = ""; let offset = 0;
      do {
        checkLifetime();
        const reply = await invoke("desktop_request_v1", { request: { version: 1, operation: { command: "details_audio", lecture_id: lectureId, expected_profile_id: profile.profile_id, expected_revision: profile.revision, selector, offset, length: 4096 } } });
        checkLifetime();
        validateReply(reply);
        refuseReadReceipt(reply);
        if (typeof reply === "object" && reply !== null && ["details", "request_id", "client_instance_id", "idempotency_key", "request_digest"].some((key) => Reflect.get(reply, key) != null)) throw new Error("Audio reply carries unrelated detail or write identity");
        if (typeof reply !== "object" || reply === null || !("version" in reply) || reply.version !== 1 || !("state" in reply) || reply.state !== "ready" || !("audio" in reply)) {
          const reason: unknown = typeof reply === "object" && reply !== null ? Reflect.get(reply, "reason") : null;
          throw new Error(reason === "AUDIO_FORMAT_OR_SIZE_UNSUPPORTED" ? "Original audio exceeds the current 4 MiB WAV limit. Full-length streaming remains unavailable."
            : reason === "REVISION_CONFLICT" || reason === "PROFILE_MISMATCH" ? "The selected audio snapshot changed. Reload detail evidence."
              : "Original audio is unavailable in this profile. Raw segments and timestamps remain visible.");
        }
        const audio: unknown = reply.audio;
        shape(audio, { lecture_id: "string", media_type: "string", content_digest: "string", total_bytes: "number", offset: "number", bytes: ["number"] });
        const chunk = audio as { lecture_id: string; media_type: string; content_digest: string; total_bytes: number; offset: number; bytes: number[] };
        closed(chunk, ["lecture_id", "media_type", "content_digest", "total_bytes", "offset", "bytes"]);
        if (chunk.lecture_id !== lectureId || chunk.media_type !== "audio/wav" || chunk.offset !== offset || chunk.total_bytes < 1 || chunk.total_bytes > MAX_AUDIO_BYTES || !/^[a-f0-9]{64}$/u.test(chunk.content_digest) || chunk.bytes.length !== Math.min(4096, chunk.total_bytes - offset) || chunk.bytes.some((byte) => byte > 255)) throw new Error("Original audio response does not match the selected source or byte range");
        if (output === null) { output = new Uint8Array(chunk.total_bytes); digest = chunk.content_digest; }
        if (output.length !== chunk.total_bytes || digest !== chunk.content_digest) throw new Error("Original audio identity changed between chunks");
        output.set(chunk.bytes, offset); offset += chunk.bytes.length;
      } while (offset < output.length);
      const actual = [...new Uint8Array(await crypto.subtle.digest("SHA-256", Uint8Array.from(output)))].map((byte) => byte.toString(16).padStart(2, "0")).join("");
      checkLifetime();
      if (actual !== digest) throw new Error("Original audio digest verification failed");
      return { lectureId, bytes: output, digest, mediaType: "audio/wav" };
    },
  };
}
function validateReply(value: unknown): void {
  wireBounds(value);
  shape(value, { version: "number", state: "string" });
  closed(value as object, ["version", "state"], ["message", "reason", "details", "audio", "receipt_id", "decision_sequence", "receipt_decision", "request_id", "client_instance_id", "idempotency_key", "request_digest"]);
}
