//! One screen, eight sections, in section 25.2's order.
//!
//! # There is no ninth section and no slot above the first
//!
//! [`HomeScreen::sections`] returns `[HomeSection; HomeGroup::COUNT]` — a fixed
//! array whose length is [`crate::HomeGroup::ALL`]'s. Its `i`th entry is
//! `HomeGroup::ALL[i]`, always, whatever order the cards arrived in. There is
//! no other rendering entry point, no `push`, no `insert` and no `Vec`, so a
//! headline component cannot be put above `오늘 실제 일정` or beside it:
//! there is nowhere to put one.
//!
//! That is the whole of section 25.2's `GPA나 streak를 hero metric으로 두지
//! 않는다`, and `no_gpa_or_streak_hero_component` is written against it as an
//! **absence claim proved by exhaustion**, never as a list of forbidden names:
//!
//! 1. [`HomeCard`]'s variants and [`crate::HomeGroup`]'s are compared as whole
//!    sets in both directions, read out of this crate's source. A ninth card of
//!    any name fails as an extra key, and a group with no card fails as a
//!    missing one.
//! 2. Every field position of every type in this crate's product source is
//!    compared, in both directions, against a reviewed inventory that says
//!    which of section 25.2's eight lines each one serves. A quantity added
//!    anywhere in this crate fails as an unreviewed position whatever it is
//!    named, in a module nobody predicted.
//! 3. The order and length of [`HomeScreen::sections`]'s output are compared
//!    against `HomeGroup::ALL` over a driven corpus, including corpora whose
//!    cards arrive in every rotation.
//! 4. `packages/ui/src/home.ts` holds the shell's eight and
//!    `no_gpa_or_streak_hero_component` there compares them against this
//!    crate's, in both directions and in order, so the ninth section cannot be
//!    added on the TypeScript side either.
//!
//! **What it is not.** It is not a claim that no number this screen shows was
//! computed from a grade average. Nothing here would notice that: a caller who
//! put a grade average into `EstimatedMinutes` would pass every check above.
//! What is refused is a *place to put one* — a card, a field, a section or a
//! slot — and the four comparisons are exhaustive over those.

use academic_domain::TimestampMillis;

use crate::{
    AlertBucket, DayWindow, FreshnessAlert, GroupedAlerts, HomeGroup, KnowledgeNeed, NextStep,
    OfficialCondition, OpenItem, PrerequisiteBrief, RecordingPermission, ScheduledItem,
};

/// One card on the home screen.
///
/// Exactly one variant per section 25.2 line, each carrying that line's own
/// payload type. There is no variant holding a free-form measure, a title, a
/// score or a number the caller chose the meaning of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeCard {
    /// Line one.
    TodaysSchedule(ScheduledItem),
    /// Line two.
    MinimumPrerequisite(PrerequisiteBrief),
    /// Line three.
    RecordingPermissionStatus(RecordingPermission),
    /// Line four.
    OpenQuestionAndMarkMoment(OpenItem),
    /// Line five.
    ProjectBlockingKnowledgeNeed(KnowledgeNeed),
    /// Line six.
    OfficialConditionAndStaleWarning(OfficialCondition),
    /// Line seven.
    CriticalPathNextStep(NextStep),
    /// Line eight.
    ConceptFreshnessAlert(FreshnessAlert),
}

impl HomeCard {
    /// Which of section 25.2's eight groups this card belongs to.
    ///
    /// A total `match` with no wildcard arm. A ninth variant has to answer this
    /// question, and the whole-set comparison in
    /// `no_gpa_or_streak_hero_component` is what refuses the answer.
    #[must_use]
    pub const fn group(&self) -> HomeGroup {
        match self {
            Self::TodaysSchedule(_) => HomeGroup::TodaysSchedule,
            Self::MinimumPrerequisite(_) => HomeGroup::MinimumPrerequisite,
            Self::RecordingPermissionStatus(_) => HomeGroup::RecordingPermissionStatus,
            Self::OpenQuestionAndMarkMoment(_) => HomeGroup::OpenQuestionAndMarkMoment,
            Self::ProjectBlockingKnowledgeNeed(_) => HomeGroup::ProjectBlockingKnowledgeNeed,
            Self::OfficialConditionAndStaleWarning(_) => {
                HomeGroup::OfficialConditionAndStaleWarning
            }
            Self::CriticalPathNextStep(_) => HomeGroup::CriticalPathNextStep,
            Self::ConceptFreshnessAlert(_) => HomeGroup::ConceptFreshnessAlert,
        }
    }

