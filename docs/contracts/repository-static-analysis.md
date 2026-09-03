# Repository static analysis and evidence tiers

`academic-repository-analysis` is the `P2-R2` boundary. It is section 17.3's
third stage — `AST, symbols, call/data flow, schema, config, IaC` — and section
17.3's own tier table over the result.

It opens no file and no socket. The bytes it analyzes arrive as an argument,
read by whoever ran `P2-R1`'s gate; what it produces is spans, digests and
closed vocabularies.

That is held by three whole-set comparisons over the product code rather than by
a list of forbidden spellings: every `use` item, every two-segment path spelled
through a crate root, and every macro invoked, each compared against a pinned
inventory in both directions. The three together cover the ways a capability can
be reached — through an import, through an absolute path, and through a macro —
and each was injected separately. A token list alone did not: `std::path::Path::
new(p).metadata()`, `include_str!` and `std::env::var` each passed an earlier
version of the guard, and `docs/contracts/policy-source-scans.md` records that
measurement.

## The analysis is bound to the snapshot it names

`AnalysisInput::of` is the one constructor and it checks four things. Each is a
way an analysis could otherwise be about something other than the snapshot it
claims:

| Check | What it refuses |
|---|---|
| the snapshot's `toolVersions` records this analyzer at this version | a confidence calibrated for one analyzer build shown for another |
| every unit's path is a manifest row | a path the gate excluded reaching a reader |
| every unit's bytes hash to that row's `blobHash` | different bytes for an admitted path |
| every unit's byte digest is sealed in `P2-G5`'s `SourceIndex` | bytes that were never ingested as untrusted content |

`the_analysis_reads_only_what_the_snapshot_froze` observes the first three
refusing against a control that is accepted.

A manifest row with no unit offered is not skipped: it gets a coverage row whose
every outcome is `Gap(BytesNotIngested)`. That is the case `academic-repository`
produces for a file it manifests by digest and does not ingest.

## Five observations, three tier values

Section 17.3's table has five rows this task owns and one column headed
`OBSERVED 가능 여부`; `REQ-34-081` names three values a reader is shown. Five
onto three is a lossy fold, so *which row becomes which value* is the contract:

| Section 17.3 observation | That column says | `LadderRung` | Tier | Carries |
|---|---|---|---|---|
| `manifest에 dependency만 있음` | `불가` | `MANIFEST_PRESENCE` | `PRESENT_ONLY` | no confidence |
| `import만 있고 reachable use 없음` | `보류` | `UNREACHABLE_IMPORT` | `POSSIBLE` | no confidence |
| `reachable call + config 존재` | `가능, confidence 표시` | `REACHABLE_CALL_WITH_CONFIG` | `OBSERVED` | a calibrated confidence |
| `test에서만 사용` | `scope를 제한해 가능` | `TEST_SCOPED_USE` | `OBSERVED` | scope `TEST` |
| `runtime trace/production config와 일치` | `가능` | `RUNTIME_AND_PRODUCTION_CONFIG` | `OBSERVED` | strength `STRONG` |

