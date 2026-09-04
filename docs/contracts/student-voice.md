# Student voice, diarization measurement, capture PII hold

`academic-student-voice` is the `P2-L5` boundary: section 32.5's rules about
the people in the room who are not the user, the section 38.3 question about
diarization accuracy, and the fail-closed rule that follows from answering it
with a measurement instead of an estimate.

It sits above `P2-L4`'s lecture document, reads `P2-L3`'s transcript and
`P2-L2`'s capture bytes, and calls `P2-G6`'s retention and deletion-preview
functions rather than restating them. It records nothing, transcribes nothing,
diarizes nothing, persists nothing, and reads no clock.

## What this is about

A lecture room holds students who never agreed to this product. Every rule here
is fail-closed for that reason, and the one that governs the rest is that **an
automatic editing claim needs a measured number**.

## What is not here

**No diarizer and no speech engine.** There is no audio decoder, no model and no
speech engine in this repository. The corpus is two committed timelines per
case — what was said, and what a diarizer would have said about it — and the
scorer is a pure function of them.

**What the number is evidence for, exactly.** It measures the **scorer** against
a stated ground truth. It is not a measurement of any real speech engine in any
real room, and this page does not claim it is. What it does support is the
fail-closed rule, because that rule is about what happens when a number is low,
and a low number is exactly what a synthetic corpus can state exactly.
`CONTRIBUTING.md` rule 1 forbids lecture media here, so a corpus over real audio
is not a thing this repository can hold.

**No store and no migration.** There is no `academic-store` edge, which is what
makes "this crate persists nothing" a graph fact rather than a sentence. This
task writes no migration: a redaction, a hold and a preview are pure functions
of a transcript, a capture, an inventory and an evidence index, and there is
nothing here whose value would survive a restart that is not already durable one
layer down.

**No key destruction.** `academic-retention` is a **dev** dependency only.
`rotation_engine_lane_is_not_default` holds that exactly two crates declare that
product edge — `academic-portability`'s encrypted restore and `P2-P2`'s deletion
flow, which is the layer that decides when a key slot is destroyed — and a
redaction has no business inside that boundary. `apply_deletion` records an
expiry through `academic-consent`'s ledger and reaches no key.

**No broker, no egress, no worker.** `academic-policy`,
`academic-egress-boundary` and `academic-worker` are not edges of either kind,
so a product file here cannot name `PermissionBroker`, `EgressProxy` or a job
descriptor — an undeclared crate is a compile error, not a lint.

## Where `GATE-38-026` stands

**Partially discharged.** Section 38.3's fifth question is two questions and this
task answers one of them.

| Half | Status |
|---|---|
| *is the technical diarization accuracy sufficient?* | **answered as a measurement**: a number on a named, versioned, digested corpus, published below, with a fail-closed threshold that configuration cannot set to zero |
| *may student voices be deleted from the **originals**, and under what policy?* | **open, and this build selects nothing.** It is a decision for the user and their institution |

The way this crate does not answer the second half is structural rather than
documentary: [`RedactionScope`] has **one** variant, `DerivativeOnly`. There is
no `Original`, so a policy authorising removal from an original recording has no
spelling here at all — the same shape as `AutomaticLevel` having no `FLUENT` in
`P2-N2` and `AuthorshipMode` having no review value in `P2-R5`.

`academic-retention` holds the *mechanism* for a voice-scoped deletion of an
original, behind an `OriginalVoiceAuthority` a caller has to state. This crate
never produces one and no product file here names one;
`no_original_voice_authority_is_produced_here` measures both directions and also
refuses any `pub` signature in any package that takes a `P2-L5` value and returns
one. `GATE_38_026_OPEN` states the open half where the policy lives, and
`academic-retention`'s `GATE_38_026_STATEMENT` states it where the mechanism
does; the same test compares both against the gate identifier so neither side
can quietly start claiming the question is settled.

## The measured number

The corpus is `student-voice-diarization`, version 1, under
`testdata/diarization/v1/`. Six cases: three diarizers that behave and three
ways one fails, each failure a different consequence.

