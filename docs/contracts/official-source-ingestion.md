# Official source ingestion and change propagation

`academic-ingestion` is `P2-U6`. It holds section 29.1's ordered ingestion
contract, section 29.2's rules for school data, and section 8.4's rule for
competing official sources. It persists nothing, opens nothing, and runs no
live connector.

## The nine stages are nine types

```text
discover/fetch/import
  → policy and terms check
  → immutable raw snapshot + hash
  → source metadata and retrieval time
  → deterministic parse
  → schema validation
  → AI proposal where appropriate
  → reconciliation/entity resolution
  → claim publication or review queue
```

Each stage is a function whose argument is the previous stage's output type.
Those types have private fields and one producer each, so a caller cannot reach
stage six without a `Parsed`, and the only thing that makes a `Parsed` is
`deterministic_parse`. `tests/compile_fail/a_stage_cannot_be_skipped.rs`
observes that handing stage seven a `Parsed` does not compile.

Every stage returns `Result`. `run` records the stages it reached, and
`ingestion_stage_order_is_strict` walks `Stage::ALL`, arranges for each stage in
turn to fail, and requires that nothing was published and that no later stage
ran. It enumerates the stages; it asserts no count of them.
`the_stage_list_is_section_29_1s_own` reads the fenced block out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and requires
`Stage::ALL`'s lines to be that block, in that order, so the list here cannot
drift from the specification without failing.

### Where the terms are consulted, and why three times

Section 29.1 puts the policy and terms check *after* acquisition, which is what
lets a user-supplied import — the thing the fallbacks produce — arrive without a
terms review of a fetch that never happened. That ordering is followed exactly,
and the ledger is read three times for three different jobs:

- **stage one** refuses to *fetch* over a connector the ledger does not permit,
  so an unreviewed source is not contacted;
- **stage two** is section 29.1's recorded policy decision about the bytes that
  arrived, and it is the check an import passes through;
- **stage nine** reads the ledger once more immediately before publication,
  which is what makes `IN06` — a permission withdrawn *during* a run — stop this
  run rather than the next one.

## The connector manifest

Section 29.1's sentence is the requirement: *every connector declares source
ownership, authentication method, allowed frequency, robots/terms status,
personal-data class, completeness, last success, next verification and parser
version*. `ManifestField::ALL` is that list.
`the_manifest_fields_are_section_29_1s_own` pins the sentence whole and walks it
forwards, so the field list cannot drift from it.

`ConnectorManifest` has private fields and no `Default`. `ManifestDraft` is the
only route, and `build` names the first empty field.
`connector_manifest_requires_every_field` iterates `ManifestField::ALL`,
rebuilds a complete draft with exactly one field dropped, and requires the exact
`ManifestError::Missing` for it. The evidence is per-field, not a count.

A manifest that declares no document to retrieve is refused too: a connector
with no target is a crawler waiting for one.

### A fetch target is `&'static`

`DeclaredTarget::declared` takes `&'static str` and is the only constructor.
Bytes that arrive at run time are owned, and `Untrusted<IngestedDocument>` hands
out neither a `String` nor a `&str` outside `academic-untrusted-content`, so a
link found inside a fetched page is a value no target can be built from.
`tests/compile_fail/a_fetch_target_cannot_be_built_at_run_time.rs` observes it.

### A credential is bound to one connector's declarations

`CredentialBinding` has no public constructor.
`ConnectorManifest::credential_binding` is its only producer and returns `None`
unless the declared authentication method is one that holds a credential —
`ScopedOfficialApiToken` and nothing else. `UserSuppliedExport` holds none on
purpose: the user authenticated, not this system, and what arrives is a file.

`ConditionalRequest::credentialed` consumes the binding, so it cannot be spent
twice, and refuses a target the manifest does not declare or a binding from
another connector.

## The conditional fetch, and the hash diff beside it

`ConditionalRequest` carries the validators a previous snapshot recorded. A
`304` produces `FetchOutcome::NotModified`, and stage three refuses to create a
snapshot version from it.

