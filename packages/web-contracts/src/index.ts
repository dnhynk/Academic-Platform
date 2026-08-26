export type MasteryLevel =
  | "UNSEEN"
  | "EXPOSED"
  | "UNDERSTOOD"
  | "PRACTICED"
  | "APPLIED"
  | "FLUENT";

export type FreshnessBand =
  | "UNKNOWN"
  | "STALE"
  | "LOW"
  | "MODERATE"
  | "HIGH"
  | "VERY_HIGH";

export interface FixtureContract {
  readonly envelope: "academic.signed-batch-envelope/v1 deterministic-cbor";
  readonly payload: "academic.event-batch/v1 deterministic-cbor";
  readonly signature: "Ed25519";
  readonly event_schema_version: 1;
}

export interface ReplaySummary {
  readonly accepted_events: number;
  readonly accept_seq_head: number;
  readonly payload_hash: string;
  readonly envelope_hash: string;
  readonly artifact_digest: string;
  readonly artifact_locator: string;
  readonly mastery: MasteryLevel;
  readonly freshness: FreshnessBand;
  readonly mastery_active_claim_ids: readonly string[];
  readonly mastery_conflicting_claim_ids: readonly string[];
  readonly mastery_rejected_claim_ids: readonly string[];
  readonly deadline_active_claim_ids: readonly string[];
  readonly semantic_digest: string;
}

export interface FixtureDocument {
  readonly fixture_version: 1;
  readonly name: string;
  readonly data_class: "SYNTHETIC_ONLY";
  readonly network_egress: "NONE";
  readonly contract: FixtureContract;
  readonly device_id: string;
  readonly user_id: string;
  readonly public_key_hex: string;
  readonly signed_batch_cbor_hex: string;
  readonly expected_replay: ReplaySummary;
}

