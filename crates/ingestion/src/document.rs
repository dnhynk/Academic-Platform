//! Stages five and six: the deterministic parse, and the schema it validates against.
//!
//! The parse reads a committed, synthetic line format. It is deterministic in
//! the sense section 29.1 means: the same bytes under the same parser version
//! produce the same document, and nothing about the reading depends on a clock,
//! a random value, or a model.
//!
//! What the parse produces is metadata: the issuing authority, the two dates,
//! the target scope, the transitional measures, and — per rule — an identifier
//! and a digest of the rule's text. **No rule text leaves this module.** That is
//! what makes the diff at stage eight, the invalidation, and the conflict case
//! carry no untrusted bytes: they carry identifiers and digests, and the bytes
//! stay behind [`crate::snapshot::RawSnapshot`]'s one sealed route.
//!
//! Identifiers read out of a document are restricted to `[A-Za-z0-9._-]` by
//! [`crate::identifier`] and by `academic_domain::engines::RuleId`, for the
//! reason `academic_untrusted_content::SourceId` gives: a name lifted out of an
//! untrusted document must not be able to carry a directive or a separator.

use academic_domain::{ContentDigest, engines::RuleId};

use crate::{
    dating::{Date, DateRelation, Dating, EffectiveDate, IssuanceDate},
    identifier::{ProgramKey, SectionPath},
    manifest::ParserVersion,
    snapshot::RawSnapshot,
};

/// Which body issued a document, and therefore where it sits in the hierarchy.
///
/// Five internal levels plus one that is deliberately outside them. An external
/// accreditation standard coexists with university rules without being above or
/// below any of them, which is what makes
/// [`HierarchyRelation::NotComparable`] a state the table really produces
/// rather than an unreachable arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LegalAuthority {
    /// The university statute.
    UniversityStatute,
    /// A central-administration regulation issued under the statute.
    UniversityRegulation,
    /// A college rule.
    CollegeRule,
    /// A department rule.
    DepartmentRule,
    /// An administrative announcement.
    OfficeAnnouncement,
    /// An external accreditation standard, outside the university hierarchy.
    ExternalAccreditationStandard,
}

impl LegalAuthority {
    /// Exhaustive listing. Not the hierarchy: [`SUPERIOR_PAIRS`] is.
    pub const ALL: [Self; 6] = [
        Self::UniversityStatute,
        Self::UniversityRegulation,
        Self::CollegeRule,
        Self::DepartmentRule,
        Self::OfficeAnnouncement,
        Self::ExternalAccreditationStandard,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UniversityStatute => "UNIVERSITY_STATUTE",
            Self::UniversityRegulation => "UNIVERSITY_REGULATION",
            Self::CollegeRule => "COLLEGE_RULE",
            Self::DepartmentRule => "DEPARTMENT_RULE",
            Self::OfficeAnnouncement => "OFFICE_ANNOUNCEMENT",
            Self::ExternalAccreditationStandard => "EXTERNAL_ACCREDITATION_STANDARD",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    /// How this authority stands to `other`.
    ///
    /// Read out of [`SUPERIOR_PAIRS`] by membership. There is no rank, no
    /// index, and no arithmetic: an authority is superior to another because
    /// the pair is written down and reviewed, not because it appears earlier in
    /// a list.
    #[must_use]
    pub fn hierarchy_relation(self, other: Self) -> HierarchyRelation {
        if self == other {
            return HierarchyRelation::SameLevel;
        }
        if SUPERIOR_PAIRS
            .iter()
            .any(|(superior, inferior)| *superior == self && *inferior == other)
        {
            return HierarchyRelation::LeftIsSuperior;
        }
        if SUPERIOR_PAIRS
            .iter()
            .any(|(superior, inferior)| *superior == other && *inferior == self)
        {
            return HierarchyRelation::RightIsSuperior;
        }
        HierarchyRelation::NotComparable
    }
}

/// Every pair in which the left authority is superior to the right.
///
/// Written out rather than derived from a position, and written transitively
/// rather than by chaining: `hierarchy_relation` asks whether a pair is in this
/// table and never how far apart two entries are.
///
/// A slice rather than an array with a length, because a length is a number and
/// `no_numeric_source_winner` refuses one in the comparison path.
pub const SUPERIOR_PAIRS: &[(LegalAuthority, LegalAuthority)] = &[
    (
        LegalAuthority::UniversityStatute,
        LegalAuthority::UniversityRegulation,
    ),
    (
        LegalAuthority::UniversityStatute,
        LegalAuthority::CollegeRule,
    ),
    (
        LegalAuthority::UniversityStatute,
        LegalAuthority::DepartmentRule,
    ),
    (
        LegalAuthority::UniversityStatute,
        LegalAuthority::OfficeAnnouncement,
    ),
    (
        LegalAuthority::UniversityRegulation,
        LegalAuthority::CollegeRule,
    ),
    (
        LegalAuthority::UniversityRegulation,
        LegalAuthority::DepartmentRule,
    ),
    (
        LegalAuthority::UniversityRegulation,
        LegalAuthority::OfficeAnnouncement,
    ),
    (LegalAuthority::CollegeRule, LegalAuthority::DepartmentRule),
    (
        LegalAuthority::CollegeRule,
        LegalAuthority::OfficeAnnouncement,
    ),
    (
        LegalAuthority::DepartmentRule,
        LegalAuthority::OfficeAnnouncement,
    ),
];

/// Where one issuing authority stands relative to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HierarchyRelation {
    /// The same body, or the same level.
    SameLevel,
    /// The left document's authority is superior.
    LeftIsSuperior,
    /// The right document's authority is superior.
    RightIsSuperior,
    /// Neither is above the other. An external standard beside a university
    /// rule is the case this exists for.
    NotComparable,
}

