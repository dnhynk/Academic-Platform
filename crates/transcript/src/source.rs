//! The three import formats of section 29.3, and the deterministic parse of each.
//!
//! # What a "PDF import" is here, exactly
//!
//! This repository has no PDF library and no OCR engine, and adding either
//! would go through the dependency admission process rather than through this
//! task. So the boundary is stated rather than implied:
//!
//! - [`TranscriptFormat::PdfTextLayer`] parses the **uncompressed text layer**
//!   of a PDF: it walks `BT`/`ET` blocks and reads the literal strings passed
//!   to `Tj`. It handles no filter, no font encoding, and no compressed object
//!   stream. It is exactly enough to read the corpus
//!   [`build_synthetic_transcript_pdf`] emits, and a real official transcript
//!   PDF needs its own declared layout before this parser may be pointed at it.
//! - [`TranscriptFormat::PdfOcr`] runs **no optical character recognition**. It
//!   names the provenance of rows whose values came from a model rather than
//!   from a deterministic read, which is what changes the resulting claim's
//!   actor and epistemic status. The values themselves are supplied by the
//!   caller.
//!
//! Everything downstream — normalization, the two-claim split, reconciliation,
//! redaction — is independent of which of these produced the rows, and that
//! independence is what `transcript_formats_normalize_equivalently` measures.

use crate::{
    TranscriptError,
    record::{
        MAX_ROWS, NormalizedTranscript, TranscriptIdentity, TranscriptRow, canonical_decimal,
        parse_decimal,
    },
};

/// Which import route produced a set of rows.
///
/// This is provenance, not a parser selector: [`Self::PdfOcr`] and
/// [`Self::PdfTextLayer`] read the same bytes when both are used, and what
/// differs is the actor and epistemic status the resulting import claim may
/// carry. See [`crate::claims`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TranscriptFormat {
    /// Deterministic read of an uncompressed PDF text layer.
    PdfTextLayer,
    /// Values a model read off a PDF. No OCR engine exists in this repository.
    PdfOcr,
    /// Deterministic read of the official CSV export.
    Csv,
    /// Values the user typed in by hand.
    ManualEntry,
}

impl TranscriptFormat {
    /// Every import format, in canonical order.
    pub const ALL: [Self; 4] = [
        Self::PdfTextLayer,
        Self::PdfOcr,
        Self::Csv,
        Self::ManualEntry,
    ];

    /// Returns the stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PdfTextLayer => "PDF_TEXT_LAYER",
            Self::PdfOcr => "PDF_OCR",
            Self::Csv => "CSV",
            Self::ManualEntry => "MANUAL_ENTRY",
        }
    }

    /// Whether a model, rather than a deterministic parser, produced the values.
    #[must_use]
    pub const fn is_model_read(self) -> bool {
        matches!(self, Self::PdfOcr)
    }
}

/// The labelled line grammar every deterministic parser in this module targets.
///
/// A transcript is a sequence of `KEY\tVALUE...` lines. The grammar is declared
/// rather than sniffed: an unknown key, a missing key, a duplicate key, or a
/// row with the wrong field count is refused, so a partially-read document can
/// never become a partially-populated transcript.
mod grammar {
    pub const STUDENT_NUMBER: &str = "STUDENT_NUMBER";
    pub const STUDENT_NAME: &str = "STUDENT_NAME";
    pub const INSTITUTION: &str = "INSTITUTION";
    pub const ISSUED_ON: &str = "ISSUED_ON";
    pub const ROW: &str = "ROW";
}

/// Header line of the official CSV export, exactly.
pub const CSV_ROW_HEADER: &str = "course_code,term,credits,grade";

/// One row as the user typed it, before any validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualRowEntry {
    /// Official course code.
    pub course_code: String,
    /// Term the attempt belongs to.
    pub term: String,
    /// Credit value as typed, in fixed-point spelling.
    pub credits: String,
    /// Recorded grade symbol.
    pub grade: String,
}

