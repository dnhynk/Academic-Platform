# The graduation export, and the restore that needs nothing else

## Posture

Export schema v2 is an open interchange format and is not encryption, not
ADR-002 acceptance, and not permission to export a real byte.

```text
adr_002_accepted=false
production_data_allowed=false
encrypted=false
projections_included=false
```

Every manifest records `production_data_allowed=false`, and a bundle claiming
otherwise is refused on read whoever wrote it. The posture block itself is read
from the profile the caller opened rather than restated here: `academic-export`
cannot open a store, so a posture it minted would be an assertion it has no way
to check. What it owns is the refusal.

## What `INV-C-015` asks for, and what the dependency list has to do with it

Section 37 ends with the sentence this format exists for: *학교 계정이나 특정 AI
vendor가 사라져도 Local Core와 export로 계속 사용할 수 있다.* The claim is not
that an export exists. It is that the export is still readable when the product
and the school account are both gone.

So `academic-export` links **no** store, vault, crypto, keystore, recovery,
retention, projection engine or transport. Its product closure is
`academic-audit`, `academic-domain`, `academic-requirement`, `serde`,
`serde_json` and `sha2`, and `the_product_closure_is_exactly_the_declared_edges`
compares that set with the manifest in both directions.

That is also why this is a separate package from `academic-portability`. All
three of that crate's lanes link `academic-store`; a v2 bundle written from it
would have put the database engine inside the closure of the artefact a user
keeps after the product is gone.

`read_bundle` takes a path. Not a key, not a passphrase, not a device
authorization, not a token, not a host, not an account, not a session, not a
provider — there is no argument to pass one as. `rerun_audit` takes a bundle and
the published rule sets the caller already holds, and nothing else.

## Layout

```text
<bundle>/
  GRADUATION_EXPORT_V2                        # plaintext marker: format + manifest version
  manifest.json                               # the provenance manifest
  inventory.md
  schemas/graduation-export-v2.schema.json
  parts/official-record-and-proof/
    part.json
    audit/frozen-inputs.txt
    audit/rule-set.txt
    audit/outcome.expected
    audit/proof-tree.txt
    claims/<domain-id>.jsonl
    record.md
  parts/lecture-and-question-archive/
    part.json
    claims/<domain-id>.jsonl
    originals/<domain-id>/<artifact-id>.bin   # only when the user included them
    archive.md
  parts/concept-and-competency-evidence/
    part.json
    claims/<domain-id>.jsonl
    history.md
  parts/repository-snapshot-and-evolution/
    part.json
    claims/<domain-id>.jsonl
    git-refs/<domain-id>.jsonl
    evolution.md
  parts/role-interest-and-alternative-paths/
    part.json
    claims/<domain-id>.jsonl
    decisions/<domain-id>.jsonl
    paths.md
  parts/machine-readable-graph/
    part.json
    canonical/<stream>/<domain-id>.jsonl
    graph/<domain-id>.jsonld
    ledger/batches/<batch-id>.cbor            # original signed envelopes, byte-for-byte
    formats.md
```

A directory, never an archive, for the reason the Phase 1 export gives: archive
containers record filesystem metadata and entry ordering that differ between
hosts.

## The six parts are section 37's, enumerated rather than counted

Section 37's closing list is the content contract:

| Part | Section 37's bullet |
| --- | --- |
| `OFFICIAL_RECORD_AND_PROOF` | 공식 성적/요건과 계산 proof |
| `LECTURE_AND_QUESTION_ARCHIVE` | 원본을 포함하거나 제외할 수 있는 강의·질문 archive |
| `CONCEPT_AND_COMPETENCY_EVIDENCE` | concept/competency evidence history |
| `REPOSITORY_SNAPSHOT_AND_EVOLUTION` | repository snapshot과 architecture evolution |
| `ROLE_INTEREST_AND_ALTERNATIVE_PATHS` | role 관심 변화와 alternative paths |
| `MACHINE_READABLE_GRAPH` | machine-readable graph와 open formats |

`graduation_bundle_contains_all_six_named_parts` parses that list back out of
the specification and compares it with `BundlePart::ALL` in both directions.
**Nothing asserts the number six.** A bullet renamed, dropped or added fails
that comparison rather than being folded into the nearest existing part.

**The sixth part is not a selection.** Its subject is the graph, so it carries
the canonical state of the exported watermark whole, and the five topical parts
are views selected out of it by the first segment of a claim's predicate
identifier. That is what makes the assignment total without inventing a seventh
"everything else" part the specification does not write: a claim whose predicate
names no section 37 topic is still exported, under the part whose subject is the
whole graph.

