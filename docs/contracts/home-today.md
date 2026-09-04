# Home / Today

`academic-home` is `P2-X2`. Section 25.2 of the authoritative specification says
`Home은 다음 우선순위로 한 화면을 구성한다` and numbers eight things. This crate
holds those eight as a closed enumeration with one payload type each, and
`packages/ui`'s `home.ts` is the shell side that says which sections the `/`
route shows and in what order.

It persists nothing. It claims no migration number, and the section below says
why with the evidence.

## What this is not evidence for

**No window opens.** `P2-X1` merged with no Tauri runtime linked, and that
decision is still open under the user gate. Nothing here depends on a window,
and nothing here is evidence that one exists: the crate is a set of typed
records and the rules between them, checked by compiling it, running its tests,
or reading its source. The shell half adds that opening `/` yields sections
naming section 25.2's own eight groups instead of a promise — that is a
structure, not a rendering, and `P2-X1`'s and `P2-X7`'s pages say the same thing
about their own.

**An upcoming use is a value the caller supplies.** `UpcomingUse::declare`
refuses an occasion that is not strictly after the reference instant, and that
is all it can check. It is not a claim that the occasion is on anybody's real
timetable. The surface that composes a card is what knows that, and this crate
has no edge to it.

**A permission status is not a permission.** This crate names
`academic_consent::CaptureStatus` and holds no grant, no token and no
capability: it has no edge of any kind to `academic-policy` or
`academic-capture-gate`. `허용` on this screen reports `P2-G6`'s answer;
`bind_permission` is still the only thing that turns a recorded permission into
a capture capability, and nothing here can be mistaken for one.

**The hero claim is about places, not about arithmetic.** The absence claim
below refuses a card, a field, a section and a slot for a headline metric, and
those four sets are exhaustive over what this crate declares. It is *not* a
claim that no number this screen shows was computed from a grade average: a
caller who put one into `EstimatedMinutes` would pass every check, and nothing
here would notice.

## The eight groups, and where their order comes from

| # | Section 25.2's line | What holds it |
|---|---|---|
| 1 | `오늘 실제 일정: 수업, assessment deadline, project event.` | `ScheduledItem` over a three-arm `ScheduledOccasion` |
| 2 | `수업 전 최소 prerequisite: 최대 1–3개, “왜 지금”과 예상 시간.` | `PrerequisiteBrief`, whose items need both |
| 3 | `녹음 permission 상태: `허용`, `조건부`, `확인 필요`, `금지`.` | `RecordingPermission`, the image of `P2-G6`'s five |
| 4 | `사용자가 직접 남긴 열린 질문과 Mark Moment review.` | `OpenItem` over a two-arm `OpenItemKind` |
| 5 | `현재 project를 막는 가장 가까운 knowledge need.` | `KnowledgeNeed` |
| 6 | `deadline이 있는 공식 학사 condition과 stale official data 경고.` | `OfficialCondition`, two arms |
| 7 | `활성 Critical Path의 사용자 선택 다음 단계.` | `NextStep` |
| 8 | `중요한 concept의 freshness 알림은 실제 upcoming use가 있을 때만.` | `FreshnessAlert`, which needs an `UpcomingUse` |

`home_group_order_is_stable_one_to_eight` parses that numbered list out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares **three**
enumerations that are not derived from one another: the document's own lines,
`HomeGroup::ALL`, and `HomeGroup::position`, which is written out for the reason
`P2-X1`'s view registry is written out rather than derived from its route
manifest — a derived number would agree with the listing because it *was* the
listing. The comparison is a set equality in both directions **and** a
position-by-position equality on the specification's own text, so a paraphrase
fails, a reordering fails, and a ninth line in the document fails as a missing
key. The parser refuses a numbered line that does not follow its predecessor
rather than skipping it: a skipped line is a group that silently stops being
required.

