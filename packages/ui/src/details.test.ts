import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import test from "node:test";
import { buildDetailFixture } from "./detail-fixture.js";
import { allRelations, coverage, detailSections, previewSourceBytes, questionGroups, selectCapture, selectParagraph, staleSnapshot } from "./details.js";
import { decodeDetailState, detailClient, detailDecisionDigest, relationRejected, type DetailState } from "./detail-client.js";
import { profileBacklinks, profileEvidence, profilePalette, profileTitle } from "./detail-navigation.js";
import { renderDrawer } from "./drawer.js";

function first<T>(values: readonly T[]): T { const value = values[0]; assert.ok(value); return value; }
const RECEIPT_ACTOR = "00000000-0000-7000-8000-000000000001";
const RECEIPT_CLAIM = "00000000-0000-7000-8000-000000000002";
function fixtureState(): DetailState {
  return { corpus: buildDetailFixture(), decisions: [], revision: 1, profile_id: "synthetic-profile-a", known_at_accept_seq: 100, valid_at_ms: 1788830000000, projector_version: "academic.details.v2", source_digest: "1".repeat(64) };
}
function operation(args: unknown): Record<string, unknown> {
  assert.ok(typeof args === "object" && args !== null && "request" in args);
  const request = args.request; assert.ok(typeof request === "object" && request !== null && "operation" in request);
  assert.ok(typeof request.operation === "object" && request.operation !== null);
  return request.operation as Record<string, unknown>;
}
function appendReject(state: DetailState): DetailState {
  return { ...state, revision: 2, known_at_accept_seq: 101, decisions: [{ sequence: 101, relationId: first(allRelations(state.corpus)).id, action: "REJECT", undoes: null, actor: RECEIPT_ACTOR }] };
}
function accepted(state: DetailState, op: Record<string, unknown>, receipt: Record<string, unknown> = {}) {
  // Independent Node digest over the Rust struct's field order, not the client helper.
  const wire = { relation_id: op.relation_id, action: op.action, expected_revision: op.expected_revision, expected_profile_id: op.expected_profile_id, selector: op.selector, request_id: op.request_id, client_instance_id: op.client_instance_id, idempotency_key: op.idempotency_key };
  return { version: 1, state: "accepted", details: state, receipt_id: Array<number>(16).fill(7), decision_sequence: receipt.sequence ?? 101,
    receipt_decision: { sequence: 101, relation_id: op.relation_id, relation_claim_id: RECEIPT_CLAIM, action: op.action, undoes: null, actor: RECEIPT_ACTOR, ...receipt },
    request_id: op.request_id, client_instance_id: op.client_instance_id, idempotency_key: op.idempotency_key,
    request_digest: [...createHash("sha256").update("academic.details-decision.v1\0").update(JSON.stringify(wire)).digest()] };
}

await test("source_identity_and_decision_known_time_are_required_before_display", () => {
  const state = fixtureState();
  for (const patch of [{ profile_id: "" }, { projector_version: " " }, { source_digest: "missing" }]) assert.throws(() => decodeDetailState({ ...state, ...patch }), /source identity/u);
  assert.throws(() => decodeDetailState({ ...appendReject(state), known_at_accept_seq: state.known_at_accept_seq }), /source watermark/u);
  assert.equal(decodeDetailState(appendReject(state)).decisions[0]?.sequence, 101);
});

