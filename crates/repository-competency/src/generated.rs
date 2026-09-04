//! Section 17.6's fifth bullet: `생성형 AI가 작성한 code라면 사용자가
//! 검증·수정·설명했는지`.
//!
//! Three verbs, in that order, and none of them optional. So there are three
//! types, each taking the previous **by value**:
//!
//! ```text
//! verified  →  modified  →  explained
//! ```
//!
//! [`ModifiedByUser::after`] takes a [`VerifiedByUser`],
//! [`ExplainedByUser::after`] takes a [`ModifiedByUser`], and
//! [`GeneratedCodeWarrant::sealed`] takes an [`ExplainedByUser`]. None of the
//! four has a public field, a [`Default`], or a second constructor, so a
//! warrant over code the user verified and explained but never modified is not
//! a value that validates badly — it is a program that does not compile.
//! `crates/repository-competency/tests/compile_fail/` holds the committed
//! diagnostics.
//!
//! That is `P2-R4`'s five-step chain applied to a three-step one, and the
//! reason is the same: **an absence is stronger than a check**, because nothing
//! has to remember to run it.
//!
//! ## Why the warrant is a variant payload rather than a field beside the
//! origin
//!
//! [`CodeOrigin::Generated`] **holds** the warrant. Generated code with no
//! warrant therefore has no representation at all, so the question *is there a
//! warrant for this generated code* has no place to be asked and no place to be
//! forgotten. The runtime half — a connector that reported generated code and
//! offered no warrant — is refused one layer out, at
//! [`crate::ContributionDraft::seal`], with its own code.
//!
//! ## What each step has to carry
//!
//! Each takes a locator, so `사용자가 검증했다` is a claim about a place in the
//! snapshot rather than a boolean. The middle step additionally requires the
//! user's own [`crate::ChangedSite`]s, because *modified* is exactly the thing
//! the fourth test name calls out: **unmodified** generated code creates no
//! applied claim.

use academic_repository_analysis::Locator;

use crate::{CompetencyError, rubric::ChangedSite};

/// Which of the three the user had not done.
///
/// The wire spelling is the missing-step code a blocked promotion carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarrantStep {
    /// `검증`: the user checked that it does what it claims.
    Verified,
    /// `수정`: the user changed it.
    Modified,
    /// `설명`: the user wrote down why it is what it is.
    Explained,
}

impl WarrantStep {
    /// Exhaustive order, in section 17.6's own order.
    pub const ALL: [Self; 3] = [Self::Verified, Self::Modified, Self::Explained];

    /// The missing-step code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "MISSING_GENERATED_CODE_VERIFICATION",
            Self::Modified => "MISSING_GENERATED_CODE_MODIFICATION",
            Self::Explained => "MISSING_GENERATED_CODE_EXPLANATION",
        }
    }
}

/// Step one: the user checked the generated code against something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedByUser {
    at: Vec<Locator>,
    note: String,
}

impl VerifiedByUser {
    /// Records a verification over at least one site.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::WarrantStepHasNoSite`] when no locator is given, and
    /// [`CompetencyError::WarrantStepHasNoNote`] when the note is empty.
    pub fn at(sites: Vec<Locator>, note: impl Into<String>) -> Result<Self, CompetencyError> {
        let note = note.into();
        if sites.is_empty() {
            return Err(CompetencyError::WarrantStepHasNoSite(WarrantStep::Verified));
        }
        if note.trim().is_empty() {
            return Err(CompetencyError::WarrantStepHasNoNote(WarrantStep::Verified));
        }
        Ok(Self { at: sites, note })
    }

    /// Where the verification was done.
    #[must_use]
    pub fn sites(&self) -> &[Locator] {
        &self.at
    }

    /// What the user said they checked.
    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }
}

/// Step two: the user changed the generated code.
///
/// Takes step one **by value**. The changed sites are the user's own, so
/// *modified* is a set of places rather than a flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifiedByUser {
    verified: VerifiedByUser,
    edits: Vec<ChangedSite>,
}

