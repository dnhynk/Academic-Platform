# Expected concepts, the minimum that is proposed, and three uncertainties

`academic-next-lecture` is the `P2-L6` boundary. It is section 12.7: the seven
places an `ExpectedConceptClaim` is extracted from, the candidate standing every
one of them keeps, the minimal likely-blocking foundations it proposes and the
breadth it does not, and the three-way uncertainty factorization the section's
last sentence requires.

It sits on five boundaries and restates none. `academic-untrusted-content`
already decided what an ingested byte may become. `academic-gap` already decided
what a gap is and which of section 15.2's five kinds is a claim the person is
missing something. `academic-lecture-document` already produced the lossless
document the seventh place is a node of. `academic-ingestion` already fixed what
a validated calendar date is. `academic-domain` already fixed `AI_INFERRED` and
the tier that carries no prerequisite of its own. This crate reads all five and
adds one thing: the proposal that some small foundation should be prepared before
tomorrow — which it makes as narrowly as section 12.7 says to.

It opens no file, opens no socket, reads no clock, persists nothing, adds no
migration, and has no edge to `academic-store`. It has no edge of any kind to
`academic-knowledge-state`, so no function here can produce the evidence a
mastery promotion is read from.

## An extraction is a candidate before it is anything

Section 12.7 extracts claims; section 27.4's low-risk row says an extracted topic
is stored `자동 저장하되 AI_INFERRED 표시`; section 27.2 says AI does not
`개념 이해·질문 해결을 사용자 대신 확정`.

Each rule is a value that does not exist rather than a check.

| Section 12.7 rule | What holds it |
|---|---|
| seven places, and no eighth | `ExpectedConceptSource::ALL`, compared with the document's own sentence in both directions, with the sentence's leftovers required to be separators |
| every extraction is a candidate | `ExpectedConceptClaim::STANDING` is an associated constant, `extract` takes no status, and no public signature here returns an `academic_knowledge_state` type |
| the material is text from outside | `extract` takes a `P2-G5` `Proposal`, the only value `adjudicate` produces; `Untrusted::expose` is `pub(crate)` there, so no product file here can read one ingested byte |
| a claim quotes the material it names | `extract` refuses a claim none of whose spans point into the declared document |
| the seventh place is a lecture this system kept | `MaterialReference::of` requires a `P2-L4` `NodeId` for it and refuses one for the other six |
| the proposal is the minimum | `minimality_defects` is three graph facts, and this crate holds no phrase to match against |
| the morning holds one to three | `PreparationBrief::assemble` takes the whole list and there is no `push` |
| the three uncertainties stay apart | three axis types with nothing in common, so there is no array to fold and no method that answers with one confidence |

`crates/next-lecture/tests/compile_fail/` holds the compiled half: four programs
that each fail to compile with a committed diagnostic.

## Seven and three are measurements

Section 12.7's first sentence is
`syllabus, 다음 title/slide, 교재 chapter, LMS 자료, 과제, 공지, 직전 강의 말미에서
`ExpectedConceptClaim`을 추출한다.` and its last is
`예상 concept, prerequisite edge, 사용자 state가 모두 불확실할 수 있으므로 각각의
근거와 confidence를 분리한다.`