await test("lecture_full_document_is_default", () => {
  assert.equal(first(detailSections("learn.lectures", true)).heading, "Full preserved document");
  const lecture = first(buildDetailFixture().lectures);
  for (const paragraph of lecture.paragraphs) assert.ok(paragraph.text.length > 0 && paragraph.segmentIds.length > 0);
});
await test("paragraph_opens_audio_timestamp_and_raw_segment", () => {
  const lecture = first(buildDetailFixture().lectures);
  const paragraph = lecture.paragraphs[1]; assert.ok(paragraph);
  const selected = first(selectParagraph(lecture, paragraph.id));
  assert.equal(selected.segment.startMs, 2000);
  assert.equal(selected.segment.raw, "Path compression shortens parent chains. The bound uses alpha n.");
  assert.deepEqual(selected.paragraphIds, [paragraph.id]);
  assert.throws(() => selectParagraph(lecture, "missing"), /unavailable/u);
});
await test("capture_alignment_round_trip", () => {
  const lecture = first(buildDetailFixture().lectures);
  const selected = selectCapture(lecture, "capture-1");
  assert.deepEqual(selected.captureIds, ["capture-1"]);
  assert.deepEqual(selected.paragraphIds, ["p2"]);
  assert.equal(selected.segment.id, "s2");
  assert.throws(() => selectCapture({ ...lecture, captures: [{ ...first(lecture.captures), atMs: 4000 }] }, "capture-1"), /outside/u);
});
await test("coverage_report_partitions_every_segment", () => {
  const lecture = first(buildDetailFixture().lectures);
  assert.deepEqual(coverage(lecture).map((row) => [row.segment.id, row.status]), [["s1", "MAPPED"], ["s2", "MAPPED"], ["s3", "UNMAPPED"], ["s4", "REDACTED_WITH_POLICY"], ["s5", "UNTRANSCRIBED_FAILURE"], ["s6", "EXCLUDED_NON_SPEECH"]]);
  assert.throws(() => coverage({ ...lecture, paragraphs: [...lecture.paragraphs, { id: "bad", text: "bad", segmentIds: ["s4"] }] }), /both/u);
  assert.throws(() => coverage({ ...lecture, paragraphs: [{ id: "bad", text: "bad", segmentIds: ["absent"] }] }), /resolvable/u);
  assert.throws(() => coverage({ ...lecture, paragraphs: lecture.paragraphs.map((paragraph) => ({ ...paragraph, text: "Short summary" })) }), /omits corrected transcript/u);
  assert.throws(() => coverage({ ...lecture, segments: lecture.segments.map((segment) => segment.id === "s5" ? { ...segment, reason: null } : segment) }), /lacks evidence/u);
});
await test("concept_detail_exposes_every_named_field", () => {
  const concept = first(buildDetailFixture().concepts);
  assert.deepEqual(Object.keys(concept.relations), ["Evidence timeline", "Contradictions", "Prerequisites", "Used in", "Open questions", "SNU courses, lectures and assessments", "Projects", "Competencies", "Roles"]);
  assert.ok(concept.state && concept.confidence && concept.freshness && concept.lastStrongEvidence);
});
await test("relation_opens_source_status_confidence", () => {
  const corpus = buildDetailFixture();
  for (const relation of allRelations(corpus)) assert.ok(relation.source.content && relation.source.locator && relation.confidence, relation.id);
  const lecture = first(corpus.lectures); const original = first(allRelations(corpus));
  const conflict = { ...lecture, links: { ...lecture.links, Conflict: [{ ...original, source: { ...original.source, content: "Different source" } }] } };
  assert.throws(() => allRelations({ ...corpus, lectures: [conflict] }), /Conflicting/u);
});
await test("relation_reject_is_append_only_with_undo", () => {
  const rejected = appendReject(fixtureState()); const relationId = first(rejected.decisions).relationId;
  const undone: DetailState = { ...rejected, revision: 3, known_at_accept_seq: 102, decisions: [...rejected.decisions, { sequence: 102, relationId, action: "UNDO", undoes: 101, actor: RECEIPT_ACTOR }] };
  assert.equal(relationRejected(decodeDetailState(rejected), relationId), true);
  assert.equal(relationRejected(decodeDetailState(undone), relationId), false);
  assert.deepEqual(undone.decisions.slice(0, 1), rejected.decisions);
  assert.deepEqual(undone.corpus, fixtureState().corpus);
  assert.throws(() => decodeDetailState({ ...undone, decisions: [{ ...first(undone.decisions), action: "UNDO", undoes: 99 }] }), /Invalid append-only/u);
  assert.throws(() => decodeDetailState({ ...undone, decisions: [{ ...first(undone.decisions), action: "UNDO", undoes: undefined }] }), /Invalid append-only/u);
});
await test("project_analyze_is_labelled_read_only", () => {
  const project = first(buildDetailFixture().projects);
  assert.ok(detailSections("build.projects", true).some((section) => section.heading === "Analyze · read-only"));
  assert.deepEqual(["OBSERVED", "REQUIRED", "WOULD_BENEFIT_FROM"].map((kind) => project.relations[kind]?.length), [1, 1, 1]);
});
await test("source_byte_preview_preserves_exact_utf8_ranges", () => {
  const project = first(buildDetailFixture().projects); const file = first(project.files);
  const ranges = [{ path: file.path, start: 3, end: 9 }, { path: file.path, start: 10, end: 19 }];
  const preview = previewSourceBytes(project, ranges);
  const expected = Buffer.concat([Buffer.from(file.text).subarray(3, 9), Buffer.from(file.text).subarray(10, 19)]);
  assert.equal(Buffer.from(preview.bytes).toString("hex"), expected.toString("hex"));
  ranges[0] = { path: "changed", start: 0, end: 1 };
  assert.deepEqual(Buffer.from(preview.bytes), expected);
  assert.throws(() => previewSourceBytes(project, [{ path: file.path, start: 0, end: 9999 }]), /outside/u);
});
await test("stale_snapshot_banner_is_persistent", () => {
  const project = first(buildDetailFixture().projects); const banner = staleSnapshot(project);
  assert.ok(banner?.includes(project.snapshot) && banner.includes(project.capturedAt) && banner.includes(project.currentSnapshot));
  assert.equal(staleSnapshot({ ...project, currentSnapshot: project.snapshot }), null);
});
await test("question_workspace_places_goal_and_upcoming_context_before_age_and_groups_origins", () => {
  const questions = buildDetailFixture().questions;
  const groups = questionGroups(questions);
  assert.deepEqual([...groups.keys()], ["Inbox · Concept detail", "Inbox · Lecture", "Inbox · Repository", "OPEN", "PARTIAL", "RESOLVED", "REFRAMED"]);
  const open = first(questions); const priority = { ...open, id: "priority", goalRank: open.goalRank + 1, ageDays: 500 };
  assert.equal(first(questionGroups([open, priority]).get("OPEN") ?? []).id, "priority");
});
await test("detail_native_client_retains_identity_after_lost_ack_and_matches_receipt", async () => {
  const initial = fixtureState(); const committed = appendReject(initial); const writes: Record<string, unknown>[] = [];
  const client = detailClient((_command, args) => {
    const op = operation(args);
    if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
    writes.push(op);
    if (writes.length === 1) return Promise.reject(new Error("ACK lost after commit"));
    return Promise.resolve(accepted(committed, op));
  });
  await client.read(); const relationId = first(committed.decisions).relationId;
  await assert.rejects(client.decide(relationId, "reject", 1), /ACK lost/u);
  assert.deepEqual(await client.decide(relationId, "reject", 1), committed);
  assert.deepEqual(writes[0], writes[1]);
  assert.equal(first(writes).expected_profile_id, initial.profile_id);
});
await test("detail_native_client_does_not_publish_rejection_mismatched_identity_or_other_profile", async () => {
  for (const kind of ["rejection", "identity", "profile"]) {
    const initial = fixtureState(); const committed = appendReject(initial);
    const client = detailClient((_command, args) => {
      const op = operation(args);
      if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
      const reply = kind === "rejection" ? { version: 1, state: "rejected", reason: "REVISION_CONFLICT" }
        : kind === "identity" ? { ...accepted(committed, op), request_id: Array<number>(16).fill(255) }
          : accepted({ ...committed, profile_id: "other-profile" }, op);
      return Promise.resolve(reply);
    });
    await client.read(); await assert.rejects(client.decide(first(committed.decisions).relationId, "reject", 1));
  }
});
await test("duplicate_acknowledgement_matches_original_event_in_a_newer_history", async () => {
  const initial = fixtureState(); const rejected = appendReject(initial); const original = first(rejected.decisions);
  const newer: DetailState = { ...rejected, revision: 3, known_at_accept_seq: 102, decisions: [...rejected.decisions, { sequence: 102, relationId: original.relationId, action: "UNDO", undoes: 101, actor: RECEIPT_ACTOR }] };
  const client = detailClient((_command, args) => { const op = operation(args); return Promise.resolve(op.command === "details_read" ? { version: 1, state: "ready", details: initial } : accepted(newer, op)); });
  await client.read();
  const result = await client.decide(original.relationId, "reject", 1);
  assert.equal(relationRejected(result, original.relationId), false);
  assert.deepEqual(result.decisions, newer.decisions);
});
await test("lost_ack_receipt_survives_removed_or_replaced_source_without_rejecting_the_replacement", async () => {
  for (const mode of ["removed", "replaced"]) {
    const initial = fixtureState(); const original = first(allRelations(initial.corpus));
    const newer: DetailState = { ...initial, revision: 3, known_at_accept_seq: 102, decisions: [], corpus: mode === "removed" ? { lectures: [], concepts: [], questions: [], projects: [] } : {
      ...initial.corpus,
      lectures: initial.corpus.lectures.map((lecture) => ({ ...lecture, links: Object.fromEntries(Object.entries(lecture.links).map(([heading, relations]) => [heading, relations.map((relation) => relation.id === original.id ? { ...relation, label: "Replacement source claim", source: { ...relation.source, content: "Different imported evidence" } } : relation)])) })),
    } };
    let reads = 0; const writes: Record<string, unknown>[] = [];
    const client = detailClient((_command, args) => {
      const op = operation(args);
      if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: reads++ === 0 ? initial : newer });
      writes.push(op);
      return writes.length === 1 ? Promise.reject(new Error("ACK lost after durable acceptance")) : Promise.resolve(accepted(newer, op));
    });
    await client.read(); await assert.rejects(client.decide(original.id, "reject", 1), /ACK lost/u);
    await client.read();
    const confirmed = await client.decide(original.id, "reject", 1);
    assert.deepEqual(writes[0], writes[1]); assert.deepEqual(confirmed, newer);
    assert.equal(relationRejected(confirmed, original.id), false);
    assert.equal(confirmed.decisions.length, 0);
    if (mode === "replaced") assert.equal(allRelations(confirmed.corpus).find((relation) => relation.id === original.id)?.label, "Replacement source claim");
  }
});
await test("receipt_requires_canonical_original_claim_user_action_sequence_and_complete_digest_identity", async () => {
  const initial = fixtureState(); const committed = appendReject(initial);
  for (const fault of ["missing", "sequence", "action", "relation", "claim", "actor", "undo", "client", "key", "digest", "extra", "rejected"]) {
    const client = detailClient((_command, args) => {
      const op = operation(args); if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
      const good = accepted(committed, op);
      const patch = fault === "sequence" ? { sequence: 102 } : fault === "action" ? { action: "undo", undoes: 99 } : fault === "relation" ? { relation_id: "another" }
        : fault === "claim" ? { relation_claim_id: "00000000-0000-4000-8000-000000000002" } : fault === "actor" ? { actor: "synthetic-user" }
          : fault === "undo" ? { undoes: 99 } : fault === "extra" ? { unexpected: true } : {};
      return Promise.resolve({ ...good, receipt_decision: fault === "missing" ? null : { ...good.receipt_decision, ...patch },
        ...(fault === "digest" ? { request_digest: Array<number>(32).fill(0) } : {}), ...(fault === "client" ? { client_instance_id: Array<number>(16).fill(99) } : {}), ...(fault === "key" ? { idempotency_key: Array<number>(32).fill(99) } : {}), ...(fault === "rejected" ? { state: "rejected" } : {}) });
    });
    await client.read(); await assert.rejects(client.decide(first(committed.decisions).relationId, "reject", 1), (error: unknown) => error instanceof Error, fault);
  }
});
await test("undo_retry_retains_its_original_rejection_target_after_source_removal", async () => {
  const initial = appendReject(fixtureState()); const relationId = first(initial.decisions).relationId;
  const newer = { ...initial, revision: 4, known_at_accept_seq: 103, corpus: { lectures: [], concepts: [], questions: [], projects: [] }, decisions: [] };
  for (const undoes of [100, 101]) {
    let reads = 0; let writes = 0;
    const client = detailClient((_command, args) => {
      const op = operation(args); if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: reads++ === 0 ? initial : newer });
      return ++writes === 1 ? Promise.reject(new Error("ACK lost")) : Promise.resolve(accepted(newer, op, { sequence: 102, undoes }));
    });
    await client.read(); await assert.rejects(client.decide(relationId, "undo", 2), /ACK lost/u); await client.read();
    if (undoes === 101) assert.deepEqual(await client.decide(relationId, "undo", 2), newer);
    else await assert.rejects(client.decide(relationId, "undo", 2), /requested change/u);
  }
});
await test("read_replies_cannot_carry_write_receipt_decisions", async () => {
  const initial = fixtureState();
  const client = detailClient(() => Promise.resolve({ version: 1, state: "ready", details: initial, receipt_decision: { sequence: 101 } }));
  await assert.rejects(client.read(), /nonaccepted reply carries/u);
  const audio = detailClient((_command, args) => Promise.resolve(operation(args).command === "details_read" ? { version: 1, state: "ready", details: initial } : { version: 1, state: "ready", receipt_decision: { sequence: 101 }, audio: {} }));
  await audio.read(); await assert.rejects(audio.audio(first(initial.corpus.lectures).id), /nonaccepted reply carries/u);
});
await test("decision_digest_preserves_struct_order_unicode_and_escaped_text", async () => {
  const request = { relation_id: "rel-α\"\n", action: "reject" as const, expected_revision: 7, expected_profile_id: "profile-test", selector: { view: "detail_workspace" as const, known_at_accept_seq: null, valid_at_ms: null }, request_id: Array<number>(16).fill(0), client_instance_id: Array<number>(16).fill(1), idempotency_key: Array<number>(32).fill(2) };
  const literal = '{"relation_id":"rel-α\\"\\n","action":"reject","expected_revision":7,"expected_profile_id":"profile-test","selector":{"view":"detail_workspace","known_at_accept_seq":null,"valid_at_ms":null},"request_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"client_instance_id":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],"idempotency_key":[2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2]}';
  assert.deepEqual(await detailDecisionDigest(request), [...createHash("sha256").update("academic.details-decision.v1\0").update(literal).digest()]);
});
await test("decision_digest_matches_the_independently_verified_rust_vector", async () => {
  const request = { idempotency_key: Array<number>(32).fill(3), client_instance_id: Array<number>(16).fill(2), request_id: Array<number>(16).fill(1), selector: { valid_at_ms: null, known_at_accept_seq: null, view: "detail_workspace" as const }, expected_profile_id: "a".repeat(64), expected_revision: 1, action: "reject" as const, relation_id: "example-relation" };
  const digest = (await detailDecisionDigest(request)).map((byte) => byte.toString(16).padStart(2, "0")).join("");
  assert.equal(digest, "24aed73cc7d6e0f3c8b97ba71c325cd649b0fbd229ec88db691f953c2615be3f");
});
await test("detail_fixture_is_built_deterministically_and_profile_empty_is_not_a_fixture", async () => {
  const json = await readFile(new URL("../../../testdata/detail-surfaces/corpus.json", import.meta.url), "utf8");
  assert.equal(json, `${JSON.stringify(buildDetailFixture(), null, 2)}\n`);
  assert.deepEqual(decodeDetailState({ ...fixtureState(), corpus: { lectures: [], questions: [], concepts: [], projects: [] } }).corpus.lectures, []);
});