The hash half is separate and answers a different question: two retrievals whose
bytes hash to the same digest are the same document even when the entity tag
changed, and two whose bytes differ are different documents even when it did
not. `RawSnapshot::has_same_content_as` is that comparison and
`conditional_fetch_and_hash_diff` exercises both directions.

### The cadence is compared against a clock

Section 29.2 asks for a *low-frequency* fetch, and a cadence nothing compares
against a clock is a declaration rather than a limit. Stage one takes the
retrieval instant and refuses a **fetch** that is earlier than
`AllowedFrequency::earliest_next(last success)` with `DenialReason::TooSoon`.
Three cases permit: a connector that has never succeeded has nothing to count
from; `OnUserRequestOnly` has no schedule to be early for, because a run *is* the
user asking; and an **import** is never refused, because a cadence is a rule
about how often this system asks a source and not about how often a person may
hand over a file they already have.

A `TooSoon` denial carries the same four fallbacks as every other and does **not**
disable the connector: the terms still permit the source, the clock does not
permit the fetch yet. `the_declared_cadence_limits_a_fetch_and_not_an_import`
runs all five cases.

A header value read out of a response and echoed back on the next request is the
one value that crosses from a response into a request. `HeaderValue` restricts it
to printable ASCII, at most 128 bytes, which removes the separator a header
injection needs.

## The snapshot, and the one route to its bytes

`RawSnapshot` retains the five things section 29.1 asks for: the retrieval
instant, the HTTP metadata, the raw bytes, the content hash, and the parser
version. Every field is private and there is no setter; a later retrieval is a
second snapshot.

`source_bytes` is private. The one **public** route to it is `RawSnapshot::seal`,
which returns `academic_untrusted_content::Untrusted<IngestedDocument>` —
`P2-G5`'s label, reused rather than reinvented. `P2-G5` then decides what a
caller may do with the result: the wrapper implements no unwrapping trait, its
accessor is `pub(crate)` to that crate, and a rendered prompt may carry it only
in a quoted data record.

Inside this crate the bytes are reachable through one `pub(crate)` accessor, and
`the_only_public_route_to_snapshot_bytes_is_the_untrusted_seal` pins
`RawSnapshot`'s whole public surface and requires that accessor to be called from
exactly one file: `document.rs`, the deterministic parser. A trusted parser
reading untrusted bytes is what a deterministic parse *is*; what does not exist
is a second reader or a public signature that hands the same bytes to anyone
else.

The seal takes the caller's `SourceKind`. This crate names none of its own, and
adds no variant to `P2-G5`'s enum — see [Open](#open) for what that costs.

### `IN01`

The transport reports the digest it computed while reading; `store` recomputes
over the bytes it was handed and refuses the snapshot when the two disagree.
Nothing is written, and the retry is an ordinary second fetch that produces a
new immutable version.

## Effective dating, and what an undated document cannot do

Section 29.2: *a document whose effective date cannot be found is
`UNSCOPED_OFFICIAL_SOURCE` and is not automatically published as a rule.*

`Dating` has two arms and no accessor that turns the second into the first.
`Reconciled::publishable` returns `None` for `Dating::Unscoped` and is the only
producer of `PublishableRules`, which is the only argument `publish` takes.
`PublishableRules` has private fields and no public constructor.

So an undated document is not published — not because a check refuses it, but
because there is no value of the argument type to call `publish` with. Two
compile-fail cases observe both halves: the struct literal, and passing the
reconciled state where the publishable value belongs, and
`the_publisher_has_one_argument_type_and_one_producer` counts the one place the
value is built — a second public entry point that assembles it in its own body
names the type nowhere in its signature, which is the empty guard this task
found in its own suite and repaired. `IN02`'s behavioural half
is `unscoped_official_source_cannot_publish`, which also runs the same bytes plus
one `EFFECTIVE:` line and requires *that* to publish, so the refusal is about the
dating rather than about the fixture.

## Change propagation

A parse produces metadata: the issuing authority, the two dates, the target
scope, the transitional measures, and — per rule — an identifier and a digest of
the rule's text. **No rule text leaves `document.rs`.** That is what makes the
diff, the invalidation and the conflict case carry no untrusted bytes: they carry
identifiers and digests.

