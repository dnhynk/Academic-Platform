# Transcript ingestion

The `P2-U7` boundary in `academic-transcript`: PDF, CSV and manual-entry import
of an official transcript into the encrypted vault, section 29.3 of the
end-state design.

## Current outcome

Nothing durable happens. `P2-K6` built an admission verifier and did not open
admission — the compiled acceptance public key is `Unprovisioned` and the
committed candidate receipt carries two of five platform rows — so
`AdmittedImport::open` refuses on every profile in this repository, and both
gated entry points take that capability by value. `import_without_admission_receipt_is_refused`
is not an error path the suite contrives; it is what an import does today.

Separately from that gate, ADR-002 is unaccepted, the default lane reports
`storage_encryption=NONE` and `production_data_allowed=false`, and every corpus
this crate builds is synthetic and comes from
`testdata/transcript-canary/canaries.txt` through the deterministic builder in
`source.rs`. No real academic record may be imported.

## Where the gate is, and where it is not

Gated, by taking `&AdmittedImport`:

- `session::ImportSession::begin` and `::resume` — the durable import session.
- `vault::store_transcript_original` — sealing an original into a profile.

Not gated: parsing bytes into an in-memory `NormalizedTranscript`. That writes
nothing and touches no profile. "No import is possible" would be stronger than
the code; what is true is that **nothing durable happens without a verified
receipt**.

Admission being closed would make both gated paths unreachable, and an
unexecuted seal is not evidence that a transcript original is ciphertext at
rest. The one hole is therefore
`AdmittedImport::for_fault_injection_only`, compiled only by the non-default
`phase2-fault-injection` feature. There is no ungated mechanism beside a gated
entry point: `transcript_admission_gate_has_one_product_constructor` freezes the
capability's whole surface, and `transcript_lanes_are_not_default` proves no
product binary selects the feature.

Only `AdmissionVerifier::verify`'s public contract is consumed — the `Result`,
and `AdmissionError::code`. Nothing here reads the receipt bytes, the platform
set, the five stages, or the posture emitter.

## The canonical record

`NormalizedTranscript` is what every import format produces and the only thing
downstream reads. It separates two things that are never merged again:

| part | fields | who may remove it |
|---|---|---|
| identity header | student number, student name, institution, issue date | a redaction projection removes the first two, independently |
| rows | course code, term, credits, grade | nothing; these are the four reconciled fields |

Rows carry the ordinal they have in the official document, assigned by the
normalizer and never by a sort, because a mismatch is reported by ordinal and
two importers that ordered rows differently would localize the same defect to
different places.

`canonical_bytes` is the one byte form this crate hashes, compares and
checksums. It is length-prefixed rather than delimited, so a field value cannot
spell a separator. Credit values are normalized through `canonical_decimal`
before hashing: `academic_domain::Decimal` spells `3` and `3.0` differently for
the same quantity, and a checksum over the raw spelling would call a CSV that
writes one and a hand entry that writes the other a field mismatch.

## What each import format is, exactly

| format | what it does | claim provenance |
|---|---|---|
| `PdfTextLayer` | walks `BT`/`ET` blocks and reads the literal strings passed to `Tj` | `Actor::Importer`, `CODE_OBSERVED`/`DIRECT_OBSERVATION` |
| `PdfOcr` | **no optical character recognition**; names the provenance of values a model produced, which the caller supplies | `Actor::ModelRun`, `AI_INFERRED`/`MODEL_INFERENCE`, `ModelRead` required |
| `Csv` | reads the declared header keys then the declared row header | `Actor::Importer`, `CODE_OBSERVED`/`DIRECT_OBSERVATION` |
| `ManualEntry` | validates typed values | `Actor::Importer`, `CODE_OBSERVED`/`DIRECT_OBSERVATION` |

There is no PDF library and no OCR engine in this repository, and this task
added neither. The text-layer parser handles no filter, no font encoding and no
compressed object stream: it reads the corpus `build_synthetic_transcript_pdf`
emits, and **a real official transcript PDF needs its own declared layout before
this parser may be pointed at it.**

All four formats target one labelled line grammar. An unknown key, a missing
key, a duplicate key, or a row without exactly four fields is refused, so a
partially-read document never becomes a partially-populated transcript. The CSV
carries no quoting and reads none: a row value that spells the separator becomes
a row with more than four fields and is refused rather than misread.

## Import row and confirmed row

