//! Section 17.6's first, second, fourth and fifth bullets, as the one door from
//! what a connector reports into what a personal claim may rest on.
//!
//! ```text
//! ContributionRecord   what a version-control connector reports
//!         │            author string, kind, origin, changed sites
//!         ▼
//!   ContributionDraft  the door: the mapping, the rubric, the warrant
//!         │
//!         ▼
//!     AuthoredWork     the user's own meaningful change, sealed
//! ```
//!
//! [`AuthoredWork`] has private fields, no [`Default`] and one producer,
//! [`ContributionDraft::seal`], so there is no eligible contribution anywhere in
//! a program that did not pass every check. Past that door there is nothing to
//! guard, because there is no half-eligible contribution.
//!
//! ## Read versus authored is a type boundary, not a comparison
//!
//! Section 17.6's fourth bullet is `다른 사람이 작성한 code를 읽은 것인지 직접
//! 구현한 것인지`. A connector can report four things a person did; a personal
//! claim can serialize two.
//!
//! | [`ContributionKind`] | [`AuthorshipMode`] |
//! |---|---|
//! | `AUTHORED` | `AUTHORED` |
//! | `MODIFIED` | `SUBSTANTIVE_CONTRIBUTION` |
//! | `REVIEWED` | — |
//! | `READ` | — |
//!
//! [`ContributionKind::authorship_mode`] is that table, it is the **only**
//! function anywhere that produces an [`AuthorshipMode`] from anything, and it
//! is total over its own enumeration with no default arm. [`AuthorshipMode`]
//! itself has no `Reviewed` and no `Read` variant, so a review has no spelling
//! in the field a claim puts its authorship in — not one that is rejected at
//! runtime, one that does not exist. `review_is_never_serialized_as_authored`
//! walks the whole of [`ContributionKind::ALL`] and the compile-fail suite holds
//! the half that is a program that does not compile.
//!
//! That is the mechanism `P2-U1` used for a field with no setter, `P2-U2` for a
//! gate that is a type, `P2-L4` for a constructor that does not exist, and
//! `P2-R4` for a chain step that cannot be skipped.

use academic_repository_analysis::Locator;

use crate::{
    CompetencyError,
    generated::{CodeOrigin, GeneratedCodeWarrant, OriginReport, WarrantStep},
    identity::{AuthorshipMap, ExternalAuthorId, UserId, validated},
    rubric::{ChangeVerdict, ChangedSite, ScaffoldRubric},
};

/// Names one change: a commit, a patch series, a pull request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChangeId {
    identifier: String,
}

impl ChangeId {
    /// Validates and takes a change identifier.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when it is empty, over 64 bytes,
    /// or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self {
            identifier: validated(value.into(), "change")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// What a person did to a change, as a connector reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContributionKind {
    /// Wrote it.
    Authored,
    /// Changed something that was already there.
    Modified,
    /// Read somebody else's change and said something about it.
    Reviewed,
    /// Read it.
    Read,
}

impl ContributionKind {
    /// Exhaustive order, strongest first.
    pub const ALL: [Self; 4] = [Self::Authored, Self::Modified, Self::Reviewed, Self::Read];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authored => "AUTHORED",
            Self::Modified => "MODIFIED",
            Self::Reviewed => "REVIEWED",
            Self::Read => "READ",
        }
    }

    /// What a personal claim may serialize this as, when it may serialize it.
    ///
    /// The one producer of an [`AuthorshipMode`] in this crate, total over the
    /// enumeration with no default arm, so a fifth kind added later has no arm
    /// and the crate stops compiling rather than defaulting to authorship.
    /// [`None`] for `REVIEWED` and `READ` is section 17.6's fourth bullet, and
    /// it is a value that does not exist rather than a comparison somebody
    /// performs.
    #[must_use]
    pub const fn authorship_mode(self) -> Option<AuthorshipMode> {
        match self {
            Self::Authored => Some(AuthorshipMode::Authored),
            Self::Modified => Some(AuthorshipMode::SubstantiveContribution),
            Self::Reviewed | Self::Read => None,
        }
    }
}

/// What a personal claim says the user did.
///
/// Two values. There is no review here and no reading here; see the module
/// documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthorshipMode {
    /// Section 17.6's `직접 구현`.
    Authored,
    /// Section 17.6's `실질적 기여`.
    SubstantiveContribution,
}

impl AuthorshipMode {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Authored, Self::SubstantiveContribution];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authored => "AUTHORED",
            Self::SubstantiveContribution => "SUBSTANTIVE_CONTRIBUTION",
        }
    }
}

