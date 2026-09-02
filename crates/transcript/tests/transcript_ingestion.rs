//! The `P2-U7` named acceptance suite.
//!
//! Six of the seven rows t068 §5 names for this task, under the exact names it
//! gives. `transcript_original_is_ciphertext_at_rest` needs the encrypted
//! object lane and lives in `transcript_encrypted.rs`; the `IN03` and `IN04`
//! fault rows live in `transcript_faults.rs`.

mod support;

use std::{error::Error, fs};

use academic_domain::{
    Actor, AuthorityClass, ClaimId, ClaimObject, ClaimRelationKind, ConfidencePermille, Decimal,
    DomainError, EntityId, EpistemicStatus, EvidenceId, PredicateId, ScopeId, TimestampMillis,
    TranscriptVersionId, ValidInterval,
};
use academic_transcript::{
    TranscriptError,
    admission::AdmittedImport,
    claims::{ModelRead, RowClaimContext, RowClaimIds, confirm_reconciled_rows, import_row_claim},
    reconcile::{HaltCause, TranscriptChecksums, reconcile},
    record::{
        IdentityField, NormalizedTranscript, TranscriptField, TranscriptIdentity, TranscriptRow,
    },
    redaction::{RedactionProfile, project, redacted_export},
    session::{self, SessionState},
    source::{
        CORPUS_CREATOR_TOOL, CORPUS_EXIF_SOFTWARE, CORPUS_PRODUCER, ORIGINAL_ONLY_MARKERS,
        TranscriptFormat, build_synthetic_transcript_pdf, parse_csv, parse_manual_entry,
        parse_pdf_text_layer, render_csv, render_manual_entries,
    },
};
use support::{TestRoot, canary, contains, find_all, refusal, synthetic_transcript};

type TestResult = Result<(), Box<dyn Error>>;

/// Stable synthetic identities. UUIDv7-shaped, never generated from a clock.
const SUBJECT_ENTITY: &str = "01900000-0000-7000-8000-0000000007a1";
const SCOPE: &str = "01900000-0000-7000-8000-0000000007a2";
const USER: &str = "01900000-0000-7000-8000-0000000007a3";
const IMPORT_EVIDENCE: &str = "01900000-0000-7000-8000-0000000007a4";
const CONFIRMATION_EVIDENCE: &str = "01900000-0000-7000-8000-0000000007a5";
const VERSION: &str = "01900000-0000-7000-8000-0000000007a6";
const MODEL_RUN: &str = "01900000-0000-7000-8000-0000000007a7";
const IMPORT_CLAIM_BASE: &str = "01900000-0000-7000-8000-0000000007b";
const CONFIRMED_CLAIM_BASE: &str = "01900000-0000-7000-8000-0000000007c";

fn context() -> Result<RowClaimContext, Box<dyn Error>> {
    Ok(RowClaimContext {
        subject_entity_id: SUBJECT_ENTITY.parse()?,
        scope_id: SCOPE.parse()?,
        valid_time: ValidInterval::new(TimestampMillis::new(1_700_000_000_000), None)?,
        import_evidence_ids: vec![IMPORT_EVIDENCE.parse::<EvidenceId>()?],
        confirmation_evidence_ids: vec![CONFIRMATION_EVIDENCE.parse::<EvidenceId>()?],
    })
}

fn claim_ids(count: usize) -> Result<Vec<RowClaimIds>, Box<dyn Error>> {
    (0..count)
        .map(|index| {
            Ok(RowClaimIds {
                import_claim_id: format!("{IMPORT_CLAIM_BASE}{index}").parse::<ClaimId>()?,
                confirmed_claim_id: format!("{CONFIRMED_CLAIM_BASE}{index}").parse::<ClaimId>()?,
            })
        })
        .collect()
}

