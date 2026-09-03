# Transcription pipeline and version discipline

`academic-transcription` is the `P2-L3` boundary: section 12.3's provider-neutral
pipeline, section 12.4's `TranscriptSegment`, and the rule that a correction is a
new version over an annotation layer that never touches a raw token.

It sits above `P2-L2`'s capture journal, beside `P2-M2`'s review queue, and under
`P2-G2`'s egress boundary. It records nothing, transcribes nothing, persists
nothing, and opens no socket.

## What is not here

**No recording and no speech engine.** There is no implementation of
[`SttProvider`] in this repository. Every audio chunk, board photograph and
provider response in this crate's test tree is a committed literal built in
process; the acceptance suite drives the real `academic_capture::begin` so that
the journal headers its manifests compare against are written by the real capture
surface, and nothing opens a microphone, a camera or a device.

**No store and no migration.** There is no `academic-store` edge, which is what
makes "this crate persists nothing" a graph fact rather than a sentence.
`STORE_MIGRATION_SQL` still pins the same migrations; `0008`, `0010` and `0011`
stay unclaimed and `0013` is unwritten. The durable half of a capture is `P2-L2`'s
chain-digested chunk journal, which this crate reads and never writes.

**No sandbox.** The plan lists `P2-G4` as a dependency of this task, and as built
it is a *sequencing* one. `academic-worker` carries a sandbox probe binary and
`tools/phase1-scaffold-policy.test.mjs` requires that **no** workspace crate
depend on that package, because the probe would then be reachable from a default
build. `P2-G2`'s precedent is to split rather than to weaken a guard, so this
crate is a sibling and `no_wall_clock_socket_or_file_reaches_this_crate` holds the
absence of that edge as an assertion. Running a provider in a worker is a
composition a caller performs; nothing here forges a job descriptor.

**No transport.** The scoped-remote arm produces a `RemoteAdmission`; turning one
into bytes on a wire is `academic-egress-boundary`'s, behind `P2-G1`'s broker. No
product file here names `EgressProxy`, implements `OutboundTransport`, or spells a
socket construct.

## Where the section 38 gate stands

`GATE-38-019` — cloud transcription per offering — **stays open, and this task
invents no default for it.** `SttPolicy::new` holds no approval, the remote arm
reads an approval by exact `(provider, model version)`, and there is no
configuration file, environment variable or fallback that could supply one:
`no_default_reaches_the_remote_arm` refuses `env::var`, `var_os`,
`unwrap_or_default`, `read_to_string` and `File::open` anywhere in this crate's
product source, and pins the whole route decision beside a call-site count of
one. An unconfigured profile blocks every remote request with
`PROVIDER_NOT_APPROVED`.

`GATE-38-009` is `P2-L1`'s and is untouched.

## The inputs, and what stops a caller widening them

Section 12.3's first line is the whole input set: *authorized audio chunks +
captures + supplied materials*. There are three admitted kinds and no fourth,
each a type with private fields whose one producer is a method on
`InputManifest`.

**The binding comes from the capture; the journal is compared against it.**
`AuthorizationBinding::of` takes a `CaptureRecorder`, which has no public
constructor: holding one is proof that `academic_capture::begin` ran its five
steps, and the first of those is `mint_capture_capability`. The lecture, the
capability token and the policy row all come from the recorder, and a journal
whose header disagrees is `JournalIsNotThisCapture`.

**That ordering is the whole of it, and the first version of this module had it
backwards.** It read the token identifier out of the very `JournalRecovery` it
was about to admit from, so the comparison could only catch a caller *mixing*
two journals — and `academic_capture::ChunkJournal::replay` is public and takes
bytes, so a synthesized recovery naming any token would have agreed with itself.
`pipeline_input_authorization` now drives that case: a journal built by the test
and replayed from bytes opens no binding.

Three rules hold the shape rather than one. `AuthorizationBinding::of` is pinned
whole; the set of functions in the crate that produce a binding from a journal is
compared against a one-entry list, so a second producer that reads the journal
instead fails as an extra key; and the construction is counted, because `U-G3`
records that a sweep over signatures says nothing about a body that builds its
own argument. `covers` is pinned beside a rule that it is the **first statement**
of both admitting methods — `T141`'s finding, applied in advance, because a
pinned comparison says nothing about whether it runs.

