//! Two retention bounds, and the rule that a derivative may only narrow them.
//!
//! # Why two values and not one
//!
//! The contract this task fixes says audio and transcript retention are
//! independent values. They are independent because the permissions differ:
//! an instructor who permits a transcript for the term and refuses to let the
//! recording outlive the lecture has stated two bounds, and a model with one
//! field can hold neither of them without silently widening or narrowing the
//! other. `P2-U4` found the same shape twice under a different name -- a
//! conflict collapsed into a single value -- and this is that shape in the
//! retention column.
//!
//! So [`RetentionTerms`] has two fields, two accessors, and no accessor that
//! returns one for the other. `consent_scans.rs` reads the struct out of this
//! file and requires exactly two [`RetentionBound`] fields with different
//! names; `audio_and_transcript_retention_are_independent` is the behavioural
//! half, and it is written so that a collapse which still compiles -- an
//! accessor returning the sibling field -- fails it.
//!
//! # Why a derivative can only become stricter
//!
//! A transcript, an embedding, a summary, and a cache all exist because the
//! recording did. If a derivative could carry a later bound than the thing it
//! was derived from, the bound on the original would mean nothing: deleting the
//! recording on time would leave a copy of its content behind, lawfully
//! labelled. So [`RetentionTerms::inherit`] takes the stricter of the two
//! bounds on each axis independently, and there is no second inheritance
//! function and no argument that reverses it.
//!
//! The direction is one comparison, so getting it backwards is one character.
//! `derivative_expiry_is_equal_or_stricter` therefore does not check a case: it
//! walks the whole cross product of a bound grid and requires
//! `derived <= parent` on both axes for every pair, so a `max` in place of the
//! `min` fails on the first pair where the two differ rather than on a case
//! somebody remembered to write.

/// How long one medium may be kept.
///
/// [`RetentionBound::Prohibited`] is not "zero milliseconds". It is the state
/// where the medium may not be retained at all, and it orders below every
/// instant, so it wins every [`RetentionBound::stricter`] comparison. There is
/// no `Unknown` variant, because a bound is a required field of a grant and a
/// grant is the only place a bound comes from: an unset bound is not
/// representable rather than defaulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RetentionBound {
    /// The medium may not be retained. Strictly below every instant.
    Prohibited,
    /// The medium may be retained until this instant, exclusive.
    Until(u64),
}

impl RetentionBound {
    /// The stricter of two bounds.
    ///
    /// `Prohibited` is the enum's first variant and `Until` its second, so the
    /// derived [`Ord`] already ranks `Prohibited` below every instant and
    /// `Until(a)` below `Until(b)` when `a < b`. That is the whole ordering,
    /// and it is derived rather than hand-written so a variant added later
    /// cannot be ranked into the middle of it by accident.
    #[must_use]
    pub fn stricter(self, other: Self) -> Self {
        self.min(other)
    }

    /// Whether this bound has been reached at `at`.
    #[must_use]
    pub const fn is_expired_at(self, at: u64) -> bool {
        match self {
            Self::Prohibited => true,
            Self::Until(until) => at >= until,
        }
    }

    /// The stable external spelling of the variant.
    #[must_use]
    pub const fn kind_str(self) -> &'static str {
        match self {
            Self::Prohibited => "PROHIBITED",
            Self::Until(_) => "UNTIL",
        }
    }
}

/// The audio bound and the transcript bound, held apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetentionTerms {
    audio: RetentionBound,
    transcript: RetentionBound,
}

impl RetentionTerms {
    /// States both bounds.
    ///
    /// Both are required arguments in a fixed order. There is no constructor
    /// taking one bound and applying it to both, because that constructor is
    /// precisely the collapse this type exists to refuse.
    #[must_use]
    pub const fn new(audio: RetentionBound, transcript: RetentionBound) -> Self {
        Self { audio, transcript }
    }

    /// How long the captured audio may be kept.
    #[must_use]
    pub const fn audio(self) -> RetentionBound {
        self.audio
    }

    /// How long a transcript of that audio may be kept.
    #[must_use]
    pub const fn transcript(self) -> RetentionBound {
        self.transcript
    }

    /// The terms a derivative of this subject inherits.
    ///
    /// Each axis independently, and never wider than this one's. `requested` is
    /// what the derivative asks for; the result is the stricter of the two on
    /// each axis, so asking for more is not an error and is not honoured
    /// either.
    #[must_use]
    pub fn inherit(self, requested: Self) -> Self {
        Self {
            audio: self.audio.stricter(requested.audio),
            transcript: self.transcript.stricter(requested.transcript),
        }
    }

    /// Whether this pair is no wider than `parent` on either axis.
    ///
    /// The predicate [`inherit`](Self::inherit) is written to satisfy, stated
    /// separately so a test can assert it against results this module did not
    /// produce.
    #[must_use]
    pub fn is_no_wider_than(self, parent: Self) -> bool {
        self.audio <= parent.audio && self.transcript <= parent.transcript
    }
}