/// Rebuilds a transcript with exactly one field of one row replaced.
fn with_field(
    transcript: &NormalizedTranscript,
    ordinal: u32,
    field: TranscriptField,
    replacement: &str,
) -> Result<NormalizedTranscript, Box<dyn Error>> {
    let identity = transcript.identity();
    let identity = TranscriptIdentity::new(
        identity.student_number(),
        identity.student_name(),
        identity.institution(),
        identity.issued_on(),
    )?;
    let mut rows = Vec::new();
    for row in transcript.rows() {
        if row.ordinal() == ordinal {
            let credits = if field == TranscriptField::Credits {
                academic_transcript::record::parse_decimal(replacement)?
            } else {
                row.credits()
            };
            let pick = |target: TranscriptField, current: &str| -> String {
                if target == field {
                    replacement.to_owned()
                } else {
                    current.to_owned()
                }
            };
            rows.push(TranscriptRow::new(
                row.ordinal(),
                pick(TranscriptField::CourseCode, row.course_code()),
                pick(TranscriptField::Term, row.term()),
                credits,
                pick(TranscriptField::Grade, row.grade()),
            )?);
        } else {
            rows.push(row.clone());
        }
    }
    Ok(NormalizedTranscript::new(identity, rows)?)
}

// ---------------------------------------------------------------------------
// 1. transcript_formats_normalize_equivalently
// ---------------------------------------------------------------------------

