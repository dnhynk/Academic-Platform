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

export type FixtureVersion = 1 | 2;

export interface FixtureContractV1 {
  readonly envelope: "academic.signed-batch-envelope/v1 deterministic-cbor";
  readonly payload: "academic.event-batch/v1 deterministic-cbor";
  readonly signature: "Ed25519";
  readonly event_schema_version: 1;
}

export interface FixtureContractV2 {
  readonly envelope: "academic.signed-batch-envelope/v1 deterministic-cbor";
  readonly payload: "academic.event-batch/v2 deterministic-cbor";
  readonly signature: "Ed25519";
  readonly event_schema_version: 2;
}

export type FixtureContract = FixtureContractV1 | FixtureContractV2;

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
  readonly fixture_version: FixtureVersion;
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
const confidentialityValues: ReadonlySet<string> = new Set([
  "PUBLIC",
  "PERSONAL",
  "RESTRICTED",
  "SECRET",
]);
const retentionClassValues: ReadonlySet<string> = new Set([
  "EPHEMERAL",
  "COURSE_TERM",
  "USER_MANAGED",
  "LEGAL_HOLD",
]);
const maxSafeJsonInteger = 9_007_199_254_740_991;
const maxUint32 = 4_294_967_295;
const fixtureIntegerPaths: ReadonlySet<string> = new Set([
  "fixture_version",
  "contract.event_schema_version",
  "expected_replay.accepted_events",
  "expected_replay.accept_seq_head",
]);

function boundedDecimalMagnitude(digits: string, limit: number): number {
  const normalized = digits.replace(/^0+/u, "") || "0";
  const limitText = String(limit);
  if (
    normalized.length > limitText.length ||
    (normalized.length === limitText.length && normalized > limitText)
  ) {
    return limit + 1;
  }
  return Number.parseInt(normalized, 10);
}

function isNonnegativeMathematicalIntegerToken(token: string): boolean {
  const match = token.match(
    /^(?<integer>0|[1-9][0-9]*)(?:\.(?<fraction>[0-9]+))?(?:[eE](?<exponentSign>[+-]?)(?<exponent>[0-9]+))?$/u,
  );
  if (match?.groups === undefined) {
    return false;
  }
  const integer = match.groups.integer ?? "";
  const fraction = match.groups.fraction ?? "";
  const coefficient = `${integer}${fraction}`;
  if (/^0+$/u.test(coefficient)) {
    return true;
  }
  const trailingZeros = coefficient.match(/0+$/u)?.[0].length ?? 0;
  const exponentDigits = match.groups.exponent ?? "0";
  const exponent = boundedDecimalMagnitude(
    exponentDigits,
    fraction.length + trailingZeros + 1,
  );
  if (match.groups.exponentSign === "-") {
    return exponent <= trailingZeros && fraction.length <= trailingZeros - exponent;
  }
  return exponent >= fraction.length || trailingZeros >= fraction.length - exponent;
}

function containsControlCharacter(value: string): boolean {
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0;
    if (codePoint < 32 || (codePoint >= 127 && codePoint <= 159)) {
      return true;
    }
  }
  return false;
}

function isLogicalPath(value: string): boolean {
  return (
    value.length > 0 &&
    !value.startsWith("/") &&
    !value.startsWith("~") &&
    !value.includes("\\") &&
    !value.includes(":") &&
    !containsControlCharacter(value) &&
    !value
      .split("/")
      .some((component) => component.length === 0 || component === "." || component === "..")
  );
}

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

function requirePortableUint(
  record: Record<string, unknown>,
  key: string,
  maximum = maxSafeJsonInteger,
): number {
  const value = record[key];
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > maximum
  ) {
    throw new TypeError(`${key} must be an unsigned portable exact integer`);
  }
  return value;
}

function requireDigest(record: Record<string, unknown>, key: string): string {
  const value = requireString(record, key);
  if (!digestPattern.test(value)) {
    throw new TypeError(`${key} must be a canonical SHA-256 digest`);
  }
  return value;
}

class PortableJsonRawParser {
  private index = 0;

  public constructor(
    private readonly input: string,
    private readonly contractLabel: string,
    private readonly requireCanonicalUnsignedIntegerNumbers: boolean,
  ) {}

  private error(message: string): never {
    throw new TypeError(
      `invalid raw ${this.contractLabel} JSON at UTF-16 offset ${String(this.index)}: ${message}`,
    );
  }

  private peek(): string {
    return this.input[this.index] ?? "";
  }

  private skipWhitespace(): void {
    while ([" ", "\t", "\r", "\n"].includes(this.peek())) {
      this.index += 1;
    }
  }

