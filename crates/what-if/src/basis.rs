//! Section 22.1's `basedOn` block: the four facts a plan is frozen against.
//!
//! ```yaml
//! basedOn:
//!   studentRecordSnapshot: ...
//!   requirementSetHash: ...
//!   offeringCatalogSnapshot: ...
//!   knowledgeStateAsOf: ...
//! ```
//!
//! Every one of the four is a *reference to a fact*, never the fact itself. The
//! record, the rule set, the catalogue and the knowledge state all live behind
//! the canonical writer; what a plan holds is the digest or the instant that
//! says which version of each it was computed against. That is what makes a
//! plan reproducible without a plan holding anything it could write back.
//!
//! # The four are a closed set, and it is a measurement
//!
//! [`BASIS_FIELDS`] is compared against section 22.1's own `basedOn:` block in
//! both directions by `scenario_basis_round_trip`. Each field carries the
//! document's own key in [`BasisField::spec_key`] and this crate's wire spelling
//! in [`BasisField::wire_key`], and the round trip requires the serialised
//! object's key set to be exactly the wire spellings — so a field added to the
//! struct without the document, or dropped while the document still names it,
//! fails at both ends rather than at neither.
//!
//! # The digest covers all four, keyed, and it is pinned
//!
//! [`ScenarioBasis::digest`] hashes the field key beside every value. The key
//! is **not** what separates a swapped pair of same-shaped fields — the field
//! order does that, and an injection that removed the key from every field left
//! every behavioural assertion in the suite unchanged. What the key buys is
//! that the digest is bound to the field *names*, so a later schema whose
//! values happen to line up does not collide with this one.
//!
//! That is invisible to any comparison the engine can make against itself, so
//! `scenario_basis_round_trip` **pins the digest of a known basis** as a
//! committed value. The cost of changing the encoding is changing that value.

use academic_domain::{ContentDigest, TimestampMillis};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Domain separator for the basis digest.
const BASIS_DIGEST_DOMAIN: &str = "academic-what-if/scenario-basis/v1";

/// One entry of section 22.1's `basedOn` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BasisField {
    /// `studentRecordSnapshot`.
    StudentRecordSnapshot,
    /// `requirementSetHash`.
    RequirementSetHash,
    /// `offeringCatalogSnapshot`.
    OfferingCatalogSnapshot,
    /// `knowledgeStateAsOf`.
    KnowledgeStateAsOf,
}

/// The four, in section 22.1's own order.
pub const BASIS_FIELDS: [BasisField; 4] = [
    BasisField::StudentRecordSnapshot,
    BasisField::RequirementSetHash,
    BasisField::OfferingCatalogSnapshot,
    BasisField::KnowledgeStateAsOf,
];

impl BasisField {
    /// The key section 22.1's YAML block writes, verbatim.
    #[must_use]
    pub const fn spec_key(self) -> &'static str {
        match self {
            Self::StudentRecordSnapshot => "studentRecordSnapshot",
            Self::RequirementSetHash => "requirementSetHash",
            Self::OfferingCatalogSnapshot => "offeringCatalogSnapshot",
            Self::KnowledgeStateAsOf => "knowledgeStateAsOf",
        }
    }

    /// This crate's wire spelling of the same field.
    ///
    /// The workspace serialises in `snake_case`, and section 22.1 writes the
    /// block in `camelCase`. The mapping is a total `match` rather than a
    /// transformation so that a renamed field has to be renamed in both places.
    #[must_use]
    pub const fn wire_key(self) -> &'static str {
        match self {
            Self::StudentRecordSnapshot => "student_record_snapshot",
            Self::RequirementSetHash => "requirement_set_hash",
            Self::OfferingCatalogSnapshot => "offering_catalog_snapshot",
            Self::KnowledgeStateAsOf => "knowledge_state_as_of",
        }
    }
}

/// The frozen facts one plan was computed against.
///
/// Private fields, one constructor taking all four by value, and no `Default`.
/// A plan whose basis was assembled field by field could be a plan whose
/// requirement set was never recorded, and an audit that read it would be
/// answering a graduation question against a rule set nobody chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ScenarioBasis {
    student_record_snapshot: ContentDigest,
    requirement_set_hash: ContentDigest,
    offering_catalog_snapshot: ContentDigest,
    knowledge_state_as_of: TimestampMillis,
}

impl ScenarioBasis {
    /// Freezes one plan's four references.
    #[must_use]
    pub const fn of(
        student_record_snapshot: ContentDigest,
        requirement_set_hash: ContentDigest,
        offering_catalog_snapshot: ContentDigest,
        knowledge_state_as_of: TimestampMillis,
    ) -> Self {
        Self {
            student_record_snapshot,
            requirement_set_hash,
            offering_catalog_snapshot,
            knowledge_state_as_of,
        }
    }

    /// The record snapshot the plan was computed against.
    #[must_use]
    pub const fn student_record_snapshot(&self) -> ContentDigest {
        self.student_record_snapshot
    }

    /// The requirement set the plan was computed against.
    #[must_use]
    pub const fn requirement_set_hash(&self) -> ContentDigest {
        self.requirement_set_hash
    }

    /// The offering catalogue snapshot the plan chose from.
    #[must_use]
    pub const fn offering_catalog_snapshot(&self) -> ContentDigest {
        self.offering_catalog_snapshot
    }

    /// The knowledge-state cut the plan was computed against.
    #[must_use]
    pub const fn knowledge_state_as_of(&self) -> TimestampMillis {
        self.knowledge_state_as_of
    }

    /// Digests the basis, keying every field by its own name.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut hasher = Sha256::new();
        let mut field = |bytes: &[u8]| {
            hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
            hasher.update(bytes);
        };
        field(BASIS_DIGEST_DOMAIN.as_bytes());
        for entry in BASIS_FIELDS {
            field(entry.wire_key().as_bytes());
            match entry {
                BasisField::StudentRecordSnapshot => field(self.student_record_snapshot.as_bytes()),
                BasisField::RequirementSetHash => field(self.requirement_set_hash.as_bytes()),
                BasisField::OfferingCatalogSnapshot => {
                    field(self.offering_catalog_snapshot.as_bytes());
                }
                BasisField::KnowledgeStateAsOf => {
                    field(&self.knowledge_state_as_of.value().to_be_bytes());
                }
            }
        }
        ContentDigest::from_sha256_bytes(hasher.finalize().into())
    }
}
