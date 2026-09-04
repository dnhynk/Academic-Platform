// A type that carries key material or decrypted plaintext must not derive
// `Debug`.
//
// This regressed three times. `OpenedHeader` shipped deriving `Debug` over a
// raw DEK and a plaintext digest; `EncryptedDomainKeyring` shipped deriving it
// over per-domain KEKs; `EncryptedObjectReader` shipped deriving it over a
// buffer holding up to one chunk -- a mebibyte by default -- of decrypted
// artifact plaintext. In each case `format!("{value:?}")` in a log line, a
// panic message, or an audit row would have printed the bytes, which is what
// ADR-005 "Zeroization and exposure boundary" forbids.
//
// `missing_debug_implementations = "deny"` is what makes the regression easy:
// the lint demands a `Debug`, and the one-line way to satisfy it is the derive
// that leaks. So the rule is checked mechanically rather than by review, in
// six halves that fail for different reasons:
//
//   1. A registry of the types already known to carry secrets. Each must still
//      exist, must not derive `Debug` or `Display`, and must have a
//      hand-written `Debug`. This is what fails if someone adds a derive back.
//   2. The registry is itself checked against the source: a type that holds
//      secret bytes in a field of its own and hand-writes a redacting `Debug`
//      must be in it. `T114` found that *deleting* a registration was silent —
//      every other check iterates the registry, so a type removed from it
//      simply stopped being covered.
//   3. What a hand-written `Debug` prints. `T114` injected an impl that
//      redacted every field but one; the registry said the impl existed and
//      nothing read it. A raw byte field may reach the formatter only through
//      a length.
//   4. A whole-set classification of every named byte-buffer field in the
//      workspace: `BYTE_FIELD_CLASSES` names each one and says what it holds,
//      compared against the source in both directions. This is the discovery
//      net for the types nobody has listed yet.
//   5. The same whole set for tuple structs and tuple enum variants, which
//      have no field name at all. `T114` found `RecoveredSecret`,
//      `BackupMasterKey`, and a variant `Dek([u8; 32])` invisible to a guard
//      that read only named fields.
//   6. Last and weakest, the `SECRET_FIELD_NAMES` alternation, which now
//      reaches only `String` and `str` fields.
//
// Half 4 used to be that alternation applied to byte buffers too, and that is
// the empty guard `T166` measured: it decided whether a `Vec<u8>` leaked by
// matching the *field name* against a closed list, so a field the list did not
// name was silently safe. `Vec<u8>` called `excerpt` passed. The list of names
// a byte buffer can hide behind is open and the list of fields that exist is
// not, so the question is asked the other way round now: every byte buffer in
// the workspace is enumerated and classified, and a new one fails until
// somebody classifies it. `S-10` on `docs/contracts/policy-source-scans.md`
// records the five previous attempts to close this by adding names, and what
// each one cost.
//
// The three layers are ordered by what carries the judgement, strongest first:
//
//   * The *type* decides, for a byte buffer. `Vec<u8>`, `[u8; N]` and `[u8]`
//     are read as bytes whatever the field is called, which is the rule this
//     file already applied to tuple positions and now applies to named fields.
//   * The *classification* decides whether those bytes may be printed, and it
//     is a closed vocabulary rather than free prose, so a widening is visible
//     as a new class and not as one more plausible sentence.
//   * The *name* decides only where the type cannot: `String` and `str`, where
//     `Qualifier.key` is a qualifier name and `OpenedHeader.dek` would be a
//     key. This layer is a token list and is known to be the weakest of the
//     three; nothing may be closed by adding a name to it alone.
//
// The shapes the net reads are the ones `T114`'s injection matrix reached:
// `&'a [u8]`, `Option<Vec<u8>>`, a path-qualified `zeroize::Zeroizing<...>`,
// a `cfg_attr`-wrapped derive, a single-line struct body, and a field type
// whose buffer is behind a comma inside its generic arguments.
//
// Scope is every `.rs` file in every workspace package except its `tests`
// tree -- the product surface ADR-005 governs. `crates/*/src` was
// the scope until `T146` measured what it missed: a `#[derive(Debug)]` type
// with a `key_bytes: Vec<u8>` field passed this scan in
// `crates/record/examples/emit_harness.rs` and in
// `crates/worker/probes/worker_probe.rs`, and failed at once when the same
// type was written under `src`. Both trees hold product-shaped code that
// `cargo clippy --workspace --all-targets` compiles; the example has no
// feature gate and is run by the documented `pnpm harness:emit` script.
// Test-only helper types are still not scanned. `benches` was excluded beside
// `tests` until `T149` observed that the reason given was a reason about
// `tests`: a bench target is compiled by `cargo clippy --workspace
// --all-targets` with no feature gate, which is the test `T146` applied to
// `examples/`.

import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const CRATES_ROOT = join(REPOSITORY_ROOT, "crates");

/** Types known to carry key material or decrypted plaintext, and why. */
const SECRET_BEARING_TYPES = new Map([
  ["OpenedHeader", "the raw per-object DEK and the plaintext digest"],
  ["EncryptedObjectReader", "a buffer of decrypted artifact plaintext"],
  ["RecoveredSecret", "the secret an operating-system broker returned"],
  ["BackupMasterKey", "the 32-byte backup root"],
  ["DomainKeyring", "raw domain key bytes"],
  ["EncryptedDomainKeyring", "per-domain KEKs and locator keys"],
  ["RuntimeToolCall", "the exact payload presented at the capability boundary"],
  ["AuthorizedToolCall", "the exact payload released after capability consumption"],
  ["ProcessActivity", "the exact external-transmission bytes before audit hashing"],
  ["TranscriptIdentity", "the student number and name a redacted export exists to remove"],
  ["NormalizedTranscript", "a whole official transcript, identity header included"],
  ["RedactedProjection", "the identity values one redaction profile chose to retain"],
  ["SourceDocument", "the private document bytes an egress request selects from"],
  ["Preview", "the exact staged bytes a transmission writes"],
  ["AcceptedResponse", "a provider response that passed the canary scan"],
  ["StagingAuthority", "the P2-G4 secret that decides whether a sandboxed worker's staged bytes become a result"],
  ["StagedOutput", "the bytes a sandboxed worker wrote, before anything has accepted them"],
  ["AcceptedOutput", "the same bytes after the core accepted them"],
  [
    "CaptureBytes",
    "the P2-L2 lecture audio chunk or board photograph a capture holds, as it arrived",
  ],
  ["RawSnapshot", "the retained bytes of one official-source retrieval, which leave it only as an Untrusted<IngestedDocument>"],
  ["FetchOutcome", "the bytes one conditional fetch or one user-supplied import produced, before anything has stored them"],
  // `P2-L3`. Five types that hold the lecture itself: the two admitted capture
  // buffers, the raw provider response, and the two decoded records whose text
  // is what the lecturer said. `S-10`'s decision for that crate is made in the
  // strengthening direction -- every one hand-writes a redacting `Debug` and
  // none of them writes a `PUBLIC_BYTES` entry -- so widening the vocabulary
  // later costs nothing here.
  ["AuthorizedChunk", "the P2-L2 lecture audio chunk a transcription job was authorized to read"],
  ["AuthorizedCapture", "the P2-L2 board photograph a transcription job was authorized to read"],
  ["ProviderResponse", "a speech-to-text provider's answer, which is the lecture in words"],
  ["RawToken", "one word the lecturer said, exactly as the provider returned it"],
  ["RawSegment", "one span of the lecture, its verbatim text and its tokens"],
  ["CorrectionCandidate", "the word somebody proposes one token should read instead"],
  ["EffectiveToken", "one word of the lecture as a transcript version reads it"],
  // `P2-L4`. Five types that hold the lecture in words on the document side:
  // what a caller offers the builder, what the builder admits, the document
  // itself, and the two study-index types whose headings are written over the
  // lecture and can quote it. `S-10`'s decision for that crate is made in the
  // same strengthening direction `P2-L3` chose -- every one hand-writes a
  // redacting `Debug` that reaches its text through a length only -- so a later
  // widening of the field-name vocabulary costs nothing here.
  ["NodeDraft", "the rendered lecture text a caller offers the document builder"],
  ["DocumentNode", "one rendered span of the lecture, as the document holds it"],
  ["LectureDocument", "the whole lossless rendering of one lecture"],
  ["StudyIndexEntry", "a heading a summary wrote over a span of the lecture"],
  ["StudyIndex", "every heading of one summary over one lecture"],
  // `P2-RF13`. Five types the whole-set classification of byte fields found
  // deriving `Debug` over bytes the name alternation did not read. The two
  // capture-gate types hold the lecture itself, one crate away from the
  // `CaptureBytes` `P2-L2` sealed; the three key-wrapping types hold a wrapped
  // root or Vault Master Key, which is one broker call or one passphrase from
  // the key.
  ["CaptureSession", "every chunk of lecture audio or board photography this session has accepted"],
  ["ReleasableArtifact", "the same capture after every chunk re-bound against its permission"],
  ["RecipientRecord", "one wrapped copy of the Vault Master Key, and the keystore blob that opens it"],
  ["BackupRecipientRecord", "one wrapped copy of the backup root"],
  ["BackupPlan", "the canonical CBOR of the recipient records a restore recovers the Vault Master Key from"],
  // `P2-RF15`. Seven of the eight registrations the repaired guard demanded:
  // each already hand-writes a redacting `Debug`, and each was outside the
  // registry because the guard read an impl only when it spelled a marker.
  // Their crates each wrote the redaction deliberately -- the doc comment above
  // every one of them says so -- and then nothing recorded that the type was
  // covered. The eighth was `Digest32`, which is a public tuple newtype and is
  // declared in `PUBLIC_TUPLE_BYTES` rather than registered.
  // `AppliedCorrection` is not one the guard demanded, because its secret is
  // text: it derived `Debug` over a word of the lecture, one type away from the
  // `CorrectionCandidate` this crate had already sealed.
  ["SourceEntry", "one in-memory source file of a repository tree, as its bytes were read"],
  ["SourceUnit", "the same bytes on the analysis side, after the frozen manifest admitted them"],
  ["IngestedDocument", "one parsed document's untrusted bytes, which every quotation is a span of"],
  ["ModelOutput", "the untrusted bytes one model wrote, before anything has accepted them"],
  ["FineGrainedToken", "the secret half of a GitHub fine-grained token"],
  ["SealedCredential", "the operating-system keystore blob that opens one stored credential"],
  ["Acquisition", "the FetchOutcome bytes one conditional fetch or one user-supplied import produced"],
  ["AppliedCorrection", "what one token of the lecture read before a correction replaced it"],
  // `P2-RF17`. The whole-set text classification's own finding: section 12.4's
  // annotation layer derived `Debug` over a rendering written across a range of
  // raw tokens, in the crate that had already sealed the tokens themselves.
  ["Annotation", "the rendering section 12.4's annotation layer writes over a range of raw tokens"],
]);