| Field | Value |
|---|---|
| corpus id | `student-voice-diarization` |
| corpus version | 1 |
| corpus digest | `sha256:783a492e5336507a6d4a68ea3c666c19f3d36788e6a276d38a9a84cd3319a5fe` |
| scorer version | 1 |
| cases | 6 |
| scored reference time | 550000 ms |
| student reference time | 60000 ms |
| attribution accuracy | 967 permille |
| student speech labelled instructor | 33 permille |
| student speech also labelled student | 766 permille |

`diarization_accuracy_is_measured_and_versioned` compares each of those against
a fresh run, with the expected values written as literals in the test file
rather than read back off the measurement — `P2-L3` shipped an oracle that read
its expected value out of the thing it was checking, and it agreed with itself.
`the_published_number_is_the_documented_one` reads this table out of **this
file** and compares it against a fresh run, so a number changed in code and left
undocumented fails rather than drifting.

### Two axes, because they are two failures

Attribution accuracy is one number and the fraction of student speech labelled
instructor is another. A lecture is mostly the instructor, so mislabelling every
student utterance costs a few permille of accuracy and costs all of the privacy.
The two are separate fields of the threshold and separate variants of the
refusal, and the acceptance suite drives each of them alone.

### The partition is what says the scorer is not lying

Every millisecond of reference time lands in exactly one of five buckets:
agreed, student-labelled-instructor, instructor-labelled-student, unattributed
(the diarizer declined), and uncovered (the diarizer said nothing there). The
five sum to the scored time, per case and over the fold. A scorer that
double-counted an overlap or dropped a hole fails that rather than reporting a
slightly wrong ratio nobody can see.

An unattributed or uncovered span of **student** speech is not counted as a
missed redaction, and that is deliberate: a fail-closed automatic redaction keeps
only the spans labelled instructor, so a span nobody attributed leaves the
derivative. That costs losslessness rather than privacy, and this task errs in
that direction throughout.

### The recorded threshold defaults

| Field | Default | Why this number |
|---|---|---|
| `version` | 1 | The first published configuration. |
| `min_accuracy_permille` | 990 | The subject is a third party's speech, so the default is strict rather than achievable. A profile may lower it, within the band below. |
| `max_missed_student_permille` | 0 | Zero student milliseconds may be labelled instructor. This is the privacy axis and its default is the only value that is not a trade-off. |

`the_recorded_defaults_are_the_documented_ones` reads this table out of this file
and compares it against `DIARIZATION_THRESHOLD_V1`.

### Configuration cannot empty the guard

Which number is enough is a user and institution decision, and `GATE-38-026`
says so. But a configuration that can be set to zero is a guard a profile can
delete, and the people it protects did not choose the profile. So
`DiarizationThreshold::new` refuses anything outside a band:

| Bound | Value |
|---|---|
| `ABSOLUTE_ACCURACY_FLOOR` | 900 permille |
| `ABSOLUTE_MISSED_STUDENT_CEILING` | 50 permille |

Inside the band the number is the user's; outside it there is no value at all.

### What the shipped corpus does with the shipped default

**It fails, on both axes.** 967 is below 990 and 33 is above 0. That is the
measured result and it is the intended default posture: with the corpus and the
threshold this build ships, **no automatic redaction claim can be made**, and the
only plan a profile can build is a manual one whose every exclusion a person
decided.

`below_threshold_diarization_blocks_automatic_redaction` drives both refusals
independently, drives the pass arm under a configuration inside the legal band
that both axes clear, drives a corpus a diarizer got exactly right against the
recorded default, and drives a bad diarizer against the weakest configuration
that is legal at all.

## An automatic claim is a type that needs a measurement

`RedactionMode` has two variants and `Automatic` carries an `AccuracyWitness`
**by value**. A witness has private fields, no `Default`, no public constructor,
and exactly one producer — `DiarizationMeasurement::witness`, which compares a
measured permille against a configured one. The whole of that function is pinned
and its construction is counted at one site across the package.