await test("historical_detail_selector_is_read_only_and_alternate_profile_is_distinct", async () => {
  const initial = fixtureState(); let writes = 0;
  const client = detailClient((_command, args) => {
    const op = operation(args);
    if (op.command !== "details_read") writes++;
    return Promise.resolve({ version: 1, state: "ready", details: initial });
  });
  await client.read({ view: "detail_workspace", known_at_accept_seq: 100, valid_at_ms: initial.valid_at_ms });
  await assert.rejects(client.decide(first(allRelations(initial.corpus)).id, "reject", 1), /Historical/u);
  assert.equal(writes, 0);
  const alternate = { ...initial, profile_id: "synthetic-profile-b", corpus: { lectures: [], concepts: [], questions: [], projects: [] } };
  const second = detailClient(() => Promise.resolve({ version: 1, state: "ready", details: alternate }));
  assert.notDeepEqual(await second.read(), initial);
});

await test("original_audio_is_profile_bound_chunked_and_digest_verified", async () => {
  const state = fixtureState(); const lectureId = first(state.corpus.lectures).id;
  const original = Uint8Array.from({ length: 8500 }, (_, index) => index % 251);
  const digest = createHash("sha256").update(original).digest("hex");
  const offsets: number[] = [];
  const client = detailClient((_command, args) => {
    const op = operation(args);
    if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: state });
    assert.equal(op.command, "details_audio"); assert.equal(op.expected_profile_id, state.profile_id); assert.equal(op.expected_revision, state.revision);
    assert.equal(typeof op.offset, "number"); const offset = Number(op.offset); offsets.push(offset);
    return Promise.resolve({ version: 1, state: "ready", audio: { lecture_id: lectureId, media_type: "audio/wav", content_digest: digest, total_bytes: original.length, offset, bytes: [...original.slice(offset, offset + 4096)] } });
  });
  await client.read(); const audio = await client.audio(lectureId);
  assert.deepEqual(audio.bytes, original); assert.deepEqual(offsets, [0, 4096, 8192]); assert.equal(audio.digest, digest);
});
await test("original_audio_refuses_changed_identity_digest_missing_and_oversize_sources", async () => {
  const state = fixtureState(); const lectureId = first(state.corpus.lectures).id;
  for (const problem of ["missing", "oversize", "digest", "lecture"]) {
    const client = detailClient((_command, args) => {
      const op = operation(args);
      if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: state });
      return Promise.resolve(problem === "missing" ? { version: 1, state: "rejected", reason: "AUDIO_NOT_FOUND" }
        : { version: 1, state: "ready", audio: { lecture_id: problem === "lecture" ? "other-lecture" : lectureId, media_type: "audio/wav", content_digest: "0".repeat(64), total_bytes: problem === "oversize" ? 4194305 : 1, offset: 0, bytes: [1] } });
    });
    await client.read(); await assert.rejects(client.audio(lectureId));
  }
});
await test("cancelled_original_audio_does_not_request_another_chunk", async () => {
  const state = fixtureState(); const lectureId = first(state.corpus.lectures).id; const abort = new AbortController(); let reads = 0;
  const client = detailClient((_command, args) => {
    if (operation(args).command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: state });
    reads++; abort.abort();
    return Promise.resolve({ version: 1, state: "ready", audio: { lecture_id: lectureId, media_type: "audio/wav", content_digest: "0".repeat(64), total_bytes: 8192, offset: 0, bytes: Array<number>(4096).fill(0) } });
  });
  await client.read(); await assert.rejects(client.audio(lectureId, abort.signal)); assert.equal(reads, 1);
});

