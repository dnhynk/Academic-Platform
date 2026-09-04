//! Section 3's `StudentProfile`, at the granularity section 11.1's selector
//! reads it.
//!
//! # Absence is a value, and it is spelled `UNKNOWN`
//!
//! Section 3: *알 수 없는 필드는 빈 문자열이 아니라 `UNKNOWN`으로 저장한다. 이
//! 상태에서도 전체 OS는 작동하지만 졸업 판정은 `INDETERMINATE`이며, 임의의
//! 학번을 선택해 결과를 확정하지 않는다.*
//!
//! [`Recorded`] is that sentence as a type. It has no `Default`, no
//! `unwrap_or`, no `From<T>` that fills one in, and [`StudentProfile::unrecorded`]
//! starts every field [`Recorded::Unknown`]. A profile field is set by calling
//! its `with_` method and by nothing else, so an unrecorded field is a state
//! the caller had to *not* reach rather than one it had to opt out of.
//!
//! # The eight dimensions are section 11.1's own
//!
//! Section 11.1: *selector는
//! 대학·단과대·학부·입학년도·사용자가 적법하게 선택한 졸업기준·주전공/복수/부/
//! 연합/연계·교환/편입·예외 승인을 함께 사용한다.*
//!
//! [`SelectorDimension`] holds each `·`-delimited unit of that sentence in
//! [`SelectorDimension::spec_words`], and
//! `the_selector_dimensions_are_the_specifications_own` splits the sentence
//! out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares
//! the two lists in both directions. **Nothing here asserts how many there
//! are.** `t001`'s `REQ-11-002`, derived from the specification without
//! reference to `t068`, lists the same units in the same order in English, and
//! that agreement is what the comparison rests on rather than a number written
//! twice.
//!
//! Nine fields sit under those units, because the sixth unit
//! (주전공/복수/부/연합/연계) is two recorded facts: which mode, and which
//! additional programmes. Section 38.1 asks for them as two lines --
//! `Degree Mode` and `Additional Major / Minor` -- and gives each its own
//! cell, so splitting them is what lets a missing check name the exact one.

use academic_domain::ContentDigest;
use academic_requirement::{AdmissionYear, ApprovalFact};

use crate::{error::AuditError, gate::OpenGate};

/// The characters admitted in a profile identifier.
///
/// The same narrow set `academic_domain::engines` and `academic_requirement`
/// admit, and for the same reason: the canonical frozen-input encoding
/// separates fields with `=`, `:` and newline, so an identifier that could
/// contain one would make the byte comparison meaningless.
fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

macro_rules! identifier_newtype {
    ($name:ident, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Validates and constructs the identifier.
            pub fn new(value: &str) -> Result<Self, AuditError> {
                if is_identifier(value) {
                    Ok(Self(value.to_owned()))
                } else {
                    Err(AuditError::InvalidIdentifier {
                        kind: $kind,
                        value: value.to_owned(),
                    })
                }
            }

            /// Returns the identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier_newtype!(
    InstitutionId,
    "institution id",
    "One node of section 11.1's `institutionPath`, such as `SNU`."
);
identifier_newtype!(
    GraduationStandard,
    "graduation standard",
    "The graduation standard the user lawfully selected, such as `2026`."
);
identifier_newtype!(
    ProgrammeId,
    "programme id",
    "One additional major or minor programme, such as `stat`."
);

/// A profile field, or the fact that no value was recorded for it.
///
/// Deliberately not `Option`. `Option::unwrap_or`, `Option::unwrap_or_default`
/// and `Option::map_or` all take a value to stand in when there is none, and
/// standing something in is the one move section 3 forbids here. This type has
/// no such method and no `Default`, so the only way to read a value out is to
/// handle the absent arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded<T> {
    /// The user recorded this value.
    Known(T),
    /// No value has been recorded. Section 3's `UNKNOWN`.
    Unknown,
}

impl<T> Recorded<T> {
    /// The recorded value, when there is one.
    #[must_use]
    pub const fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unknown => None,
        }
    }

    /// Whether a value was recorded.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }
}

/// Section 11.1's `majorMode`.
///
/// The yaml block writes one of these identifiers verbatim -- `SINGLE_MAJOR`
/// for 주전공. The other four have no identifier anywhere in the document, so
/// their `SCREAMING_SNAKE_CASE` spelling is this crate's, derived by the one
/// mechanical rule `academic-requirement` already uses for section 11.2's
/// prose rule types: the English name of the mode, upper-cased, with each
/// space becoming an underscore. `the_selector_dimensions_are_the_specifications_own`
/// requires the sixth dimension's `/`-separated alternatives to be exactly as
/// many as the modes below and in the same order, so a mode added here without
/// a matching alternative in the sentence fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DegreeMode {
    /// 주전공 -- the yaml's own `SINGLE_MAJOR`.
    SingleMajor,
    /// 복수전공.
    DoubleMajor,
    /// 부전공.
    Minor,
    /// 연합전공.
    UnitedMajor,
    /// 연계전공.
    LinkedMajor,
}