/**
 * Field names that mean key material, plaintext, or transient egress payload
 * when they hold raw bytes. `T126` injected `AuditRow.payload_bytes`; the old
 * vocabulary did not include payload/prompt/response names and the injection
 * passed, so those names live in this generic discovery net rather than in a
 * second audit-only scanner.
 * `blob` was added by `P2-R1`, which is the first task to hold one in a struct
 * field: what an operating-system key broker returns is half of recovering the
 * secret it holds. It is on the `S-10` list of names this net trailed the code
 * by, and it is the only one of them whose measured cost is a single site --
 * `SealedCredential.blob` in `crates/repository/src/github.rs`, which
 * hand-writes a redacting `Debug` -- so widening by it needed no redaction work
 * in another crate's contract and no `PUBLIC_BYTES` entry. The other five names
 * that row measures are not added here; their cost is on the row.
 *
 * A name is only a signal; the exceptions below carry the judgement.
 *
 * `P2-RF13` demoted this list to the **weakest of the three layers** and to
 * `String` and `str` fields alone. It used to decide byte buffers too, and
 * `T166` measured what that cost: `excerpt: Vec<u8>` under a derived `Debug`
 * passed, because the list does not name `excerpt` and nothing else looked. A
 * byte buffer is now judged by its type and its entry in
 * {@link BYTE_FIELD_CLASSES}, whatever it is called. Adding a name here closes
 * nothing on its own and must not be offered as a repair for a byte buffer
 * that leaked -- `S-10` on `docs/contracts/policy-source-scans.md` records
 * five rounds of that.
 */
const SECRET_FIELD_NAMES =
  /^_?(dek|kek|key|keys|key_bytes|key_material|material|secret|secrets|secret_bytes|plaintext|plaintext_bytes|plain|payload|payload_bytes|prompt|prompt_text|provider_response|provider_response_bytes|response_text|transmitted|transmitted_bytes|transmission|transmission_bytes|source_bytes|digest|seed|chunk|chunk_bytes|hex|raw|passphrase|password|phrase|mnemonic|entropy|opened|blob|vmk|skey|master|student_number|student_name)$/;

/**
 * Field types that hold bytes transparently, so a derived `Debug` prints them.
 *
 * Matched against a *normalized* spelling: `normalizeFieldType` strips a
 * borrow, an `Option`/`Box`/`Zeroizing` wrapper, and any path qualification, so
 * `&'a [u8]`, `Option<Vec<u8>>`, and `zeroize::Zeroizing<Vec<u8>>` reach it as
 * the byte buffer each one is. `T114` found all three silent.
 */
const RAW_BYTE_TYPES =
  /^(Vec\s*<\s*u8\s*>|\[\s*u8\s*;[^\]]*\]|\[\s*u8\s*\]|String|str)$/;

/**
 * The subset of {@link RAW_BYTE_TYPES} a *tuple* position carries alone.
 *
 * A named field is judged by its name and its type together, and the name is
 * what tells `Qualifier.key: String` from a key. A tuple position has no name,
 * so the type has to carry the whole signal: a byte buffer does, and `String`
 * and `str` do not — every error type in this workspace reports through one and
 * no key in `P2-K1`'s schedule is text. `T116` found `AuditLeak(Vec<u8>)`
 * silent.
 */
const RAW_BYTE_PAYLOAD_TYPES =
  /^(Vec\s*<\s*u8\s*>|\[\s*u8\s*;[^\]]*\]|\[\s*u8\s*\])$/;

/**
 * Fields the *name* layer matches whose text is not secret. Each entry states
 * why, because the reason is the whole content of the exception.
 *
 * Byte buffers are not excepted here any more: their judgement is the class in
 * {@link BYTE_FIELD_CLASSES}, which every one of them must carry. What is left
 * is `String` and `str` -- the fields whose type cannot say what they hold, so
 * that a `Qualifier.key` is told from a key by its name and nothing else.
 * `KeyMaterialState.digest` and `StreamingPrefix.digest` moved out of this map
 * and into that classification when `P2-RF13` made the type decide.
 */
const PUBLIC_BYTES = new Map([
  [
    "Qualifier.key",
    "a qualifier name from the predicate registry's closed schema, not a cryptographic key",
  ],
  [
    "RegistryError.key",
    "the qualifier name a rejected assertion used, reported so the caller can fix it",
  ],
  [
    "QualifierSchema.key",
    "the qualifier name a predicate schema declares, which is the registry's public vocabulary",
  ],
  // `P2-G5`. Four SHA-256 fields over ingested or model-written bytes. A digest
  // of untrusted content is not the content, `Untrusted::digest` returns it
  // through the public API, and the rendered data record carries it in the
  // clear so a model can cite a span. What the boundary hides is the bytes, and
  // every field holding those is reduced to a length by a hand-written `Debug`.
  [
    "QuotedDocument.digest",
    "SHA-256 over one ingested document, which the rendered data record carries in the clear so a model can cite a span of it",
  ],
  [
    "QuarantinedOutput.digest",
    "SHA-256 over a refused model output, which is the only identity the quarantine record keeps of bytes it deliberately does not hold",
  ],
  [
    "ResolvedSpan.digest",
    "SHA-256 truncated to 128 bits over one cited range of an already-ingested document, which is what makes the citation checkable",
  ],
  [
    "FixtureContract.payload",
    "the public encoding label (for example academic.event-batch/v3 deterministic-cbor), not fixture payload bytes",
  ],
  [
    "AcceptedResponse.digest",
    "SHA-256 of a provider response, which the egress_audit row carries in the clear as provider_response_digest",
  ],
  [
    "Issued.digest",
    "SHA-256 of a P2-G4 capability descriptor, whose whole plaintext the parent writes into the job's staged input directory for the sandboxed process to read; the digest is what the registry compares, not a secret it holds",
  ],
]);

/**
 * What every named byte-buffer field in the workspace holds.
 *
 * This is the whole set half 4 compares against, in both directions: a field
 * whose declared type normalizes to `Vec<u8>`, `[u8; N]` or `[u8]` must be
 * here, and an entry naming a field that no longer exists must go. A new byte
 * buffer therefore fails this file until somebody says what it holds --
 * whatever it is called. `T166` measured the alternation this replaces
 * admitting `excerpt: Vec<u8>` under a derived `Debug`.
 *
 * The second column is one of {@link BYTE_CLASSES} and nothing else, so it is
 * a classification and not a sentence somebody can make fit. Two classes --
 * `key-material` and `content` -- forbid a derived `Debug`; the other nine say
 * where the bytes already are in the clear. `String` and `str` fields are not
 * here: their type does not say what they hold, so they stay with the name
 * alternation, which is the weakest layer and is documented as such.
 */
const BYTE_CLASSES = new Map([
  ["identifier", "an opaque identity a row, a header or a path already carries in the clear"],
  ["digest", "a cryptographic hash; the bytes it was taken over are not recoverable from it"],
  ["nonce", "public per-encryption randomness, which is not a key and is stored beside the ciphertext"],
  ["salt", "a public KDF input, which is not a key and is stored beside the record it derives"],
  ["signature", "a signature, or the public verifying half of a signing key; the private half is elsewhere"],
  ["mac", "an authentication tag over a record whose own fields are classified here"],
  ["locator", "an address into already-stored data, not the data: the domain-keyed header HMAC, or a span into a stored row"],
  ["ciphertext", "bytes under an AEAD whose key is not in the same value"],
  ["canonical-encoding", "the deterministic encoding of a structure whose fields are themselves classified here"],
  ["mask", "a bitmask over a closed vocabulary"],
  ["public-fixture", "bytes this repository commits in the clear as a test corpus"],
  ["key-material", "key bytes, raw or wrapped -- a derived Debug is forbidden"],
  ["content", "document, capture, transcript, prompt or provider bytes -- a derived Debug is forbidden"],
]);

/** The two classes whose bytes a derived `Debug` may not reach. */
const SECRET_BYTE_CLASSES = new Set(["key-material", "content"]);

