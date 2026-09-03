# Cross-artifact correlation and drift lanes

`academic-repository-correlation` is the `P2-R3` boundary. It is section 17.3's
fourth stage — `cross-artifact correlation, spec ↔ ADR ↔ code ↔ config ↔ test ↔
runtime/incident` — section 17.5's typed relations and `ImplementationDrift`,
and section 19's two diff channels.

It sits directly on `P2-R2`. A relation about code comes from a
`academic_repository_analysis::Finding` and from nothing else; there is no
second reader, no second tier vocabulary, and no path from repository bytes to a
relation that does not pass through the evidence ladder. It opens no file and no
socket, and it never holds an analyzed byte: every artifact that is not code
arrives as an argument naming `SubjectId`s, the way `P2-R2`'s runtime trace
does.

## The relation vocabulary is section 17.5's, compared and not counted

Section 17.5 writes the relations as a bullet list, and this is that list:

| Section 17.5 | Meaning, quoted | Lane |
|---|---|---|
| `PROJECT_SPEC_MENTIONS` | `규범적 의도` | intent |
| `PROJECT_CODE_USES` | `실제 코드 구조에서 관찰` | implementation |
| `PROJECT_ARCHITECTURE_REQUIRES` | `architecture constraint로 필요` | intent |
| `PROJECT_TEST_EXERCISES` | `test가 동작/failure를 검증` | implementation |
| `PROJECT_CONFIG_ENABLES` | `실행 구성에서 활성화` | implementation |
| `PROJECT_INCIDENT_EXPOSED` | `incident가 failure mode를 드러냄` | implementation |
| `PROJECT_DOC_EXPLAINS` | `문서가 현재 동작을 설명` | neither |

The number of rows is not asserted anywhere in the code or in this page.
`seven_relation_types_are_distinct` reads the bullets back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares them against
`EvidenceRelation::ALL` in both directions, so the vocabulary is a measurement
of the design document. A relation renamed on either side is a failure, and that
was injected: renaming `PROJECT_DOC_EXPLAINS` to `PROJECT_DOCUMENT_EXPLAINS`
fails on `section 17.5's relation list and this enumeration disagree`.

The same test also requires each relation to be **produced** by a corpus in the
suite, and removes each producer in turn. A vocabulary nothing emits is a
vocabulary, not a contract.

### Two subjects, because one cannot carry all seven

`P2-R2`'s fourth row is *used **nowhere but** tests*, so a subject with a
production use is never test-scoped. The all-seven corpus therefore uses two
subjects: one used in production with a configuration a runtime trace agrees
with, and one used only in the test tree.

## Which lane a relation answers for, and why one answers neither

Section 30.3's table has six rows and this task owns two, quoted whole:

| Claim 종류 | active view 우선순위 | 충돌 처리 |
|---|---|---|
| 현재 구현 | 같은 snapshot의 runtime/config/code direct evidence > user clarification > AI | spec은 intent lane에 보존 |
| project intent | 승인된 최신 spec/ADR > user clarification > AI | code와 drift 생성 |

Row four's authority list is `runtime/config/code`, so the four relations
carrying a runtime, a configuration or a code observation answer it. Row five's
is `spec/ADR`, so the two carrying one of those answer that one.

`PROJECT_DOC_EXPLAINS` answers **neither**, and that is a reading of section
17.5 rather than an omission. It is defined as `문서가 현재 동작을 설명` — a
description. A description approves nothing, so it is not row five's authority,
and it makes nothing run, so it is not row four's. Its **absence** beside a
`PROJECT_CODE_USES` is what section 17.5's second diagram turns into
`IMPLEMENTED_NOT_DOCUMENTED`; putting it in the implementation lane would make
that absence weaken the implementation claim, which is the opposite of what the
diagram says. `AuthorityLane::Description` has no section 30.3 row and
`active_view` refuses it rather than inventing a precedence.

## This crate adds no rank and no ordering