So a below-threshold measurement does not produce a weaker claim. It produces no
claim, and the value that would carry one does not exist. Four of the seven
`compile_fail` cases are that rule and its neighbours, each with its diagnostic
committed.

`Manual` carries no witness and cannot. Every span it excludes is a
`ManualExclusion` a person decided, one at a time, and every automatic actor is
refused by an exhaustive `match` over `academic-domain`'s closed `Actor`, so a
fifth actor class stops this crate compiling until it is classified.

A plan that excludes nothing is refused: `RedactionFault::NothingExcluded`. A
redaction that removes nothing would satisfy "the derivative contains no targeted
speaker" by containing everything.

## `P2-L4`'s `D-3`, closed

`docs/contracts/lecture-document.md` leaves `D-3` open in its own words: a
`RedactionPolicyRef` holds a digest that crate does not resolve, and "what the
digest names, and whether the redaction it authorises actually happened, is
`P2-L5`'s".

`RedactionPolicy::digest` is what that digest is — a hash over the policy's
version, basis, scope, targeting and deciding actor — and `RedactionPolicy::resolve`
is the comparison, run as the **first statement** of `redact`. A reference citing
a different policy is refused there rather than left for a reader to notice. The
basis is compared as well as the digest, because a reference that agrees on the
digest and disagrees on the basis is two records of one decision that do not say
the same thing.

## One redaction produces two values

`REQ-12-031` is two sentences — the sensitive utterance is hidden in the display
and redacted projections, *and* deletion of the original follows retention
policy — so one redaction produces two values.

| Value | What it holds | What it does not |
|---|---|---|
| `RedactedDerivative` | the utterances that survived, and for each one removed, the speaker and the span | **no text** for a removed utterance, and no field one could go in |
| `RestrictedOriginal` | every removed utterance, text included | **no accessor** for that text |

Their retention terms differ on purpose. The derivative's are the parent's
narrowed by whatever it asked for; the original's are the parent's, because
redacting a copy is not a reason to move the bound on the original.

### The canary is what makes the exclusion measurable

Every non-instructor utterance in the acceptance fixture carries a token no
instructor utterance carries. `redacted_derivative_excludes_targeted_speakers`
asserts that the token appears in **nothing** the derivative can be turned into:
not its canonical bytes, not its `Debug`, not any accessor. Walking the kept list
is the weak half — it reads the field the code decided to fill.

The control is a policy naming one student: the other students stay, the canary
is then present, and that is what says the exclusion follows the targeting rather
than the token.

Every type here that holds the lecture in words hand-writes a redacting `Debug`
that reaches its text through a length only, which is `P2-L3`'s and `P2-L4`'s
decision for their own text-bearing types, applied in the strengthening
direction.

### The restriction survives an authorized read

`REQ-12-031` requires the removed speech to be reachable under authorized raw
access. Four things make "restricted" more than a label:

* a `RawAccessGrant` is issued only to a user actor, by an exhaustive `match`;
* it is bound to one original by digest, and a grant for another is refused;
* it is **spent by being used** — `RestrictedOriginal::open` takes it by value,
  so a second read on one authorization is a program that does not compile.
  There is deliberately no `GrantAlreadySpent` refusal, because a variant nothing
  can produce is a value this repository does not ship; and
* the audit row is written **inside** `open`, before the disclosure is returned,
  so an authorized read with no record is not a call that exists.

Afterwards the classification, the removed count, the terms and the derivative's
bytes are exactly what they were, and the canary is still absent from the
derivative. A second read is a second grant and a second row.

`DisclosedOriginal` borrows the original, implements no `Clone`, and has no owned
form. `no_disclosure_reaches_a_derivative` is a rule over a **pair of types**
rather than a list of function names: no `pub` signature in any package takes a
disclosure or a restricted original and returns a derivative type, so a route back
fails however it is spelled.

## The capture PII hold

