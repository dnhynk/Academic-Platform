//! Redaction as a projection, and the export built from it.
//!
//! The invariant this module exists for: **there is no path that edits an
//! original.** [`project`] takes `&NormalizedTranscript` and returns a new
//! value; [`NormalizedTranscript`] exposes no `&mut self` method and no public
//! field, so a caller holding one cannot remove a name from it; and the sealed
//! original bytes are not reachable from a [`RedactedProjection`] at all,
//! because the projection owns nothing but the values it retains.
//!
//! That last point is what [`redacted_export`] rests on. It takes a projection
//! and nothing else, so an export cannot carry a byte or a metadata string of
//! the original document unless someone changes its signature. The acceptance
//! row scans the produced bytes for both anyway, because a structural argument
//! that is never executed is the failure mode this repository keeps hitting.
//!
//! A removed field is **absent**, not blanked. A blanked field still says how
//! long the value was and still occupies its position in a diff against an
//! unredacted export; absence says only that the profile removed it, which the
//! export declares in one line so a reader is never guessing.

use std::fmt;

use crate::record::{IdentityField, NormalizedTranscript, TranscriptField};

/// Label the export writes as its first line.
pub const REDACTED_EXPORT_LABEL: &str = "ACADEMIC-TRANSCRIPT-REDACTED-EXPORT-V1";

/// Which identity fields a projection removes.
///
/// The four combinations of the two removable fields are all constructible and
/// all meaningful, which is the whole content of
/// `student_number_and_name_can_be_removed_independently`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RedactionProfile {
    remove_student_number: bool,
    remove_student_name: bool,
}

impl RedactionProfile {
    /// Removes nothing.
    #[must_use]
    pub const fn retain_all() -> Self {
        Self {
            remove_student_number: false,
            remove_student_name: false,
        }
    }

    /// Removes exactly the listed fields.
    #[must_use]
    pub fn removing(fields: &[IdentityField]) -> Self {
        Self {
            remove_student_number: fields.contains(&IdentityField::StudentNumber),
            remove_student_name: fields.contains(&IdentityField::StudentName),
        }
    }

    /// Every profile over the two removable fields, in a stable order.
    ///
    /// Four entries, because there are two independently removable fields. A
    /// suite that enumerates this constant is exhaustive by construction, so a
    /// third removable field cannot be added without the matrix growing with
    /// it.
    #[must_use]
    pub fn all() -> [Self; 4] {
        [
            Self::retain_all(),
            Self::removing(&[IdentityField::StudentNumber]),
            Self::removing(&[IdentityField::StudentName]),
            Self::removing(&[IdentityField::StudentNumber, IdentityField::StudentName]),
        ]
    }

    /// Whether this profile removes the given field.
    #[must_use]
    pub const fn removes(self, field: IdentityField) -> bool {
        match field {
            IdentityField::StudentNumber => self.remove_student_number,
            IdentityField::StudentName => self.remove_student_name,
        }
    }

    /// The removed fields, in canonical order.
    #[must_use]
    pub fn removed_fields(self) -> Vec<IdentityField> {
        IdentityField::ALL
            .into_iter()
            .filter(|field| self.removes(*field))
            .collect()
    }
}

/// A transcript reduced to what one redaction profile permits.
///
/// It holds owned copies of retained values and nothing else. There is no
/// handle back to the transcript it was projected from and no handle to the
/// sealed original.
///
/// `Debug` is hand-written: a projection that retains the student number holds
/// it, and a derived `Debug` would print it into any log line that formatted an
/// enclosing value.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedProjection {
    profile: RedactionProfile,
    student_number: Option<String>,
    student_name: Option<String>,
    institution: String,
    issued_on: String,
    rows: Vec<[String; 4]>,
    source_digest: [u8; 32],
}

impl RedactedProjection {
    /// Returns the profile this projection was taken under.
    #[must_use]
    pub const fn profile(&self) -> RedactionProfile {
        self.profile
    }

    /// Returns the retained student number, or `None` when it was removed.
    #[must_use]
    pub fn student_number(&self) -> Option<&str> {
        self.student_number.as_deref()
    }

    /// Returns the retained student name, or `None` when it was removed.
    #[must_use]
    pub fn student_name(&self) -> Option<&str> {
        self.student_name.as_deref()
    }