/// Parses the uncompressed text layer of a transcript PDF.
pub fn parse_pdf_text_layer(bytes: &[u8]) -> Result<NormalizedTranscript, TranscriptError> {
    parse_labelled_lines(&extract_pdf_text(bytes)?)
}

/// Parses the official CSV export.
pub fn parse_csv(bytes: &[u8]) -> Result<NormalizedTranscript, TranscriptError> {
    let text = std::str::from_utf8(bytes).map_err(|_| TranscriptError::MalformedSource {
        format: TranscriptFormat::Csv,
        reason: "not valid UTF-8",
    })?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let mut student_number = None;
    let mut student_name = None;
    let mut institution = None;
    let mut issued_on = None;
    let mut saw_header = false;
    let mut rows = Vec::new();
    for line in &mut lines {
        let (key, value) = line.split_once(',').unwrap_or((line, ""));
        match key {
            grammar::STUDENT_NUMBER => set_once(&mut student_number, value, "student number")?,
            grammar::STUDENT_NAME => set_once(&mut student_name, value, "student name")?,
            grammar::INSTITUTION => set_once(&mut institution, value, "institution")?,
            grammar::ISSUED_ON => set_once(&mut issued_on, value, "issue date")?,
            _ if line == CSV_ROW_HEADER => {
                saw_header = true;
                break;
            }
            _ => {
                return Err(TranscriptError::MalformedSource {
                    format: TranscriptFormat::Csv,
                    reason: "unknown header key before the row header",
                });
            }
        }
    }
    if !saw_header {
        return Err(TranscriptError::MalformedSource {
            format: TranscriptFormat::Csv,
            reason: "missing the declared row header",
        });
    }
    for line in lines {
        let fields: Vec<&str> = line.split(',').collect();
        let [course_code, term, credits, grade] = fields.as_slice() else {
            return Err(TranscriptError::MalformedSource {
                format: TranscriptFormat::Csv,
                reason: "row does not carry exactly four fields",
            });
        };
        rows.push(push_row(rows.len(), course_code, term, credits, grade)?);
    }
    finish(student_number, student_name, institution, issued_on, rows)
}

/// Builds a transcript from values the user typed by hand.
pub fn parse_manual_entry(
    student_number: &str,
    student_name: &str,
    institution: &str,
    issued_on: &str,
    entries: &[ManualRowEntry],
) -> Result<NormalizedTranscript, TranscriptError> {
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        rows.push(push_row(
            rows.len(),
            &entry.course_code,
            &entry.term,
            &entry.credits,
            &entry.grade,
        )?);
    }
    finish(
        Some(student_number.to_owned()),
        Some(student_name.to_owned()),
        Some(institution.to_owned()),
        Some(issued_on.to_owned()),
        rows,
    )
}

/// Renders the official CSV export a normalized transcript would parse back to.
///
/// This exists so the corpus builder has one place that spells the CSV, and so
/// a round trip is executable rather than asserted.
#[must_use]
pub fn render_csv(transcript: &NormalizedTranscript) -> String {
    use crate::record::TranscriptField;

    let identity = transcript.identity();
    let mut out = String::new();
    out.push_str(&format!(
        "{},{}\n",
        grammar::STUDENT_NUMBER,
        identity.student_number()
    ));
    out.push_str(&format!(
        "{},{}\n",
        grammar::STUDENT_NAME,
        identity.student_name()
    ));
    out.push_str(&format!(
        "{},{}\n",
        grammar::INSTITUTION,
        identity.institution()
    ));
    out.push_str(&format!(
        "{},{}\n",
        grammar::ISSUED_ON,
        identity.issued_on()
    ));
    out.push_str(CSV_ROW_HEADER);
    out.push('\n');
    for row in transcript.rows() {
        out.push_str(&format!(
            "{},{},{},{}\n",
            row.field(TranscriptField::CourseCode),
            row.field(TranscriptField::Term),
            row.field(TranscriptField::Credits),
            row.field(TranscriptField::Grade),
        ));
    }
    out
}

