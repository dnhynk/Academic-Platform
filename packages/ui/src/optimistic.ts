/**
 * Optimistic shell state, and the receipt that is the only way out of it.
 *
 * ADR-001: "A UI optimistic update is not canonical until the core returns an
 * immutable object/event ID and local acceptance receipt." That is enforced
 * here by the absence of an exit rather than by a convention:
 *
 * - `Optimistic<T>` carries no readable member. Its value lives in a
 *   module-scoped `WeakMap` that is not exported, so there is no property to
 *   read, no spread that recovers it, and no `JSON.stringify` that emits it.
 * - The only function that returns `Canonical<T>` is {@link confirm}, and it
 *   returns one only when the receipt's request digest and idempotency key
 *   equal the ones the optimistic update was submitted under.
 * - A rejected or mismatched receipt yields a typed failure, never a value.
 *
 * `crates/desktop/src/optimistic.rs` is the same contract in Rust, with
 * `trybuild` cases for the exits that must not compile. This module is the
 * TypeScript half, for the shell state that never crosses into Rust.
 *
 * This is the same *kind* of seal as `academic_scenario::Proposed<T>` and is
 * deliberately not that type: `Proposed<T>` has no promotion at all, because a
 * projection becomes canonical only through a user decision recorded as its own
 * event. An optimistic update has exactly one promotion, and it is a receipt.
 */

/** The receipt fields the shell compares. Mirrors `ImmutableReceipt`. */
export interface AcceptanceReceipt {
  /** Sixteen opaque bytes, lower-case hex. */
  readonly receiptId: string;
  /** Sixteen opaque bytes, lower-case hex. */
  readonly requestId: string;
  /** Thirty-two bytes, lower-case hex. */
  readonly idempotencyKey: string;
  /** Thirty-two SHA-256 bytes, lower-case hex. */
  readonly requestDigest: string;
  /** The profile revision the acceptance produced. */
  readonly profileRevision: bigint;
}

/** What the shell submitted, and what a receipt has to match. */
export interface SubmittedRequest {
  readonly requestId: string;
  readonly idempotencyKey: string;
  readonly requestDigest: string;
}

/** An update the user has seen and the core has not accepted. */
export interface Optimistic<T> {
  /** Discriminates the two states at a glance and in a log. */
  readonly state: "OPTIMISTIC";
  /** What the update was submitted as. */
  readonly request: SubmittedRequest;
  /**
   * Present only in the type, never at runtime.
   *
   * It makes `Optimistic<string>` and `Optimistic<number>` different types
   * without giving the value a runtime home outside the closed `WeakMap`.
   */
  readonly __value?: undefined extends T ? never : (value: T) => void;
}

/** An update the core has accepted, with the receipt that says so. */
export interface Canonical<T> {
  readonly state: "CANONICAL";
  readonly value: T;
  readonly receipt: AcceptanceReceipt;
}

/** Why a receipt did not promote an optimistic update. */
export type ConfirmationFailure =
  | { readonly reason: "UNKNOWN_OPTIMISTIC_UPDATE" }
  | { readonly reason: "REQUEST_ID_MISMATCH" }
  | { readonly reason: "IDEMPOTENCY_KEY_MISMATCH" }
  | { readonly reason: "REQUEST_DIGEST_MISMATCH" };

/** The result of presenting a receipt. */
export type Confirmation<T> =
  | { readonly ok: true; readonly canonical: Canonical<T> }
  | { readonly ok: false; readonly failure: ConfirmationFailure };

/**
 * The sealed values.
 *
 * Module scope and not exported. This is the whole seal: an `Optimistic<T>`
 * handed to any other module is an object with a state tag and a request, and
 * the value it stands for is not reachable from it.
 */
const sealed = new WeakMap<object, unknown>();

/** Wraps a value the user has seen and the core has not accepted. */
export function optimistic<T>(value: T, request: SubmittedRequest): Optimistic<T> {
  const handle: Optimistic<T> = { state: "OPTIMISTIC", request };
  sealed.set(handle, value);
  return handle;
}

/**
 * Presents a receipt for an optimistic update.
 *
 * Every field the core binds a request to is compared. A receipt that matches
 * on two of the three does not promote anything.
 */
export function confirm<T>(
  update: Optimistic<T>,
  receipt: AcceptanceReceipt,
): Confirmation<T> {
  if (!sealed.has(update)) {
    return { ok: false, failure: { reason: "UNKNOWN_OPTIMISTIC_UPDATE" } };
  }
  if (receipt.requestId !== update.request.requestId) {
    return { ok: false, failure: { reason: "REQUEST_ID_MISMATCH" } };
  }
  if (receipt.idempotencyKey !== update.request.idempotencyKey) {
    return { ok: false, failure: { reason: "IDEMPOTENCY_KEY_MISMATCH" } };
  }
  if (receipt.requestDigest !== update.request.requestDigest) {
    return { ok: false, failure: { reason: "REQUEST_DIGEST_MISMATCH" } };
  }
  return {
    ok: true,
    canonical: { state: "CANONICAL", value: sealed.get(update) as T, receipt },
  };
}

/**
 * Whether a value is canonical.
 *
 * The shell renders an optimistic value with its own affordance; this is the
 * predicate that decides which, and it reads the tag rather than the presence
 * of a value, because an optimistic update has no value to test for.
 */
export function isCanonical<T>(value: Optimistic<T> | Canonical<T>): value is Canonical<T> {
  return value.state === "CANONICAL";
}
