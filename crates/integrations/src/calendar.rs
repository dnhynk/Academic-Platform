//! The calendar payload: identifiers, a kind and two instants, and nothing a
//! grade or a knowledge state could ride out on.
//!
//! Section 33's calendar row keeps an offering or deadline event identifier and
//! its boundary column is one clause: grades and knowledge state are not
//! transmitted.
//!
//! ## Why the field types are the guard
//!
//! An absence is not established by a name list. A `grade` field would be
//! caught by a list; `points: u32`, `band: u8` and `note: String` would not,
//! and this run measured a shared secret-`Debug` scan passing twelve of twelve
//! over a `String` field a later injection walked straight through.
//!
//! So the guard here is a classification of the **whole** field set, at this
//! boundary, in two layers that fail for different reasons:
//!
//! * `calendar_payload_contains_no_grade_or_state` reads this struct's field
//!   list out of this file and requires every field's type to be one of the four
//!   types this module admits -- [`ExternalId`], [`CanonicalRef`],
//!   [`CalendarEventKind`] and `TimestampMillis`. A `String`, an `f64`, a `u32`
//!   or a `MasteryLevel` fails as an inadmissible *type*, whatever it is named;
//! * the same test compares the whole `(name, type)` set against a pinned
//!   inventory, so a field of an admitted type but a new name fails as an added
//!   key. A reviewer who widens the type list still has to add the field.
//!
//! There is deliberately **no free-text field at all**, not even a title. What
//! section 33 says this system keeps is the event identifier; a human-readable
//! label is a decision about what a label may hold, and inventing one here
//! would be inventing exactly the position this task was told to close.
//! [`CalendarPayload::summary`] returns a `&'static str` chosen by
//! [`CalendarEventKind`], so what the provider displays is one of a closed set
//! of words this crate compiled in.
//!
//! ## The byte half
//!
//! `calendar_payload_contains_no_grade_or_state` also encodes a payload built
//! for a subject that *does* carry a grade and a mastery level in its fixture,
//! and scans the encoded bytes for the whole set of grade symbols and knowledge
//! state levels -- read out of `crates/record/src/grade.rs` and
//! `crates/domain/src/lib.rs` rather than transcribed, so a variant added there
//! is scanned for here without anyone editing a list. The same scanner is run
//! against a deliberately leaky buffer and required to find them, so a scanner
//! that matched nothing would not pass.

use academic_domain::TimestampMillis;
use sha2::{Digest as _, Sha256};

use crate::identity::{CanonicalRef, ExternalId};

/// Why a payload was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarError {
    /// The interval was empty or ran backwards.
    #[error("a calendar interval is a non-empty half-open interval")]
    MalformedInterval,
    /// The subject was not something a calendar event can be about.
    #[error("a calendar event is about an offering or a course")]
    UnsupportedSubject,
}

/// What kind of thing a calendar event marks.
///
/// A closed vocabulary, because it is also the only text the payload carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalendarEventKind {
    /// A scheduled meeting of one offering.
    OfferingSession,
    /// An assignment due instant.
    AssignmentDeadline,
    /// An examination window.
    ExamWindow,
    /// A registration window.
    RegistrationWindow,
}

impl CalendarEventKind {
    /// Exhaustive order.
    pub const ALL: [Self; 4] = [
        Self::OfferingSession,
        Self::AssignmentDeadline,
        Self::ExamWindow,
        Self::RegistrationWindow,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfferingSession => "OFFERING_SESSION",
            Self::AssignmentDeadline => "ASSIGNMENT_DEADLINE",
            Self::ExamWindow => "EXAM_WINDOW",
            Self::RegistrationWindow => "REGISTRATION_WINDOW",
        }
    }

    /// The words a provider displays for this kind.
    ///
    /// A total function into `&'static str`, so every word a calendar shows was
    /// compiled into this crate. There is no path by which a value read out of
    /// the record becomes display text.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::OfferingSession => "Course session",
            Self::AssignmentDeadline => "Assignment due",
            Self::ExamWindow => "Exam",
            Self::RegistrationWindow => "Registration",
        }
    }
}

/// One event this system offers a calendar.
///
/// Five fields. Their whole `(name, type)` set is pinned and every type is one
/// of the four this module admits -- see the module documentation for why that
/// is two independent failures rather than one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarPayload {
    event_id: ExternalId,
    subject: CanonicalRef,
    kind: CalendarEventKind,
    starts_at: TimestampMillis,
    ends_at: TimestampMillis,
}

impl CalendarPayload {
    /// Builds one event.
    ///
    /// # Errors
    ///
    /// [`CalendarError::MalformedInterval`] when the interval is empty or
    /// backwards, and [`CalendarError::UnsupportedSubject`] when the subject is
    /// not an offering or a course -- an artifact, an entity or a repository is
    /// not a thing a calendar has a slot for, and admitting one would be a way
    /// to address something the user's provider has no business holding.
    pub fn new(
        event_id: ExternalId,
        subject: CanonicalRef,
        kind: CalendarEventKind,
        starts_at: TimestampMillis,
        ends_at: TimestampMillis,
    ) -> Result<Self, CalendarError> {
        if starts_at.value() >= ends_at.value() {
            return Err(CalendarError::MalformedInterval);
        }
        if !matches!(subject, CanonicalRef::Offering(_) | CanonicalRef::Course(_)) {
            return Err(CalendarError::UnsupportedSubject);
        }
        Ok(Self {
            event_id,
            subject,
            kind,
            starts_at,
            ends_at,
        })
    }

    /// The provider's own event identifier. A mapping, never a canonical one.
    #[must_use]
    pub const fn event_id(&self) -> &ExternalId {
        &self.event_id
    }

    /// What the event is about.
    #[must_use]
    pub const fn subject(&self) -> CanonicalRef {
        self.subject
    }

    /// What kind of event it is.
    #[must_use]
    pub const fn kind(&self) -> CalendarEventKind {
        self.kind
    }

    /// When it starts.
    #[must_use]
    pub const fn starts_at(&self) -> TimestampMillis {
        self.starts_at
    }

    /// When it ends. Half-open: this instant is already outside.
    #[must_use]
    pub const fn ends_at(&self) -> TimestampMillis {
        self.ends_at
    }

    /// The words a provider displays. One of four, chosen by [`kind`].
    ///
    /// [`kind`]: CalendarPayload::kind
    #[must_use]
    pub const fn summary(&self) -> &'static str {
        self.kind.summary()
    }

    /// The exact bytes handed to a provider.
    ///
    /// Length-prefixed and in field order, so two encodings of one payload are
    /// byte-identical and a reader cannot be confused by a delimiter inside a
    /// value. The subject appears as its sixteen opaque identifier bytes: an
    /// event carries a *reference*, and there is no branch here that reads
    /// anything the reference points at.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_text(&mut out, self.event_id.as_str());
        push_text(&mut out, self.subject.kind().as_str());
        out.extend_from_slice(self.subject.as_bytes());
        push_text(&mut out, self.kind.as_str());
        push_text(&mut out, self.kind.summary());
        out.extend_from_slice(&self.starts_at.value().to_be_bytes());
        out.extend_from_slice(&self.ends_at.value().to_be_bytes());
        out
    }

    /// The digest of [`encode`]'s bytes.
    ///
    /// [`encode`]: CalendarPayload::encode
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"academic-integrations/calendar-payload/v1\0");
        hasher.update(self.encode());
        hasher.finalize().into()
    }
}

fn push_text(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    out.extend_from_slice(value.as_bytes());
}