  private assertUnicodeScalarString(value: string): void {
    for (let index = 0; index < value.length; index += 1) {
      const codeUnit = value.charCodeAt(index);
      if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
        const next = value.charCodeAt(index + 1);
        if (!(next >= 0xdc00 && next <= 0xdfff)) {
          this.error("string contains an unpaired high surrogate");
        }
        index += 1;
      } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
        this.error("string contains an unpaired low surrogate");
      }
    }
  }

  private parseString(): string {
    const start = this.index;
    this.index += 1;
    while (this.index < this.input.length) {
      const character = this.peek();
      if (character === '"') {
        this.index += 1;
        let parsed: unknown;
        try {
          parsed = JSON.parse(this.input.slice(start, this.index)) as unknown;
        } catch {
          this.error("malformed JSON string");
        }
        if (typeof parsed !== "string") {
          this.error("internal string parser mismatch");
        }
        this.assertUnicodeScalarString(parsed);
        return parsed;
      }
      if (character === "\\") {
        this.index += 1;
        if (this.peek() === "") {
          this.error("unterminated escape sequence");
        }
        this.index += 1;
        continue;
      }
      if (character.charCodeAt(0) < 0x20) {
        this.error("unescaped control character in string");
      }
      this.index += 1;
    }
    this.error("unterminated string");
  }

  private parseNumber(path: readonly string[]): void {
    const token = this.input.slice(this.index).match(
      /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/u,
    )?.[0];
    if (token === undefined) {
      this.error("malformed number token");
    }
    this.index += token.length;
    if (
      this.requireCanonicalUnsignedIntegerNumbers &&
      !/^(?:0|[1-9][0-9]*)$/u.test(token)
    ) {
      this.error("numbers must use canonical unsigned integer tokens");
    }
    if (
      this.contractLabel === "fixture" &&
      fixtureIntegerPaths.has(path.join(".")) &&
      !isNonnegativeMathematicalIntegerToken(token)
    ) {
      this.error("fixture integer fields must be mathematically integral before conversion");
    }
  }

  private parseLiteral(literal: "true" | "false" | "null"): void {
    if (!this.input.startsWith(literal, this.index)) {
      this.error(`expected ${literal}`);
    }
    this.index += literal.length;
  }

  private parseArray(path: readonly string[]): void {
    this.index += 1;
    this.skipWhitespace();
    if (this.peek() === "]") {
      this.index += 1;
      return;
    }
    let elementIndex = 0;
    while (this.index < this.input.length) {
      this.parseValue([...path, String(elementIndex)]);
      elementIndex += 1;
      this.skipWhitespace();
      if (this.peek() === "]") {
        this.index += 1;
        return;
      }
      if (this.peek() !== ",") {
        this.error("array entries must be comma-separated");
      }
      this.index += 1;
      this.skipWhitespace();
    }
    this.error("unterminated array");
  }

  private parseObject(path: readonly string[]): void {
    const keys = new Set<string>();
    this.index += 1;
    this.skipWhitespace();
    if (this.peek() === "}") {
      this.index += 1;
      return;
    }
    while (this.index < this.input.length) {
      if (this.peek() !== '"') {
        this.error("object keys must be JSON strings");
      }
      const key = this.parseString();
      if (keys.has(key)) {
        this.error(`duplicate object key ${JSON.stringify(key)}`);
      }
      keys.add(key);
      this.skipWhitespace();
      if (this.peek() !== ":") {
        this.error("object key must be followed by ':'");
      }
      this.index += 1;
      this.skipWhitespace();
      this.parseValue([...path, key]);
      this.skipWhitespace();
      if (this.peek() === "}") {
        this.index += 1;
        return;
      }
      if (this.peek() !== ",") {
        this.error("object entries must be comma-separated");
      }
      this.index += 1;
      this.skipWhitespace();
    }
    this.error("unterminated object");
  }

  private parseValue(path: readonly string[]): void {
    const character = this.peek();
    if (character === "{") {
      this.parseObject(path);
    } else if (character === "[") {
      this.parseArray(path);
    } else if (character === '"') {
      this.parseString();
    } else if (character === "t") {
      this.parseLiteral("true");
    } else if (character === "f") {
      this.parseLiteral("false");
    } else if (character === "n") {
      this.parseLiteral("null");
    } else if (character === "-" || (character >= "0" && character <= "9")) {
      this.parseNumber(path);
    } else {
      this.error("expected a JSON value");
    }
  }

  public parse(): void {
    this.skipWhitespace();
    this.parseValue([]);
    this.skipWhitespace();
    if (this.index !== this.input.length) {
      this.error("trailing content after the JSON value");
    }
  }
}

/**
 * Enforces the complete raw ArtifactDescriptor JSON profile before JSON.parse:
 * well-formed JSON, unique decoded property names, Unicode-scalar strings, and
 * canonical unsigned integer number tokens.
 */
export function assertPortableArtifactJsonText(input: string): void {
  new PortableJsonRawParser(input, "artifact", true).parse();
}