/// Renders the manual-entry rows a normalized transcript would parse back to.
#[must_use]
pub fn render_manual_entries(transcript: &NormalizedTranscript) -> Vec<ManualRowEntry> {
    transcript
        .rows()
        .iter()
        .map(|row| ManualRowEntry {
            course_code: row.course_code().to_owned(),
            term: row.term().to_owned(),
            credits: canonical_decimal(row.credits()),
            grade: row.grade().to_owned(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The synthetic corpus
// ---------------------------------------------------------------------------

/// Strings that exist only in an original transcript document.
///
/// Every one is metadata or container structure. None is transcript content, so
/// none of them may appear in a redacted export — which is what
/// `redacted_export_contains_no_original_bytes_or_metadata` reads this list
/// for. The corpus builder below writes all of them, so the scan has something
/// to find whenever a leak is injected.
pub const ORIGINAL_ONLY_MARKERS: &[&str] = &[
    "%PDF-",
    "endobj",
    "stream",
    "/Producer",
    "/Creator",
    "/Author",
    "/Keywords",
    "/CreationDate",
    "SNU-Transcript-Composer",
    "Academic-Platform-Synthetic-Corpus",
    "<?xpacket",
    "http://ns.adobe.com/xap/1.0/",
    "xmp:CreatorTool",
    "pdf:Producer",
    "dc:creator",
    "Exif\u{0}\u{0}",
    "EXIF-Software-Marker",
];

/// The generator string the corpus writes into `/Producer` and `pdf:Producer`.
pub const CORPUS_PRODUCER: &str = "SNU-Transcript-Composer 4.2";
/// The generator string the corpus writes into `/Creator` and `xmp:CreatorTool`.
pub const CORPUS_CREATOR_TOOL: &str = "Academic-Platform-Synthetic-Corpus 1.0";
/// The software string the corpus writes into the EXIF-shaped APP1 segment.
pub const CORPUS_EXIF_SOFTWARE: &str = "EXIF-Software-Marker 9.9";

/// A synthetic transcript PDF and the byte ranges of its transcript content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticTranscriptPdf {
    /// The complete document bytes.
    pub bytes: Vec<u8>,
}

/// Builds the deterministic synthetic transcript PDF used by every acceptance row.
///
/// The document is a real PDF container — header, object table, a content
/// stream, a trailer — carrying three separate metadata surfaces that a
/// redacted export must not reproduce:
///
/// 1. a document information dictionary with `/Producer`, `/Creator`,
///    `/Author` (the student name) and `/Keywords` (the student number);
/// 2. an XMP packet with `xmp:CreatorTool`, `pdf:Producer` and `dc:creator`;
/// 3. an **EXIF-shaped APP1 segment** inside an embedded image stream. It is
///    not a decodable JPEG and this crate never decodes it; what it buys is
///    that the metadata scan has an EXIF vocabulary to find.
///
/// The bytes are a pure function of the transcript, so the corpus is
/// reproducible from the deterministic builder and from nothing else.
#[must_use]
pub fn build_synthetic_transcript_pdf(transcript: &NormalizedTranscript) -> SyntheticTranscriptPdf {
    use crate::record::TranscriptField;

    let identity = transcript.identity();
    let mut text = String::new();
    text.push_str(&text_line(
        grammar::STUDENT_NUMBER,
        &[identity.student_number()],
    ));
    text.push_str(&text_line(
        grammar::STUDENT_NAME,
        &[identity.student_name()],
    ));
    text.push_str(&text_line(grammar::INSTITUTION, &[identity.institution()]));
    text.push_str(&text_line(grammar::ISSUED_ON, &[identity.issued_on()]));
    for row in transcript.rows() {
        text.push_str(&text_line(
            grammar::ROW,
            &[
                &row.field(TranscriptField::CourseCode),
                &row.field(TranscriptField::Term),
                &row.field(TranscriptField::Credits),
                &row.field(TranscriptField::Grade),
            ],
        ));
    }

    let xmp = format!(
        "<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"\n\
         xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n\
         xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"\n\
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n\
         <rdf:Description rdf:about=\"\">\n\
         <xmp:CreatorTool>{CORPUS_CREATOR_TOOL}</xmp:CreatorTool>\n\
         <pdf:Producer>{CORPUS_PRODUCER}</pdf:Producer>\n\
         <dc:creator>{}</dc:creator>\n\
         </rdf:Description>\n\
         </rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end=\"w\"?>",
        identity.student_name()
    );

    // An EXIF-shaped APP1 segment: SOI, APP1 marker, the `Exif\0\0` header,
    // and two ASCII strings. Deliberately not a decodable JPEG.
    let mut exif = Vec::new();
    exif.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE1]);
    exif.extend_from_slice(b"Exif\0\0MM\0*");
    exif.extend_from_slice(CORPUS_EXIF_SOFTWARE.as_bytes());
    exif.push(0);
    exif.extend_from_slice(identity.student_number().as_bytes());
    exif.push(0);

    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
    push_object(
        &mut out,
        1,
        b"<< /Type /Catalog /Pages 2 0 R /Metadata 6 0 R >>",
    );
    push_object(&mut out, 2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    push_object(
        &mut out,
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R >>",
    );
    push_stream_object(&mut out, 4, b"<< /Length %LEN% >>", text.as_bytes());
    push_object(
        &mut out,
        5,
        format!(
            "<< /Producer ({CORPUS_PRODUCER}) /Creator ({CORPUS_CREATOR_TOOL}) \
             /Author ({}) /Keywords ({}) /CreationDate (D:20240301000000Z) >>",
            identity.student_name(),
            identity.student_number()
        )
        .as_bytes(),
    );
    push_stream_object(
        &mut out,
        6,
        b"<< /Type /Metadata /Subtype /XML /Length %LEN% >>",
        xmp.as_bytes(),
    );
    push_stream_object(
        &mut out,
        7,
        b"<< /Type /XObject /Subtype /Image /Filter /DCTDecode /Length %LEN% >>",
        &exif,
    );
    out.extend_from_slice(b"trailer\n<< /Size 8 /Root 1 0 R /Info 5 0 R >>\n%%EOF\n");
    SyntheticTranscriptPdf { bytes: out }
}

fn push_object(out: &mut Vec<u8>, number: u32, body: &[u8]) {
    out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(b"\nendobj\n");
}

fn push_stream_object(out: &mut Vec<u8>, number: u32, dictionary: &[u8], payload: &[u8]) {
    let rendered = String::from_utf8_lossy(dictionary)
        .replace("%LEN%", &payload.len().to_string())
        .into_bytes();
    out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
    out.extend_from_slice(&rendered);
    out.extend_from_slice(b"\nstream\n");
    out.extend_from_slice(payload);
    out.extend_from_slice(b"\nendstream\nendobj\n");
}

/// Renders one labelled line as a `BT`/`Tj`/`ET` block.
fn text_line(key: &str, values: &[&str]) -> String {
    let mut joined = String::from(key);
    for value in values {
        joined.push('\t');
        joined.push_str(value);
    }
    format!("BT /F1 11 Tf 72 700 Td ({}) Tj ET\n", escape_pdf(&joined))
}

fn escape_pdf(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Deterministic parsing
// ---------------------------------------------------------------------------

/// Walks `BT`/`ET` blocks and returns the literal strings passed to `Tj`.
fn extract_pdf_text(bytes: &[u8]) -> Result<String, TranscriptError> {
    let malformed = |reason: &'static str| TranscriptError::MalformedSource {
        format: TranscriptFormat::PdfTextLayer,
        reason,
    };
    if !bytes.starts_with(b"%PDF-") {
        return Err(malformed("missing the PDF header"));
    }
    let text = String::from_utf8_lossy(bytes);
    let mut lines = Vec::new();
    let mut rest = text.as_ref();
    while let Some(start) = rest.find("BT ") {
        let opened = &rest[start..];
        let Some(end) = opened.find(" ET") else {
            return Err(malformed("unterminated text block"));
        };
        let block = &opened[..end];
        let Some(open) = block.find('(') else {
            return Err(malformed("text block carries no literal string"));
        };
        let mut value = String::new();
        let mut escaped = false;
        let mut closed = false;
        for character in block[open + 1..].chars() {
            if escaped {
                match character {
                    't' => value.push('\t'),
                    other => value.push(other),
                }
                escaped = false;
                continue;
            }
            match character {
                '\\' => escaped = true,
                ')' => {
                    closed = true;
                    break;
                }
                other => value.push(other),
            }
        }
        if !closed {
            return Err(malformed("unterminated literal string"));
        }
        lines.push(value);
        rest = &opened[end..];
        if lines.len() > MAX_ROWS + 8 {
            return Err(malformed("more text blocks than the row limit permits"));
        }
    }
    if lines.is_empty() {
        return Err(malformed("no text blocks"));
    }
    Ok(lines.join("\n"))
}

fn parse_labelled_lines(text: &str) -> Result<NormalizedTranscript, TranscriptError> {
    let malformed = |reason: &'static str| TranscriptError::MalformedSource {
        format: TranscriptFormat::PdfTextLayer,
        reason,
    };
    let mut student_number = None;
    let mut student_name = None;
    let mut institution = None;
    let mut issued_on = None;
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.split('\t');
        let Some(key) = fields.next() else {
            return Err(malformed("empty line"));
        };
        let rest: Vec<&str> = fields.collect();
        match (key, rest.as_slice()) {
            (grammar::STUDENT_NUMBER, [value]) => {
                set_once(&mut student_number, value, "student number")?;
            }
            (grammar::STUDENT_NAME, [value]) => {
                set_once(&mut student_name, value, "student name")?;
            }
            (grammar::INSTITUTION, [value]) => set_once(&mut institution, value, "institution")?,
            (grammar::ISSUED_ON, [value]) => set_once(&mut issued_on, value, "issue date")?,
            (grammar::ROW, [course_code, term, credits, grade]) => {
                rows.push(push_row(rows.len(), course_code, term, credits, grade)?);
            }
            (grammar::ROW, _) => return Err(malformed("row does not carry exactly four fields")),
            _ => return Err(malformed("unknown line key")),
        }
    }
    finish(student_number, student_name, institution, issued_on, rows)
}