await test("selected_profile_navigation_and_pinned_evidence_support_nonfixture_ids", () => {
  const initial = fixtureState(); const concept = { ...first(initial.corpus.concepts), id: "profile-concept-42", title: "Profile-specific concept" };
  const project = first(initial.corpus.projects); const relation = { ...first(allRelations(initial.corpus)), id: "profile-backlink", target: { routeId: "learn.concepts", id: concept.id } };
  const state: DetailState = { ...initial, corpus: { ...initial.corpus, concepts: [concept], projects: [{ ...project, relations: { ...project.relations, Custom: [relation] } }] } };
  const destination = { routeId: "learn.concepts", entityId: concept.id, path: `/learn/concepts/${concept.id}` };
  assert.equal(first(profilePalette(state, destination, concept.title)).target.path, destination.path);
  assert.equal(profileTitle(state, destination), concept.title);
  assert.ok(profileBacklinks(state, destination).some((link) => link.target.entityId === project.id));
  const pinned = profileEvidence(state, { kind: "Concept", id: concept.id }); assert.ok(pinned);
  const drawer = renderDrawer({ selected: { kind: "Concept", id: concept.id }, pinned });
  assert.ok(drawer.title.includes(concept.title)); assert.ok(first(drawer.evidence).statement.includes(state.source_digest));
  const rejected: DetailState = { ...state, decisions: [{ sequence: 101, relationId: relation.id, action: "REJECT", undoes: null, actor: "synthetic-user" }] };
  assert.equal(profileBacklinks(rejected, destination).some((link) => link.target.entityId === project.id), false);
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}
function retryStorage() {
  const items = new Map<string, string>();
  return { keys: () => [...items.keys()], getItem: (key: string) => items.get(key) ?? null, setItem: (key: string, value: string) => { items.set(key, value); }, removeItem: (key: string) => { items.delete(key); } };
}

await test("newest_selected_read_wins_and_selector_mutation_cannot_change_its_binding", async () => {
  const old = deferred<unknown>(); const latest = deferred<unknown>(); let reads = 0;
  const client = detailClient(() => ++reads === 1 ? old.promise : latest.promise);
  const selector = { view: "detail_workspace" as const, known_at_accept_seq: 100 as number | null, valid_at_ms: 1788830000000 as number | null };
  const firstRead = client.read(selector); const stale = assert.rejects(firstRead, /superseded/u);
  const secondRead = client.read(selector); selector.known_at_accept_seq = 90;
  latest.resolve({ version: 1, state: "ready", details: fixtureState() });
  assert.equal((await secondRead).known_at_accept_seq, 100);
  old.resolve({ version: 1, state: "ready", details: { ...fixtureState(), known_at_accept_seq: 90 } });
  await stale;
  await assert.rejects(client.decide(first(allRelations(fixtureState().corpus)).id, "reject", 1), /Historical/u);
});

await test("late_original_confirmation_cannot_replace_a_selected_historical_snapshot", async () => {
  const initial = fixtureState(); const write = deferred<unknown>(); const sent = deferred<Record<string, unknown>>(); let reads = 0;
  const historical = { ...initial, known_at_accept_seq: 90, valid_at_ms: 10 };
  const client = detailClient((_command, args) => {
    const op = operation(args);
    if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: ++reads === 1 ? initial : historical });
    sent.resolve(op); return write.promise;
  });
  await client.read(); const result = client.decide(first(allRelations(initial.corpus)).id, "reject", 1);
  const op = await sent.promise;
  await client.read({ view: "detail_workspace", known_at_accept_seq: 90, valid_at_ms: 10 });
  write.resolve(accepted(appendReject(initial), op));
  assert.deepEqual(await result, historical); assert.equal(client.pendingDecisions().length, 0);
  await assert.rejects(client.decide(first(allRelations(initial.corpus)).id, "reject", 1), /Historical/u);
});

