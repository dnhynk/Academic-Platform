# Deletion and retention product flow (`P2-P2`)

## Posture

Deleting a synthetic artifact is not ADR-002, ADR-004, ADR-005, or ADR-012
acceptance.

```text
adr_002_accepted=false
production_data_allowed=false
default storage_encryption=NONE
```

`academic-deletion` is a workspace crate no product binary links, and no crate
declares an edge to it: `deletion_lane_is_not_default` holds the dependent set
at empty, because `P2-Z1` is what will drive it. It persists nothing — there is
no `academic-store` edge, this task claims **no migration number**, and the
provider deletion receipt it links is the row `academic-policy` already
persists.

## What `P2-K5` left and what this is

`P2-K5` built the mechanism and named the seams: a plan that enumerates seven
derivative classes, a four-word result vocabulary, an append-only journal, a
keyless backup tombstone, a `DerivativeResolver` and a `RetentionExecutor`, each
with "`P2-P2` supplies the real implementation" beside it. This crate is those
implementations and the product flow around them.

| step | type | what it refuses |
|---|---|---|
| dry run | `DeletionDryRun` | nothing; it carries one node per class, in registry order, always |
| protection | `ProtectionDecision` | a refusal with no policy reason — the refusing arm carries one and there is no boolean form |
| impact preview | `DeletionImpactPreview` | a citation map that does not cover every artifact the dry run reaches |
| confirmation | `DeletionConfirmation` | an automatic actor, and a digest that is not the preview's |
| execution | `execute_deletion` | a plan that is no longer the dry run it came from |
| provider | `ProviderErasureLog` | a receipt for a request nobody made |
| leakage | `ExternalLeakIncident` | a close that no recovery step backs |

## A locator is not an identity, and this is the layer that had to fix it

The fifth `P2-A1` audit found `P1-G1`: deleting two artifacts that held the same
bytes in one domain made the second tombstone replace the first, made a restore
republish the artifact deleted first as readable, and made the receipt report it
as a copy the deletion had deliberately spared. `P2-K5` closed the tombstone
record and the tombstone file name and left the rest open as item **`P3-G10`** of
[the rotation contract](rotation-and-retention.md) — "adding the artifact to
these records is a journal format change and is left for whoever writes the
executor."

This crate is whoever writes the executor.

- **`DeletionTarget` is the pair**: an `ArtifactId` and a 32-byte locator, with
  no constructor that takes one without the other. Every map here is keyed by
  it — the citation map, the path map, the descriptor map, the provider log — so
  two registrations of one document are two entries and never one.
- **The unresolved list names artifacts.** `UnresolvedTarget` carries the pair
  and renders through `P2-K5`'s own `UnresolvedLocator::to_row`, so there is one
  spelling of the four reason words and not two.
- **The executor is bound positionally, not by lookup.** `DeletionPlan::build`
  asks the dry run for each class in registry order and `ClassResolution::Locators`
  keeps the order the index answered in, so the actions `settle` walks *are* the
  targets the dry run holds, in the same order and with the same multiplicity.
  `TargetAdapter` walks both at once and compares each action's class and
  locator against the target it is about to run; a mismatch is
  `DeletionFlowError::PlanDrifted` rather than a failure attributed to the wrong
  artifact. Nothing here looks a descriptor up by locator, which is the shape
  `P2-K5`'s own `ShreddingExecutor` fixture still has.
- **The receipt is checked against the journal.** `execute_deletion` compares its
  own unresolved list with the one `settle` wrote — length, class and reason, row
  by row — and returns `PlanDrifted` if they disagree, so the list a user reads
  and the list the journal holds cannot become two lists.

## The seven classes are section 32.10's, read out of it

Section 32.10's first bullet is the whole list:

```text
artifact 삭제 요청은 파생 transcript, embedding, graph claim, PDF, cache,
sync replica, backup expiry까지 dependency plan을 보여준다.
```

`SPEC_DERIVATIVE_WORDS` pairs each `DerivativeClass` variant with the phrase that
bullet uses for it, and `dry_run_enumerates_every_derivative_class` parses the
bullet out of the design document, splits it, and compares the two sets **in both
directions and in order**. The count is taken from the parse rather than
asserted, so "seven" is what the document has rather than what the test
remembers. A class with no phrase fails, a phrase with no class fails, and a
document that stops saying it fails.

**Two of the seven are spelled differently in three places, and that is
recorded rather than normalised away.** Section 32.10 writes `PDF` where t068
section 5 and `P2-K5` write `document`, and `sync replica` where they write
`replica`. The enum spelling is `DOCUMENT` and `REPLICA`; a reader who greps
section 32.10 for either finds nothing, and this is why.

The other half of that row is not a list at all. The test drives **every**
assignment of the three `ClassTargets` shapes to the seven classes — 3⁷ = 2187
resolvers — and requires the node list to be the registry, in registry order, in
all of them, with exactly the `Unresolved` classes it was given. A build that
dropped an empty class, or that reordered on one resolver answer and not
another, fails on the case that reaches it.

## The impact preview is `P2-L5`'s walk, called

