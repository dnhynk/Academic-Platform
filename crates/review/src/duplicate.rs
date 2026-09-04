//! Near-duplicate detection, and the one place this crate reads a review.
//!
//! Section 34's *강의평 편향* row names the detection: *sample size/time/
//! instructor distribution, **duplicate similarity***. Section 29.5 asks the
//! aggregate to disclose *중복 가능성*. This module is the measurement behind
//! [`crate::bias::BiasDimension::Duplication`].
//!
//! # The metric, written out
//!
//! Two reviews are compared by the overlap of their word trigrams.
//!
//! 1. **Normalise.** Every character is lowercased; every character that is not
//!    alphanumeric becomes a separator. Words are the non-empty runs between
//!    separators. So punctuation, casing and whitespace do not make two copies
//!    of one text look different.
//! 2. **Shingle.** The trigrams are the windows of three consecutive words, as
//!    a *set*: a text that repeats a phrase contributes that trigram once. A
//!    text of fewer than three words has one shingle, the whole word list, so
//!    two short reviews are still comparable.
//! 3. **Compare.** The similarity is `1000 * |A ∩ B| / |A ∪ B|`, in permille,
//!    with integer division, so it is `0` for disjoint texts and `1000` for a
//!    pair with the same shingle set. There is no floating point anywhere in
//!    it.
//!
//! Both texts contribute symmetrically and neither is privileged:
//! `duplicate_similarity_is_symmetric` swaps the arguments over every pair of a
//! fixture and requires the value to be identical.
//!
//! The expected values in `duplicate_similarity_is_detected` are computed by
//! hand from this definition and written into the test as literals, together
//! with the intersection and union sizes they came from. Nothing in that test
//! asks this module what the answer is, which is the failure `P2-U3` avoided by
//! writing an independent oracle: an engine checked against itself always
//! passes.
//!
//! # Why the text is read here and nowhere else
//!
//! [`crate::text::RawReviewText::content`] is `pub(crate)` and this is its one
//! call site. `raw_review_text_is_excluded_from_export_and_share` requires that
//! -- it finds every call of the accessor across the crate's product source and
//! compares the file set against a one-entry list. A shingle set leaves this
//! module as a count, never as a word.

use std::collections::BTreeSet;

use crate::{error::ReviewError, record::ReviewRecord};

/// A similarity, in permille of the union.
///
/// Bounded at construction, the way `academic_proposal::ImpactPermille` is
/// bounded, so a threshold is a value on a known scale rather than a number
/// somebody hopes is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimilarityPermille(u16);

impl SimilarityPermille {
    /// Takes a similarity from 0 through 1000 inclusive.
    ///
    /// # Errors
    ///
    /// [`ReviewError::SimilarityOutOfRange`] above 1000.
    pub const fn new(value: u16) -> Result<Self, ReviewError> {
        if value > 1000 {
            return Err(ReviewError::SimilarityOutOfRange(value));
        }
        Ok(Self(value))
    }

    /// The value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// One pair of reviews whose texts overlap at or above a threshold.
///
/// The two positions are indices into the slice that was compared, always with
/// `left < right`, so a pair appears once and the order is a property of the
/// input rather than of the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DuplicateFinding {
    left: usize,
    right: usize,
    similarity: SimilarityPermille,
}

impl DuplicateFinding {
    /// The earlier of the two positions.
    #[must_use]
    pub const fn left(self) -> usize {
        self.left
    }

    /// The later of the two positions.
    #[must_use]
    pub const fn right(self) -> usize {
        self.right
    }

    /// How much of the union the two texts share.
    #[must_use]
    pub const fn similarity(self) -> SimilarityPermille {
        self.similarity
    }
}

/// The words of `text`, lowercased, with every non-alphanumeric run dropped.
fn words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The word-trigram set of `text`.
///
/// A text of fewer than three words yields one shingle holding all of them, so
/// two short reviews compare against each other rather than against nothing.
fn shingles(text: &str) -> BTreeSet<String> {
    let words = words(text);
    if words.len() < 3 {
        if words.is_empty() {
            return BTreeSet::new();
        }
        return BTreeSet::from([words.join(" ")]);
    }
    words.windows(3).map(|window| window.join(" ")).collect()
}

/// How much two reviews' texts overlap, in permille of their union.
///
/// This is the one function in this crate that reads a retained text.
#[must_use]
pub fn similarity(left: &ReviewRecord, right: &ReviewRecord) -> SimilarityPermille {
    let left = shingles(left.raw_artifact().content());
    let right = shingles(right.raw_artifact().content());
    let union = left.union(&right).count();
    if union == 0 {
        return SimilarityPermille(0);
    }
    let shared = left.intersection(&right).count();
    // `shared <= union` and `union >= 1`, so the quotient is 0..=1000 and the
    // conversion cannot fail. The arithmetic is integer throughout. The
    // saturating arm is unreachable by that construction rather than a default
    // standing in for a value nobody computed: the only way to reach it is for
    // an intersection to be larger than its own union, and the value it yields
    // -- complete overlap -- is the only reading consistent with that.
    let permille = (shared * 1000) / union;
    SimilarityPermille(u16::try_from(permille).unwrap_or(1000))
}

/// Every pair of `records` whose similarity is at or above `threshold`.
///
/// Ordered by the earlier position and then the later one, so the result is a
/// function of the input rather than of an iteration order.
#[must_use]
pub fn duplicate_findings(
    records: &[ReviewRecord],
    threshold: SimilarityPermille,
) -> Vec<DuplicateFinding> {
    let mut found = Vec::new();
    for left in 0..records.len() {
        for right in (left + 1)..records.len() {
            let similarity = similarity(&records[left], &records[right]);
            if similarity >= threshold {
                found.push(DuplicateFinding {
                    left,
                    right,
                    similarity,
                });
            }
        }
    }
    found
}

/// How many of `records` are in at least one duplicate pair.
///
/// This is the measurement [`crate::bias::BiasDimension::Duplication`]
/// discloses: not how many pairs were found, but how much of the sample may be
/// one text counted twice.
#[must_use]
pub fn duplicated_record_count(records: &[ReviewRecord], threshold: SimilarityPermille) -> u32 {
    let mut involved = BTreeSet::new();
    for finding in duplicate_findings(records, threshold) {
        involved.insert(finding.left());
        involved.insert(finding.right());
    }
    // Saturating rather than wrapping: a sample of more than four billion
    // reviews is not a sample this crate will see, and reporting the maximum is
    // the only honest reading of one.
    u32::try_from(involved.len()).unwrap_or(u32::MAX)
}