Two linked claims, enforced by primitives the canonical vocabulary already has
rather than by a rule this crate adds:

- `Claim::validate_for_actor` permits `AuthorityClass::UserExplicit` to
  `Actor::User` and to no one else. So neither an importer nor a model run can
  mint a claim that reads as user-confirmed, whatever a caller passes.
- The two carry different `ClaimId`s and are joined by an explicit
  `ClaimRelation { source: import, target: confirmed, kind: SUPPORTS }`, so a
  projection reaches one from the other without either replacing the other.
- `confirm_reconciled_rows` takes a `ReconciledTranscript`, which `reconcile`
  returns only when every row agreed. A halted import has no value to pass.

A model read carries a `ModelRead { run_id, confidence }` and a deterministic
read must not. The two travel together because neither is meaningful alone: a
confidence with no run behind it names an estimate nothing can be traced to, and
a run with no confidence is indistinguishable from a deterministic read in every
projection that reads the claim. The run is its own entity, not the row's
subject — citing a run means naming the run, not what is being asserted about.

The claim object carries the four reconciled fields and no identity value. A
claim object is copied into projections, proof trees and explanation snapshots;
putting the student number there would reintroduce, one step outside the export
path, the value the redaction projection exists to remove.

## Reconciliation, and what a failure looks like

`TranscriptChecksums::of` derives a per-field SHA-256 digest for every row plus
one digest over the identity header. A reference is a second, independent
reading of the same official document — the CSV export beside the PDF, or the
user's manual entry beside an OCR pass.

`reconcile` walks rows in document order and **stops at the first row that
disagrees**. It does not continue to collect every downstream mismatch: that is
what turns a localized failure back into a whole-document verdict. A
`ReconciliationHalt` carries

- the ordinal of the halting row,
- the fields inside it that disagree, in canonical order,
- how many rows reconciled before it,

and no field value and no identity value, because a mismatch report is a second
surface that reaches a screen. The caller holds the candidate transcript and
renders the disputed values from it; the reference side is a checksum and has
nothing to render, which sends the user back to the official document rather
than to the other import.

A row-count difference is localized the same way, and its two directions —
`ROW_ABSENT_FROM_REFERENCE` and `ROW_ABSENT_FROM_CANDIDATE` — are kept apart.

**The identity header is not a halt condition.** Two readings of one document
can spell a name differently, and refusing the whole import for that would
discard four correct academic fields over a field no downstream calculation
reads. Both identity digests are available so a caller can show the difference;
nothing here decides it.

## The original at rest

A transcript original is sealed through `academic-vault`'s public
`EncryptedVault::ingest`. **This crate defines no object format.** It reuses
ADR-004's `AEAD_CHUNKED_V2` and contains no cipher, no nonce schedule and no
second format label; `transcript_defines_no_second_object_format` is what
enforces that, over source rather than over intent.