const BYTE_FIELD_CLASSES = new Map([
  ["AcceptanceCommand.client_instance_id", "identifier"],
  ["AcceptanceCommand.envelope_bytes", "canonical-encoding"],
  ["AcceptanceCommand.idempotency_key", "identifier"],
  ["AcceptanceCommand.request_id", "identifier"],
  ["AcceptedOutput.bytes", "content"],
  ["AcceptedResponse.payload", "content"],
  ["AggregateClosureRow.aggregate_id", "identifier"],
  ["AggregateClosureRow.parent", "identifier"],
  ["AggregateTimelineRow.aggregate_id", "identifier"],
  ["AggregateTimelineRow.registered_event_id", "identifier"],
  ["AuthorizedCapture.chunk_bytes", "content"],
  ["AuthorizedChunk.chunk_bytes", "content"],
  ["AuthorizedToolCall.payload", "content"],
  // `P2-RF13`. Canonical CBOR of this profile's recovery-class recipient
  // records, and a recipient record is one wrapped copy of the Vault Master
  // Key. Reached through `&[u8]`, so the wrapping does not make it printable.
  ["BackupPlan.profile_recovery_recipients", "key-material"],
  ["BackupRecipientRecord.recipient_id", "identifier"],
  ["BackupRecipientRecord.record_mac", "mac"],
  ["BackupRecipientRecord.salt", "salt"],
  ["BackupRecipientRecord.wrap_nonce", "nonce"],
  ["BackupRecipientRecord.wrapped_root", "key-material"],
  ["CaptureBytes.chunk_bytes", "content"],
  // `P2-RF13`. The same lecture audio and board photographs `P2-L2` sealed in
  // `academic-capture`, held one crate away under a name the alternation did
  // not read. `ReleasableArtifact.bytes` below is the released half of it.
  ["CaptureSession.bytes", "content"],
  ["CorpusFile.bytes", "public-fixture"],
  ["Cursor.body", "canonical-encoding"],
  ["DbFaultState.receipt", "canonical-encoding"],
  ["DecodedEnvelope.payload", "canonical-encoding"],
  ["DecodedEnvelope.public_key", "signature"],
  ["DecodedEnvelope.signature", "signature"],
  ["DecodedPayload.spec_digest", "digest"],
  ["DescriptorMigration.artifact_id", "identifier"],
  ["DescriptorMigration.retention_action_id", "identifier"],
  ["DesktopCommand.backup_receipt_id", "identifier"],
  ["DispositionRecord.record_digest", "digest"],
  ["DomainKeyring.keys", "key-material"],
  ["DurableAcceptanceReceipt.response_bytes", "canonical-encoding"],
  ["EncryptedObjectReader.chunk", "content"],
  ["ExactLocator.locator_payload", "locator"],
  ["FetchOutcome.source_bytes", "content"],
  ["FileIdentity.file_id", "identifier"],
  ["FineGrainedToken.secret", "key-material"],
  ["FingerprintEncoder.bytes", "canonical-encoding"],
  ["FixtureContext.envelope", "canonical-encoding"],
  ["KeyMaterialState.digest", "digest"],
  ["LedgerState.registrations", "identifier"],
  ["MaterializedSnapshot.snapshot_id", "identifier"],
  ["ObjectHeader.artifact_id", "identifier"],
  ["ObjectHeader.base_nonce", "nonce"],
  ["ObjectHeader.domain_id", "identifier"],
  ["ObjectHeader.locator", "locator"],
  ["ObjectHeader.permission_lineage_id", "identifier"],
  ["ObjectHeader.streaming_prefix_digest", "digest"],
  ["OpenedHeader.dek", "key-material"],
  ["OpenedHeader.plaintext_digest", "digest"],
  ["OutboxEntry.event_kind_mask", "mask"],
  ["PlannedAction.locator", "locator"],
  ["PlatformRow.build_digest", "digest"],
  ["PlatformRow.fault_matrix_digest", "digest"],
  ["PlatformRow.independent_restore_digest", "digest"],
  ["PlatformRow.spec_digest", "digest"],
  ["Preview.payload", "content"],
  ["ProcessActivity.transmitted_bytes", "content"],
  // ADR-005's public generation name: SHA-256 of an HKDF output, readable on a
  // locked profile and structurally not usable as a key. `KeyGeneration` in
  // PUBLIC_TUPLE_BYTES is the same bytes as a newtype.
  ["ProfileKeys.generation", "identifier"],
  ["ProjectionEvidenceLocator.locator_payload", "locator"],
  ["ProtoSha256Digest.value", "digest"],
  ["ProtoUuidV7.value", "identifier"],
  ["ProviderResponse.provider_response_bytes", "content"],
  ["RawActive.active_policy_hash", "digest"],
  ["RawActive.active_source_digest", "digest"],
  ["RawActive.checksum", "digest"],
  ["RawActive.cursor_policy_hash", "digest"],
  ["RawActive.cursor_source_digest", "digest"],
  ["RawActive.generation_id", "identifier"],
  ["RawActive.generation_policy_hash", "digest"],
  ["RawActive.generation_source_digest", "digest"],
  ["RawGeneration.builder_digest", "digest"],
  ["RawGeneration.checksum", "digest"],
  ["RawGeneration.config_hash", "digest"],
  ["RawGeneration.domain", "identifier"],
  ["RawGeneration.policy_registry_hash", "digest"],
  ["RawGeneration.source_ledger_digest", "digest"],
  ["RawSnapshot.source_bytes", "content"],
  ["RecipientParameters.salt", "salt"],
  // `P2-RF13`. `RecipientRecord` is "one wrapped copy of the Vault Master
  // Key". `keystore_blob` is what an operating-system key broker returns,
  // which is the thing `P2-R1` put `blob` in the vocabulary for -- and the
  // alternation missed this one because of the four characters in front of it.
  ["RecipientRecord.keystore_blob", "key-material"],
  ["RecipientRecord.recipient_id", "identifier"],
  ["RecipientRecord.record_mac", "mac"],
  ["RecipientRecord.wrap_nonce", "nonce"],
  ["RecipientRecord.wrapped_vmk", "key-material"],
  ["ReconciledTranscript.reference_identity_digest", "digest"],
  ["RedactedProjection.source_digest", "digest"],
  ["Redaction.payload", "content"],
  ["Registered.version_event", "identifier"],
  ["RehearsalObservations.restored_canonical_semantic_digest", "digest"],
  ["RehearsalReceipt.key_material_digest", "digest"],
  ["RehearsalReceipt.receipt_mac", "mac"],
  ["RehearsalReceipt.restored_canonical_semantic_digest", "digest"],
  ["ReleasableArtifact.bytes", "content"],
  ["RetentionSubject.locator", "locator"],
  ["RotationUnit.source_locator", "locator"],
  ["RotationUnit.unit_id", "identifier"],
  ["RuntimeToolCall.payload", "content"],
  ["SchemaFingerprint.canonical_bytes", "canonical-encoding"],
  ["SchemaIdentity.creating_build_digest", "digest"],
  ["SchemaIdentity.format_uuid", "identifier"],
  ["SealedCredential.blob", "key-material"],
  ["SealedManifest.ciphertext", "ciphertext"],
  ["SealedManifest.nonce", "nonce"],
  ["SealedManifest.signature", "signature"],
  ["SealedManifest.verifying_key", "signature"],
  ["ShredReceipt.locator", "locator"],
  ["SnapshotAggregateRow.aggregate_id", "identifier"],
  ["SnapshotAggregateRow.registered_event_id", "identifier"],
  ["SourceDocument.payload", "content"],
  ["SourceEntry.source_bytes", "content"],
  ["SourceUnit.source_bytes", "content"],
  ["StagedOutput.bytes", "content"],
  ["StagingAuthority.secret", "key-material"],
  ["StoredBatchMaterial.deterministic_payload", "canonical-encoding"],
  ["StoredBatchMaterial.signature", "signature"],
  ["StoredBatchMaterial.signed_envelope", "canonical-encoding"],
  ["StoredBatchMaterial.signing_public_key", "signature"],
  ["StoredEvent.canonical_payload", "canonical-encoding"],
  ["StoredEvent.event_id", "identifier"],
  ["StoredEvent.payload_hash", "digest"],
  ["StreamingPrefix.bytes", "canonical-encoding"],
  ["StreamingPrefix.digest", "digest"],
  ["SubmittedRequest.client_instance_id", "identifier"],
  ["SubmittedRequest.idempotency_key", "identifier"],
  ["SubmittedRequest.request_digest", "digest"],
  ["SubmittedRequest.request_id", "identifier"],
  ["SyntheticTranscriptPdf.bytes", "public-fixture"],
  ["TranscriptChecksums.identity_digest", "digest"],
  ["TranscriptChecksums.rows", "digest"],
  ["VerifiedBatch.signature", "signature"],
  ["VerifiedBatch.source_envelope", "canonical-encoding"],
  ["VerifiedBatch.source_payload", "canonical-encoding"],
  ["Wanted.artifact", "identifier"],
  ["Wanted.locators", "locator"],
  ["WireField.bytes", "canonical-encoding"],
]);

/**
 * Classification keys whose type name is declared in more than one crate.
 *
 * `BYTE_FIELD_CLASSES` is keyed by `Type.field` and not by path, so two types
 * that happen to share a name share one classification. `P2-L4` made that
 * concrete rather than hypothetical: it added a second `CorpusFile` with a
 * second `bytes`, and that field arrived already classified by a line written
 * for a different crate's type. Both are committed corpus files and the class
 * is right, but nobody decided it was right -- which is the shape this whole
 * file exists to refuse.
 *
 * So a key that spans crates is not forbidden, it is *declared*, and the set is
 * compared in both directions. A third `CorpusFile.bytes` in a crate where the
 * bytes are not a committed fixture fails here until somebody says so.
 */
const SHARED_BYTE_FIELD_KEYS = new Map([
  [
    "CorpusFile.bytes",
    "`academic-record`'s and `P2-L4`'s harnesses each declare one; both are files this repository commits in the clear as a corpus, which is the `public-fixture` class both need",
  ],
  [
    "DecodedEnvelope.payload",
    "`academic-admission` and `academic-contracts` each decode the same wire envelope; both hold the canonical encoding and neither holds a key",
  ],
  [
    "DecodedEnvelope.public_key",
    "the same two decoders, each holding the public verifying half; the private half is in neither",
  ],
  [
    "DecodedEnvelope.signature",
    "the same two decoders, each holding the signature over that envelope",
  ],
]);

