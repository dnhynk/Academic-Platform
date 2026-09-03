//! Two conflict classes, both sides of each, and nothing that settles one.
//!
//! Section 25.13's third bullet is *`unresolved conflict: user override vs new
//! evidence, code vs spec`*. Section 30.4 fixes what happens to the first:
//! *`AI 재분석은 이 결정을 지우지 않는다. 새 runtime trace가 반박하면
//! NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE를 만들고, 사용자가 유지·수정·scope 종료를
//! 선택한다.`* Section 34.4 fixes the second: a specification read as an
//! implementation produces an `INTENDED_NOT_IMPLEMENTED` drift, with the intent
//! lane and the implementation lane kept apart.
//!
//! # What "unresolved until user action" is made of
//!
//! Four things, and only the first is a check:
//!
//! 1. [`ConflictCase::settle`] takes an `academic_proposal::UserDecision`,
//!    which `UserDecision::by` issues only for `Actor::User`. `P2-M2` owns that
//!    door; this crate reuses it instead of writing a second actor match that
//!    could drift from it.
//! 2. There is no other way in. `ConflictCase` has no `resolve`, no
//!    `auto_resolve`, no `expire`, no setter and no public field, and
//!    [`Resolution`] has no public constructor: it is computed from the
//!    history, so a resolution with no record behind it is not a value that
//!    exists.
//! 3. Settling **appends**. [`ConflictCase::settle`] pushes a
//!    [`CorrectionRecord`] and rewrites nothing, which is `CONTRIBUTING.md`'s
//!    append-only rule and section 34.6's second recovery principle.
//! 4. Neither side is rewritten. `P2-R3` made `ImplementationDrift` a record
//!    that rewrites neither lane; the same holds here for both classes, and
//!    `both_conflict_classes_are_unresolved_until_user_action` compares each
//!    side before and after every attempt.

use academic_domain::{
    Actor, AuthorityClass, ClaimId, EpistemicStatus, SnapshotId, TimestampMillis, ValidInterval,
};
use academic_proposal::UserDecision;

use crate::CenterError;

/// The one place an actor becomes a receipt this crate will accept.
///
/// `P2-M2`'s `UserDecision::by` is what decides, over `academic-domain`'s
/// closed `Actor` enum, and its refusal is carried rather than restated. Every
/// door in this crate that settles anything takes the `UserDecision` this
/// returns, so an automatic actor is refused once, here, instead of by four
/// checks that could drift apart.
///
/// # Errors
///
/// [`CenterError::NotTheUser`] for a deterministic engine, a model run or an
/// importer.
pub fn user_receipt(actor: &Actor) -> Result<UserDecision, CenterError> {
    UserDecision::by(actor).map_err(|refusal| CenterError::NotTheUser { refusal })
}

/// The two classes section 25.13 names.
///
/// Closed, and each arm is a different pair of things in disagreement, which is
/// why a single class with a tag would be wrong: the two are not two instances
/// of one shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConflictClass {
    /// Section 30.4: a user override, and evidence that arrived after it.
    OverrideVersusNewEvidence,
    /// Section 34.4: what the code does, and what the specification says.
    CodeVersusSpec,
}

impl ConflictClass {
    /// Exhaustive listing, in section 25.13's own reading order.
    pub const ALL: [Self; 2] = [Self::OverrideVersusNewEvidence, Self::CodeVersusSpec];

    /// Section 25.13's own words for this class.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::OverrideVersusNewEvidence => "user override vs new evidence",
            Self::CodeVersusSpec => "code vs spec",
        }
    }

    /// The vocabulary token the specification gives this conflict.
    ///
    /// Section 34.2's `user override를 AI가 다시 덮어씀` row displays
    /// `NEW_EVIDENCE_CONFLICT` and section 34.4's `spec을 구현으로 오인` row
    /// displays `INTENDED_NOT_IMPLEMENTED`. Both are the specification's own
    /// spellings.
    #[must_use]
    pub const fn marker_token(self) -> &'static str {
        match self {
            Self::OverrideVersusNewEvidence => "NEW_EVIDENCE_CONFLICT",
            Self::CodeVersusSpec => "INTENDED_NOT_IMPLEMENTED",
        }
    }
}

/// Which lane a side of a conflict speaks for.
///
/// `P2-R3` keeps the intent lane and the implementation lane apart, and the
/// override case has the same shape one level up: a decision the user recorded,
/// beside an observation that arrived later. A side names its lane so that a
/// reader of a conflict card can tell which one is being asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConflictLane {
    /// The decision or specification already on record.
    Held,
    /// The observation or implementation that disagrees with it.
    Incoming,
}

impl ConflictLane {
    /// Exhaustive listing.
    pub const ALL: [Self; 2] = [Self::Held, Self::Incoming];
}