**What is still open is inherited rather than invented.** A caller holding a real
recorder can hand this module a journal they wrote themselves under that same
authorization: the frame chain detects truncation and corruption and is not a
signature, which [the capture subsystem contract](capture-subsystem.md) says in
its own words. What this module adds is that the authorization has to be one a
capture actually obtained.

`pipeline_input_authorization` builds two captures of the same lecture under the
same permission at two instants — `token_id` hashes the instant, so the tokens
differ — and observes each recorder refusing the other's journal, a forged
journal refused, every cross admission refused, both wrong-kind refusals, a frame
nothing recorded, a `MARK` frame that is in the journal and is not an input, and
every automatic actor refused as a supplier of material. The positive control
records the exact chunk digests, so an authorized run names what it read.

Supplied material is the user's own act: `admit_supplied_material` matches
`academic-domain`'s closed `Actor` enum exhaustively, so a fifth actor class
stops this crate compiling until it is classified.

## The provider contract: eight declarations, and four that are elsewhere

Section 12.3's last paragraph lists twelve declarations. Eight are technical and
are this crate's: audio format, chunk boundary, language hints, vocabulary hints,
word and segment timestamps, confidence semantics, diarization, and math/code
capability. `the_capability_fields_are_section_12_3s_own` reads the phrases out
of the specification rather than transcribing them.

The other four — data retention, training use, processing region, deletion
receipt — are **not restated here.** `P2-G3`'s `provider_policy_snapshot` in
`academic-policy` already owns them, and the same test asserts that none of this
crate's eight is one of those four, so the split is executed rather than
promised.

**Omission is not a declaration.** `ContractDraft` uses `Option` for every field
so that a fact left out and a fact declared absent are different values.
`stt_capability_contract` drops each of `CapabilityField::ALL` in turn, calling
every other setter, and requires each omission to be refused by name. A declared
absence then travels with the contract: a run that depends on a `FeatureClaim`
the contract declares unsupported is refused at `ReadProviderContract` with the
field that decided it, and the suite drives all six claims.

One contract per `(provider, model version)`. Keyed on the version because
section 12.3 requires the exact model version to be preserved so a
re-transcription can be compared, and a registry keyed on the vendor alone would
have made two versions indistinguishable at exactly the moment the comparison
matters.

## The route: default local, scoped remote, everything else blocked

Three outcomes and no fourth. `SttPolicy::route_for` is a total `match` over
`ProviderPlacement::ALL`, and the placement is a field of the *contract*, so
"run this one remotely" is not a request a caller can make.

| Arm | What it needs | What it carries |
|---|---|---|
| `Local` | nothing; it is the default route for raw audio | the provider and model version |
| `ScopedRemote` | all three of `REQ-32-040`'s facets | a `RemoteAdmission` with the approved retention |
| `Blocked` | — | which of the three facets was missing |

`RouteDenial` has exactly three variants because `REQ-32-040` names three facets:
the permission covers external processing, the user approved the exact provider,
and a retention is declared. `stt_provider_policy` drives each of them against an
approval that is complete except for the facet under test, drives an approval for
the same vendor's *other* model version, and observes a blocked run halting at
`SelectProviderRoute` with nothing asked and nothing retained.

**The absence of configuration never falls through to remote.** A new profile
holds no approval and every remote request is `PROVIDER_NOT_APPROVED`. That is a
structure rather than a check: `SttPolicy::new` builds an empty list, the remote
arm looks one up by exact key, and the whole of `impl SttPolicy` is pinned as
text so a second constructor is an edit to a constant.

**The remote arm is not permission to transmit.** What it produces is a
`RemoteAdmission`, and the only function that consumes one —
`ProviderResponse::from_remote` — also takes an
`academic_egress_boundary::AcceptedResponse`, whose one producer is
`EgressProxy::accept_response` behind `PermissionBroker::execute` and
`bind_grant`. `a_remote_response_comes_through_the_egress_boundary` builds a real
proxy over a real broker and drives the whole scoped-remote run through it.

The response's placement is decided by which constructor built it, and the
`Transcribe` stage compares it against the arm the route admitted — so a `Local`
route that came back with a remote-built response, or the reverse, is
`RouteMismatch`.