/// PDF, CSV and manual entry of one official record produce byte-identical
/// canonical bytes.
///
/// Byte equality of the canonical encoding is the assertion rather than a
/// field-by-field walk: the checksum block is derived from these bytes, so a
/// difference the walk would tolerate is a difference reconciliation would
/// halt on.
#[test]
fn transcript_formats_normalize_equivalently() -> TestResult {
    let expected = synthetic_transcript()?;

    let pdf = build_synthetic_transcript_pdf(&expected);
    let from_pdf = parse_pdf_text_layer(&pdf.bytes)?;
    let from_csv = parse_csv(render_csv(&expected).as_bytes())?;
    let identity = expected.identity();
    let from_manual = parse_manual_entry(
        identity.student_number(),
        identity.student_name(),
        identity.institution(),
        identity.issued_on(),
        &render_manual_entries(&expected),
    )?;

    for (label, actual) in [
        ("PDF text layer", &from_pdf),
        ("CSV", &from_csv),
        ("manual entry", &from_manual),
    ] {
        assert_eq!(
            actual.canonical_bytes(),
            expected.canonical_bytes(),
            "{label} did not normalize to the canonical record"
        );
        assert_eq!(actual.canonical_digest(), expected.canonical_digest());
    }

    // Credit spelling is normalized, not compared raw: an official CSV that
    // writes `3.000` and a hand entry that writes `3` are the same value, and a
    // checksum over the raw spelling would call them a field mismatch.
    let respelt = render_csv(&expected).replace(",3,", ",3.000,");
    assert_ne!(respelt, render_csv(&expected), "the respelling did nothing");
    assert_eq!(
        parse_csv(respelt.as_bytes())?.canonical_bytes(),
        expected.canonical_bytes()
    );

    // Non-vacuity: a changed grade is a different record. If this passed, the
    // three equalities above would be measuring nothing.
    let altered = render_csv(&expected).replace(",A0", ",B0");
    assert_ne!(altered, render_csv(&expected), "the alteration did nothing");
    assert_ne!(
        parse_csv(altered.as_bytes())?.canonical_bytes(),
        expected.canonical_bytes()
    );

    // A source that does not match its declared grammar is refused rather than
    // partially read: section 29.1's "deterministic parse".
    let error = refusal(
        parse_csv(b"STUDENT_NUMBER,x\nnot-the-declared-header\n"),
        "an undeclared header parsed",
    )?;
    assert_eq!(error.code(), "MALFORMED_SOURCE");
    let error = refusal(parse_pdf_text_layer(b"not a pdf"), "a non-PDF parsed")?;
    assert_eq!(error.code(), "MALFORMED_SOURCE");

    // A document truncated mid-row is refused at the short row rather than
    // yielding the rows before it. A refusal is an `Err`, so there is no
    // partially-populated transcript to observe — which is the whole content of
    // "a partially-read document never becomes a partially-populated one".
    let truncated = format!("{}M1522.000900,2024-2,3\n", render_csv(&expected));
    let error = refusal(
        parse_csv(truncated.as_bytes()),
        "a row with three fields parsed",
    )?;
    assert_eq!(error.code(), "MALFORMED_SOURCE");

    // The canonical encoding is length-prefixed rather than delimited, so no
    // field value can spell a separator and change the parse of its neighbour:
    // two records that differ only in where a boundary falls encode
    // differently.
    let left = NormalizedTranscript::new(
        TranscriptIdentity::new("AB", "C", "inst", "date")?,
        Vec::new(),
    )?;
    let right = NormalizedTranscript::new(
        TranscriptIdentity::new("A", "BC", "inst", "date")?,
        Vec::new(),
    )?;
    assert_ne!(left.canonical_bytes(), right.canonical_bytes());
    assert_ne!(left.canonical_digest(), right.canonical_digest());
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. ocr_row_and_confirmed_row_are_distinct_claims
// ---------------------------------------------------------------------------

/// An import row and the row the user confirmed are two claims, and no import
/// route can mint the second.
///
/// The separation is not a rule this crate adds. `Claim::validate_for_actor`
/// already refuses `UserExplicit` to every actor but `Actor::User`, so an OCR
/// pass that tried to publish a user-confirmed row is rejected by the canonical
/// vocabulary before this crate sees it. The last two blocks inject exactly
/// that and observe the refusal.
#[test]
fn ocr_row_and_confirmed_row_are_distinct_claims() -> TestResult {
    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);
    let outcome = reconcile(&transcript, &reference);
    let reconciled = outcome.reconciled().ok_or("reconciliation halted")?;
    let context = context()?;
    let ids = claim_ids(transcript.rows().len())?;
    let confidence = ConfidencePermille::new(880)?;
    let model_read = ModelRead {
        run_id: MODEL_RUN.parse::<EntityId>()?,
        confidence,
    };

    let linked = confirm_reconciled_rows(
        reconciled,
        TranscriptFormat::PdfOcr,
        Some(model_read),
        USER.parse::<EntityId>()?,
        &ids,
        &context,
    )?;
    assert_eq!(linked.len(), transcript.rows().len());

    for (row, entry) in transcript.rows().iter().zip(&linked) {
        let import = entry.import.claim();
        let confirmed = entry.confirmed.claim();

        assert_ne!(import.id, confirmed.id, "one claim, not two");
        assert_eq!(import.epistemic_status, EpistemicStatus::AiInferred);
        assert_eq!(import.authority_class, AuthorityClass::ModelInference);
        assert_eq!(import.confidence, Some(confidence));
        // The model run is its own entity: citing a run means naming the run,
        // not the row's subject.
        assert_eq!(
            *entry.import.actor(),
            Actor::ModelRun {
                run_id: MODEL_RUN.parse::<EntityId>()?
            }
        );
        assert_ne!(model_read.run_id, context.subject_entity_id);

        assert_eq!(confirmed.epistemic_status, EpistemicStatus::UserConfirmed);
        assert_eq!(confirmed.authority_class, AuthorityClass::UserExplicit);
        assert_eq!(
            confirmed.confidence, None,
            "a confirmation is not an estimate"
        );
        assert!(matches!(entry.confirmed.actor(), Actor::User { .. }));

        // Both are about the same row, and the link is explicit rather than
        // positional.
        assert_eq!(import.object, confirmed.object);
        assert_eq!(entry.import.ordinal(), row.ordinal());
        assert_eq!(entry.confirmed.import_claim_id(), import.id);
        assert_eq!(entry.relation.source_claim_id, import.id);
        assert_eq!(entry.relation.target_claim_id, confirmed.id);
        assert_eq!(entry.relation.kind, ClaimRelationKind::Supports);

        // One step outside this row's contract: a claim object is copied into
        // projections and proof trees, so it must not carry the identity the
        // redaction projection removes.
        let ClaimObject::Text(text) = &import.object else {
            return Err("a transcript row object is not text".into());
        };
        assert!(!text.contains(canary("CANARY-STUDENT-NUMBER")));
        assert!(!text.contains(canary("CANARY-STUDENT-NAME")));
    }

    // A deterministic read is a third, distinct provenance: an importer, a
    // direct observation, and no confidence.
    let deterministic = import_row_claim(
        &transcript.rows()[0],
        TranscriptFormat::Csv,
        None,
        ids[0],
        &context,
    )?;
    assert_eq!(
        deterministic.claim().epistemic_status,
        EpistemicStatus::CodeObserved
    );
    assert_eq!(
        deterministic.claim().authority_class,
        AuthorityClass::DirectObservation
    );
    assert!(matches!(deterministic.actor(), Actor::Importer { .. }));

    // Confidence belongs to inference and nowhere else.
    let error = refusal(
        import_row_claim(
            &transcript.rows()[0],
            TranscriptFormat::PdfOcr,
            None,
            ids[0],
            &context,
        ),
        "a model read published with no run or confidence",
    )?;
    assert_eq!(error.code(), "MODEL_READ_NEEDS_CONFIDENCE");
    let error = refusal(
        import_row_claim(
            &transcript.rows()[0],
            TranscriptFormat::Csv,
            Some(model_read),
            ids[0],
            &context,
        ),
        "a deterministic read published a model run",
    )?;
    assert_eq!(error.code(), "DETERMINISTIC_READ_CARRIES_CONFIDENCE");

    // The injection: an import actor asserting a user-confirmed row. Both
    // import actors are tried, because a guard that only refuses one of them
    // leaves the other route open.
    let user_confirmed = academic_domain::Claim {
        id: ids[0].import_claim_id,
        subject_entity_id: context.subject_entity_id,
        predicate_id: PredicateId::parse(academic_transcript::claims::TRANSCRIPT_ROW_PREDICATE)?,
        object: ClaimObject::Text("COURSE_CODE=x;TERM=y;CREDITS=3;GRADE=A0".to_owned()),
        scope_id: context.scope_id,
        authority_class: AuthorityClass::UserExplicit,
        epistemic_status: EpistemicStatus::UserConfirmed,
        confidence: None,
        prediction_metadata: None,
        valid_time: context.valid_time,
        evidence_ids: context.import_evidence_ids.clone(),
    };
    for actor in [
        Actor::ModelRun {
            run_id: context.subject_entity_id,
        },
        Actor::Importer {
            name: "academic-transcript-normalizer".to_owned(),
            version: "1".to_owned(),
        },
    ] {
        let kind = actor.kind_name();
        let error = refusal(
            user_confirmed.validate_for_actor(&actor),
            "an import actor asserted a user confirmation",
        )?;
        assert!(
            matches!(error, DomainError::ActorAuthorityMismatch { .. }),
            "{kind} was refused for the wrong reason: {error}"
        );
    }
    // ... and the same claim under the user actor is accepted, so the refusal
    // above is about the actor rather than about the claim being malformed.
    user_confirmed.validate_for_actor(&Actor::User {
        user_id: USER.parse::<EntityId>()?,
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. field_level_mismatch_is_localized_before_confirmation
// ---------------------------------------------------------------------------

/// A checksum disagreement names one row and the fields inside it, and yields
/// nothing a confirmation can be built from.
#[test]
fn field_level_mismatch_is_localized_before_confirmation() -> TestResult {
    let transcript = synthetic_transcript()?;
    let reference = TranscriptChecksums::of(&transcript);

    // One field at a time, in the middle row, so there is a reconciled row on
    // each side of the halt.
    for field in TranscriptField::ALL {
        let replacement = match field {
            TranscriptField::CourseCode => "M9999.000000",
            TranscriptField::Term => "2099-2",
            TranscriptField::Credits => "3.5",
            TranscriptField::Grade => "C-",
        };
        let candidate = with_field(&transcript, 1, field, replacement)?;
        let outcome = reconcile(&candidate, &reference);
        let halt = outcome
            .halt()
            .ok_or_else(|| format!("{field} disagreement did not halt"))?;

        assert_eq!(halt.ordinal(), 1, "{field} halted at the wrong row");
        assert_eq!(
            halt.disagreeing_fields(),
            &[field],
            "{field} was not the localized field"
        );
        assert_eq!(halt.rows_reconciled_before_halt(), 1);
        assert!(
            outcome.reconciled().is_none(),
            "{field} produced a confirmable transcript"
        );

        // The halt is a report that reaches a screen. It carries no identity.
        let rendered = format!("{halt:?}");
        assert!(!rendered.contains(canary("CANARY-STUDENT-NUMBER")));
        assert!(!rendered.contains(canary("CANARY-STUDENT-NAME")));
    }

    // Two fields of one row: both named, still one row.
    let candidate = with_field(&transcript, 1, TranscriptField::Term, "2099-2")?;
    let candidate = with_field(&candidate, 1, TranscriptField::Grade, "C-")?;
    let outcome = reconcile(&candidate, &reference);
    let halt = outcome
        .halt()
        .ok_or("a two-field disagreement did not halt")?;
    assert_eq!(halt.ordinal(), 1);
    assert_eq!(
        halt.disagreeing_fields(),
        &[TranscriptField::Term, TranscriptField::Grade]
    );

    // A disagreement in the last row halts there, not at the document. This is
    // what "does not collapse into a whole-transcript failure" measures: the
    // reported position moves with the defect.
    let candidate = with_field(&transcript, 2, TranscriptField::Grade, "F")?;
    let halt = reconcile(&candidate, &reference)
        .halt()
        .cloned()
        .ok_or("a last-row disagreement did not halt")?;
    assert_eq!(halt.ordinal(), 2);
    assert_eq!(halt.rows_reconciled_before_halt(), 2);

    // A row count difference is localized the same way, and the two directions
    // are distinguished rather than merged into "counts differ".
    let mut short_rows = transcript.rows().to_vec();
    short_rows.pop();
    let short = NormalizedTranscript::new(
        TranscriptIdentity::new(
            transcript.identity().student_number(),
            transcript.identity().student_name(),
            transcript.identity().institution(),
            transcript.identity().issued_on(),
        )?,
        short_rows,
    )?;
    let halt = reconcile(&short, &reference)
        .halt()
        .cloned()
        .ok_or("a short candidate did not halt")?;
    assert_eq!(halt.ordinal(), 2);
    assert_eq!(*halt.cause(), HaltCause::RowAbsentFromCandidate);

    let short_reference = TranscriptChecksums::of(&short);
    let halt = reconcile(&transcript, &short_reference)
        .halt()
        .cloned()
        .ok_or("a long candidate did not halt")?;
    assert_eq!(halt.ordinal(), 2);
    assert_eq!(*halt.cause(), HaltCause::RowAbsentFromReference);

    // Non-vacuity: the same corpus with no perturbation reconciles, so the
    // halts above are caused by the perturbations and not by the harness.
    let outcome = reconcile(&transcript, &reference);
    assert!(outcome.halt().is_none());
    let reconciled = outcome
        .reconciled()
        .ok_or("a clean corpus did not reconcile")?;
    assert_eq!(
        reconciled.reference_identity_digest(),
        reference.identity_digest()
    );

    // Two readings may spell the identity header differently without halting:
    // the four reconciled fields are the contract, and discarding them over a
    // transliterated name would be a whole-document verdict on a field no
    // calculation reads.
    let renamed = NormalizedTranscript::new(
        TranscriptIdentity::new(
            "2019-00000",
            "a different transliteration",
            transcript.identity().institution(),
            transcript.identity().issued_on(),
        )?,
        transcript.rows().to_vec(),
    )?;
    assert!(reconcile(&renamed, &reference).reconciled().is_some());
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. student_number_and_name_can_be_removed_independently
// ---------------------------------------------------------------------------

/// All four combinations of the two removable fields are constructible,
/// distinct, and leave the source untouched.
#[test]
fn student_number_and_name_can_be_removed_independently() -> TestResult {
    let transcript = synthetic_transcript()?;
    let before = transcript.canonical_digest();
    let number = canary("CANARY-STUDENT-NUMBER");
    let name = canary("CANARY-STUDENT-NAME");

    let profiles = RedactionProfile::all();
    // Exhaustive by arithmetic, not by a literal: `all()` must enumerate every
    // subset of the removable fields. A third `IdentityField` added without
    // growing this constant fails here rather than silently narrowing the
    // matrix below to a fraction of the combinations.
    assert_eq!(
        profiles.len(),
        1_usize << IdentityField::ALL.len(),
        "RedactionProfile::all() no longer enumerates every subset of IdentityField::ALL"
    );
    let mut removals: Vec<Vec<IdentityField>> = profiles
        .iter()
        .map(|profile| profile.removed_fields())
        .collect();
    removals.sort();
    removals.dedup();
    assert_eq!(
        removals.len(),
        profiles.len(),
        "two profiles remove the same fields"
    );

    let mut exports = Vec::new();
    for profile in profiles {
        let projection = project(&transcript, profile);
        let removes_number = profile.removes(IdentityField::StudentNumber);
        let removes_name = profile.removes(IdentityField::StudentName);

        assert_eq!(
            projection.student_number().is_none(),
            removes_number,
            "{profile:?} kept or dropped the student number against its own profile"
        );
        assert_eq!(
            projection.student_name().is_none(),
            removes_name,
            "{profile:?} kept or dropped the name against its own profile"
        );

        let export = redacted_export(&projection);
        assert_eq!(
            contains(&export, number.as_bytes()),
            !removes_number,
            "{profile:?} exported the student number against its own profile"
        );
        assert_eq!(
            contains(&export, name.as_bytes()),
            !removes_name,
            "{profile:?} exported the name against its own profile"
        );

        // Removal is absence, not blanking: the label is gone with the value.
        assert!(
            contains(&export, IdentityField::StudentNumber.as_str().as_bytes()),
            "the export must always declare what it removed"
        );

        // The rows and the fields nothing removes survive every profile.
        assert!(contains(&export, canary("CANARY-COURSE-CODE").as_bytes()));
        assert!(contains(&export, canary("CANARY-INSTITUTION").as_bytes()));
        assert_eq!(projection.rows().len(), transcript.rows().len());

        exports.push(export);
    }

    // The four are pairwise distinct: independence means each field's removal
    // changes the result on its own.
    for (left_index, left) in exports.iter().enumerate() {
        for right in exports.iter().skip(left_index + 1) {
            assert_ne!(
                left, right,
                "two of the four profiles export the same bytes"
            );
        }
    }

    // Redaction is a projection: the source is byte-identical afterwards.
    assert_eq!(transcript.canonical_digest(), before);
    assert_eq!(
        transcript.identity().student_number(),
        number,
        "projecting edited the source"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. redacted_export_contains_no_original_bytes_or_metadata
// ---------------------------------------------------------------------------

/// Smallest byte run the window scan treats as shared with the original.
///
/// Sixteen: long enough that a coincidental collision between an export line
/// and the original's text layer is not plausible, short enough to catch a
/// single leaked metadata string.
const SHARED_WINDOW_BYTES: usize = 16;

/// Names every way the export bytes still carry the original document.
///
/// Returned as findings rather than asserted here, so the same function can be
/// run against a deliberately leaked export and observed to fail.
fn leak_findings(
    export: &[u8],
    original: &[u8],
    retained: &[&str],
    removed: &[&str],
) -> Vec<String> {
    let mut findings = Vec::new();
    for marker in ORIGINAL_ONLY_MARKERS {
        if contains(export, marker.as_bytes()) {
            findings.push(format!("container or metadata marker {marker:?}"));
        }
    }
    for generator in [CORPUS_PRODUCER, CORPUS_CREATOR_TOOL, CORPUS_EXIF_SOFTWARE] {
        if contains(export, generator.as_bytes()) {
            findings.push(format!("generator string {generator:?}"));
        }
    }
    for value in removed {
        if contains(export, value.as_bytes()) {
            findings.push("a removed identity value".to_owned());
        }
    }
    if original.len() >= SHARED_WINDOW_BYTES {
        for start in 0..=original.len() - SHARED_WINDOW_BYTES {
            let window = &original[start..start + SHARED_WINDOW_BYTES];
            if find_all(export, window).is_empty() {
                continue;
            }
            let inside_retained = retained
                .iter()
                .any(|value| contains(value.as_bytes(), window));
            if !inside_retained {
                findings.push(format!(
                    "a {SHARED_WINDOW_BYTES}-byte run of the original at offset {start} that is \
                     not inside a retained value"
                ));
            }
        }
    }
    findings
}

/// A redacted export carries no byte and no metadata string of the original.
///
/// The structural half is the signature: `redacted_export` takes a projection
/// and nothing else, and a projection owns no original byte. The observed half
/// is below, because a structural argument that is never executed is exactly
/// the failure this repository keeps repeating — so the scan is also run
/// against three injected leaks and observed to fail on each.
#[test]
fn redacted_export_contains_no_original_bytes_or_metadata() -> TestResult {
    let transcript = synthetic_transcript()?;
    let original = build_synthetic_transcript_pdf(&transcript).bytes;

    // The corpus really carries what the scan looks for. Without this, a clean
    // result would mean "the original had no metadata" as easily as "the export
    // dropped it".
    for marker in ORIGINAL_ONLY_MARKERS {
        assert!(
            contains(&original, marker.as_bytes()),
            "the corpus does not carry {marker:?}, so the scan cannot be evidence"
        );
    }
    for generator in [CORPUS_PRODUCER, CORPUS_CREATOR_TOOL, CORPUS_EXIF_SOFTWARE] {
        assert!(contains(&original, generator.as_bytes()));
    }

    let profile =
        RedactionProfile::removing(&[IdentityField::StudentNumber, IdentityField::StudentName]);
    let projection = project(&transcript, profile);
    let export = redacted_export(&projection);
    let retained = projection.retained_values();
    let removed = [
        canary("CANARY-STUDENT-NUMBER"),
        canary("CANARY-STUDENT-NAME"),
    ];

    let findings = leak_findings(&export, &original, &retained, &removed);
    assert!(
        findings.is_empty(),
        "the redacted export leaked: {findings:?}"
    );

    // Injection 1 — a metadata string. Producing an export from the document
    // information dictionary instead of from the projection would look like
    // this.
    let mut leaked = export.clone();
    leaked.extend_from_slice(format!("PRODUCER\t{CORPUS_PRODUCER}\n").as_bytes());
    assert!(
        !leak_findings(&leaked, &original, &retained, &removed).is_empty(),
        "the scan passed an export carrying the generator string"
    );

    // Injection 2 — raw original bytes. A "keep the source for provenance"
    // change would look like this.
    let mut leaked = export.clone();
    leaked.extend_from_slice(&original[..128]);
    assert!(
        !leak_findings(&leaked, &original, &retained, &removed).is_empty(),
        "the scan passed an export carrying raw original bytes"
    );

    // Injection 3 — the removed identity, reintroduced from the source. A
    // redaction implemented as a display filter rather than as a projection
    // would look like this.
    let mut leaked = export.clone();
    leaked.extend_from_slice(transcript.identity().student_number().as_bytes());
    assert!(
        !leak_findings(&leaked, &original, &retained, &removed).is_empty(),
        "the scan passed an export carrying the removed student number"
    );

    // The unredacted export is not a leak: it retains the identity on purpose,
    // and the window scan must not confuse a retained value with an original
    // byte run.
    let retain_all = project(&transcript, RedactionProfile::retain_all());
    let export = redacted_export(&retain_all);
    let retained = retain_all.retained_values();
    let findings = leak_findings(&export, &original, &retained, &[]);
    assert!(
        findings.is_empty(),
        "the retain-all export was read as a leak: {findings:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. import_without_admission_receipt_is_refused
// ---------------------------------------------------------------------------

/// The committed candidate receipt, hex-encoded.
const CANDIDATE_RECEIPT_HEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/admission/incomplete-receipt.cbor.hex"
));

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let digits: Vec<u8> = value
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !digits.len().is_multiple_of(2) {
        return Err("odd hex length".into());
    }
    let mut out = Vec::with_capacity(digits.len() / 2);
    for pair in digits.as_chunks::<2>().0 {
        let text = std::str::from_utf8(pair)?;
        out.push(u8::from_str_radix(text, 16)?);
    }
    Ok(out)
}

/// Every profile-touching import is refused, because admission is closed.
///
/// This is the current behaviour of the product, not an error path the suite
/// contrives: `P2-K6` compiled an unprovisioned acceptance key and the
/// committed candidate receipt carries two of the five required platform rows,
/// so both halves fail closed.
#[test]
fn import_without_admission_receipt_is_refused() -> TestResult {
    let version: TranscriptVersionId = VERSION.parse()?;

    // A profile with no receipt at all.
    let root = TestRoot::new("no-receipt")?;
    let error = refusal(
        AdmittedImport::open(root.path()),
        "an unadmitted profile was admitted",
    )?;
    assert!(
        matches!(
            error,
            TranscriptError::AdmissionRefused {
                code: "ADMISSION_RECEIPT_ABSENT"
            }
        ),
        "refused for the wrong reason: {error}"
    );

    // A profile carrying the committed candidate receipt. It is refused before
    // its platform rows are ever counted, because the compiled acceptance key
    // is unprovisioned — which is the first of the two open conditions.
    let root = TestRoot::new("candidate-receipt")?;
    let receipt = root.path().join("admission");
    fs::create_dir_all(&receipt)?;
    fs::write(
        receipt.join("receipt.cbor"),
        decode_hex(CANDIDATE_RECEIPT_HEX)?,
    )?;
    let error = refusal(
        AdmittedImport::open(root.path()),
        "a candidate receipt was admitted",
    )?;
    assert!(
        matches!(
            error,
            TranscriptError::AdmissionRefused {
                code: "ADMISSION_ACCEPTANCE_KEY_UNPROVISIONED"
            }
        ),
        "refused for the wrong reason: {error}"
    );

    // The gate is the verifier, not a second opinion about it.
    assert!(academic_admission::AdmissionVerifier::verify(root.path()).is_err());

    // Nothing durable was created by either refusal: no session directory, no
    // lease, no staged set. `ImportSession::begin` takes an `AdmittedImport` by
    // reference and no `AdmittedImport` exists, so there is no argument to call
    // it with — the refusal is carried by the type, and this is the observable
    // consequence.
    assert_eq!(
        session::inspect(root.path(), version)?,
        SessionState::Absent
    );
    assert!(!session::session_directory(root.path(), version).exists());
    assert!(!root.path().join("transcript").exists());
    Ok(())
}

/// The two policy labels a transcript original is sealed under are fixed.
///
/// The seal itself needs the encrypted object lane and is in
/// `transcript_encrypted.rs`. What is checked here, in the default lane on
/// every platform, is that the two labels the plan fixes are the two the
/// canonical vocabulary spells.
#[test]
fn transcript_original_policy_labels_are_restricted_and_user_managed() -> TestResult {
    use academic_domain::{Confidentiality, RetentionClass};

    assert_eq!(
        format!("{:?}", Confidentiality::Restricted),
        "Restricted",
        "the RESTRICTED label moved"
    );
    assert_eq!(
        format!("{:?}", RetentionClass::UserManaged),
        "UserManaged",
        "the USER_MANAGED label moved"
    );
    Ok(())
}

/// Section 38 stays open: nothing here invents a transcript or a term.
///
/// `GATE-38-005` and `GATE-38-007` are user-supplied inputs. The observable
/// form of "nothing is inferred" is that every absent input is a refusal naming
/// the absent field, and never a default.
#[test]
fn nothing_is_inferred_for_an_absent_user_input() -> TestResult {
    // A CSV missing the issue date is refused, not defaulted.
    let transcript = synthetic_transcript()?;
    let without_date = render_csv(&transcript)
        .lines()
        .filter(|line| !line.starts_with("ISSUED_ON,"))
        .collect::<Vec<_>>()
        .join("\n");
    let error = refusal(
        parse_csv(without_date.as_bytes()),
        "an absent issue date was filled in",
    )?;
    assert!(
        matches!(
            error,
            TranscriptError::MalformedField {
                field: "issue date",
                reason: "absent"
            }
        ),
        "an absent field produced {error}"
    );

    // An empty transcript is a transcript with no rows, not a transcript with
    // an assumed one.
    let empty =
        NormalizedTranscript::new(TranscriptIdentity::new("s", "n", "i", "d")?, Vec::new())?;
    assert!(empty.rows().is_empty());
    assert_eq!(TranscriptChecksums::of(&empty).row_count(), 0);

    // A credit value is read, never assumed.
    assert_eq!(
        academic_transcript::record::canonical_decimal(Decimal::new(30, 1)?),
        "3"
    );
    assert!(academic_transcript::record::parse_decimal("").is_err());
    Ok(())
}

fn _scope_type_is_used(_: ScopeId) {}