/**
 * Enforces the raw signed-fixture wrapper boundary before JSON.parse or Ajv:
 * well-formed JSON, unique decoded property names, and Unicode-scalar strings.
 * Fixture numbers retain ordinary JSON syntax because schema/semantic integer
 * validation intentionally accepts integral decimal and exponent lexemes.
 */
export function assertPortableFixtureJsonText(input: string): void {
  new PortableJsonRawParser(input, "fixture", false).parse();
}

/**
 * Decodes original fixture bytes with fatal UTF-8 semantics, then enforces the
 * raw fixture profile. Invalid byte sequences never become replacement text.
 */
export function decodePortableFixtureJsonBytes(input: Uint8Array): string {
  if (!(input instanceof Uint8Array)) {
    throw new TypeError("fixture JSON input must be original bytes");
  }
  let text: string;
  try {
    text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(input);
  } catch (error: unknown) {
    throw new TypeError("fixture JSON bytes must be strict UTF-8", { cause: error });
  }
  assertPortableFixtureJsonText(text);
  return text;
}

/** Backward-compatible name for the raw gate, now stronger than number checks alone. */
export function assertCanonicalArtifactJsonNumberTokens(input: string): void {
  assertPortableArtifactJsonText(input);
}

/**
 * Enforces ArtifactDescriptor invariants that JSON Schema cannot express.
 * Callers first enforce raw tokens with assertCanonicalArtifactJsonNumberTokens,
 * then parse and run the Draft 2020-12 schema. This layer deliberately repeats
 * security-sensitive primitive checks so it also fails closed alone.
 */
export function assertArtifactDescriptorSemantics(
  value: unknown,
): asserts value is Record<string, unknown> {
  if (!isRecord(value)) {
    throw new TypeError("artifact descriptor must be an object");
  }
  requireExactKeys(
    value,
    [
      "id",
      "content_digest",
      "media_type",
      "byte_length",
      "domain_id",
      "confidentiality",
      "retention_class",
      "permission_lineage_id",
      "format_version",
      "vault_locator",
      "evidence_representations",
    ],
    "artifact descriptor",
  );
  for (const key of ["id", "domain_id", "permission_lineage_id"]) {
    if (!uuidV7Pattern.test(requireString(value, key))) {
      throw new TypeError(`${key} must be a UUIDv7 string`);
    }
  }
  if (!/^[a-z0-9.+-]+\/[a-z0-9.+-]+$/u.test(requireString(value, "media_type"))) {
    throw new TypeError("media_type must be a canonical media type");
  }
  if (!confidentialityValues.has(requireString(value, "confidentiality"))) {
    throw new TypeError("unsupported confidentiality value");
  }
  if (!retentionClassValues.has(requireString(value, "retention_class"))) {
    throw new TypeError("unsupported retention_class value");
  }
  if (value.format_version !== 1) {
    throw new TypeError("unsupported artifact format_version");
  }
  if (!locatorPattern.test(requireString(value, "vault_locator"))) {
    throw new TypeError("vault_locator must be a keyed v1 locator");
  }
  const artifactDigest = requireDigest(value, "content_digest");
  const artifactByteLength = requirePortableUint(value, "byte_length");
  const representations = value.evidence_representations;
  if (!Array.isArray(representations)) {
    throw new TypeError("evidence_representations must be an array");
  }

  const locatorIdentities = new Set<string>();
  for (const representationValue of representations) {
    if (!isRecord(representationValue)) {
      throw new TypeError("evidence representation must be an object");
    }
    requireExactKeys(
      representationValue,
      ["locator", "content_digest", "byte_length"],
      "evidence representation",
    );
    const representationDigest = requireDigest(representationValue, "content_digest");
    const representationByteLength = requirePortableUint(representationValue, "byte_length");
    const locator = representationValue.locator;
    if (!isRecord(locator)) {
      throw new TypeError("evidence locator must be an object");
    }
    const kind = requireString(locator, "kind");
    let identity: string;
    switch (kind) {
      case "PAGE": {
        requireExactKeys(locator, ["kind", "page_number"], "PAGE evidence locator");
        const pageNumber = requirePortableUint(locator, "page_number", maxUint32);
        if (pageNumber === 0) {
          throw new TypeError("page_number must be positive");
        }
        identity = JSON.stringify(["PAGE", pageNumber]);
        break;
      }
      case "TEXT_BYTES": {
        requireExactKeys(
          locator,
          ["kind", "source_digest", "start", "end"],
          "TEXT_BYTES evidence locator",
        );
        const sourceDigest = requireDigest(locator, "source_digest");
        const start = requirePortableUint(locator, "start");
        const end = requirePortableUint(locator, "end");
        if (start >= end) {
          throw new TypeError("TEXT_BYTES range must be nonempty and increasing");
        }
        if (
          sourceDigest !== artifactDigest ||
          end > artifactByteLength ||
          representationByteLength !== end - start
        ) {
          throw new TypeError("TEXT_BYTES must be bounded by the registered artifact bytes");
        }
        if (
          start === 0 &&
          end === artifactByteLength &&
          representationDigest !== artifactDigest
        ) {
          throw new TypeError("full-range TEXT_BYTES digest must equal the artifact digest");
        }
        identity = JSON.stringify(["TEXT_BYTES", sourceDigest, start, end]);
        break;
      }
      case "TRANSCRIPT_TIME": {
        requireExactKeys(
          locator,
          ["kind", "start_ms", "end_ms"],
          "TRANSCRIPT_TIME evidence locator",
        );
        const start = requirePortableUint(locator, "start_ms");
        const end = requirePortableUint(locator, "end_ms");
        if (start >= end) {
          throw new TypeError("TRANSCRIPT_TIME range must be nonempty and increasing");
        }
        identity = JSON.stringify(["TRANSCRIPT_TIME", start, end]);
        break;
      }
      case "REPOSITORY_BYTES": {
        requireExactKeys(
          locator,
          ["kind", "snapshot_digest", "path", "start", "end"],
          "REPOSITORY_BYTES evidence locator",
        );
        const snapshotDigest = requireDigest(locator, "snapshot_digest");
        const path = requireString(locator, "path");
        const start = requirePortableUint(locator, "start");
        const end = requirePortableUint(locator, "end");
        if (!isLogicalPath(path)) {
          throw new TypeError("repository path must be normalized and repository-relative");
        }
        if (start >= end || representationByteLength !== end - start) {
          throw new TypeError("REPOSITORY_BYTES length must match a nonempty increasing span");
        }
        identity = JSON.stringify(["REPOSITORY_BYTES", snapshotDigest, path, start, end]);
        break;
      }
      default:
        throw new TypeError("unsupported evidence locator kind");
    }
    if (locatorIdentities.has(identity)) {
      throw new TypeError("artifact evidence locator identities must be unique");
    }
    locatorIdentities.add(identity);
  }
}