await test("accepted_source_and_saved_request_are_immutable_detached_values", async () => {
  const initial = fixtureState(); const client = detailClient((_command, args) => operation(args).command === "details_read"
    ? Promise.resolve({ version: 1, state: "ready", details: initial }) : Promise.reject(new Error("ACK lost")));
  const state = await client.read(); assert.ok(Object.isFrozen(state.corpus.lectures[0]?.segments));
  assert.notEqual(state, initial); assert.equal(Object.isFrozen(initial), false);
  const relation = first(allRelations(state.corpus)); await assert.rejects(client.decide(relation.id, "reject", 1));
  const pending = first(client.pendingDecisions());
  assert.ok(Object.isFrozen(pending) && Object.isFrozen(pending.request_id) && Object.isFrozen(pending.selector));
  await assert.rejects(client.decide(relation.id, "undo", 1), /original pending/u);
});

await test("ui_restart_retries_original_request_after_alias_reuse_and_keeps_other_profiles_separate", async () => {
  const initial = fixtureState(); const original = first(allRelations(initial.corpus)); const store = retryStorage(); const writes: Record<string, unknown>[] = [];
  const firstClient = detailClient((_command, args) => {
    const op = operation(args); if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
    writes.push(structuredClone(op)); return Promise.reject(new Error("service stopped after commit"));
  }, store);
  await firstClient.read(); await assert.rejects(firstClient.decide(original.id, "reject", 1));
  const other = detailClient(() => Promise.resolve({ version: 1, state: "ready", details: { ...initial, profile_id: "other-incarnation" } }), store);
  await other.read(); assert.deepEqual(other.pendingDecisions(), []);
  const newer = { ...initial, revision: 3, known_at_accept_seq: 102, corpus: { lectures: [], concepts: [{ ...first(initial.corpus.concepts), relations: { Replacement: [{ ...original, source: { ...original.source, content: "Replacement evidence" } }] } }], projects: [], questions: [] } };
  const restarted = detailClient((_command, args) => {
    const op = operation(args); if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: newer });
    writes.push(structuredClone(op)); return Promise.resolve(accepted(newer, op));
  }, store);
  await restarted.read(); await assert.rejects(restarted.decide(original.id, "reject", 3), /original pending/u);
  const pending = first(restarted.pendingDecisions());
  const confirmed = await restarted.decide(pending.relation_id, pending.action, pending.expected_revision);
  assert.deepEqual(writes[0], writes[1]); assert.equal(relationRejected(confirmed, original.id), false);
  assert.deepEqual(restarted.pendingDecisions(), []);
});