Section 32.5: *어느 하나의 삭제가 concept/evidence projection에 미치는 영향을
미리 보여준다.* `P2-L5` answered that for a lecture expiry over `P2-G6`'s own
preview. An artifact deletion asks the same question about a subject that is not
always a lecture and so has no consent expiry beneath it, so the projection half
of that walk was made public — `affected_projections` and `unreferenced_objects`
— and this crate calls it. A second walk over one index would be two answers to
one question.

**The preview is total, and the citation map is where it could stop being.**
Every artifact the dry run reaches is either cited by a listed projection or
listed as unreferenced, never both and never neither. That partition is only
meaningful if the preview knows the evidence key of every reached artifact, so a
short citation map is `DeletionFlowError::EvidenceCitationMissing` naming the
artifact — a refusal, not a shorter list.

## Preview precedes confirmation, as an absent function

`DeletionConfirmation::given` takes a `DeletionImpactPreview` **by value** and is
the only constructor; the confirmation owns the preview, so a caller cannot run a
different plan under a matching digest. It also compares the digest the surface
displayed against the preview's own, so a surface that rendered one preview and
submitted another fails there instead of deleting the second.

## The confirmation is non-delegable

`P2-M2`'s `UserDecision::by` takes an `Actor` and matches exhaustively over
`academic-domain`'s closed enum, issuing a receipt only for `Actor::User`. A
fifth actor variant stops `academic-proposal` compiling until it says which side
it is on, so the refusal is not a negated list a new variant slips past.
`P2-M4` — the task that forces non-delegable actions generally — has not merged;
this crate closes its own case with that door plus a binding to one exact preview
rather than waiting for it, and `tests/compile_fail` holds the struct literal
that would go round both. **Whoever lands `P2-M4` should read
`DeletionConfirmation::given` as one of the sites it has to cover.**

## A protected artifact returns a policy reason

`ProtectionDecision` has two arms and the refusing one carries a
`ProtectionReason`; there is no boolean, no `Default`, and no arm meaning
"refused, reason unavailable". A reason names one arm of a closed
`ProtectionPolicyKind`, the section of the specification that arm rests on, and
the registry's own words, and `to_row` always renders the policy and the section.

| policy | section | the sentence it rests on |
|---|---|---|
| `ORIGINAL_IS_PRESERVED` | 34.6 | 원본 artifact와 기존 Claim은 보존한다 |
| `INSTRUCTOR_CONDITION_FORBIDS_IT` | 32.5 | 허가 조건을 artifact policy로 상속 |
| `RETENTION_FLOOR_NOT_REACHED` | 32.5 | 각 derivative는 부모의 허가 조건과 더 엄격한 만료일을 상속하고 |
| `QUARANTINED_BY_OPEN_INCIDENT` | 34.4 | artifact quarantine |

Each sentence is checked against the design document. A protected subject's dry
run is still walked and still enumerates every class: a user told "this cannot be
deleted" and shown nothing has been told less than the previous screen showed.

## Provider erasure: stored, and linked to the deletion that caused it

Three layers already existed. `academic-policy` persists the receipt row and
links it to the grant and the exact allow-audit row of the transmission it
deletes; `academic-evidence-center` carries those columns as a
`DeletionReceiptRef` beside the transmission; and `EG07` makes "this provider
offers no receipt" a state rather than an absence. What was missing is the link
to the **artifact deletion that caused the request**, and that is this crate's.

`ProviderErasureLog` is keyed by `(DeletionTarget, EgressDecisionId)`, so one
artifact sent to two providers is two entries and two registrations of the same
bytes sent to one provider are two as well. `record_receipt` refuses a receipt
for a pair no request names.

**An outstanding provider copy is not a fifth result word.** `P2-K5`'s vocabulary
is `PLANNED`, `COMPLETE`, `PARTIAL`, `REPAIR_REQUIRED` over the seven *local*
derivative classes, and a provider copy is not one of them. What an outstanding
erasure does is stop `ArtifactDeletionReceipt::is_fully_erased` from being true
and appear, named exactly, in `outstanding()`. A local `COMPLETE` beside an
unsettled provider copy is an honest pair of facts; collapsing them into one word
would lose one of them. A provider that offers no receipt (`EG07`) therefore
keeps `is_fully_erased` false forever, which is the truth: this build cannot
observe that copy being erased.

## External leakage is a security incident, not a correction

Section 34.6 lists five recovery principles. The first four are the ordinary
correction path. The fifth is a different sentence about a different kind of
event:

```text
5. 외부 유출은 일반 correction이 아니라 security incident lifecycle로 처리한다.
```

A leak is not wrong information a better claim replaces. The bytes left the
device; superseding the claim that described them changes what the graph says and
changes nothing about where they are.

`ExternalLeakIncident` advances only by recording the four `RecoveryStep`s section
34.4's leak row names — `token revoke/rotate`, `provider deletion request`,
`artifact quarantine`, `incident log와 범위 조사` — parsed out of that row and
compared in both directions. `close` returns an `IncidentClosure` only when all
four are present; `IncidentClosure` has private fields, no `Default`, and one
producer. `LeakIncidentState` has three arms and neither of them is `Superseded`
or `Corrected`.