The two policy labels are not parameters. `transcript_ingest_request` hard-codes
`RESTRICTED`/`USER_MANAGED` (section 32.2's `Z1`, section 29.3), and
`store_transcript_original` refuses a request carrying any other pair before a
byte is written.

## Redaction is a projection

`project(&NormalizedTranscript, RedactionProfile) -> RedactedProjection`. The
source is borrowed and unchanged, no record type exposes a `&mut self` method or
a public field, and a projection owns nothing but the values it retains — there
is no handle back to the transcript and none to the sealed original.

`redacted_export(&RedactedProjection) -> Vec<u8>` takes a projection and nothing
else, so an export cannot carry a byte or a metadata string of the original
unless someone changes its signature. `transcript_redaction_has_no_source_edit_path`
freezes both signatures and the absence of every mutator, because an absence is
the one thing a Rust suite cannot assert about itself.

A removed field is **absent, not blanked**. A blanked field still says how long
the value was and still occupies its position in a diff against an unredacted
export. The export declares in one `REMOVED` line what the profile took out.

`RedactionProfile::all()` is the four combinations of the two independently
removable fields, and the acceptance row enumerates that constant, so a third
removable field cannot be added without the matrix growing with it.

## The import session, and `IN04`

One session lives at `<profile>/transcript/sessions/<transcript_version_id>/`.

| file | meaning |
|---|---|
| `session.lock` | the lease |
| `staging.part` | a complete, unpublished row set |
| `confirmed.set` | the published row set |

- **No partial set.** Both durable files arrive by rename over a fully written,
  fsynced temporary. A reader sees a complete file or no file.
- **Lease.** Created with `create_new`, so two *live* sessions cannot both hold
  it. It is **not** an operating-system advisory lock: a killed holder leaves the
  file behind, and `ImportSession::resume` is what releases it. That is
  deliberately weaker than "a crashed process releases its lease", which would
  not be true.
- **Resumable.** `session::inspect` reports the durable state; `resume` re-enters
  an unpublished session and refuses a published one, because re-entering one
  would be a second publication rather than a recovery.
- **Durability.** The rename is atomic with respect to a reader on both
  platforms. Only Unix fsyncs the containing directory: Windows exposes no
  directory handle through `std`, so the weaker guarantee against power loss is
  stated rather than papered over.

The confirmed set is identity-free — a durable file beside the vault is one more
place the student number must not be — and names its own transcript version and
the reconciliation's reference digest inside its bytes rather than only in its
path, so a file moved into another session's directory does not read there as
that session's confirmed set.

## Named acceptance evidence

| row | where |
|---|---|
| `transcript_formats_normalize_equivalently` | `crates/transcript/tests/transcript_ingestion.rs` |
| `ocr_row_and_confirmed_row_are_distinct_claims` | same |
| `field_level_mismatch_is_localized_before_confirmation` | same |
| `student_number_and_name_can_be_removed_independently` | same |
| `redacted_export_contains_no_original_bytes_or_metadata` | same |
| `import_without_admission_receipt_is_refused` | same |
| `transcript_original_is_ciphertext_at_rest` | `crates/transcript/tests/transcript_encrypted.rs` |
| `IN03` | `crates/transcript/tests/transcript_faults.rs` |
| `IN04` | same, three kill rows |

The first six run in the default lane, on every platform, inside
`cargo test --workspace`. The last three need
`--features encrypted-vault,phase2-fault-injection`, which is a hosted CI step on
every Rust matrix label.

Two of those rows measure an absence, so both are executed against a violation
inside the suite itself rather than argued for:

- `redacted_export_contains_no_original_bytes_or_metadata` first asserts the
  corpus *does* carry every marker the scan looks for, then scans a clean export,
  then re-scans three deliberately leaked copies — a metadata string, 128 raw
  original bytes, and the removed student number — and requires each to be
  caught. The window check treats any 16-byte run shared with the original as a
  leak unless it lies inside a value the projection deliberately retained.
- `transcript_original_is_ciphertext_at_rest` asserts every committed canary is
  in the bytes being sealed, streams every file below the profile root, requires
  zero hits, then writes the plaintext original to a sidecar inside the profile
  and requires the same scan to report hits. It reports `canary_file_count`,
  `canary_byte_count` and `canary_hit_count`, the same three counts an admission
  receipt's platform row carries, because a scan reporting only "no hits" cannot
  be told from a scan that read nothing.

## Structural guards outside the Rust suite

In `tools/phase1-scaffold-policy.test.mjs`:

- `transcript_lanes_are_not_default` — neither feature resolves in a default
  build and no product binary links the crate.
- `transcript_redaction_has_no_source_edit_path` — no `&mut` record borrow, no
  public record field, and both projection signatures frozen.
- `transcript_admission_gate_has_one_product_constructor` — the capability's
  whole surface, and the feature gate on the fault-lane constructor.
- `transcript_defines_no_second_object_format` — no cryptographic primitive
  named in the crate's source, one sealing call, two frozen policy labels.

In `tools/secret-debug-policy.test.mjs`: `TranscriptIdentity`,
`NormalizedTranscript` and `RedactedProjection` are registered secret-bearing
types. A derived `Debug` on any of them would print the student number into any
log line, panic message or audit row that formatted an enclosing value — the
same exposure ADR-005 forbids for key material, one step outside the export path
the acceptance rows watch.

## What this does not close

`GATE-38-005` (the current official transcript) and `GATE-38-007` (current and
planned enrollments) stay open. Both are user-supplied inputs and nothing here
infers one: an absent field is a refusal naming the field, a reconciliation with
an unmatched row halts rather than assuming a side, and there is no default
identity, term, or credit value anywhere in the crate.

`P2-U4` builds the attempt model from confirmed rows. This crate publishes no
event and appends to no ledger; it produces claim values and a durable confirmed
row set, and the ledger boundary is `academic-ledger`'s.