impl HierarchyRelation {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::SameLevel,
        Self::LeftIsSuperior,
        Self::RightIsSuperior,
        Self::NotComparable,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SameLevel => "SAME_LEVEL",
            Self::LeftIsSuperior => "LEFT_IS_SUPERIOR",
            Self::RightIsSuperior => "RIGHT_IS_SUPERIOR",
            Self::NotComparable => "NOT_COMPARABLE",
        }
    }
}

/// An admission year, as a cohort boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdmissionYear {
    year: u16,
}

impl AdmissionYear {
    /// Takes an admission year.
    #[must_use]
    pub const fn new(year: u16) -> Self {
        Self { year }
    }

    /// The year.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.year
    }
}

/// The cohorts a rule applies to, as a closed or open interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CohortRange {
    from: Option<AdmissionYear>,
    to: Option<AdmissionYear>,
}

impl CohortRange {
    /// Every cohort.
    #[must_use]
    pub const fn every() -> Self {
        Self {
            from: None,
            to: None,
        }
    }

    /// From this admission year onwards.
    #[must_use]
    pub const fn from(year: AdmissionYear) -> Self {
        Self {
            from: Some(year),
            to: None,
        }
    }

    /// Between two admission years, inclusive.
    #[must_use]
    pub const fn between(first: AdmissionYear, last: AdmissionYear) -> Self {
        Self {
            from: Some(first),
            to: Some(last),
        }
    }

    /// Whether `year` is inside the range.
    #[must_use]
    pub fn covers(self, year: AdmissionYear) -> bool {
        self.from.is_none_or(|first| year >= first) && self.to.is_none_or(|last| year <= last)
    }

    /// Whether every cohort this range covers is also covered by `other`.
    #[must_use]
    pub fn is_within(self, other: Self) -> bool {
        let lower_ok = match (self.from, other.from) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(mine), Some(theirs)) => mine >= theirs,
        };
        let upper_ok = match (self.to, other.to) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(mine), Some(theirs)) => mine <= theirs,
        };
        lower_ok && upper_ok
    }

    /// Whether the two ranges share a cohort.
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        let lower = match (self.from, other.from) {
            (None, None) => None,
            (Some(year), None) | (None, Some(year)) => Some(year),
            (Some(mine), Some(theirs)) => Some(mine.max(theirs)),
        };
        let upper = match (self.to, other.to) {
            (None, None) => None,
            (Some(year), None) | (None, Some(year)) => Some(year),
            (Some(mine), Some(theirs)) => Some(mine.min(theirs)),
        };
        match (lower, upper) {
            (Some(first), Some(last)) => first <= last,
            _ => true,
        }
    }
}

/// Who a rule applies to: one programme, one cohort range.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetScope {
    program: ProgramKey,
    cohorts: CohortRange,
}

impl TargetScope {
    /// A scope.
    #[must_use]
    pub const fn new(program: ProgramKey, cohorts: CohortRange) -> Self {
        Self { program, cohorts }
    }

    /// Which programme.
    #[must_use]
    pub const fn program(&self) -> &ProgramKey {
        &self.program
    }

    /// Which cohorts.
    #[must_use]
    pub const fn cohorts(&self) -> CohortRange {
        self.cohorts
    }