/// One side of a conflict, as a conflict card shows it.
///
/// Section 30.1's rule is that a competing claim is not rewritten, so a side is
/// a reference to a claim plus what a reader needs to weigh it: its status, the
/// authority class its claim type ranks under, when it was recorded, when it
/// applies, and — for the implementation lane — which immutable snapshot the
/// observation was made in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictSide {
    lane: ConflictLane,
    claim: ClaimId,
    status: EpistemicStatus,
    authority: AuthorityClass,
    recorded_at: TimestampMillis,
    applies: ValidInterval,
    observed_in: Option<SnapshotId>,
}

impl ConflictSide {
    /// One side of a conflict.
    #[must_use]
    pub const fn new(
        lane: ConflictLane,
        claim: ClaimId,
        status: EpistemicStatus,
        authority: AuthorityClass,
        recorded_at: TimestampMillis,
        applies: ValidInterval,
        observed_in: Option<SnapshotId>,
    ) -> Self {
        Self {
            lane,
            claim,
            status,
            authority,
            recorded_at,
            applies,
            observed_in,
        }
    }

    /// Which lane this side speaks for.
    #[must_use]
    pub const fn lane(&self) -> ConflictLane {
        self.lane
    }

    /// Which claim.
    #[must_use]
    pub const fn claim(&self) -> ClaimId {
        self.claim
    }

    /// Section 30.2's status vocabulary.
    #[must_use]
    pub const fn status(&self) -> EpistemicStatus {
        self.status
    }

    /// Section 30.3's authority class.
    #[must_use]
    pub const fn authority(&self) -> AuthorityClass {
        self.authority
    }

    /// Transaction time: when the system learned this.
    #[must_use]
    pub const fn recorded_at(&self) -> TimestampMillis {
        self.recorded_at
    }

    /// Valid time: when it applies.
    #[must_use]
    pub const fn applies(&self) -> ValidInterval {
        self.applies
    }

    /// The immutable snapshot an observation was made in, if it was one.
    #[must_use]
    pub const fn observed_in(&self) -> Option<SnapshotId> {
        self.observed_in
    }
}

/// The three things section 30.4 offers, and nothing else.
///
/// *`사용자가 유지·수정·scope 종료를 선택한다`*. The set is closed and is the same
/// for both classes: [`ConflictCase::offered`] takes no argument, so no
/// condition can narrow it, and no confidence, age or authority comparison can
/// remove an option a user is entitled to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CorrectionChoice {
    /// 유지 — the held side stands and the incoming observation is recorded
    /// against it.
    Keep,
    /// 수정 — the held side is superseded by a replacement the user names.
    Modify,
    /// scope 종료 — the held side stops applying from an instant the user
    /// names, and stays in history for every instant before it.
    EndScope,
}

impl CorrectionChoice {
    /// Exhaustive listing, in section 30.4's own reading order.
    pub const ALL: [Self; 3] = [Self::Keep, Self::Modify, Self::EndScope];

    /// Section 30.4's own word for this choice.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::Keep => "유지",
            Self::Modify => "수정",
            Self::EndScope => "scope 종료",
        }
    }
}

/// What a user chose, and what the choice needs beside it.
///
/// `Modify` names a replacement claim and `EndScope` names the instant the
/// held side stops applying, because neither choice means anything without it.
/// `Keep` needs nothing, and carries nothing. That is why this is an enum and
/// not a struct with two `Option` fields: an `Option` would let a `Modify` be
/// recorded with no replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionOutcome {
    /// The held side stands.
    Keep,
    /// The held side is superseded by this claim.
    Modify {
        /// The replacement the user named.
        replacement: ClaimId,
    },
    /// The held side stops applying at this instant.
    EndScope {
        /// When it stops applying.
        ends_at: TimestampMillis,
    },
}

impl CorrectionOutcome {
    /// Which of section 30.4's three this outcome is.
    #[must_use]
    pub const fn choice(&self) -> CorrectionChoice {
        match self {
            Self::Keep => CorrectionChoice::Keep,
            Self::Modify { .. } => CorrectionChoice::Modify,
            Self::EndScope { .. } => CorrectionChoice::EndScope,
        }
    }
}

/// One appended decision about one conflict.
///
/// It carries the user receipt it was filed under, so the record says who
/// settled the conflict without this crate reading an actor of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionRecord {
    outcome: CorrectionOutcome,
    decided_by: UserDecision,
    decided_at: TimestampMillis,
}

impl CorrectionRecord {
    /// What was chosen.
    #[must_use]
    pub const fn outcome(&self) -> CorrectionOutcome {
        self.outcome
    }

    /// Which of the three.
    #[must_use]
    pub const fn choice(&self) -> CorrectionChoice {
        self.outcome.choice()
    }

    /// The user receipt this was filed under.
    #[must_use]
    pub const fn decided_by(&self) -> &UserDecision {
        &self.decided_by
    }

    /// When it was filed.
    #[must_use]
    pub const fn decided_at(&self) -> TimestampMillis {
        self.decided_at
    }
}

