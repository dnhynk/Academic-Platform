//! Section 32.10's three per-file attributes, and why none of them has a
//! default.
//!
//! *export 파일에는 sensitivity label, sharing restriction, source copyright
//! notice를 포함.* A file in a bundle carries all three or the bundle is not
//! written, and that is a property of the types rather than of a check
//! somewhere: [`crate::bundle::FileRecord`] takes all three as constructor
//! parameters, has private fields, no setter and no `Default`, so a file
//! without a notice is not a value that exists.
//!
//! # One of the three is derived, and two are recorded and then checked
//!
//! A **sharing restriction** is a consequence of the label, so it is derived
//! and cannot be set to disagree with it.
//!
//! A **sensitivity label** is recorded per security domain and then checked.
//! `Confidentiality` is a column on an artifact and on nothing else: a claim,
//! an event and a decision carry none, so a claims file's label is not readable
//! anywhere. What is readable is the domain the row belongs to, which is the
//! policy boundary section 32.2 draws and the vault keys by. So the caller
//! records the label covering each domain, and the writer refuses a recorded
//! label weaker than the strongest confidentiality that domain's own artifacts
//! carry — the number is stated by whoever knows it and is still not allowed to
//! contradict the ledger.
//!
//! A **source copyright notice** is not derivable from anything in the ledger:
//! who holds copyright in a lecture recording, and on what terms it may be
//! kept, is a fact about the world. Section 37 says the export must respect
//! 학교 강의 저작물의 보존·사용 조건 *그대로*, so the notice is recorded by the
//! caller in a [`TermsRegister`] and the export **fails closed** on a security
//! domain the register does not name. There is no fallback string, because a
//! fallback is how a bundle ends up asserting terms nobody stated.
//!
//! # The restriction is a function of the label, not a second opinion
//!
//! Section 32.10 lists the two separately, and a bundle that let a caller set
//! them independently would let a `SECRET` file ship as freely redistributable
//! with both fields populated and the manifest looking complete.
//! `sharing_restriction_is_a_total_function_of_sensitivity` pins the whole
//! four-row mapping, so widening one cell is a visible edit to a table rather
//! than a plausible sentence in a review.

use std::collections::BTreeMap;

use academic_domain::Confidentiality;
use serde::{Deserialize, Serialize};

use crate::{ExportError, ExportResult};

/// Section 32.10's sensitivity label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SensitivityLabel {
    /// Redistributable as it stands.
    Public,
    /// About the user, and nobody else.
    Personal,
    /// Held under a source's terms.
    Restricted,
    /// Not to be disclosed at all.
    Secret,
}

impl SensitivityLabel {
    /// Every label, weakest first.
    pub const ALL: [Self; 4] = [Self::Public, Self::Personal, Self::Restricted, Self::Secret];

    /// The label of one registered artifact's confidentiality.
    #[must_use]
    pub const fn of(confidentiality: Confidentiality) -> Self {
        match confidentiality {
            Confidentiality::Public => Self::Public,
            Confidentiality::Personal => Self::Personal,
            Confidentiality::Restricted => Self::Restricted,
            Confidentiality::Secret => Self::Secret,
        }
    }

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Personal => "PERSONAL",
            Self::Restricted => "RESTRICTED",
            Self::Secret => "SECRET",
        }
    }

    /// How exposing this label is, as an explicit rank.
    ///
    /// Written out rather than derived from the declaration order, because a
    /// derived order silently follows a reordering of the variants and this one
    /// decides what a mixed file is labelled.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Personal => 1,
            Self::Restricted => 2,
            Self::Secret => 3,
        }
    }

    /// The more exposing of two labels.
    ///
    /// A file derived from several artifacts carries the strongest label any of
    /// them carries. Taking the weaker one would let one `PUBLIC` contribution
    /// launder a `SECRET` one into a redistributable rendering.
    #[must_use]
    pub fn strongest(self, other: Self) -> Self {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }

    /// The strongest label over a sequence, or [`Self::Public`] when empty.
    ///
    /// Empty means the file is derived from nothing sensitive — the format
    /// marker, the embedded schema — and `PUBLIC` is the honest label for a
    /// constant this repository publishes anyway.
    pub fn strongest_of(labels: impl IntoIterator<Item = Self>) -> Self {
        labels.into_iter().fold(Self::Public, |accumulated, label| {
            accumulated.strongest(label)
        })
    }
}