impl ModifiedByUser {
    /// Records the edits the user made on top of a verification.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::WarrantStepHasNoSite`] when no edit is given.
    pub fn after(
        verified: VerifiedByUser,
        edits: Vec<ChangedSite>,
    ) -> Result<Self, CompetencyError> {
        if edits.is_empty() {
            return Err(CompetencyError::WarrantStepHasNoSite(WarrantStep::Modified));
        }
        Ok(Self { verified, edits })
    }

    /// Step one, carried.
    #[must_use]
    pub const fn verified(&self) -> &VerifiedByUser {
        &self.verified
    }

    /// What the user changed.
    #[must_use]
    pub fn edits(&self) -> &[ChangedSite] {
        &self.edits
    }
}

/// Step three: the user wrote down why the code is what it is.
///
/// Takes step two **by value**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainedByUser {
    modified: ModifiedByUser,
    explanation: String,
}

impl ExplainedByUser {
    /// Records an explanation on top of a modification.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::WarrantStepHasNoNote`] when the explanation is empty.
    pub fn after(
        modified: ModifiedByUser,
        explanation: impl Into<String>,
    ) -> Result<Self, CompetencyError> {
        let explanation = explanation.into();
        if explanation.trim().is_empty() {
            return Err(CompetencyError::WarrantStepHasNoNote(
                WarrantStep::Explained,
            ));
        }
        Ok(Self {
            modified,
            explanation,
        })
    }

    /// Step two, carried.
    #[must_use]
    pub const fn modified(&self) -> &ModifiedByUser {
        &self.modified
    }

    /// What the user wrote.
    #[must_use]
    pub fn explanation(&self) -> &str {
        &self.explanation
    }
}

/// All three, sealed.
///
/// Takes step three **by value**, and there is no other constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCodeWarrant {
    explained: ExplainedByUser,
}

impl GeneratedCodeWarrant {
    /// Seals a complete warrant.
    #[must_use]
    pub const fn sealed(explained: ExplainedByUser) -> Self {
        Self { explained }
    }

    /// The whole chain, from its last step.
    #[must_use]
    pub const fn explained(&self) -> &ExplainedByUser {
        &self.explained
    }

    /// Step two, for a reader that wants what the user changed.
    #[must_use]
    pub const fn modified(&self) -> &ModifiedByUser {
        self.explained.modified()
    }

    /// Step one, for a reader that wants what the user checked.
    #[must_use]
    pub const fn verified(&self) -> &VerifiedByUser {
        self.explained.modified().verified()
    }
}

/// Where the code under a contribution came from.
///
/// `Generated` **holds** the warrant, so generated code with no warrant has no
/// representation. See the module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeOrigin {
    /// The user wrote it.
    HandWritten,
    /// A model wrote it and the user verified, modified and explained it.
    Generated(GeneratedCodeWarrant),
}

impl CodeOrigin {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::HandWritten => "HAND_WRITTEN",
            Self::Generated(_) => "GENERATED",
        }
    }

    /// The warrant, when the code was generated.
    #[must_use]
    pub const fn warrant(&self) -> Option<&GeneratedCodeWarrant> {
        match self {
            Self::Generated(warrant) => Some(warrant),
            Self::HandWritten => None,
        }
    }
}

/// What a connector reports about where the code came from.
///
/// The untyped half, before a warrant exists. This is what a
/// [`crate::ContributionRecord`] carries and it is deliberately **not**
/// [`CodeOrigin`]: a report is a claim about provenance, and turning one into
/// an origin is what [`crate::ContributionDraft::seal`] does or refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OriginReport {
    /// The user wrote it.
    HandWritten,
    /// A model wrote it.
    Generated,
}

impl OriginReport {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::HandWritten, Self::Generated];

    /// Stable spelling, which is [`CodeOrigin`]'s own.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HandWritten => "HAND_WRITTEN",
            Self::Generated => "GENERATED",
        }
    }
}
