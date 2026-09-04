# Consent ledger and capture permission

`academic-consent` is the `P2-G6` boundary: the section 3.7 `capture_permission`
aggregate, the append-only consent ledger under it, and the one decision that
turns a recorded permission into a capture capability.

It opens no socket — `only_egress_crate_has_a_socket` holds that, and its
allowance entry for this crate is absent rather than empty. It reads no clock —
`every_instant_this_crate_compares_is_an_argument` holds that, and every instant
it compares arrives as an argument, which is also why the acceptance rows can
name the instants they assert against. It stores no byte of anything it
describes: every evidence item is a locator, a digest and a length, which is the
"No text field" section below.

`P2-L1` owns the daemon-side evaluation that turns a `CaptureCapabilityToken`
into a microphone and the enforcement at the device layer. What is here is the
decision and the ledger the decision reads.

## What a status is, and where it comes from

`CaptureStatus` is section 3.7's five: `UNKNOWN`, `PROHIBITED`, `PERMITTED`,
`PERMITTED_WITH_CONDITIONS`, `EXPIRED`. `UNKNOWN` is its `Default`.

`status_of` is the whole derivation, and it takes a `PermissionRecord`. A scope
with no record has nothing to hand it, so `ConsentLedger::status` returns
`UNKNOWN` for one — that is the default, and it is the absence of a record
rather than a branch on the way to a permissive base. A new `ConsentLedger` has
no records, no template, and no seeded row.

The order of the tests inside `status_of` is section 3.7's own: a written
refusal is `PROHIBITED` whatever else is true; then the grant's own `not_after`
and the scope interval; then a stale verification; then whether anything is
outstanding. A verification is stale when it was recorded before the interval it
is recorded against began, which is how a grant carried forward from one term
into the next reads `EXPIRED` rather than continuing.

`status_of` has three callers — the binding, the ledger query, and the append
that records a permission — and `a_status_comes_from_one_derivation_and_absence_is_unknown`
counts them. Outside `status.rs`, the two permitting variants are named in
exactly one place, which is the pinned binding below.

## Evidence is not authority, and the difference is a type

Section 3.7: `user_attestation` is an evidence kind, never a status transition.
Section 12.1 of the authoritative spec says the same at length, and adds the
second case: a self-judgement that personal use makes a recording acceptable
does not move a permission either.

Here the two are unrelated types:

| Type | What it is | What produces it |
|---|---|---|
| `AttestationRecord` | a user's own account of events: when an oral permission was heard, and the digest of the conditions heard with it | `AttestationRecord::file` |
| `WrittenAuthority` | an act by one of section 3.7's three authorities, with the document it is written in | `WrittenAuthority::new` |
| `AuthorityGrant` | what a written authority granted, with everything section 3.7 requires of a grant | `AuthorityGrant::record`, which takes a `WrittenAuthority` |

There is no `From`, no `TryFrom`, and no fallible upgrade between the first and
the second. Four separate things hold that:

* the compiler, for a caller who passes one where the other belongs —
  `tests/compile_fail/attestation_is_not_a_written_authority.rs` is that program,
  and its committed diagnostic is compared;
* the whole set of `impl` blocks whose header names `AuthorityGrant`, and the
  whole set naming `AttestationRecord`, each compared against a one-entry list,
  so a conversion trait added later fails as an extra key;
* a rule over every `pub` signature in every package in `crates/`: none takes an
  `AttestationRecord` and returns a type naming `AuthorityGrant`,
  `WrittenAuthority`, `BoundPermission`, `CaptureCapabilityToken` or
  `CaptureStatus`. That rule is workspace-wide because `AttestationRecord` is a
  public type any crate can name, and it is `P2-RF10`'s
  `no_public_signature_hands_out_ingested_text` applied to the other direction
  of the same mistake;
* behaviourally, `ConsentLedger::record_attestation` appends one entry, touches
  neither the records nor the recheck queue, and leaves the status it found.

`ConsentEventKind::AttestationRecorded` and
`ConsentEventKind::PermissionGranted` are different arms for the same reason:
filing an attestation is a recorded act, and it is not the act of granting.

## The section 3.7 aggregate

`PermissionRecord` carries the aggregate identifier, the `permission_seq` that
completes section 3.7's `(offering_id, permission_seq)` key — refused below one
by the constructor and by the migration's `CHECK`, so a record the database
would reject is not representable in memory either — a
`PermissionScope`, a `Disposition`, the seven-dimension `Checklist`,
`verified_at`, and `verification_source_digest`. `Disposition` has two arms —
`Prohibited(RefusalRecord)` and `Granted(AuthorityGrant)` — and no third: the
unset case is the absence of a record, so `UNKNOWN` is not reachable from the
enum.

