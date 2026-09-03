//! The `TRANSCRIPT_COVERAGE` harness corpus, rendered from one builder.
//!
//! `docs/contracts/engine-harness.md` requires an `IMPLEMENTED` engine to ship
//! golden fixtures, a property bound, version-compatibility fixtures and an
//! explanation snapshot, and `CONTRIBUTING.md` rule 5 requires the committed
//! bytes to be a fresh render rather than a file somebody edited into agreement
//! with a broken engine. This module is that render.
//!
//! It is product code rather than a test so that `cargo run --example
//! emit_harness` can write the same bytes the suite compares, exactly as
//! `academic-record::harness` does for `GPA` and `CREDIT_ACCOUNTING`.
//!
//! **Nothing here reads a file.** It returns bytes; the example writes them and
//! the suite compares them.

use academic_domain::engines::{EngineError, EngineVersion, FrozenInputs, RuleSetHash};

use crate::engine::{
    RULESET_TEXT, TRANSCRIPT_COVERAGE_ENGINE_ID, TRANSCRIPT_COVERAGE_ENGINE_VERSION,
    TranscriptCoverageEngine, ruleset_hash,
};

/// The harness root every engine's directory sits under.
pub const HARNESS_ROOT: &str = "testdata/engines";

/// This engine's harness directory, which is its registry name in lower case.
pub const HARNESS_DIR: &str = "transcript_coverage";

/// The property test's generator bound.
///
/// The property is the partition: over a generated transcript of up to this
/// many segments, every eligible segment lands in exactly one of the four
/// statuses or in the unmapped list, and the two counts reconcile.
pub const PROPERTY_MAX_SEGMENTS: usize = 24;

/// One rendered file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusFile {
    /// Its repository-relative path.
    pub path: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// One golden case: a name and the frozen inputs it is evaluated over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Case {
    /// The case name, which is the `.input`/`.expected` stem.
    pub name: &'static str,
    /// How many eligible segments.
    pub eligible: u32,
    /// How many of them the document maps.
    pub mapped: u32,
    /// How many are declared non-speech.
    pub excluded: u32,
    /// How many are declared redacted.
    pub redacted: u32,
    /// How many are declared recording failures.
    pub failed: u32,
    /// How many have no status at all.
    pub unmapped: u32,
    /// Mapped tokens.
    pub mapped_tokens: u32,
    /// Tokens in the coverage denominator.
    pub countable_tokens: u32,
    /// Ordering findings.
    pub ordering_findings: u32,
    /// Ordering exceptions.
    pub ordering_exceptions: u32,
    /// Captures placed.
    pub placed_captures: u32,
    /// Captures excluded with a reason.
    pub excluded_captures: u32,
    /// Captures that are neither.
    pub unaccounted_captures: u32,
    /// Holes above the threshold.
    pub gaps: u32,
    /// Holes above the threshold that nothing explains.
    pub unexplained_gaps: u32,
    /// Render defects.
    pub render_defects: u32,
}

/// The golden cases, each one a coverage shape the validator has to keep
/// answering the same way.
pub const GOLDEN: [Case; 4] = [
    // Every segment mapped, every capture placed, nothing broken.
    Case {
        name: "whole",
        eligible: 4,
        mapped: 4,
        excluded: 0,
        redacted: 0,
        failed: 0,
        unmapped: 0,
        mapped_tokens: 20,
        countable_tokens: 20,
        ordering_findings: 0,
        ordering_exceptions: 0,
        placed_captures: 1,
        excluded_captures: 0,
        unaccounted_captures: 0,
        gaps: 0,
        unexplained_gaps: 0,
        render_defects: 0,
    },
    // One segment with no status at all. Section 12.6's `UNMAPPED` arm.
    Case {
        name: "one_unmapped",
        eligible: 4,
        mapped: 3,
        excluded: 0,
        redacted: 0,
        failed: 0,
        unmapped: 1,
        mapped_tokens: 15,
        countable_tokens: 20,
        ordering_findings: 0,
        ordering_exceptions: 0,
        placed_captures: 1,
        excluded_captures: 0,
        unaccounted_captures: 0,
        gaps: 0,
        unexplained_gaps: 0,
        render_defects: 0,
    },
    // All four statuses present at once, and the partition still reconciles.
    Case {
        name: "four_statuses",
        eligible: 4,
        mapped: 1,
        excluded: 1,
        redacted: 1,
        failed: 1,
        unmapped: 0,
        mapped_tokens: 5,
        countable_tokens: 15,
        ordering_findings: 0,
        ordering_exceptions: 1,
        placed_captures: 1,
        excluded_captures: 1,
        unaccounted_captures: 0,
        gaps: 1,
        unexplained_gaps: 0,
        render_defects: 0,
    },
    // Every dimension broken at once, which is `REQ-APA-008`'s shape.
    Case {
        name: "every_dimension_broken",
        eligible: 4,
        mapped: 2,
        excluded: 0,
        redacted: 0,
        failed: 0,
        unmapped: 2,
        mapped_tokens: 10,
        countable_tokens: 20,
        ordering_findings: 1,
        ordering_exceptions: 0,
        placed_captures: 0,
        excluded_captures: 0,
        unaccounted_captures: 1,
        gaps: 1,
        unexplained_gaps: 1,
        render_defects: 3,
    },
];

