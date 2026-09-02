# Untrusted-content boundary contract

`academic-untrusted-content` is the `P2-G5` boundary between bytes that came
from outside this system and anything this system will act on. It sits above
`P2-G4`'s worker sandbox and beside `P2-G2`'s egress boundary.

It answers three questions and nothing else: what carries the trust label, which
channel of a prompt ingested bytes may occupy, and what has to be true before
something a model wrote becomes a proposal.

## The label is a type

`Untrusted<T>` wraps a value at the moment it is parsed. It implements no
`Deref`, `DerefMut`, `AsRef`, `AsMut`, `Borrow`, `Display`, `ToString`, `From`,
or `Into`, and it has one accessor, `pub(crate) fn expose`, so outside this crate
there is no function that returns the wrapped value.

A label kept as a field or a convention survives one refactor. This one is
propagated by the compiler: a caller cannot spend an `Untrusted<IngestedDocument>`
as a `&str` because no such conversion exists to call.

Three things hold that, and they hold different halves:

- **Three `compile_fail` doctests** in `crates/untrusted-content/src/label.rs`
  observe that `Deref`, `Into<String>`, and `Display` are absent today.
- **`untrusted_has_no_unwrapping_trait_impl`** compares the crate's *whole set*
  of `impl` blocks whose header names `Untrusted<` against a two-entry pinned
  list, so an implementation of a trait nobody predicted fails as an extra key.
  `WHOLE_UNTRUSTED` pins the inherent block beside it, because an inherent
  `pub fn into_inner` would name no trait at all.
- **The orphan rule** refuses the same implementation written in another crate:
  both the trait and the type would be foreign there. That is the one half
  nothing in this repository needs to check.

`Untrusted<T>`'s `Debug` is hand-written, prints provenance, digest and byte
count, and is implemented for every `T` with no `T: Debug` bound — so there is no
instantiation whose payload a format string reaches.
`the_untrusted_wrapper_prints_no_payload` observes that over corpus entries.

### The three places the label is taken off, and why each is allowed

`every_exposure_site_is_named_and_justified` compares the whole inventory of
`.expose()` call sites against this list. A fourth fails as an extra key; a
removed one fails as a missing key.

| Site | Why |
|---|---|
| `PromptEnvelope::quote` | The quoted data channel is the one place ingested bytes may appear, and quoting has to read the bytes it escapes. What leaves is escaped, one line, pure ASCII, and recorded as an untrusted span. |
| `resolve_span` | Provenance resolution has to compare a cited range against the source bytes. What leaves is a `ResolvedSpan`: offsets and a digest, no text. |
| `adjudicate` | Schema validation has to read the output it validates. What leaves is a closed `ProposalKind`, resolved spans, and a summary sealed again. |

## Parse-time tagging

Every constructor returns `Untrusted`. `IngestedDocument` has private fields, no
`Default`, and a `compile_fail` case closes assembling one from outside the
crate. The six source kinds are the execution plan's own list: syllabus, README,
issue, code comment, review text, provider response.

`ingest_provider_response` takes `&AcceptedResponse`. That type has one producer,
`academic-egress-boundary`'s `EgressProxy::accept_response`, which is `P2-G2`'s
canary and rulepack scan. A provider response this crate is handed has therefore
been scanned, and this crate scans nothing of its own: the reuse is the argument
type rather than a comment. A response that failed that scan is an `Incident`,
which `quarantine_incident` turns into the same quarantine state a schema failure
produces.

## The three channels

A rendered prompt has four kinds of region — `Structure`, `System`,
`ToolInstruction`, `Data` — and one rule: ingested bytes occupy `Data` regions
and nothing else.

`SystemDirective::new` and `ToolDirective::new` take `&'static str`. Ingested
content arrives at run time as owned bytes, and the wrapper hands out no `&str`
and no `String` outside this crate, so there is no value a caller could turn into
the `&'static str` those constructors want. The remaining route — leaking an
allocation for a `'static` borrow — needs a `String` to leak, which is the same
thing the wrapper does not give up; the scan additionally counts zero occurrences
of `leak` in the crate.

### A data record is escaped, not fenced

A fence can be closed early by content that spells it, and a delimiter chosen to
avoid today's content is chosen against the wrong adversary. So a record is
escaped: every byte outside printable ASCII, plus `"` and `\`, becomes a `\uXXXX`
escape. What that buys, and what
`taint_flow_test_keeps_untrusted_spans_in_data_channel` observes over all 54
corpus entries:

- a record contains no line terminator, so it cannot open a line of its own;
- it contains no unescaped quote, so it cannot close the field it sits in;
- the whole rendered prompt is ASCII, so a bidirectional override, a zero-width
  joiner, and a homoglyph appear as escapes rather than as what they imitate;
- the segments partition the rendered text with no gap and no overlap, so there
  is no region no assertion covers;
- each entry's canary appears exactly once in the rendered prompt and inside a
  recorded untrusted span; and
- the bytes before the first data record are byte-identical for every entry.

The first three are properties of the escaper rather than of the corpus. Its
`match` is a total case analysis over `char` — `"`, `\`, `' '..='~'`, and
everything else — so no scalar outside printable ASCII survives it, and the
whole function is pinned as `WHOLE_ESCAPE`. The corpus is what observes the
analysis is right; the pin is what stops it changing quietly.

