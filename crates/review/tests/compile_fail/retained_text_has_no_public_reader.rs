//! Somebody else's writing does not come out of the retained artifact.
//!
//! The one accessor is `pub(crate)`, so outside this crate no function returns
//! the text and no caller can spell one.

use academic_review::RawReviewText;

fn read(text: &RawReviewText) -> &str {
    text.content()
}

fn main() {}
