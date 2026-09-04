//! Favouriting a role, and the four things it is not.
//!
//! Section 25.11: `role을 즐겨찾기해도 “진로 확정”으로 간주하지 않는다.`
//! Section 37: `과거 Backend path를 실패로 표시하지 않는다` and
//! `관심이 없으면 다시 neutral 상태로 둘 수 있다`.
//!
//! ## The vocabulary is three words, and the two that are missing are named
//!
//! [`InterestStanding`] has exactly the standings those two sections spell:
//! favourited, exploring, neutral. It has **no** arm meaning the user has
//! chosen this career and **no** arm meaning a path failed, and
//! [`REFUSED_STANDINGS`] carries both refusals with the sentence each comes
//! from. `favoriting_a_role_is_not_a_career_decision` reads those sentences out
//! of the specification and compares the standings in both directions, so a
//! fourth arm cannot arrive without the specification saying so first.
//!
//! This is `P2-N2`'s shape one stage over: `AutomaticLevel` has no `Fluent`,
//! because the level nothing may promote to is held by the absence of the value
//! rather than by a check somebody could forget. And it is `P2-U5`'s: a
//! prediction cannot be promoted to an official claim because there is no
//! constructor that takes one.
//!
//! ## An interest reaches nothing
//!
//! A [`RoleInterest`] holds a [`crate::RoleProfileId`] and a standing. Not a
//! version — so it does not even select which bundle is in force — and not a
//! competency, a weight, a plan, a goal or a date.
//!
//! Nothing in this crate takes one as an argument. The only functions whose
//! signatures mention [`RoleInterest`] are its own four functions, and
//! `an_interest_is_not_an_input_to_anything` compares that set against every
//! public signature in the package in both directions. In particular
//! [`crate::fork`] — the one act section 24.2 says a user performs on a bundle
//! — takes a `&RoleProfile`, so an interest cannot be forked into a bundle, and
//! `crates/role-profile/tests/compile_fail/` holds the compiled half.
//!
//! ## Changing your mind rewrites nothing
//!
//! [`RoleInterest::standing_now`] consumes and returns a new value rather than
//! taking `&mut self`. The value it was made from is still whatever it was, so
//! moving from favourited to neutral is not a rewrite of the past — which is
//! section 37's `과거 Backend path를 실패로 표시하지 않는다` at the smallest
//! scale this crate can hold it at.

use serde::{Deserialize, Serialize};

use crate::identity::RoleProfileId;

/// The standings this crate refuses, and the sentence that refuses each.
///
/// A record rather than a check: there is no code path that could produce
/// either, because neither is an arm of [`InterestStanding`].
pub const REFUSED_STANDINGS: [(&str, &str); 2] = [
    (
        "CHOSEN",
        "role을 즐겨찾기해도 “진로 확정”으로 간주하지 않는다",
    ),
    ("FAILED", "과거 Backend path를 실패로 표시하지 않는다"),
];

/// How a user stands towards a role bundle.
///
/// Three arms, one per standing sections 25.11 and 37 name. See
/// [`REFUSED_STANDINGS`] for the two that are deliberately absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterestStanding {
    /// Section 25.11's `즐겨찾기`. The user marked it; that is all it means.
    Favorited,
    /// Section 37's `탐색`. The user is looking at it, possibly beside another.
    Exploring,
    /// Section 37's `다시 neutral 상태`. Not a rejection and not a failure.
    Neutral,
}

impl InterestStanding {
    /// Exhaustive, in the order the two sections introduce them.
    pub const ALL: [Self; 3] = [Self::Favorited, Self::Exploring, Self::Neutral];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Favorited => "FAVORITED",
            Self::Exploring => "EXPLORING",
            Self::Neutral => "NEUTRAL",
        }
    }
}

/// How a user stands towards one role lineage.
///
/// It names a [`RoleProfileId`] and nothing finer: an interest does not pick a
/// version, so it cannot be read as *this bundle is in force*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleInterest {
    profile: RoleProfileId,
    standing: InterestStanding,
}

impl RoleInterest {
    /// Records how the user stands towards one lineage.
    #[must_use]
    pub const fn in_role(profile: RoleProfileId, standing: InterestStanding) -> Self {
        Self { profile, standing }
    }

    /// Which lineage.
    #[must_use]
    pub const fn profile(&self) -> &RoleProfileId {
        &self.profile
    }

    /// The standing.
    #[must_use]
    pub const fn standing(&self) -> InterestStanding {
        self.standing
    }

    /// The same interest at a different standing, as a new value.
    ///
    /// Consuming rather than `&mut self`: what this was made from is unchanged,
    /// so a move to [`InterestStanding::Neutral`] does not rewrite the standing
    /// that came before it.
    #[must_use]
    pub fn standing_now(self, standing: InterestStanding) -> Self {
        Self {
            profile: self.profile,
            standing,
        }
    }
}