/**
 * Reports whether a declared type carries bytes the classification must cover.
 *
 * Both the buffer itself and a buffer inside a container: `Vec<[u8; 32]>` and
 * `BTreeMap<DomainId, Vec<u8>>` print their contents through a derived `Debug`
 * exactly as a bare `Vec<u8>` does, and an exact type match reaches neither.
 * `P2-RF13` measured five such fields in the workspace, two of them under a
 * derived `Debug`; they are classified below like every other buffer.
 *
 * `String` and `str` are deliberately not here. Their type does not say what
 * they hold, so they stay with the name alternation -- the weakest layer.
 */
function isClassifiedByteBuffer(text) {
  const normalized = normalizeFieldType(text);
  return (
    RAW_BYTE_PAYLOAD_TYPES.test(normalized) ||
    /(Vec\s*<\s*u8\s*>|\[\s*u8\s*(;[^\]]*)?\])/.test(normalized)
  );
}

/**
 * What a `String`/`str` field holds, for the crates that classify text.
 *
 * The same closed vocabulary {@link BYTE_CLASSES} is, for the layer that has no
 * type to read. One class -- `content` -- forbids a derived `Debug`; the rest
 * say why the text is already in the clear.
 */
const TEXT_CLASSES = new Map([
  ["identifier", "an opaque identity a row, a path or a header already carries in the clear"],
  ["vocabulary", "a value from a closed set this workspace declares, or the spelling of one"],
  ["provenance", "who or what produced a record: a provider name, a model name, a version, a tool label"],
  ["diagnostic", "an error, a reason or a refusal, whose whole purpose is to be read"],
  ["reference", "a name for something outside this value: a locale, a mime type, a file name, a heading key"],
  ["content", "document, capture, transcript or provider text -- a derived Debug is forbidden"],
]);

/** The class whose text a derived `Debug` may not reach. */
const SECRET_TEXT_CLASSES = new Set(["content"]);

/**
 * The crates whose every named `String`/`str` field carries a class.
 *
 * `S-18` on `docs/contracts/policy-source-scans.md` is the text half of the
 * gap `P2-RF13` closed for bytes, and its cost is why this is a *set of crates*
 * rather than the workspace: 805 named text fields against the 137 byte fields
 * `P2-RF13` classified, and what a text field holds is a judgement its crate's
 * owner makes. A crate joins this map when its owner has made those judgements
 * for all of its fields; until then the crate stays on the
 * {@link SECRET_FIELD_NAMES} alternation, which is documented as the weakest
 * layer and is the reason `S-18` exists.
 *
 * `P2-RF17` enters the two crates that hold the ten registrations `T177`
 * measured inert. Their secret is text under a field name the alternation does
 * not carry, so nothing required their registration and deleting one was
 * silent in both directions.
 */
const TEXT_CLASSIFIED_CRATES = new Map([
  [
    "transcription",
    "`P2-L3`: every raw token, segment, correction and effective token is a word the lecturer said",
  ],
]);

const TEXT_FIELD_CLASSES = new Map([
  // `crates/transcription` -- `P2-L3`. A token, a segment's verbatim text, a
  // proposed replacement and what it replaced are all words the lecturer said.
  // `Annotation.rendering` is `content` in the strengthening direction this
  // crate already chose twice: three of section 12.4's four kinds render a
  // mark, a boundary or a closed speaker shape, but `MathFormatting` renders
  // notation written over a token range, the field is one caller-supplied
  // `String` shared by all four kinds, and the type is public. Nothing in the
  // crate needs to print it, so sealing it costs nothing and leaving it open
  // would be a judgement made by omission.
  ["Annotation.rendering", "content"],
  ["AppliedCorrection.previous_text", "content"],
  ["AudioFormat.container", "reference"],
  ["CorrectionCandidate.replacement_text", "content"],
  ["EffectiveToken.effective_text", "content"],
  ["OpenSegment.id", "identifier"],
  ["OpenSegment.verbatim_text", "content"],
  ["RawSegment.id", "identifier"],
  ["RawSegment.verbatim_text", "content"],
  ["RawToken.text", "content"],
  ["SuppliedMaterial.identifier", "identifier"],
]);

/**
 * Reports whether a declared type carries text the classification must cover.
 *
 * Both the bare field and one inside a container: `Vec<String>` and
 * `BTreeMap<String, Span>` print their contents through a derived `Debug`
 * exactly as a bare `String` does, which is the rule
 * {@link isClassifiedByteBuffer} applies to bytes.
 */
function isClassifiedText(text) {
  return /\b(String|str)\b/.test(normalizeFieldType(text));
}

/** The crate one definition site is declared in. */
function crateOf(site) {
  return site.location.split("/")[1];
}

/**
 * Reports whether a named field is one a derived `Debug` may not print.
 *
 * Bytes are judged by {@link BYTE_FIELD_CLASSES} whatever they are called. Text
 * is judged by {@link TEXT_FIELD_CLASSES} in a crate that classifies text, and
 * by the {@link SECRET_FIELD_NAMES} alternation everywhere else.
 */
function fieldIsSecret(site, name, field, declared) {
  return (
    !PUBLIC_BYTES.has(`${name}.${field}`) &&
    fieldIsSecretBeforeExceptions(site, name, field, declared)
  );
}

/**
 * The same judgement without the {@link PUBLIC_BYTES} exception applied.
 *
 * The derived-`Debug` guard needs the un-excepted answer so it can record which
 * exceptions were exercised and delete the ones that match nothing.
 */
function fieldIsSecretBeforeExceptions(site, name, field, declared) {
  if (isClassifiedByteBuffer(declared)) {
    const assigned = BYTE_FIELD_CLASSES.get(`${name}.${field}`);
    return assigned === undefined || SECRET_BYTE_CLASSES.has(assigned);
  }
  if (TEXT_CLASSIFIED_CRATES.has(crateOf(site)) && isClassifiedText(declared)) {
    const assigned = TEXT_FIELD_CLASSES.get(`${name}.${field}`);
    return assigned === undefined || SECRET_TEXT_CLASSES.has(assigned);
  }
  return SECRET_FIELD_NAMES.test(field) && holdsRawBytes(declared);
}

/**
 * Tuple newtypes whose whole payload is a public identifier or digest.
 *
 * The payload check below cannot read a field name, so every `[u8; N]` newtype
 * reaches it and each one needs a judgement recorded here. Each entry states
 * where those bytes already are in the clear; a new newtype is refused until
 * someone writes that sentence, which is the point.
 */
const PUBLIC_TUPLE_BYTES = new Map([
  [
    "ProfileId",
    "the profile identity every key derivation is salted with and every profile path spells",
  ],
  [
    "DomainId",
    "the domain identity written in the clear at a fixed offset of every object header",
  ],
  [
    "ContentDigest",
    "SHA-256 over plaintext or canonical semantic bytes, which the signed artifact_descriptor row and a backup manifest's plaintext_sha256 both carry in the clear",
  ],
  [
    "VaultLocator",
    "the domain-keyed HMAC written in the clear at a fixed header offset and spelled into the object's own path",
  ],
  [
    "KeyGeneration",
    "the public generation name from ADR-005: SHA-256 of an HKDF output, readable on a locked profile and structurally not usable as a key",
  ],
  ["RotationId", "the rotation identity the journal records in the clear"],
  ["ActionId", "one retention action's identity, which the store's retention_action row carries"],
  [
    "BackupSetId",
    "the backup set identity, which is the HKDF salt written into the backup's own manifest",
  ],
  ["GenerationId", "an opaque identifier for one disposable projection generation"],
  [
    "ModelRunId",
    "one model execution's identity, which the store's model_run and model_run_provenance rows carry as a BLOB primary key",
  ],
  [
    "ArtifactId",
    "the artifact identity artifact_descriptor already carries in the clear; academic-model-run names inputs and outputs by it and holds no artifact byte",
  ],
  [
    "CandidateId",
    "one reanalysis candidate's identity, which the store's model_run_candidate row carries as a BLOB primary key; the candidate value itself is a digest and not in this newtype",
  ],
  ["OpaqueId", "an RPC wire identifier carried with no narrowing"],
  ["IdempotencyKey", "an RPC retry key, which the caller supplies and the wire carries"],
  [
    "SessionNonce",
    "the P1 handshake nonce, whose hex `capability_id` and `as_hex` publish into the owner-only runtime metadata the same user reads; it is not ADR-005 key material and what confines it is that directory",
  ],
  [
    "Digest32",
    "SHA-256 over a model run's inputs or outputs, which the model_run and model_run_provenance rows carry as a BLOB and which `to_lower_hex` publishes",
  ],
]);

// `#[cfg_attr(<cfg>, derive(Debug))]` derives exactly as `#[derive(Debug)]`
// does whenever the cfg holds. `T114` found the guard silent on it.
const DERIVE_PATTERN =
  /#\s*\[\s*(?:cfg_attr\s*\([\s\S]*?,\s*)?derive\s*\(([\s\S]*?)\)\s*\)?\s*\]/g;
const DEFINITION_PATTERN =
  /^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(struct|enum|union)\s+([A-Za-z_][A-Za-z0-9_]*)/;