**No count is asserted anywhere.** Both sides are enumerated. Six
planning-versus-specification count mismatches were measured in this run, one of
them in section 25's own neighbourhood, and this is the discipline `P2-N3` and
`P2-N6` set in response.

Then the rendered screen is driven. `HomeScreen::sections` returns
`[HomeSection; HomeGroup::COUNT]` whose `i`th entry is `HomeGroup::ALL[i]`, and
the test composes a corpus, rotates it through every one of its own positions,
and requires the section sequence to be unchanged each time — so composition
order reaches the cards inside a section and never the sections themselves.

## Section 25.2's second line, held by a constructor with no way round it

`PrerequisiteItem::offer` takes the reason and the time as parameters. The
fields are private, there is no `Default`, no setter and no builder, so there is
no state in which an item exists and either is missing.
`tests/compile_fail/a_prerequisite_item_needs_its_reason_and_its_time.rs` is the
compiled half.

**The reason is a type, not prose.** `“왜 지금”` asks why *now*, and the answer
is the occasion that makes it now — an `UpcomingUse`, which by construction is
strictly ahead of the instant it was judged from. A free-text field would let
`왜 지금` be answered with anything at all, including with nothing, and would put
a caller's text into a crate that holds none.

**The bound is the document's.** `prerequisite_count_is_within_one_to_three_with_reason_and_time`
splits `최대 1–3개` on the document's own en dash, compares the two halves with
`LOWEST_BRIEF` and `HIGHEST_BRIEF`, and then drives `PrerequisiteBrief::assemble`
at every count from zero to two past the upper bound, judging each answer
against the **parsed** bound rather than against a hard-coded expectation. The
empty brief is refused too: a group with nothing to offer shows no card rather
than an empty one. There is no `push` on a brief, so a fourth item cannot be
added after the check.

## Four permission words, and the five statuses behind them

`RecordingPermission` has four arms carrying section 25.2's own spellings, no
`FromStr`, no `TryFrom<&str>`, no `From<&str>` and no arm holding a free-form
word. `permission_status_is_exactly_four_values` reads the four out of the
document's own back quotes, compares them with `RecordingPermission::ALL` in
both directions **and in order**, and then removes them from the line and
requires what is left to be punctuation — so a fifth word in the document leaves
text behind and fails rather than being folded into the nearest arm. The arm set
is read back out of this crate's source and compared with a four-entry
expectation, and
`tests/compile_fail/a_recording_permission_is_not_built_from_a_string.rs` is the
compiled half.

**The compile-fail case is not the only thing holding that door, because one
guard held it and an injection walked past everything else.** Adding a `FromStr`
to `RecordingPermission` failed the compile-fail case and passed every
behavioural assertion in the suite. So the same test now compares two more whole
sets, in both directions, over this crate's product source: every `impl` header,
and every derived trait **keyed on the type deriving it**. There is no
`impl Trait for Type` line in this crate outside the `thiserror` derive, and a
conversion, a `Deref`, a `Borrow` or a `Serialize` on any type here fails as an
extra key — not because it is on a list of things to refuse, but because the
comparison is against the list of implementations that *exist*.

The derives are keyed on the type for a measured reason. A flattened set holds
`Default` once, because `HomeScreen` derives it for the empty screen, and an
injected `Default` on `AlertBucket` passed the flattened comparison. Keyed on
the type, the same injection changes that type's value and fails. No payload
type derives `Default`: an empty card would answer a question nobody asked, and
a `Default` on `UpcomingUse` or `FreshnessAlert` would be exactly the value
section 25.2's second and eighth lines refuse — and that one the compiler
already refuses, because `EntityId`, `ScheduledOccasion` and `FreshnessBand`
have no `Default` for a derive to build on.

`RecordingPermission::of` maps `P2-G6`'s five statuses onto those four, so this
crate declares no second status vocabulary:

| `CaptureStatus` | Word | Why |
|---|---|---|
| `PERMITTED` | `허용` | a written authority granted and nothing is outstanding |
| `PERMITTED_WITH_CONDITIONS` | `조건부` | granted, with something still to satisfy |
| `UNKNOWN` | `확인 필요` | nobody has answered for this scope |
| `EXPIRED` | `확인 필요` | the grant no longer covers this instant |
| `PROHIBITED` | `금지` | a written authority refused |

The map is onto and it is not injective, and both halves are deliberate.
`UNKNOWN` and `EXPIRED` are the same word because nobody refused either — so
neither is `금지` — nothing currently grants either — so neither is `허용` or
`조건부` — and what the user must do about both is the same. Folding them is
what section 25.2 asks for by naming four words for a five-valued status. The
two stay distinct where the difference is recorded: `academic-consent` keeps
them apart and this crate cannot change that.

**The compiler does not hold the sixth-status case.** `CaptureStatus` is
`#[non_exhaustive]`, so a `match` on it from outside `academic-consent` must
carry a wildcard arm, and a sixth status would compile here and inherit whatever
that arm answers. This page said the opposite in an earlier draft and the
compiler measured it wrong. What holds it instead is a source-level comparison:
`permission_status_is_exactly_four_values` reads the arms of
`CaptureStatus::as_str` out of `crates/consent/src/status.rs` and compares them,
in both directions, against the five this crate names explicitly. The wildcard
answers `확인 필요`, which is the default-deny reading and the one
`CaptureStatus::Unknown` is `Default` for: an unrecognised status must not read
as permission to record.

## Grouping loses nothing, at any volume

`GroupedAlerts::group` takes the whole list by value and returns three lists. It
has no count parameter, no threshold, no importance argument and no cut-off,
because there is nothing for one to be. What it hands back is `&[HomeCard]` in
each direction — no `&mut` accessor, no owned `Vec`, no `drain`, no `truncate`,
no `retain` — so a caller cannot shorten a bucket either;
`tests/compile_fail/a_grouped_bucket_cannot_be_shortened.rs` is the compiled
half. `total` is derived from the buckets rather than remembered from the input,
so a count that disagreed with the buckets is not expressible.

`overflow_is_grouped_not_hidden_and_count_preserved` compares the union of the
three buckets against the input as **multisets in both directions**, not as
lengths: a length comparison passes an implementation that dropped one card and
duplicated another, and the control beside it is exactly that implementation,
required to fail the same comparison. It runs at one, two, seven and sixty
rounds of a corpus that puts a card in every group, requires each card to be in
the bucket its own deadline puts it in, and requires all three buckets to be
non-empty from two rounds up — so the loop proves something about each of them.

The three names are read out of the document's own back quotes and compared with
`AlertBucket::ALL` in both directions and in order.

**Where the boundary between `Today` and `Soon` comes from.** From the caller.
`DayWindow` carries the reference instant and the instant the caller says today
ends at, because this crate reads no clock and knows no calendar: a day boundary
depends on a time zone and a screen may not guess one. A deadline at or before
the window's end is `Today` **including one already past** — a deadline that has
gone by is the most `Today` thing on the screen, and the alternative would be a
fourth bucket the specification does not have.

Only `ends` decides a bucket. `starts` is on the value because it is the instant
the screen is being read at, and it is the instant a caller passes to
`UpcomingUse::declare` as the reference a card's `왜 지금` is judged against: a
card whose reason was judged against one instant and bucketed against another
would be incoherent, and carrying both on one value is what keeps a caller from
having two. `DayWindow::new` refuses a window whose end precedes its start, so
the pair cannot be inverted.

## A freshness alert needs a use, and that is `P2-N3`'s discipline

`FreshnessAlert::raise` takes an `UpcomingUse` by value; the fields are private
and there is no `Default` and no second constructor, so an alert with nothing
behind it is not writable. The refusal is one step earlier than the alert:
`UpcomingUse::declare` is the only producer of that value and it refuses an
occasion that is not strictly ahead of the reference instant, so an alert about
a use in the past has nothing to be built from.
`freshness_alert_requires_an_upcoming_use` sweeps the boundary rather than
sampling it — every instant from five before the reference to five after, over
all three occasions — and drives all six `FreshnessBand` values, so the rule is
visibly about the use and not about the band.

