# The evidence and correction centre

`academic-evidence-center` is `P2-X7`. Section 25.13 of the authoritative
specification calls it *`OS의 신뢰를 만드는 핵심 화면`* and names six things it
holds. This crate holds those six as typed records, and `packages/ui`'s
`evidence-center.ts` is the shell side that says which of section 25.1's four
`Evidence & Settings` children shows which of them.

It persists nothing. It claims no migration number, and the section below says
why with the evidence.

## What this is not evidence for

**No window opens.** `P2-X1` merged with no Tauri runtime linked, and that
decision is still open under the user gate. Nothing here depends on a window,
and nothing here is evidence that one exists: the crate is a set of typed
records and the rules between them, checked by compiling it, running its tests,
or reading its source. The shell half adds that opening one of four destinations
yields sections naming section 25.13's own content instead of a promise — that
is a structure, not a rendering, and `P2-X1`'s page says the same thing about
its own frame.

**The provider cannot be rendered as words.** A transmission row names its
provider by the digest `academic-policy`'s `ProviderIdentity::destination_id`
derives, not by a vendor name. That is a deliberate cost: a surface that wants to
show "which provider" must resolve the digest against `P2-G3`'s registry, and
this crate cannot do it. What it buys is that the centre holds no text a caller
supplied at all, which is what makes the field-type inventory below a closed set
rather than a set with one exception in it.

**The identifiers it does hold are other crates' validated ones.**
`PredicateId`, `RuleId`, `ConnectorId` and `DependentId` are `String` inside
their own crates, restricted by their own validators to `[A-Za-z0-9._-]` or
narrower. So the claim is not that no byte of any document can reach this crate;
it is that nothing reaches it except through an identifier that admits no space,
no separator and no directive. A secret that happened to be a valid rule
identifier would pass, and nothing here would notice.

## The six sections

| Section (25.13) | What holds it | The rule that makes it non-trivial |
|---|---|---|
| `AI 제안 inbox` | `ProposalInbox` | four classes are four payload types, not a tag |
| `official source change` | `SourceChangeLog` | the rules and the plans are `P2-U6`'s own answers |
| `unresolved conflict` | `ConflictBoard` | both sides shown, three choices offered, nothing settles it but a user |
| `low-confidence transcript/math/code` | `LowConfidenceQueue` | three span kinds, each with a locator back to its source |
| `permission/consent expiry` | `PermissionQueue` | a lapsed permission blocks its dependents by failing to produce a value |
| `provider transmission log와 deletion receipt` | `TransmissionLog` | six fields, and no type here can hold a payload byte |

`CenterSection::ALL` is that list and `the_six_sections_are_section_25_13s_own`
reads the six bullets out of the specification, requires each arm's words to be
in its own bullet, and then removes the four proposal classes from the first
bullet and the two conflict classes from the third and requires what remains to
be punctuation. A class this crate invented leaves text behind and fails.

Correction markers are not a seventh section. Section 34.6 makes a correction
something that appears *on the screen it corrects*, so `CorrectionLedger` is a
surface every historical view reads rather than an item in the centre's own
list.

## One inbox, and four classes that are four types

`InboxEntry` has four variants and each carries a different payload type:
`RelationProposal`, `ConceptMergeProposal`, `ProjectClassificationProposal`,
`StateUpdateProposal`. The four share only a `ProposalHeader` — `P2-M2`'s
identity, tier, confidence and impact — and no field list beyond it.

The class is read *off* the payload, never carried beside it:

* `InboxEntry::class` is a total `match`, pinned whole, so a fifth variant stops
  the crate compiling and a rewired arm fails the pin as well as the test;
* `ProposalClass` implements no `FromStr`, no `TryFrom<&str>` and no
  `From<&str>`, and the whole `impl` set naming it is compared against a
  one-entry list, so a route from text into a class fails as an extra key;
* the only two fields in the crate whose declared type is `ProposalClass` are
  the index reference and one error variant, compared as a whole set;
* `tests/compile_fail/a_proposal_class_is_not_built_from_a_string.rs` and
  `the_four_proposal_payloads_are_not_interchangeable.rs` are the other half.
  If the four were one shape with a tag, both would compile.

`proposal_inbox_holds_four_typed_classes` drives four hundred proposals across
the four classes and both tier extremes and asserts **set equality in both
directions plus no duplication** — not a count, which a partition that dropped
one entry and emitted another twice would pass. Beside it runs a deliberately
lossy partition, and the test requires *that* one to fail the same equality.