await test("persisted_undo_keeps_exact_target_across_client_restart", async () => {
  const initial = appendReject(fixtureState()); const store = retryStorage(); const relation = first(initial.decisions).relationId;
  const before = detailClient((_command, args) => operation(args).command === "details_read" ? Promise.resolve({ version: 1, state: "ready", details: initial }) : Promise.reject(new Error("lost")), store);
  await before.read(); await assert.rejects(before.decide(relation, "undo", 2));
  const newer: DetailState = { ...initial, revision: 3, known_at_accept_seq: 102, decisions: [...initial.decisions, { sequence: 102, action: "UNDO", relationId: relation, undoes: 101, actor: RECEIPT_ACTOR }] };
  const after = detailClient((_command, args) => { const op = operation(args); return Promise.resolve(op.command === "details_read" ? { version: 1, state: "ready", details: newer } : accepted(newer, op, { sequence: 102, undoes: 101 })); }, store);
  await after.read(); assert.equal(first(after.pendingDecisions()).action, "undo");
  assert.equal(relationRejected(await after.decide(relation, "undo", 2), relation), false);
});

await test("write_ahead_storage_failure_prevents_native_write_and_duplicate_clicks_share_one_request", async () => {
  const initial = fixtureState(); let writes = 0;
  const failing = detailClient(() => { writes++; return Promise.resolve({ version: 1, state: "ready", details: initial }); }, { keys: () => [], getItem: () => null, setItem: () => { throw new Error("storage unavailable"); }, removeItem: () => {} });
  await failing.read(); await assert.rejects(failing.decide(first(allRelations(initial.corpus)).id, "reject", 1), /storage unavailable/u); assert.equal(writes, 1);
  const delayed = deferred<unknown>(); const sent = deferred<Record<string, unknown>>(); writes = 0;
  const client = detailClient((_command, args) => { const op = operation(args); if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial }); writes++; sent.resolve(op); return delayed.promise; });
  await client.read(); const id = first(allRelations(initial.corpus)).id;
  const one = client.decide(id, "reject", 1); const two = client.decide(id, "reject", 1);
  delayed.resolve(accepted(appendReject(initial), await sent.promise));
  assert.deepEqual(await one, await two); assert.equal(writes, 1);
});