impl DegreeMode {
    /// Every mode, in the order section 11.1's sentence writes them.
    pub const ALL: [Self; 5] = [
        Self::SingleMajor,
        Self::DoubleMajor,
        Self::Minor,
        Self::UnitedMajor,
        Self::LinkedMajor,
    ];

    /// The identifier a published scope spells.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleMajor => "SINGLE_MAJOR",
            Self::DoubleMajor => "DOUBLE_MAJOR",
            Self::Minor => "MINOR",
            Self::UnitedMajor => "UNITED_MAJOR",
            Self::LinkedMajor => "LINKED_MAJOR",
        }
    }

    /// The `/`-separated alternative of section 11.1's sixth unit this mode is.
    #[must_use]
    pub const fn spec_word(self) -> &'static str {
        match self {
            Self::SingleMajor => "주전공",
            Self::DoubleMajor => "복수",
            Self::Minor => "부",
            Self::UnitedMajor => "연합",
            Self::LinkedMajor => "연계",
        }
    }
}

/// Whether the user has answered section 38.1's transferred-credit line.
///
/// `Declared` is not "there are none": it says the user has addressed the
/// question. Which credits there are, and whether each is recognized, is the
/// transcript's own `RecognitionDecision`, and an undecided one is its own
/// missing check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExchangeOrTransfer {
    /// The user has recorded which transferred or exchange credits apply.
    Declared,
}

/// One of section 11.1's selector inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectorDimension {
    /// 대학.
    University,
    /// 단과대.
    College,
    /// 학부.
    Department,
    /// 입학년도.
    AdmissionYear,
    /// 사용자가 적법하게 선택한 졸업기준.
    GraduationStandard,
    /// 주전공/복수/부/연합/연계.
    MajorMode,
    /// 교환/편입.
    ExchangeOrTransfer,
    /// 예외 승인.
    ExceptionApproval,
}

impl SelectorDimension {
    /// Every dimension, in the order section 11.1's sentence writes them.
    pub const ALL: [Self; 8] = [
        Self::University,
        Self::College,
        Self::Department,
        Self::AdmissionYear,
        Self::GraduationStandard,
        Self::MajorMode,
        Self::ExchangeOrTransfer,
        Self::ExceptionApproval,
    ];

    /// The `·`-delimited unit of section 11.1's sentence this dimension is.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::University => "대학",
            Self::College => "단과대",
            Self::Department => "학부",
            Self::AdmissionYear => "입학년도",
            Self::GraduationStandard => "사용자가 적법하게 선택한 졸업기준",
            Self::MajorMode => "주전공/복수/부/연합/연계",
            Self::ExchangeOrTransfer => "교환/편입",
            Self::ExceptionApproval => "예외 승인",
        }
    }

    /// The stable token the frozen inputs and the missing checks spell.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::University => "university",
            Self::College => "college",
            Self::Department => "department",
            Self::AdmissionYear => "admission_year",
            Self::GraduationStandard => "graduation_standard",
            Self::MajorMode => "major_mode",
            Self::ExchangeOrTransfer => "exchange_or_transfer",
            Self::ExceptionApproval => "exception_approval",
        }
    }

    /// Whether a published `DegreeRequirementSet` declares a scope field for
    /// this dimension.
    ///
    /// Section 11.1's yaml declares `institutionPath`, `admissionYear`,
    /// `selectedGraduationStandardRange` and `majorMode`, which cover the first
    /// six dimensions. It declares no field for 교환/편입 or 예외 승인. Those
    /// two are therefore *required inputs that narrow nothing*: an unrecorded
    /// one is `INDETERMINATE`, and a recorded one removes no candidate. Saying
    /// so is the honest reading; inventing two scope fields the specification
    /// does not write would have made the matrix look stronger than the
    /// document it comes from.
    #[must_use]
    pub const fn narrows_the_catalogue(self) -> bool {
        match self {
            Self::University
            | Self::College
            | Self::Department
            | Self::AdmissionYear
            | Self::GraduationStandard
            | Self::MajorMode => true,
            Self::ExchangeOrTransfer | Self::ExceptionApproval => false,
        }
    }
}