/// Section 32.10's sharing restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SharingRestriction {
    /// May be redistributed as it stands.
    RedistributionPermitted,
    /// May be kept and read by the user it is about, and not published.
    PersonalUseOnly,
    /// May not be redistributed without the source's permission.
    NoRedistributionWithoutSourcePermission,
    /// May not be disclosed.
    NoDisclosure,
}

impl SharingRestriction {
    /// Every restriction, in the order the labels above produce them.
    pub const ALL: [Self; 4] = [
        Self::RedistributionPermitted,
        Self::PersonalUseOnly,
        Self::NoRedistributionWithoutSourcePermission,
        Self::NoDisclosure,
    ];

    /// The restriction one sensitivity label carries.
    #[must_use]
    pub const fn of(label: SensitivityLabel) -> Self {
        match label {
            SensitivityLabel::Public => Self::RedistributionPermitted,
            SensitivityLabel::Personal => Self::PersonalUseOnly,
            SensitivityLabel::Restricted => Self::NoRedistributionWithoutSourcePermission,
            SensitivityLabel::Secret => Self::NoDisclosure,
        }
    }

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RedistributionPermitted => "REDISTRIBUTION_PERMITTED",
            Self::PersonalUseOnly => "PERSONAL_USE_ONLY",
            Self::NoRedistributionWithoutSourcePermission => {
                "NO_REDISTRIBUTION_WITHOUT_SOURCE_PERMISSION"
            }
            Self::NoDisclosure => "NO_DISCLOSURE",
        }
    }
}

/// The retention and use terms of the source a file came from.
///
/// Private field and one fallible constructor: an empty notice is not a value
/// this type can hold, so "the notice is present" and "the notice says
/// something" are the same statement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CopyrightNotice(String);

impl CopyrightNotice {
    /// Records one notice.
    pub fn new(text: impl Into<String>) -> ExportResult<Self> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(ExportError::Malformed {
                item: "source copyright notice",
                value: text,
            });
        }
        if text.contains('\n') || text.contains('\r') {
            return Err(ExportError::Malformed {
                item: "source copyright notice",
                value: text,
            });
        }
        Ok(Self(text))
    }

    /// The recorded text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The label and the notice covering one security domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainTerms {
    sensitivity: SensitivityLabel,
    notice: CopyrightNotice,
}

impl DomainTerms {
    /// Records the terms covering one security domain.
    #[must_use]
    pub const fn new(sensitivity: SensitivityLabel, notice: CopyrightNotice) -> Self {
        Self {
            sensitivity,
            notice,
        }
    }

    /// The label every file in the domain carries.
    #[must_use]
    pub const fn sensitivity(&self) -> SensitivityLabel {
        self.sensitivity
    }

    /// The restriction that label produces.
    #[must_use]
    pub const fn sharing_restriction(&self) -> SharingRestriction {
        SharingRestriction::of(self.sensitivity)
    }

    /// The terms the source stated.
    #[must_use]
    pub const fn notice(&self) -> &CopyrightNotice {
        &self.notice
    }
}

/// One set of terms per security domain, plus the bundle's own.
///
/// The register is a required part of a [`crate::BundleRequest`]. It has no
/// `Default` and no wildcard entry: [`Self::for_domain`] returns
/// [`ExportError::NoticeAbsent`] rather than a placeholder, and the writer
/// resolves terms for every domain before it writes a byte, so a bundle either
/// states terms for every domain it carries or is not published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermsRegister {
    bundle: CopyrightNotice,
    by_domain: BTreeMap<String, DomainTerms>,
}

impl TermsRegister {
    /// Opens a register with the notice covering the bundle's own files.
    ///
    /// The manifest, the format marker, the embedded schema and the inventory
    /// are this repository's own text, so they are covered by one notice the
    /// caller states rather than by a domain that does not own them.
    #[must_use]
    pub fn new(bundle: CopyrightNotice) -> Self {
        Self {
            bundle,
            by_domain: BTreeMap::new(),
        }
    }

    /// Records the terms covering one security domain.
    #[must_use]
    pub fn with_domain(mut self, domain_id: impl Into<String>, terms: DomainTerms) -> Self {
        self.by_domain.insert(domain_id.into(), terms);
        self
    }

