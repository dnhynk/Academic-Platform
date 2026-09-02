//! The canonical transcript record: what every import format normalizes to.
//!
//! Two things are separated here and never merged again.
//!
//! - The **identity header** — student number, student name, institution,
//!   issue date. This is what a redaction projection removes.
//! - The **rows** — course code, term, credits, grade. These are the four
//!   fields section 29.3 reconciles, and no redaction touches them.
//!
//! The canonical encoding below is the only byte form this crate hashes,
//! compares, or exports. It is length-prefixed rather than delimited, so no
//! field value can spell a separator and change the parse of its neighbour.

use std::fmt;

use academic_domain::Decimal;
use sha2::{Digest as _, Sha256};

use crate::TranscriptError;

/// Canonical-encoding version. Any change to the encoding changes this label,
/// so a checksum taken under one version cannot silently reconcile against
/// another.
pub const CANONICAL_ENCODING_LABEL: &[u8] = b"ACADEMIC-TRANSCRIPT-CANONICAL-V1";

/// Longest accepted value for any single canonical field.
///
/// The limit exists so a malformed import cannot make the encoder allocate
/// without bound; it is far above any real transcript field.
pub const MAX_FIELD_BYTES: usize = 512;

/// Largest accepted row count for one transcript.
pub const MAX_ROWS: usize = 4096;

/// The four fields section 29.3 names for reconciliation.
///
/// This enum is closed on purpose. "Checksum across course code, term,
/// credits, and grade" is the contract; adding a fifth arm would silently
/// widen every reconciliation in the crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TranscriptField {
    /// Official course code, for example `M1522.000900`.
    CourseCode,
    /// Term the attempt belongs to, for example `2024-1`.
    Term,
    /// Credit value carried as an exact base-10 decimal.
    Credits,
    /// Recorded grade symbol, for example `A+` or `S`.
    Grade,
}

impl TranscriptField {
    /// Every reconciled field, in canonical order.
    pub const ALL: [Self; 4] = [Self::CourseCode, Self::Term, Self::Credits, Self::Grade];

    /// Returns the stable wire name used in reports and canonical bytes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CourseCode => "COURSE_CODE",
            Self::Term => "TERM",
            Self::Credits => "CREDITS",
            Self::Grade => "GRADE",
        }
    }
}

impl fmt::Display for TranscriptField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The identity fields a redaction projection may remove.
///
/// Closed for the same reason [`TranscriptField`] is: "student number and name
/// can be removed independently" is a statement about exactly two removable
/// fields, and the four-combination matrix in the acceptance suite is
/// exhaustive only because this list is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdentityField {
    /// The student number printed on the official transcript.
    StudentNumber,
    /// The student's name printed on the official transcript.
    StudentName,
}

impl IdentityField {
    /// Every removable identity field, in canonical order.
    pub const ALL: [Self; 2] = [Self::StudentNumber, Self::StudentName];

    /// Returns the stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StudentNumber => "STUDENT_NUMBER",
            Self::StudentName => "STUDENT_NAME",
        }
    }
}

impl fmt::Display for IdentityField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The identity header of one official transcript.
///
/// `Debug` is hand-written and prints no value. This type holds the exact
/// student number and name that the whole redaction contract exists to keep off
/// a shared screen; a derived `Debug` would put both into any log line, panic
/// message, or audit row that formatted an enclosing value. That is the same
/// exposure ADR-005 forbids for key material, one step outside the export path
/// this task's acceptance rows watch, and
/// `tools/secret-debug-policy.test.mjs` is what enforces it.
#[derive(Clone, PartialEq, Eq)]
pub struct TranscriptIdentity {
    student_number: String,
    student_name: String,
    institution: String,
    issued_on: String,
}

impl TranscriptIdentity {
    /// Builds a validated identity header.
    pub fn new(
        student_number: impl Into<String>,
        student_name: impl Into<String>,
        institution: impl Into<String>,
        issued_on: impl Into<String>,
    ) -> Result<Self, TranscriptError> {
        let identity = Self {
            student_number: student_number.into(),
            student_name: student_name.into(),
            institution: institution.into(),
            issued_on: issued_on.into(),
        };
        check_field("student number", &identity.student_number)?;
        check_field("student name", &identity.student_name)?;
        check_field("institution", &identity.institution)?;
        check_field("issue date", &identity.issued_on)?;
        Ok(identity)
    }

    /// Returns the student number.
    #[must_use]
    pub fn student_number(&self) -> &str {
        &self.student_number
    }

    /// Returns the student name.
    #[must_use]
    pub fn student_name(&self) -> &str {
        &self.student_name
    }

