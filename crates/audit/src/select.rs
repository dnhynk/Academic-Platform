//! Section 11.1's `RuleSet` selection, fail-closed in both directions.
//!
//! *selector는 대학·단과대·학부·입학년도·사용자가 적법하게 선택한 졸업기준·
//! 주전공/복수/부/연합/연계·교환/편입·예외 승인을 함께 사용한다. 하나라도 필수
//! 입력이 없거나 두 RuleSet이 경쟁하면 임의 선택하지 않고 `INDETERMINATE`와
//! 필요한 확인 항목을 반환한다.*
//!
//! Both halves of the second sentence are here, and both produce a list rather
//! than a number:
//!
//! - **One required input absent.** Every unrecorded profile field is reported,
//!   not just the first, each naming its section 38 cell where it has one. The
//!   selector does not stop at the first gap, because a user who fixes one gap
//!   and meets the next one has been told the truth twice instead of once.
//! - **Two sets compete.** Every competing version is named and none is chosen.
//!   Nothing here reads a position out of the catalogue: `select` sorts nothing
//!   by order of publication, takes no `first`, and has no tie-break.
//!
//! # What a published set declares a scope for, and what it does not
//!
//! Section 11.1's yaml gives a `DegreeRequirementSet` four scope fields --
//! `institutionPath`, `admissionYear`, `selectedGraduationStandardRange` and
//! `majorMode` -- which between them cover the first six of the sentence's
//! eight inputs. It declares no field for 교환/편입 and none for 예외 승인.
//! Those two are therefore **required inputs that narrow nothing**: an
//! unrecorded one is `INDETERMINATE`, and a recorded one removes no candidate.
//! Inventing two scope fields the specification does not write would have made
//! `selector_dimension_matrix` look stronger than the document it comes from.
//! [`crate::profile::SelectorDimension::narrows_the_catalogue`] is that split,
//! and the matrix asserts both halves of it.

use academic_requirement::{
    AdmissionYear, CreditCategory, RuleBody, RuleId, RuleSet, RuleSetVersion,
};

use crate::{
    error::AuditError,
    profile::{DegreeMode, GraduationStandard, InstitutionId, ProfileField, StudentProfile},
    verdict::{IndeterminateVerdict, MissingCheck},
};

/// What one published requirement set declares it covers.
///
/// Section 11.1's yaml, field for field. Every field is required: a scope with
/// an absent field would admit every profile on that axis, which is the
/// arbitrary choice the selector exists to refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSetScope {
    university: InstitutionId,
    college: InstitutionId,
    department: InstitutionId,
    admission_year: AdmissionYear,
    standard_from: GraduationStandard,
    standard_to: GraduationStandard,
    major_mode: DegreeMode,
}

impl RuleSetScope {
    /// Declares a scope.
    ///
    /// The two ends of `selectedGraduationStandardRange` are compared as text,
    /// and the constructor refuses a range whose ends differ in length. Section
    /// 11.1 writes the range as two four-digit years, and a fixed-width numeric
    /// identifier orders lexicographically exactly as it orders numerically; a
    /// range with ends of different widths would not, and comparing it as text
    /// would silently admit or exclude the wrong cohorts.
    pub fn new(
        university: InstitutionId,
        college: InstitutionId,
        department: InstitutionId,
        admission_year: AdmissionYear,
        standard_from: GraduationStandard,
        standard_to: GraduationStandard,
        major_mode: DegreeMode,
    ) -> Result<Self, AuditError> {
        if standard_from.as_str().len() != standard_to.as_str().len() {
            return Err(AuditError::InvalidIdentifier {
                kind: "graduation standard range",
                value: format!("{standard_from}..{standard_to}"),
            });
        }
        if standard_from.as_str() > standard_to.as_str() {
            return Err(AuditError::InvalidIdentifier {
                kind: "graduation standard range",
                value: format!("{standard_from}..{standard_to}"),
            });
        }
        Ok(Self {
            university,
            college,
            department,
            admission_year,
            standard_from,
            standard_to,
            major_mode,
        })
    }

    /// The university this scope covers.
    #[must_use]
    pub const fn university(&self) -> &InstitutionId {
        &self.university
    }

    /// The college.
    #[must_use]
    pub const fn college(&self) -> &InstitutionId {
        &self.college
    }

    /// The department.
    #[must_use]
    pub const fn department(&self) -> &InstitutionId {
        &self.department
    }

    /// The admission year.
    #[must_use]
    pub const fn admission_year(&self) -> AdmissionYear {
        self.admission_year
    }