fn set_once(
    slot: &mut Option<String>,
    value: &str,
    field: &'static str,
) -> Result<(), TranscriptError> {
    if slot.is_some() {
        return Err(TranscriptError::MalformedField {
            field,
            reason: "declared twice",
        });
    }
    *slot = Some(value.to_owned());
    Ok(())
}

fn push_row(
    position: usize,
    course_code: &str,
    term: &str,
    credits: &str,
    grade: &str,
) -> Result<TranscriptRow, TranscriptError> {
    let ordinal = u32::try_from(position).map_err(|_| TranscriptError::TooManyRows {
        actual: position,
        maximum: MAX_ROWS,
    })?;
    TranscriptRow::new(ordinal, course_code, term, parse_decimal(credits)?, grade)
}

fn finish(
    student_number: Option<String>,
    student_name: Option<String>,
    institution: Option<String>,
    issued_on: Option<String>,
    rows: Vec<TranscriptRow>,
) -> Result<NormalizedTranscript, TranscriptError> {
    let required = |slot: Option<String>, field: &'static str| {
        slot.ok_or(TranscriptError::MalformedField {
            field,
            reason: "absent",
        })
    };
    let identity = TranscriptIdentity::new(
        required(student_number, "student number")?,
        required(student_name, "student name")?,
        required(institution, "institution")?,
        required(issued_on, "issue date")?,
    )?;
    NormalizedTranscript::new(identity, rows)
}