Section 30.3's six rows are already implemented. `academic_ledger::
ProductClaimType` is that table, `CurrentImplementation` is row four,
`ProjectIntent` is row five, and `AuthorityTable::rank` is the comparison. Full
decision replay stays in `academic_ledger::resolve_product_snapshot`.

What this crate adds is the half a rank table cannot hold: the **qualifiers** on
the two rows' authority lists.

| Row | Qualifier | Held by |
|---|---|---|
| 현재 구현 | `같은 snapshot` | direct evidence naming another snapshot is admitted at `Unknown` |
| project intent | `승인된` | a draft or deprecated document is admitted at `Unknown` |
| project intent | `최신` | an approved document below the highest approved revision is admitted at `Unknown` |

`Unknown` is rank zero in both tables, which the table already places below a
user clarification. Nothing is dropped: an inadmissible candidate is still
listed, at rank zero, because dropping it would be the deletion this task's
whole subject is against.

The lanes do not lend each other authority either. A specification is admitted
at `Unknown` for the implementation question — row four's conflict column is
`spec은 intent lane에 보존` — and a direct code observation is admitted at
`Unknown` for the intent question, because row five's is `code와 drift 생성`.
Both were injected: removing the same-snapshot comparison, and removing the
approved-and-latest check, each fail their named test.

## Neither side of a conflict is overwritten

Section 17.5: *둘은 같은 질문의 경쟁 답이 아니므로 한쪽으로 덮지 않고
`ImplementationDrift`를 만든다*. `CONTRIBUTING.md` rule 2 says the same thing
about this repository's canonical records — append-only, and *a correction is a
new event*.

So a drift is a record beside the edges and never a replacement for one. It
carries all three lanes' edges as they are, and `Correlation::lane_view` filters
on `EvidenceRelation::lane` alone, so the presence of a drift cannot change what
a lane returns. `conflict_creates_drift_without_overwrite` compares each lane's
view against the same corpus with the other side removed, requires the two to be
identical, and requires the three lane views to account for the whole edge set.
Correlating again with one more document appends: every edge of the earlier run
survives in the later one, and the correction resolves the drift by adding a
description rather than by deleting a side.

The overwrite was injected — dropping intent edges for any subject with a
`PROJECT_CODE_USES` — and fails on `the intent lane was rewritten by the
implementation lane`.

### What produces each drift, exactly

* `INTENDED_NOT_IMPLEMENTED`: an intent-lane relation exists and no
  `PROJECT_CODE_USES` does. Manifest presence does not suppress it — section
  17.3's first row is `불가` and section 18.1 spells the case out — and neither
  does an unreachable import, which section 17.3's second row calls `보류`. Both
  are asserted, and reading `POSSIBLE` as a use was injected and fails.
* `IMPLEMENTED_NOT_DOCUMENTED`: a `PROJECT_CODE_USES` exists and no
  `PROJECT_DOC_EXPLAINS` does. The relation section 17.5's diagram names as
  absent is `PROJECT_DOC_EXPLAINS` and **not** `PROJECT_SPEC_MENTIONS`: a
  specification is intent, not a description of behaviour, so it does not close
  this drift. Making it close the drift was injected and fails.

There is no third kind. `ANALYSIS_CHANGED` is section 19's word for a difference
between two runs, not for a disagreement inside one, and the comparison owns it.

## Four scopes, four different payloads

Section 17.5: `deprecated spec, feature flag, 미배포 code, branch 차이도 scope로
구분한다`. `DriftScopes` holds four `Option`s of four different types, not one
enumeration and not four booleans:

| Scope | Established by | Carries |
|---|---|---|
| `DEPRECATED_SPEC` | an intent document's own `ApprovalStatus::Deprecated` | which document, which revision |
| `FEATURE_FLAG` | a `FeatureFlagRecord` gating the subject | which flag, and whether it is on or off |
| `UNDEPLOYED_CODE` | no `DeploymentRecord` naming this snapshot | which target, and which snapshot it runs |
| `BRANCH_DIFFERENCE` | an intent document naming a branch this snapshot is not on | both branch names |

They can hold at once, so one enumeration would drop three of them; each is read
from a different argument, so no two can be established by the same evidence.
`deprecated_flagged_undeployed_branch_scopes_are_distinct` flips one input at a
time from a base with no scope at all, then all four at once, then removes each
of the four and requires exactly its own scope to clear. Collapsing the
undeployed scope onto *any deployment record exists* was injected and fails on
`the base corpus already carries a scope`.

`UNDEPLOYED_CODE` needs a deployment record to be absent *for this snapshot*
while some record exists. No deployment record at all is a question the input
does not answer rather than an absence of deployment.

## Section 19's two channels, and what a difference is attributed to

`diff는 단순 dependency diff와 semantic finding diff를 나누고`. Two accessors,
two types, and no accessor returning their union:

* `dependency_diff()` is over declared dependencies alone. A site counts when
  `P2-R2`'s own `FileKind::of_path` calls its path a manifest or a lock file, so
  the classification is the analyzer's rather than a second one written here.
* `semantic_diff()` is over what the correlation concluded: relations and drift.
  Section 18.1's `NO_LONGER_OBSERVED` is one of its transitions, because a
  subject that stops being observed is not one that was never used.

A dependency declared and never used therefore moves the first channel and not
the second; a use that declares nothing new moves the second and not the first.
Merging the two was injected and fails.

### The attribution table

| snapshot | analyzer | attribution |
|---|---|---|
| same | different | `ANALYSIS_CHANGED` |
| different | same | `CODE_CHANGED` |
| different | different | refused: `ConfoundedComparison` |
| same | same | refused: `NoComparisonAxis` |

Only `ANALYSIS_CHANGED` is section 19's own word. **`CODE_CHANGED` is this
contract's spelling for its complement**; section 19 names the contrast without
naming the second value.

Both refusals are deliberate. When both axes moved there is no attribution to
make, and reporting the difference anyway would put it in one of the two
buckets — which is the display section 19 forbids; the way out is to re-run the
older snapshot under the newer analyzer and compare one axis at a time. When
neither moved there is no axis at all, and a difference between two such runs
came from the arguments beside them.

The cause is on every entry as well as on the comparison, because section 19's
requirement is about what a reader sees on the row.

### How one snapshot is read by two analyzer builds

`same_bytes_two_analyzers_is_analysis_changed` freezes one snapshot whose
`toolVersions` names two analyzer builds and analyzes it twice. The older build
is not offered the configuration document; the newer one is. That is what *a
newer analyzer version has a reader the older one lacked* looks like from
outside, and it is `P2-R2`'s own state: a manifest row that is not offered
becomes `Gap(BytesNotIngested)` rather than an error. Both runs name the same
snapshot identifier, which the test asserts.

The test also runs the code-changed comparison, so an implementation that always
answered `ANALYSIS_CHANGED` fails. That was injected.

## What this crate may reach, hold and hand out

### The reach guard is three whole-set allowlists

`P2-R2` measured what a forbidden-token list misses: seven spellings of a
filesystem or environment reach — `std::path::Path::new(p).metadata()`, its
leading-`::` form, its whitespace-inside-the-path form, `std::env::var` and
`include_str!` among them — compile, spell none of the listed tokens, add no
`use` item, and passed. `docs/contracts/policy-source-scans.md` records it.

So the primary nets here are whole sets, compared in both directions: every
`use` item, every two-segment path reached through a crate root, and every macro
invoked. The token pass is kept as an explicit third and weakest layer. Four
bypasses spelling none of the forbidden tokens were injected separately and each
was observed failing: an absolute-path environment read, its leading-`::`
filesystem form, its whitespace-inside-the-path form, and `include_str!`.

### Every field is inventoried, by enumeration

`no_public_signature_hands_out_ingested_text` and `P2-R2`'s
`no_public_accessor_hands_out_analyzed_text` guard what a public function
*returns*. The complementary question — what a type *holds* — is answered
workspace-wide by `tools/secret-debug-policy.test.mjs`, and it answers it with a
**field-name alternation**: `source_bytes`, `payload`, `plaintext`, and a few
dozen more.

That is the same defect class as the token list, one step out. It was measured
here rather than assumed: a `#[derive(Debug)]` variant of this crate holding
`excerpt: Vec<u8>` — raw bytes, a name outside the alternation — passes that
scan. `node --test tools/secret-debug-policy.test.mjs` reports `pass 10, fail 0`
with that field present. Widening the alternation would be the repair that was
already rejected once; the repair is a whole-set inventory, and doing that
workspace-wide is a task of its own.

