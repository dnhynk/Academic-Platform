# Lossless lecture document, coverage validator, render QA

`academic-lecture-document` is the `P2-L4` boundary: section 12.5's
machine-readable `LectureDocument`, section 12.6's deterministic
`CoverageValidator`, the render QA that stands between a rendering and the word
"complete", and the `StudyIndex` that is allowed to leave things out and says
so.

It sits above `P2-L3`'s transcript, reads `P2-L2`'s capture journal, and is
registry entry seven of `P2-C5`'s deterministic engine harness. It renders
nothing, persists nothing, transmits nothing, and reads no clock.

## What is not here

**No renderer.** There is no PDF engine, no layout engine, no font and no image
decoder in this repository. [`RenderQa::inspect`] takes *measurements* a
renderer produced — a page's content height against its frame, whether a box was
clipped, whether an image resolved, how many glyphs came back missing — and
every measurement in this crate's test tree is a committed literal. Saying that
plainly is the honest half; what the module adds is that the four defects are a
closed set compared against section 12.6's own sentence, that a measurement
which does not cover every node of the document is a refusal rather than a clean
report, and that any defect denies the completeness witness.

**No store and no migration.** There is no `academic-store` edge, which is what
makes "this crate persists nothing" a graph fact rather than a sentence.
`STORE_MIGRATION_SQL` is untouched: `0013` and `0016` stay unclaimed, `0017` is
`P2-X7`'s, and `0018` is unwritten. A `LectureDocument` and a `CoverageReport`
are pure functions of a transcript, a journal and a configuration; there is
nothing here whose value would survive a restart that is not already durable one
layer down.

**No raw token, and no raw type named at all.** `P2-L3` holds a workspace-wide
rule that no file outside `crates/transcription/` names `RawToken`, `RawSegment`
or `RawTranscript`, and recorded it as "a tripwire for `P2-L4`, the first task
that will". **It is not.** The document is built over `TranscriptSegment` and
`EffectiveToken` read at one version, which is the same discipline stated as a
graph fact instead of a promise. `the_document_names_no_raw_type` asserts it
from this side too, with a control that this crate does read a transcript.

**No transport and no broker.** `academic-policy` and `academic-egress-boundary`
are not edges of either kind, so a product file here cannot name
`PermissionBroker`, `CapabilityToken` or `EgressProxy` — an undeclared crate is
a compile error, not a lint.

## Where the section 38 gates stand

None are opened or closed. The plan's `P2-L4` row says the confidence and gap
thresholds are versioned configuration with recorded defaults, and that is what
they are: a threshold that can be superseded and dated is a decision the user
makes per profile, not a product question waiting on an answer.

### The recorded defaults

| Field | Default | Why this number |
|---|---|---|
| `version` | 1 | The first published configuration. |
| `gap_threshold_nanos` | 2000000000 | `P2-L2` records a frame's session instant and not its duration, so what is measurable is the elapsed distance between two consecutive audio frames. Two seconds is above any chunk cadence the capture subsystem writes and below the shortest hole a listener would call a hole. |
| `low_confidence_at_or_below_permille` | 700 | A calibrated seven-in-ten is where section 12.6 wants a span in front of a person rather than in a document. |

`the_recorded_defaults_are_the_documented_ones` reads this table out of this file
and compares it against `COVERAGE_CONFIG_V1`, so a threshold changed in code and
left undocumented fails rather than drifting.

## Section 12.5's document

A node is one of five kinds — section, paragraph, equation, code block, capture
placement — and carries an identifier, its rendered text, one or more source
mappings, the captures placed beside it, its annotations, and at most one
ordering cross-reference.

A mapping names a segment index, the identifier the provider gave that segment,
a character range into its verbatim text, and one of section 12.5's nine
transforms. Three things have to hold before the builder admits it, and the
third is the one that matters:

1. the segment exists at the document's version and its identifier matches;
2. the character range is inside the verbatim text and covers at least one
   token; and
3. **every token the range covers still occurs, in order, in the rendered
   text.**

