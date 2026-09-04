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

use academic_domain::{ArtifactId, ContentDigest, EvidenceLocator, engines::SourceLocator};
use academic_requirement::RuleId;
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

/// Where each published rule was read from.
///
/// A map with no fallback: [`RuleSourceIndex::span`] returns `None` for a rule
/// nobody placed, and [`crate::bind::BoundRuleSet::bind`] turns that into an
/// unevaluated rule rather than into a leaf with a made-up citation.
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

    /// Where one rule was read from, when it was recorded.
    #[must_use]
    pub fn span(&self, rule: &RuleId) -> Option<&RuleSourceSpan> {
        self.spans.get(rule)
    }

    /// Every recorded placement, by rule.
    pub fn entries(&self) -> impl Iterator<Item = (&RuleId, &RuleSourceSpan)> {
        self.spans.iter()
    }
}