    /// Returns the issuing institution, which no profile removes.
    #[must_use]
    pub fn institution(&self) -> &str {
        &self.institution
    }

    /// Returns the issue date, which no profile removes.
    #[must_use]
    pub fn issued_on(&self) -> &str {
        &self.issued_on
    }

    /// Returns the four canonical field values of every row, in document order.
    #[must_use]
    pub fn rows(&self) -> &[[String; 4]] {
        &self.rows
    }

    /// Returns the canonical digest of the transcript this was projected from.
    ///
    /// A digest, not a handle: it lets an export be tied back to its source
    /// without the export carrying anything the source contains.
    #[must_use]
    pub const fn source_digest(&self) -> &[u8; 32] {
        &self.source_digest
    }

    /// Every value this projection retains, concatenated.
    ///
    /// The acceptance row uses this as its allow-list: any byte run the export
    /// shares with the original document must lie inside one of these values,
    /// because those are exactly the values the projection deliberately kept.
    #[must_use]
    pub fn retained_values(&self) -> Vec<&str> {
        let mut values: Vec<&str> = Vec::new();
        if let Some(value) = &self.student_number {
            values.push(value);
        }
        if let Some(value) = &self.student_name {
            values.push(value);
        }
        values.push(&self.institution);
        values.push(&self.issued_on);
        for row in &self.rows {
            for field in row {
                values.push(field);
            }
        }
        values
    }
}

impl fmt::Debug for RedactedProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedactedProjection")
            .field("profile", &self.profile)
            .field("student_number_retained", &self.student_number.is_some())
            .field("student_name_retained", &self.student_name.is_some())
            .field("row_count", &self.rows.len())
            .finish_non_exhaustive()
    }
}

/// Projects a transcript under one redaction profile.
///
/// The source is borrowed and unchanged. This is the only constructor of a
/// [`RedactedProjection`].
#[must_use]
pub fn project(transcript: &NormalizedTranscript, profile: RedactionProfile) -> RedactedProjection {
    let identity = transcript.identity();
    let retain = |field: IdentityField| {
        if profile.removes(field) {
            None
        } else {
            Some(identity.field(field).to_owned())
        }
    };
    RedactedProjection {
        profile,
        student_number: retain(IdentityField::StudentNumber),
        student_name: retain(IdentityField::StudentName),
        institution: identity.institution().to_owned(),
        issued_on: identity.issued_on().to_owned(),
        rows: transcript
            .rows()
            .iter()
            .map(|row| TranscriptField::ALL.map(|field| row.field(field)))
            .collect(),
        source_digest: transcript.canonical_digest(),
    }
}

/// Renders the shareable export of a projection.
///
/// Takes a projection and nothing else. There is no overload that also takes
/// the original bytes, the sealed object, or the vault: an export that carried
/// original bytes would have to be written by changing this signature.
#[must_use]
pub fn redacted_export(projection: &RedactedProjection) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(REDACTED_EXPORT_LABEL);
    out.push('\n');
    out.push_str("REMOVED\t");
    let removed = projection.profile.removed_fields();
    if removed.is_empty() {
        out.push_str("NONE");
    } else {
        let names: Vec<&str> = removed.iter().map(|field| field.as_str()).collect();
        out.push_str(&names.join(","));
    }
    out.push('\n');
    out.push_str(&format!(
        "SOURCE_DIGEST\t{}\n",
        hex(projection.source_digest())
    ));
    if let Some(value) = projection.student_number() {
        out.push_str(&format!(
            "{}\t{value}\n",
            IdentityField::StudentNumber.as_str()
        ));
    }
    if let Some(value) = projection.student_name() {
        out.push_str(&format!(
            "{}\t{value}\n",
            IdentityField::StudentName.as_str()
        ));
    }
    out.push_str(&format!("INSTITUTION\t{}\n", projection.institution()));
    out.push_str(&format!("ISSUED_ON\t{}\n", projection.issued_on()));
    for (ordinal, row) in projection.rows().iter().enumerate() {
        out.push_str(&format!("ROW\t{ordinal}"));
        for value in row {
            out.push('\t');
            out.push_str(value);
        }
        out.push('\n');
    }
    out.into_bytes()
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('0'));
    }
    out
}