A refusal carries a `WrittenAuthority` too. A `PROHIBITED` nobody verified would
be a status transition from an attestation in the other direction.

`AuthorityGrant` carries the `WrittenAuthority`, a `PermittedUse`, a
`RetentionTerms`, a sorted and deduplicated `Vec<Condition>`, the
`conditions_hash` computed over it, and `not_after`. `PermittedUse` is the
allowed-media set, the allowed-processing set,
`external_processing_allowed` and `sharing_allowed`. All four are required
arguments; section 3.7 defaults the two flags to `0`, and they are arguments a
caller has to write rather than fields a caller can omit. There is no `Default`,
and the whole-set `impl` rule is what refuses one added later.

A grant whose `not_after` falls outside the scope interval is refused at
`PermissionRecord::record` rather than clamped, because clamping would rewrite
what the authority wrote.

## Scope, and the semester recheck

`PermissionScope` pins an offering, a `TermKey`, a `ScopeGrain` (a whole term or
one named session), and a half-open interval. `PermissionScope::answers`
compares all of the offering, the term, and the session.

Comparing the term as well as the offering is redundant today, because an
offering is already a term's section of a course. It is compared anyway: the two
identifiers travel separately through every surface above this one, and a
permission that answered on the offering alone would answer a request whose term
field said something else.

That comparison is the semester recheck. A record written for `2026-1` does not
answer a request in `2026-2` — not because a timer fired, but because the
request names a term the record does not cover — so the next term starts at
`UNKNOWN`. There is no carry-forward path, and the recheck queue is a
consequence rather than a schedule: a scope reaches `RecheckItem` because a
capture was refused for it at `UNKNOWN` or `EXPIRED`, the two states a user can
clear by confirming the offering again. `PROHIBITED` does not queue.

## The checklist

`ChecklistDimension` is section 12.1's seven: syllabus or LMS policy, student
speech, filming scope, accessibility procedure, copyright, privacy,
institutional rules. `CHECKLIST_DIMENSIONS` holds all seven in declaration
order, and the scan reads both the enum and the array out of the source and
compares them against each other and against the seven this page names.

A dimension is answered with an artifact or with a `NotApplicableReason` from a
closed list, or it is unanswered. `ChecklistEntry` has exactly those two arms:
"does not apply" and "nobody looked" would otherwise produce the same empty
cell. The reasons are an enum rather than free text both because a reason nobody
can enumerate is a reason nothing can review, and because a free-text field here
would be this crate's only string field — see "No text field" below.

An omission does not deny the recorder. With a written grant and a hole in the
checklist the status is `PERMITTED_WITH_CONDITIONS`, which is a permitting
status; with no written grant an answered checklist changes nothing and the
status is `UNKNOWN`. Those two are what
`checklist_omission_yields_conditional_or_unknown` asserts, one per dimension
across all seven. What the omission does is travel: `Checklist::unanswered` is
copied onto the minted token, so the exact dimensions nobody answered are
visible at the device layer rather than lost between the ledger and the
microphone.

`Checklist::answer` is the one mutator, it refuses a dimension that already has
an entry, and there is no removal path. A correction is a new `PermissionRecord`
at the next `permission_seq`.

## Audio and transcript retention are two values

`RetentionTerms` holds an audio `RetentionBound` and a transcript
`RetentionBound`, and no accessor returns one for the other. `RetentionBound` is
`Prohibited` or `Until(instant)`; `Prohibited` is the first variant, so the
derived `Ord` ranks it below every instant and `stricter` is `min`.

The independence is not a convention. An instructor may permit a transcript for
the term while refusing to let the recording outlive the lecture, and one
`retention_until` column can hold neither of those without silently widening or
narrowing the other. Three things hold it:

* the struct is pinned as whole text, and the scan reads its fields out of the
  source and requires exactly two `RetentionBound` fields with different names;
* migration `0006` gives each axis its own kind column and its own instant
  column, with a `CHECK` pairing them, and the scan requires each of the four to
  be declared exactly once;
* `audio_and_transcript_retention_are_independent` walks a grid of
  `(audio, transcript)` pairs through the deletion preview and requires every
  pair to produce a distinct digest, so no two pairs collapse onto one report.

## A derivative may only narrow

`RetentionTerms::inherit` takes the stricter bound on each axis independently.
There is one inheritance function in this crate and one caller of it, counted;
a second copy of the rule is how the two would drift apart.

`derivative_expiry_is_equal_or_stricter` does not check a case. It walks the
whole cross product of a five-value bound grid on both axes of both sides — 625
pairs — and requires `derived <= parent` and `derived <= requested` for every
one, plus that inheriting twice is inheriting once. One character reverses the
comparison, and the grid fails on the first pair where the two bounds differ
rather than on a case somebody remembered to write.

