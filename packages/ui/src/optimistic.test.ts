/**
 * `optimistic_update_is_not_canonical_before_receipt`, the TypeScript half.
 *
 * The Rust half is `crates/desktop/tests/optimistic.rs` and the compile-fail
 * cases beside it; that is where the seal is enforced by the type system. Here
 * the seal is enforced by the value having no runtime home outside a module
 * scoped `WeakMap`, and this file observes that there is no route from an
 * `Optimistic<T>` to its value, and that a receipt that matches on some fields
 * and not others promotes nothing.
 */

import assert from "node:assert/strict";
import test from "node:test";

import {
  confirm,
  isCanonical,
  optimistic,
  type AcceptanceReceipt,
  type SubmittedRequest,
} from "./optimistic.js";

const request: SubmittedRequest = {
  requestId: "000102030405060708090a0b0c0d0e0f",
  idempotencyKey: "20".repeat(32),
  requestDigest: "40".repeat(32),
};

const receipt: AcceptanceReceipt = {
  receiptId: "60".repeat(16),
  requestId: request.requestId,
  idempotencyKey: request.idempotencyKey,
  requestDigest: request.requestDigest,
  profileRevision: 18_446_744_073_709_551_615n,
};

void test("optimistic_update_is_not_canonical_before_receipt", () => {
  const update = optimistic({ title: "renamed by the user" }, request);

  // The tag says what it is, and nothing on it is the value.
  assert.equal(update.state, "OPTIMISTIC");
  assert.equal(isCanonical(update), false);
  assert.deepEqual(Object.keys(update).toSorted(), ["request", "state"]);
  assert.deepEqual(Object.getOwnPropertySymbols(update), []);
  assert.equal(
    JSON.stringify(update),
    JSON.stringify({ state: "OPTIMISTIC", request }),
    "the optimistic wrapper serialised something other than its tag and its request",
  );
  assert.equal(
    JSON.stringify(update).includes("renamed by the user"),
    false,
    "the optimistic value reached the wire",
  );

  // The receipt is the only promotion, and it produces the value.
  const confirmed = confirm(update, receipt);
  assert.equal(confirmed.ok, true);
  assert.ok(confirmed.ok);
  assert.equal(isCanonical(confirmed.canonical), true);
  assert.deepEqual(confirmed.canonical.value, { title: "renamed by the user" });
  assert.equal(confirmed.canonical.receipt, receipt);
});

void test("optimistic_update_is_not_canonical_before_receipt rejects its violations", () => {
  const update = optimistic(7, request);

  // Every bound field is compared. Matching on two of three promotes nothing.
  const mismatches: readonly (readonly [Partial<AcceptanceReceipt>, string])[] = [
    [{ requestId: "ff".repeat(16) }, "REQUEST_ID_MISMATCH"],
    [{ idempotencyKey: "ff".repeat(32) }, "IDEMPOTENCY_KEY_MISMATCH"],
    [{ requestDigest: "ff".repeat(32) }, "REQUEST_DIGEST_MISMATCH"],
  ];
  for (const [override, reason] of mismatches) {
    const outcome = confirm(update, { ...receipt, ...override });
    assert.equal(outcome.ok, false, `${reason} was accepted as a promotion`);
    assert.ok(!outcome.ok);
    assert.equal(outcome.failure.reason, reason);
  }

  // A wrapper this module did not seal has no value to promote, so a forged
  // object shaped like an optimistic update cannot be confirmed into one.
  const forged = { state: "OPTIMISTIC", request } as ReturnType<typeof optimistic<number>>;
  const forgedOutcome = confirm(forged, receipt);
  assert.equal(forgedOutcome.ok, false);
  assert.ok(!forgedOutcome.ok);
  assert.equal(forgedOutcome.failure.reason, "UNKNOWN_OPTIMISTIC_UPDATE");

  // A structural clone is a forgery for the same reason: the seal is identity,
  // not shape, so copying the wrapper does not copy the value with it.
  const copied = { ...update };
  const copiedOutcome = confirm(copied, receipt);
  assert.equal(copiedOutcome.ok, false);
  assert.ok(!copiedOutcome.ok);
  assert.equal(copiedOutcome.failure.reason, "UNKNOWN_OPTIMISTIC_UPDATE");

  // The genuine wrapper still promotes, so the refusals above are the forgeries
  // rather than a `confirm` that refuses everything.
  assert.equal(confirm(update, receipt).ok, true);
});
