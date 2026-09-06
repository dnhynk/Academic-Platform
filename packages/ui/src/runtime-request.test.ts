import assert from "node:assert/strict";
import test from "node:test";
import { runtimeRequests, type RuntimeView } from "./runtime-request.js";

function deferred() {
  let resolve!: (value: unknown) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<unknown>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const accepted = { version: 1, state: "accepted", message: "Synthetic example saved.", receipt_id: Array<number>(16).fill(7) };

void test("diagnostics and a second save cannot release or overwrite an outstanding save", async () => {
  const first = deferred();
  const check = deferred();
  const second = deferred();
  const replies = [first, check, second];
  const calls: unknown[] = [];
  const views: RuntimeView[] = [];
  const request = runtimeRequests(async (command, args) => {
    assert.equal(command, "desktop_request_v1");
    calls.push(args);
    const reply = replies[calls.length - 1];
    assert.ok(reply, "unexpected native call");
    return reply.promise;
  }, (view) => { views.push(view); });
  const current = () => { const view = views.at(-1); assert.ok(view); return view; };
  const save = request("synthetic_ingest");
  assert.equal(current().pending, "synthetic_ingest");
  assert.match(current().saveMessage, /not yet confirmed/u);
  assert.equal(current().receiptId, null);
  await request("diagnostics");
  await request("synthetic_ingest");
  assert.equal(calls.length, 1);
  assert.equal(current().pending, "synthetic_ingest");
  assert.equal(views.length, 1, "ignored requests publish nothing");
  first.resolve(accepted);
  await save;
  assert.equal(current().pending, null);
  assert.equal(current().receiptId, "07".repeat(16));
  assert.equal(current().saveMessage, "Synthetic example saved.");
  const diagnostic = request("diagnostics");
  await request("synthetic_ingest");
  assert.equal(calls.length, 2);
  check.resolve({ version: 1, state: "ready", message: "Local service connected." });
  await diagnostic;
  assert.equal(current().receiptId, "07".repeat(16), "diagnostics preserves the last completed save");
  assert.equal(current().saveMessage, "Synthetic example saved.");
  const nextSave = request("synthetic_ingest");
  assert.equal(current().receiptId, null, "new save clears the previous receipt before invoking");
  second.reject(new Error("acknowledgement lost"));
  await nextSave;
  assert.equal(current().pending, null);
  assert.equal(current().receiptId, null);
  assert.match(current().saveMessage, /could not confirm/u);
});

void test("unavailable, declined and malformed acknowledgements never display a saved receipt", async () => {
  for (const reply of [
    { version: 1, state: "unavailable", message: "Save could not be confirmed." },
    { version: 1, state: "rejected", message: "Request declined." },
    { ...accepted, receipt_id: null },
    { ...accepted, receipt_id: Array(16).fill(256) },
    { ...accepted, version: 2 },
    { ...accepted, state: "unknown" },
  ]) {
    let final: RuntimeView | undefined;
    await runtimeRequests(() => Promise.resolve(reply), (view) => { final = view; })("synthetic_ingest");
    assert.ok(final);
    assert.equal(final.receiptId, null);
    assert.equal(final.pending, null);
    assert.notEqual(final.saveMessage, "Synthetic example saved.");
  }
});