The third does not read the transform at all. A rendering that drops a word, or
replaces it with a paraphrase, or reorders two, fails it under **every one of
the nine** — `lossless_transform_allowlist` drives all three bypasses against
all nine rather than against the one that sounds relevant.

**What it does not catch is insertion**, and that is deliberate: punctuation, a
heading, a timestamp and a speaker label are insertions, and they are the
allow-list's whole content. An inserted *claim* is the `StudyIndex`'s problem,
not preservation's.

### The token alignment, and why it is here

A segment's verbatim text and its tokens are two fields of one record, and
nothing in `P2-L3` compares them. `token_spans` locates each token in the
verbatim text by a left-to-right scan from the end of the one before it, which
is deterministic and is also the token alignment section 12.6 asks for: a
segment whose verbatim text does not contain its own tokens in order is refused
rather than mapped.

### The allow-list is the specification's own

`PreservationTransform::ALL` carries the phrase section 12.5 uses for each of
its nine members, and `lossless_transform_allowlist` reads that sentence out of
the specification and compares the **whole set** in order. A tenth transform
fails against the specification rather than against a second list written here,
and a spelling that is not one of the nine has no value a caller can name.

## Section 12.6's coverage

| Check | What it measures |
|---|---|
| segment coverage | mapped segments over eligible segments, less the ones declared non-speech |
| token coverage | mapped tokens over all tokens, less the ones in a non-speech segment |
| ordering | the lowest segment each node maps is non-decreasing, unless a cross-reference names the segment it goes back to |
| captures | every authorized capture is placed or excluded with a reason from a closed set |
| gaps | no hole in the audio timeline above the configured threshold that the journal does not explain |

### Exactly one status, and where each one comes from

`SegmentStatus` has four variants and a `SegmentAccount` has **one** field of
that type. Two statuses is not a value — there is no set, no vector and no
`Option`. Zero statuses is not a value, for the same reason. An unknown status
is not a value, because the enum is closed and carries no `#[non_exhaustive]`.
A non-mapped status without its evidence is not a value, because the evidence is
a field of the variant. Each of those is a `compile_fail` case with its
diagnostic committed.

| Status | Who decides | What it needs |
|---|---|---|
| `MAPPED` | **derived**: the document maps the segment | nothing; there is no `SegmentDisposition::mapped` at all, which is what stops a coverage number being asserted rather than measured |
| `EXCLUDED_NON_SPEECH` | a person | a reason from a closed four-value set, and the deciding actor; every automatic actor is refused by an exhaustive `match` over `academic-domain`'s closed `Actor`, so a fifth actor class stops this crate compiling until it is classified |
| `REDACTED_WITH_POLICY` | a person | a policy digest, a basis from a closed three-value set, and the deciding actor |
| `UNTRANSCRIBED_FAILURE` | **the journal** | a frame sequence whose body is a `Gap`; the cause comes out of the frame, so a caller cannot name one the recording did not have |

The one property that is genuinely about two inputs — a segment both mapped and
declared — is a total `match` over `(mapped, declared)` whose four arms the
compiler enumerates. Three produce an outcome and the fourth is a refusal, so a
report that exists partitions its segments and there is no report in which a
segment carries two statuses.

**`UNMAPPED` is the absence of a status, not a fifth one.** Section 12.6 lists
four statuses and then says a single unmapped segment makes the document
`INCOMPLETE`; those are two sentences and this crate keeps them apart.

### The failure status, and what it does and does not cover

A segment that is in the transcript was transcribed, so `UNTRANSCRIBED_FAILURE`
is the status of a segment the pipeline recorded as failing. This crate's only
producer of one reads a journal gap frame. A span of audio with **no** segment
at all is not a segment status: it is the gap check, which section 12.6 lists
separately, and the two are not merged here.

### The gap check has no length across a clock change

`P2-L2`'s `SessionTick::offset_from` refuses a distance between two clocks, and
inventing one here would be the same error one layer up. A hole between two
audio frames of the same session clock has a length and is a finding when that
length is **above** the threshold. A hole across a clock change has no length,
and it is **always** a finding, because unknown is not below a threshold —
folding it into a pass would manufacture a verdict, which is the rule
`InputValue::Unknown` already states for the engine harness. A resume writes a
gap frame, so such a hole is normally explained; the report says both things.

