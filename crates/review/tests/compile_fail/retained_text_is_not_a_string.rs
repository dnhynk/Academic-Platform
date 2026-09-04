//! The retained artifact implements nothing that turns it into a `String`.
//!
//! No `Display`, no `ToString`, no `Into<String>`, no `AsRef<str>`. Those are
//! the shapes that put text into an export at a call site that reads like
//! nothing happened.

use academic_review::RawReviewText;

fn redistribute(text: &RawReviewText) -> String {
    format!("{text}")
}

fn main() {}
