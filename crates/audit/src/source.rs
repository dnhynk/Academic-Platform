//! Where a rule came from, at the granularity section 11.3 requires on a leaf.
//!
//! Section 11.3: *모든 leaf에는 적용 rule ID, source page/paragraph, 사용한
//! CourseAttempt, equivalency decision이 붙는다.*
//!
//! `academic_requirement::ExecutableRule` carries the digest of the official
//! snapshot it was read out of and no position inside it, which is the right
//! boundary for that crate -- a rule's meaning does not depend on which page
//! printed it. The position is the audit's obligation, so it is recorded here
//! and it is **required**: [`RuleSourceSpan`] has private fields, one
//! constructor, and no arm that stands for "the page is not known". A rule with
//! no recorded span is not evaluated at all -- see [`crate::bind`] -- and is
//! reported as a missing check, because a leaf that could not say where its
//! rule came from would be a verdict without a citation.
//!
//! # A page inside another document is not this rule's page
//!
//! A span names a paragraph *inside a snapshot*, and the rule names the
//! snapshot it was read out of. Those are two recorded digests, and while
//! nothing compared them a leaf could cite a paragraph of a document its rule
//! was never read from -- the index is keyed by rule identifier, and an
//! identifier is not a document. Measured on this tree, an index built with a
//! digest no published rule rests on gave `DETERMINATE NOT_POSSIBLE` with no
//! outstanding check and thirteen leaves citing that other snapshot.
//!
//! So there is no accessor that hands out a span for a bare identifier.
//! [`RuleSourceIndex::placement`] takes the published set the identifier
//! belongs to and returns [`Placement`], whose three arms are the three things
//! that can be true: the recorded page is this rule's page, this index has no
//! page for it, or the recorded page is inside another document. The last two
//! are different missing checks and neither is a leaf.

use academic_domain::{ArtifactId, ContentDigest, EvidenceLocator, engines::SourceLocator};
use academic_requirement::{RuleId, RuleSet};
use std::collections::BTreeMap;

use crate::error::AuditError;

/// The page and the paragraph one published rule was read from.
///
/// Both halves are required and neither is an `Option`. Section 11.3 writes
/// *page/paragraph* as one thing, and the two are carried as the two
/// `EvidenceLocator` arms that say them exactly: a page number, and the byte
/// span of the paragraph inside the official snapshot whose digest the rule
/// already carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSourceSpan {
    artifact: ArtifactId,
    source_digest: ContentDigest,
    page: u32,
    paragraph_start: u64,
    paragraph_end: u64,
}

impl RuleSourceSpan {
    /// Records where a rule was read from.
    ///
    /// Refuses a page of zero and an empty or inverted paragraph span, both
    /// through `EvidenceLocator::validate`, so a span that cannot be pointed at
    /// is not a value.
    pub fn new(
        artifact: ArtifactId,
        source_digest: ContentDigest,
        page: u32,
        paragraph_start: u64,
        paragraph_end: u64,
    ) -> Result<Self, AuditError> {
        let span = Self {
            artifact,
            source_digest,
            page,
            paragraph_start,
            paragraph_end,
        };
        for locator in span.locators() {
            locator.locator.validate()?;
        }
        Ok(span)
    }

    /// The artifact the span belongs to.
    #[must_use]
    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    /// The digest of the official snapshot the paragraph is inside.
    #[must_use]
    pub const fn source_digest(&self) -> ContentDigest {
        self.source_digest
    }

    /// The page.
    #[must_use]
    pub const fn page(&self) -> u32 {
        self.page
    }

    /// The paragraph's byte span.
    #[must_use]
    pub const fn paragraph(&self) -> (u64, u64) {
        (self.paragraph_start, self.paragraph_end)
    }

