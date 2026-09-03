//! The connector declaration, and the two things about it that are types.
//!
//! Section 29.1's sentence is the whole requirement: *every connector declares
//! source ownership, authentication method, allowed frequency, robots/terms
//! status, personal-data class, completeness, last success, next verification
//! and parser version*. [`ManifestField`] is that list, and
//! [`ManifestDraft::build`] refuses a draft that leaves one of them empty, one
//! field at a time, so `connector_manifest_requires_every_field` enumerates
//! them rather than counting them.
//!
//! # The two type-level rules
//!
//! **A fetch target is `&'static`.** [`DeclaredTarget::declared`] is the only
//! constructor and it takes `&'static str`. Bytes that arrive at run time are
//! owned, and `Untrusted<IngestedDocument>` hands out neither a `String` nor a
//! `&str` outside `academic-untrusted-content`, so a link found inside a
//! fetched page is a value no target can be built from. That is what
//! "this is not a crawler" means here, and it is a compile error rather than a
//! review note.
//!
//! **A credential is bound to one manifest.** [`CredentialBinding`] has no
//! public constructor. [`ConnectorManifest::credential_binding`] is the only
//! producer and it returns `None` unless the declared authentication method is
//! one that holds a credential at all. The binding carries the connector's
//! identity, and `ConditionalRequest::credentialed` refuses a target the
//! bound manifest does not declare.

use core::fmt;

use crate::{
    identifier::{ConnectorId, NameError},
    terms::TermsStatus,
};

/// Section 8.4's six collection targets, as names.
///
/// The section prints them as a numbered list and then says, in the same
/// paragraph, that a conflict is **not** settled by the higher or lower number.
/// So this enum derives no ordering, exposes no rank, and has no numeric
/// conversion: `no_numeric_source_winner` pins the whole set of `impl` blocks
/// naming it and its derive list as one line, and `tests/compile_fail/` observes
/// that two values cannot be compared.
///
/// [`Self::ALL`] exists for exhaustiveness, not for precedence. Nothing reads a
/// position out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceCategory {
    /// University statutes, grading, retake and common major regulations.
    UniversityRegulations,
    /// College of Engineering common courses and graduation conditions.
    CollegeMaterial,
    /// Department official pages, bylaws, substitution tables.
    DepartmentPage,
    /// The registration system's offerings, timetable and limits.
    RegistrationSystem,
    /// Instructor or LMS material for one offering.
    InstructorOrLmsMaterial,
    /// A prediction computed from history, which is not an official fact.
    HistoricalPrediction,
}

impl SourceCategory {
    /// Exhaustive listing. Not a precedence order.
    pub const ALL: [Self; 6] = [
        Self::UniversityRegulations,
        Self::CollegeMaterial,
        Self::DepartmentPage,
        Self::RegistrationSystem,
        Self::InstructorOrLmsMaterial,
        Self::HistoricalPrediction,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UniversityRegulations => "UNIVERSITY_REGULATIONS",
            Self::CollegeMaterial => "COLLEGE_MATERIAL",
            Self::DepartmentPage => "DEPARTMENT_PAGE",
            Self::RegistrationSystem => "REGISTRATION_SYSTEM",
            Self::InstructorOrLmsMaterial => "INSTRUCTOR_OR_LMS_MATERIAL",
            Self::HistoricalPrediction => "HISTORICAL_PREDICTION",
        }
    }
}

/// One document this connector is declared to retrieve.
///
/// The declaration is `&'static str`: it is written in a manifest, compiled in,
/// and cannot be produced from anything read at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredTarget {
    declared: &'static str,
}

impl DeclaredTarget {
    /// The only constructor.
    #[must_use]
    pub const fn declared(value: &'static str) -> Self {
        Self { declared: value }
    }

    /// The declaration, verbatim.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.declared
    }
}

impl fmt::Display for DeclaredTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.declared)
    }
}

/// Who owns the source, in the sense section 29.1 asks a connector to declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceOwnership {
    /// The university publishes it and owns the terms.
    University,
    /// A college or department publishes it under the university's terms.
    CollegeOrDepartment,
    /// The user holds it: an export they downloaded, a file they saved.
    UserHeld,
    /// A third party publishes it under its own terms.
    ThirdParty,
}