`record_claim_correction` exists and deliberately advances nothing: the first
four principles still apply to the claim, and recording that they happened is how
an operator sees that the claim was handled *and* the incident is still open.
Three guards hold it, at three different strengths:

1. `leak_incident_cannot_be_closed_by_claim_supersession` drives all three
   `CorrectionOutcome` arms through `P2-X7`'s own conflict board — including
   `Modify`, which *is* supersession — and requires the state to be unchanged and
   `close` to still refuse after each. It then records the four steps one at a
   time and requires `close` to keep refusing until the last, so the guard is not
   passing because `close` refuses unconditionally.
2. `the_public_signature_set_is_this` requires no public signature anywhere in
   the crate to mention a correction type *and* an incident state or closure.
3. `the_impl_blocks_naming_the_gate_types_are_these` pins the whole `impl` header
   set for the four gate types, so a `From<CorrectionRecord> for IncidentClosure`
   fails even though it adds no `pub fn` and spells no forbidden name — the class
   `P2-Y3` measured escaping every public-function sweep.

## Fault coverage

Executed, not injected. Three of the four are real failures at the real write
boundary and need no failpoint at all.

| Fault | Outcome proved | How it is produced | Where |
| --- | --- | --- | --- |
| `RB01` kill during crypto-shred | intact before the write; shredded after, with only the key slot moved; the journal's `ArtifactShredded` row inside the action | a child process killed at `academic-vault`'s own `RB01` failpoint, driving **this crate's product flow** over a real sealed object | `rb01_a_kill_during_the_product_shred_leaves_shredded_or_intact` |
| `RB02` backup tombstone write fails | `REPAIR_REQUIRED`, not `PARTIAL`; no partial tombstone and no tombstone directory | the backup root is a regular file, so `write_into_backup`'s `create_dir_all` returns the host's own error | `rb02_a_failed_tombstone_write_is_repair_required_and_leaves_nothing` |
| `RB03` derivative not found while planning | the deletion does not run at all — a cache file that would have been purged is still there — and the node is named with its class and the index's own words | the index answers `Unresolved` for a class | `rb03_an_unresolved_class_refuses_the_deletion_and_names_the_node` |
| `RB04` replica or cache purge partial | `PARTIAL` naming the exact artifact still there, and **not** the one removed, where the two share a locator | the failing path is a non-empty directory, so `remove_file` returns the host's own error on both hosts | `rb04_a_partial_purge_reports_the_exact_remaining_artifacts` |

`RB01` needs the `deletion-engine` and `phase2-fault-injection` features and runs
in the `rust-features-*` job; the other three are default-lane and run inside
`cargo test --workspace`.

## The seam this task changed in `P2-K5`

`RetentionExecutor::execute` now takes the journal:

```rust
fn execute(
    &mut self,
    journal: &mut AppendOnlyJournal,
    action: &PlannedAction,
) -> Result<(), ExecutionFailure>;
```

The real executor found the reason. `shred_with_tombstone` appends
`ArtifactShredded` after the slot is destroyed, and that record has to land
between the action it describes and the `RetentionSettled` that closes the run.
With the journal borrowed for the whole of `settle`, the only place a shred could
be recorded was *after* the settlement — which leaves a kill window in which a
settled action has no record of what it destroyed. `settle` now passes the
journal through, `rb01_a_kill_during_the_product_shred_leaves_shredded_or_intact`
reads the three record kinds back in order, and the three fixtures in
`academic-retention`'s own suites take the argument and ignore it.

## Running it

```powershell
cargo clippy -p academic-deletion --all-targets --locked --offline -- -D warnings
cargo test -p academic-deletion --all-targets --locked --offline
cargo clippy -p academic-deletion --all-targets --locked --offline --features deletion-engine,phase2-fault-injection -- -D warnings
cargo test -p academic-deletion --all-targets --locked --offline --features deletion-engine,phase2-fault-injection
```

The first two are inside `cargo clippy --workspace` and `cargo test --workspace`
and run on every hosted Rust label. The second two are the
`deletion-engine` lane and are two hosted `rust-features-*` steps, because the
key-slot write, the replica purge and the tombstone rename are per-platform.

## What this is not evidence for

**No window opens.** This crate is the content behind a deletion surface, not the
surface. It is a set of typed records and the rules that hold between them, and
nothing here observes a rendered pixel.

**Nothing persists.** No `academic-store` edge, no migration, no canonical table.
Every value in every test is synthetic and built in process.

**No key is destroyed in the default lane.** The crypto-shred is
`academic-vault`'s positioned write, behind `deletion-engine`, which selects
`academic-retention`'s own non-default object lane.
`deletion_lane_is_not_default` proves the default graph resolves neither the
object namespace nor an AEAD through this crate — and that it does still resolve
`academic-crypto`, deliberately, because the journal names key generations.

**A real deletion has never run.** Every object these suites shred is one they
sealed themselves, in a temporary profile, from synthetic bytes.
