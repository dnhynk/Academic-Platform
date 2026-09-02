//! Shared synthetic corpus for the `P2-U7` acceptance suites.
//!
//! Every value here is a committed canary from
//! `testdata/transcript-canary/canaries.txt` or a small literal. Nothing in
//! this module or anything it builds is derived from a real academic record —
//! `CONTRIBUTING.md` rule 1, and this task's own absolute constraint.

#![allow(dead_code)]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use academic_domain::Decimal;
use academic_transcript::{
    record::{NormalizedTranscript, TranscriptIdentity, TranscriptRow},
    source::{ManualRowEntry, TranscriptFormat},
};

/// The committed canary corpus, verbatim.
const CANARY_FILE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/transcript-canary/canaries.txt"
));

/// Every canary token, in file order, with comment lines dropped.
#[must_use]
pub fn canaries() -> Vec<&'static str> {
    CANARY_FILE
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// Returns the canary whose token carries `marker`.
///
/// Looked up by marker rather than by index so a reordered corpus file cannot
/// silently swap the student number for the grade.
#[must_use]
pub fn canary(marker: &str) -> &'static str {
    canaries()
        .into_iter()
        .find(|line| line.contains(marker))
        .unwrap_or("MISSING-CANARY")
}

/// The synthetic official transcript every acceptance row is built from.
///
/// Three rows, so a mismatch can be localized to a middle row with reconciled
/// rows on both sides of it. Two of the three carry canary field values and the
/// third carries plausible-looking literals, so the suite reads as a transcript
/// rather than only as a canary sheet.
pub fn synthetic_transcript() -> Result<NormalizedTranscript, Box<dyn Error>> {
    let identity = TranscriptIdentity::new(
        canary("CANARY-STUDENT-NUMBER"),
        canary("CANARY-STUDENT-NAME"),
        canary("CANARY-INSTITUTION"),
        canary("CANARY-ISSUED-ON"),
    )?;
    let rows = vec![
        TranscriptRow::new(
            0,
            canary("CANARY-COURSE-CODE"),
            canary("CANARY-TERM"),
            Decimal::new(3, 0)?,
            canary("CANARY-GRADE"),
        )?,
        TranscriptRow::new(1, "M1522.000900", "2024-2", Decimal::new(30, 1)?, "A0")?,
        TranscriptRow::new(2, "L0446.000200", "2025-1", Decimal::new(15, 1)?, "S")?,
    ];
    Ok(NormalizedTranscript::new(identity, rows)?)
}

/// The manual-entry rows of [`synthetic_transcript`].
pub fn manual_entries() -> Result<Vec<ManualRowEntry>, Box<dyn Error>> {
    Ok(academic_transcript::source::render_manual_entries(
        &synthetic_transcript()?,
    ))
}

/// Every format that reads bytes rather than typed values.
pub const BYTE_FORMATS: [TranscriptFormat; 2] =
    [TranscriptFormat::PdfTextLayer, TranscriptFormat::Csv];

/// Owner of one unique, owner-only, disposable profile root.
///
/// The owner-only Unix mode is not decoration: the vault's path policy refuses
/// a profile root that a group or other bit can reach, so a root created with
/// the default mode fails on Unix while passing on Windows.
#[derive(Debug)]
pub struct TestRoot {
    path: PathBuf,
}

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

impl TestRoot {
    /// Creates the root directory.
    pub fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "academic-transcript-{label}-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { path })
    }

    /// Reserves a unique path without creating anything at it.
    pub fn reserved(label: &str) -> Result<Self, Box<dyn Error>> {
        let root = Self::new(label)?;
        fs::remove_dir_all(&root.path)?;
        Ok(root)
    }

    /// Returns the root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// One plaintext canary occurrence found in a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanaryFinding {
    /// File the canary was found in.
    pub artifact: PathBuf,
    /// Index of the canary in the committed corpus.
    pub canary_index: usize,
    /// Byte offset of the occurrence.
    pub byte_offset: u64,
}

/// Aggregate result of streaming every file below one root.
///
/// The three counts carry the same names the admission receipt's platform rows
/// use — `canary_file_count`, `canary_byte_count`, `canary_hit_count` — because
/// this is the same measurement at a different boundary, and a scan that
/// reports only "no hits" cannot be told from a scan that read nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    /// Files streamed.
    pub canary_file_count: u64,
    /// Bytes streamed.
    pub canary_byte_count: u64,
    /// Occurrences found.
    pub findings: Vec<CanaryFinding>,
}

impl ScanSummary {
    /// Number of occurrences found.
    #[must_use]
    pub fn canary_hit_count(&self) -> u64 {
        u64::try_from(self.findings.len()).unwrap_or(u64::MAX)
    }
}

/// Scans every regular file below `root` for every committed canary.
pub fn scan_for_canaries(root: &Path) -> Result<ScanSummary, Box<dyn Error>> {
    let needles = canaries();
    let mut summary = ScanSummary {
        canary_file_count: 0,
        canary_byte_count: 0,
        findings: Vec::new(),
    };
    let mut pending = vec![root.to_path_buf()];
    while let Some(current) = pending.pop() {
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&current)? {
                pending.push(entry?.path());
            }
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let bytes = fs::read(&current)?;
        summary.canary_file_count += 1;
        summary.canary_byte_count += u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        for (canary_index, needle) in needles.iter().enumerate() {
            for offset in find_all(&bytes, needle.as_bytes()) {
                summary.findings.push(CanaryFinding {
                    artifact: current.clone(),
                    canary_index,
                    byte_offset: u64::try_from(offset).unwrap_or(u64::MAX),
                });
            }
        }
    }
    Ok(summary)
}

/// Returns every offset at which `needle` occurs in `haystack`.
#[must_use]
pub fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    (0..=haystack.len() - needle.len())
        .filter(|start| &haystack[*start..*start + needle.len()] == needle)
        .collect()
}

/// Returns the error a refusal produced, or fails naming what got through.
///
/// The workspace denies `expect_used`, and a refusal assertion still has to
/// read the error it got, so the unwrap lives here once instead of at every
/// call site.
pub fn refusal<T, E>(result: Result<T, E>, what: &str) -> Result<E, Box<dyn Error>> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => Err(what.to_owned().into()),
    }
}

/// Whether `haystack` contains `needle`.
#[must_use]
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !find_all(haystack, needle).is_empty()
}