/// How a connector authenticates, and therefore whether it holds a credential.
///
/// Section 29.2: *mySNU/LMS credentials are not given to a general crawler; an
/// official API, a user export, and a file the user saved are the default.*
/// Only [`Self::ScopedOfficialApiToken`] holds a credential at all, and
/// [`ConnectorManifest::credential_binding`] is the one place that fact is
/// read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthenticationMethod {
    /// A public page. Nothing is presented.
    PublicNoCredential,
    /// A token the user provisioned for one declared official interface.
    ScopedOfficialApiToken,
    /// The user authenticated in their own session and handed over the export.
    /// This system never holds the credential that produced it.
    UserSuppliedExport,
}

impl AuthenticationMethod {
    /// Exhaustive listing.
    pub const ALL: [Self; 3] = [
        Self::PublicNoCredential,
        Self::ScopedOfficialApiToken,
        Self::UserSuppliedExport,
    ];

    /// Whether this system holds a credential for the source.
    ///
    /// [`Self::UserSuppliedExport`] is `false` on purpose: the user
    /// authenticated, not this system, and what arrives is a file.
    #[must_use]
    pub const fn holds_a_credential(self) -> bool {
        matches!(self, Self::ScopedOfficialApiToken)
    }
}

/// How often the connector may fetch, as a named cadence.
///
/// Section 29.2 asks for *low-frequency* conditional fetch. A cadence is named
/// rather than free so a manifest cannot declare "every second" by editing a
/// number, and [`Self::earliest_next`] is the only reader of the interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllowedFrequency {
    /// At most once a day.
    Daily,
    /// At most once a week.
    Weekly,
    /// At most once a term.
    PerTerm,
    /// Never on a schedule; only when the user asks.
    OnUserRequestOnly,
}

/// Seconds in a day, as the cadence table's unit.
const SECONDS_PER_DAY: u64 = 86_400;

impl AllowedFrequency {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::Daily,
        Self::Weekly,
        Self::PerTerm,
        Self::OnUserRequestOnly,
    ];

    /// The earliest wall-clock second at which a scheduled fetch is permitted.
    ///
    /// [`Self::OnUserRequestOnly`] returns `None`: there is no schedule, so
    /// there is no next scheduled time, and a caller that wants one has to ask
    /// the user instead of reading a default out of here.
    #[must_use]
    pub const fn earliest_next(self, last: RetrievalInstant) -> Option<RetrievalInstant> {
        let days: u64 = match self {
            Self::Daily => 1,
            Self::Weekly => 7,
            Self::PerTerm => 120,
            Self::OnUserRequestOnly => return None,
        };
        Some(RetrievalInstant::at(
            last.seconds()
                .saturating_add(days.saturating_mul(SECONDS_PER_DAY)),
        ))
    }
}

/// A wall-clock reading taken when bytes were retrieved.
///
/// One of the three axes `CONTRIBUTING.md` keeps apart. This is the *retrieval*
/// clock. Origin order is [`crate::stage::IngestSeq`] and valid time is
/// [`crate::dating::EffectiveDate`]; no arithmetic in this crate mixes them,
/// and `the_three_time_axes_are_distinct_types` is what says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetrievalInstant {
    seconds_since_epoch: u64,
}

impl RetrievalInstant {
    /// A reading, in seconds since the Unix epoch.
    #[must_use]
    pub const fn at(seconds_since_epoch: u64) -> Self {
        Self {
            seconds_since_epoch,
        }
    }

    /// The reading.
    #[must_use]
    pub const fn seconds(self) -> u64 {
        self.seconds_since_epoch
    }
}

/// Section 29.1's personal-data class for what a source yields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PersonalDataClass {
    /// Published to everyone; no person is identified.
    Public,
    /// Identifies a member of staff in their public role.
    PubliclyIdentifiedStaff,
    /// Identifies the user. Restricted, user-managed.
    UserPersonal,
}