/// One recorded field of the profile, at the granularity a missing check names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProfileField {
    /// `university: SNU`.
    University,
    /// `college: CollegeOfEngineering`.
    College,
    /// `department: CSE`.
    Department,
    /// `admissionYear`.
    AdmissionYear,
    /// `selectedGraduationStandard`.
    GraduationStandard,
    /// `degreeMode`.
    DegreeMode,
    /// `additionalMajors`.
    AdditionalMajors,
    /// `exchangeOrTransferCredits`.
    ExchangeOrTransferCredits,
    /// The approved exceptions section 11.1's sentence names.
    ExceptionApprovals,
}

impl ProfileField {
    /// Every field the selector reads, in section 3's own order.
    pub const ALL: [Self; 9] = [
        Self::University,
        Self::College,
        Self::Department,
        Self::AdmissionYear,
        Self::GraduationStandard,
        Self::DegreeMode,
        Self::AdditionalMajors,
        Self::ExchangeOrTransferCredits,
        Self::ExceptionApprovals,
    ];

    /// Section 3's own key for this field, when section 3 writes one.
    ///
    /// `exceptionApprovals` is `None` because section 3's `StudentProfile`
    /// block has no such key: it is section 11.1's sentence that names 예외
    /// 승인 as a selector input. Recording that difference here is what stops
    /// `the_profile_fields_are_the_specifications_own` from having to be told
    /// about an exception.
    #[must_use]
    pub const fn spec_key(self) -> Option<&'static str> {
        match self {
            Self::University => Some("university"),
            Self::College => Some("college"),
            Self::Department => Some("department"),
            Self::AdmissionYear => Some("admissionYear"),
            Self::GraduationStandard => Some("selectedGraduationStandard"),
            Self::DegreeMode => Some("degreeMode"),
            Self::AdditionalMajors => Some("additionalMajors"),
            Self::ExchangeOrTransferCredits => Some("exchangeOrTransferCredits"),
            Self::ExceptionApprovals => None,
        }
    }

    /// Which of section 11.1's eight units this field sits under.
    #[must_use]
    pub const fn dimension(self) -> SelectorDimension {
        match self {
            Self::University => SelectorDimension::University,
            Self::College => SelectorDimension::College,
            Self::Department => SelectorDimension::Department,
            Self::AdmissionYear => SelectorDimension::AdmissionYear,
            Self::GraduationStandard => SelectorDimension::GraduationStandard,
            Self::DegreeMode | Self::AdditionalMajors => SelectorDimension::MajorMode,
            Self::ExchangeOrTransferCredits => SelectorDimension::ExchangeOrTransfer,
            Self::ExceptionApprovals => SelectorDimension::ExceptionApproval,
        }
    }

    /// The section 38 cell an unrecorded value of this field leaves open.
    ///
    /// Four fields have none. Section 38.1 lists what the user has to supply
    /// and does not list the institution path -- section 3's block writes
    /// `SNU`, `CollegeOfEngineering` and `CSE` as recorded values -- nor the
    /// approved exceptions. An unrecorded one is still a missing check; it is
    /// just not a section 38 cell, and calling it one would put a name on the
    /// page that section 38 does not carry.
    #[must_use]
    pub const fn gate(self) -> Option<OpenGate> {
        match self {
            Self::University | Self::College | Self::Department | Self::ExceptionApprovals => None,
            Self::AdmissionYear => Some(OpenGate::ProfileAdmissionYear),
            Self::GraduationStandard => Some(OpenGate::ProfileGraduationStandard),
            Self::DegreeMode => Some(OpenGate::ProfileDegreeMode),
            Self::AdditionalMajors => Some(OpenGate::ProfileAdditionalMajor),
            Self::ExchangeOrTransferCredits => Some(OpenGate::ProfileExchangeOrTransfer),
        }
    }

    /// What the user has to do to record it.
    #[must_use]
    pub const fn action(self) -> &'static str {
        match self {
            Self::University => "record which university the programme belongs to",
            Self::College => "record which college the programme belongs to",
            Self::Department => "record which department the programme belongs to",
            Self::AdmissionYear => "record the admission year on the official record",
            Self::GraduationStandard => {
                "record which graduation standard was lawfully selected, and from which source"
            }
            Self::DegreeMode => {
                "record the degree mode: single major, double major, minor, united, or linked"
            }
            Self::AdditionalMajors => {
                "record every additional major or minor, or record that there are none"
            }
            Self::ExchangeOrTransferCredits => {
                "record whether transferred or exchange credits apply, with the recognition \
                 decision on each"
            }
            Self::ExceptionApprovals => {
                "record every approved exception that bears on the requirement set, or record \
                 that there are none"
            }
        }
    }
}