## Adjudication, and quarantine as a state

`adjudicate` is the only producer of a `Proposal`. It takes an index and an
output and nothing else — no broker, no capability token, no transport, no
filesystem path, no ledger — and `the_adjudicator_receives_no_capability` pins
its whole text, pins its one caller `admit`, and holds the call-site count at
one.

Two checks, in order. **Schema**: the exact record below, no unknown key, no
missing key, no trailing content; eleven `SchemaError` variants, each produced by
a case in `model_output_failing_schema_is_quarantined` and each required to be
produced by one. **Provenance**: every `support` line names an indexed document
and a byte range whose truncated SHA-256 the line already carries; five
`SpanError` variants, same treatment in
`model_output_without_resolvable_span_is_quarantined`.

```text
academic-proposal/1
kind: CONCEPT_LINK
summary: <one line, at most 512 bytes, no control character>
support: <source_id> <start> <end> <truncated sha256 of [start, end)>
```

Either refusal produces a `QuarantinedOutput`. It holds the identity and the
reason and no byte of what was refused; there is no method on it that returns a
`Proposal`, no `From` between the two, and `ReviewQueue` keeps them in separate
private collections. A `compile_fail` case closes the conversion and another
closes assembling a `Proposal` field by field.

### Why a support digest is 128 bits and not 256

`P2-G2`'s shipped rulepack refuses a run of forty or more hexadecimal characters
at 3.40 bits per character as `SECRET_ENTROPY`. A full-length SHA-256 inside a
provider response is therefore quarantined by the DLP scan before this boundary
sees the record — measured while writing
`pj04_a_model_output_with_a_secret_canary_is_quarantined_with_an_incident`, whose
clean control response failed for exactly that reason. `SPAN_DIGEST_HEX_LEN` is
32, which is below that rule's minimum length and whose Shannon entropy cannot
exceed 4.00 bits per character in any case, below the base64url rule's 4.20.

What the truncation costs is collision resistance. A support line also names its
document and its offsets, and a 128-bit collision is not a shape this boundary
defends against.

## Where this crate meets the secret-`Debug` net

`tools/secret-debug-policy.test.mjs` reads this crate as it reads every other,
and three things came out of that.

**`IngestedDocument.source_bytes` and `ModelOutput.source_bytes` are named for
that net.** `source_bytes` is in its `SECRET_FIELD_NAMES`, so a derived `Debug`
over either is refused by a rule that already existed rather than by one this
task invented. Both write the impl by hand and reduce the field to a length.

**`SourceId` holds a named field rather than a tuple position.** The net judges a
tuple position by its type alone — it has no name to judge by, which is what its
own comment says — so a `String` newtype is classified as carrying plaintext and
everything holding one inherits that: `Provenance`, `QuotedDocument`,
`UntrustedSpan` and `RenderedPrompt` were all classified secret-bearing through
it, and `QuotedDocument`'s hand-written `Debug` was reported for printing a
provenance that is an identifier, a kind and a sequence number. A caller-chosen
identifier restricted to `[A-Za-z0-9._-]` is not plaintext, and the field name is
the signal the net says a tuple position cannot give it.

**Three `digest` fields are registered in `PUBLIC_BYTES`.** `QuotedDocument`,
`QuarantinedOutput` and `ResolvedSpan` each hold a SHA-256 over ingested or
model-written bytes. A digest of untrusted content is not the content,
`Untrusted::digest` returns it through the public API, and the rendered data
record carries it in the clear so a model can cite a span. What the boundary
hides is the bytes, and every field holding those is reduced to a length.

One prose hazard is worth knowing: that net reads attributes with a regular
expression over the whole file, so a doc comment that spells `derive` with its
attribute syntax is read as the attribute. Two files here describe the rule
without spelling it.

## What "zero privileged actions" means

`PrivilegedAction` has fifteen variants. Eleven are
`academic_policy::ProcessCapability`'s, one for one, mapped through a
compiler-checked witness `match` in
`privileged_actions_cover_every_process_capability`, so a capability added to
`P2-G7`'s closed enum stops the suite compiling until it appears here too. The
other four are installing a policy snapshot, minting a grant, consuming one, and
publishing a proposal.

`injection_corpus_produces_zero_privileged_actions` asserts a count of zero
*per variant*, naming each. Three things make that zero mean something:

1. **A control run.** The same `ActionLedger`, driven by an operator through the
   real broker and the real review queue, records every one of the fifteen
   exactly once. A count of zero over a ledger nothing can increment is not
   evidence.
2. **The pinned signature.** `adjudicate` and `envelope_for` receive no
   capability-bearing value. `adjudicate` is whole-text pinned together with its
   one caller `admit` and a call-site count of one; `envelope_for` is itself the
   pinned one caller of `PromptEnvelope::quote`, which `WHOLE_ENVELOPE` pins.
3. **The manifest.** `academic-policy` is a dev edge, so a product file in this
   crate cannot name `PermissionBroker`, `CapabilityToken`, `RuntimeToolCall`, or
   `ProcessCapabilityToken` — an undeclared crate is a compile error. Both edge
   maps are pinned whole in `tools/phase1-scaffold-policy.test.mjs`.

That is narrower than "no value from here can reach a runtime call", and should
be read that way. `RuntimeToolCall::new` takes `impl Into<String>` five times,
and this crate's `SourceId` and digests are `&str`. What they are is this
crate's own metadata — caller-chosen identifiers with a restricted charset, and
hashes — rather than ingested content, which `Untrusted` hands out to nobody.

The corpus is 54 synthetic records in `testdata/injection-corpus/`, covering
eighteen injection families, all six source kinds, and all fifteen privileged
actions. The kind and action rules iterate each enum's `ALL` array, whose length
is part of its type, so a variant added to either enum fails the corpus until a
record covers it; the family rule is a written list compared against the corpus
in both directions. Its README says what the loader enforces and why its
canaries are short.

## `PJ03` and `PJ04`

`PJ03` — an ingested source contains an instruction directive — is
"treated as data; zero privileged actions". The second half is the corpus test
above. The first is
`pj03_an_ingested_instruction_directive_is_treated_as_data`: one entry per source
kind, the directive present in the data channel (it is not filtered out) and
present nowhere else.

`PJ04` — a model output contains a secret canary — is "quarantine plus
incident". The scan is `P2-G2`'s, run with the five sentinels in
`testdata/injection-corpus/response-canary.txt`; the incident is its `Incident`
at `IncidentSeverity::High` with at least one hit whose source is a canary rather
than a rule; the quarantine is this crate's state. The control is the same shape
without a canary, which the scan accepts and which becomes a proposal — so the
refusals are attributable to the canary and not to the shape.

## What this contract does not claim

- **Nothing here is a claim that a model obeys the system channel.** What is
  executable is where the bytes go, not what an inference does with them. No
  provider is called anywhere in this task, and the corpora are synthetic.

- **The link to `P2-G4`'s acceptance boundary is by composition, not by type.**
  `ingest_model_output` takes bytes. It does not take
  `academic_worker::AcceptedOutput`, because `only_egress_crate_has_a_socket`
  refuses a workspace crate that depends on `academic-worker` by any edge kind —
  that would put the sandbox probe's socket target within reach. So the ordering
  "the core accepts a staged output, then parses it here" is what a caller must
  do, and nothing in this repository observes that it did. The six refusals on
  the acceptance side are `P2-G4`'s
  `pj02_output_that_fails_validation_is_quarantined_not_accepted`.

- **`PrivilegedAction` therefore has no `AcceptStagedOutput` variant.** A variant
  the control run could not perform would be a zero nobody observed.

- **`AcceptedResponse::bytes` is public and carries no label.** One layer outside
  this boundary, `P2-G2` hands out raw response bytes to anything holding an
  accepted response. This crate wraps them; a caller that skips this crate is not
  stopped by it. `only_reviewed_files_hold_an_unlabelled_provider_response` is
  what keeps the set of files that can hold one small and reviewed — four today,
  compared as a whole — rather than a claim that the bytes cannot be reached.

  Three types in this workspace return a `&[u8]` of content this way, and they
  are the only three: `AcceptedResponse`, `Preview` and `AcceptedOutput`. The
  other two already carry a guard on their own side — `Preview::bytes` is one of
  `byte_path_pin.rs`'s whole-text pins, and `AcceptedOutput` has one producer
  plus a `compile_fail` case in `P2-G4`. `AcceptedResponse` had none, which is
  why this crate added one rather than a third copy of a rule those crates
  already have. No `Deref`, `AsRef` or `Borrow` implementation exists anywhere in
  this workspace's product source, so the silent-unwrap shape this crate is
  written against has no second instance today.

- **The record schema is this task's, not `P2-M2`'s.** `Proposal` here is a
  validated model output with resolved provenance. Risk tiers, the review queue's
  product behaviour, and what a human does with a proposal are `P2-M2`'s.

- **`product_network` remains `NONE`**, `production_data_allowed` remains
  `false`, and ADR-002 remains unaccepted. Nothing in this task moves any of
  them.