What this crate does is close its own half by the whole-set route:
`every_field_of_this_crate_is_in_the_inventory` extracts every named field of
every `struct` and `enum` the product code declares — enum struct-variants
included, reported as `Enum::Variant` — and compares them against a justified
inventory in both directions. Each entry says what the field holds, and there
are five things it may be: a caller-supplied identifier, a path the gate already
classified, a system-derived identifier, a closed-vocabulary value, or a value
of another reviewed crate. The same `excerpt: Vec<u8>` fails here immediately,
which is the difference between the two shapes of guard.

The extractor also requires this crate to declare no tuple struct, because a
tuple field has no name to inventory.

### No public function takes `&mut self`

`CONTRIBUTING.md` rule 2 is append-only and a correction is a new event.
`no_public_function_mutates_in_place` compares every `pub fn` signature of the
product code against that, so a correlation is corrected by producing a new
value rather than by editing one.

## What this contract does not claim

- **It is not a second authority resolver.** The ranks, the ordering, the
  decision replay, the scope and valid-time filtering and the conflict cards are
  `academic-ledger`'s. This crate decides which authority class a piece of
  correlation evidence may enter a section 30.3 table as, and reads the rank
  back out.
- **It does not classify.** `OBSERVED`, `REQUIRED` and `WOULD_BENEFIT_FROM`,
  their proof chains, `ClassificationConflict` and locator migration are
  `P2-R4`'s.
- **It does not decide competency.** Section 17.6's `User APPLIED Concept` is
  `P2-R5`'s.
- **A document is an argument.** This crate opens nothing and reads no prose. A
  specification, an architecture decision, a behaviour document and an incident
  record arrive naming `SubjectId`s and a manifest path, and the path is checked
  against the frozen manifest the way `P2-R2` checks an analyzed unit's.
- **It persists nothing.** No migration, no table, no schema. Section 19's
  comparison is computed from two correlation values.
- **Nothing here is ADR-002 acceptance.** The default lane remains
  `storage_encryption=NONE`, `production_data_allowed=false`,
  `adr_002_accepted=false`.
- **No real repository was correlated.** Every corpus is synthetic and built in
  process, captured through `P2-R1`'s `capture_local` and classified through
  `P2-R2`'s ladder. No network call is made and none can be.
- **`§38` is neither opened nor closed by this task.**