/// Section 3's minimal profile, at the granularity section 11.1's selector reads.
///
/// Private fields, no `Default`, and no setter that takes a whole profile: each
/// field is recorded by its own `with_` method. There is no accessor that
/// answers a field with a value the user did not record.
///
/// The fields section 3 lists that are not selector inputs -- `gradingContext`,
/// `interests`, `privacyPolicy` -- are deliberately absent. `gradingContext` is
/// `academic-record`'s versioned `GradingScheme` and is bound to the grade-point
/// reading rather than to the requirement set; `interests` and `privacyPolicy`
/// select no rule. A field that selects nothing and is hashed into the audit
/// identity would make an audit change when a privacy preference did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudentProfile {
    university: Recorded<InstitutionId>,
    college: Recorded<InstitutionId>,
    department: Recorded<InstitutionId>,
    admission_year: Recorded<AdmissionYear>,
    graduation_standard: Recorded<GraduationStandard>,
    degree_mode: Recorded<DegreeMode>,
    additional_majors: Recorded<Vec<ProgrammeId>>,
    exchange_or_transfer: Recorded<ExchangeOrTransfer>,
    exception_approvals: Recorded<Vec<ApprovalFact>>,
}

impl StudentProfile {
    /// An entirely unrecorded profile.
    ///
    /// Section 3's current state, and the one `unknown_profile_audit` starts
    /// from. Every field is `UNKNOWN` and nothing here fills one in.
    ///
    /// Deliberately not `new`, and deliberately not a `Default`: a profile
    /// that arrives by defaulting is one nobody decided to leave empty.
    #[must_use]
    pub const fn unrecorded() -> Self {
        Self {
            university: Recorded::Unknown,
            college: Recorded::Unknown,
            department: Recorded::Unknown,
            admission_year: Recorded::Unknown,
            graduation_standard: Recorded::Unknown,
            degree_mode: Recorded::Unknown,
            additional_majors: Recorded::Unknown,
            exchange_or_transfer: Recorded::Unknown,
            exception_approvals: Recorded::Unknown,
        }
    }

    /// Records the university.
    #[must_use]
    pub fn with_university(mut self, value: InstitutionId) -> Self {
        self.university = Recorded::Known(value);
        self
    }

    /// Records the college.
    #[must_use]
    pub fn with_college(mut self, value: InstitutionId) -> Self {
        self.college = Recorded::Known(value);
        self
    }

    /// Records the department.
    #[must_use]
    pub fn with_department(mut self, value: InstitutionId) -> Self {
        self.department = Recorded::Known(value);
        self
    }

    /// Records the admission year.
    #[must_use]
    pub fn with_admission_year(mut self, value: AdmissionYear) -> Self {
        self.admission_year = Recorded::Known(value);
        self
    }

    /// Records the lawfully selected graduation standard.
    #[must_use]
    pub fn with_graduation_standard(mut self, value: GraduationStandard) -> Self {
        self.graduation_standard = Recorded::Known(value);
        self
    }

    /// Records the degree mode.
    #[must_use]
    pub fn with_degree_mode(mut self, value: DegreeMode) -> Self {
        self.degree_mode = Recorded::Known(value);
        self
    }

    /// Records every additional major or minor, which may be none.
    #[must_use]
    pub fn with_additional_majors(mut self, value: Vec<ProgrammeId>) -> Self {
        self.additional_majors = Recorded::Known(value);
        self
    }

    /// Records that the transferred and exchange credits have been addressed.
    #[must_use]
    pub fn with_exchange_or_transfer(mut self, value: ExchangeOrTransfer) -> Self {
        self.exchange_or_transfer = Recorded::Known(value);
        self
    }

    /// Records every approved exception, which may be none.
    #[must_use]
    pub fn with_exception_approvals(mut self, value: Vec<ApprovalFact>) -> Self {
        self.exception_approvals = Recorded::Known(value);
        self
    }

    /// The university.
    #[must_use]
    pub const fn university(&self) -> &Recorded<InstitutionId> {
        &self.university
    }

    /// The college.
    #[must_use]
    pub const fn college(&self) -> &Recorded<InstitutionId> {
        &self.college
    }

    /// The department.
    #[must_use]
    pub const fn department(&self) -> &Recorded<InstitutionId> {
        &self.department
    }

    /// The admission year.
    #[must_use]
    pub const fn admission_year(&self) -> &Recorded<AdmissionYear> {
        &self.admission_year
    }