    /// Returns the issuing institution.
    #[must_use]
    pub fn institution(&self) -> &str {
        &self.institution
    }

    /// Returns the issue date exactly as the official document spells it.
    #[must_use]
    pub fn issued_on(&self) -> &str {
        &self.issued_on
    }

    /// Returns the value of one removable identity field.
    #[must_use]
    pub fn field(&self, field: IdentityField) -> &str {
        match field {
            IdentityField::StudentNumber => &self.student_number,
            IdentityField::StudentName => &self.student_name,
        }
    }
}

impl fmt::Debug for TranscriptIdentity {
    /// Prints field lengths, never field values.
    ///
    /// A length is what a caller needs in a diagnostic; the value is the thing
    /// this crate exists to withhold.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptIdentity")
            .field("student_number_len", &self.student_number.len())
            .field("student_name_len", &self.student_name.len())
            .field("institution_len", &self.institution.len())
            .field("issued_on_len", &self.issued_on.len())
            .finish_non_exhaustive()
    }
}

/// One transcript row: the four reconciled fields under an ordinal.
///
/// The ordinal is the row's position in the official document, assigned by the
/// normalizer and never by a sort. Reconciliation reports a mismatch by
/// ordinal, so two importers that ordered rows differently would localize the
/// same mismatch to different places; keeping document order is what makes the
/// report mean one line of one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptRow {
    ordinal: u32,
    course_code: String,
    term: String,
    credits: Decimal,
    grade: String,
}

impl TranscriptRow {
    /// Builds a validated row.
    pub fn new(
        ordinal: u32,
        course_code: impl Into<String>,
        term: impl Into<String>,
        credits: Decimal,
        grade: impl Into<String>,
    ) -> Result<Self, TranscriptError> {
        let row = Self {
            ordinal,
            course_code: course_code.into(),
            term: term.into(),
            credits,
            grade: grade.into(),
        };
        check_field("course code", &row.course_code)?;
        check_field("term", &row.term)?;
        check_field("grade", &row.grade)?;
        Ok(row)
    }

    /// Returns the row's position in the official document.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the official course code.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// Returns the term.
    #[must_use]
    pub fn term(&self) -> &str {
        &self.term
    }

    /// Returns the exact decimal credit value.
    #[must_use]
    pub const fn credits(&self) -> Decimal {
        self.credits
    }

    /// Returns the recorded grade symbol.
    #[must_use]
    pub fn grade(&self) -> &str {
        &self.grade
    }

    /// Returns one reconciled field's canonical text.
    #[must_use]
    pub fn field(&self, field: TranscriptField) -> String {
        match field {
            TranscriptField::CourseCode => self.course_code.clone(),
            TranscriptField::Term => self.term.clone(),
            TranscriptField::Credits => canonical_decimal(self.credits),
            TranscriptField::Grade => self.grade.clone(),
        }
    }
}

/// A complete normalized transcript.
///
/// Every import format produces this and nothing else. It owns no byte of the
/// original document: the original stays sealed in the vault, and the only way
/// back to it is through the vault handle.
///
/// `Debug` is hand-written for the reason [`TranscriptIdentity`]'s is.
#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedTranscript {
    identity: TranscriptIdentity,
    rows: Vec<TranscriptRow>,
}

impl NormalizedTranscript {
    /// Builds a transcript whose rows are contiguous from zero in document order.
    pub fn new(
        identity: TranscriptIdentity,
        rows: Vec<TranscriptRow>,
    ) -> Result<Self, TranscriptError> {
        if rows.len() > MAX_ROWS {
            return Err(TranscriptError::TooManyRows {
                actual: rows.len(),
                maximum: MAX_ROWS,
            });
        }
        for (index, row) in rows.iter().enumerate() {
            let expected = u32::try_from(index).unwrap_or(u32::MAX);
            if row.ordinal != expected {
                return Err(TranscriptError::NonContiguousOrdinal {
                    position: expected,
                    ordinal: row.ordinal,
                });
            }
        }
        Ok(Self { identity, rows })
    }

    /// Returns the identity header.
    #[must_use]
    pub const fn identity(&self) -> &TranscriptIdentity {
        &self.identity
    }

    /// Returns the rows in document order.
    #[must_use]
    pub fn rows(&self) -> &[TranscriptRow] {
        &self.rows
    }