/// What a version-control connector reports about one change.
///
/// Public fields, the way `P2-R3`'s `CorrelationInput` and `P2-R4`'s
/// `ClassificationInput` have them: this is an argument list and every field is
/// required. Nothing here is trusted — it is the input a draft judges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionRecord {
    /// Which change.
    pub change: ChangeId,
    /// Which snapshot the change is being read against.
    pub snapshot_id: String,
    /// The author string the version-control system carries.
    pub author: ExternalAuthorId,
    /// What that person did.
    pub kind: ContributionKind,
    /// Where the code came from, before any warrant exists.
    pub origin: OriginReport,
    /// Every place the change touched, with what kind of edit each was.
    pub sites: Vec<ChangedSite>,
    /// When the connector recorded it.
    pub recorded_at: u64,
}

/// The one door from a report into an eligible contribution.
///
/// Section 17.6's `다음을 별도로 확인한다` needs a place where the checks are
/// applied, and this is it. [`ContributionDraft::seal`] names the **first**
/// failing check as a [`crate::PromotionCheck`]-bearing refusal.
#[derive(Debug)]
pub struct ContributionDraft<'a> {
    record: &'a ContributionRecord,
    map: &'a AuthorshipMap,
    rubric: &'a ScaffoldRubric,
    warrant: Option<GeneratedCodeWarrant>,
}

impl<'a> ContributionDraft<'a> {
    /// Opens a draft over one report, one mapping and one rubric.
    #[must_use]
    pub const fn over(
        record: &'a ContributionRecord,
        map: &'a AuthorshipMap,
        rubric: &'a ScaffoldRubric,
    ) -> Self {
        Self {
            record,
            map,
            rubric,
            warrant: None,
        }
    }

    /// Offers the warrant section 17.6's fifth bullet asks for.
    ///
    /// Takes a whole [`GeneratedCodeWarrant`], which cannot be built without
    /// all three of its steps. A draft over generated code with no warrant is
    /// refused by [`Self::seal`]; a draft over a *partial* warrant is a program
    /// that does not compile.
    #[must_use]
    pub fn warranted_by(mut self, warrant: GeneratedCodeWarrant) -> Self {
        self.warrant = Some(warrant);
        self
    }

    /// Applies section 17.6's checks and seals an eligible contribution.
    ///
    /// # Errors
    ///
    /// In section 17.6's own bullet order:
    /// [`CompetencyError::AuthorIsNotTheUser`] when the mapping does not
    /// resolve the author string;
    /// [`CompetencyError::ContributionIsNotAuthorship`] when the kind has no
    /// [`AuthorshipMode`];
    /// [`CompetencyError::ChangeIsScaffoldOnly`] when the rubric says so; and
    /// [`CompetencyError::GeneratedCodeHasNoWarrant`] when the report says the
    /// code was generated and no warrant was offered.
    pub fn seal(self) -> Result<AuthoredWork, CompetencyError> {
        let record = self.record;
        // Bullet 1: `사용자 authorship 또는 실질적 기여`. Whose change is it.
        let Some(user) = self.map.resolve(&record.author) else {
            return Err(CompetencyError::AuthorIsNotTheUser {
                change: record.change.as_str().to_owned(),
                namespace: record.author.source().as_str(),
                mapping_version: self.map.version(),
            });
        };
        // Bullet 4: `읽은 것인지 직접 구현한 것인지`. The one door to a mode.
        let Some(mode) = record.kind.authorship_mode() else {
            return Err(CompetencyError::ContributionIsNotAuthorship {
                change: record.change.as_str().to_owned(),
                kind: record.kind,
            });
        };
        // Bullet 2: `단순 scaffold가 아닌 이해가 필요한 선택·수정`.
        let verdict = self.rubric.judge(&record.sites);
        if let ChangeVerdict::ScaffoldOnly {
            rubric,
            version,
            bearing_sites,
            required,
        } = &verdict
        {
            return Err(CompetencyError::ChangeIsScaffoldOnly {
                change: record.change.as_str().to_owned(),
                rubric: rubric.as_str().to_owned(),
                version: *version,
                bearing_sites: *bearing_sites,
                required: *required,
            });
        }
        // Bullet 5: `생성형 AI가 작성한 code라면 사용자가 검증·수정·설명했는지`.
        let origin = match (record.origin, self.warrant) {
            (OriginReport::HandWritten, _) => CodeOrigin::HandWritten,
            (OriginReport::Generated, Some(warrant)) => CodeOrigin::Generated(warrant),
            (OriginReport::Generated, None) => {
                return Err(CompetencyError::GeneratedCodeHasNoWarrant {
                    change: record.change.as_str().to_owned(),
                    first_missing: WarrantStep::Verified,
                });
            }
        };
        Ok(AuthoredWork {
            change: record.change.clone(),
            snapshot_id: record.snapshot_id.clone(),
            user: user.clone(),
            author: record.author.clone(),
            mapping_version: self.map.version(),
            mode,
            origin,
            verdict,
            recorded_at: record.recorded_at,
        })
    }
}