## Section 32.10's three per-file attributes

*export 파일에는 sensitivity label, sharing restriction, source copyright
notice를 포함.* A file carries all three or the bundle is not written, and that
is a property of the type: `FileRecord::new` takes them as parameters, the
fields are private, there is no setter and no `Default`.

`manifest.json` cannot carry its own digest, so it is not one of its own
`FileRecord`s. Its three attributes are in `semantic.manifest_attributes`
instead, because a file with no attributes is exactly what the exhaustive check
exists to catch and the manifest is a file.

**One is derived and two are recorded and then checked.**

The **sharing restriction** is a total function of the label, so the two cannot
be set to disagree — a `SECRET` file marked freely redistributable would have
both fields populated and a complete-looking manifest:

| Sensitivity | Sharing restriction |
| --- | --- |
| `PUBLIC` | `REDISTRIBUTION_PERMITTED` |
| `PERSONAL` | `PERSONAL_USE_ONLY` |
| `RESTRICTED` | `NO_REDISTRIBUTION_WITHOUT_SOURCE_PERMISSION` |
| `SECRET` | `NO_DISCLOSURE` |

The **sensitivity label** is recorded per security domain and then checked.
`Confidentiality` is a column on an artifact and on nothing else — a claim, an
event and a decision carry none — so a claims file's label is not readable
anywhere. What is readable is the domain the row belongs to, which is the policy
boundary section 32.2 draws and the vault keys by. The caller records the label
covering each domain and the writer refuses a recorded label weaker than the
strongest confidentiality that domain's own artifacts carry.

The **source copyright notice** is not derivable from anything in the ledger.
Who holds copyright in a lecture recording, and on what terms it may be kept, is
a fact about the world, and section 37 says the export respects 학교 강의
저작물의 보존·사용 조건 *그대로*. So it is recorded per domain in a
`TermsRegister` and the export fails closed on a domain the register does not
name. There is no fallback string: a fallback is how a bundle ends up asserting
terms nobody stated.

**Content files are written per security domain** for the same reason. A file
mixing two domains would carry one notice for two sets of terms, and the only
ways out of that are inventing a combined notice or silently picking one.

## Determinism

`generated_at_unix_ms` is a **parameter of the request**, not a clock this crate
reads. Two bundles of one watermark are therefore byte-identical **whole-file**,
manifest included, rather than identical except for one integer nobody can
compare; and a caller who records two different instants still gets one
`semantic_digest`, which is what the field being outside the digest is for.

`semantic_digest` is SHA-256 over the domain separator
`academic-os.graduation-export.manifest.v2`, an unsigned big-endian 64-bit
length, and the compact canonical JSON of `semantic`. Every field is bound
because the JSON structure is in those bytes; the length keeps the separator
from running into the body and binds nothing on its own.

The one clock this crate reads names the staging directory, which the publish
rename removes and which is never inside a bundle.
`the_only_clock_read_names_a_staging_directory` counts the calls across every
product file and requires exactly one, inside that function.

## Originals are a user choice with no default

`OriginalInclusion` implements no `Default` and `BundleRequest` takes it by
value. Section 37 writes the archive as *원본을 포함하거나 제외할 수 있는*,
which is a choice and therefore may not have a value somebody gets by not
deciding.

The two branches produce different manifests. A carried original has a path and
no reason; a withheld one has a `WithheldReason` and **no path**, keeping its
identity, its exact plaintext digest and its length so it stays identifiable and
verifiable against a copy held elsewhere. There is no third state where a record
names a file the directory does not hold, and `ObjectRecord::validate` refuses
both the record that carries a path *and* a reason and the record that carries
neither.

**An artifact is addressed by its own identifier, everywhere.**
`VaultLocator::derive` is a function of the domain key, the media type and the
content digest and not of the artifact identifier, so two artifacts with
identical bytes in one security domain share **one** locator. A bundle keyed by
locator would publish one file where two artifacts exist and lose one of them —
the shape `P2-A1` found as a P1 and `P2-R4` found again in its own work. The
`vault_locator` is recorded as an attribute and is a key, a filename and a path
segment nowhere. The fixture registers one byte string twice under two
identifiers so the collision is present rather than assumed, and
`original_inclusion_is_user_selected_with_no_dangling_locator` requires both to
survive at two distinct paths.

## The restore re-runs the audit rather than re-reading it

A bundle that carried "you may graduate" as text would prove only that somebody
once computed it. `rerun_audit` re-performs `P2-U3`'s work from what the
directory carries:

1. parse the frozen inputs out of `audit/frozen-inputs.txt`;
2. take SHA-256 over `audit/rule-set.txt` and require it to equal the recorded
   `rule_set_hash` — section 37's *과거 audit은 당시 rule hash로 재현된다*;
3. find, among the rule sets the **caller** supplies, the one whose canonical
   text is those exact bytes;
4. rebuild the catalogue scope from the manifest, decode the student profile out
   of the frozen inputs, and re-run section 11.1's selector;
5. evaluate the engine and byte-compare `EngineOutcome::canonical_bytes` with
   `audit/outcome.expected`.

Step 4 is what makes this a re-run. `SelectedRuleSet` has private fields and
exactly one producer, inside `academic_audit::select`, so which published rules
apply is genuinely decided again.

**The rule set comes from the caller, never from the bundle.** `P2-U2` puts a
published rule behind a two-attestation review gate, and a bundle that could
mint a `RuleSet` would be a way around it. A bundle whose rules nobody still
holds fails with `ExportError::Absent` rather than being evaluated under
different ones.

## What a reader refuses

The order is fixed and every step fails closed:

1. the plaintext format marker must be this format at this manifest version,
   before anything is parsed;
2. the manifest must parse, recompute its own semantic digest, and carry this
   format's frozen fields and a posture that does not admit real data;
3. the set of files on disk must **equal** the recorded inventory plus the
   manifest, in both directions, so an unlisted file is a refusal rather than a
   file nobody checked;
4. every recorded file must hash and measure exactly as recorded;
5. every path referenced anywhere in the manifest — an object's, the audit's, a
   part's — must appear in that inventory. This is the dangling-locator rule,
   and it is checked over the manifest's own references rather than over a list
   of the places references are known to appear;
6. the six parts must be exactly section 37's six, each with its own sentence;
7. every file record's sharing restriction must still follow from its label, and
   every one must carry a notice;
8. an object record carries a path or a withheld reason and never both or
   neither, and `originals_included` must agree with every one of them.

## What section 32.10 names that this build does not write

Section 32.10 lists *machine-readable JSON/JSON-LD, Markdown/PDF, audio 원본,
Git refs와 provenance manifest*. Every item is carried except one.

**There is no PDF.** Nothing in this repository produces PDF bytes:
`academic_lecture_document::PdfArtifact` records a rendering's digest and its
completeness and holds no page. So a bundle carries `text/markdown` as the
human-readable open format, states the absence verbatim in `inventory.md` and in
`parts/machine-readable-graph/formats.md`, and
`graduation_bundle_contains_all_six_named_parts` fails if any file in a bundle
ends in `.pdf`. Shipping a file with that extension that no PDF reader opens
would be worse than not shipping one.

`GitRef` is field for field what `academic_repository::RepositorySnapshot`
records about where a snapshot sits in history — branch, commit, parent
snapshots, submodule pins. It is a **value** rather than an edge, because a
reader must be able to read a repository's history out of a directory without
the analyser. A commit that is not a lowercase hexadecimal object name is
refused rather than written into an open format no other tool can resolve.

## Which named test proves what

| Test | Where |
| --- | --- |
| `open_export_round_trip` | `crates/export/tests/export.rs` |
| `export_is_deterministic_at_a_fixed_watermark` | same |
| `export_carries_labels_restrictions_and_notices` | same |
| `original_inclusion_is_user_selected_with_no_dangling_locator` | same |
| `clean_offline_restore_reruns_deterministic_audit` | same |
| `restore_without_vendor_or_school_account_succeeds` | same |
| `graduation_bundle_contains_all_six_named_parts` | same |
| `the_product_closure_is_exactly_the_declared_edges` | `crates/export/tests/export_scans.rs` |
| `the_product_source_reaches_only_the_declared_vocabulary` | same |
| `no_type_in_this_crate_holds_an_unclassified_byte_buffer` | same |
| `the_only_clock_read_names_a_staging_directory` | same |
| `the_portable_path_rules_match_the_phase_1_export` | same |
| `the_walk_reads_every_module_in_this_crate` | same |

All of them are pure Rust in the default workspace lane and run on every CI
platform. What the source scans read and what they still leave open is in
[policy source scans](policy-source-scans.md).

## Building and verifying

```sh
cargo clippy -p academic-export --all-targets --locked --offline -- -D warnings
cargo test -p academic-export --locked --offline
```

## Non-goals

Encrypting the bundle, importing a bundle **into** a live profile, cross-format
migration from schema v1, and any cloud destination are out of scope. The
encrypted artefact is `P2-K4`'s backup, which is a different format with a
different manifest and a different threat boundary; conflating the two would
make the open format unreadable exactly when it is needed.