/// The version-compatibility case. Its explanation must survive every admitted
/// engine version.
pub const VERSION_COMPAT: Case = GOLDEN[0];

/// The case the committed explanation snapshot is rendered from.
pub const SNAPSHOT_CASE: Case = GOLDEN[2];

/// The canonical frozen-input text for one case.
///
/// The statuses are laid out in a fixed order — mapped, excluded, redacted,
/// failed, unmapped — and the tokens are spread evenly over the mapped
/// segments, so the encoding is a total function of the [`Case`] and two
/// renders of the same case are the same bytes.
///
/// # Errors
///
/// [`EngineError`] when a key or a reference is malformed, which is a bug in
/// this module rather than in a caller.
pub fn case_input(case: &Case) -> Result<FrozenInputs, EngineError> {
    let mut lines = String::new();
    let counted = case.eligible;
    let countable_segments = counted.saturating_sub(case.excluded);
    push(
        &mut lines,
        "coverage.captures.excluded",
        i64::from(case.excluded_captures),
    );
    push(
        &mut lines,
        "coverage.captures.placed",
        i64::from(case.placed_captures),
    );
    push(
        &mut lines,
        "coverage.captures.unaccounted",
        i64::from(case.unaccounted_captures),
    );
    push(
        &mut lines,
        "coverage.config.gap_threshold_nanos",
        i64::from(crate::COVERAGE_CONFIG_V1.gap_threshold_nanos() as i32),
    );
    push(
        &mut lines,
        "coverage.config.low_confidence_permille",
        i64::from(crate::COVERAGE_CONFIG_V1.low_confidence_at_or_below_permille()),
    );
    push(
        &mut lines,
        "coverage.config.version",
        i64::from(crate::COVERAGE_CONFIG_V1.version()),
    );
    push(&mut lines, "coverage.gaps.total", i64::from(case.gaps));
    push(
        &mut lines,
        "coverage.gaps.unexplained",
        i64::from(case.unexplained_gaps),
    );
    push(
        &mut lines,
        "coverage.ordering.exceptions",
        i64::from(case.ordering_exceptions),
    );
    push(
        &mut lines,
        "coverage.ordering.findings",
        i64::from(case.ordering_findings),
    );
    push(
        &mut lines,
        "coverage.render.defects",
        i64::from(case.render_defects),
    );
    for index in 0..counted {
        let status = status_of(case, index);
        let tokens = tokens_of(case, index);
        lines.push_str(&format!(
            "coverage.segment.{index:04}.id=ref:raw_segment_{:04}\n",
            index + 1
        ));
        lines.push_str(&format!("coverage.segment.{index:04}.status=ref:{status}\n"));
        push_into(
            &mut lines,
            &format!("coverage.segment.{index:04}.tokens"),
            i64::from(tokens),
        );
    }
    push(
        &mut lines,
        "coverage.segment_coverage.denominator",
        i64::from(countable_segments),
    );
    push(
        &mut lines,
        "coverage.segment_coverage.numerator",
        i64::from(case.mapped),
    );
    push(
        &mut lines,
        "coverage.segments.eligible",
        i64::from(case.eligible),
    );
    push(
        &mut lines,
        "coverage.segments.unmapped",
        i64::from(case.unmapped),
    );
    push(
        &mut lines,
        "coverage.token_coverage.denominator",
        i64::from(case.countable_tokens),
    );
    push(
        &mut lines,
        "coverage.token_coverage.numerator",
        i64::from(case.mapped_tokens),
    );
    let mut sorted: Vec<&str> = lines.lines().collect();
    sorted.sort_unstable();
    let mut canonical = String::new();
    for line in sorted {
        canonical.push_str(line);
        canonical.push('\n');
    }
    FrozenInputs::parse(&canonical)
}

fn push(lines: &mut String, key: &str, value: i64) {
    push_into(lines, key, value);
}

fn push_into(lines: &mut String, key: &str, value: i64) {
    lines.push_str(key);
    lines.push_str("=int:");
    lines.push_str(&value.to_string());
    lines.push('\n');
}