This matters beyond this screen. `P2-N3` fixes that time decay reaches a
freshness projection and never a mastery, and that a `STALE` band is a statement
about immediate retrieval rather than a demotion. A first screen that raised
*you have forgotten this* on a timer would make that discipline invisible
whatever the crate below it did, because the timer is what the user would
actually experience.

`the_home_surface_cannot_name_a_mastery` holds the other half the way
`academic-freshness` holds its own: `academic_domain::MasteryLevel` is one `use`
away from this crate, so the claim is made by the source rather than by the
closure — six spellings against every product file, **with a control** requiring
the same reader to find at least four of them in `P2-N2`'s own `ladder.rs`, so
the zero reported here is a measurement rather than a broken reader.

## No hero metric, proved by exhaustion

Section 25.2: `GPA나 streak를 hero metric으로 두지 않는다`.
`no_gpa_or_streak_hero_component` is an absence claim, and **there is no list of
forbidden spellings anywhere in the suite**. A name list refuses the edits
somebody thought of in advance and admits every edit spelled differently, which
this run measured six times, and `P2-RF13` found six real leaks the moment one
became a whole-set classification. Four whole-set comparisons, each blind to a
different bypass and each in both directions:

1. **The cards and the groups.** `HomeCard`'s variants and `HomeGroup`'s are
   read out of this crate's source and compared with each other and with
   `HomeGroup::ALL`. A ninth card of any name fails as an extra key, and a group
   with no card fails as a missing one.
2. **Every field position.** Every named field, tuple position and
   struct-variant field of every type in this crate's product source, compared
   against a reviewed inventory that says which of section 25.2's eight lines
   each one serves. This is the exhaustive net, and it is deliberately not a
   list of names to refuse — it is the list of positions that *exist*. A
   quantity added anywhere, spelling nothing anybody thought to forbid, in a
   module nobody predicted, fails as an extra entry.
3. **The section sequence.** `HomeScreen::sections` returns a fixed-length array
   whose order is `HomeGroup::ALL`'s, driven at zero, one and five rounds, with
   the first section required to be `오늘 실제 일정`. There is no `push`, no
   `insert` and no `Vec`, so there is nowhere to put a slot above the first.
4. **The shell.** `packages/ui/src/home.test.ts` compares the rendered `/`
   view's sections against `HOME_GROUPS` in both directions and in order, and
   compares `HOME_GROUPS` against `HomeGroup::id`, `HomeGroup::position` and
   `HomeGroup::ALL` read out of `crates/home/src/lib.rs`. A ninth section cannot
   be added on the TypeScript side either.

The reader under comparison two is itself measured.
`the_field_reader_finds_a_position_nobody_reviewed` drives it over a fixture
holding exactly what a headline metric would look like — a struct field, an enum
tuple position and an enum struct-variant field, none of them spelling anything
this suite forbids, because this suite forbids no spelling — and requires all
five positions to be found, and requires the reader to find nothing in a file
with no type in it. A whole-set comparison is only as good as the reader under
it, and the first version of this one could not see a single-line struct variant
at all; the fixture is what said so.

## What stays open

- The Tauri runtime binding, with the 388-package admission `P2-X1` measured.
- Every card's *content*. This crate fixes the screen's shape, its order, and
  the four refusals above; what fills `KnowledgeNeed`, `NextStep` and
  `OfficialCondition` is `P2-N5`, `P2-C7` and `P2-U6`, and this crate has no
  edge to any of them, so a card is a record of their answers rather than a
  second one.
- `P2-X6`'s accessibility conformance over these eight sections. `X2` is one of
  its five prerequisites; nothing here claims a contrast ratio, a non-colour
  encoding or a keyboard path.