    /// The instant this card is answerable by, when it has one.
    ///
    /// A total `match`. Three of the eight carry a deadline and five do not,
    /// and the five answer `None` rather than being omitted, so a card whose
    /// group grew a deadline has to say so here.
    ///
    /// A prerequisite brief answers with the earliest occasion any of its items
    /// is offered for, because that is the first moment the brief stops being
    /// useful. `PrerequisiteBrief::assemble` refuses the empty brief, so the
    /// minimum is over a non-empty set.
    #[must_use]
    pub fn deadline(&self) -> Option<TimestampMillis> {
        match self {
            Self::TodaysSchedule(item) => Some(item.at()),
            Self::MinimumPrerequisite(brief) => {
                brief.items().iter().map(|item| item.why_now().at()).min()
            }
            Self::RecordingPermissionStatus(_)
            | Self::OpenQuestionAndMarkMoment(_)
            | Self::ProjectBlockingKnowledgeNeed(_)
            | Self::CriticalPathNextStep(_) => None,
            Self::OfficialConditionAndStaleWarning(condition) => condition.due(),
            Self::ConceptFreshnessAlert(alert) => Some(alert.upcoming().at()),
        }
    }
}

/// One of section 25.2's eight groups, with the cards that fell into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeSection<'screen> {
    group: HomeGroup,
    cards: Vec<&'screen HomeCard>,
}

impl<'screen> HomeSection<'screen> {
    /// Which group.
    #[must_use]
    pub const fn group(&self) -> HomeGroup {
        self.group
    }

    /// The cards in it, in the order they were composed.
    ///
    /// Possibly empty: a group with nothing to show is still a group, and
    /// dropping it would make the screen's shape depend on its contents.
    #[must_use]
    pub fn cards(&self) -> &[&'screen HomeCard] {
        &self.cards
    }
}

/// The home screen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeScreen {
    cards: Vec<HomeCard>,
}

impl HomeScreen {
    /// Composes a screen from whatever the surfaces below produced.
    #[must_use]
    pub fn compose(cards: Vec<HomeCard>) -> Self {
        Self { cards }
    }

    /// Everything on it, in composition order.
    #[must_use]
    pub fn cards(&self) -> &[HomeCard] {
        &self.cards
    }

    /// The eight sections, in section 25.2's order.
    ///
    /// The `i`th entry is `HomeGroup::ALL[i]` by construction. Composition
    /// order reaches the cards inside a section and never the sections
    /// themselves.
    #[must_use]
    pub fn sections(&self) -> [HomeSection<'_>; HomeGroup::COUNT] {
        HomeGroup::ALL.map(|group| HomeSection {
            group,
            cards: self
                .cards
                .iter()
                .filter(|card| card.group() == group)
                .collect(),
        })
    }

    /// The whole screen, bundled into section 25.2's three buckets.
    ///
    /// Every card, in exactly one bucket. Nothing on this path can shorten the
    /// screen: [`GroupedAlerts::group`] has no parameter that could.
    #[must_use]
    pub fn grouped(&self, window: DayWindow) -> GroupedAlerts {
        GroupedAlerts::group(self.cards.clone(), window)
    }

    /// Which bucket one card of this screen falls into.
    ///
    /// The same total function [`GroupedAlerts::group`] uses, exposed so a
    /// caller can label a card without regrouping the screen.
    #[must_use]
    pub fn bucket_of(card: &HomeCard, window: DayWindow) -> AlertBucket {
        AlertBucket::of(card.deadline(), window)
    }
}
