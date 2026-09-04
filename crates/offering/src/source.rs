//! Which reading confirms an offering, and which readings only cross-check it.
//!
//! Section 8.3: *공식 개설 확인은 [서울대학교 수강신청 시스템]의 최신 강좌
//! 상세를 기준으로 하고, CSE 홈페이지·수강편람은 교차 출처로 사용한다. 2026-2
//! 수강편람도 작성 기준일 이후 변경 가능하므로 수강신청 시스템의 최신 상태를
//! 재확인하도록 공식 안내되어 있다.*
//!
//! That sentence names a basis, and it is the only place in this design where
//! one source is named as the basis for a question. It is **not** section 8.4's
//! mechanical winner: 8.4 forbids deciding a *regulation* conflict by a source's
//! number in a list, and this is not a regulation conflict. It is the one
//! reading that says whether a section exists in a term, stated as such by the
//! specification.
//!
//! # What this refuses
//!
//! - A confirmation founded on anything but
//!   `SourceCategory::RegistrationSystem`. [`ConfirmationEvidence::from_registration_system`]
//!   compares against that one value, so all five other levels of section 8.4 --
//!   including a department page, which is the source a plausible shortcut would
//!   accept -- are refused by the same arm. `offering_source_authority` runs
//!   every value of `SourceCategory::ALL` through it, so a level added to that
//!   enumeration arrives refused rather than unconsidered.
//! - A reading older than the recorded [`crate::policy::VerificationRecency`].
//!   Section 8.3's `CONFIRMED` requires 최근 확인 and this is where that word
//!   is executed.
//! - A cross source that disagrees is **disclosed, not dropped and not
//!   promoted**. [`ConfirmationEvidence::disagreements`] carries every
//!   cross-source reading that contradicts the basis, and there is no method
//!   here that makes a cross source the basis -- not by being newer, not by
//!   being more numerous, and not by being a higher-numbered section 8.4 level.

use academic_curriculum::{Capacity, CourseCode, InstructorName, Meeting};
use academic_domain::TimestampMillis;
use academic_ingestion::{ConnectorId, SourceCategory};
use academic_record::term::TermKey;

use crate::{error::OfferingError, policy::VerificationRecency};

/// One official reading of one course in one term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialListing {
    source: SourceCategory,
    connector: ConnectorId,
    retrieved_at: TimestampMillis,
    term: TermKey,
    course: CourseCode,
    instructors: Vec<InstructorName>,
    capacity: Option<Capacity>,
    meetings: Vec<Meeting>,
    lists_a_section: bool,
}

impl OfficialListing {
    /// Records one reading.
    ///
    /// `lists_a_section` is what the reading found, kept separate from whether
    /// the reading happened: a registration system read and found nothing is a
    /// different fact from a registration system nobody read.
    #[must_use]
    pub fn new(
        source: SourceCategory,
        connector: ConnectorId,
        retrieved_at: TimestampMillis,
        term: TermKey,
        course: CourseCode,
        lists_a_section: bool,
    ) -> Self {
        Self {
            source,
            connector,
            retrieved_at,
            term,
            course,
            instructors: Vec::new(),
            capacity: None,
            meetings: Vec::new(),
            lists_a_section,
        }
    }

    /// Appends one instructor the listing printed.
    #[must_use]
    pub fn instructor(mut self, name: InstructorName) -> Self {
        self.instructors.push(name);
        self
    }

    /// Records the seat count the listing printed.
    #[must_use]
    pub const fn capacity(mut self, capacity: Capacity) -> Self {
        self.capacity = Some(capacity);
        self
    }

    /// Appends one meeting the listing printed.
    #[must_use]
    pub fn meeting(mut self, meeting: Meeting) -> Self {
        self.meetings.push(meeting);
        self
    }

    /// Which of section 8.4's six levels the reading came from.
    #[must_use]
    pub const fn source(&self) -> SourceCategory {
        self.source
    }

    /// The connector that retrieved it.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// When it was retrieved.
    #[must_use]
    pub const fn retrieved_at(&self) -> TimestampMillis {
        self.retrieved_at
    }

    /// The term read.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The course read.
    #[must_use]
    pub const fn course(&self) -> &CourseCode {
        &self.course
    }

    /// Whether the reading found a section.
    #[must_use]
    pub const fn lists_a_section(&self) -> bool {
        self.lists_a_section
    }

    /// The instructors the listing printed.
    #[must_use]
    pub fn instructors(&self) -> &[InstructorName] {
        &self.instructors
    }

    /// The seat count the listing printed, when it printed one.
    #[must_use]
    pub const fn announced_capacity(&self) -> Option<Capacity> {
        self.capacity
    }

    /// The meetings the listing printed.
    #[must_use]
    pub fn meetings(&self) -> &[Meeting] {
        &self.meetings
    }
}

