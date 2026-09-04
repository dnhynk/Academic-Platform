# Course review ingestion and bias disclosure

`academic-review` is `P2-U8`. It holds section 29.5: what a review is attached
to, what an aggregate has to admit to, what happens to the words somebody else
wrote, and what this system will not do to obtain them. It persists nothing,
opens nothing, and runs no live connector.

## A review is attached to four things, and a course is not one of them

Section 29.5's first sentence: *Review는 기본적으로 `CourseOffering + Instructor
+ Term + Source`에 연결하고 Course 전체로 승격할 때 명시적 aggregation을
사용한다.* `ScopeDimension::ALL` is that list.
`review_default_scope_is_offering_instructor_term_source` reads the sentence out
of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits its backticked
span on `+`, and compares the result with `ScopeDimension::ALL` in both
directions — so a fifth name in the specification fails as an extra entry and a
dropped one fails as a missing one. It enumerates them; it asserts no count.

Three of the four are `Option`, because the record writes them as `... | null`.
The source is not, and has no null spelling: a review with no source has no
provenance, and section 29.5 keeps the raw artifact *for* provenance.

`ReviewScope` has no `CourseId` field, no constructor that takes one, and no
accessor that returns one. `tests/compile_fail/a_review_scope_has_no_course.rs`
observes the third. Section 34's own failure row — *Course와 Offering 혼동:
catalog row에 교수·학기 속성을 덮어씀* — is what that absence is about.

Two reviews that differ in **any one** of the four do not aggregate together.
`OfferingAggregate::over` names the first dimension they disagree on, and the
acceptance test changes one dimension at a time and requires the exact
`ScopeMixed` error for each: the evidence is per-dimension rather than one mixed
fixture that could pass for the wrong reason.

## The record is section 29.5's, field for field

`the_record_fields_are_section_29_5s_own` reads the `ReviewRecord:` block out of
the specification and compares its ten keys against the ten accessors this crate
answers them with, in both directions. The nested `dimensions:` keys are
compared against `ReviewDimension::ALL` the same way.

**`extractionStatus` is not a field.** The specification writes it as a literal
rather than a union, so there is nothing to store and nothing a caller could
pass. `ReviewRecord::collected` takes an
`academic_proposal::Autosaved<ReviewExtraction>`, whose `EPISTEMIC_STATUS` is
`AI_INFERRED`, and `ReviewRecord::EXTRACTION_STATUS` **is** that constant.
`P2-M2` owns the rule and this crate reuses it: section 27.4's low-risk row is
*save it and mark it `AI_INFERRED`*, and `ReviewQueue::autosave` is the one door
that produces an `Autosaved` — it serves `LOW_AUTOSAVE` alone and takes no user
decision. So there is no argument anywhere here that would make a review record
claim a stronger status.

**A provenance span is checked where it is made.** `RawReviewText::retain` takes
`(start, end, digest)` triples and refuses a range outside the text, a range off
a character boundary, and a digest that is not the digest of the covered bytes.
A span that survives is one a later reader can resolve *without the text being
handed to them*, which is why spans carry offsets and a digest and never bytes.

## The raw text: retained, and not redistributed

`raw_review_text_is_excluded_from_export_and_share` executes four things, and the
fourth is about `crates/export` directly rather than about this crate alone.

1. **The whole `impl` inventory of this crate** is compared against a pinned
   list. `RawReviewText` implements `fmt::Debug` and nothing else; there is no
   `Display`, no `ToString`, no `Serialize`, no `AsRef<str>`, no
   `From<RawReviewText> for String`. A new one fails as an extra key whatever it
   is named, and the orphan rule closes the same shape from outside — both the
   trait and the type would be foreign in another crate.
   `tests/compile_fail/retained_text_is_not_a_string.rs` observes the `Display`
   half.
2. **The one internal reader is enumerated.** `RawReviewText::content` is
   `pub(crate)`, and the set of files whose code calls it is compared against a
   one-entry list: `duplicate.rs`. A deterministic near-duplicate check reading
   the text is what such a check *is*; what does not exist is a second reader.
   `tests/compile_fail/retained_text_has_no_public_reader.rs` observes that a
   caller outside the crate cannot spell it.
3. **The whole public text-returning surface** — every public function whose
   return type is `String`, `&str`, `&String`, `Vec<u8>` or `&[u8]` — is three
   entries, and every one is the FNV-128 digest this crate computed:
   `RawReviewText::digest`, `ProvenanceSpan::digest` (two accessors with one
   signature, in one file, so the set records the signature twice), and
   `RawReviewText::digest_of`, which takes the caller's own bytes.
4. **The bundle.** Every `.rs` file under `crates/export` is read and required to
   name none of this crate's public types and not the crate path
   `academic_review`; `crates/export/Cargo.toml` is required to declare no edge
   to this crate and this crate's manifest none to it. A bundle row is filled
   from a `String` the caller already holds, so the remaining question is whether
   a `String` of a review can exist at all — and (1) says no conversion trait
   makes one, (3) says every public function that returns text returns a digest
   this crate computed, and the last assertion says no product file here reaches
   `serde` under any spelling.

The one **public** route out of the artifact is `RawReviewText::seal`, which
returns `academic_untrusted_content::Untrusted<IngestedDocument>` — `P2-G5`'s
label, reused rather than reinvented, sealed as the `SourceKind::ReviewText`
variant that crate already carries. `P2-G5` then decides what a caller may do
with it: the wrapper implements no unwrapping trait, its accessor is
`pub(crate)` to that crate, and a rendered prompt may carry it only in a quoted
data record. So the extraction that produces the `AI_INFERRED` dimensions can
happen, and no `String` of somebody else's writing exists on the way.

### The field inventory, and why this crate does not lean on the shared tool

`S-18` on [the policy source-scan inventory](policy-source-scans.md) closed
`tools/secret-debug-policy.test.mjs`'s **text** half for a *set of crates* and
not for the workspace, and `crates/review` is not in that set:
`TEXT_CLASSIFIED_CRATES` names `transcription` and `lecture-document`, and every
other crate stays on the `SECRET_FIELD_NAMES` alternation that row documents as
the weakest layer. So a `String` field of this crate under a name outside that
alternation is still judged by nothing there, and a registration that is merely
present is still not evidence. Measured rather than assumed: a `handle: String`
added to `PermittedCollection` and filled at its one construction site leaves
that tool at **14 of 14 passing**.

So `every_field_of_every_type_is_classified` closes it here.
Every field of every `struct` and `enum` this crate declares — enum variants
with named fields included, and tuple positions reported as `.0` so a newtype
cannot hide a payload by having no name — is discovered and compared against a
committed table of `(type, field, field type, class)` in both directions. A
field added, removed, or whose *type* changed all fail. The class comes from a
six-word vocabulary — `review-content`, `digest`, `identifier`, `count`, `enum`,
`composite` — and two rules run over it: a field whose type is bare text may only
be `review-content` or `digest`, and the set of types holding a `review-content`
field has to be exactly the set that hand-writes a redacting `Debug`.

The two types are also registered in the shared tool — `RawReviewText` in
`SECRET_BEARING_TYPES`, and `RawReviewText.digest` and `ProvenanceSpan.digest`
in `PUBLIC_BYTES` beside `P2-G5`'s four SHA-256 fields, for the same reason: a
digest of untrusted content is not the content, and it is what makes a citation
checkable. That registration is a second net, not the evidence.

## Login bypass, account sharing and anti-bot evasion

`no_login_bypass_or_evasion_module_exists` compares **five whole sets, each in
both directions**. There is no list of forbidden spellings in the file, on
purpose: a name list refuses the edits somebody thought of and admits every edit
spelled differently, which this run measured five times, and `P2-RF13` found six
real leaks the moment one became a whole-set classification.

1. **Every `use` statement in this crate**, whole, read to its `;` rather than to
   the end of its line. Every one, including the `pub use` re-exports — a
   re-export is how a crate widens what a caller can reach. An HTTP client, a
   TLS stack, a browser driver, a headless runtime, an image decoder or a cookie
   store cannot be reached without a line here.
2. **Every function declaration in this crate's product source**, as a
   visibility and a signature. This is the exhaustive net, and it is not a list
   of names to refuse — it is the list of functions that exist. A function added
   anywhere in this crate, spelling nothing anybody thought to forbid, in a
   module nobody predicted, fails as an extra entry. Read as a whole it answers
   "what can this crate do": every entry takes and returns values this crate
   already holds. There is no signature from a response to a request, none that
   takes a credential, a session, a header or a cookie, and none returning
   anything an outbound request could be built from.
3. **Every file in the workspace whose code names a value an outbound request is
   composed from** — `OutboundTransport`, `ConditionalFetch`,
   `ConditionalRequest`, `CredentialBinding`, `DeclaredTarget`, `StagingRequest`.
   `academic-egress-boundary`'s own module documentation says `OutboundTransport`
   is the only trait in this workspace whose method hands bytes to something
   outside, and `academic-ingestion`'s four are the only values a fetch is
   composed from. This crate is in none of them, and a new file anywhere in the
   workspace that composes a request fails as an extra key.
4. **Every file outside this crate that names one of its types or its crate
   path.** Empty: no other package depends on `academic-review` by any edge kind.
5. **The manifest edges**, both sections.

Section 29.5's three access modes are read out of the specification's own
`sourceAccessMode` union and compared both ways.
`SourceAccessMode::presents_a_credential` is `false` for all three, as a total
`match` rather than a constant, so a fourth arm has to answer the question rather
than inherit an answer.

## What a refused source is offered instead

Section 29.5 names four things this system does when a source may not be
collected the way it wanted: manual paste, a user export, saving it from the
browser yourself, low-frequency manual sync. `P2-U6` owns that list as
`Fallback::ALL`, owns the single `DenialRoute::ManualOrStop`, and owns the one
`Denial` constructor whose text is pinned whole in that crate.
`crate::access::permit` produces `P2-U6`'s `Denial` rather than a second value
shaped like one, so `denied_source_exposes_only_the_four_fallbacks` is a claim
about the shipped constructor and not about a copy. It drives every unpermitting
status in every access mode, then drives the case `GATE-38-021` is about: an
empty ledger.

`permit` is one total `match` over `TermsStatus::ALL` with no wildcard, and the
permitting arm is the only construction site of a `PermittedCollection` in the
crate. There is no arm that both permits and names a reason.

## Course-level promotion

Section 29.5 requires the promotion to use an *explicit aggregation*; the
execution plan requires the method to be *named*. Both are types.

`AggregationClaim` has private fields, no `Default`, no `Clone`, no `Copy`, and
one constructor whose first argument is an `AggregationMethod` — an enum with no
`Unknown` arm and no value a caller gets by not deciding.
`CourseAggregate::promote` is the only producer of a course-level value, it takes
the claim **by value**, and there is no `From`, no `TryFrom`, and no constructor
taking offering aggregates alone. Two `compile_fail` cases observe both halves:
assembling the claim from outside, and handing `promote` a bare list.

**The method is not decoration.** `CourseReading` has one arm per method and the
two arms hold structurally different values — a pooled distribution per
dimension, or the offerings kept apart — and
`the_named_method_decides_what_the_course_value_is` runs both over one input,
requires the results to differ, and requires each reading's own idea of which
method made it to agree with the claim's. Both arms are then read, so neither is
an unvisited branch.

**Each refusal is driven separately, and there is one place to drive.** `T186`
measured a guard written in two places and driven in one: the undriven site could
be relaxed and every test still passed. Two things close that here.
`promote` has three refusals — an empty set, a repeated input, and a claim spent
on a set it was not asserted over — and each is driven by its own assertion with
its own error, as are `OfferingAggregate::over`'s two. And
`each_value_has_one_producer` reads each type's `impl` block and compares the
whole set of functions returning `Self` or a `Result` over it against a
one-entry list, for `CourseAggregate`, `OfferingAggregate` and `ReviewRecord`
alike — so a second producer fails as an extra key *before* anybody has to
notice that its own refusals are undriven.

## No scalar is a course property

Section 29.5 ends: *"난이도 4.2"를 객관적 과목 속성으로 쓰지 않는다.* Four things
carry it.

* `DimensionBand` is five ordered bands with no numeric conversion and no
  arithmetic. There is no function anywhere in this crate from a set of bands to
  a band, because a representative band is the scalar under another name.
* A reading is a `BandDistribution` — the count of reviews in each band — and it
  has no accessor that reduces it. `scalar_is_not_a_course_property` builds two
  samples whose mean would be identical and whose shapes are not, and requires
  them to stay distinguishable.
* A distribution cannot be obtained without an aggregate, and an aggregate cannot
  be built without a `BiasDisclosure`. So there is no value here that is a course
  reading without its six warnings attached.
* `academic-curriculum`'s `Course` is three fields — an identifier, a code and a
  canonical identity. The same test reads that struct's whole field list out of
  its source, compares it against those three in both directions, and requires
  every review-dimension spelling and every course-aggregate type name to be
  absent from that module. `academic-curriculum` has no edge of any kind to this
  crate, so a `Course` that named a review reading is a graph fact rather than a
  rule inside a function.

## The six disclosures

Section 29.5: *강의평 aggregate는 표본 수, 최근성, 교수/학기 mix, 응답자
self-selection, 극단 경험 편향, 중복 가능성을 표시한다.* `BiasDimension::ALL` is
that list. `aggregate_discloses_all_six_bias_dimensions` reads the sentence out
of the specification, splits it between its subject and its verb, and compares
the comma-separated items against the variants' `spec_phrase` in both
directions.

`BiasDisclosureDraft::build` is the only producer of a `BiasDisclosure` and it
names the first dimension nothing disclosed. The test drops each of the six in
turn and requires the exact `BiasDimensionMissing` for it, then discloses each
twice and requires the exact `BiasDimensionRepeated`. Both aggregate
constructors take a built disclosure **by value**, so there is no partial value
to hand them: an aggregate that exists names all six.

Section 34's *강의평 편향* row is the same list from the failure side, and its
detection column names *duplicate similarity* — which is the measurement behind
`BiasDimension::Duplication`.

## Duplicate similarity

Two reviews are compared by the overlap of their word trigrams: lowercase, every
non-alphanumeric run a separator, the trigram *set* of the resulting words, and
`1000 * |A ∩ B| / |A ∪ B|` in permille with integer division. A text of fewer
than three words yields one shingle holding all of them, so two short reviews
compare against each other rather than against nothing. There is no floating
point in it.

`duplicate_similarity_is_detected` writes its expected values as literals
computed by hand from that definition, with the intersection and union sizes each
came from written beside it. Nothing in the test asks the implementation what the
answer is: `P2-U3` wrote a separate JavaScript oracle for the same reason, and an
engine checked against itself always passes. The test also swaps every pair to
require symmetry, runs two thresholds, and requires the duplicated-record count
to be of records involved rather than of pairs.

## Section 38

**`GATE-38-021`** — per-source access, storage, analysis and retention rights —
stays open. It is a user and legal decision and there is no default:
`SourceTermsLedger` starts empty, a `(source, access mode)` pair nobody recorded
reads as `TermsStatus::Unreviewed`, and `permit` refuses it with the whole of
`Fallback::ALL`. An unconfigured source keeps its connector disabled by having no
record rather than by holding a switch somebody could flip. The fixture-driven
tests here are not blocked by the gate — they record their own synthetic
decisions — and no live connector runs behind it, because this crate has no
transport to run one with.

The ledger is keyed on the source **and** the access mode rather than on the
source alone. A ledger keyed on the source would refuse the manual paste that is
the remedy for a source whose terms refuse an automated collection, which is the
whole reason the four fallbacks exist.

## What this crate does not have

**No transport, no decoder, no driver.** The whole set of `use` statements is
pinned and the whole set of function declarations is pinned. An HTTP client, a
browser driver, an image decoder or an audio decoder cannot be reached without a
line on the first list, and a module that reads a challenge and writes an answer
cannot exist without an entry on the second.

**No store edge and no migration.** It persists nothing. Migration `0033` is
unclaimed and stays that way. The typed rows a review ingestion writes belong to
whichever aggregate owner writes them; this crate produces the values, the way
`P2-U6` does.

**No trust label of its own.** `P2-G5` owns `Untrusted<T>` and this crate reuses
it, sealing as the `SourceKind::ReviewText` variant that crate already carries
for exactly this.

**No fallback list, no denial route and no terms vocabulary of its own.**
`P2-U6` owns all three.

**No serializer.** No product file here reaches `serde` under any spelling, which
is asserted rather than described.

## What this contract does not claim

- **It does not claim that no bypass of an access control can be written
  anywhere.** What is executed is narrower and is the composite of four facts.
  First, `only_egress_crate_has_a_socket` compares the whole per-file allowance
  map of socket spellings across every workspace package; `academic-review` has
  no entry in it, which is what an empty allowance looks like there, and a module
  written anywhere else has nothing to transmit with unless it is added to that
  map in the same commit. Second, the whole set of workspace files whose code
  names one of the six values an outbound request is composed from is compared
  in both directions, and this crate is in none of them. Third, the whole set of
  function declarations in this crate is compared, so a function of any name
  fails. Fourth, the whole set of files outside this crate naming its types or
  its crate path is compared, and it is empty. A module that spells none of those
  and opens no socket is refused by nothing here — it also reaches no source.
- **It does not claim that a digest of a review reveals nothing.** The digest is
  FNV-128, which is a checksum and not a preimage-resistant hash: it answers "are
  these the bytes I recorded a span over" inside one process, against no
  adversary who chooses the bytes afterwards. A span digest over a very short
  range is guessable by search, and nothing here prevents that. Where a digest
  has to survive an adversary it is the SHA-256 `Untrusted` computes over the
  same bytes at `seal`. This is recorded rather than fixed, because the fix is a
  hash-crate edge this task did not need.
- **The similarity metric is this crate's own.** Section 34 asks for *duplicate
  similarity* and names no method. Word-trigram Jaccard is a choice, its
  threshold is the caller's argument and not a constant here, and nothing in this
  crate turns a similarity into a decision — `duplicate_findings` reports pairs
  and `duplicated_record_count` reports how much of a sample may be one text
  twice. Which threshold is right for a source is `GATE-38-021`-adjacent and is
  not settled here.
- **The two aggregation methods are this crate's own.** Section 29.5 requires the
  aggregation to be explicit and the execution plan requires the method to be
  named; neither names methods. `PooledBandCounts` and `PerOfferingListing` are
  two, and the contract is that a promotion carries one — not that these are the
  only two that could exist. A third is a new arm of both enums and a new
  paragraph here.
- **The fixtures are synthetic.** Every review in the test tree is an English
  sentence written in this repository. Nothing here is evidence about parsing,
  extracting from, or classifying a real review site's text.
- **`product_network` remains `NONE` and `production_data_allowed` remains
  `false`.** Nothing in this task moves either.

## Open

**`SampleBias` records which of the six dimensions one review contributes to,
and nothing computes it.** Section 29.5 writes `sampleBias: ...` on the record
with no reading of what fills it. This crate makes it a set of `BiasDimension`
so the per-record field and the aggregate's disclosure share one vocabulary, and
`ReviewRecord::collected` takes it as an argument. What decides that a particular
review is an extreme experience, or is self-selected, is a judgement no function
here makes — and an aggregate's `BiasFinding` values are likewise the caller's
measurements. The crate's claim is that all six are *disclosed*, not that it
knows how to measure them. Section 34's *Thresholds* cell is recorded open in
`t001`'s `REQ-34-070` row for the same reason, and this is that cell seen from
here.

**A `BiasFinding` carries one `u32` for every dimension.** Sample count and
duplicate count are counts; recency and instructor/term mix are counts of terms
and instructors; self-selection and extreme-experience have no natural count and
their `measured` is whatever the caller measured. A per-dimension measurement
type would say more, and it would also fix a measurement method this crate has
no basis to fix. It is left as one number with a per-dimension `BiasStrength`
beside it.