## One binding, and every path runs it

`bind_permission` is where every comparison section 3.7 asks for happens: the
request is whole, a record answers the scope, the status permits, the interval
contains the instant, at least one medium is requested and every requested
medium and processing step is on the grant, external processing has its own
flag, and the requested lifetime reaches past neither the grant nor the scope
and is not already over. That last clause is why
`mint_capture_capability` cannot return a token `continue_capture` would refuse
on its first call.

Two functions call it, both as their first statement:
`mint_capture_capability`, and `continue_capture` — which re-runs the whole
binding for a capture that is already running rather than comparing the token's
own `not_after`. `P2-RF10` is why the second exists at all and why the call
sites are counted: `EgressProxy::transmit_without_completion` was a second
public path that skipped a check the first one made, and nothing counted the
sites, so deleting the check outright left the whole suite green.

`CaptureRequest` is seven `Option` fields and `ResolvedRequest::resolve` turns
each absent one into `INCOMPLETE_REQUEST`. That is `CompleteRequest::resolve` in
`academic-policy`, reused rather than reinvented. An empty `media` list
resolves — it is a present, empty set, and it fails the subset comparison rather
than the presence one, which keeps "asked for nothing" distinct from "did not
say".

`CaptureCapabilityToken` has no public constructor and one struct literal, in
`mint_capture_capability`. It holds the exact `CaptureRequest` it was minted
from, so `continue_capture` binds against the same tuple rather than a narrower
one assembled at the call site.

Every refusing path appends its audit row before returning:
`record_capture_denial` returns the denial it was handed, so no early return can
skip the row on its way out, and its three call sites are counted.

## An expiry cannot be applied without its preview

`preview_expiry` reports an audio line, a transcript line, and one node per
derivative class in registry order — including classes with nothing in them, so
a class with no objects is a node saying so rather than a row that vanishes.
Every derivative node carries the whole inherited `RetentionTerms`, both axes,
so a subject whose audio has expired and whose transcript has not produces a
preview that says exactly that.

`ExpiryPlan::from_preview` is the only constructor of `ExpiryPlan`, `apply_expiry`
is the only consumer of one, and `apply_expiry` compares the plan's previewed
instant against the instant it is applied at rather than trusting it. A plan
previewed at one instant and applied at another would delete against a set the
user was never shown.

## A legal exception is an external task

Section 12.1: an exception that needs a legal judgement is not something the
system estimates; it stays as an item for the institution's responsible office
or a professional to confirm.

`ExternalReviewTask` carries a `LegalQuestion`, a `ReferralTarget`, a scope, and
an instant. It has no resolution API: no field holds a determination, so there is
nothing on it to read a conclusion from. `open_external_review` takes no ledger
and returns a task; `ConsentLedger::record_external_review` is the separate
append and reads two enum fields off the task.

`no_legal_conclusion_reaches_a_permission` refuses any `pub` signature in any
package that takes a `LegalQuestion` or an `ExternalReviewTask` and returns one
of the permitting types, and refuses `external.rs` declaring a function named
for a conclusion. `legal_exception_is_an_external_task_not_an_inference` is the
behavioural half: a scope with an open review has the status it had before, and
opening every question against a written refusal leaves it `PROHIBITED`.

## `GATE-38-009` and `GATE-38-019` stay open

Both are per-offering, per-term inputs, and both are the absence of a value here
rather than a default:

| Gate | What it is | What stands while it is empty |
|---|---|---|
| `GATE-38-009` | whether this offering permits capture, and on whose written authority — section 38.1 asks for it every term | no record answers the scope, so the status is `UNKNOWN` and nothing is mintable |
| `GATE-38-019` | which media and which local or external processing the offering's conditions cover — section 38.2 asks the user to confirm it | the grant's media set is empty, so every request naming a medium is refused with `MEDIUM_NOT_GRANTED` — and so is a request naming none, which is why asking for nothing is not a way past an unconfirmed offering |

`ConsentLedger::unfilled_cells` reports which of the two is empty for one
offering and term. There is no constant holding a "usual" media set, no
`Default` on `AuthorityGrant` or `PermittedUse`, and no fallback that reads one
offering's answer for another. `academic-retention`'s `OriginalVoiceAuthority`
leaves `GATE-38-026` open the same way.

## Migration `0006`

`0006_phase2_consent_and_capture.sql` adds the typed columns for the
`CAPTURE_PERMISSION_RECORDED` and `CONSENT_RECORDED` arms. Migration `0004`
states the rule it follows: the v3 registration frame carries no typed aggregate
attributes and each aggregate owner adds its own later. It adds no event kind,
no Proto tag, and no CBOR arm.