Section 32.5: *Capture에 학생 얼굴·명단·개인 화면이 들어가면 review 전 graph/OCR
ingestion을 보류.* Three classes and two jobs, both closed sets read out of that
sentence and compared in both directions.

The way to get this wrong is a boolean nobody downstream reads. It is held three
ways at once and the first is structural:

* `CaptureUnderReview` holds the `CaptureBytes` privately and has **no byte
  accessor**. There is nothing to hand an OCR pass. `P2-L1`'s
  `QuarantinedArtifact` is the same shape one layer down.
* `ReviewedCapture` is the only type here with a byte accessor, it has no public
  constructor, and its one producer is inside `dispatch` after the hold state
  admitted. The construction is counted at one site across the package.
* `dispatch` is the one door, and it is pinned whole. The acceptance row drives
  the **real** dispatch against a **real** stage on every held arm and observes
  the stage's call count stay at zero — which is what makes the guard load-bearing
  rather than a flag.

A review is a person's and it has to address every class the findings hold: a
reviewer who saw one of three findings has not reviewed the capture. A review for
another capture is refused, an automatic reviewer is refused, and a review that
withholds still blocks. A capture nothing was found in needs no review, because
there is nothing to review. Two findings of one class are one reason, not two.

## Retention: a derivative can only narrow

`P2-G6` owns the rule — the stricter of two bounds on each axis, in one function
— and this crate calls it at exactly **one** place, `inherit_terms`.
`derivative_terms_have_one_producer` counts `RetentionTerms::inherit` at one call
site across the package and `inherit_terms` at three, with `use` items dropped so
a re-export is not counted as a caller.

`derivative_expiry_is_equal_or_stricter` does not check a case. It walks the whole
cross product of a four-value bound grid on both axes — 256 `(parent, requested)`
pairs — through the **real** redaction, and requires `derived <= parent` on both
axes for every pair, plus the assertion that at least one pair narrowed strictly,
so the comparison is exercised rather than vacuously satisfied. Then it walks a
three-link chain — derivative, a transcript of it, an embedding of that — asking
for the widest terms there are at every link, and requires the result to be no
wider than the root.

The two axes stay independent through all of it: a parent whose audio is
`Prohibited` and whose transcript runs to the end of term produces a derivative
that says exactly that.

## The deletion preview names projections, not just classes

`academic_consent::preview_expiry` already walks every derivative class, inherits
each one's terms through the one inheritance function, and records that the
preview was shown. `preview_deletion` **calls** it and adds the layer section 32.5
asks for on top — *어느 하나의 삭제가 concept/evidence projection에 미치는 영향을
미리 보여준다* — so there is one preview carrying both halves rather than two
answers to one question.

Every affected projection is listed with its family, its identifier, how many of
the objects it cites this deletion reaches, how many it cites in total, and
whether it loses all of its evidence or some. A projection citing none of the
deleted objects is not listed.

**The totality guard is the partition.** Every deleted object is either cited by a
listed projection or listed in `unreferenced`, never both and never neither. A
walk that stopped short leaves an object in neither set; one that double-counts
puts it in both. `LectureDeletionPreview::partition_reconciles` states it and the
acceptance row asserts it over the whole deletion set.