await test("only_matched_definite_refusal_releases_pending_identity", async () => {
  const initial = fixtureState(); const client = detailClient((_command, args) => {
    const op = operation(args); if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
    const good = accepted(appendReject(initial), op);
    return Promise.resolve({ ...good, state: "rejected", reason: "REVISION_CONFLICT", details: null, receipt_id: null, receipt_decision: null, decision_sequence: null });
  });
  await client.read(); await assert.rejects(client.decide(first(allRelations(initial.corpus)).id, "reject", 1), /core refused/u);
  assert.deepEqual(client.pendingDecisions(), []);
});

await test("unknown_projector_and_every_unrelated_receipt_field_are_unavailable", async () => {
  const initial = fixtureState(); assert.throws(() => decodeDetailState({ ...initial, projector_version: "academic.domain-details.v3" }), /unsupported/u);
  for (const key of ["receipt_id", "decision_sequence", "receipt_decision", "request_id", "client_instance_id", "idempotency_key", "request_digest"]) {
    const client = detailClient(() => Promise.resolve({ version: 1, state: "ready", details: initial, [key]: [1] }));
    await assert.rejects(client.read());
  }
});

await test("changing_selected_time_cancels_audio_before_any_more_chunks", async () => {
  const initial = fixtureState(); const chunk = deferred<unknown>(); const sent = deferred<null>(); let chunks = 0;
  const client = detailClient((_command, args) => {
    if (operation(args).command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
    chunks++; sent.resolve(null); return chunk.promise;
  });
  await client.read(); const audio = client.audio(first(initial.corpus.lectures).id); const refused = assert.rejects(audio, /snapshot changed/u);
  await sent.promise; await client.read(); chunk.resolve({ version: 1, state: "ready", audio: {} });
  await refused; assert.equal(chunks, 1);
});

await test("independent_profile_journals_never_overwrite_or_delete_each_others_requests", async () => {
  const store = retryStorage(); const initial = fixtureState(); const id = first(allRelations(initial.corpus)).id;
  const make = (profile: string, confirm: boolean) => detailClient((_command, args) => {
    const state = { ...initial, profile_id: profile }; const op = operation(args);
    if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: state });
    return confirm ? Promise.resolve(accepted(appendReject(state), op)) : Promise.reject(new Error("lost"));
  }, store);
  const a = make("profile-a", false); const b = make("profile-b", false);
  await a.read(); await b.read(); await assert.rejects(a.decide(id, "reject", 1)); await assert.rejects(b.decide(id, "reject", 1));
  assert.equal(store.keys().length, 2);
  const aRestart = make("profile-a", true); await aRestart.read(); await aRestart.decide(id, "reject", 1);
  assert.equal(store.keys().length, 1);
  const bRestart = make("profile-b", true); await bRestart.read();
  assert.deepEqual(bRestart.pendingDecisions(), b.pendingDecisions()); await bRestart.decide(id, "reject", 1); assert.equal(store.keys().length, 0);
});

await test("failed_selected_read_disables_writes_and_failed_journal_removal_retains_exact_retry", async () => {
  const initial = fixtureState(); const id = first(allRelations(initial.corpus)).id; let reads = 0;
  const client = detailClient(() => ++reads === 1 ? Promise.resolve({ version: 1, state: "ready", details: initial }) : Promise.reject(new Error("read unavailable")));
  await client.read(); await assert.rejects(client.read()); await assert.rejects(client.decide(id, "reject", 1), /Reload/u); assert.equal(reads, 2);
  const store = retryStorage(); let removalFails = true; const writes: Record<string, unknown>[] = [];
  const retry = detailClient((_command, args) => {
    const op = operation(args); if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
    writes.push(structuredClone(op)); return Promise.resolve(accepted(appendReject(initial), op));
  }, { ...store, removeItem: (key) => { if (removalFails) throw new Error("storage unavailable"); store.removeItem(key); } });
  await retry.read(); await assert.rejects(retry.decide(id, "reject", 1), /storage unavailable/u);
  assert.equal(retry.pendingDecisions().length, 1); removalFails = false; await retry.decide(id, "reject", 1);
  assert.deepEqual(writes[0], writes[1]); assert.equal(store.keys().length, 0);
});