The inbox has no `remove`. A rejection is a decision recorded against a
proposal, which is `P2-M2`'s append-only history; an inbox that could drop an
entry would be a second, silent disposition.

## A source change, and the rules and plans it moves

`SourceChangeEntry::from_diff` computes neither half. The impacted rules are
`SourceDiff::impacted_rules` and the impacted plans are
`DependencyGraph::invalidate`; both are `P2-U6`'s, and recomputing either here
would give the centre a second answer that could disagree with the pipeline's.
What this crate adds is the link: one entry holding a change and both of its
consequences, so a reader sees what moved and what has to be redone together.

Section 29.2's three dependent kinds are requirements, scenarios and course
mappings. Section 25.13's word is `plan`, and `impacted_plans` returns all three
rather than guessing which kind the word covers; `plans_of_kind` is how a caller
narrows it.

`source_change_links_impacted_rules_and_plans` drives `P2-U6`'s own stages one to
five over documents composed in the test, compares both consequence sets whole in
both directions, and carries two controls: a change to a *different* rule must
reach a *different* plan set, and a change to the document's effective date must
reach every plan in it. Without the first, an implementation that returned every
plan would pass.

## Both conflict classes, and what makes them unresolved

Section 30.4 fixes the first: *`AI 재분석은 이 결정을 지우지 않는다. 새 runtime
trace가 반박하면 NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE를 만들고, 사용자가
유지·수정·scope 종료를 선택한다.`* Section 34.4 fixes the second: a specification
read as an implementation produces an `INTENDED_NOT_IMPLEMENTED` drift with the
intent lane and the implementation lane kept apart.

`ConflictCase::both_sides` returns the pair in one call, because a card that can
be built from one side is a card that can show one side.