// Anchored on the separator rather than on a line start: `struct X { key:
// Vec<u8>, }` written on one line is the same declaration as the rustfmt
// spelling, and `T114` found the guard reading only the second.
const NAMED_FIELD_PATTERN =
  /(?:^|[{,])\s*(?:pub(?:\s*\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*([^\n]+)/gm;

/**
 * Trims a captured declaration to the type itself.
 *
 * A field type is captured to the end of its line, because stopping at the
 * first comma reads `BTreeMap<DomainId, Vec<u8>>` as `BTreeMap<DomainId` and
 * loses the buffer -- which is how `T114`'s `DomainKeyring` injection stayed
 * silent. The trailing separator is removed here, at the first comma that is
 * not inside a bracket.
 */
function trimDeclaredType(text) {
  let depth = 0;
  for (let cursor = 0; cursor < text.length; cursor += 1) {
    const character = text[cursor];
    if (character === "<" || character === "(" || character === "[") {
      depth += 1;
    } else if (character === ">" || character === ")" || character === "]") {
      depth -= 1;
      if (depth < 0) {
        return text.slice(0, cursor);
      }
    } else if (depth === 0 && character === ",") {
      return text.slice(0, cursor);
    }
  }
  return text;
}

/** Payload positions of a tuple struct, which has no field name at all. */
const TUPLE_FIELD_PATTERN = /(?:^|[(,])\s*(?:pub(?:\s*\([^)]*\))?\s+)?([^,()]+)/g;

/**
 * Enum variants, spelled `Type::Variant`, whose byte payload is public.
 *
 * The check that reaches these payloads does not read the variant's name, so
 * each entry must state why its bytes may be printed rather than be excused by
 * being called something bland.
 */
const PUBLIC_TUPLE_VARIANT_BYTES = new Map([
  [
    "AcceptancePublicKey::Provisioned",
    "the user's Ed25519 public half compiled as receipt-verification authority; the private half is offline and absent",
  ],
]);

/** Enum variants written as `Dek([u8; 32])`, whose name is the only signal. */
const TUPLE_VARIANT_PATTERN = /(?:^|[,{])\s*([A-Z][A-Za-z0-9_]*)\s*\(([^()]*)\)/gm;

/**
 * Reduces a field type to the buffer it holds.
 *
 * A borrow, a path qualification, and an `Option`, `Box`, `Rc`, `Arc`,
 * `Zeroizing`, or `Cow` wrapper all print their contents through a derived
 * `Debug`, so none of them makes a byte buffer safe. `T114` found each one
 * silent in turn.
 */
function normalizeFieldType(text) {
  let current = text.trim().replace(/\s+/g, " ");
  for (let round = 0; round < 8; round += 1) {
    const before = current;
    current = current.replace(/^&\s*(?:'[A-Za-z_][A-Za-z0-9_]*\s*)?(?:mut\s+)?/, "");
    current = current.replace(/^(?:[A-Za-z_][A-Za-z0-9_]*\s*::\s*)+/, "");
    const wrapped = /^(Option|Box|Rc|Arc|Zeroizing|Cow)\s*<([\s\S]*)>$/.exec(current);
    if (wrapped !== null) {
      current = wrapped[2].trim().replace(/^'[A-Za-z_][A-Za-z0-9_]*\s*,\s*/, "");
    }
    if (current === before) {
      return current;
    }
  }
  return current;
}

/** Reports whether a field type prints raw bytes through a derived `Debug`. */
function holdsRawBytes(text) {
  return RAW_BYTE_TYPES.test(normalizeFieldType(text));
}

async function rustSourcesUnder(directory) {
  const found = [];
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      found.push(...(await rustSourcesUnder(path)));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      found.push(path);
    }
  }
  return found;
}

/** Package subdirectories that hold no product code, and are not scanned. */
const NON_PRODUCT_TREES = new Set(["tests"]);

async function productSources() {
  const crates = await readdir(CRATES_ROOT, { withFileTypes: true });
  const sources = [];
  for (const crate of crates) {
    if (!crate.isDirectory()) {
      continue;
    }
    const root = join(CRATES_ROOT, crate.name);
    const entries = await readdir(root, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.isDirectory() && NON_PRODUCT_TREES.has(entry.name)) {
        continue;
      }
      const path = join(root, entry.name);
      if (entry.isDirectory()) {
        sources.push(...(await rustSourcesUnder(path)));
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        sources.push(path);
      }
    }
  }
  return sources.sort();
}

/** Returns the attribute block immediately above `index`, if any. */
function attributesAbove(lines, index) {
  const collected = [];
  for (let cursor = index - 1; cursor >= 0; cursor -= 1) {
    const trimmed = lines[cursor].trim();
    if (trimmed === "") {
      break;
    }
    // Doc comments sit between a derive and its definition often enough that
    // stopping at one would miss the derive; a line that is neither a comment
    // nor part of an attribute ends the block.
    const isAttributePart =
      trimmed.startsWith("//") ||
      trimmed.startsWith("#") ||
      trimmed.startsWith("]") ||
      trimmed.endsWith(",") ||
      trimmed.endsWith("(");
    if (!isAttributePart) {
      break;
    }
    collected.unshift(lines[cursor]);
  }
  return collected.join("\n");
}

/** Returns the brace-matched body of the definition starting at `index`. */
function bodyAt(lines, index) {
  const text = lines.slice(index, index + 400).join("\n");
  const opener = text.search(/[{(;]/);
  if (opener === -1 || text[opener] !== "{") {
    return "";
  }
  return matched(text, opener, "{", "}");
}

/** Returns the parenthesised body of a tuple struct starting at `index`. */
function tupleBodyAt(lines, index) {
  const text = lines.slice(index, index + 40).join("\n");
  const opener = text.search(/[{(;]/);
  if (opener === -1 || text[opener] !== "(") {
    return "";
  }
  return matched(text, opener, "(", ")");
}

function matched(text, opener, open, close) {
  let depth = 0;
  for (let cursor = opener; cursor < text.length; cursor += 1) {
    if (text[cursor] === open) {
      depth += 1;
    } else if (text[cursor] === close) {
      depth -= 1;
      if (depth === 0) {
        return text.slice(opener + 1, cursor);
      }
    }
  }
  return text.slice(opener + 1);
}

/**
 * Returns the body of every hand-written `Debug` *and* `Display` impl, by type.
 *
 * Both, because `Display` prints too. The registry test reads derives only, and
 * `handWrittenDebug` recorded a `Display` impl as satisfying the "has a
 * hand-written Debug" requirement while nothing ever read what it printed —
 * `T118` injected a `Display` on the registered `OpenedHeader` that wrote
 * `self.dek` and every test passed. A type with both impls has them
 * concatenated here: each one is scanned, and neither may reach the bytes.
 */
function handWrittenFormatterBodies(contents) {
  const bodies = new Map();
  const pattern =
    /\bimpl\s+(?:core::fmt::|std::fmt::|fmt::)?(?:Debug|Display)\s+for\s+([A-Za-z_][A-Za-z0-9_]*)/g;
  for (const match of contents.matchAll(pattern)) {
    const opener = contents.indexOf("{", match.index + match[0].length);
    if (opener === -1) {
      continue;
    }
    const body = matched(contents, opener, "{", "}");
    const existing = bodies.get(match[1]);
    bodies.set(match[1], existing === undefined ? body : `${existing}\n${body}`);
  }
  return bodies;
}

/**
 * Names a formatter body binds by destructuring `self`, mapped to the field
 * each one stands for.
 *
 * `let Self { dek } = self;` and `let Self { dek: bytes, .. } = self;` both put
 * a secret field behind a bare identifier, and a net that greps `self.dek`
 * reads neither. `T118` injected the first and nothing failed.
 */
const DESTRUCTURE_PATTERN =
  /let\s+(?:Self|[A-Z][A-Za-z0-9_]*)\s*\{([^}]*)\}\s*=\s*(?:\*?self|&\s*\*?self)\b\s*;?/g;

function destructuredBindings(body) {
  const bindings = new Map();
  for (const match of body.matchAll(DESTRUCTURE_PATTERN)) {
    for (const part of match[1].split(",")) {
      const trimmed = part.trim();
      if (trimmed === "" || trimmed === "..") {
        continue;
      }
      const [field, alias] = trimmed.split(":").map((piece) => piece.trim());
      if (field === "") {
        continue;
      }
      bindings.set(alias === undefined || alias === "" ? field : alias, field);
    }
  }
  return bindings;
}

/**
 * Methods a formatter body may call on `self` without printing bytes.
 *
 * `is_some` and `is_none` are here for the same reason `is_empty` is: they
 * reduce a value to whether it is there. `P2-U7` needs them because a
 * redaction projection holds `Option<String>` for every field a profile may
 * remove, and "this profile kept the student number" is what its `Debug` has
 * to be able to say.
 */
const FORMATTER_SAFE_METHODS = /^(len|is_empty|count|capacity|is_some|is_none)$/;


function derivedTraits(attributeBlock) {
  const traits = new Set();
  for (const match of attributeBlock.matchAll(DERIVE_PATTERN)) {
    for (const name of match[1].split(",")) {
      const trimmed = name.trim().replace(/^.*::/, "");
      if (trimmed !== "") {
        traits.add(trimmed);
      }
    }
  }
  return traits;
}

async function scan() {
  const definitions = new Map();
  const handWrittenDebug = new Set();
  const debugBodies = new Map();
  const macroKeyTypes = new Set();
  let keysSource = "";
  for (const path of await productSources()) {
    const contents = await readFile(path, "utf8");
    const lines = contents.split(/\r?\n/);
    const location = relative(REPOSITORY_ROOT, path).split("\\").join("/");
    if (location.endsWith("crates/crypto/src/keys.rs")) {
      keysSource = contents;
    }
    for (const match of contents.matchAll(
      /\bsecret_key!\s*\(\s*\n?\s*([A-Za-z_][A-Za-z0-9_]*)\s*,/g,
    )) {
      macroKeyTypes.add(match[1]);
    }
    for (const match of contents.matchAll(
      /\bimpl\s+(?:core::fmt::|std::fmt::|fmt::)?(?:Debug|Display)\s+for\s+([A-Za-z_][A-Za-z0-9_]*)/g,
    )) {
      handWrittenDebug.add(match[1]);
    }
    for (const [name, body] of handWrittenFormatterBodies(contents)) {
      const existing = debugBodies.get(name);
      debugBodies.set(name, existing === undefined ? body : `${existing}\n${body}`);
    }
    lines.forEach((line, index) => {
      const match = DEFINITION_PATTERN.exec(line);
      if (match === null) {
        return;
      }
      const sites = definitions.get(match[2]) ?? [];
      sites.push({
        name: match[2],
        kind: match[1],
        location: `${location}:${index + 1}`,
        derives: derivedTraits(attributesAbove(lines, index)),
        // Comments are stripped so prose naming a type is not read as a field.
        body: bodyAt(lines, index).replace(/\/\/.*$/gm, ""),
        // A tuple struct has no named field for the net to read, so its
        // payload positions are collected separately; `T114` found a tuple
        // secret invisible to a guard that only read named fields.
        tuple: tupleBodyAt(lines, index).replace(/\/\/.*$/gm, ""),
      });
      definitions.set(match[2], sites);
    });
  }
  return { definitions, handWrittenDebug, debugBodies, macroKeyTypes, keysSource };
}

const { definitions, handWrittenDebug, debugBodies, macroKeyTypes, keysSource } =
  await scan();

test("the secret_key! macro still declares the ADR-005 key types and redacts them", () => {
  assert.ok(
    macroKeyTypes.size >= 11,
    `expected at least the eleven ADR-005 key types from secret_key!, found ${macroKeyTypes.size}: ${[...macroKeyTypes].sort().join(", ")}`,
  );
  const macroBody = keysSource.slice(
    keysSource.indexOf("macro_rules! secret_key"),
    keysSource.indexOf("secret_key!("),
  );
  assert.ok(
    macroBody.length > 0,
    "the secret_key! macro body was not found in crates/crypto/src/keys.rs",
  );
  assert.ok(
    !/#\s*\[\s*derive\s*\([^)]*\bDebug\b/.test(macroBody),
    "secret_key! must not derive Debug: every key type it declares would print its bytes",
  );
  assert.ok(
    /impl\s+fmt::Debug\s+for\s+\$name/.test(macroBody),
    "secret_key! must hand-write the redacting Debug its key types rely on",
  );
});

test("every secret_key! type is named in the zeroize-on-drop enumeration", () => {
  // The enumeration is written out by name rather than counted, so it has to be
  // extended when a key type is added. `RehearsalKey` was the eleventh type and
  // was missing from it; nothing failed, because nothing checked. This does.
  const enumeration = keysSource.slice(
    keysSource.indexOf("fn every_key_type_is_zeroize_on_drop"),
  );
  const body = enumeration.slice(0, enumeration.indexOf("\n    }"));
  const unlisted = [...macroKeyTypes].filter(
    (name) => !body.includes(`assert_zeroize_on_drop::<${name}>()`),
  );
  assert.deepEqual(
    unlisted.sort(),
    [],
    `secret_key! declares these key types and every_key_type_is_zeroize_on_drop does not name them: ${unlisted.join(", ")}`,
  );
});

test("every registered secret-bearing type still exists", () => {
  const missing = [...SECRET_BEARING_TYPES.keys()].filter(
    (name) => !definitions.has(name),
  );
  assert.deepEqual(
    missing,
    [],
    `renamed or removed, so this guard silently stopped covering them: ${missing.join(", ")}`,
  );
});

test("no registered secret-bearing type derives Debug or Display", () => {
  const leaks = [];
  for (const [name, carries] of SECRET_BEARING_TYPES) {
    for (const site of definitions.get(name) ?? []) {
      for (const trait of ["Debug", "Display"]) {
        if (site.derives.has(trait)) {
          leaks.push(
            `${site.location}: ${site.kind} ${name} derives ${trait} over ${carries}; write the impl by hand and redact`,
          );
        }
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));
});

test("every registered secret-bearing type has a hand-written redacting Debug", () => {
  const undebuggable = [...SECRET_BEARING_TYPES.keys()].filter(
    (name) => !handWrittenDebug.has(name),
  );
  assert.deepEqual(
    undebuggable.sort(),
    [],
    `these carry secrets and have no hand-written Debug, so any added derive would leak: ${undebuggable.join(", ")}`,
  );
});

/**
 * Every `Type.field` in the workspace whose declared type is a byte buffer,
 * mapped to the crates that declare it.
 *
 * The value is what tells one classification covering one type from one
 * covering several, which the key alone cannot say.
 */
function declaredByteFieldCrates() {
  const found = new Map();
  for (const sites of definitions.values()) {
    for (const site of sites) {
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        if (!isClassifiedByteBuffer(trimDeclaredType(field[2]))) {
          continue;
        }
        const key = `${site.name}.${field[1]}`;
        const crate = site.location.split("/")[1];
        const crates = found.get(key);
        if (crates === undefined) {
          found.set(key, new Set([crate]));
        } else {
          crates.add(crate);
        }
      }
    }
  }
  return found;
}

/** Every `Type.field` in the workspace whose declared type is a byte buffer. */
function declaredByteFields() {
  return new Set(declaredByteFieldCrates().keys());
}

test("every named byte buffer in the workspace is classified", () => {
  // The whole set, in both directions. `T166` measured the alternation this
  // replaces admitting a `Vec<u8>` called `excerpt`, because the question used
  // to be "is this name on the list" and the list of names bytes can hide
  // behind has no end. The list of fields that exist does, so it is asked the
  // other way round: every byte buffer is named here, and a new one fails
  // until somebody says what it holds.
  const found = declaredByteFields();
  const unclassified = [...found].filter((entry) => !BYTE_FIELD_CLASSES.has(entry));
  assert.deepEqual(
    unclassified.sort(),
    [],
    `these hold raw bytes and BYTE_FIELD_CLASSES does not say what: ${unclassified.join(", ")}. Add each one with a class from BYTE_CLASSES; do not add a name to SECRET_FIELD_NAMES instead.`,
  );

  // An entry naming a field that no longer exists is a judgement about
  // something gone, and the next reader would take it for a live one.
  const stale = [...BYTE_FIELD_CLASSES.keys()].filter((entry) => !found.has(entry));
  assert.deepEqual(
    stale.sort(),
    [],
    `these BYTE_FIELD_CLASSES entries name no byte field any more and must be deleted: ${stale.join(", ")}`,
  );

  // The class column is a closed vocabulary. Free prose would let a widening
  // pass as one more plausible sentence; a new class has to be declared.
  const unknown = [...BYTE_FIELD_CLASSES]
    .filter(([, assigned]) => !BYTE_CLASSES.has(assigned))
    .map(([entry, assigned]) => `${entry}: ${assigned}`);
  assert.deepEqual(
    unknown.sort(),
    [],
    `these carry a class that BYTE_CLASSES does not declare: ${unknown.join(", ")}`,
  );
});

function declaredTextFields() {
  const found = new Set();
  for (const sites of definitions.values()) {
    for (const site of sites) {
      if (!TEXT_CLASSIFIED_CRATES.has(crateOf(site))) {
        continue;
      }
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        if (isClassifiedText(trimDeclaredType(field[2]))) {
          found.add(`${site.name}.${field[1]}`);
        }
      }
    }
  }
  return found;
}

test("every named text field in a text-classifying crate is classified", () => {
  // `S-18`: ten registrations were inert because their secret is text under a
  // field name `SECRET_FIELD_NAMES` does not carry, so nothing required the
  // registration and deleting one was silent. Widening that alternation is what
  // `S-10` records five rounds of; the answer is the whole set, asked the other
  // way round. In a crate that classifies text, every `String`/`str` field is
  // named here and a new one fails until somebody says what it holds.
  const found = declaredTextFields();
  const unclassified = [...found].filter((entry) => !TEXT_FIELD_CLASSES.has(entry));
  assert.deepEqual(
    unclassified.sort(),
    [],
    `these hold text in a crate that classifies text and TEXT_FIELD_CLASSES does not say what: ${unclassified.join(", ")}. Add each one with a class from TEXT_CLASSES; do not add a name to SECRET_FIELD_NAMES instead.`,
  );

  const stale = [...TEXT_FIELD_CLASSES.keys()].filter((entry) => !found.has(entry));
  assert.deepEqual(
    stale.sort(),
    [],
    `these TEXT_FIELD_CLASSES entries name no text field any more and must be deleted: ${stale.join(", ")}`,
  );

  const unknown = [...TEXT_FIELD_CLASSES]
    .filter(([, assigned]) => !TEXT_CLASSES.has(assigned))
    .map(([entry, assigned]) => `${entry}: ${assigned}`);
  assert.deepEqual(
    unknown.sort(),
    [],
    `these carry a class that TEXT_CLASSES does not declare: ${unknown.join(", ")}`,
  );

  // A crate listed here that declares no text field at all would make the
  // whole check vacuous for it, which is the empty-guard shape this file is
  // written against.
  const empty = [...TEXT_CLASSIFIED_CRATES.keys()].filter(
    (crate) =>
      ![...definitions.values()]
        .flat()
        .some(
          (site) =>
            crateOf(site) === crate &&
            [...site.body.matchAll(NAMED_FIELD_PATTERN)].some((field) =>
              isClassifiedText(trimDeclaredType(field[2])),
            ),
        ),
  );
  assert.deepEqual(
    empty.sort(),
    [],
    `these crates classify text and declare no text field, so the check is vacuous for them: ${empty.join(", ")}`,
  );
});

test("a text classification covering more than one crate says so", () => {
  // The same hazard `SHARED_BYTE_FIELD_KEYS` records: `TEXT_FIELD_CLASSES` is
  // keyed by `Type.field` and not by path, so a second type of the same name in
  // another text-classifying crate would arrive already classified by a line
  // written for the first. `P2-L4` did exactly that with a second
  // `CorpusFile.bytes` and it was silent.
  const crates = new Map();
  for (const sites of definitions.values()) {
    for (const site of sites) {
      if (!TEXT_CLASSIFIED_CRATES.has(crateOf(site))) {
        continue;
      }
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        if (!isClassifiedText(trimDeclaredType(field[2]))) {
          continue;
        }
        const key = `${site.name}.${field[1]}`;
        const seen = crates.get(key);
        if (seen === undefined) {
          crates.set(key, new Set([crateOf(site)]));
        } else {
          seen.add(crateOf(site));
        }
      }
    }
  }
  const spanning = [...crates]
    .filter(([, seen]) => seen.size > 1)
    .map(([key, seen]) => `${key} (${[...seen].sort().join(", ")})`);
  assert.deepEqual(
    spanning.sort(),
    [],
    `these text classifications cover a type name declared in more than one text-classifying crate, so one crate's text is judged by a line written for another's: ${spanning.join("; ")}`,
  );
});

test("a classification covering more than one crate says so", () => {
  // `BYTE_FIELD_CLASSES` is keyed by name, so a new type reusing an existing
  // one's name inherits its class without anybody deciding. `P2-L4` did
  // exactly that with a second `CorpusFile.bytes`, and it was silent. A key
  // that spans crates is declared here instead, in both directions.
  const spanning = new Map(
    [...declaredByteFieldCrates()].filter(([, crates]) => crates.size > 1),
  );

  const undeclared = [...spanning]
    .filter(([key]) => !SHARED_BYTE_FIELD_KEYS.has(key))
    .map(([key, crates]) => `${key} (${[...crates].sort().join(", ")})`);
  assert.deepEqual(
    undeclared.sort(),
    [],
    `these classifications cover a type name declared in more than one crate, so one crate's bytes are judged by a line written for another's: ${undeclared.join("; ")}. Say so in SHARED_BYTE_FIELD_KEYS, after checking the class is right for both.`,
  );

  const stale = [...SHARED_BYTE_FIELD_KEYS.keys()].filter(
    (key) => !spanning.has(key),
  );
  assert.deepEqual(
    stale.sort(),
    [],
    `these SHARED_BYTE_FIELD_KEYS entries no longer span two crates and must be deleted: ${stale.join(", ")}`,
  );
});

test("no unregistered type derives Debug over a raw key or plaintext buffer", () => {
  const leaks = [];
  const exercised = new Set();
  for (const sites of definitions.values()) {
    for (const site of sites) {
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        const [, fieldName, fieldType] = field;
        const declared = trimDeclaredType(fieldType);
        const qualified = `${site.name}.${fieldName}`;
        // Layer 1 is the byte class, layer 2 the text class in a crate that
        // classifies text, layer 3 the name alternation everywhere else. An
        // unclassified field counts as secret in the first two, so the test
        // above naming it and this one refusing it are the same failure and
        // not a race.
        const secret = fieldIsSecretBeforeExceptions(
          site,
          site.name,
          fieldName,
          declared,
        );
        if (!secret) {
          continue;
        }
        if (PUBLIC_BYTES.has(qualified)) {
          exercised.add(qualified);
          continue;
        }
        if (!site.derives.has("Debug") && !site.derives.has("Display")) {
          continue;
        }
        const repair = isClassifiedByteBuffer(declared)
          ? "Write the impl by hand and redact, or classify it in BYTE_FIELD_CLASSES."
          : TEXT_CLASSIFIED_CRATES.has(crateOf(site))
            ? "Write the impl by hand and redact, or give it a non-secret class in TEXT_FIELD_CLASSES."
            : "Write the impl by hand and redact, or record in PUBLIC_BYTES why these bytes are public.";
        leaks.push(
          `${site.location}: ${site.kind} ${site.name} derives Debug over ${qualified}: ${declared.trim()}. ${repair}`,
        );
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));

  // An exception that no longer matches anything is stale and hides nothing,
  // so it is removed rather than left to look like coverage.
  const stale = [...PUBLIC_BYTES.keys()].filter((entry) => !exercised.has(entry));
  assert.deepEqual(
    stale.sort(),
    [],
    `these PUBLIC_BYTES exceptions match no field any more and must be deleted: ${stale.join(", ")}`,
  );
});


/** Reports whether a type text holds a raw byte buffer anywhere inside it. */
function containsRawByteBuffer(text) {
  const normalized = normalizeFieldType(text);
  return (
    RAW_BYTE_TYPES.test(normalized) ||
    /(Vec\s*<\s*u8\s*>|\[\s*u8\s*(;[^\]]*)?\])/.test(normalized)
  );
}

/** Type-name tokens a field's declared type mentions. */
function typeTokens(text) {
  return normalizeFieldType(text).match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? [];
}

/** Every field a definition declares, as `{ name, type }`. */
function declaredFields(site) {
  const fields = [...site.body.matchAll(NAMED_FIELD_PATTERN)].map((match) => ({
    name: match[1],
    type: trimDeclaredType(match[2]),
  }));
  for (const position of site.tuple.matchAll(TUPLE_FIELD_PATTERN)) {
    const type = position[1].trim();
    if (type !== "") {
      fields.push({ name: null, type });
    }
  }
  return fields;
}

/**
 * Types that carry key material or plaintext, computed from the source.
 *
 * A field carries it when its declared type *is* one of the `secret_key!` key
 * types, or when it holds a raw byte buffer under a name from the vocabulary,
 * or -- for a tuple position, which has no name -- when it holds one at all.
 * A type holding such a type is one too, iterated to a fixed point so a two-hop
 * holder is reached: `EncryptedDomainKeyring` holds `DomainObjectKeys` holds
 * `DomainKek`.
 *
 * Propagation runs over *declared field types* only. Reading the whole body
 * would make every type that merely mentions a key type in a method secret,
 * which is how a first attempt at this flagged `AcceptanceService`.
 */
function secretBearingTypeNames() {
  const direct = new Set();
  const bearing = new Set(macroKeyTypes);
  for (const [name, sites] of definitions) {
    for (const site of sites) {
      for (const field of declaredFields(site)) {
        const isKeyType = macroKeyTypes.has(normalizeFieldType(field.type));
        const buffer = field.name !== null && isClassifiedByteBuffer(field.type);
        const assigned = buffer
          ? BYTE_FIELD_CLASSES.get(`${name}.${field.name}`)
          : undefined;
        const classified =
          buffer && (assigned === undefined || SECRET_BYTE_CLASSES.has(assigned));
        const named =
          field.name !== null &&
          !buffer &&
          SECRET_FIELD_NAMES.test(field.name) &&
          containsRawByteBuffer(field.type);
        const positional = field.name === null && containsRawByteBuffer(field.type);
        if (isKeyType || classified || named || positional) {
          bearing.add(name);
          direct.add(name);
        }
      }
    }
  }
  for (let round = 0; round < 8; round += 1) {
    let grew = false;
    for (const [name, sites] of definitions) {
      if (bearing.has(name)) {
        continue;
      }
      const reaches = sites
        .flatMap(declaredFields)
        .flatMap((field) => typeTokens(field.type))
        .some((token) => token !== name && bearing.has(token));
      if (reaches) {
        bearing.add(name);
        grew = true;
      }
    }
    if (!grew) {
      break;
    }
  }
  return { direct, bearing };
}

/**
 * Types the source requires a registration for.
 *
 * {@link secretBearingTypeNames} answers a different question -- what the
 * *content* check must read -- and `T177` measured what happens when the
 * registration guard reuses its answer: **21 of the 38 registrations could be
 * deleted with the suite still at 12 pass, 0 fail**, which is `S-18`. Two
 * mechanisms produced that, and this function exists because they are
 * different:
 *
 *   * The guard skipped any impl without a `<redacted>` or
 *     `finish_non_exhaustive` marker, and ten registered types redact by
 *     reducing a buffer to a length and write no marker at all. The marker was
 *     standing in for the exception maps: dropping it alone would fire on
 *     `ContentDigest`, `GenerationId` and `VaultLocator`, which
 *     {@link PUBLIC_TUPLE_BYTES} already declares public, and on
 *     `QuotedDocument.digest`, which {@link PUBLIC_BYTES} does. So the
 *     exceptions are applied here instead, and the marker is gone.
 *   * A tuple position was read through {@link RAW_BYTE_TYPES}, which contains
 *     `String` and `str`. The documented rule for a position with no name is
 *     {@link RAW_BYTE_PAYLOAD_TYPES} -- bytes only, because every error type
 *     here reports through a `String` -- and reading the wider one made
 *     `JobId`, `KeystoreLabel` and `ExplanationSnapshot` secret-bearing.
 *
 * One hop is added on top of the direct evidence: a type that declares a field
 * whose type mentions a directly secret-bearing one. `EncryptedDomainKeyring`
 * and `NormalizedTranscript` are registered and hold their secret only that
 * way, so without the hop their registrations stay inert. It stops at one hop
 * because the fixed point reaches `AcceptanceService`, which this file already
 * records as the false positive that sank a first attempt at this.
 */
function registrationRequiredTypes() {
  const direct = new Set();
  for (const [name, sites] of definitions) {
    for (const site of sites) {
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        const declared = trimDeclaredType(field[2]);
        if (
          macroKeyTypes.has(normalizeFieldType(declared)) ||
          fieldIsSecret(site, name, field[1], declared)
        ) {
          direct.add(name);
        }
      }
      if (PUBLIC_TUPLE_BYTES.has(name)) {
        continue;
      }
      for (const position of site.tuple.matchAll(TUPLE_FIELD_PATTERN)) {
        if (RAW_BYTE_PAYLOAD_TYPES.test(normalizeFieldType(position[1].trim()))) {
          direct.add(name);
        }
      }
    }
  }
  const required = new Set(direct);
  const seed = new Set([...direct, ...macroKeyTypes]);
  for (const [name, sites] of definitions) {
    for (const site of sites) {
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        for (const token of typeTokens(trimDeclaredType(field[2]))) {
          if (token !== name && seed.has(token)) {
            required.add(name);
          }
        }
      }
    }
  }
  return required;
}


const { direct: DIRECT_SECRET_BEARING, bearing: SECRET_BEARING } =
  secretBearingTypeNames();
const REGISTRATION_REQUIRED = registrationRequiredTypes();

test("a type whose Debug is hand-written over secret bytes is registered", () => {
  // Deleting a registration was silent: the remaining tests all iterate the
  // registry, so a type removed from it stopped being covered and nothing
  // failed. The registry is now checked against the source. A type that carries
  // secret bytes in a field of its own -- and whose author wrote `Debug` by
  // hand rather than deriving it -- is exactly the shape the registry is for,
  // so it must be in it. A type that only reaches a secret through another
  // type is not required here: the inner type's own redacting `Debug` is what
  // a derive on the outer one would print, and a type holding a secret with no
  // `Debug` at all cannot be derived over without a compile error.
  const unregistered = [];
  for (const name of debugBodies.keys()) {
    if (SECRET_BEARING_TYPES.has(name) || !REGISTRATION_REQUIRED.has(name)) {
      continue;
    }
    unregistered.push(name);
  }
  assert.deepEqual(
    unregistered.sort(),
    [],
    `these hand-write a redacting Debug over secret bytes and are not in SECRET_BEARING_TYPES, so removing that impl would be silent: ${unregistered.join(", ")}`,
  );
});

test("no hand-written Debug prints a secret field it was written to hide", () => {
  // The registry says a type has a hand-written `Debug`; it did not say what
  // that `Debug` prints. `T114` injected an impl that redacted every field but
  // one and neither this guard nor the Rust unit test noticed. A raw byte field
  // may reach the formatter only through a length.
  //
  // It runs over every type that holds secret bytes in a field of its own, not
  // only the registered ones. The registration test above reads an impl only
  // when it contains `<redacted>` or `finish_non_exhaustive`, so an impl that
  // does not redact at all was in neither net: `T116` injected an unregistered
  // type whose hand-written `Debug` printed `self.dek` and nothing failed.

  // Reductions that yield a length but do not sit directly after the field.
  // `Option<&[u8]>` has no `len()` of its own, so a redacting `Debug` over one
  // reaches its length through `map_or`, which the scan below would otherwise
  // read as a raw use. Each entry is one exact spelling whose result is a
  // `usize`; a `map_or` carrying a closure is not one of them and still fails.
  // They are rewritten to the plain spelling before the scan.
  const LENGTH_REDUCTIONS = [
    [/\.\s*map_or\(\s*0\s*,\s*<\[u8\]>::len\s*\)/gu, ".len()"],
  ];

  const leaks = [];
  const handWritten = new Set([
    ...SECRET_BEARING_TYPES.keys(),
    ...DIRECT_SECRET_BEARING,
    ...REGISTRATION_REQUIRED,
  ]);
  for (const name of handWritten) {
    const declaredBody = debugBodies.get(name);
    if (declaredBody === undefined) {
      continue;
    }
    const body = LENGTH_REDUCTIONS.reduce(
      (reduced, [pattern, plain]) => reduced.replace(pattern, plain),
      declaredBody,
    );
    const rawFields = new Set();
    for (const site of definitions.get(name) ?? []) {
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        const declared = trimDeclaredType(field[2]);
        if (
          fieldIsSecret(site, name, field[1], declared) ||
          (!PUBLIC_BYTES.has(`${name}.${field[1]}`) &&
            SECRET_BEARING.has(normalizeFieldType(declared)))
        ) {
          rawFields.add(field[1]);
        }
      }
    }
    for (const field of rawFields) {
      const uses = body.matchAll(
        new RegExp(`self\\s*\\.\\s*${field}\\b([^,)]*)`, "g"),
      );
      for (const use of uses) {
        if (/^\s*\.\s*(len|is_empty|count|capacity|is_some|is_none)\s*\(/.test(use[1])) {
          continue;
        }
        leaks.push(
          `${name}: its hand-written Debug reaches self.${field} without reducing it to a length, so the bytes it was written to hide are printed`,
        );
      }
    }

    // The same field reached through a name the body bound by destructuring.
    // The binding statements are removed first, so what is left is uses of the
    // name and nothing else: `{dek:?}` in a format string is a use and the
    // `let Self { dek } = self;` that introduced it is not.
    const afterBinding = body.replace(DESTRUCTURE_PATTERN, " ");
    for (const [binding, field] of destructuredBindings(body)) {
      if (!rawFields.has(field)) {
        continue;
      }
      const uses = afterBinding.matchAll(
        new RegExp(`(?<![.\\w])${binding}\\b([^,)]*)`, "gu"),
      );
      for (const use of uses) {
        if (/^\s*\.\s*(len|is_empty|count|capacity|is_some|is_none)\s*\(/.test(use[1])) {
          continue;
        }
        leaks.push(
          `${name}: its hand-written Debug destructures self and reaches ${field} as ${binding}, so the bytes it was written to hide are printed`,
        );
      }
    }

    // A method call on `self` is the other way out. A redacting formatter has
    // no reason to make one, and one that does hands the decision to code this
    // net never reads: `T118` injected `self.render()` returning
    // `hex::encode(self.dek)` and nothing failed. Reducing a buffer to a length
    // is the exception, and it is the only one.
    if (rawFields.size > 0) {
      for (const call of body.matchAll(/self\s*\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(/g)) {
        if (FORMATTER_SAFE_METHODS.test(call[1])) {
          continue;
        }
        leaks.push(
          `${name}: its hand-written Debug calls self.${call[1]}(), so what reaches the formatter is decided somewhere this guard cannot read`,
        );
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));
});

test("every public tuple payload exception still names a type that has one", () => {
  // The named exceptions are checked the same way. An entry left behind after
  // its type changed shape is a judgement about something that no longer
  // exists, and the next reader would take it for a live one.
  const stale = [];
  for (const name of PUBLIC_TUPLE_BYTES.keys()) {
    const sites = definitions.get(name) ?? [];
    const bears = sites.some((site) =>
      [...site.tuple.matchAll(TUPLE_FIELD_PATTERN)].some((position) =>
        RAW_BYTE_PAYLOAD_TYPES.test(normalizeFieldType(position[1].trim())),
      ),
    );
    if (!bears) {
      stale.push(name);
    }
  }
  assert.deepEqual(
    stale.sort(),
    [],
    `these are excepted as public tuple bytes but no longer declare a byte payload: ${stale.join(", ")}`,
  );
});

test("no unregistered tuple type derives Debug over a secret payload", () => {
  // A tuple struct and a tuple enum variant have no field name, so the named
  // net never saw them. `T114` found `RecoveredSecret`, `BackupMasterKey`, and
  // an enum variant `Dek([u8; 32])` all silent, and `T116` found a plain
  // `AuditLeak(Vec<u8>)` silent after them: the payload check read only
  // `Zeroizing` and the `secret_key!` types, so a tuple that simply held the
  // bytes was not a shape it looked for.
  const leaks = [];
  for (const [name, sites] of definitions) {
    for (const site of sites) {
      if (!site.derives.has("Debug") && !site.derives.has("Display")) {
        continue;
      }
      if (site.tuple !== "") {
        for (const position of site.tuple.matchAll(TUPLE_FIELD_PATTERN)) {
          const payload = position[1].trim();
          if (payload === "") {
            continue;
          }
          const zeroizing = /(^|::)Zeroizing\s*</.test(payload);
          const rawBytes =
            RAW_BYTE_PAYLOAD_TYPES.test(normalizeFieldType(payload)) &&
            !PUBLIC_TUPLE_BYTES.has(name);
          if (zeroizing || rawBytes || macroKeyTypes.has(normalizeFieldType(payload))) {
            leaks.push(
              `${site.location}: ${site.kind} ${name} derives Debug over the tuple payload ${payload}`,
            );
          }
        }
      }
      if (site.kind === "enum") {
        for (const variant of site.body.matchAll(TUPLE_VARIANT_PATTERN)) {
          const [, variantName, payload] = variant;
          const snake = variantName
            .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
            .toLowerCase();
          const named = SECRET_FIELD_NAMES.test(snake);
          for (const position of payload.matchAll(TUPLE_FIELD_PATTERN)) {
            const inner = position[1].trim();
            if (inner === "") {
              continue;
            }
            const normalized = normalizeFieldType(inner);
            // A variant name is one signal and the payload type is the other.
            // Requiring both is what made `Payload(Vec<u8>)` and
            // `Buffer([u8; 32])` silent while `Dek([u8; 32])` was caught —
            // `T118` injected exactly that. A raw byte payload is enough on its
            // own, the same way it is for a tuple struct, and no enum in this
            // workspace declares one that is public.
            const secretByName =
              named && (holdsRawBytes(inner) || macroKeyTypes.has(normalized));
            const secretByPayload =
              (RAW_BYTE_PAYLOAD_TYPES.test(normalized) ||
                macroKeyTypes.has(normalized) ||
                /(^|::)Zeroizing\s*</.test(inner)) &&
              !PUBLIC_TUPLE_VARIANT_BYTES.has(`${name}::${variantName}`);
            if (secretByName || secretByPayload) {
              leaks.push(
                `${site.location}: enum ${name} derives Debug over variant ${variantName}(${inner})`,
              );
            }
          }
        }
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));
});