`expected_concept_source_matrix` and `prep_uncertainty_factorization` each read
their sentence back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`,
split it on the document's own `, `, compare the cells with
`EXPECTED_CONCEPT_SOURCES` and `PREP_AXES` in order and as sets, and then remove
every matched cell and require what is left to be separators. Seven and three are
therefore measurements of the design document, and neither is a number this crate
chose.

**`다음 title/slide` is one place and not two, and the reading is executed.** The
sentence separates its places with `, `, and this item holds a `/` inside a single
comma-delimited cell, exactly as `synonym/granularity` does in section 15.2's
table. Reading the slash as a separator would make the count eight while the
document is punctuated for seven; the removal pass is what would then fail,
because `다음 title` and `slide` would each be left unmatched. If the design
document is ever repunctuated to separate them, that test fails rather than the
enumeration silently becoming wrong.

## The seven places and `P2-G5`'s six document kinds are different vocabularies

`ExpectedConceptSource` answers *which of section 12.7's places is this*.
`SourceKind` answers *what kind of document arrived*. They have different
cardinalities — seven against six — and this crate maps neither onto the other,
because there is no reading of section 12.7 under which `과제` and `공지` are
`README`s and every reading that forced one would be inventing a correspondence
the design document does not state.

What it does instead is bind them at the one point where a mismatch would matter.
`MaterialReference::of` carries the `SourceId` of an ingested document, and
`ExpectedConceptClaim::extract` refuses a claim none of whose cited spans point
into that document. So a claim labelled `SYLLABUS` whose citations all point into
a README is a value that cannot be built, and this crate never has to decide
which `SourceKind` a syllabus has.

## `강한 부족` is section 15.2's reading, not a threshold here

Section 12.7 asks for what is `이해를 막을 가능성이 큰` — likely to block
understanding. Section 15.2's table gives exactly one of its five kinds a `뜻`
that is a claim the person is missing something: `MASTERY_GAP`, `prerequisite
수행 evidence가 부족`. The other four say the person may know it
(`EVIDENCE_GAP`), that immediate use is *uncertain* (`FRESHNESS_GAP`), that the
graph is wrong (`ONTOLOGY_GAP`), or that the goal has not chosen (`CONTEXT_GAP`).

So `MinimalityDefect::NotALikelyBlock` is `RootCandidate::is_strong_deficit`
answering false, and a `FRESHNESS_GAP` root proposed as tomorrow's preparation is
refused. **This is a reading of the two sections together and it is recorded
here rather than left implicit**: section 12.7 does not itself say `강한 부족`,
and a later reader who decides a refresher *is* a blocking foundation is changing
this line rather than discovering it.

## The minimality validator holds no phrase, and that is executed twice

Section 12.7's `not included` block is `full lecture preview, advanced
replacement-policy survey`. A validator that refused those by matching their
words would pass the next paraphrase, so each variant of `MinimalityDefect` is a
fact about the prerequisite graph and the descent:

| Section 12.7 phrase | The fact that refuses it |
|---|---|
| `이해를 막을 가능성이 큰` | the routed kind is `P2-N5`'s `강한 부족` |
| `full lecture preview` | every candidate is reached by descending from the expected concept, so a concept the descent never reached is not one |
| `advanced replacement-policy survey` | the graph holds the expected concept as a prerequisite **of** the proposal, so it is above the lecture rather than beneath it |

The two refusals are distinguishable in the answer: an unrelated topic answers
with one defect and an advanced survey with two, so the refusal is attributable
without this crate holding either phrase.

`the_next_lecture_crate_holds_no_phrase_list` observes the absence as **two
whole sets**. Every non-ASCII string literal in every product file is required to
occur in the design document verbatim, which refuses a list of broad Korean
phrases. And every method any product file calls is compared with a pinned
inventory in both directions, which is the half a literal check cannot make: an
English-language phrase list is ASCII and could only be *used* through a text
comparison, and a text comparison is a method the inventory does not hold. The
two `contains` calls in the crate are enumerated with their receivers, and both
receivers are required to be a range or an identity set rather than a string.

## Three axes, and the fold is unrepresentable rather than refused

A three-element array of one reading type would be separated on paper and folded
in one line. So the three axes are three **different types** —
`ExpectedConceptReading`, `PrerequisiteEdgeReading`, `UserStateReading` — with no
trait between them, no shared supertype, and no collection holding more than one
of them. There is no array to reduce and no `PrepUncertainty` method that answers
with a confidence.

`PrepUncertainty::factor` reads each axis out of the argument that owns it, so
there is no parameter a caller could pass one list to three times:

| Axis | `근거` | `confidence` |
|---|---|---|
| `예상 concept` | the claim's own `P2-G5` spans, with the material's `자료 날짜` beside them | the claim's own |
| `prerequisite edge` | the edge's own cited items | **supplied** |
| `사용자 state` | the overlay's own supporting and contradicting items | the overlay's own |

**The third confidence is supplied and the other two are not, because `P2-N5`'s
`PrerequisiteEdge` carries no confidence.** Section 7.3 puts an edge's confidence
on the claim that asserts it rather than on the traversal, so there is nothing on
the edge to read. That asymmetry is recorded here rather than hidden behind a
default of some kind.

The evidence *types* differ too: axis one cites `ResolvedSpan`s into untrusted
documents and the other two cite `EvidenceId`s. `P2-N5`'s own
`RootCandidate::evidence` merges the edge's items with the overlay's into one
list, which is the fold this crate exists not to inherit — nothing here reads
that accessor.

The absence half is a whole-set classification rather than a list of forbidden
method names: every public signature in the crate whose return type mentions
`ConfidencePermille` is enumerated and each is required to be an accessor on one
of the three reading types or on the claim. A `PrepUncertainty::confidence`, a
`PreparationCandidate::score` or a `PreparationBrief::overall` added later is a
new entry in that set with an owner the answer does not hold, whatever it is
called.

## The morning bound is written in two sections and held by two crates

Section 4: `다음 강의에서 막힐 가능성이 큰 선수개념 1–3개가 보인다.`
Section 25.2: `수업 전 최소 prerequisite: 최대 1–3개, “왜 지금”과 예상 시간.`

`morning_home_contract` parses both, splits each on the document's own en dash,
requires the two readings to agree with each other, compares them with
`LOWEST_PREPARATION` and `HIGHEST_PREPARATION`, and then compares those with
`P2-X2`'s `LOWEST_BRIEF` and `HIGHEST_BRIEF`. `academic-home` is a **dev** edge
for that reason: the claim is that two crates offering the same card cannot drift
apart, and a product edge would make one crate's bound the other's by
construction rather than by comparison.

`왜 지금` and `예상 시간` are on the candidate by construction and neither is free
text. `왜 지금` — the occasion — is the `BlockingPath` from tomorrow's concept
down to this one with a strength on every hop, and `예상 시간` is `P2-N5`'s own
bounded `최소 보강`. A `RootCandidate` cannot exist without either, so a
candidate missing one is a value that cannot be written.

## `propose` neither ranks nor hides

Section 25.2: `알림 수가 많으면 자동 중요도 순으로 숨기지 않고`. When the descent
finds more strong deficits than the morning has room for, `propose` answers
`TooManyBlockingFoundations` with the count rather than choosing three. Ranking
blocking foundations against each other is `P2-N6`'s AND/OR question and nothing
here decides it.

A descent that found no strong deficit answers `Ok(None)` rather than an empty
brief, which is `P2-X2`'s rule that a group with nothing to offer shows no card.
That is a different answer from a refusal: nothing is wrong, there is simply
nothing to prepare.

## Two refusals this crate cites rather than makes

Both were branches no input could reach, which is the shape `P2-R5` measured as a
suite that cannot see a real defect.

**An unbounded or uncited preparation.** `GapExplanation::of` already refuses
`REMEDIATION_UNBOUNDED` and `REMEDIATION_UNCITED` before a `RootCandidate`
exists, so a candidate reaching this crate always carries a positive number of
minutes and at least one source. A fourth and fifth `MinimalityDefect` for those
would never fire.

**A claim citing nothing at all.** `P2-G5`'s proposal schema requires a `support`
line, so `adjudicate` never produces a `Proposal` with an empty support list. An
earlier draft carried a `ClaimCitesNothing` refusal for it; deleting that
refusal left the whole suite passing, and it was removed rather than given a
test. `ClaimDoesNotQuoteItsMaterial` is the surviving answer and it covers the
empty case for the same reason: a citation set empty for either reason leaves the
claim resting on no material.

## One guard was masked and is now driven directly

Deleting `PrepUncertainty::factor`'s own edge/state agreement left every test
passing. The guard is reachable — `factor` is public — but every path a
*candidate* takes runs `PreparationCandidate::of`'s concept check first, so the
constructor above was masking a guard on the value it builds.
`prep_uncertainty_factorization` now drives `factor` directly with an edge and an
overlay about two different concepts, and with the matching pair as a control.

## What this task does not decide

* **Which foundation comes first.** `P2-N6` owns the AND/OR hypergraph and the
  choice between routes. Nothing here ranks.
* **Whether the person knows the foundation.** Section 27.2. Nothing here writes
  a knowledge state, and no public signature returns a type `P2-N2` reads.
* **Persistence.** Nothing here is written. No migration, no `academic-store`
  edge, no file, no socket, no clock.
* **`§38`.** `P2-L6` opens and closes no gate.
