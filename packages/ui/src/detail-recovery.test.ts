import assert from "node:assert/strict";
import { mkdtemp, readdir, readFile, writeFile, unlink } from "node:fs/promises";
import { readFileSync, readdirSync, writeFileSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Worker } from "node:worker_threads";
import { createHash } from "node:crypto";
import test from "node:test";
import { buildDetailFixture } from "./detail-fixture.js";
import { detailClient, detailDecisionDigest, type DetailDecisionRequest, type DetailState } from "./detail-client.js";

const initial: DetailState = { corpus: buildDetailFixture(), decisions: [], revision: 1, profile_id: "concurrent-synthetic-profile", known_at_accept_seq: 100, valid_at_ms: 1000, projector_version: "academic.details.v2", source_digest: "1".repeat(64) };
const relation = initial.corpus.lectures[0]?.links["Concept candidates and approval"]?.[0]?.id;
assert.ok(relation);
function operation(args: unknown): DetailDecisionRequest & { command: string; offset: number } {
  return (args as { request: { operation: DetailDecisionRequest & { command: string; offset: number } } }).request.operation;
}
async function receipt(op: DetailDecisionRequest) {
  const actor = "00000000-0000-7000-8000-000000000001";
  return { version: 1, state: "accepted", details: { ...initial, revision: 2, known_at_accept_seq: 101, decisions: [{ sequence: 101, relationId: op.relation_id, action: "REJECT", undoes: null, actor }] }, receipt_id: Array<number>(16).fill(7), decision_sequence: 101,
    receipt_decision: { sequence: 101, relation_id: op.relation_id, relation_claim_id: "00000000-0000-7000-8000-000000000002", action: op.action, undoes: null, actor },
    request_id: op.request_id, client_instance_id: op.client_instance_id, idempotency_key: op.idempotency_key, request_digest: await detailDecisionDigest(op) };
}