/// How complete the connector believes its coverage of the source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Completeness {
    /// Every document the source publishes in scope is declared.
    Full,
    /// A declared subset; the manifest names what it covers.
    Partial,
    /// Coverage has not been assessed. Not a synonym for `Full`.
    Unassessed,
}

/// When the connector last succeeded, if it ever has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LastSuccess {
    /// Never run to success. Distinct from "ran and found nothing".
    Never,
    /// Succeeded at this retrieval instant.
    At(RetrievalInstant),
}

/// When the declaration itself is next due for review.
///
/// Section 29.1 asks for this separately from `last success`: a connector that
/// keeps succeeding against terms nobody re-read is the failure mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NextVerification {
    due_at: RetrievalInstant,
}

impl NextVerification {
    /// A due date.
    #[must_use]
    pub const fn due_at(instant: RetrievalInstant) -> Self {
        Self { due_at: instant }
    }

    /// The due date.
    #[must_use]
    pub const fn instant(self) -> RetrievalInstant {
        self.due_at
    }

    /// Whether the declaration is overdue at `now`.
    #[must_use]
    pub const fn is_overdue(self, now: RetrievalInstant) -> bool {
        now.seconds() > self.due_at.seconds()
    }
}

/// The parser this connector's documents are parsed with.
///
/// Retained on every snapshot, because a re-parse under a different version is
/// a different reading of the same bytes and section 29.1 keeps both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParserVersion {
    version: u16,
}

impl ParserVersion {
    /// A version.
    #[must_use]
    pub const fn new(version: u16) -> Self {
        Self { version }
    }

    /// The version.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.version
    }
}

/// Section 29.1's nine declared fields, as an enumerable list.
///
/// `connector_manifest_requires_every_field` iterates this and drops one field
/// at a time, so the evidence is per-field rather than a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManifestField {
    /// Who owns the source.
    SourceOwnership,
    /// How the connector authenticates.
    AuthenticationMethod,
    /// How often it may fetch.
    AllowedFrequency,
    /// The robots and terms status of the source.
    TermsStatus,
    /// The personal-data class of what it yields.
    PersonalDataClass,
    /// How complete the declared coverage is.
    Completeness,
    /// When it last succeeded.
    LastSuccess,
    /// When the declaration is next verified.
    NextVerification,
    /// Which parser reads its documents.
    ParserVersion,
}

impl ManifestField {
    /// Section 29.1's order, which is the order the sentence lists them in.
    pub const ALL: [Self; 9] = [
        Self::SourceOwnership,
        Self::AuthenticationMethod,
        Self::AllowedFrequency,
        Self::TermsStatus,
        Self::PersonalDataClass,
        Self::Completeness,
        Self::LastSuccess,
        Self::NextVerification,
        Self::ParserVersion,
    ];

    /// Section 29.1's own spelling of the field.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceOwnership => "source ownership",
            Self::AuthenticationMethod => "authentication method",
            Self::AllowedFrequency => "allowed frequency",
            Self::TermsStatus => "robots/terms status",
            Self::PersonalDataClass => "personal-data class",
            Self::Completeness => "completeness",
            Self::LastSuccess => "last success",
            Self::NextVerification => "next verification",
            Self::ParserVersion => "parser version",
        }
    }
}

/// Why a draft is not a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// A declared field was left empty.
    #[error("the connector manifest declares no {}", .0.as_str())]
    Missing(ManifestField),
    /// The connector declared no document to retrieve.
    #[error("the connector manifest declares no source target")]
    NoDeclaredTarget,
}

/// A manifest under construction. Every field starts empty.
///
/// There is no `Default` for [`ConnectorManifest`] and no constructor that
/// fills a field in for the caller. A draft is the only route, and
/// [`Self::build`] names the first field that is still empty.
#[derive(Debug, Clone)]
pub struct ManifestDraft {
    connector: ConnectorId,
    category: SourceCategory,
    targets: Vec<DeclaredTarget>,
    ownership: Option<SourceOwnership>,
    authentication: Option<AuthenticationMethod>,
    frequency: Option<AllowedFrequency>,
    terms: Option<TermsStatus>,
    personal_data: Option<PersonalDataClass>,
    completeness: Option<Completeness>,
    last_success: Option<LastSuccess>,
    next_verification: Option<NextVerification>,
    parser_version: Option<ParserVersion>,
}