/// The user's own meaningful change over one snapshot: section 17.6's first,
/// second, fourth and fifth bullets, all four held.
///
/// Private fields, no [`Default`], one producer. Every value of this type
/// anywhere in a program passed all four.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredWork {
    change: ChangeId,
    snapshot_id: String,
    user: UserId,
    author: ExternalAuthorId,
    mapping_version: u64,
    mode: AuthorshipMode,
    origin: CodeOrigin,
    verdict: ChangeVerdict,
    recorded_at: u64,
}

impl AuthoredWork {
    /// Which change.
    #[must_use]
    pub const fn change(&self) -> &ChangeId {
        &self.change
    }

    /// Which snapshot it was read against.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Whose it is, by this system's own identifier.
    #[must_use]
    pub const fn user(&self) -> &UserId {
        &self.user
    }

    /// The external identity the mapping resolved.
    #[must_use]
    pub const fn author(&self) -> &ExternalAuthorId {
        &self.author
    }

    /// Which version of the mapping admitted it.
    #[must_use]
    pub const fn mapping_version(&self) -> u64 {
        self.mapping_version
    }

    /// What a claim serializes this as. Never a review; see the module
    /// documentation.
    #[must_use]
    pub const fn mode(&self) -> AuthorshipMode {
        self.mode
    }

    /// Where the code came from, with the warrant when it was generated.
    #[must_use]
    pub const fn origin(&self) -> &CodeOrigin {
        &self.origin
    }

    /// The rubric's answer, always `MEANINGFUL`, with the rubric and version
    /// that gave it.
    #[must_use]
    pub const fn verdict(&self) -> &ChangeVerdict {
        &self.verdict
    }

    /// The sites the rubric counted as bearing understanding.
    #[must_use]
    pub fn bearing_sites(&self) -> &[ChangedSite] {
        self.verdict.bearing_sites()
    }

    /// When the connector recorded it.
    #[must_use]
    pub const fn recorded_at(&self) -> u64 {
        self.recorded_at
    }

    /// Whether this work touched the place an observation names.
    ///
    /// A change and an observed use meet at a **site**, not at a concept name:
    /// authoring anything in a repository that observes a concept is not
    /// authoring that concept's use, which is the whole of
    /// `repo_use_alone_creates_no_personal_claim` one level in.
    ///
    /// The comparison is `P2-R2`'s own: the paths must be equal, and then the
    /// two must be at the same granularity — both inside the same declaration,
    /// by [`academic_repository_analysis::SymbolFingerprint`], or both outside
    /// every declaration. It is span-independent for `P2-R4`'s locator-migration
    /// reason: an edit above a declaration moves its span and leaves its
    /// fingerprint alone.
    #[must_use]
    pub fn touches(&self, observed: &[Locator]) -> bool {
        self.bearing_sites().iter().any(|site| {
            observed
                .iter()
                .any(|locator| sites_meet(site.locator(), locator))
        })
    }
}

/// Whether a changed site and an observed locator are the same place.
///
/// The path has to be equal, and then the two have to be at the same
/// **granularity**: a site inside a declaration meets a locator inside the same
/// declaration, and a site outside every declaration — a manifest row, a
/// configuration key, a module-level import — meets a locator outside every
/// declaration.
///
/// The last arm is why the pair is compared rather than the path alone. A
/// symbol-bearing change matched against a symbol-less locator at the same path
/// would make *any* edit inside *any* declaration of a file meet a use recorded
/// at that file's import line, so editing an unrelated function in a file that
/// happens to import a library would credit the user with that library's use.
/// `a_work_meets_an_observation_by_fingerprint_before_by_path` measured exactly
/// that: an edit inside a differently named declaration at the same path met
/// the observation while this function read the path in that case.
fn sites_meet(changed: &Locator, observed: &Locator) -> bool {
    if changed.path() != observed.path() {
        return false;
    }
    match (changed.symbol(), observed.symbol()) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}