    /// How this scope stands to `other`, as a name.
    #[must_use]
    pub fn relation_to(&self, other: &Self) -> ScopeRelation {
        if self.program != other.program {
            return ScopeRelation::Disjoint;
        }
        let mine_within = self.cohorts.is_within(other.cohorts);
        let theirs_within = other.cohorts.is_within(self.cohorts);
        match (mine_within, theirs_within) {
            (true, true) => ScopeRelation::Identical,
            (true, false) => ScopeRelation::RightContainsLeft,
            (false, true) => ScopeRelation::LeftContainsRight,
            (false, false) => {
                if self.cohorts.intersects(other.cohorts) {
                    ScopeRelation::Overlapping
                } else {
                    ScopeRelation::Disjoint
                }
            }
        }
    }
}

/// How two target scopes stand to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeRelation {
    /// The same programme and the same cohorts.
    Identical,
    /// The left scope covers everything the right one does, and more.
    LeftContainsRight,
    /// The right scope covers everything the left one does, and more.
    RightContainsLeft,
    /// They share cohorts and neither contains the other.
    Overlapping,
    /// They share no cohort, or they are different programmes.
    Disjoint,
}

impl ScopeRelation {
    /// Exhaustive listing.
    pub const ALL: [Self; 5] = [
        Self::Identical,
        Self::LeftContainsRight,
        Self::RightContainsLeft,
        Self::Overlapping,
        Self::Disjoint,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identical => "IDENTICAL",
            Self::LeftContainsRight => "LEFT_CONTAINS_RIGHT",
            Self::RightContainsLeft => "RIGHT_CONTAINS_LEFT",
            Self::Overlapping => "OVERLAPPING",
            Self::Disjoint => "DISJOINT",
        }
    }
}

/// What a document says about people caught by a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionalMeasures {
    /// The document says nothing. Not the same as saying there are none.
    Silent,
    /// Cohorts admitted before the effective date keep the previous rule.
    PriorCohortKeepsPreviousRule,
    /// The change phases in by admission year.
    PhasedByAdmissionYear,
    /// It applies to everyone from the effective date.
    ImmediateForEveryone,
}

impl TransitionalMeasures {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::Silent,
        Self::PriorCohortKeepsPreviousRule,
        Self::PhasedByAdmissionYear,
        Self::ImmediateForEveryone,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Silent => "SILENT",
            Self::PriorCohortKeepsPreviousRule => "PRIOR_COHORT_KEEPS_PREVIOUS_RULE",
            Self::PhasedByAdmissionYear => "PHASED_BY_ADMISSION_YEAR",
            Self::ImmediateForEveryone => "IMMEDIATE_FOR_EVERYONE",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    /// Whether the document provides for the people a change catches.
    #[must_use]
    pub const fn provides_for_a_transition(self) -> bool {
        matches!(
            self,
            Self::PriorCohortKeepsPreviousRule | Self::PhasedByAdmissionYear
        )
    }

    /// How this stands to `other`, as a name.
    #[must_use]
    pub const fn relation_to(self, other: Self) -> TransitionRelation {
        match (
            self.provides_for_a_transition(),
            other.provides_for_a_transition(),
        ) {
            (false, false) => TransitionRelation::NeitherProvides,
            (true, false) => TransitionRelation::OnlyLeftProvides,
            (false, true) => TransitionRelation::OnlyRightProvides,
            (true, true) => TransitionRelation::BothProvide,
        }
    }
}

/// How two documents' transitional measures stand to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionRelation {
    /// Neither document provides for a transition.
    NeitherProvides,
    /// Only the left one does.
    OnlyLeftProvides,
    /// Only the right one does.
    OnlyRightProvides,
    /// Both do.
    BothProvide,
}

impl TransitionRelation {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::NeitherProvides,
        Self::OnlyLeftProvides,
        Self::OnlyRightProvides,
        Self::BothProvide,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeitherProvides => "NEITHER_PROVIDES",
            Self::OnlyLeftProvides => "ONLY_LEFT_PROVIDES",
            Self::OnlyRightProvides => "ONLY_RIGHT_PROVIDES",
            Self::BothProvide => "BOTH_PROVIDE",
        }
    }
}

/// One rule as parsed: an identifier and a digest of its text.
///
/// The text itself is not here. A structural or textual change to a rule is
/// visible as a changed digest, which is what the impact analysis needs, and no
/// consumer of a diff receives document bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRule {
    id: RuleId,
    section: SectionPath,
    text_digest: ContentDigest,
}