    /// The notice covering the bundle's own generated files.
    #[must_use]
    pub const fn bundle_notice(&self) -> &CopyrightNotice {
        &self.bundle
    }

    /// The terms covering one security domain, failing closed when absent.
    pub fn for_domain(&self, domain_id: &str) -> ExportResult<&DomainTerms> {
        self.by_domain
            .get(domain_id)
            .ok_or_else(|| ExportError::NoticeAbsent {
                domain_id: domain_id.to_owned(),
            })
    }

    /// Every security domain the register names, in canonical order.
    pub fn domains(&self) -> impl Iterator<Item = &str> {
        self.by_domain.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use academic_domain::Confidentiality;

    use super::{
        CopyrightNotice, DomainTerms, SensitivityLabel, SharingRestriction, TermsRegister,
    };

    #[test]
    fn sharing_restriction_is_a_total_function_of_sensitivity() {
        let pinned = [
            (
                SensitivityLabel::Public,
                SharingRestriction::RedistributionPermitted,
            ),
            (
                SensitivityLabel::Personal,
                SharingRestriction::PersonalUseOnly,
            ),
            (
                SensitivityLabel::Restricted,
                SharingRestriction::NoRedistributionWithoutSourcePermission,
            ),
            (SensitivityLabel::Secret, SharingRestriction::NoDisclosure),
        ];
        assert_eq!(pinned.len(), SensitivityLabel::ALL.len());
        for (label, restriction) in pinned {
            assert_eq!(SharingRestriction::of(label), restriction);
        }
        let mut produced: Vec<SharingRestriction> = SensitivityLabel::ALL
            .into_iter()
            .map(SharingRestriction::of)
            .collect();
        produced.sort_unstable();
        produced.dedup();
        assert_eq!(produced.len(), SharingRestriction::ALL.len());
    }

    #[test]
    fn every_confidentiality_maps_to_its_own_label() {
        let pinned = [
            (Confidentiality::Public, SensitivityLabel::Public),
            (Confidentiality::Personal, SensitivityLabel::Personal),
            (Confidentiality::Restricted, SensitivityLabel::Restricted),
            (Confidentiality::Secret, SensitivityLabel::Secret),
        ];
        assert_eq!(pinned.len(), SensitivityLabel::ALL.len());
        for (confidentiality, label) in pinned {
            assert_eq!(SensitivityLabel::of(confidentiality), label);
        }
    }

    #[test]
    fn the_strongest_label_of_a_mixture_is_the_most_exposing_one() {
        assert_eq!(
            SensitivityLabel::strongest_of([
                SensitivityLabel::Public,
                SensitivityLabel::Secret,
                SensitivityLabel::Personal,
            ]),
            SensitivityLabel::Secret
        );
        assert_eq!(
            SensitivityLabel::strongest_of([]),
            SensitivityLabel::Public,
            "an empty mixture is the label of a constant this repository publishes"
        );
        for label in SensitivityLabel::ALL {
            assert_eq!(SensitivityLabel::strongest_of([label]), label);
        }
    }

    #[test]
    fn a_notice_may_not_be_empty_or_span_lines() {
        assert!(CopyrightNotice::new("").is_err());
        assert!(CopyrightNotice::new("   ").is_err());
        assert!(CopyrightNotice::new("first\nsecond").is_err());
        assert!(CopyrightNotice::new("a university, teaching use only").is_ok());
    }

    #[test]
    fn a_register_refuses_a_domain_it_does_not_name() -> Result<(), Box<dyn std::error::Error>> {
        let register = TermsRegister::new(CopyrightNotice::new("bundle notice")?).with_domain(
            "known",
            DomainTerms::new(
                SensitivityLabel::Restricted,
                CopyrightNotice::new("domain notice")?,
            ),
        );
        let terms = register.for_domain("known")?;
        assert_eq!(terms.sensitivity(), SensitivityLabel::Restricted);
        assert_eq!(
            terms.sharing_restriction(),
            SharingRestriction::NoRedistributionWithoutSourcePermission
        );
        assert!(register.for_domain("unknown").is_err());
        assert_eq!(register.domains().collect::<Vec<_>>(), vec!["known"]);
        Ok(())
    }
}