/// A cross source that contradicts the basis, kept beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossSourceDisagreement {
    source: SourceCategory,
    connector: ConnectorId,
    retrieved_at: TimestampMillis,
    said_a_section_exists: bool,
}

impl CrossSourceDisagreement {
    /// Which level disagreed.
    #[must_use]
    pub const fn source(&self) -> SourceCategory {
        self.source
    }

    /// The connector that retrieved the disagreeing reading.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// When the disagreeing reading was retrieved.
    #[must_use]
    pub const fn retrieved_at(&self) -> TimestampMillis {
        self.retrieved_at
    }

    /// What the disagreeing reading said.
    #[must_use]
    pub const fn said_a_section_exists(&self) -> bool {
        self.said_a_section_exists
    }
}

/// A registration-system reading fresh enough to confirm, with its cross
/// sources.
///
/// Private fields, one constructor, and the constructor takes the recency
/// bound: an offering confirmed with no recorded criterion is not a value that
/// exists. `INV-C-002` -- a confirmed offering has a source and a recent
/// verification -- is that sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationEvidence {
    basis: OfficialListing,
    cross_sources: Vec<OfficialListing>,
    disagreements: Vec<CrossSourceDisagreement>,
    verified_at: TimestampMillis,
    recency: VerificationRecency,
}

impl ConfirmationEvidence {
    /// The only constructor.
    ///
    /// # Errors
    ///
    /// [`OfferingError::NotTheRegistrationSystem`] when `basis` came from any
    /// other level of section 8.4, [`OfferingError::BasisListsNoSection`] when
    /// the reading found no section -- which is an observation about the term,
    /// not a confirmation of one -- and [`OfferingError::VerificationStale`]
    /// when the reading was retrieved longer than the recorded bound before
    /// `verified_at`.
    pub fn from_registration_system(
        basis: OfficialListing,
        cross_sources: Vec<OfficialListing>,
        recency: VerificationRecency,
        verified_at: TimestampMillis,
    ) -> Result<Self, OfferingError> {
        if basis.source() != SourceCategory::RegistrationSystem {
            return Err(OfferingError::NotTheRegistrationSystem(
                basis.source().as_str(),
            ));
        }
        if !basis.lists_a_section() {
            return Err(OfferingError::BasisListsNoSection);
        }
        let age = verified_at
            .value()
            .checked_sub(basis.retrieved_at().value())
            .ok_or(OfferingError::VerificationStale)?;
        if age < 0 {
            return Err(OfferingError::VerificationStale);
        }
        let within = i64::try_from(recency.within_millis()).unwrap_or(i64::MAX);
        if age > within {
            return Err(OfferingError::VerificationStale);
        }
        let disagreements = cross_sources
            .iter()
            .filter(|listing| listing.lists_a_section() != basis.lists_a_section())
            .map(|listing| CrossSourceDisagreement {
                source: listing.source(),
                connector: listing.connector().clone(),
                retrieved_at: listing.retrieved_at(),
                said_a_section_exists: listing.lists_a_section(),
            })
            .collect();
        Ok(Self {
            basis,
            cross_sources,
            disagreements,
            verified_at,
            recency,
        })
    }

    /// The registration-system reading this confirmation rests on.
    #[must_use]
    pub const fn basis(&self) -> &OfficialListing {
        &self.basis
    }

    /// Every cross source consulted.
    #[must_use]
    pub fn cross_sources(&self) -> &[OfficialListing] {
        &self.cross_sources
    }

    /// Every cross source that contradicts the basis.
    ///
    /// A non-empty list does not change the basis and does not block the
    /// confirmation. It is disclosed because section 8.3 says the 수강편람 can
    /// change after its own compilation date, so a stale cross source
    /// disagreeing is the expected case and hiding it would lose the reason to
    /// re-check.
    #[must_use]
    pub fn disagreements(&self) -> &[CrossSourceDisagreement] {
        &self.disagreements
    }

    /// The instant the confirmation was made at, which is section 8.3's 확인일.
    #[must_use]
    pub const fn verified_at(&self) -> TimestampMillis {
        self.verified_at
    }

    /// The recorded bound this confirmation was made under.
    #[must_use]
    pub const fn recency(&self) -> VerificationRecency {
        self.recency
    }
}

/// An official notice that a term's offering will not happen.
///
/// Section 8.3's `CANCELLED/WITHDRAWN` row requires 공식 폐강·변경 공지 -- an
/// official cancellation *or change* notice. Both spellings the row gives are
/// this one value: `t068` section 2.3-4 writes the status as `CANCELLED` and
/// migration `0014`'s `CHECK` admits `CANCELLED`, so the withdrawal half of
/// the row's name is a synonym rather than a fifth status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationNotice {
    source: SourceCategory,
    connector: ConnectorId,
    issued_at: TimestampMillis,
    term: TermKey,
    course: CourseCode,
}

