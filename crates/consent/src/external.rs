//! A legal exception is an external task. This module is where it stops.
//!
//! # What section 12.1 asks for
//!
//! "An exception that needs a legal judgement is not something the system
//! estimates; it stays as an item for the institution's responsible office or
//! for a professional to confirm." That sentence is a prohibition on an output,
//! not a request for a feature. The feature it does ask for is the referral.
//!
//! # How the prohibition is executable
//!
//! [`ExternalReviewTask`] has no resolution API. There is no `resolve`, no
//! `conclude`, no `answer`, and no field holding a determination -- so a caller
//! holding one has nothing to read off it that could move a status. Three
//! things keep it that way:
//!
//! * The type carries a question and a referral target and nothing else, so
//!   there is no place to put a conclusion without editing this file.
//! * `no_legal_conclusion_reaches_a_permission` in `consent_scans.rs` refuses
//!   any signature in this crate that takes a [`LegalQuestion`] or an
//!   [`ExternalReviewTask`] and returns a
//!   [`CaptureStatus`](crate::CaptureStatus), an
//!   [`AuthorityGrant`](crate::AuthorityGrant), a
//!   [`BoundPermission`](crate::BoundPermission), or a
//!   [`CaptureCapabilityToken`](crate::CaptureCapabilityToken).
//! * [`open_external_review`] returns the task and takes no ledger, so opening
//!   one cannot be the same act as recording anything.
//!   [`ConsentLedger::record_external_review`](crate::ConsentLedger::record_external_review)
//!   is the separate append, and it reads two enum fields off the task and no
//!   third.
//!
//! `legal_exception_is_an_external_task_not_an_inference` is the behavioural
//! half: a scope with an open review has the same status it had before the
//! review was opened, and no capability is mintable for it that was not
//! mintable before.

use academic_domain::OfferingId;

use crate::permission::TermKey;

/// A question this system refuses to answer.
///
/// Closed, and every arm is a question of law or institutional authority rather
/// than of fact. A question of fact belongs in the checklist, where the answer
/// is an artifact; a question on this list has no artifact that settles it,
/// which is exactly why it leaves the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum LegalQuestion {
    /// Whether a copyright exception covers this capture.
    CopyrightExceptionApplies,
    /// Whether an accessibility accommodation overrides a refusal.
    AccommodationOverridesRefusal,
    /// Whether recorded student speech may be retained without each speaker's
    /// consent.
    StudentSpeechRetentionIsLawful,
    /// Whether the institution's rules permit this capture where the instructor
    /// is silent.
    InstitutionalRulePermitsWhereInstructorIsSilent,
    /// Whether a capture may cross a border for processing.
    CrossBorderProcessingIsLawful,
}

impl LegalQuestion {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CopyrightExceptionApplies => "COPYRIGHT_EXCEPTION_APPLIES",
            Self::AccommodationOverridesRefusal => "ACCOMMODATION_OVERRIDES_REFUSAL",
            Self::StudentSpeechRetentionIsLawful => "STUDENT_SPEECH_RETENTION_IS_LAWFUL",
            Self::InstitutionalRulePermitsWhereInstructorIsSilent => {
                "INSTITUTIONAL_RULE_PERMITS_WHERE_INSTRUCTOR_IS_SILENT"
            }
            Self::CrossBorderProcessingIsLawful => "CROSS_BORDER_PROCESSING_IS_LAWFUL",
        }
    }
}

/// Who the question goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ReferralTarget {
    /// The department office responsible for the offering.
    DepartmentOffice,
    /// The office that issues accessibility accommodations.
    AccessibilityOffice,
    /// The institution's legal or compliance function.
    InstitutionalLegalOffice,
    /// A qualified professional outside the institution.
    ExternalProfessional,
}

impl ReferralTarget {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DepartmentOffice => "DEPARTMENT_OFFICE",
            Self::AccessibilityOffice => "ACCESSIBILITY_OFFICE",
            Self::InstitutionalLegalOffice => "INSTITUTIONAL_LEGAL_OFFICE",
            Self::ExternalProfessional => "EXTERNAL_PROFESSIONAL",
        }
    }
}

/// A question that left the system, and did not come back as an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalReviewTask {
    offering_id: OfferingId,
    term: TermKey,
    question: LegalQuestion,
    referred_to: ReferralTarget,
    opened_at: u64,
}

impl ExternalReviewTask {
    /// Which offering the question is about.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Which term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// What was asked.
    #[must_use]
    pub const fn question(&self) -> LegalQuestion {
        self.question
    }

    /// Who it was asked of.
    #[must_use]
    pub const fn referred_to(&self) -> ReferralTarget {
        self.referred_to
    }

    /// When it was opened.
    #[must_use]
    pub const fn opened_at(&self) -> u64 {
        self.opened_at
    }
}

/// Refers a legal question outside this system.
///
/// It takes no ledger, no permission record, and no status, and it returns a
/// task. There is no variant of this function that also decides something.
#[must_use]
pub const fn open_external_review(
    offering_id: OfferingId,
    term: TermKey,
    question: LegalQuestion,
    referred_to: ReferralTarget,
    opened_at: u64,
) -> ExternalReviewTask {
    ExternalReviewTask {
        offering_id,
        term,
        question,
        referred_to,
        opened_at,
    }
}