Five tables: `capture_permission_terms` keyed by the aggregate identifier with
`UNIQUE (offering_id, permission_seq)`, the two set tables
`capture_permission_medium` and `capture_permission_processing`,
`capture_permission_checklist` with a `CHECK` requiring exactly one of an
artifact or a not-applicable reason, and `consent_record`. Each carries the
append-only trigger pair and each is in the authorizer's canonical set, which
`authorizer_covers_every_canonical_table` compares against each other and
`the_migration_vocabularies_are_the_rust_ones` compares against the file.

Two triggers bind a row to its event, the way migration `0005` does: the
aggregate identifier is a foreign key, so a typed row cannot exist without the
accepted event that registered it, and the row's `record_digest` must be the
`source_digest` that event carries.

Every `CHECK` list in the file is compared against the Rust `as_str` spellings
of the enum it mirrors, in both directions, and against that enum's variant
count. Eight vocabularies are compared this way.

The migration runs in the encrypted lane, beside `0004` and `0005`. The
plaintext default lane applies `0001` only, which is why nothing in
`cargo test --workspace` reads `STORE_MIGRATION_SQL`:
`encrypted_profile_v2_is_created_only_by_cipher_lane` enumerates that list
whole, in the `sqlcipher-store` lane, and it is the assertion `0006` extends.

## No text field

Every evidence item in this crate is a locator plus a digest plus a byte count.
No field holds the syllabus text, the announcement body, or the words a user
typed, and the closed `NotApplicableReason` enum is why the checklist needs
none.

That is why this crate adds nothing to the `S-10` row in
[policy source scans](policy-source-scans.md). The generic secret-`Debug`
vocabulary that row is about reaches field names like `text`, `bytes`,
`escaped` and `staged_text`; this crate declares none of them, so the decision
that row leaves open for a crate that does is not one this task had to make.

It is also why an evidence item needs no `Untrusted<T>` wrapper from `P2-G5`:
there are no ingested bytes here to mislabel. The bytes stay in the vault the
locator points at, and `P2-G5` owns them there.

## Why the derivative-class list is restated

`academic-retention` owns the closed list of things a deletion has to reach.
Importing it would mean a product edge to that crate, and
`rotation_engine_lane_is_not_default` holds that exactly two crates declare that
edge — `academic-portability`'s encrypted restore and `P2-P2`'s deletion flow,
which is the layer that decides when a key slot is destroyed. A consent ledger
is neither, so the list is declared here and `academic-retention` is a **dev**
dependency.

`the_two_derivative_vocabularies_are_the_same_list` compares both lists whole —
the spellings, their order, and the two enums' variant names — so the day either
side gains, loses, or reorders a class, the suite fails. Reordering the consent
copy alone is injection `I22`.

## Open

| # | What is open | When it starts mattering |
|---|---|---|
| C-1 | `PermissionRequest.consent_evidence_id` in `academic-policy` is a `String` the caller supplies and the rule compares for equality. Its uses in that crate are four: a non-empty check on the rule, a non-empty check on the request, an equality comparison between the two, and two hashes over the result. Both halves are strings a caller writes, and every caller in this repository writes a literal — `"synthetic-consent-event"` in three test trees and `"synthetic-consent"` in a fourth. So what the broker enforces is that a request cites the same consent the rule was written against, which is narrower than what "consent evidence" reads as: nothing establishes that the identifier names a consent any ledger issued. | The first egress rule written against a real consent record — `P2-U6`, `P2-R1`, and `P2-M1`'s `transmitted_ranges_reconcile_with_egress_audit`. Closing it means the broker resolving the identifier against a ledger, which is a product edge and a decision for `academic-policy`'s owner. Severity **P2**: it is the eighth of the eight §32.3 fields, and it is the one with no issuer. |
| C-2 | Nothing writes migration `0006`'s rows. The tables, their `CHECK`s, their triggers and their vocabularies are in place and compared against the Rust enums; the writer that turns a `PermissionRecord` into a `capture_permission_terms` row is `P2-L1`'s, alongside the daemon evaluation. So `0006` is checked as a schema and not as a round trip. | `P2-L1`. Until then the aggregate's durable form is asserted structurally rather than exercised. |
| C-3 | `continue_capture` compares the binding's `permission_id` against the token's, so a superseding record at a higher `permission_seq` stops a running capture. It does not distinguish a superseding record that is *wider* from one that is *narrower*: both stop the capture. That is the fail-closed direction, and it means a user who widens a permission mid-lecture has to start a new capture. | A capture UX that wants to continue across a widening. Deciding otherwise means deciding which widenings are safe to apply to a capture already running, which is `P2-L1`'s question and not this crate's. |

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`, the acceptance public key is unprovisioned, and every
fixture in this crate's test tree is synthetic and built from committed
literals. No recording is made, no device is opened, and no permission in this
repository refers to a real offering.