    /// The lawfully selected graduation standard.
    #[must_use]
    pub const fn graduation_standard(&self) -> &Recorded<GraduationStandard> {
        &self.graduation_standard
    }

    /// The degree mode.
    #[must_use]
    pub const fn degree_mode(&self) -> &Recorded<DegreeMode> {
        &self.degree_mode
    }

    /// Every additional major or minor.
    #[must_use]
    pub const fn additional_majors(&self) -> &Recorded<Vec<ProgrammeId>> {
        &self.additional_majors
    }

    /// Whether the transferred and exchange credits have been addressed.
    #[must_use]
    pub const fn exchange_or_transfer(&self) -> &Recorded<ExchangeOrTransfer> {
        &self.exchange_or_transfer
    }

    /// Every approved exception.
    #[must_use]
    pub const fn exception_approvals(&self) -> &Recorded<Vec<ApprovalFact>> {
        &self.exception_approvals
    }

    /// Whether one field has been recorded.
    #[must_use]
    pub fn is_recorded(&self, field: ProfileField) -> bool {
        match field {
            ProfileField::University => self.university.is_known(),
            ProfileField::College => self.college.is_known(),
            ProfileField::Department => self.department.is_known(),
            ProfileField::AdmissionYear => self.admission_year.is_known(),
            ProfileField::GraduationStandard => self.graduation_standard.is_known(),
            ProfileField::DegreeMode => self.degree_mode.is_known(),
            ProfileField::AdditionalMajors => self.additional_majors.is_known(),
            ProfileField::ExchangeOrTransferCredits => self.exchange_or_transfer.is_known(),
            ProfileField::ExceptionApprovals => self.exception_approvals.is_known(),
        }
    }

    /// The canonical text this profile's digest is taken over.
    ///
    /// A total function of the recorded fields in a fixed order, with `UNKNOWN`
    /// written where nothing was recorded, so a profile that gains a field
    /// hashes differently and `degree_audit_input_binding` can say which input
    /// moved.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = String::new();
        for field in ProfileField::ALL {
            rendered.push_str(field.dimension().as_str());
            rendered.push('.');
            rendered.push_str(match field {
                ProfileField::DegreeMode => "mode",
                ProfileField::AdditionalMajors => "additional",
                _ => "value",
            });
            rendered.push(' ');
            rendered.push_str(&self.rendered_field(field));
            rendered.push('\n');
        }
        rendered
    }

    /// The digest section 6's `DegreeAuditAggregate` binds the profile by.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(self.canonical_text().as_bytes())
    }

    fn rendered_field(&self, field: ProfileField) -> String {
        match field {
            ProfileField::University => render(self.university.known(), InstitutionId::as_str),
            ProfileField::College => render(self.college.known(), InstitutionId::as_str),
            ProfileField::Department => render(self.department.known(), InstitutionId::as_str),
            ProfileField::AdmissionYear => self
                .admission_year
                .known()
                .map_or_else(unknown, |year| year.get().to_string()),
            ProfileField::GraduationStandard => {
                render(self.graduation_standard.known(), GraduationStandard::as_str)
            }
            ProfileField::DegreeMode => self
                .degree_mode
                .known()
                .map_or_else(unknown, |mode| mode.as_str().to_owned()),
            ProfileField::AdditionalMajors => {
                self.additional_majors.known().map_or_else(unknown, |list| {
                    let mut names: Vec<&str> =
                        list.iter().map(|programme| programme.as_str()).collect();
                    names.sort_unstable();
                    if names.is_empty() {
                        "none".to_owned()
                    } else {
                        names.join(",")
                    }
                })
            }
            ProfileField::ExchangeOrTransferCredits => self
                .exchange_or_transfer
                .known()
                .map_or_else(unknown, |_| "declared".to_owned()),
            ProfileField::ExceptionApprovals => {
                self.exception_approvals
                    .known()
                    .map_or_else(unknown, |approvals| {
                        let mut rendered: Vec<String> = approvals
                            .iter()
                            .map(|approval| {
                                format!("{}@{}", approval.rule.as_str(), approval.issued_at.value())
                            })
                            .collect();
                        rendered.sort();
                        if rendered.is_empty() {
                            "none".to_owned()
                        } else {
                            rendered.join(",")
                        }
                    })
            }
        }
    }
}

fn unknown() -> String {
    "UNKNOWN".to_owned()
}

fn render<T>(value: Option<&T>, accessor: fn(&T) -> &str) -> String {
    value.map_or_else(unknown, |value| accessor(value).to_owned())
}