### The two denominators are not the same, because section 12.6 does not write
### them the same

Section 12.6 states the ratios as:

```text
segment coverage = mapped non-silence transcript segments / all eligible segments
token coverage   = mapped normalized tokens / all normalized tokens
```

**The segment line carries `non-silence`. The token line carries no qualifier.**
The implementation reads them apart on exactly that difference:

| ratio | numerator | denominator | what `EXCLUDED_NON_SPEECH` does |
|---|---|---|---|
| segment | mapped segments | eligible segments less those declared non-speech | leaves both sides |
| token | mapped tokens | **all** normalized tokens | leaves the numerator only |

A redacted segment and a recording failure are eligible and unmapped on both
lines, so they lower both ratios and the document is `INCOMPLETE`.

**The segment line has two readings and only one of them is closed.** `non-silence`
can qualify the numerator alone — "the mapped, non-silence segments, over all
eligible segments" — in which case a declaration would lower the segment ratio
too and the subtraction above would be wrong. It can equally restrict the whole
ratio to the non-silence subset, which is what the code does. Nothing in section
12.6 chooses between them, so the code keeps the reading it shipped with and
`section_12_6_states_both_ratios` parses both lines back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` in both directions: if the
document stops writing the qualifier on one line and not the other, that test
fails and this paragraph has to be rewritten rather than quietly diverged from.

**The token line has one reading, and the code used to break it.** Until `P2-RF20`
the non-speech tokens were subtracted from the *token* denominator as well, on
the strength of the segment line's word. `RawSegment::close` refuses a
zero-token segment, so every segment a caller can declare non-speech holds at
least one transcribed word — the subtraction was therefore not an edge case but
the only way the status could ever be used. `P2-A4` measured a document
rendering **one of the fixture's twenty-one tokens** minting a
`CompletenessWitness` and reading `COMPLETE` on Windows native and on WSL2, with
four transcribed segments of real lecture speech declared `SILENCE` and absent
from the PDF. Under section 12.6's own denominator that document is `1/21` and
`INCOMPLETE`.

### Why a redaction — or a non-speech declaration — cannot be complete

**This has a cost worth stating**: a lecture with one student's voice redacted
can never carry a completeness badge, and neither can one with a single segment
declared non-speech, because that segment's tokens stay in the token
denominator. It should be so. The document no longer contains everything that
was said, and section 34.1's row for lecture-PDF information loss is about
exactly that. `EXCLUDED_NON_SPEECH` remains an account of where a segment went —
section 12.6 requires every segment to carry one of the four statuses — but it
is not a discount on the completeness claim.

### The unmapped condition is implied, and is kept anyway

`CoverageReport::completeness_witness` requires an empty unmapped list *and*
whole segment coverage. The first is implied by the second: an unmapped segment
is in the denominator and not in the numerator. An injection deleting the
unmapped condition passed every row of the suite unchanged, and that is recorded
rather than hidden. The condition stays because it is section 12.6's own
sentence and because the implication is a property of the *denominator rule*,
which is configuration-shaped. The implication itself is asserted over all 2101
shapes of the partition sweep rather than assumed.

## Incomplete is the only value with no measurement behind it

`PdfArtifact::render` writes `DocumentCompleteness::Incomplete` and replaces it
only when it holds a `CompletenessWitness`, whose fields are private and whose
one producer is `CoverageReport::completeness_witness`. There is no completeness
parameter, no setter, and no second producer — the whole `impl` is pinned, the
witness construction is counted at one site, and the `COMPLETE` upgrade is
counted at one caller.

These have to hold for a witness, and there is no argument that relaxes any of
them: no unmapped segment, whole segment coverage, whole token coverage, no
ordering finding, no unaccounted capture, no unexplained hole, and a partition
that reconciles. They are listed and not counted — this page said "six" and the
function's own doc comment said "five" while the code checked seven, which is
`P2-A4`'s F7 and the sixth count/list disagreement this Run has measured.

## The PDF is a rendering, and it is a sink

`PdfArtifact` records which document was rendered, that document's digest, the
digest of the bytes a renderer produced, and what the measurement says. It holds
no page.

Nothing anywhere in `crates/` has a public signature that takes a `PdfArtifact`
and returns a `LectureDocument`, a `CoverageReport`, a `CompletenessWitness`, a
`TranscriptLineage`, a `SegmentAccount` or a `StudyIndex`. That is a rule over a
**pair of types** rather than a list of function names nobody may write, so a
route from the rendering back to the record fails however it is spelled.
`pdf_non_authority` also drives the behavioural half: discarding the rendering
changes no digest of the record, re-rendering from the same record produces the
same artifact, and a rendering of different bytes is a different artifact with
the same completeness.

## The study index is a separate artifact and says so

Three obligations, each a structure rather than a sentence:

* **Separate artifact.** `StudyIndexId` is a distinct type from `DocumentId`
  with no conversion in either direction, and `PdfArtifact::render` takes a
  `LectureDocument` — passing a `StudyIndex` is a committed compile error.
* **Visible disclosure.** `STUDY_INDEX_DISCLOSURE` is a constant carried as a
  field with no setter and no constructor parameter, so there is no study index
  whose disclosure is missing, empty or something else.
* **Not a replacement.** A study index has no completeness of any kind, and
  every entry links to a node of the document it names, so section 35's
  round-trip holds.

`Salience` lives here and nowhere else. It is what a summary ranks by, and the
whole set of files that may name it is one. An index that drops every
low-salience entry is a legitimate index, and the document and the coverage
report are the same values afterwards — which is what
`no_low_importance_deletion` measures, beside the type-level half: neither
preservation type offers a method that returns less than it holds, and what a
coverage run reads is pinned whole so an added ranking parameter fails as an
extra field however it is named.

## The review queue

Three risk classes and no fourth: equation, code, and low confidence. A
paragraph that is none of them does not enter, which is the half of
`REQ-04-005` that is about *excessive* review rather than missed errors.

Every item carries an `AudioLocator` **by value**, and a locator with no chunk
is refused at construction, so "never orphaned text only" is a shape rather than
an assertion.

**A raw provider confidence is never compared.** `P2-M1` says a provider's raw
number has no readable units and no ordering, so a low-confidence span is
decided by `CalibrationRegistry::interpret` against the configured permille. A
token whose score has no usable dataset **enters the queue** rather than passing
it: having a number nobody can read is a reason to look, not a reason to trust.
A provider that declared segment-level confidence produces no token score at
all, and that is not a low-confidence signal — there is no number.

The instant a dataset's freshness is judged against is an argument, because this
crate reads no clock.

## The deterministic engine

Section 12.6's first sentence is "CoverageValidator는 deterministic하다", and
this repository already has the shape that makes such a sentence executable.
`TRANSCRIPT_COVERAGE` is `P2-C5` registry entry seven and was `PLANNED` until
this task; it is now `IMPLEMENTED`, with a corpus under
`testdata/engines/transcript_coverage/` rendered from one builder in this crate,
executed against the real engine and byte-compared. `docs/contracts/engine-harness.md`
says an entry that flips without that second half "has satisfied the audit and
demonstrated nothing", and `crates/lecture-document/tests/lecture_document_harness.rs`
is that half.

The engine implements exactly one published rule set. A presented
`RuleSetHash` that is not the digest of `RULESET_TEXT` is refused rather than
evaluated under, because an outcome's canonical bytes bind a hash the evaluation
never read.

Two encodings exist and both are total functions of the report:
`CoverageReport::canonical_bytes` is the whole report including every segment's
evidence, and `freeze` is the engine's view — the counts and statuses a
completeness verdict is a function of, in `P2-C5`'s `key=value` grammar, which
admits integers and identifier-shaped references and no free text at all.

A segment status the engine does not recognise is a `CONFLICT`, and
`EngineOutcome::new` refuses a `SATISFIED` result over one, so an unknown status
cannot pass silently.

## What the plan and the specification disagree about

| Plan says | What is true | Resolution |
|---|---|---|
| `P2-L4` closes `REQ-34-006`–`REQ-34-009` | those are section 34.1's equation/code detection, prevention, recovery and uncertainty display | this task builds the **prevention** half — a typed equation and code node, the capture placed beside it, and the verbatim rendering that the token rule enforces — and the **uncertainty** half as a document annotation (`UNVERIFIED_EQUATION`, `UNVERIFIED_CODE`). It builds no OCR comparison, no LaTeX compile and no syntax check: `REQ-34-006`'s validators are not here, and `REQ-34-009`'s badge is a `packages/ui` row. |
| `P2-L4` closes `REQ-34-012` | that is omission *recovery*: re-transcribing remaining audio and marking a reconstruction `RECONSTRUCTED` | not here. This crate re-transcribes nothing and mints no reconstruction status; what it holds is the detection half — the gap finding with its length and whether the journal explains it, which is `REQ-34-013`'s content. |
| `P2-L4` closes `REQ-34-016` | that is PDF-loss recovery: regenerate, retain prior versions, emit an omission report | partly. Re-rendering from the same record produces the same artifact and the report *is* the omission report, with the unmapped list and every finding. Retaining prior PDF versions is storage, and this crate persists nothing. |
| `P2-L4` closes `REQ-12-046` | that is the normalization alignment diff | the alignment is here — `token_spans` is it, and a segment whose verbatim text does not contain its own tokens is refused. The *diff report over a normalization pass* is not, because this crate performs no normalization. |
| `P2-L4` closes `REQ-04-004` | "post-class lecture documents preserve original audio and every transcript segment" | the segment half is here and is the whole of `segment_status_exhaustive`. The audio half is `P2-L2`'s journal, which this crate reads and never writes; nothing here can delete a chunk. |
| the acceptance rows name `packages/ui` surfaces | `REQ-12-045`, `REQ-34-013`, `REQ-34-017` and `REQ-34-009` are display rows | this crate produces the values a display would read — the unmapped count, each gap's length and whether it is explained, the completeness and its counts, the two unverified annotations. It renders none of them. |

The specification is authoritative in every row.

## Open

| # | What is open | When it starts mattering |
|---|---|---|
| D-1 | The token-preservation rule catches deletion, substitution and reordering, and not insertion. A rendering that adds a sentence the lecturer never said passes every check here. | The first renderer that is a model rather than a formatter. Closing it means a rule about what may be *added*, which is a different contract from a preservation one and is `P2-M2`'s neighbourhood. |
| D-2 | `token_spans` locates a token by its next occurrence in the verbatim text. A provider whose verbatim text is not a concatenation of its own tokens — a different normalization, a dropped filler — is refused rather than aligned, so such a provider cannot be used at all until an aligner exists. | The first real adapter whose verbatim text and token list disagree. `P2-L3`'s `T-1` is the same shape one layer down. |
| D-3 | `RedactionPolicyRef` holds a digest this crate does not resolve. What the digest names, and whether the redaction it authorises actually happened, is `P2-L5`'s. | `P2-L5`. What is closed here is that a `REDACTED_WITH_POLICY` status cannot exist without a reference and a deciding person. |
| D-4 | Render QA reads measurements. Nothing in this repository produces one, so the four defects are checked against numbers a caller supplies. | The first renderer. What is here bounds the damage: a measurement that does not cover every node is a refusal, and a placed capture the measurement does not mention is a missing image rather than a pass. |
| D-5 | Nothing writes a document or a report to disk. Two runs over one process agree by construction; two runs across a restart re-derive from the transcript and the journal. | The first document that outlives its process, which is the same dependency admission `P2-L3`'s `T-5` has not made either. |
| D-6 | The unmapped condition in `completeness_witness` is implied by the coverage condition and is therefore not independently observable. | A change to the coverage denominator rule — for instance excluding redacted segments from it — which is exactly when the two sentences would stop coinciding. |

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`. No recording is made, no device is opened, no document
is rendered, no byte leaves the machine, and every fixture in this crate's test
tree is synthetic and built from committed literals.
