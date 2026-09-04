//! Section 23's `압박을 막는 UX`, as the two things a finding may say.
//!
//! ## Every string here is the design document's own
//!
//! [`headline`] and [`NOT_A_JUDGEMENT_OF_ABILITY`] are read out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, and
//! `low_relevance_uses_neutral_tokens` compares the **whole set** of strings a
//! [`NeutralPresentation`] can render against the document in both directions:
//! six values, each required to occur verbatim in the document, and the document
//! required to yield exactly those six. A copy edit that demands an action is
//! then refused for a reason that does not depend on its wording — it is a
//! sentence the design document does not contain — which a forbidden-word list
//! cannot do, because a list refuses the spellings somebody thought of and
//! admits every other one.
//!
//! ## There is no warning
//!
//! Section 23: `관련성이 낮은 Blind Spot은 warning red가 아니라 중립 outline으로
//! 표시한다`, and section 34.5's prevention column for the whole failure mode is
//! `neutral UI`. So this crate has **no name for a warning presentation**: there
//! is no `WarningRed`, no severity, no alert level, and [`EMPHASIS`] is one
//! token. A red blind spot would need a vocabulary that is not here, which is
//! the same shape `P2-N3` used to make a mastery unreachable from a decay.
//!
//! ## There is no action slot
//!
//! Section 23's last sentence: when a blind spot is unrelated to the user's
//! goals, `행동 요구를 만들지 않는다`. [`NeutralPresentation`] therefore has no
//! field an action could occupy. The only presentation that carries one is
//! [`FindingPresentation::Explore`], and its [`crate::taste::TastePath`] is
//! reachable only from the user's own verified `EXPLORE` choice — so what a
//! finding may ask of the user is decided by the type it holds, not by a check
//! at the end of a function.

use serde::{Deserialize, Serialize};

use crate::{
    relevance::GoalRelevance,
    state::{BLIND_SPOT_STATES, BlindSpotState},
    taste::TastePath,
};

/// Section 23's `warning red가 아니라 중립 outline`, as the one token this crate
/// emits.
pub const EMPHASIS: &str = "NEUTRAL_OUTLINE";

/// Section 23's `"약하다" 대신 "판단할 exposure가 없다"고 쓴다`, right half.
pub const CANNOT_INFER_ABILITY: &str = "판단할 exposure가 없다";

/// The same bullet's left half — the claim about the person this replaces.
///
/// It is here so `unobserved_says_cannot_infer_ability` can require that no
/// string this crate emits is it, and so the pair is one measurement of one
/// sentence rather than two constants that could drift apart.
pub const CLAIM_ABOUT_THE_PERSON: &str = "약하다";

/// Section 34.5's `불확실성 표시` cell for this failure mode.
pub const NOT_A_JUDGEMENT_OF_ABILITY: &str = "실력 판단 불가";

/// What a finding says about one state.
///
/// Total, with no wildcard arm. `UNOBSERVED` says section 23's replacement
/// phrase; every other state says the design document's own cell for it.
#[must_use]
pub const fn headline(state: BlindSpotState) -> &'static str {
    match state {
        BlindSpotState::Unobserved => CANNOT_INFER_ABILITY,
        BlindSpotState::Weak
        | BlindSpotState::Stale
        | BlindSpotState::OutOfScope
        | BlindSpotState::Gap => state.meaning(),
    }
}

/// Every string a [`NeutralPresentation`] can render.
///
/// Five headlines and the uncertainty phrase they are all shown beside.
#[must_use]
pub fn renderable_copy() -> Vec<&'static str> {
    let mut found: Vec<&'static str> = BLIND_SPOT_STATES
        .iter()
        .map(|state| headline(*state))
        .collect();
    found.push(NOT_A_JUDGEMENT_OF_ABILITY);
    found.sort_unstable();
    found.dedup();
    found
}

/// What a finding shows. No action slot; see the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeutralPresentation {
    state: BlindSpotState,
    relevance: GoalRelevance,
}

impl NeutralPresentation {
    /// Assembles the presentation for `state` under `relevance`.
    #[must_use]
    pub const fn of(state: BlindSpotState, relevance: GoalRelevance) -> Self {
        Self { state, relevance }
    }

    /// Which state.
    #[must_use]
    pub const fn state(&self) -> BlindSpotState {
        self.state
    }

    /// The one emphasis token this crate emits.
    #[must_use]
    pub const fn emphasis(&self) -> &'static str {
        EMPHASIS
    }

    /// What it says about the state.
    #[must_use]
    pub const fn headline(&self) -> &'static str {
        headline(self.state)
    }

    /// Section 34.5's uncertainty phrase, shown with every finding.
    #[must_use]
    pub const fn uncertainty(&self) -> &'static str {
        NOT_A_JUDGEMENT_OF_ABILITY
    }

    /// Section 34.5's `goal relevance`, the other half of the same cell.
    #[must_use]
    pub const fn relevance(&self) -> &GoalRelevance {
        &self.relevance
    }
}

/// What one finding presents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingPresentation {
    /// The default and the only one this engine can reach on its own.
    Neutral {
        /// The copy.
        presentation: NeutralPresentation,
    },
    /// The user pressed `EXPLORE`, so exactly one bounded step exists.
    Explore {
        /// The same copy.
        presentation: NeutralPresentation,
        /// Section 23's `작은 taste path`, of exactly one step.
        path: TastePath,
    },
}

impl FindingPresentation {
    /// The copy, whichever variant this is.
    #[must_use]
    pub const fn presentation(&self) -> &NeutralPresentation {
        match self {
            Self::Neutral { presentation } | Self::Explore { presentation, .. } => presentation,
        }
    }

    /// The taste path, which only the `EXPLORE` variant has.
    #[must_use]
    pub const fn path(&self) -> Option<&TastePath> {
        match self {
            Self::Neutral { .. } => None,
            Self::Explore { path, .. } => Some(path),
        }
    }
}