    /// Both locators, in the order `ProofNode::validate` requires.
    ///
    /// The proof node requires `source_locators` to be sorted and deduplicated
    /// by canonical text; `@page/` sorts before `@text/`, and the sort is done
    /// here rather than relied on, so a rendering change cannot make a valid
    /// tree invalid.
    #[must_use]
    pub fn locators(&self) -> Vec<SourceLocator> {
        let mut locators = vec![
            SourceLocator {
                artifact_id: self.artifact,
                locator: EvidenceLocator::Page {
                    page_number: self.page,
                },
            },
            SourceLocator {
                artifact_id: self.artifact,
                locator: EvidenceLocator::TextBytes {
                    source_digest: self.source_digest,
                    start: self.paragraph_start,
                    end: self.paragraph_end,
                },
            },
        ];
        locators.sort_by_key(SourceLocator::canonical_text);
        locators
    }

    /// The canonical text this span contributes to the audit's input digest.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        format!(
            "{} page {} paragraph {}-{} in {}",
            self.artifact, self.page, self.paragraph_start, self.paragraph_end, self.source_digest
        )
    }
}

/// What the index has for one rule.
///
/// Three arms because three things can be true, and only the first is a
/// citation: the recorded page is inside the snapshot the rule rests on, no
/// page was recorded at all, or a page was recorded inside a different
/// document. The last is the one that had no arm -- it was read as the first,
/// and the leaf cited a document its rule was never read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement<'index> {
    /// The recorded page and paragraph, inside the rule's own snapshot.
    InItsOwnSource(&'index RuleSourceSpan),
    /// This index has no page for the rule.
    ///
    /// Either nobody recorded one, or the set does not publish the identifier
    /// at all -- and both are the same thing here: there is no citation for it,
    /// so it cannot become a leaf.
    Absent,
    /// A page was recorded inside a document this rule was not read from.
    AnotherDocument {
        /// The snapshot the recorded span points inside.
        cited: ContentDigest,
        /// The snapshot the rule itself rests on.
        rests_on: ContentDigest,
    },
}

/// Where each published rule was read from.
///
/// A map with no fallback: [`RuleSourceIndex::placement`] is
/// [`Placement::Absent`] for a rule nobody placed, and
/// [`crate::engine::DegreeAudit`] turns that into an unevaluated rule rather
/// than into a leaf with a made-up citation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleSourceIndex {
    spans: BTreeMap<RuleId, RuleSourceSpan>,
}

impl RuleSourceIndex {
    /// An index that places no rule.
    ///
    /// The only `Default` in this crate, and it is emptiness rather than a
    /// value: an empty index places nothing, so every rule is unevaluated and
    /// the audit is `INDETERMINATE`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            spans: BTreeMap::new(),
        }
    }

    /// Records where one rule was read from.
    #[must_use]
    pub fn with(mut self, rule: RuleId, span: RuleSourceSpan) -> Self {
        self.spans.insert(rule, span);
        self
    }

    /// Where one rule was read from, when the recorded page is its own.
    ///
    /// The published set is a parameter and that is the check: the set knows
    /// the digest of the snapshot each of its rules was read out of, the span
    /// carries the digest of the snapshot its paragraph is inside, and this is
    /// the one place in the crate the two are compared. A caller cannot ask for
    /// a span without supplying the set to compare it against -- it cannot
    /// supply the digest either, so it cannot supply the span's own -- and a
    /// leaf citing another document is therefore not a branch somebody has to
    /// remember to write.
    #[must_use]
    pub fn placement(&self, rules: &RuleSet, rule: &RuleId) -> Placement<'_> {
        let (Some(span), Some(published)) = (self.spans.get(rule), rules.rule(rule)) else {
            return Placement::Absent;
        };
        if span.source_digest() == published.source_digest() {
            Placement::InItsOwnSource(span)
        } else {
            Placement::AnotherDocument {
                cited: span.source_digest(),
                rests_on: published.source_digest(),
            }
        }
    }

    /// Every recorded placement, by rule.
    pub fn entries(&self) -> impl Iterator<Item = (&RuleId, &RuleSourceSpan)> {
        self.spans.iter()
    }
}