fn status_of(case: &Case, index: u32) -> &'static str {
    let mut boundary = case.mapped;
    if index < boundary {
        return "MAPPED";
    }
    boundary = boundary.saturating_add(case.excluded);
    if index < boundary {
        return "EXCLUDED_NON_SPEECH";
    }
    boundary = boundary.saturating_add(case.redacted);
    if index < boundary {
        return "REDACTED_WITH_POLICY";
    }
    boundary = boundary.saturating_add(case.failed);
    if index < boundary {
        return "UNTRANSCRIBED_FAILURE";
    }
    "UNMAPPED"
}

fn tokens_of(case: &Case, index: u32) -> u32 {
    let _ = index;
    if case.eligible == 0 {
        return 0;
    }
    5
}

/// Every file the committed corpus holds.
///
/// # Errors
///
/// [`EngineError`] when a case does not encode or does not evaluate, which is
/// a bug in this module.
pub fn corpus_files() -> Result<Vec<CorpusFile>, EngineError> {
    let hash = ruleset_hash();
    let version = EngineVersion::new(TRANSCRIPT_COVERAGE_ENGINE_VERSION)?;
    let mut files = vec![CorpusFile {
        path: format!("{HARNESS_ROOT}/{HARNESS_DIR}/ruleset.txt"),
        bytes: RULESET_TEXT.as_bytes().to_vec(),
    }];
    files.push(CorpusFile {
        path: format!("{HARNESS_ROOT}/{HARNESS_DIR}/property/bounds.txt"),
        bytes: property_bounds().into_bytes(),
    });
    for case in &GOLDEN {
        let inputs = case_input(case)?;
        files.push(CorpusFile {
            path: format!("{HARNESS_ROOT}/{HARNESS_DIR}/golden/{}.input", case.name),
            bytes: inputs.canonical_text().into_bytes(),
        });
        files.push(CorpusFile {
            path: format!("{HARNESS_ROOT}/{HARNESS_DIR}/golden/{}.expected", case.name),
            bytes: expected_bytes(&inputs, hash, version)?,
        });
    }
    let compat = case_input(&VERSION_COMPAT)?;
    files.push(CorpusFile {
        path: format!(
            "{HARNESS_ROOT}/{HARNESS_DIR}/version-compat/v1-{}.input",
            VERSION_COMPAT.name
        ),
        bytes: compat.canonical_text().into_bytes(),
    });
    files.push(CorpusFile {
        path: format!(
            "{HARNESS_ROOT}/{HARNESS_DIR}/version-compat/v1-{}.explanation",
            VERSION_COMPAT.name
        ),
        bytes: explanation_bytes(&compat, hash)?,
    });
    let snapshot = case_input(&SNAPSHOT_CASE)?;
    files.push(CorpusFile {
        path: format!("{HARNESS_ROOT}/{HARNESS_DIR}/explanation.snapshot"),
        bytes: explanation_bytes(&snapshot, hash)?,
    });
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

/// The canonical bytes of one case's outcome.
///
/// # Errors
///
/// [`EngineError`] when the case does not evaluate.
pub fn expected_bytes(
    inputs: &FrozenInputs,
    hash: RuleSetHash,
    version: EngineVersion,
) -> Result<Vec<u8>, EngineError> {
    let outcome = TranscriptCoverageEngine::evaluate_coverage(inputs, hash)?;
    Ok(outcome.canonical_bytes(TRANSCRIPT_COVERAGE_ENGINE_ID, hash, version, inputs))
}

/// The normalized explanation of one case.
///
/// # Errors
///
/// [`EngineError`] when the case does not evaluate.
pub fn explanation_bytes(inputs: &FrozenInputs, hash: RuleSetHash) -> Result<Vec<u8>, EngineError> {
    let outcome = TranscriptCoverageEngine::evaluate_coverage(inputs, hash)?;
    Ok(outcome.explanation_snapshot.as_str().as_bytes().to_vec())
}

fn property_bounds() -> String {
    format!(
        "property: every eligible segment lands in exactly one of the four \
         statuses or in the unmapped list\n\
         generator: segments 1..={PROPERTY_MAX_SEGMENTS}\n\
         generator: status per segment drawn from \
         MAPPED|EXCLUDED_NON_SPEECH|REDACTED_WITH_POLICY|UNTRANSCRIBED_FAILURE|UNMAPPED\n\
         generator: tokens per segment 1..=8\n\
         invariant: accounts + unmapped == eligible, and no index appears twice\n\
         invariant: a witness exists only when unmapped == 0\n"
    )
}