/// Whether a conflict has been settled, computed from its history.
///
/// There is no public constructor. `Settled` is reachable only by walking a
/// history that holds a [`CorrectionRecord`], and the only thing that appends
/// one is [`ConflictCase::settle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Nobody has decided. This is what a conflict is until a user acts.
    Unresolved,
    /// A user decided, and this is what they chose.
    Settled(CorrectionChoice),
}

/// One conflict, both of its sides, and its append-only history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictCase {
    class: ConflictClass,
    held: ConflictSide,
    incoming: ConflictSide,
    opened_at: TimestampMillis,
    history: Vec<CorrectionRecord>,
}

impl ConflictCase {
    /// Opens a conflict.
    ///
    /// Opening records the disagreement and decides nothing: the history starts
    /// empty and [`Self::resolution`] is [`Resolution::Unresolved`].
    #[must_use]
    pub const fn open(
        class: ConflictClass,
        held: ConflictSide,
        incoming: ConflictSide,
        opened_at: TimestampMillis,
    ) -> Self {
        Self {
            class,
            held,
            incoming,
            opened_at,
            history: Vec::new(),
        }
    }

    /// Which class.
    #[must_use]
    pub const fn class(&self) -> ConflictClass {
        self.class
    }

    /// Both sides, in one call.
    ///
    /// A card that could be built from one side is a card that can show one
    /// side. The accessor returns the pair so that no caller has to remember to
    /// ask twice.
    #[must_use]
    pub const fn both_sides(&self) -> (&ConflictSide, &ConflictSide) {
        (&self.held, &self.incoming)
    }

    /// When the disagreement was recorded.
    #[must_use]
    pub const fn opened_at(&self) -> TimestampMillis {
        self.opened_at
    }

    /// The three choices section 30.4 offers.
    ///
    /// Takes no argument, so nothing can narrow it.
    #[must_use]
    pub const fn offered(&self) -> [CorrectionChoice; 3] {
        CorrectionChoice::ALL
    }

    /// Every decision filed against this conflict, in filing order.
    #[must_use]
    pub fn history(&self) -> &[CorrectionRecord] {
        &self.history
    }

    /// Whether a user has settled it, and how.
    ///
    /// Computed from the history rather than stored, so there is no field a
    /// write could set.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.history
            .last()
            .map_or(Resolution::Unresolved, |record| {
                Resolution::Settled(record.choice())
            })
    }

    /// Files a user decision against this conflict.
    ///
    /// Appends. Neither side is touched, and no earlier record is removed: a
    /// user who changes their mind files a second record, and the first stays
    /// in the history.
    ///
    /// It takes a `UserDecision` rather than an `Actor`, so there is no actor
    /// this function refuses: an actor that is not the user cannot produce the
    /// argument. [`user_receipt`] is the one place this crate turns an actor
    /// into one, and it is where the refusal is observed.
    pub fn settle(
        &mut self,
        outcome: CorrectionOutcome,
        decided_by: UserDecision,
        decided_at: TimestampMillis,
    ) {
        self.history.push(CorrectionRecord {
            outcome,
            decided_by,
            decided_at,
        });
    }
}

/// Every open and settled conflict, of both classes, in one place.
#[derive(Debug, Clone, Default)]
pub struct ConflictBoard {
    cases: Vec<ConflictCase>,
}

impl ConflictBoard {
    /// An empty board.
    #[must_use]
    pub const fn new() -> Self {
        Self { cases: Vec::new() }
    }

    /// Records a conflict.
    pub fn open(&mut self, case: ConflictCase) {
        self.cases.push(case);
    }

    /// Every conflict, of both classes.
    #[must_use]
    pub fn cases(&self) -> &[ConflictCase] {
        &self.cases
    }

    /// Exactly the conflicts of one class.
    #[must_use]
    pub fn of_class(&self, class: ConflictClass) -> Vec<&ConflictCase> {
        self.cases
            .iter()
            .filter(|case| case.class() == class)
            .collect()
    }

    /// Exactly the conflicts nobody has decided.
    #[must_use]
    pub fn unresolved(&self) -> Vec<&ConflictCase> {
        self.cases
            .iter()
            .filter(|case| case.resolution() == Resolution::Unresolved)
            .collect()
    }

    /// Files a user decision against the open conflict of `class` whose held
    /// side is `claim`.
    ///
    /// # Errors
    ///
    /// [`CenterError::NoSuchConflict`] when the board holds no such conflict.
    pub fn settle(
        &mut self,
        class: ConflictClass,
        claim: ClaimId,
        outcome: CorrectionOutcome,
        decided_by: UserDecision,
        decided_at: TimestampMillis,
    ) -> Result<(), CenterError> {
        let case = self
            .cases
            .iter_mut()
            .find(|case| case.class() == class && case.both_sides().0.claim() == claim)
            .ok_or(CenterError::NoSuchConflict { class, claim })?;
        case.settle(outcome, decided_by, decided_at);
        Ok(())
    }
}