impl ParsedRule {
    /// Which rule.
    #[must_use]
    pub const fn id(&self) -> &RuleId {
        &self.id
    }

    /// Which section it sits in. The structural half.
    #[must_use]
    pub const fn section(&self) -> &SectionPath {
        &self.section
    }

    /// The digest of its text. The textual half.
    #[must_use]
    pub const fn text_digest(&self) -> &ContentDigest {
        &self.text_digest
    }
}

/// One official document, as parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialDocument {
    authority: LegalAuthority,
    issued: Option<IssuanceDate>,
    dating: Dating,
    scope: TargetScope,
    transition: TransitionalMeasures,
    rules: Vec<ParsedRule>,
    parser_version: ParserVersion,
}

impl OfficialDocument {
    /// Which body issued it.
    #[must_use]
    pub const fn authority(&self) -> LegalAuthority {
        self.authority
    }

    /// When it was issued, if it says.
    #[must_use]
    pub const fn issued(&self) -> Option<IssuanceDate> {
        self.issued
    }

    /// When it starts to apply, or [`Dating::Unscoped`].
    #[must_use]
    pub const fn dating(&self) -> Dating {
        self.dating
    }

    /// Who it applies to.
    #[must_use]
    pub const fn scope(&self) -> &TargetScope {
        &self.scope
    }

    /// What it says about people caught by the change.
    #[must_use]
    pub const fn transitional_measures(&self) -> TransitionalMeasures {
        self.transition
    }

    /// The rules it carries, in document order.
    #[must_use]
    pub fn rules(&self) -> &[ParsedRule] {
        &self.rules
    }

    /// Which parser read it.
    #[must_use]
    pub const fn parser_version(&self) -> ParserVersion {
        self.parser_version
    }

    /// One rule by identifier.
    #[must_use]
    pub fn rule(&self, id: &RuleId) -> Option<&ParsedRule> {
        self.rules.iter().find(|rule| rule.id() == id)
    }
}

/// Why bytes did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// The bytes were not UTF-8.
    #[error("the retained bytes are not UTF-8")]
    NotUtf8,
    /// A line was not one of the recognized forms.
    #[error("line {line}: unrecognized directive")]
    UnrecognizedLine {
        /// Which line, counting from one.
        line: usize,
    },
    /// A directive's value was not valid.
    #[error("line {line}: {directive} does not accept this value")]
    BadValue {
        /// Which line, counting from one.
        line: usize,
        /// Which directive.
        directive: &'static str,
    },
    /// A required directive was absent.
    #[error("the document declares no {directive}")]
    MissingDirective {
        /// Which directive.
        directive: &'static str,
    },
    /// A rule appeared outside any section.
    #[error("line {line}: a rule appears before any section")]
    RuleOutsideSection {
        /// Which line, counting from one.
        line: usize,
    },
}

/// Why a parsed document is not schema-valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// The document carries no rule.
    #[error("the document carries no rule")]
    NoRules,
    /// Two rules share an identifier.
    #[error("the rule identifier {0} appears more than once")]
    DuplicateRuleId(String),
    /// An effective date precedes the issuance date.
    #[error("the effective date precedes the issuance date")]
    EffectiveBeforeIssuance,
}

/// The `EFFECTIVE:` directive's name, as the parse and its tests both spell it.
const EFFECTIVE_DIRECTIVE: &str = "EFFECTIVE";