The sixth row of that table — `사용자 직접 구현·debugging 확인` — is not here. It
is section 17.6's `User APPLIED Concept` rather than `ProjectSnapshot OBSERVES
Concept`, and `P2-R5` owns it.

**Rows three, four and five are all `OBSERVED`.** What separates them is what
else the finding carries: row four narrows `ArtifactScope` to `TEST`, row five
raises `EvidenceStrength` to `STRONG`. Neither is a higher tier, and reading row
four as one is the over-claim `test_only_use_is_test_scoped` exists against.

`LadderRung::tier` is the fold, as one total function, pinned as whole text.
`the_tier_vocabulary_is_three_values_and_the_ladder_is_five_rungs` compares the
table above against it row for row *and* requires each of the five rungs to be
produced by one of the acceptance corpora, so the fold is exercised rather than
only declared.

### The rungs are tried downwards, and the order is load-bearing

Row five, then four, then three, then two, then one. Trying three before four
classifies a subject used only in a test harness — a reachable call plus a test
configuration — as a production observation, which is section 34.4's `test 도구를
운영 사용으로 오인`. That reordering was injected and observed failing
`test_only_use_is_test_scoped` and two tests beside it.

### One step up needs its own ingredient

`each_promotion_needs_its_own_ingredient` injects, for each step, a change that
looks like the missing ingredient and is not:

- an import in a vendored tree does not lift row one to row two;
- configuration does not lift row two to row three while the call stays
  unreachable;
- a production configuration *does* take a corpus out of row four, and removing
  it puts it back — row four is `used nowhere but tests`, not `used in tests`;
- a trace naming another snapshot, and a trace naming another subject, each
  leave a corpus at row three.

## Confidence is `P2-M1`'s or it is not shown

Section 17.3's third row is `가능, confidence 표시`. `P2-M1`'s contract is that
`CalibrationRegistry::interpret` is the only producer of a `CalibratedConfidence`
and `DisplayedConfidence::of` takes one.

So the private tier value's `Observed` arm *holds* a `DisplayedConfidence`. An
observed finding without a calibrated number has no representation, and a rung
that would be observed without a fresh dataset is **refused** rather than shown
with a bare score or quietly demoted. `reachable_call_plus_config_is_observed_
with_confidence` observes both refusals — no dataset, and a dataset whose
refresh interval has elapsed.

The analyzer is the `ProviderId` and its version the `ModelVersion`, which is
why `AnalysisInput::of` requires the snapshot's `toolVersions` to name the pair:
a dataset is registered for an exact analyzer build, and section 17.5's
`ANALYSIS_CHANGED` lane needs that binding to tell an analyzer change from a
code change later.

The raw unit is *how many independent kinds of evidence corroborate*, on a scale
of five — a manifest entry, an import, a reachable call, a configuration site, a
runtime trace — times 200. It is a count and not a weighting, because a
weighting would be a number this task invented and then displayed.

## A finding is never repository-wide

Section 34.4's prevention column for over-generalised snippets is *finding
scope를 symbol/component로 시작*; `REQ-34-091` states it as *a new finding cannot
default to repository-wide scope*. Four things hold it, and they are different
claims:

**No variant.** `FindingScope` has `Symbol` and `Component` and nothing else.
`finding_scope_cannot_name_the_repository` fails to compile, with a committed
diagnostic, on `FindingScope::Repository` and on a wildcard variant.

**No root component.** `ComponentId::new` refuses `""`, `"."`, `"/"` and `"./"`
with `ComponentError::RepositoryRoot`, and `ComponentId::containing` gives a
root-level file *itself* as its component rather than widening to `.`.
The acceptance test asserts the exact error variant rather than `is_err()`: the
malformed-path rule happens to reject all four spellings too, so an `is_err()`
assertion passed with the root branch deleted. That was measured by injection,
and asserting the reason is what makes the branch load-bearing.

**No constructor.** `Finding` has private fields, no `Default`, and one
crate-private constructor whose call sites are counted at one, in the ladder,
over every product file of the package. The constructor is `pub(crate)`, so an
integration test cannot reach it either.
`finding_cannot_be_assembled_field_by_field` and
`component_id_cannot_skip_its_constructor` are the compiled half.

**No widening.** The ladder emits one finding per component. Evidence in three
components is three findings, which
`new_finding_cannot_default_to_repository_scope` observes — because once the two
obvious spellings are closed, one wide finding over three components is what a
repository-wide default looks like next.

Each finding carries `REQ-34-093`'s coverage: how many components hold evidence
for the subject, over how many components this run read at all. The denominator
is components the analyzer had a reader for, not components in the tree.

## Two axes over a path, and they disagree

Section 34.4's first row names vendored, example, generated and monorepo code as
sources of stack false positives, and its prevention column is `generated/vendor/
test 분리`. That is two independent classifications, not one:

- **`PathClass`** — `FIRST_PARTY`, `VENDORED`, `GENERATED`, `EXAMPLE` — answers
  *may evidence here raise a tier*. Exactly one of the four does.
- **`ArtifactScope`** — section 18.1's `PRODUCTION`, `TEST`, `BUILD`,
  `MIGRATION`, `DEVELOPMENT` — answers *what kind of use this would be*. There is
  no unscoped value, which is `REQ-18-003`'s second half.

They are separate because they disagree: `tests/cache.test.ts` is first-party
code that promotes, at test scope, and `vendor/x/src/main.rs` is
production-shaped code that promotes nothing. Collapsing them would force one of
those two answers to be wrong.

`EXAMPLE` covers `examples/`, `benches/` and `probes/` together, for `S-12`'s
reason: those three are compiled by `cargo clippy --workspace --all-targets` and
look exactly like product code to a walk that names only the first.

A path with no rule is `FIRST_PARTY` and `PRODUCTION`. That direction is
deliberate — an unclassified path treated as non-promoting would drop evidence,
and the drop would look like an absence of use.

**`target/` never reaches this crate at all.** `P2-R1`'s point-1 file policy
holds `target` in its secret-file segments, so the gate removes the subtree
before the inventory opens anything.
`vendored_generated_example_paths_do_not_promote` asserts that the manifest
holds no `target/` path, so the two layers do not each assume the other covers
it; the generated-path case is exercised on `dist/` and `generated/` instead.

### The monorepo axis is package attribution

`PackageMap` is built from the frozen manifest: a package is a directory holding
`Cargo.toml`, `package.json`, `pyproject.toml` or `go.mod`. A site in one
package does not corroborate a finding in another, and is kept on the finding as
an `ExcludedSite` with `OTHER_PACKAGE` rather than dropped.

A site in a non-promoting path is kept the same way, with `NON_PROMOTING_PATH`.
A reader told `PRESENT_ONLY` who can see the vendored copy needs to be told the
analyzer saw it too.

### What is package-level and what is component-level

An import and a call are about the file they sit in, so they name a component. A
manifest entry and a configuration key are about the package: a manifest
installs a dependency for every module beside it, and a configuration file
configures the program rather than the directory it happens to sit in. Grouping
configuration by its own directory would mean section 17.3's third row could
only be satisfied by a configuration file that sat next to the call.

A dependency entry is manifest presence and never configuration. Emitting it as
both would let section 17.3's first row corroborate itself, and a manifest-only
fixture would classify as a use of what it merely installs.

## The coverage report is total

`REQ-17-011`'s acceptance is *each listed index kind emits typed locator;
unsupported kind explicitly reports coverage gap*. The failure it is written
against is a silent skip: an analyzer that returns nothing for a file it did not
understand is indistinguishable from one that understood the file and found
nothing.

So every manifest path gets one `PathCoverage`, and every `PathCoverage` holds
`[CoverageOutcome; IndexKind::COUNT]` — a fixed-size array built by mapping over
`IndexKind::ALL`. There is no partially-filled coverage value to skip a file
into, and an added index kind fails to compile until every construction site
answers for it.

Three outcomes, and the difference between the last two is the point:

- `Analyzed(n)` — a reader ran; `n` may be zero.
- `NotApplicable` — the question is not about this file kind. A Rust source file
  has no infrastructure-as-code facts, and reporting a gap there would make the
  gap list noise rather than a list of what the analyzer cannot do.
- `Gap(reason)` — `UNSUPPORTED_LANGUAGE`, or `BYTES_NOT_INGESTED`.

`support` is the total function that separates them, over `FileKind` ×
`IndexKind` with no default arm.
`unsupported_language_reports_a_coverage_gap` walks all 98 cells and requires
`Unsupported` to occur for exactly one file kind, checks a Go file is a gap for
all seven index kinds, and checks a supported file and a container file each
report none — so the assertion is about the language rather than about
everything being a gap.

The thirteen file kinds with a reader are Rust, TypeScript/JavaScript, Python
and SQL source; `Cargo.toml`, `package.json`, `pyproject.toml`/`requirements.txt`
and four lock formats; TOML/YAML/JSON documents; `Dockerfile`/`Containerfile`,
compose files and `.github/workflows/*.yml`; and Markdown/text prose. Everything
else is `Unsupported`, and `RepositoryAnalysis::supported_file_kinds` derives the
list from the matrix rather than restating it.

## Nothing analyzed reaches a reader as text

`P2-G5`'s `Untrusted::seal` is private to that crate, so this crate cannot label
a value — and therefore must not hold one that needs a label. What it holds
instead is digests, spans and closed vocabularies. A declaration is a
`SymbolFingerprint`, which is section 17.4's own word: *blob hash, symbol
fingerprint, syntax span과 commit을 함께 저장하고*. A dependency name, an import
specifier or a configuration key read out of a file is compared against a needle
the caller supplied and then dropped; what a finding carries is the caller's
`SubjectId`. Matching is *untrusted text selects from a trusted set*, and the
trusted half is what survives.

**This is the half of that boundary that lives one step outside
`no_public_signature_hands_out_ingested_text`.** That scan refuses a `pub fn`
that *takes* an `Untrusted<…>` and returns `str`, `String` or `u8`. This crate
takes no `Untrusted<…>` at all, so nothing there covers its surface, and a
symbol name handed back as a `&str` would be the same leak by another route.
Two things close it:

- `no_public_accessor_hands_out_analyzed_text` compares the whole set of this
  crate's public functions whose return type names text against a 14-entry
  inventory, in both directions, each entry carrying one of four reasons: a
  fixed spelling of a closed vocabulary, a path, a caller-supplied identifier,
  or a system-derived identifier.
- `no_analyzed_byte_reaches_a_text_accessor` runs the analyzer over a corpus
  whose every identifier, dependency name and configuration key is a canary and
  whose paths hold none, and requires the canary in no accessor's output **and
  in no `Debug` output**. `AnalyzedFile` hand-writes `Debug` for that reason: an
  accessor is not the only way a `String` reaches a log, and a derived one would
  print every symbol name the analyzer read through the derived `Debug` of every
  public value that holds one.

  It renders the *input* values too — `SourceUnit`, which is the only public
  type here that holds the analyzed bytes, and the `AnalysisInput` that carries
  a vector of them. Walking only the analysis outputs left the one value
  carrying the payload unobserved, and a hand-written `Debug` printing those
  bytes was measured passing this test before it was widened.
  `tools/secret-debug-policy.test.mjs` refuses both shapes — a derived `Debug`
  over a field named `source_bytes`, and a hand-written one that prints it —
  and both refusals were observed by injection; that is another crate's net, and
  a crate whose whole subject is untrusted repository bytes should fail in its
  own suite as well.

A path *is* text and stays text. `academic-repository`'s own manifest already
hands paths out, and the gate classified every one of them before anything
opened a file.

## What this contract does not claim

- **The readers are not a language front end.** They are hand-written, in the
  same shape as this repository's `.gitignore` parser and its data-record
  escaper, and no parser generator is admitted — the receipt's
  `no_parser_dependency_note` records why. What they do not model is reported as
  a typed coverage gap rather than left implicit.
- **Reachability is name-based and over-approximates.** A call whose leaf
  identifier equals a declaration's name reaches that declaration, so two
  functions with one name in two files are both marked reachable. That direction
  is chosen: under-approximating would move a live call to section 17.3's second
  row and report a use as merely possible.
- **"Data flow" here is a def-use chain.** A module-level binding and later
  mentions of its name in the same file. It is not an interprocedural analysis,
  and `IndexKind::DataFlow` counts exactly those edges.
- **This crate persists nothing.** It adds no migration and no table. Section
  17.4's finding is a value here; `P2-R4` owns classification proof schemas and
  locator migration, and `P2-R3` owns cross-artifact correlation.
- **A runtime trace is an argument.** This crate runs no program and reads no
  trace file. What it decides is whether a trace *agrees* — a trace naming
  another snapshot or another subject is not evidence about this one.
- **Nothing here is ADR-002 acceptance.** The default lane remains
  `storage_encryption=NONE`, `production_data_allowed=false`,
  `adr_002_accepted=false`.
- **No real repository was analyzed.** Every corpus is synthetic and built in
  process, captured through `P2-R1`'s own `capture_local`. No network call is
  made and none can be: the crate spells no transport construct and its link
  closure holds nothing that can open a socket.
- **`§38` is neither opened nor closed by this task.**