impl CancellationNotice {
    /// Records one official cancellation or change notice.
    ///
    /// # Errors
    ///
    /// [`OfferingError::NotTheRegistrationSystem`] when the notice came from a
    /// level that does not publish offering changes. Section 8.3 names two:
    /// the registration system itself, and the department page that publishes
    /// 교과목 변경 내역.
    pub fn official(
        source: SourceCategory,
        connector: ConnectorId,
        issued_at: TimestampMillis,
        term: TermKey,
        course: CourseCode,
    ) -> Result<Self, OfferingError> {
        if !matches!(
            source,
            SourceCategory::RegistrationSystem | SourceCategory::DepartmentPage
        ) {
            return Err(OfferingError::NotTheRegistrationSystem(source.as_str()));
        }
        Ok(Self {
            source,
            connector,
            issued_at,
            term,
            course,
        })
    }

    /// Which level issued it.
    #[must_use]
    pub const fn source(&self) -> SourceCategory {
        self.source
    }

    /// The connector that retrieved it.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// When it was issued.
    #[must_use]
    pub const fn issued_at(&self) -> TimestampMillis {
        self.issued_at
    }

    /// The term cancelled.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The course cancelled.
    #[must_use]
    pub const fn course(&self) -> &CourseCode {
        &self.course
    }
}

/// An official notice that a term's offering **will** happen.
///
/// Section 8.3's `HISTORICALLY_LIKELY` row requires 미래 공식 공지 없음, so a
/// future official notice has to be a value the resolver can be handed. It is
/// not a [`ConfirmationEvidence`]: that row requires the offering to *exist* in
/// the term's listing and to have been *recently verified*, and an
/// announcement is neither. Section 8.3's own sentence says what it does
/// instead -- *공식 향후 공지가 생기면 예측을 사실로 "승격"하지 않고 별도
/// official Claim을 활성화한다* -- and
/// [`crate::claims::announcement_claim`] is that separate claim.
///
/// # Why the 수강편람 does not confirm
///
/// The `CONFIRMED` row's own words admit two sources: *해당 학기 공식
/// 수강편람/수강신청 시스템에 존재하고 최근 확인*. The paragraph under the
/// table is narrower and more specific -- *공식 개설 확인은 수강신청 시스템의
/// 최신 강좌 상세를 기준으로 하고, CSE 홈페이지·수강편람은 교차 출처로
/// 사용한다* -- and adds that the bulletin can change after its own compilation
/// date. The two sentences are in tension and this crate follows the narrower
/// one, which is the fail-closed direction: a bulletin entry is an
/// announcement, not a confirmation, and it produces an official claim without
/// producing a seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferingAnnouncement {
    source: SourceCategory,
    connector: ConnectorId,
    issued_at: TimestampMillis,
    term: TermKey,
    course: CourseCode,
}

impl OfferingAnnouncement {
    /// Records one official notice that the course will run.
    ///
    /// # Errors
    ///
    /// [`OfferingError::NotTheRegistrationSystem`] when the notice came from a
    /// level that publishes no offering changes. The two that do are the
    /// registration system and the department page.
    pub fn official(
        source: SourceCategory,
        connector: ConnectorId,
        issued_at: TimestampMillis,
        term: TermKey,
        course: CourseCode,
    ) -> Result<Self, OfferingError> {
        if !matches!(
            source,
            SourceCategory::RegistrationSystem | SourceCategory::DepartmentPage
        ) {
            return Err(OfferingError::NotTheRegistrationSystem(source.as_str()));
        }
        Ok(Self {
            source,
            connector,
            issued_at,
            term,
            course,
        })
    }

    /// Which level issued it.
    #[must_use]
    pub const fn source(&self) -> SourceCategory {
        self.source
    }

    /// The connector that retrieved it.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// When it was issued.
    #[must_use]
    pub const fn issued_at(&self) -> TimestampMillis {
        self.issued_at
    }

    /// The term announced.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The course announced.
    #[must_use]
    pub const fn course(&self) -> &CourseCode {
        &self.course
    }
}

/// What an official source says about the term being forecast.
///
/// The forecast runs whether or not one of these exists. That is section
/// 30.1's parallel, executed: an official reading decides the *standing*, and
/// the prediction beside it keeps its own probability, its own window and its
/// own claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfficialTermReading {
    /// A registration-system reading, fresh, listing a section.
    Confirmed(ConfirmationEvidence),
    /// An official notice that the course will run, without a verified listing.
    ///
    /// Section 8.3's 미래 공식 공지. It defeats `HISTORICALLY_LIKELY`, whose
    /// row requires the absence of one, and it does not reach `CONFIRMED`,
    /// whose row requires a listing that was recently verified.
    Announced(OfferingAnnouncement),
    /// An official cancellation or change notice.
    Cancelled(CancellationNotice),
}