/// Reads a document out of one snapshot's retained bytes.
///
/// Deterministic: no clock, no random value, no model. The parser version is
/// carried through from the snapshot, so a document parsed by a different
/// version is a different reading and says so.
///
/// # Errors
///
/// [`ParseError`] for bytes that are not UTF-8, an unrecognized line, a value a
/// directive does not accept, an absent required directive, or a rule outside
/// any section.
pub fn parse(snapshot: &RawSnapshot) -> Result<OfficialDocument, ParseError> {
    let Ok(text) = core::str::from_utf8(snapshot.source_bytes()) else {
        return Err(ParseError::NotUtf8);
    };

    let mut authority = None;
    let mut issued = None;
    let mut effective = None;
    let mut program = None;
    let mut cohorts = CohortRange::every();
    let mut transition = TransitionalMeasures::Silent;
    let mut section: Option<SectionPath> = None;
    let mut rules = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((directive, value)) = trimmed.split_once(':') else {
            return Err(ParseError::UnrecognizedLine { line });
        };
        let value = value.trim();
        match directive.trim() {
            "AUTHORITY" => {
                authority = Some(LegalAuthority::parse(value).ok_or(ParseError::BadValue {
                    line,
                    directive: "AUTHORITY",
                })?);
            }
            "ISSUED" => {
                issued = Some(IssuanceDate::on(parse_date(value).ok_or(
                    ParseError::BadValue {
                        line,
                        directive: "ISSUED",
                    },
                )?));
            }
            EFFECTIVE_DIRECTIVE => {
                effective = Some(EffectiveDate::on(parse_date(value).ok_or(
                    ParseError::BadValue {
                        line,
                        directive: EFFECTIVE_DIRECTIVE,
                    },
                )?));
            }
            "PROGRAM" => {
                program = Some(ProgramKey::new(value).map_err(|_| ParseError::BadValue {
                    line,
                    directive: "PROGRAM",
                })?);
            }
            "COHORTS" => {
                cohorts = parse_cohorts(value).ok_or(ParseError::BadValue {
                    line,
                    directive: "COHORTS",
                })?;
            }
            "TRANSITION" => {
                transition = TransitionalMeasures::parse(value).ok_or(ParseError::BadValue {
                    line,
                    directive: "TRANSITION",
                })?;
            }
            "SECTION" => {
                section = Some(SectionPath::new(value).map_err(|_| ParseError::BadValue {
                    line,
                    directive: "SECTION",
                })?);
            }
            "RULE" => {
                let here = section
                    .clone()
                    .ok_or(ParseError::RuleOutsideSection { line })?;
                let (id, body) = value.split_once('|').ok_or(ParseError::BadValue {
                    line,
                    directive: "RULE",
                })?;
                let id = RuleId::new(id.trim()).map_err(|_| ParseError::BadValue {
                    line,
                    directive: "RULE",
                })?;
                rules.push(ParsedRule {
                    id,
                    section: here,
                    text_digest: ContentDigest::sha256(body.trim().as_bytes()),
                });
            }
            _ => return Err(ParseError::UnrecognizedLine { line }),
        }
    }

    let authority = authority.ok_or(ParseError::MissingDirective {
        directive: "AUTHORITY",
    })?;
    let program = program.ok_or(ParseError::MissingDirective {
        directive: "PROGRAM",
    })?;

    // `IN02`. An absent effective date is not an error and not a default: it is
    // the `UNSCOPED_OFFICIAL_SOURCE` arm, and the type is what stops it from
    // being published.
    let dating = effective.map_or(Dating::Unscoped, Dating::Effective);

    Ok(OfficialDocument {
        authority,
        issued,
        dating,
        scope: TargetScope::new(program, cohorts),
        transition,
        rules,
        parser_version: snapshot.parser_version(),
    })
}

/// Stage six. Checks the parsed document against the schema.
///
/// # Errors
///
/// [`SchemaError`] when the document carries no rule, repeats a rule
/// identifier, or dates its effect before its issuance.
pub fn validate(document: &OfficialDocument) -> Result<(), SchemaError> {
    if document.rules.is_empty() {
        return Err(SchemaError::NoRules);
    }
    for (index, rule) in document.rules.iter().enumerate() {
        if document
            .rules
            .iter()
            .skip(index + 1)
            .any(|later| later.id() == rule.id())
        {
            return Err(SchemaError::DuplicateRuleId(rule.id().as_str().to_owned()));
        }
    }
    if let (Some(issued), Some(effective)) = (document.issued, document.dating.effective_date())
        && effective.date().relation_to(issued.date()) == DateRelation::Earlier
    {
        return Err(SchemaError::EffectiveBeforeIssuance);
    }
    Ok(())
}

/// `YYYY-MM-DD`.
fn parse_date(value: &str) -> Option<Date> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Date::new(year, month, day).ok()
}

/// `*`, `2023-`, `-2022`, or `2020-2022`.
fn parse_cohorts(value: &str) -> Option<CohortRange> {
    if value == "*" {
        return Some(CohortRange::every());
    }
    let (first, last) = value.split_once('-')?;
    let first = if first.is_empty() {
        None
    } else {
        Some(AdmissionYear::new(first.parse().ok()?))
    };
    let last = if last.is_empty() {
        None
    } else {
        Some(AdmissionYear::new(last.parse().ok()?))
    };
    match (first, last) {
        (None, None) => None,
        (Some(first), None) => Some(CohortRange::from(first)),
        (None, Some(last)) => Some(CohortRange {
            from: None,
            to: Some(last),
        }),
        (Some(first), Some(last)) => Some(CohortRange::between(first, last)),
    }
}