Nothing is deleted without the preview. `LectureDeletionPlan::from_preview` is the
only constructor, `apply_deletion` the only consumer, and three things are compared
rather than trusted: the digest of the preview the user was actually shown, the
instant (which is `apply_expiry`'s own check reached through it), and the expiry
itself, which stays `P2-G6`'s.

## No floating point

Every ratio here is permille computed in `u64`. `academic-record` fixed that rule
for money and it holds for the same reason here: a number that decides whether
somebody's voice may be processed automatically must not depend on a rounding
mode. `no_floating_point_reaches_this_crate` refuses `f32`, `f64` and every
digit-dot-digit literal anywhere in the package, tests included, with the reader
checked against a sample that has one and two that do not.

## The corpus is executed, not counted

`docs/contracts/engine-harness.md` says a set of fixtures that only exists "has
satisfied the audit and demonstrated nothing". Diarization is **not** one of §28's
twelve engines — the registry is pinned to that table and a thirteenth entry
would fail `engine_registry_is_complete` — so what is borrowed here is the
discipline rather than the registry:

* every committed `.input` is read off disk and parsed back into a case by a
  parser written in the **test** file rather than in the crate, so the crate does
  not agree with itself;
* the parsed case is scored by the real scorer and byte-compared against the
  committed `.expected`;
* the whole directory is re-rendered from `harness::corpus_files` and compared,
  so a fixture edited by hand into agreement with a broken scorer fails; and
* the directory is walked, so a file nobody renders is a failure rather than a
  file nothing reads.

`cargo run -p academic-student-voice --example emit_corpus` writes the same bytes
the suite compares. It exists to update them after a deliberate change and never
as the source of truth for what they should contain.

## What the plan and the specification disagree about

| Plan says | What is true | Resolution |
|---|---|---|
| `P2-L5` closes `REQ-32-037` | that is a `apps/mobile-capture` accessibility snapshot asserting an explicit non-instructor-voice warning before recording | not here. This crate produces the values such a warning would read — the hold state, the classes found, the targeting a policy carries — and renders none of them. There is no capture UI in this repository. |
| `P2-L5` closes `REQ-32-043` | audio and transcript retention independently configurable | that is `P2-G6`'s two bounds, and this task adds no second retention model. What is added is that a **redaction's** two products carry different terms on the two axes, and that a derivative chain narrows on each axis independently. |
| `P2-L5` closes `REQ-32-045` | "before deletion, UI previews impact on concept/evidence projections" | the values are here and the UI is not. `packages/ui` renders no preview yet. |
| `P2-L5` closes `REQ-37-030` | school lecture works keep obeying original conditions after graduation | the inheritance half is here and is the whole of `derivative_expiry_is_equal_or_stricter`; the lifecycle half — advancing time and re-deciding an export — is `P2-P2`'s product flow, which reaches this crate's projection walk through [the deletion flow](deletion-and-retention-flow.md). |

The specification is authoritative in every row.

## Open

| # | What is open | When it starts mattering |
|---|---|---|
| V-1 | Whether student voices may be removed from an **original** recording, and under whose authority. `GATE-38-026`'s second half. This build implements no path to it and selects no policy. | The first user who has an institutional answer. Closing it means a scope value that does not exist here and an `OriginalVoiceAuthority` this crate does not produce. |
| V-2 | The corpus is synthetic, so the number measures the scorer rather than a speech engine in a room. A real diarizer's accuracy on this user's lectures is unmeasured and this build does not estimate it. | The first real adapter. The corpus identity and digest are already carried on every measurement, so a second corpus is a version rather than a rewrite. |
| V-3 | The hold's findings arrive from a caller. There is no detector in this repository, so what a capture is screened *by* is outside this boundary and a capture nothing was reported for reads as clear. | The first vision detector. What is here bounds the damage: a clear capture is a caller's claim, and the reviewer path exists for every class the caller does report. |
| V-4 | An automatic plan reads the transcript's own speaker attribution, which is the diarizer's output at one version. A re-transcription that attributes differently produces a different derivative, and this crate does not compare two of them. | The first re-transcription of a lecture that was already redacted. `P2-L3`'s version lineage is where such a comparison would live. |
| V-5 | The evidence index a preview walks is supplied. A projection that cites a deleted object and is missing from the index is missing from the preview, and nothing here can see that. | The first projection store. The partition rule is what makes the gap visible the moment the index is complete. |
| V-6 | Nothing writes a derivative, an original or a preview to disk. Two runs over one process agree by construction; two runs across a restart re-derive from the transcript. | The first redaction that outlives its process, which is the same dependency admission `P2-L4`'s `D-5` has not made either. |

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`. No recording is made, no device is opened, no audio is
decoded, no model is run, no byte leaves the machine, and every fixture in this
crate's test tree is synthetic and built from committed literals.