`offered` takes no argument and returns `CorrectionChoice::ALL`, so no
confidence, age, status or authority comparison can remove an option the user is
entitled to. It is pinned as whole text, and the behavioural half drives the
whole nine-value status vocabulary on both sides of both classes — see
[what a sweep does not cover](#what-a-sweep-does-not-cover).

**Unresolved until a user acts** is four things, and only the first is a check:

1. `settle` takes an `academic_proposal::UserDecision`, which `UserDecision::by`
   issues only for `Actor::User`. `P2-M2` owns that door; `user_receipt` is the
   one place this crate reaches it, and it is pinned whole. There is no actor for
   `settle` to refuse, because an automatic actor cannot produce the argument.
2. There is no other way in. `ConflictCase` has no `resolve`, no `auto_resolve`,
   no `expire`, no setter and no public field, and `Resolution` is computed from
   the history rather than stored — no field of this crate has that type, which
   the field inventory says as a whole set.
3. Settling **appends**. `self.history.push` appears once, `settle` is pinned
   whole, and a second decision leaves the first in the history.
4. Neither side is rewritten. `P2-R3` made `ImplementationDrift` a record that
   rewrites neither lane; the same holds here, and
   `both_conflict_classes_are_unresolved_until_user_action` compares each side
   before and after every attempt.

The test runs every automatic actor against both classes and requires the board
to stay unresolved, then settles with a user receipt and requires the history to
grow rather than to be replaced.

## Three span kinds, and what context means

Section 34.1 requires `token/segment underline, provider/version, 원음 듣기` for
a transcript span and `UNVERIFIED_EQUATION/CODE, confidence와 source image` for
an equation or code span. Section 25.7 requires that selecting a paragraph
returns you to the original audio timestamp and raw segment.

**Context here is a locator, not the text.** A transcript span carries the
session, the transcript version and the millisecond interval a player seeks to; a
document span carries the session, the document, the page and the digest of the
source image. Neither carries a transcribed word or an image byte, which is
section 32.8's rule — the audit surface does not copy sensitive originals into
itself — and which is what makes the field-type inventory below possible at all.

The two locators are different types, so a transcript span cannot carry a page
and an equation span cannot carry an audio range. The three uncertainty markers
are required to be distinct, so two kinds cannot share one.

## An expiry that blocks by failing to produce a value

Section 32.6: *`Provider 정책이 바뀌거나 마지막 확인이 오래되면 permission을 자동
연장하지 않는다`*. Section 34.1's `허가 없는 녹음` row: *`default UNKNOWN, Record
fail-closed`*.

`LivePermission` has private fields, no public constructor, no `Default`, no
`Clone` and no `Copy`. `PermissionQueue::gate` is its only producer and produces
one only strictly before the expiry. A dependent action that needs a permission
takes a `LivePermission` by value, so an expired permission does not fail a check
— it fails to produce the argument, and one gate call authorises one action.
`tests/compile_fail/a_live_permission_has_no_struct_literal.rs` is the privacy
half, in its own file because a field-privacy diagnostic is suppressed when the
same file already carries a type error.

`has_lapsed` compares `expires_at <= at`, so an instant exactly on the expiry is
expired. That boundary is section 34.1's `Record fail-closed`, and the comparison
is pinned as whole text.

An action whose permission the queue does not hold at all is blocked too. That is
section 34.1's `default UNKNOWN`: an unrecorded permission is not an unrestricted
one.

Nothing extends an expiry. There is no `renew`, `extend`, `refresh` or setter,
and no public signature in the module mutates anything but the two recording
doors. A lapsed permission is re-attested, which is a new record with a new
expiry, and that is `P2-G6`'s ledger rather than this crate's.

## The transmission log, and why no payload byte can be in it

Six accessors, one per thing the contract fixes — purpose, payload digest,
ranges, provider, time, receipt — plus the `P2-G1` egress decision that links a
row to the broker's own audit row.

A range is two integers. `ReceiptState::NotOffered` is fault `EG07` as a state
rather than as an absent receipt, so "this provider will never give one" and "we
have not asked yet" are different things on the screen, and
`without_offered_receipt` makes the `EG07` transmissions findable rather than
silently missing from the receipt list.

`Debug` is derived on every type here. That is safe *because of* the layers
below rather than despite them: a derive prints fields, and no field of this
crate can hold a byte of a payload. `P2-G7`'s leak went the other way — an audit
side table grew a `transmitted_bytes` field, the canary guard was a token list,
and the derive printed it.

### Four layers, and the one that was false as first drafted

**The closure does not say `StagedPayload` is unreachable, and this page does not
claim it does.** `academic-untrusted-content` declares a product edge to
`academic-egress-boundary`, so every crate that links `P2-G5`'s trust label
carries that closure — `academic-ingestion`, `academic-curriculum`,
`academic-requirement`, `academic-repository`, and this one. `P2-U2`'s admission
receipt already records it. What the closure comparison carries is the narrower
claim: the whole closure is compared at thirteen and twelve crates that own a
canonical write, a key, a model run or a process are each required to be absent.

1. **Path roots.** A closed world over every identifier the product source writes
   a `::` after, compared in both directions, read on paths rather than on `use`
   items, in which `academic_untrusted_content`, `academic_egress_boundary`,
   `academic_policy`, `std`, `alloc`, `libc` and `rusqlite` do not appear. This
   is what replaces the edge claim. `P2-R2`'s three repairs — a leading `::`,
   whitespace inside a path, a middle segment — are each exercised against the
   reader inside the test.
2. **Declared types.** Every field position in the crate is read as an
   `(owner, name, declared type)` triple, enum struct-variants as
   `Enum::Variant` and enum tuple positions as `Enum::Variant#n`, and the whole
   set of *type constructors* those declarations use is compared in both
   directions against a reviewed seventy-seven-entry allowlist. `String`, `str`,
   `u8`, `Box`, `Cow` and `Untrusted` are absent from it, and their absence is
   the claim. The crate declares no tuple struct, because a tuple field has no
   name to inventory.
3. **The public surface.** The same extraction over every public signature, with
   argument names excluded, so bytes cannot cross the boundary in a value nothing
   stores. Beside it, the eight functions returning a `&'static str` are
   enumerated as a whole set: that is the one shape by which text leaves here.
4. **The spellings.** A ten-entry forbidden-token list, **explicitly the weakest
   layer** and listed last, because a list is broken by the spelling nobody
   predicted.

`T166` measured `tools/secret-debug-policy.test.mjs` passing a `Vec<u8>` field
named `excerpt`, because that tool matches field **names** against a fixed
alternation. Layer two fails on `excerpt: [u8; 64]`, on a private struct holding
`[u8; 32]`, and on a `String` field whose constructor was edited to match — none
of which spells anything on layer four's list. Repairing that tool is
`T167`/`P2-RF13`'s; what this task did is close its own boundary by the whole-set
route, as `P2-R3` did.

## Correction markers, and why a time-travel view cannot hide one

Section 34.6's fourth recovery principle: *`과거 화면에는 당시 잘못된 결과가
사용되었음을 correction marker로 남긴다`*.

A correction is always recorded *after* the reading it corrects. So a marker
filtered by the same as-known-at coordinate as the claims it annotates would be
invisible in exactly the view that needs it. `CorrectionLedger::view_at`
therefore reads two things at two different coordinates on purpose:

* the **claims shown** are filtered by both bitemporal axes, because that is what
  the past screen showed and rewriting it would destroy the record;
* the **markers** are not, because a marker is a present-tense statement *about*
  that past screen.

The wrong claim is still in `HistoricalView::shown` after the correction lands —
sections 34.6's first and second principles — and a view that dropped it could
not answer the question the marker exists for.

`correction_marker_appears_in_historical_views` reads the view at a coordinate
strictly before the correction's own acceptance sequence, requires the marker to
be there and the wrong claim still to be shown, and carries three controls: an
uncorrected claim in the same view is unmarked, a view the wrong claim never
reached carries no marker, and a claim that applies from a later instant stays
out of an earlier valid-at view. Without them the assertion would pass over a
marker shown unconditionally.

`CorrectionOrigin` is `P2-C6`'s four, named rather than redefined. Only
`EVIDENCE_CHANGE` means the user changed; the other three are changes in the
observation system, which is section 34.4's `analyzer/model 변화가 code 변화처럼
보임` row applied to the trust screen itself.

## What a sweep does not cover

`X7-I15` narrowed the three offered choices when the incoming side is
`OFFICIAL_CONFIRMED`. The assertion that used to check `offered` ran once, on a
corpus whose two sides both carry `CODE_OBSERVED`, so the injected branch was
never reached and the test passed; the injection was caught only by a source scan
that noticed an unrelated path root. **A guard that refuses something is not
evidence that it refuses what you meant.**

The repair drives the whole nine-value status vocabulary on both sides of both
classes, with a compiler-checked witness so a tenth arm in `academic-domain`
stops the suite compiling. And the same shape is one step out: a sweep is bounded
by what it varies, and this one holds the authority class fixed, so `X7-I27`
narrows on `AuthorityClass::Curated` and the sweep cannot see it. `offered` is
pinned as whole text beside the sweep for that reason — the pin refuses a
narrowing keyed on anything at all, and the sweep is what says the constant is
actually returned.

## The shell half

`packages/ui/src/evidence-center.ts` maps section 25.13's six sections onto
section 25.1's four `Evidence & Settings` children. The mapping is not
one-to-one and is not meant to be: there are six sections and four children, so
`Source / Claim Review` carries four of them.

`the_shell_sections_are_the_crates_own` compares the six identifiers and the six
specification strings against `CenterSection::spec_words`, read out of the Rust
source. Neither side is derived from the other, so a section renamed in Rust
fails in TypeScript rather than drifting. There is no runtime across which the
two could be compared, which is why the comparison is on source text and why
this file has a row in [policy source scans](policy-source-scans.md).

**`Export / Backup / Audit` is not `P2-X7`'s.** Section 25.13 names six sections
and none of them is export, backup or audit: those are section 32.10's, and the
plan gives them to `P2-P1` and `P2-P2`. `P2-X1` assigned that route to `P2-X7`
before either was written; the assignment is corrected here rather than left as a
promise nobody owns. The deletion-*receipt* half that is `P2-X7`'s sits on
`Privacy / Providers`, beside the transmission it belongs to, and `P2-P2` reads
it from here: `DeletionReceiptRef` and `ReceiptState` are what
[the deletion flow](deletion-and-retention-flow.md) links to the artifact
deletion that caused the erasure request, rather than declaring a second receipt
shape.

## No migration, and why

`P2-X7` adds **no migration**. The number reserved for it is unused.

This crate persists nothing. Every value it holds is computed from records other
aggregates already own — a proposal `P2-M2` queued, a diff `P2-U6` produced, a
claim the ledger holds, a grant `P2-G1` minted, a receipt `P2-G3` stored — and
the centre is the screen that puts them side by side. There is no table, no
schema and no writer.

Evidence: `git diff --stat main -- migrations/ crates/store/` is empty, and
`crates/store/tests/encrypted_profile.rs`'s `STORE_MIGRATION_SQL` element-and-
length pin is untouched. The `sqlcipher-store` lane was run under WSL2 anyway,
as insurance against the shape `P2-U1`'s `0014` hit, where the default lane was
structurally unable to see the pin it broke.

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`. `GATE-38-028` is open, so cloud egress still routes to
`LOCAL_ONLY_OR_STOP`, and nothing in this crate transmits anything: every
proposal, conflict, span, permission, transmission and correction in its tests is
synthetic and built in process, and the official-source fixtures are driven
through `academic-ingestion`'s own stages as imports. Its link closure holds
nothing that can open a socket and it spells no socket construct.

`P2-X7` opens and closes no section 38 gate.