Identifiers read out of a document are restricted to `[A-Za-z0-9._-]`, for the
reason `academic_untrusted_content::SourceId` gives: a name lifted out of an
untrusted document must not be able to carry a directive or a separator.

`SourceDiff::impacted_rules` names a rule when that rule changed — added,
removed, moved between sections, or edited — and every rule in the document when
the document's own header changed, because the effective date, the target scope,
the transitional measures and the issuing authority each decide when and to whom
every rule in it applies. It names no other rule.
`rule_change_impact_identifies_exact_rules` compares the whole set, so an
unchanged rule fails as an extra entry and a changed one fails as a missing one.

`DependencyGraph::invalidate` walks the graph in reverse from the impacted rules,
transitively, and stops. `source_change_invalidates_exact_dependents` compares
the whole set both ways, so over-invalidation fails as an extra entry and
under-invalidation as a missing one; it also drives a cycle, and all three of
section 29.2's kinds — requirement, scenario, course mapping.

## Competing sources: five dimensions and no winner

Section 8.4, in the paragraph after the numbered list of collection targets:
*when sources conflict, a mechanical winner is not chosen by the higher or lower
number. The legal hierarchy of the regulation, the issuance date, the effective
date, the target scope and the transitional measures are compared, and a
`ConflictCase` is made. A dangerous determination such as graduation is left
`INDETERMINATE` until it is resolved.*

`ConflictDimension` is those five. `ConflictCase::open` records one finding per
dimension and does nothing else with them. **There is no function anywhere in
this crate from a set of findings to a source.** A case is
`Resolution::Unresolved` until a person records a decision, and
`ConflictCase::disposition` is `AuditDisposition::Indeterminate` for as long as
it is. `IN05`'s behavioural half is `in05_two_official_sources_conflict`, which
also runs two sources that agree and two that apply to disjoint cohorts and
requires both to publish.

`the_conflict_dimensions_are_section_8_4s_own` pins the specification's sentence
whole, maps each Korean dimension name to a variant, and walks the sentence
forwards, so the five cannot drift from it.

The legal hierarchy is read out of `SUPERIOR_PAIRS`, an explicit table of pairs,
by membership. There is no rank, no index and no arithmetic: an authority is
superior to another because the pair is written down and reviewed.
`the_legal_hierarchy_is_a_reviewed_relation` checks the relation is irreflexive,
antisymmetric and transitive over every pair, and that the one incomparable pair
— an external accreditation standard beside a university rule — really is
incomparable, so `NotComparable` is not an unreachable arm.

Dates are compared, because two of the five dimensions *are* dates. What does
not exist is the step after that: a number saying how many dimensions favoured a
side, a rank read out of a list, or a source picked because it came first.
`a_conflict_case_has_no_privileged_side` swaps the two arguments and requires
every finding to mirror.

## What is refused, and what is offered instead

Section 29.5 names four things this system does when a source may not be
collected the way a connector wanted: manual paste, a user export, saving it from
the browser yourself, low-frequency manual sync. `Fallback::ALL` is that list.

`deny` is the only constructor of a `Denial`. Every denial carries the whole of
`Fallback::ALL` and routes to `DenialRoute::ManualOrStop`; neither is a
parameter. `manual_and_export_fallbacks_are_offered_when_denied` checks every
`DenialReason`, and then checks the denials four real stage failures produce, so
the claim is about the pipeline rather than about one function.

`IN06` — a permission withdrawn during a run — additionally disables the
connector. A cadence denial does not: the terms still permit the source, the
clock does not permit the fetch yet.

## Section 38

Two cells stay open, and `OpenGate` states each where it bites.

**`GATE-38-020`** — which access methods and which frequency each source
permits, its robots directives and its rate limits — is a user and legal
decision recorded per source. There is no default. A connector with no recorded
review reads as `TermsStatus::Unreviewed`, which permits no fetch, and
`AllowedFrequency::OnUserRequestOnly` returns `None` for "the next scheduled
time" rather than inventing one. The fixture-driven tests in this crate are not
blocked by the gate, and no live connector runs behind it.