impl ManifestDraft {
    /// Starts a draft for one connector and one section 8.4 category.
    #[must_use]
    pub const fn for_connector(connector: ConnectorId, category: SourceCategory) -> Self {
        Self {
            connector,
            category,
            targets: Vec::new(),
            ownership: None,
            authentication: None,
            frequency: None,
            terms: None,
            personal_data: None,
            completeness: None,
            last_success: None,
            next_verification: None,
            parser_version: None,
        }
    }

    /// Declares one document the connector may retrieve.
    #[must_use]
    pub fn declaring(mut self, target: DeclaredTarget) -> Self {
        self.targets.push(target);
        self
    }

    /// Sets who owns the source.
    #[must_use]
    pub const fn source_ownership(mut self, value: SourceOwnership) -> Self {
        self.ownership = Some(value);
        self
    }

    /// Sets how the connector authenticates.
    #[must_use]
    pub const fn authentication_method(mut self, value: AuthenticationMethod) -> Self {
        self.authentication = Some(value);
        self
    }

    /// Sets how often it may fetch.
    #[must_use]
    pub const fn allowed_frequency(mut self, value: AllowedFrequency) -> Self {
        self.frequency = Some(value);
        self
    }

    /// Sets the robots and terms status.
    #[must_use]
    pub const fn terms_status(mut self, value: TermsStatus) -> Self {
        self.terms = Some(value);
        self
    }

    /// Sets the personal-data class.
    #[must_use]
    pub const fn personal_data_class(mut self, value: PersonalDataClass) -> Self {
        self.personal_data = Some(value);
        self
    }

    /// Sets the declared completeness.
    #[must_use]
    pub const fn completeness(mut self, value: Completeness) -> Self {
        self.completeness = Some(value);
        self
    }

    /// Sets the last success.
    #[must_use]
    pub const fn last_success(mut self, value: LastSuccess) -> Self {
        self.last_success = Some(value);
        self
    }

    /// Sets when the declaration is next verified.
    #[must_use]
    pub const fn next_verification(mut self, value: NextVerification) -> Self {
        self.next_verification = Some(value);
        self
    }

    /// Sets the parser version.
    #[must_use]
    pub const fn parser_version(mut self, value: ParserVersion) -> Self {
        self.parser_version = Some(value);
        self
    }

    /// Builds the manifest.
    ///
    /// # Errors
    ///
    /// [`ManifestError::Missing`] naming the first empty field in section
    /// 29.1's own order, or [`ManifestError::NoDeclaredTarget`] when nothing
    /// is declared to retrieve.
    pub fn build(self) -> Result<ConnectorManifest, ManifestError> {
        let ownership = self
            .ownership
            .ok_or(ManifestError::Missing(ManifestField::SourceOwnership))?;
        let authentication = self
            .authentication
            .ok_or(ManifestError::Missing(ManifestField::AuthenticationMethod))?;
        let frequency = self
            .frequency
            .ok_or(ManifestError::Missing(ManifestField::AllowedFrequency))?;
        let terms = self
            .terms
            .ok_or(ManifestError::Missing(ManifestField::TermsStatus))?;
        let personal_data = self
            .personal_data
            .ok_or(ManifestError::Missing(ManifestField::PersonalDataClass))?;
        let completeness = self
            .completeness
            .ok_or(ManifestError::Missing(ManifestField::Completeness))?;
        let last_success = self
            .last_success
            .ok_or(ManifestError::Missing(ManifestField::LastSuccess))?;
        let next_verification = self
            .next_verification
            .ok_or(ManifestError::Missing(ManifestField::NextVerification))?;
        let parser_version = self
            .parser_version
            .ok_or(ManifestError::Missing(ManifestField::ParserVersion))?;
        if self.targets.is_empty() {
            return Err(ManifestError::NoDeclaredTarget);
        }
        Ok(ConnectorManifest {
            connector: self.connector,
            category: self.category,
            targets: self.targets,
            ownership,
            authentication,
            frequency,
            terms,
            personal_data,
            completeness,
            last_success,
            next_verification,
            parser_version,
        })
    }
}

