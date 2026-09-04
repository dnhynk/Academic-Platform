//! The non-guarantee notice, and why it has no constructor that takes text.
//!
//! ## It is the document's own words
//!
//! Section 24.3 ends its last sentence with `비교·채용 가능성을 보장하는 수치가
//! 아님을 표시한다`, and section 35's anti-goal table refuses `LinkedIn식
//! career scoring` for `수행의 다차원성과 불확실성 소실`, allowing a
//! `competency × evidence matrix` in its place. Both spans are read back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` by
//! `non_guarantee_disclaimer_survives_export`, so the notice a recipient reads
//! is the specification's own text and a specification that stops saying so
//! fails this crate.
//!
//! ## There is no constructor and no field
//!
//! [`NonGuaranteeNotice::rendered`] takes no argument, and the type has no
//! public field, no `Default`, no setter and no `Deserialize`. So there is no
//! expression anywhere that produces a *different* notice, and no document that
//! carries one in. That is what makes [`crate::view::ReadinessView`]'s
//! deserialization able to refuse a document whose notice is not this one: the
//! comparison is against a value with a single producer rather than against a
//! constant somebody could pass a different string beside.
//!
//! `P2-Y1` uses the same shape for section 24.1's statement — rendered from
//! parts, never supplied — and for the same reason: a sentence a caller can
//! write is a sentence a caller can leave out.

use core::fmt;

use serde::Serialize;

/// Section 24.3's own span, verbatim.
pub const SPECIFICATION_PHRASE: &str = "비교·채용 가능성을 보장하는 수치가 아님";

/// Section 35's anti-goal, verbatim: what this view is not.
pub const REFUSED_PRODUCT: &str = "LinkedIn식 career scoring";

/// Section 35's stated reason for refusing it, verbatim.
pub const REFUSAL_REASON: &str = "수행의 다차원성과 불확실성 소실";

/// Section 35's allowed boundary, verbatim: what this view is instead.
pub const ALLOWED_INSTEAD: &str = "competency × evidence matrix";

/// The notice every readiness view carries.
///
/// One producer, [`NonGuaranteeNotice::rendered`], and no argument to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "String")]
pub struct NonGuaranteeNotice;

impl NonGuaranteeNotice {
    /// The notice.
    ///
    /// Assembled from the four spans above and from nothing a caller supplies.
    #[must_use]
    pub const fn rendered() -> Self {
        Self
    }

    /// Its text.
    #[must_use]
    pub fn text(self) -> String {
        format!("{SPECIFICATION_PHRASE} · {REFUSED_PRODUCT}({REFUSAL_REASON}) → {ALLOWED_INSTEAD}")
    }
}

impl fmt::Display for NonGuaranteeNotice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text())
    }
}

impl From<NonGuaranteeNotice> for String {
    fn from(value: NonGuaranteeNotice) -> Self {
        value.text()
    }
}