    /// The graduation-standard range, inclusive at both ends.
    #[must_use]
    pub const fn standard_range(&self) -> (&GraduationStandard, &GraduationStandard) {
        (&self.standard_from, &self.standard_to)
    }

    /// The degree mode.
    #[must_use]
    pub const fn major_mode(&self) -> DegreeMode {
        self.major_mode
    }

    /// Whether this scope covers a fully recorded profile.
    ///
    /// `None` when the profile is not fully recorded, which is a different
    /// answer from "does not cover": the selector never asks this question of
    /// an incomplete profile.
    #[must_use]
    pub fn covers(&self, profile: &StudentProfile) -> Option<bool> {
        let university = profile.university().known()?;
        let college = profile.college().known()?;
        let department = profile.department().known()?;
        let admission_year = *profile.admission_year().known()?;
        let standard = profile.graduation_standard().known()?;
        let mode = *profile.degree_mode().known()?;
        Some(
            university == &self.university
                && college == &self.college
                && department == &self.department
                && admission_year == self.admission_year
                && standard.as_str() >= self.standard_from.as_str()
                && standard.as_str() <= self.standard_to.as_str()
                && mode == self.major_mode,
        )
    }

    /// The scope rendered for a missing check and for the frozen inputs.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        format!(
            "{}/{}/{} year={} standard={}..{} mode={}",
            self.university,
            self.college,
            self.department,
            self.admission_year.get(),
            self.standard_from,
            self.standard_to,
            self.major_mode.as_str()
        )
    }
}

/// One published requirement set and the scope it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    scope: RuleSetScope,
    rules: RuleSet,
}

impl CatalogEntry {
    /// Pairs a scope with the set it scopes.
    #[must_use]
    pub const fn new(scope: RuleSetScope, rules: RuleSet) -> Self {
        Self { scope, rules }
    }

    /// The declared scope.
    #[must_use]
    pub const fn scope(&self) -> &RuleSetScope {
        &self.scope
    }

    /// The published set.
    #[must_use]
    pub const fn rules(&self) -> &RuleSet {
        &self.rules
    }
}

/// Every published requirement set a selection may choose between.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleSetCatalog {
    entries: Vec<CatalogEntry>,
}

impl RuleSetCatalog {
    /// An empty catalogue.
    ///
    /// An empty catalogue covers no profile, so every selection over it is
    /// `INDETERMINATE` with [`MissingCheck::NoRuleSetCovers`]. That is
    /// emptiness behaving as emptiness rather than as a default.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Adds one published set.
    #[must_use]
    pub fn with(mut self, entry: CatalogEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Every entry.
    #[must_use]
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }
}

/// A requirement set the selector chose, and the scope it matched.
///
/// Private fields and one construction site, inside [`select`]. An audit takes
/// this by value, so an audit over a set nobody selected is not a call that can
/// be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRuleSet {
    scope: RuleSetScope,
    rules: RuleSet,
}

impl SelectedRuleSet {
    /// The scope that matched.
    #[must_use]
    pub const fn scope(&self) -> &RuleSetScope {
        &self.scope
    }

    /// The published set.
    #[must_use]
    pub const fn rules(&self) -> &RuleSet {
        &self.rules
    }

    /// The published version.
    #[must_use]
    pub fn version(&self) -> RuleSetVersion {
        self.rules.version()
    }
}

/// What the selector concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Exactly one published set covers a fully recorded profile.
    ///
    /// Boxed because a selected set carries every published rule and an
    /// indeterminate verdict carries a short list, so an unboxed enum would be
    /// the size of a requirement set at every call site.
    Selected(Box<SelectedRuleSet>),
    /// A required input is absent, nothing covers the profile, or two sets do.
    Indeterminate(IndeterminateVerdict),
}

impl Selection {
    /// The selected set, when one was selected.
    #[must_use]
    pub fn selected(&self) -> Option<&SelectedRuleSet> {
        match self {
            Self::Selected(selected) => Some(selected.as_ref()),
            Self::Indeterminate(_) => None,
        }
    }

    /// The outstanding checks, which are empty exactly when a set was selected.
    #[must_use]
    pub fn missing(&self) -> &[MissingCheck] {
        match self {
            Self::Selected(_) => &[],
            Self::Indeterminate(verdict) => verdict.missing(),
        }
    }
}