const digestPattern = /^sha256:[0-9a-f]{64}$/u;
const locatorPattern = /^locator:v1:[0-9a-f]{64}$/u;
const uuidV7Pattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const hexPattern = /^(?:[0-9a-f]{2})+$/u;
const masteryLevels: ReadonlySet<string> = new Set([
  "UNSEEN",
  "EXPOSED",
  "UNDERSTOOD",
  "PRACTICED",
  "APPLIED",
  "FLUENT",
]);
const freshnessBands: ReadonlySet<string> = new Set([
  "UNKNOWN",
  "STALE",
  "LOW",
  "MODERATE",
  "HIGH",
  "VERY_HIGH",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireString(record: Record<string, unknown>, key: string): string {
  const value = record[key];
  if (typeof value !== "string") {
    throw new TypeError(`${key} must be a string`);
  }
  return value;
}

function requireExactKeys(
  record: Record<string, unknown>,
  keys: readonly string[],
  label: string,
): void {
  const actual = Object.keys(record);
  const allowed = new Set(keys);
  if (actual.length !== keys.length || !actual.every((key) => allowed.has(key))) {
    throw new TypeError(`${label} must contain exactly the declared properties`);
  }
}

function requireNonemptyString(record: Record<string, unknown>, key: string): string {
  const value = requireString(record, key);
  if (value.length === 0) {
    throw new TypeError(`${key} must be nonempty`);
  }
  return value;
}

function requirePositiveInteger(record: Record<string, unknown>, key: string): number {
  const value = record[key];
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw new TypeError(`${key} must be a positive safe integer`);
  }
  return value;
}

function requireUuidArray(record: Record<string, unknown>, key: string): readonly string[] {
  const value = record[key];
  if (!Array.isArray(value) || !value.every((item) => typeof item === "string" && uuidV7Pattern.test(item))) {
    throw new TypeError(`${key} must contain only UUIDv7 strings`);
  }
  if (new Set(value).size !== value.length) {
    throw new TypeError(`${key} must contain unique UUIDv7 strings`);
  }
  return value.map((item) => String(item));
}

function parseContract(value: unknown): FixtureContract {
  if (!isRecord(value)) {
    throw new TypeError("contract must be an object");
  }
  requireExactKeys(value, ["envelope", "payload", "signature", "event_schema_version"], "contract");
  const envelope = requireString(value, "envelope");
  const payload = requireString(value, "payload");
  if (
    envelope !== "academic.signed-batch-envelope/v1 deterministic-cbor" ||
    payload !== "academic.event-batch/v1 deterministic-cbor" ||
    value.signature !== "Ed25519" ||
    value.event_schema_version !== 1
  ) {
    throw new TypeError("unsupported fixture contract");
  }
  return {
    envelope: "academic.signed-batch-envelope/v1 deterministic-cbor",
    payload: "academic.event-batch/v1 deterministic-cbor",
    signature: "Ed25519",
    event_schema_version: 1,
  };
}

function parseReplay(value: unknown): ReplaySummary {
  if (!isRecord(value)) {
    throw new TypeError("expected_replay must be an object");
  }
  requireExactKeys(
    value,
    [
      "accepted_events",
      "accept_seq_head",
      "payload_hash",
      "envelope_hash",
      "artifact_digest",
      "artifact_locator",
      "mastery",
      "freshness",
      "mastery_active_claim_ids",
      "mastery_conflicting_claim_ids",
      "mastery_rejected_claim_ids",
      "deadline_active_claim_ids",
      "semantic_digest",
    ],
    "expected_replay",
  );
  const digests = [
    requireString(value, "payload_hash"),
    requireString(value, "envelope_hash"),
    requireString(value, "artifact_digest"),
    requireString(value, "semantic_digest"),
  ];
  if (!digests.every((digest) => digestPattern.test(digest))) {
    throw new TypeError("replay digests must be canonical SHA-256 values");
  }
  const artifactLocator = requireString(value, "artifact_locator");
  if (!locatorPattern.test(artifactLocator)) {
    throw new TypeError("artifact_locator must be a keyed v1 locator");
  }
  const mastery = requireString(value, "mastery");
  const freshness = requireString(value, "freshness");
  if (!masteryLevels.has(mastery) || !freshnessBands.has(freshness)) {
    throw new TypeError("unsupported mastery or freshness vocabulary");
  }
  return {
    accepted_events: requirePositiveInteger(value, "accepted_events"),
    accept_seq_head: requirePositiveInteger(value, "accept_seq_head"),
    payload_hash: digests[0] ?? "",
    envelope_hash: digests[1] ?? "",
    artifact_digest: digests[2] ?? "",
    artifact_locator: artifactLocator,
    mastery: mastery as MasteryLevel,
    freshness: freshness as FreshnessBand,
    mastery_active_claim_ids: requireUuidArray(value, "mastery_active_claim_ids"),
    mastery_conflicting_claim_ids: requireUuidArray(value, "mastery_conflicting_claim_ids"),
    mastery_rejected_claim_ids: requireUuidArray(value, "mastery_rejected_claim_ids"),
    deadline_active_claim_ids: requireUuidArray(value, "deadline_active_claim_ids"),
    semantic_digest: digests[3] ?? "",
  };
}

export function parseFixtureDocument(value: unknown): FixtureDocument {
  if (!isRecord(value)) {
    throw new TypeError("fixture must be an object");
  }
  requireExactKeys(
    value,
    [
      "fixture_version",
      "name",
      "data_class",
      "network_egress",
      "contract",
      "device_id",
      "user_id",
      "public_key_hex",
      "signed_batch_cbor_hex",
      "expected_replay",
    ],
    "fixture",
  );
  if (value.fixture_version !== 1) {
    throw new TypeError("unsupported fixture_version");
  }
  if (value.data_class !== "SYNTHETIC_ONLY" || value.network_egress !== "NONE") {
    throw new TypeError("Phase 0 fixtures must be synthetic and offline");
  }
  const publicKey = requireString(value, "public_key_hex");
  const signedBatch = requireString(value, "signed_batch_cbor_hex");
  if (!/^[0-9a-f]{64}$/u.test(publicKey) || !hexPattern.test(signedBatch)) {
    throw new TypeError("fixture cryptographic fields must be lowercase hex");
  }
  const deviceId = requireString(value, "device_id");
  const userId = requireString(value, "user_id");
  if (!uuidV7Pattern.test(deviceId) || !uuidV7Pattern.test(userId)) {
    throw new TypeError("device_id and user_id must be UUIDv7 strings");
  }
  return {
    fixture_version: 1,
    name: requireNonemptyString(value, "name"),
    data_class: "SYNTHETIC_ONLY",
    network_egress: "NONE",
    contract: parseContract(value.contract),
    device_id: deviceId,
    user_id: userId,
    public_key_hex: publicKey,
    signed_batch_cbor_hex: signedBatch,
    expected_replay: parseReplay(value.expected_replay),
  };
}