/// A connector's whole declaration.
///
/// Private fields and no `Default`: `tests/compile_fail/` observes that the
/// struct literal is not writable outside this crate, so [`ManifestDraft`] is
/// the only route and every field is answered.
#[derive(Debug, Clone)]
pub struct ConnectorManifest {
    connector: ConnectorId,
    category: SourceCategory,
    targets: Vec<DeclaredTarget>,
    ownership: SourceOwnership,
    authentication: AuthenticationMethod,
    frequency: AllowedFrequency,
    terms: TermsStatus,
    personal_data: PersonalDataClass,
    completeness: Completeness,
    last_success: LastSuccess,
    next_verification: NextVerification,
    parser_version: ParserVersion,
}

impl ConnectorManifest {
    /// Which connector.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// Which section 8.4 category the source belongs to.
    #[must_use]
    pub const fn category(&self) -> SourceCategory {
        self.category
    }

    /// The documents this connector may retrieve.
    #[must_use]
    pub fn declared_targets(&self) -> &[DeclaredTarget] {
        &self.targets
    }

    /// Whether `target` is one of them.
    #[must_use]
    pub fn declares(&self, target: DeclaredTarget) -> bool {
        self.targets.contains(&target)
    }

    /// Who owns the source.
    #[must_use]
    pub const fn source_ownership(&self) -> SourceOwnership {
        self.ownership
    }

    /// How the connector authenticates.
    #[must_use]
    pub const fn authentication_method(&self) -> AuthenticationMethod {
        self.authentication
    }

    /// How often it may fetch.
    #[must_use]
    pub const fn allowed_frequency(&self) -> AllowedFrequency {
        self.frequency
    }

    /// The robots and terms status as declared.
    #[must_use]
    pub const fn terms_status(&self) -> TermsStatus {
        self.terms
    }

    /// The personal-data class.
    #[must_use]
    pub const fn personal_data_class(&self) -> PersonalDataClass {
        self.personal_data
    }

    /// The declared completeness.
    #[must_use]
    pub const fn completeness(&self) -> Completeness {
        self.completeness
    }

    /// The last success.
    #[must_use]
    pub const fn last_success(&self) -> LastSuccess {
        self.last_success
    }

    /// When the declaration is next verified.
    #[must_use]
    pub const fn next_verification(&self) -> NextVerification {
        self.next_verification
    }

    /// Which parser reads its documents.
    #[must_use]
    pub const fn parser_version(&self) -> ParserVersion {
        self.parser_version
    }

    /// The credential this connector may present, if it holds one at all.
    ///
    /// The only producer of a [`CredentialBinding`]. `None` for every
    /// authentication method that holds no credential, which is section 29.2's
    /// rule written where it is read.
    #[must_use]
    pub fn credential_binding(&self) -> Option<CredentialBinding> {
        self.authentication
            .holds_a_credential()
            .then(|| CredentialBinding {
                connector: self.connector.clone(),
            })
    }
}

/// Permission to present the user's scoped credential for one connector.
///
/// It holds no credential byte. `academic-policy` grants
/// `BORROW_CONNECTOR_CREDENTIAL` to the connector process class and the broker
/// holds the opaque handle; this is the binding that says *which* connector a
/// borrow belongs to, and the request constructor refuses a target that
/// connector does not declare.
///
/// Deliberately not `Copy` and not `Clone`: a binding consumed by one request
/// cannot be spent again on a second.
pub struct CredentialBinding {
    connector: ConnectorId,
}

impl CredentialBinding {
    /// Which connector the borrow belongs to.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }
}

impl fmt::Debug for CredentialBinding {
    /// Prints the connector and nothing else. There is nothing else.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialBinding")
            .field("connector", &self.connector)
            .finish()
    }
}

/// Builds a connector identifier.
///
/// # Errors
///
/// [`NameError`] when the value is not `[A-Za-z0-9._-]{1,64}`.
pub fn connector_id(value: &str) -> Result<ConnectorId, NameError> {
    ConnectorId::new(value)
}