/** Parses raw ArtifactDescriptor JSON through lexical, structural, and semantic validation. */
export function parseArtifactDescriptorJson(input: string): Record<string, unknown> {
  assertPortableArtifactJsonText(input);
  const value: unknown = JSON.parse(input) as unknown;
  assertArtifactDescriptorSemantics(value);
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

function parseContract(value: unknown, fixtureVersion: FixtureVersion): FixtureContract {
  if (!isRecord(value)) {
    throw new TypeError("contract must be an object");
  }
  requireExactKeys(value, ["envelope", "payload", "signature", "event_schema_version"], "contract");
  const envelope = requireString(value, "envelope");
  const payload = requireString(value, "payload");
  const expectedEnvelope = "academic.signed-batch-envelope/v1 deterministic-cbor";
  const expectedPayload = fixtureVersion === 1
    ? "academic.event-batch/v1 deterministic-cbor"
    : "academic.event-batch/v2 deterministic-cbor";
  if (
    envelope !== expectedEnvelope ||
    payload !== expectedPayload ||
    value.signature !== "Ed25519" ||
    value.event_schema_version !== fixtureVersion
  ) {
    throw new TypeError("unsupported fixture contract");
  }
  if (fixtureVersion === 1) {
    return {
      envelope: "academic.signed-batch-envelope/v1 deterministic-cbor",
      payload: "academic.event-batch/v1 deterministic-cbor",
      signature: "Ed25519",
      event_schema_version: 1,
    };
  }
  return {
    envelope: "academic.signed-batch-envelope/v1 deterministic-cbor",
    payload: "academic.event-batch/v2 deterministic-cbor",
    signature: "Ed25519",
    event_schema_version: 2,
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
  if (value.fixture_version !== 1 && value.fixture_version !== 2) {
    throw new TypeError("unsupported fixture_version");
  }
  const fixtureVersion = value.fixture_version;
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
    fixture_version: fixtureVersion,
    name: requireNonemptyString(value, "name"),
    data_class: "SYNTHETIC_ONLY",
    network_egress: "NONE",
    contract: parseContract(value.contract, fixtureVersion),
    device_id: deviceId,
    user_id: userId,
    public_key_hex: publicKey,
    signed_batch_cbor_hex: signedBatch,
    expected_replay: parseReplay(value.expected_replay),
  };
}

/** Parses original fixture bytes through strict UTF-8, raw JSON, and TypeScript semantics. */
export function parseFixtureDocumentJson(input: Uint8Array): FixtureDocument {
  const text = decodePortableFixtureJsonBytes(input);
  return parseFixtureDocument(JSON.parse(text) as unknown);
}