/// Section 11.1's selector.
///
/// Reports **every** unrecorded profile field before it looks at the catalogue,
/// so a user is told the whole list rather than one gap at a time. Nothing
/// below reads a position out of the catalogue and nothing breaks a tie.
#[must_use]
pub fn select(profile: &StudentProfile, catalog: &RuleSetCatalog) -> Selection {
    let mut missing: Vec<MissingCheck> = Vec::new();
    for field in ProfileField::ALL {
        if !profile.is_recorded(field) {
            missing.push(MissingCheck::ProfileField {
                field,
                gate: field.gate(),
            });
        }
    }
    if let Some(verdict) = IndeterminateVerdict::from_checks(missing) {
        return Selection::Indeterminate(verdict);
    }

    let covering: Vec<&CatalogEntry> = catalog
        .entries()
        .iter()
        .filter(|entry| entry.scope().covers(profile) == Some(true))
        .collect();

    match covering.as_slice() {
        [] => Selection::Indeterminate(IndeterminateVerdict::new(
            MissingCheck::NoRuleSetCovers {
                rendered_profile: rendered_profile(profile),
            },
            Vec::new(),
        )),
        [only] => Selection::Selected(Box::new(SelectedRuleSet {
            scope: only.scope().clone(),
            rules: only.rules().clone(),
        })),
        competing => Selection::Indeterminate(IndeterminateVerdict::new(
            MissingCheck::CompetingRuleSets {
                versions: competing
                    .iter()
                    .map(|entry| entry.rules().version())
                    .collect(),
            },
            Vec::new(),
        )),
    }
}

/// One published credit floor, stated as a public fact about the programme.
///
/// Section 11.4 closes: *현재 사용자의 입학년도가 없으므로 이 문서는 130학점 등
/// 공개된 공통 사실을 예시로 사용할 뿐, 개인의 "남은 학점"을 산출하지 않는다.*
///
/// This type is that sentence. It carries the **threshold** and nothing else:
/// there is no attained figure, no remaining figure, no accessor for either,
/// and no constructor that takes a transcript. A remaining-credit number is not
/// a value this type can hold, so a screen that renders one cannot have got it
/// from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonRuleExample {
    rule: RuleId,
    category: CreditCategory,
    threshold: u16,
}

impl CommonRuleExample {
    /// The rule the floor belongs to.
    #[must_use]
    pub const fn rule(&self) -> &RuleId {
        &self.rule
    }

    /// The category it counts.
    #[must_use]
    pub const fn category(&self) -> &CreditCategory {
        &self.category
    }

    /// The published threshold. A public fact about the programme.
    #[must_use]
    pub const fn threshold(&self) -> u16 {
        self.threshold
    }
}

/// Every published credit floor of one set, as a non-personalized example.
///
/// The label is not decoration: it is what section 11.4 requires a reader to
/// see instead of a personal figure. [`CommonRuleExamples::LABEL`] is the
/// spelling, and there is no arm of this type that is personalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonRuleExamples {
    floors: Vec<CommonRuleExample>,
}

impl CommonRuleExamples {
    /// The label every rendering of this value carries.
    pub const LABEL: &'static str = "NOT_PERSONALIZED";

    /// Reads the published floors out of a rule set.
    ///
    /// It takes a [`RuleSet`] and **not** a [`SelectedRuleSet`]: a public
    /// common fact is readable whether or not a set was selected for this user,
    /// and that is the whole point of the sentence it comes from. It takes no
    /// transcript, so there is nothing here to subtract a personal total from.
    pub fn of(rules: &RuleSet) -> Result<Self, AuditError> {
        let mut floors = Vec::new();
        for (rule, body) in rules.rules() {
            if let RuleBody::CreditMinimum {
                category,
                threshold,
            } = body
            {
                floors.push(CommonRuleExample {
                    rule: rule.clone(),
                    category: category.clone(),
                    threshold: threshold.get(),
                });
            }
        }
        Ok(Self { floors })
    }

    /// Every published floor.
    #[must_use]
    pub fn floors(&self) -> &[CommonRuleExample] {
        &self.floors
    }
}

fn rendered_profile(profile: &StudentProfile) -> String {
    let mode = profile
        .degree_mode()
        .known()
        .map_or_else(|| "UNKNOWN".to_owned(), |mode| mode.as_str().to_owned());
    let year = profile
        .admission_year()
        .known()
        .map_or_else(|| "UNKNOWN".to_owned(), |year| year.get().to_string());
    format!(
        "{}/{}/{} year={year} standard={} mode={mode}",
        render(profile.university().known()),
        render(profile.college().known()),
        render(profile.department().known()),
        profile
            .graduation_standard()
            .known()
            .map_or_else(|| "UNKNOWN".to_owned(), ToString::to_string),
    )
}

fn render(value: Option<&InstitutionId>) -> String {
    value.map_or_else(|| "UNKNOWN".to_owned(), ToString::to_string)
}