await test("concurrent_equal_action_v1_requests_restart_and_confirm_independently", async () => {
  const directory = await mkdtemp(join(tmpdir(), "detail-journal-"));
  // Exact clients in Node threads, with the two journal enumerations held before
  // either write. This models synchronous shared storage, not WebView scheduling.
  const script = `
    const {workerData, parentPort} = require('node:worker_threads');
    const {readdirSync,readFileSync,writeFileSync,unlinkSync} = require('node:fs');
    (async () => {
      const {detailClient} = await import(workerData.module);
      const barrier = new Int32Array(workerData.barrier); let calls = 0;
      const path = key => workerData.directory + '/' + encodeURIComponent(key);
      const storage = {
        keys() { const keys = readdirSync(workerData.directory).map(decodeURIComponent);
          if (++calls === 2) { Atomics.add(barrier,0,1); Atomics.notify(barrier,0);
            const deadline = Date.now()+10000;
            while (Atomics.load(barrier,0)<2) { if(Date.now()>deadline) throw Error('Journal barrier timeout'); Atomics.wait(barrier,0,1,100); }
          } return keys;
        },
        getItem(key) { try { return readFileSync(path(key),'utf8'); } catch(e) { if(e.code==='ENOENT')return null; throw e; } },
        setItem(key,value) { writeFileSync(path(key),value); }, removeItem(key) { unlinkSync(path(key)); }
      };
      let submitted;
      const client = detailClient(async (_,args) => { const op = args.request.operation;
        if(op.command==='details_read')return {version:1,state:'ready',details:workerData.initial};
        submitted = op; throw Error('Ambiguous transport loss');
      },storage);
      await client.read();
      try { await client.decide(workerData.relation,'reject',1); } catch(e) { if(!String(e).includes('Ambiguous'))throw e; }
      parentPort.postMessage(submitted);
    })().catch(error => { throw error; });`;
  const barrier = new SharedArrayBuffer(4);
  const start = () => new Promise<DetailDecisionRequest>((resolve, reject) => {
    const worker = new Worker(script, { eval: true, workerData: { directory, barrier, initial, relation, module: new URL("./detail-client.js", import.meta.url).href } });
    worker.once("message", (message: DetailDecisionRequest) => { resolve(message); }); worker.once("error", reject);
    worker.once("exit", (code) => { if (code !== 0) reject(new Error(`Worker exit ${String(code)}`)); });
  });
  const [a, b] = await Promise.all([start(), start()]);
  assert.notDeepEqual(a.request_id, b.request_id);
  const path = (key: string) => join(directory, encodeURIComponent(key));
  const storage = { keys: () => readdirSync(directory).map(decodeURIComponent), getItem: (key: string) => { try { return readFileSync(path(key), "utf8"); } catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return null; throw error; } }, setItem: (key: string, value: string) => { writeFileSync(path(key), value); }, removeItem: (key: string) => { unlinkSync(path(key)); } };
  assert.equal(storage.keys().length, 2);
  const original = new Map(await Promise.all((await readdir(directory)).map(async (key) => [key, await readFile(join(directory, key), "utf8")] as const)));
  const submissions: DetailDecisionRequest[] = [];
  const make = () => detailClient(async (_, args) => {
    const op = operation(args); if (op.command === "details_read") return { version: 1, state: "ready", details: initial };
    submissions.push(op); const accepted = await receipt(op);
    return JSON.stringify(op.request_id) === JSON.stringify(a.request_id) ? accepted
      : { ...accepted, state: "rejected", reason: "REVISION_CONFLICT", details: null, receipt_id: null, receipt_decision: null, decision_sequence: null };
  }, storage);
  const first = make(); const second = make();
  await first.read(); await second.read();
  assert.equal(first.pendingDecisions().length, 2);
  const other = detailClient(() => Promise.resolve({ version: 1, state: "ready", details: { ...initial, profile_id: "unaffected-profile" } }), storage);
  await other.read(); assert.equal(other.pendingDecisions().length, 0);
  await assert.rejects(other.decide(relation, "reject", 1, { requestId: a.request_id }), /no longer available/u);
  await assert.rejects(first.decide(relation, "reject", 1), /exact original/u);
  assert.equal(submissions.length, 0);
  await first.decide(relation, "reject", 1, { requestId: a.request_id });
  assert.deepEqual(submissions[0], a);
  assert.equal(storage.keys().length, 1);
  const surviving = (await readdir(directory))[0]; assert.ok(surviving);
  assert.equal(await readFile(join(directory, surviving), "utf8"), original.get(surviving));
  // The second client still had both in memory. A stale click must neither
  // synthesize a request nor write the already-removed v1 record back.
  await assert.rejects(second.decide(relation, "reject", 1, { requestId: a.request_id }), /no longer available/u);
  await assert.rejects(second.decide(relation, "reject", 1, { requestId: b.request_id }), /core refused/u);
  assert.deepEqual(submissions[1], b); assert.equal(storage.keys().length, 0);
  await other.read(); assert.equal(other.pendingDecisions().length, 0);
  // Keep the exercised legacy record format: no migration/coalescing is needed.
  for (const [key, value] of original) await writeFile(join(directory, key), value);
  const restarted = make(); await restarted.read(); assert.equal(restarted.pendingDecisions().length, 2);
  for (const key of original.keys()) await unlink(join(directory, key));
});

await test("late_confirmation_retains_destination_audio_snapshot_until_explicit_read", async () => {
  let confirm!: (value: unknown) => void; let audioChunk!: (value: unknown) => void;
  let sent!: (value: DetailDecisionRequest) => void;
  const written = new Promise<DetailDecisionRequest>((resolve) => { sent = resolve; });
  const response = new Promise<unknown>((resolve) => { confirm = resolve; });
  const chunk = new Promise<unknown>((resolve) => { audioChunk = resolve; });
  const bytes = Uint8Array.from([1, 2, 3]); const lectureId = initial.corpus.lectures[0]?.id; assert.ok(lectureId);
  const client = detailClient((_, args) => {
    const op = operation(args);
    if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
    if (op.command === "details_audio") return chunk;
    sent(op); return response;
  });
  const owner = new AbortController(); await client.read();
  const decision = client.decide(relation, "reject", 1, { selection: owner.signal }); const op = await written;
  owner.abort(); // Navigation releases the old view; its request still settles.
  const audio = client.audio(lectureId);
  confirm(await receipt(op)); assert.equal((await decision).revision, 1); assert.equal(client.pendingDecisions().length, 0);
  audioChunk({ version: 1, state: "ready", audio: { lecture_id: lectureId, media_type: "audio/wav", content_digest: createHash("sha256").update(bytes).digest("hex"), total_bytes: 3, offset: 0, bytes: [...bytes] } });
  assert.deepEqual((await audio).bytes, bytes);
  await assert.rejects(client.decide(relation, "reject", 1), /Reload/u);
});