**`GATE-38-027`** — where a user-performed export ends and a browser-assisted
capture begins — is undecided. **Phase 2 ships manual import and user-provided
export and contains no browser automation module.** The four fallbacks are four
things a person does; not one of them is a module that drives anything.

## What this crate does not have

**No transport.** `ConditionalFetch` is a trait the caller implements, exactly as
`academic-egress-boundary` takes its `OutboundTransport`.
`credentials_never_reach_a_general_crawler` requires this crate to implement it
nowhere, and `only_egress_crate_has_a_socket` is the workspace-wide statement
that no crate spells a socket construct outside the eight files that run the
local IPC seam. The link half is `SOCKET_CAPABLE_CLOSURES`, where this crate's
row is `["libc"]`, reaching it through `academic-domain` — that row records
*availability*, which is why the source half is what says nothing here uses it.

**No decoder, no driver, no HTTP client.** The whole set of imports rooted
outside this crate is pinned by `no_captcha_or_access_control_bypass_module_exists`,
and the manifest's product and dev edges are pinned by
`this_crate_declares_three_product_edges`. An image decoder, an audio decoder, a
browser driver or an HTTP client cannot be reached without a line that is not on
either list.

**No store edge and no migration.** Migrations `0006`, `0007` and `0009` are in
use, `0008` is unclaimed and stays that way, and this task claims no number: it
persists nothing. The typed rows an ingestion writes belong to whichever
aggregate owner writes them; this crate produces the values.

**No trust label of its own.** `P2-G5` owns `Untrusted<T>` and this crate reuses
it.

## What this contract does not claim

- **It does not claim that no bypass of an access control can be written.** What
  is executed is narrower and is the composite of three facts: no crate outside
  the two egress crates may open a socket, so a module elsewhere cannot transmit;
  no function in this crate produces a request, a target or a credential from a
  response, and the whole set of signatures that touch any of the three is
  pinned; and no file outside `crates/ingestion/` names this crate's request,
  target or credential types, which is pinned as a whole map. A module that
  spells none of those and opens no socket is not refused by anything here,
  because it also cannot reach a source.
- **It does not claim that the five dimensions decide anything.** They are
  recorded. Deciding between two official sources is a person's act, and this
  crate records that decision without checking who made it — `P2-M4` is where a
  non-delegable action refuses a model actor, and nothing here duplicates that
  check or claims to have made it.
- **The textual diff is at rule granularity.** A changed rule is reported as two
  digests, not as a character diff. That is what keeps document bytes out of
  every value a diff consumer receives, and it is the reason a caller cannot see
  *what* changed inside a rule from the diff alone.
- **The document format is synthetic.** The parser reads a committed line format
  composed in this repository. It is not a reader for any real publication, and
  nothing here is evidence about parsing one.
- **`product_network` remains `NONE` and `production_data_allowed` remains
  `false`.** Nothing in this task moves either. ADR-002 is unaccepted and the
  default lane is `storage_encryption=NONE`.

## Open

**`SourceKind` has no variant for an official regulation document.** `P2-G5`'s
six kinds are that task's own list — syllabus, README, issue, code comment,
review text, provider response — and an SNU graduation regulation is none of
them. `RawSnapshot::seal` therefore takes the kind as the caller's argument
rather than choosing one, and this crate's own tests seal with
`SourceKind::Syllabus`, which is honest for a syllabus connector and would not be
for a regulation. Adding a seventh variant means a new entry in
`testdata/injection-corpus/` and edits to `academic-untrusted-content`'s
completeness rules, which is that crate's contract rather than this one's. It is
recorded here rather than done here.

**`AuthorityTable::rank` returns `0` for an authority class its table does not
list.** `AuthorityClass` has eight variants and every table in
`crates/ledger/src/product_authority.rs` lists all eight, so the arm is
unreachable today; a ninth variant would be silently indistinguishable from
`Unknown`, whose rank is already `0`. Nothing asserts that a table covers every
class. This was looked for because it is the same shape as the rule this task
executes — a default standing in for "I do not know" without saying so — and it
is left as an observation against `P2-M3`'s contract rather than changed here.