### What the rulepack refused, and what that changed

The provider wire grammar's per-token line was `token: <start> <confidence>
<text>` until the scoped-remote row was written. `P2-G2`'s shipped rulepack has
`token` on `generic-credential-assignment`'s needle list with `min_value: 8`, so
every `token: 1000000000` line read as a credential assignment and
`accept_response` refused the whole response with `CANARY_IN_RESPONSE`. The key
is now `word`, which is what the record is. This is recorded because the shape
generalizes: **a wire grammar whose key is a word on the DLP needle list cannot
cross the egress boundary**, and the failure is a refusal of the whole payload
rather than a redaction of one line.

## The raw response, and the one form it leaves in

Every raw provider response is retained. `RawResponseArchive` has exactly one
`&mut self` method and it pushes: there is no removal, no replacement, and no
`&mut` accessor into an entry, which is ADR-003's rule rather than a second
mechanism invented here. `the_archive_appends_and_nothing_removes` pins the whole
`impl` and holds the mutating surface at one method, and refuses `remove`,
`retain`, `clear`, `truncate`, `drain`, `pop` and `swap` on the entry vector.

`raw_stt_response_immutable` runs two providers over one manifest, observes both
entries present with their own digests and their own model versions, and observes
the first entry byte-identical after the second arrives.

**The bytes have one accessor and it is crate-private.**
`ProviderResponse::response_bytes` is `pub(crate)`, and every call site is
inventoried below with a written reason. The inventory counts the accessor's
**name**, with a non-identifier byte required on each side, less declarations of a
function named exactly that — which is `P2-RF10`'s and `P2-RF11`'s repairs to
`Untrusted::expose`'s inventory copied deliberately rather than reinvented.

| Site | Why |
|---|---|
| `RawResponseArchive::retain` | Sealing has to read the bytes it hashes and wraps. What leaves is an `Untrusted<IngestedDocument>`, which implements no `Deref`, no `Display` and no `Into`, and whose one accessor is private to `academic-untrusted-content`. |
| `decode` | The wire grammar has to read the response it validates. What leaves is a closed record of segments and tokens whose fields are private and whose one producer is that function, and no byte of the response itself. |

Crate-private stops a caller from calling it; it does not stop this crate from
calling it on a caller's behalf. So a second rule runs beside the inventory, and
it runs over **every package in `crates/`**, because `ProviderResponse` and
`ArchivedResponse` are public types any crate can name: no `pub` signature may
take one and return a type naming `str`, `String` or `u8`.

**The `P2-G5` reuse is the argument type.** `ArchivedResponse::labelled` returns
`&Untrusted<IngestedDocument>` and there is no other route out of the archive.
`crates/transcription/src/response.rs` is the fifth entry in
`ACCEPTED_RESPONSE_FILES`, and `the_accepted_response_is_sealed_immediately` is
the half that scopes it: `from_remote` is the only function in this crate taking
an `AcceptedResponse`, and no product file here names `EgressProxy`, so a second
unlabelled response cannot be produced locally either.

## Section 12.4's record, and the raw layer under it

`RawToken`, `RawSegment` and `RawTranscript` have private fields, no `&mut self`
method, no setter and no `From`. `RawSegment` hands out `&[RawToken]` and never
`&mut [RawToken]`; `RawTranscript` has no mutating method at all.

**Nothing writes a raw token, and four things hold that rather than one
sentence.**

1. **The compiler.** Every field of all three is private, so a struct literal for
   any of them is an error outside `transcript.rs`. `raw_token_write_protection`
   checks the condition that rests on — each declaration carries exactly one
   `pub` — and three `compile_fail` cases observe the three refusals with their
   diagnostics committed.
2. **The whole `impl` set.** The set of `impl` blocks in this crate whose header
   names a raw type is compared against a pinned list, so an implementation of a
   trait nobody predicted fails as an extra key. A token list of forbidden trait
   names runs beside it and is the weaker half.
3. **A workspace-wide signature sweep.** No `pub` signature anywhere in `crates/`
   may take or return a mutable raw value.
4. **A workspace-wide scope rule.** No file outside `crates/transcription/` names
   a raw type at all, which is `P2-U6`'s
   `credentials_never_reach_a_general_crawler` shape. It is a tripwire for
   `P2-L4`, the first task that will.

The three assemblies — `parse_token`, `OpenSegment::close`, `decode` — are pinned
as whole text, and `decode` is held at one caller in this crate's product source.

`raw_token_write_protection`'s behavioural half applies three corrections over
four versions and observes the raw token digest and every raw token's text
unchanged, and every version still reading the raw token beside the effective
one.

### The wire grammar

```text
academic-stt-response/1
segment: <id> <start_nanos> <end_nanos> <speaker> <chunk,chunk,…>
verbatim: <one line, no control character>
word: <start_nanos|-> <confidence_units|-> <text>
```

Three keys and no fourth. `a_malformed_provider_response_is_refused` drives an
unknown key, a missing key, a duplicate key, a field count that is not the
record's, a value that is not a number, an empty segment interval, a token
outside its segment, segments out of order, a response with no segment, an
unknown speaker spelling, and both banner failures — and then requires **every**
`DecodeFault` variant to have been produced by one of the cases.

Three of the refusals are about the contract rather than the grammar: a word time
against a `SEGMENT_ONLY` declaration, a missing confidence against a `PER_TOKEN`
declaration, and an attributed speaker against a provider that declared no
diarization. `REQ-12-025`'s failure mode is a provider swap changing semantics
unnoticed, and those three are where a declaration and an answer are compared.

Speaker is section 12.4's own three shapes: `instructor`, `student_unknown_<n>`,
`unresolved`. The ordinal distinguishes two unresolved students and is not an
identity; `P2-L5` is where student voice and the PII hold live.

## Annotations are the layer that may be thrown away

Section 12.4 names four things that live outside the token stream: punctuation,
paragraphs, speaker labels, and mathematical formatting. `AnnotationLayer` holds
no raw token and no reference to one — it borrows the transcript to validate a
range and keeps nothing — so applying or removing an annotation cannot change
`RawTranscript::token_sequence_digest`.

The raw transcript is append-only and a correction is a new version. The
annotation layer is the opposite on purpose: it is derived, so it can be emptied
and rebuilt. `annotation_layer_separation` applies each of
`AnnotationKind::ALL`, removes each kind independently and checks the others
survive, empties and rebuilds the whole layer to an equal digest, and checks the
raw token digest at every step. A formatting change is a version too, because a
reader has to be able to say which rendering they saw.

## Corrections are versions, and there are three dispositions

`academic-domain`'s `DecisionAction` has exactly three arms and `P2-M2` is where
they are the queue's vocabulary. `LineageEffect::of` is a total `match` over that
closed enum, so **a fourth disposition stops this crate compiling until it says
what it does to a lineage** — which is how "do not invent a fourth" is held by
the compiler rather than by this sentence.

| `DecisionAction` | Section 3 | Effect | How the version is built |
|---|---|---|---|
| `Confirm` | 승인 | appends version *n+1* | `SettledCorrection::confirmed` takes the `Approved<CorrectionCandidate>` that `ReviewQueue::commit` produces only after a user `CONFIRM` for that exact proposal is in its history |
| `Replace` | 수정 | appends version *n+1* | ADR-003 has a replacement reject the target and select a different object, so `commit` refuses to release the model's payload; the user's own candidate is passed in beside the disposition record that names the proposal |
| `Reject` | 거절 | appends nothing | there is no constructor at all, so a rejected correction is not a value `append_correction` can be handed |

`user_correction_lineage` drives all three through a real `ReviewQueue` at
`MEDIUM_REVIEW`, observes the rejection releasing nothing and being retained,
observes `commit` refusing a `REPLACE`, observes a `CONFIRM` record refusing to
mint a replacement, and observes the raw token digest and the retained provider
response unchanged after all three.

`CorrectionAuthor` has two arms, one per appending disposition. It is not a
fourth disposition: it records which of the three that were already recorded
produced the version, so a reader of the lineage can tell a confirmed model
candidate from the user's own replacement without going back to the queue.

`CorrectionStatus` is section 12.4's field and has three values.
`NEEDS_REVIEW` is a projection of the queue's own append-only history:
`TranscriptLineage::open_review` records that a correction is open. **What decides
that a token needs review is the caller's**, and this crate says so rather than
implying otherwise: a provider's raw confidence is an
`academic_model_run::RawScore` with no readable units, and turning one into a
comparable number needs that crate's `CalibrationRegistry`, which this crate does
not carry.

## Comparing two providers without ranking them

Section 12.3 requires the raw response and the exact model version to be kept so
a re-transcription can be *compared*. `P2-M1` requires that a provider's raw
number and another provider's raw number are never ordered. Those two are
compatible, and this is where the line runs.

What is reported is where the two disagree, symmetrically. Neither side is a
baseline — they are `Left` and `Right` — and `divergence_digest` sorts the two
runs by their own identity and sorts both sides of every count before hashing, so
`compare(a, b)` and `compare(b, a)` carry the same digest.
`provider_retranscription_compare` observes that equality, which makes "the
comparison is not an order" executable rather than asserted.

What is not reported is which one is better. `ProviderRun` and
`RetranscriptionComparison` implement neither `PartialOrd` nor `Ord`, the whole
`impl` set naming a comparison type is compared against a pinned list, the three
derive lists are pinned, and no identifier in `compare.rs` spells `winner`,
`better`, `preferred`, `rank`, `score`, `best`, `worse` or `prefer`.
`two_provider_runs_are_not_ordered.rs` drives `<`, `sort`, `max`, a
`BTreeSet<ProviderRun>` and `>` on the comparison itself as five compile errors.

**What this does not claim.** It does not claim a caller cannot invent an order
out of the agreement counts; `RawScore`'s contract makes the same distinction.
What it claims is that this crate offers none, and that a provider's own
confidence number cannot be read out at all.

**The alignment is positional** — segment *i* against segment *i*, token *j*
against token *j*. That is deterministic and it is not a sequence aligner: an
insertion at the front of a segment reports every token after it as divergent.
What the report is for is telling a reader where to listen; `P2-L4`'s coverage
validator is where mapping quality is judged.

## The pipeline, stage by stage

The stages run in section 12.3's order. A stage that fails ends the run and no
later stage runs; the record names the stages that were **reached**, so
`lecture_pipeline_dag` reads the prefix rather than inferring it. That is
`P2-U6`'s `ingestion_stage_order_is_strict` shape, reused rather than reinvented.

**The stages are enumerated, not counted.** Every rule iterates `Stage::ALL` and
nothing asserts how long that list is.

`ReadProviderContract` runs before `SelectProviderRoute` because the placement a
route decides over is a field of the contract. `NormalizeTranscript` decodes and
records the `ModelRun` in one step: a transcript nothing recorded a run for has no
provenance, and a run recorded for a response that did not decode names an output
that does not exist.

**One stage has no failure of its own, and saying so is more honest than
inventing one.** `FanOutDownstreamJobs` derives three handles from values every
earlier stage has already validated — the job list is `DownstreamJob::ALL`, each
identifier is a digest over that constant and the input digest, and the input
digest is the manifest's own. `INFALLIBLE_STAGES` names it with that reason and is
compared against `Stage::ALL`, so a stage added without an arranged failure has to
be classified. What covers it instead is the positive control, which asserts the
run reached it and then asserts every property of what it produced.

### The fan-out

Section 12.3's diagram ends in three arrows out of one box.
`the_downstream_jobs_are_section_12_3s_own` reads those three lines out of the
specification's own fenced block and compares them against `DownstreamJob::ALL`
in order, and checks that every stage claiming a box in the diagram has one.

Each job gets an identifier of its own and every one cites the **same** input
digest, which is what `t001`'s `REQ-12-024` row means by "independent IDs, shared
input hash". The one AI-authored job is marked `produces_proposals`, because
section 27.1 lets a model produce a candidate and section 27.2 does not let it
decide. **This crate runs none of the three**: `P2-L4` owns the lossless document
and the coverage validator, and `P2-M2` owns the queue the proposal jobs feed.

### Every run records `P2-M1`'s twelve fields

There is no provenance record of this crate's own. Seven of the twelve are the
caller's (`RunIdentity`); the other five are derived, so a run cannot record a
provider it did not ask or a transmission its route did not permit:

* `provider` and `model_version` come from the selection, and the `Transcribe`
  stage refuses a response that names another;
* `input_artifact_refs` is one reference per admitted input, so the count is the
  manifest's;
* `transmitted_byte_ranges` is `LocalOnly` for a local run — a local run that
  carries a transmission record is `LocalRunTransmitted`, and a scoped-remote run
  that carries none is `NoTransmissionRecord`; and
* `retention_declaration` is the approval's for a scoped-remote run and the
  spelled constant `LOCAL_ONLY_NO_EXTERNAL_RETENTION` for a local one, because
  `ModelRun::record` takes all twelve by value and an absence has to be spelled.

`ModelRun::record` has exactly one call site in this crate's product source, and
`the_transmission_is_decided_by_the_route` pins the function that reaches it.

## What the plan and the specification disagree about

| Plan says | What is true | Resolution |
|---|---|---|
| `P2-L3` closes `REQ-34-002`–`REQ-34-005` | those four are section 34.1's STT-error detection, prevention, recovery and uncertainty display | this task builds the **prevention** half (the declared chunk overlap, the vocabulary and language hint declarations, and the multi-provider comparison) and the **recovery** half (a scoped correction appends a version and the original audio locator survives as the manifest's chunk digests). It builds no detector and no display: `REQ-34-002`'s four signals and `REQ-34-005`'s underline are not here, and `REQ-34-005` is a `packages/ui` row. |
| `P2-L3` closes `REQ-12-031` and `REQ-12-046` | those are the redaction projection and the normalization alignment diff | neither is here. This crate produces no redacted projection and performs no text normalization; a correction is a version and not a normalization pass. |
| `P2-L3` closes `REQ-27-011` and `REQ-33-010` | "AI must not replace the original transcript" | what is true here is narrower and executable: the raw transcript is owned by the lineage and handed out by shared reference, no summary or answer is produced by this crate at all, and `raw_token_write_protection` is what stops any later output replacing it. The end-to-end claim belongs to the task that produces a summary. |
| the acceptance row is `provider_technical_manifest` for `REQ-12-025` | the plan's `P2-L3` row names `stt_capability_contract` | this crate uses the plan's `P2-L3` spelling. |

The specification is authoritative in every row.

## Open

| # | What is open | When it starts mattering |
|---|---|---|
| T-1 | The alignment in `compare` is positional. Two runs that agree on every word but disagree on where one segment ends report every token after the boundary as divergent. | The first re-transcription against a provider whose segmentation differs, which is the common case for a provider swap. Closing it means a sequence aligner, which is its own reviewed piece of work and is `P2-L4`'s neighbourhood. |
| T-2 | The provider wire grammar is this crate's own and no provider in the world speaks it. A real adapter has to translate, and that translation is where a provider's semantics are actually interpreted — the place `REQ-12-025`'s "provider swap changes semantics silently" failure lives. | The first real adapter. What is here bounds the damage: the contract declares what the provider claims, and the decoder refuses a response that contradicts the declaration. |
| T-3 | `CorrectionStatus::NeedsReview` is a projection this crate maintains in memory beside `academic-proposal`'s append-only history, and nothing compares the two. A caller that settles a proposal without calling `open_review`/`append_correction` leaves the projection stale. | A daemon that owns both, which is `P2-M4`'s layer. Closing it means the queue owning the projection, or a reconciliation. |
| T-4 | The audio a provider reads is public: `AuthorizedChunk::audio` returns the buffer, because a provider is an implementation of `SttProvider` outside this crate. It adds no reach — `academic_capture::CaptureBytes::as_slice` already returns the same bytes one crate over — but it means the redacting `Debug` is the whole of what protects a lecture recording from a log, and nothing protects it from a caller that wants it. | The first provider adapter that is not a fixture, which is when "who may hold the audio" becomes a real question. |
| T-5 | Nothing writes the archive to disk. A re-transcription compares two responses that are both in memory, and a process restart loses both. | The first transcription that outlives its process. Closing it means sealing the archive under `AEAD_CHUNKED_V2`, which is `academic-capture`'s open `C-8` restated one layer up and the same dependency admission neither task has made. |

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`, the acceptance public key is unprovisioned, and the
committed candidate receipt carries two of five platform rows. No recording is
made, no device is opened, no provider is called, no byte leaves the machine, and
every fixture in this crate's test tree is synthetic and built from committed
literals.
