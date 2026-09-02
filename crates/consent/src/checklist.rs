//! The seven dimensions, each answered explicitly or not answered at all.
//!
//! # Why "not applicable" has to be written down
//!
//! Section 12.1 lists what a user checks besides the instructor's permission:
//! the offering's own syllabus or LMS policy, whether student questions and
//! presentations are recorded, how far a screen capture reaches, the
//! accessibility procedure, copyright, privacy, and the institution's rules.
//! Some of those genuinely do not apply to a given offering -- a lecture with
//! no student microphone has no student speech to consider.
//!
//! The failure mode is that "does not apply" and "nobody looked" produce the
//! same empty cell. So [`ChecklistEntry`] has two arms and neither of them is
//! absence: a dimension is [`Evidenced`](ChecklistEntry::Evidenced) with an
//! artifact behind it, or [`NotApplicable`](ChecklistEntry::NotApplicable) with
//! a reason from a closed list, or it is not in the map -- and a dimension not
//! in the map is what [`Checklist::unanswered`] returns and what keeps the
//! status off `PERMITTED`.
//!
//! The reasons are a closed enum rather than free text for two reasons. A
//! reason nobody can enumerate is a reason nothing can review; and a free-text
//! field here would be the crate's only string field, which is the shape the
//! `S-10` row in `docs/contracts/policy-source-scans.md` is about.
//!
//! # What an omission costs
//!
//! It does not deny the recorder. Section 3.7's permitting statuses are
//! `PERMITTED` and `PERMITTED_WITH_CONDITIONS`, and an offering whose
//! instructor granted in writing while one dimension is still open is the
//! second of those, not the first and not `UNKNOWN`. What the omission does is
//! travel: [`Checklist::unanswered`] is copied onto the minted
//! [`CaptureCapabilityToken`](crate::CaptureCapabilityToken), so the exact
//! dimensions nobody answered are visible at the device layer `P2-L1` builds
//! rather than lost between the ledger and the microphone.
//!
//! With no written grant, an answered checklist changes nothing: the status is
//! `UNKNOWN` because no authority spoke. That is the second half of
//! `checklist_omission_yields_conditional_or_unknown`.

/// One thing section 12.1 asks a user to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ChecklistDimension {
    /// The offering's own recording policy, in the syllabus or the LMS.
    SyllabusOrLmsPolicy,
    /// Whether student questions and presentations are captured.
    StudentSpeech,
    /// How far a photograph or screen capture reaches.
    FilmingScope,
    /// The accessibility support procedure for this offering.
    AccessibilityProcedure,
    /// Copyright in the material being captured.
    Copyright,
    /// Personal data in what is captured.
    Privacy,
    /// The institution's own rules.
    InstitutionalRules,
}

/// Every dimension, in the order a checklist reports them.
///
/// Closed and complete. `consent_scans.rs` reads the enum variants out of this
/// file and compares them against the seven the contract names, so a dimension
/// added to the enum without being added to the contract fails, and so does one
/// dropped from either.
pub const CHECKLIST_DIMENSIONS: [ChecklistDimension; 7] = [
    ChecklistDimension::SyllabusOrLmsPolicy,
    ChecklistDimension::StudentSpeech,
    ChecklistDimension::FilmingScope,
    ChecklistDimension::AccessibilityProcedure,
    ChecklistDimension::Copyright,
    ChecklistDimension::Privacy,
    ChecklistDimension::InstitutionalRules,
];

impl ChecklistDimension {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SyllabusOrLmsPolicy => "SYLLABUS_OR_LMS_POLICY",
            Self::StudentSpeech => "STUDENT_SPEECH",
            Self::FilmingScope => "FILMING_SCOPE",
            Self::AccessibilityProcedure => "ACCESSIBILITY_PROCEDURE",
            Self::Copyright => "COPYRIGHT",
            Self::Privacy => "PRIVACY",
            Self::InstitutionalRules => "INSTITUTIONAL_RULES",
        }
    }
}

/// Why a dimension does not apply to this offering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum NotApplicableReason {
    /// The offering has no student microphone or presentation slot.
    NoStudentParticipationIsCaptured,
    /// Nothing visual is captured at all.
    NoVisualCaptureRequested,
    /// The user holds no accommodation for this offering.
    NoAccommodationInEffect,
    /// The material captured is the user's own work.
    MaterialIsTheUsersOwn,
    /// No personal data of a third party is present.
    NoThirdPartyPersonalData,
    /// The institution publishes no rule reaching this offering.
    InstitutionPublishesNoApplicableRule,
}

impl NotApplicableReason {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoStudentParticipationIsCaptured => "NO_STUDENT_PARTICIPATION_IS_CAPTURED",
            Self::NoVisualCaptureRequested => "NO_VISUAL_CAPTURE_REQUESTED",
            Self::NoAccommodationInEffect => "NO_ACCOMMODATION_IN_EFFECT",
            Self::MaterialIsTheUsersOwn => "MATERIAL_IS_THE_USERS_OWN",
            Self::NoThirdPartyPersonalData => "NO_THIRD_PARTY_PERSONAL_DATA",
            Self::InstitutionPublishesNoApplicableRule => {
                "INSTITUTION_PUBLISHES_NO_APPLICABLE_RULE"
            }
        }
    }
}

/// How one dimension was answered. There is no "unknown" arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChecklistEntry {
    /// Something was read, and this is what.
    Evidenced(crate::evidence::EvidenceArtifact),
    /// The dimension does not apply here, for a reason from the closed list.
    NotApplicable(NotApplicableReason),
}

/// The seven dimensions as they stand for one permission record.
///
/// Append-only in the same sense `AttemptHistory` is: [`answer`](Self::answer)
/// is the one mutator, it refuses a dimension that already has an entry, and
/// there is no removal path. A correction is a new
/// [`PermissionRecord`](crate::PermissionRecord) at the next `permission_seq`,
/// which is what the section 3.7 key is for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Checklist {
    entries: Vec<(ChecklistDimension, ChecklistEntry)>,
}

impl Checklist {
    /// An empty checklist. Nothing has been answered.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Answers one dimension.
    pub fn answer(
        &mut self,
        dimension: ChecklistDimension,
        entry: ChecklistEntry,
    ) -> Result<(), crate::ConsentError> {
        if self.entry(dimension).is_some() {
            return Err(crate::ConsentError::DimensionAlreadyAnswered);
        }
        self.entries.push((dimension, entry));
        self.entries.sort_by_key(|(dimension, _)| *dimension);
        Ok(())
    }

    /// The entry for one dimension, if it has one.
    #[must_use]
    pub fn entry(&self, dimension: ChecklistDimension) -> Option<&ChecklistEntry> {
        self.entries
            .iter()
            .find(|(named, _)| *named == dimension)
            .map(|(_, entry)| entry)
    }

    /// The dimensions with no entry, in registry order.
    ///
    /// Built by walking [`CHECKLIST_DIMENSIONS`] rather than by inverting the
    /// map, so the answer is over the whole closed set even when the map is
    /// empty. `plan.rs` in `academic-retention` enumerates its classes the same
    /// way and for the same reason.
    #[must_use]
    pub fn unanswered(&self) -> Vec<ChecklistDimension> {
        CHECKLIST_DIMENSIONS
            .into_iter()
            .filter(|dimension| self.entry(*dimension).is_none())
            .collect()
    }

    /// Whether every dimension has an explicit entry.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unanswered().is_empty()
    }
}