    /// Returns the canonical encoding of the whole record.
    ///
    /// This is the byte form `transcript_formats_normalize_equivalently`
    /// compares. Two imports are equivalent when these bytes are equal — not
    /// when a field-by-field walk happens to agree, which would let an encoding
    /// difference the checksum can still see pass as equality.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(CANONICAL_ENCODING_LABEL);
        push_field(&mut out, self.identity.student_number.as_bytes());
        push_field(&mut out, self.identity.student_name.as_bytes());
        push_field(&mut out, self.identity.institution.as_bytes());
        push_field(&mut out, self.identity.issued_on.as_bytes());
        push_u32(&mut out, u32::try_from(self.rows.len()).unwrap_or(u32::MAX));
        for row in &self.rows {
            push_u32(&mut out, row.ordinal);
            for field in TranscriptField::ALL {
                push_field(&mut out, row.field(field).as_bytes());
            }
        }
        out
    }

    /// Returns SHA-256 over [`Self::canonical_bytes`].
    #[must_use]
    pub fn canonical_digest(&self) -> [u8; 32] {
        Sha256::digest(self.canonical_bytes()).into()
    }
}

impl fmt::Debug for NormalizedTranscript {
    /// Prints the row count and nothing else.
    ///
    /// Not even the identity's own redacting `Debug`: `tools/secret-debug-policy.test.mjs`
    /// treats a field whose type carries plaintext as a field that must be
    /// reduced, and reaching one through a second formatter is how a redaction
    /// stops being one edit away from a leak.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NormalizedTranscript")
            .field("row_count", &self.rows.len())
            .finish_non_exhaustive()
    }
}

/// Renders a decimal in the one spelling this crate hashes.
///
/// `academic_domain::Decimal` is a coefficient and a scale, so `30` at scale 1
/// and `3` at scale 0 are the same quantity spelled two ways. A checksum over
/// the raw spelling would call those a field mismatch — a CSV that writes
/// `3.0` and a manual entry that writes `3` are the same credit value — so the
/// encoder normalizes to fixed point with trailing zeros removed.
#[must_use]
pub fn canonical_decimal(value: Decimal) -> String {
    let negative = value.coefficient() < 0;
    let mut digits = value.coefficient().unsigned_abs().to_string();
    let scale = usize::from(value.scale());
    if scale > 0 {
        while digits.len() <= scale {
            digits.insert(0, '0');
        }
        digits.insert(digits.len() - scale, '.');
        while digits.ends_with('0') {
            digits.pop();
        }
        if digits.ends_with('.') {
            digits.pop();
        }
    }
    if digits.is_empty() {
        digits.push('0');
    }
    if negative && digits != "0" {
        digits.insert(0, '-');
    }
    digits
}

/// Parses a fixed-point credit value into the canonical exact decimal.
pub fn parse_decimal(value: &str) -> Result<Decimal, TranscriptError> {
    const FIELD: &str = "credits";
    let trimmed = value.trim();
    let (sign, body) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1_i128, rest),
        None => (1_i128, trimmed),
    };
    let (integral, fractional) = match body.split_once('.') {
        Some((left, right)) => (left, right),
        None => (body, ""),
    };
    if integral.is_empty() && fractional.is_empty() {
        return Err(TranscriptError::MalformedField {
            field: FIELD,
            reason: "no digits",
        });
    }
    if !integral
        .bytes()
        .chain(fractional.bytes())
        .all(|byte| byte.is_ascii_digit())
    {
        return Err(TranscriptError::MalformedField {
            field: FIELD,
            reason: "non-digit",
        });
    }
    let scale = u8::try_from(fractional.len()).map_err(|_| TranscriptError::MalformedField {
        field: FIELD,
        reason: "scale out of range",
    })?;
    let coefficient = format!("{integral}{fractional}")
        .parse::<i128>()
        .map_err(|_| TranscriptError::MalformedField {
            field: FIELD,
            reason: "coefficient out of range",
        })?;
    Decimal::new(sign * coefficient, scale).map_err(|_| TranscriptError::MalformedField {
        field: FIELD,
        reason: "scale out of range",
    })
}

fn check_field(name: &'static str, value: &str) -> Result<(), TranscriptError> {
    if value.trim().is_empty() {
        return Err(TranscriptError::MalformedField {
            field: name,
            reason: "empty",
        });
    }
    if value.len() > MAX_FIELD_BYTES {
        return Err(TranscriptError::MalformedField {
            field: name,
            reason: "longer than the canonical field limit",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(TranscriptError::MalformedField {
            field: name,
            reason: "control character",
        });
    }
    Ok(())
}

fn push_field(out: &mut Vec<u8>, bytes: &[u8]) {
    push_u32(out, u32::try_from(bytes.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(bytes);
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
