//! The frozen inputs the graduation engine reads, and the reader that takes
//! them back apart.
//!
//! A deterministic engine is a pure function of `(frozen_inputs,
//! rule_set_hash, engine_version)`. That leaves two ways to build a graduation
//! audit and only one of them is honest: encode the profile and the transcript
//! as frozen inputs and evaluate from *those*, or keep a second in-memory path
//! the golden fixtures never exercise. This module is the first.
//! [`encode`] renders a profile and a transcript as the canonical `key=value`
//! text, [`decode`] reads them back, and the engine computes from the decoded
//! values either way -- so a golden fixture and a product call run the same
//! evaluation over the same values.
//!
//! **The plan is not encoded.** Section 6 binds a `DegreeAuditAggregate` to a
//! profile, a requirement set and a transcript snapshot; a plan is none of
//! those, and putting it in the digest would make an audit's identity move when
//! a proposal did. See [`crate::plan`].
//!
//! Every malformed shape is a typed error. The decoder substitutes no default,
//! because a default here would be a profile field or an attempt fact nobody
//! recorded.

use std::collections::BTreeMap;

use academic_domain::{
    ArtifactId, AttemptId, ContentDigest, CourseId, Decimal, TimestampMillis,
    engines::{FrozenInputs, InputKey, InputValue},
};
use academic_record::{
    attempt::AttemptStatus as RecordAttemptStatus, term::TermKey, views::DispositionReason,
};
use academic_requirement::{
    AdmissionYear, ApprovalAuthority, ApprovalFact, AreaId, CreditAmount, CreditCategory,
    GpaReading, InstructionLanguage, LanguageEvidence, RuleId,
};

use crate::{
    error::AuditError,
    profile::{
        DegreeMode, ExchangeOrTransfer, GraduationStandard, InstitutionId, ProgrammeId,
        StudentProfile,
    },
    source::{RuleSourceIndex, RuleSourceSpan},
    transcript::{EntryAdmission, TranscriptEntry, TranscriptSnapshot},
    verdict::{ConflictReference, SourceFreshnessPolicy},
};

/// Everything one audit reads.
///
/// Every field is a frozen input, and that is the point rather than an
/// implementation detail. A value the engine reads and the digest does not
/// cover would make the engine something other than a function of
/// `(frozen_inputs, rule_set_hash, engine_version)` -- two evaluations could
/// then agree on the declared signature and disagree on the answer, which is
/// precisely what the byte comparison exists to rule out.
///
/// So the source placements, the open conflict cases and the freshness
/// criterion are here rather than on the engine. The published rule set is
/// **not**: it is covered by `rule_set_hash`, which is the other half of the
/// signature and the half a historical replay walks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFacts {
    /// The instant the evaluation is anchored to. An argument, never a clock.
    pub as_of: TimestampMillis,
    /// The frozen profile.
    pub profile: StudentProfile,
    /// The frozen transcript.
    pub transcript: TranscriptSnapshot,
    /// Where each published rule was read from.
    pub sources: RuleSourceIndex,
    /// The conflict cases `P2-U6` opened over this rule set, when the conflict
    /// store was read at all.
    ///
    /// `None` is *nobody looked*; `Some(vec![])` is *the store was read and
    /// held nothing*. They are different facts and they reach different
    /// verdicts, exactly as `freshness` below does. A bare `Vec` spelled both
    /// as an empty list, and `ConflictFreeWitness::establish` then issued a
    /// witness over zero cases and called the audit `DETERMINATE` -- the same
    /// vacuous witness `CoverageWitness::establish` refuses eleven lines above
    /// it, in the same file, in a comment naming this failure mode.
    pub conflicts: Option<Vec<ConflictReference>>,
    /// The recorded source-freshness criterion, when one is recorded.
    pub freshness: Option<SourceFreshnessPolicy>,
}

/// Renders one audit's facts as canonical frozen-input text.
///
/// Entries are indexed by their position in transcript order, zero-padded to
/// three digits so the keys sort the way the entries do. Each list carries its
/// own count, so a truncated input is a refusal rather than a shorter
/// transcript.
pub fn encode(facts: &AuditFacts) -> Result<FrozenInputs, AuditError> {
    let mut entries: Vec<(InputKey, InputValue)> = Vec::new();
    let mut push = |key: String, value: InputValue| -> Result<(), AuditError> {
        entries.push((InputKey::new(&key)?, value));
        Ok(())
    };

    push(
        "audit.as_of".to_owned(),
        InputValue::Integer(facts.as_of.value()),
    )?;

    push(
        "freshness.max_age_seconds".to_owned(),
        facts.freshness.map_or(InputValue::Unknown, |policy| {
            InputValue::Integer(i64::try_from(policy.limit_seconds()).unwrap_or(i64::MAX))
        }),
    )?;

    encode_profile(&facts.profile, &mut push)?;
    encode_transcript(&facts.transcript, &mut push)?;
    encode_sources(&facts.sources, &mut push)?;
    encode_conflicts(facts.conflicts.as_deref(), &mut push)?;

    entries.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
    Ok(FrozenInputs::new(entries)?)
}

/// Encodes where each published rule was read from.
///
/// The digest of an audit therefore moves when a placement moves, which is what
/// makes `adverse/partial_failure` -- a rule the index does not place -- an
/// input file rather than a differently configured engine.
fn encode_sources(
    sources: &RuleSourceIndex,
    push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>,
) -> Result<(), AuditError> {
    let placements: Vec<(&RuleId, &RuleSourceSpan)> = sources.entries().collect();
    push(
        "sources.count".to_owned(),
        InputValue::Integer(count(placements.len())?),
    )?;
    for (index, (rule, span)) in placements.iter().enumerate() {
        let prefix = format!("sources.{index:03}");
        push(
            format!("{prefix}.artifact"),
            InputValue::Reference(span.artifact().to_string()),
        )?;
        push(
            format!("{prefix}.page"),
            InputValue::Integer(i64::from(span.page())),
        )?;
        let (start, end) = span.paragraph();
        push(
            format!("{prefix}.paragraph_end"),
            InputValue::Integer(i64::try_from(end).unwrap_or(i64::MAX)),
        )?;
        push(
            format!("{prefix}.paragraph_start"),
            InputValue::Integer(i64::try_from(start).unwrap_or(i64::MAX)),
        )?;
        push(
            format!("{prefix}.rule"),
            InputValue::Reference((*rule).as_str().to_owned()),
        )?;
        push(
            format!("{prefix}.source_digest"),
            InputValue::Reference(digest_reference(span.source_digest())),
        )?;
    }
    Ok(())
}

/// Encodes the conflict cases `P2-U6` opened over this rule set.
fn encode_conflicts(
    conflicts: Option<&[ConflictReference]>,
    push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>,
) -> Result<(), AuditError> {
    let Some(conflicts) = conflicts else {
        // Not an empty list: `Unknown` is what the frozen encoding spells for
        // a value that is declared and not known, and an audit whose conflict
        // store nobody read is that.
        return push("conflicts.count".to_owned(), InputValue::Unknown);
    };
    let mut ordered: Vec<&ConflictReference> = conflicts.iter().collect();
    ordered.sort_by(|left, right| {
        left.rule()
            .cmp(right.rule())
            .then_with(|| left.left_connector().cmp(right.left_connector()))
            .then_with(|| left.right_connector().cmp(right.right_connector()))
    });
    push(
        "conflicts.count".to_owned(),
        InputValue::Integer(count(ordered.len())?),
    )?;
    for (index, case) in ordered.iter().enumerate() {
        let prefix = format!("conflicts.{index:03}");
        push(
            format!("{prefix}.left"),
            InputValue::Reference(case.left_connector().to_owned()),
        )?;
        push(
            format!("{prefix}.resolved"),
            InputValue::Integer(i64::from(case.is_resolved())),
        )?;
        push(
            format!("{prefix}.right"),
            InputValue::Reference(case.right_connector().to_owned()),
        )?;
        push(
            format!("{prefix}.rule"),
            InputValue::Reference(case.rule().to_owned()),
        )?;
    }
    Ok(())
}

/// A digest as an identifier-shaped reference.
///
/// `ContentDigest` displays as `sha256:<hex>`, and `:` is one of the three
/// characters the frozen encoding separates fields with. The prefix is dropped
/// here and put back by [`digest_from_reference`], so the value that travels is
/// the sixty-four hex characters and the parser that reads it back is the
/// domain's own.
fn digest_reference(digest: ContentDigest) -> String {
    digest.to_string().replace("sha256:", "")
}

fn digest_from_reference(value: &str) -> Result<ContentDigest, AuditError> {
    format!("sha256:{value}")
        .parse()
        .map_err(|_| AuditError::MalformedEngineInput("source digest"))
}

fn encode_profile(
    profile: &StudentProfile,
    push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>,
) -> Result<(), AuditError> {
    push(
        "profile.admission_year".to_owned(),
        profile
            .admission_year()
            .known()
            .map_or(InputValue::Unknown, |year| {
                InputValue::Integer(i64::from(year.get()))
            }),
    )?;
    push(
        "profile.college".to_owned(),
        reference(profile.college().known().map(InstitutionId::as_str)),
    )?;
    push(
        "profile.degree_mode".to_owned(),
        reference(profile.degree_mode().known().map(|mode| mode.as_str())),
    )?;
    push(
        "profile.department".to_owned(),
        reference(profile.department().known().map(InstitutionId::as_str)),
    )?;
    push(
        "profile.exchange_or_transfer".to_owned(),
        reference(profile.exchange_or_transfer().known().map(|_| "DECLARED")),
    )?;
    push(
        "profile.graduation_standard".to_owned(),
        reference(
            profile
                .graduation_standard()
                .known()
                .map(GraduationStandard::as_str),
        ),
    )?;
    push(
        "profile.university".to_owned(),
        reference(profile.university().known().map(InstitutionId::as_str)),
    )?;

    match profile.additional_majors().known() {
        None => push(
            "profile.additional_majors.count".to_owned(),
            InputValue::Unknown,
        )?,
        Some(list) => {
            push(
                "profile.additional_majors.count".to_owned(),
                InputValue::Integer(count(list.len())?),
            )?;
            let mut names: Vec<&str> = list.iter().map(ProgrammeId::as_str).collect();
            names.sort_unstable();
            for (index, name) in names.iter().enumerate() {
                push(
                    format!("profile.additional_majors.{index:03}"),
                    InputValue::Reference((*name).to_owned()),
                )?;
            }
        }
    }

    match profile.exception_approvals().known() {
        None => push(
            "profile.exception_approvals.count".to_owned(),
            InputValue::Unknown,
        )?,
        Some(approvals) => {
            push(
                "profile.exception_approvals.count".to_owned(),
                InputValue::Integer(count(approvals.len())?),
            )?;
            let mut ordered: Vec<&ApprovalFact> = approvals.iter().collect();
            ordered.sort_by(|left, right| {
                left.rule
                    .as_str()
                    .cmp(right.rule.as_str())
                    .then_with(|| left.issued_at.value().cmp(&right.issued_at.value()))
            });
            for (index, approval) in ordered.iter().enumerate() {
                let prefix = format!("profile.exception_approvals.{index:03}");
                push(
                    format!("{prefix}.authority"),
                    InputValue::Reference(approval.authority.as_str().to_owned()),
                )?;
                push(
                    format!("{prefix}.expires_at"),
                    approval
                        .expires_at
                        .map_or(InputValue::Unknown, |at| InputValue::Integer(at.value())),
                )?;
                push(
                    format!("{prefix}.issued_at"),
                    InputValue::Integer(approval.issued_at.value()),
                )?;
                push(
                    format!("{prefix}.rule"),
                    InputValue::Reference(approval.rule.as_str().to_owned()),
                )?;
            }
        }
    }
    Ok(())
}

fn encode_transcript(
    transcript: &TranscriptSnapshot,
    push: &mut impl FnMut(String, InputValue) -> Result<(), AuditError>,
) -> Result<(), AuditError> {
    push(
        "transcript.count".to_owned(),
        InputValue::Integer(count(transcript.entries().len())?),
    )?;
    for (index, entry) in transcript.entries().iter().enumerate() {
        let prefix = format!("transcript.{index:03}");
        let (admission, credits, reason) = match entry.admission() {
            EntryAdmission::Counted { credits, reason } => {
                ("COUNTED", i64::from(credits.get()), reason)
            }
            EntryAdmission::Excluded { reason } => ("EXCLUDED", 0, reason),
            EntryAdmission::Pending { reason } => ("PENDING", 0, reason),
        };
        push(
            format!("{prefix}.admission"),
            InputValue::Reference(admission.to_owned()),
        )?;
        push(
            format!("{prefix}.area"),
            reference(entry.area().map(AreaId::as_str)),
        )?;
        push(
            format!("{prefix}.attempt"),
            InputValue::Reference(entry.attempt().to_string()),
        )?;
        let mut categories: Vec<&str> = entry
            .categories()
            .iter()
            .map(CreditCategory::as_str)
            .collect();
        categories.sort_unstable();
        push(
            format!("{prefix}.categories.count"),
            InputValue::Integer(count(categories.len())?),
        )?;
        for (position, category) in categories.iter().enumerate() {
            push(
                format!("{prefix}.categories.{position:03}"),
                InputValue::Reference((*category).to_owned()),
            )?;
        }
        push(
            format!("{prefix}.course"),
            InputValue::Reference(entry.course().to_string()),
        )?;
        push(
            format!("{prefix}.course_code"),
            InputValue::Reference(entry.course_code().to_owned()),
        )?;
        push(format!("{prefix}.credits"), InputValue::Integer(credits))?;
        push(
            format!("{prefix}.language"),
            InputValue::Reference(crate::transcript::language_token(entry.language()).to_owned()),
        )?;
        push(
            format!("{prefix}.major"),
            InputValue::Integer(i64::from(entry.is_major())),
        )?;
        push(
            format!("{prefix}.reason"),
            InputValue::Reference(crate::transcript::reason_token(reason).to_owned()),
        )?;
        push(
            format!("{prefix}.record_status"),
            InputValue::Reference(entry.record_status().as_str().to_owned()),
        )?;
        push(
            format!("{prefix}.term"),
            InputValue::Reference(entry.term().canonical_text()),
        )?;
    }

    let readings: Vec<(&String, &GpaReading)> = transcript.readings().collect();
    push(
        "gpa.count".to_owned(),
        InputValue::Integer(count(readings.len())?),
    )?;
    for (index, (scope, reading)) in readings.iter().enumerate() {
        let prefix = format!("gpa.{index:03}");
        push(
            format!("{prefix}.denominator"),
            InputValue::Integer(i64::from(reading.denominator_credits)),
        )?;
        push(
            format!("{prefix}.scope"),
            InputValue::Reference((*scope).clone()),
        )?;
        push(
            format!("{prefix}.weighted_points"),
            InputValue::Decimal(reading.weighted_points),
        )?;
    }
    Ok(())
}

/// Reads one audit's facts back out of frozen inputs.
pub fn decode(inputs: &FrozenInputs) -> Result<AuditFacts, AuditError> {
    let as_of = TimestampMillis::new(required_integer(inputs, "audit.as_of")?);
    let profile = decode_profile(inputs)?;
    let transcript = decode_transcript(inputs)?;
    let sources = decode_sources(inputs)?;
    let conflicts = decode_conflicts(inputs)?;
    let freshness = optional_integer(inputs, "freshness.max_age_seconds")?
        .map(|seconds| {
            u64::try_from(seconds)
                .map(SourceFreshnessPolicy::max_age_seconds)
                .map_err(|_| AuditError::MalformedEngineInput("freshness criterion"))
        })
        .transpose()?;
    Ok(AuditFacts {
        as_of,
        profile,
        transcript,
        sources,
        conflicts,
        freshness,
    })
}

fn decode_sources(inputs: &FrozenInputs) -> Result<RuleSourceIndex, AuditError> {
    let total = index_count(required_integer(inputs, "sources.count")?)?;
    let mut sources = RuleSourceIndex::new();
    for index in 0..total {
        let prefix = format!("sources.{index:03}");
        let artifact: ArtifactId = required_reference(inputs, &format!("{prefix}.artifact"))?
            .parse()
            .map_err(|_| AuditError::MalformedEngineInput("source artifact id"))?;
        let page = u32::try_from(required_integer(inputs, &format!("{prefix}.page"))?)
            .map_err(|_| AuditError::MalformedEngineInput("source page"))?;
        let start = u64::try_from(required_integer(
            inputs,
            &format!("{prefix}.paragraph_start"),
        )?)
        .map_err(|_| AuditError::MalformedEngineInput("paragraph start"))?;
        let end = u64::try_from(required_integer(
            inputs,
            &format!("{prefix}.paragraph_end"),
        )?)
        .map_err(|_| AuditError::MalformedEngineInput("paragraph end"))?;
        sources = sources.with(
            RuleId::new(&required_reference(inputs, &format!("{prefix}.rule"))?)?,
            RuleSourceSpan::new(
                artifact,
                digest_from_reference(&required_reference(
                    inputs,
                    &format!("{prefix}.source_digest"),
                )?)?,
                page,
                start,
                end,
            )?,
        );
    }
    Ok(sources)
}

fn decode_conflicts(inputs: &FrozenInputs) -> Result<Option<Vec<ConflictReference>>, AuditError> {
    let Some(total) = optional_integer(inputs, "conflicts.count")? else {
        return Ok(None);
    };
    let total = index_count(total)?;
    let mut conflicts = Vec::with_capacity(total);
    for index in 0..total {
        let prefix = format!("conflicts.{index:03}");
        conflicts.push(ConflictReference::decoded(
            required_reference(inputs, &format!("{prefix}.rule"))?,
            required_reference(inputs, &format!("{prefix}.left"))?,
            required_reference(inputs, &format!("{prefix}.right"))?,
            required_integer(inputs, &format!("{prefix}.resolved"))? != 0,
        ));
    }
    Ok(Some(conflicts))
}

fn decode_profile(inputs: &FrozenInputs) -> Result<StudentProfile, AuditError> {
    let mut profile = StudentProfile::unrecorded();
    if let Some(value) = optional_reference(inputs, "profile.university")? {
        profile = profile.with_university(InstitutionId::new(&value)?);
    }
    if let Some(value) = optional_reference(inputs, "profile.college")? {
        profile = profile.with_college(InstitutionId::new(&value)?);
    }
    if let Some(value) = optional_reference(inputs, "profile.department")? {
        profile = profile.with_department(InstitutionId::new(&value)?);
    }
    if let Some(value) = optional_integer(inputs, "profile.admission_year")? {
        let year = u16::try_from(value)
            .map_err(|_| AuditError::MalformedEngineInput("profile admission year"))?;
        profile = profile.with_admission_year(AdmissionYear::new(year)?);
    }
    if let Some(value) = optional_reference(inputs, "profile.graduation_standard")? {
        profile = profile.with_graduation_standard(GraduationStandard::new(&value)?);
    }
    if let Some(value) = optional_reference(inputs, "profile.degree_mode")? {
        let mode = DegreeMode::ALL
            .into_iter()
            .find(|mode| mode.as_str() == value)
            .ok_or(AuditError::MalformedEngineInput("profile degree mode"))?;
        profile = profile.with_degree_mode(mode);
    }
    if let Some(total) = optional_integer(inputs, "profile.additional_majors.count")? {
        let total = index_count(total)?;
        let mut programmes = Vec::with_capacity(total);
        for index in 0..total {
            programmes.push(ProgrammeId::new(&required_reference(
                inputs,
                &format!("profile.additional_majors.{index:03}"),
            )?)?);
        }
        profile = profile.with_additional_majors(programmes);
    }
    if optional_reference(inputs, "profile.exchange_or_transfer")?.is_some() {
        profile = profile.with_exchange_or_transfer(ExchangeOrTransfer::Declared);
    }
    if let Some(total) = optional_integer(inputs, "profile.exception_approvals.count")? {
        let total = index_count(total)?;
        let mut approvals = Vec::with_capacity(total);
        for index in 0..total {
            let prefix = format!("profile.exception_approvals.{index:03}");
            approvals.push(ApprovalFact {
                rule: RuleId::new(&required_reference(inputs, &format!("{prefix}.rule"))?)?,
                authority: ApprovalAuthority::new(&required_reference(
                    inputs,
                    &format!("{prefix}.authority"),
                )?)?,
                issued_at: TimestampMillis::new(required_integer(
                    inputs,
                    &format!("{prefix}.issued_at"),
                )?),
                expires_at: optional_integer(inputs, &format!("{prefix}.expires_at"))?
                    .map(TimestampMillis::new),
            });
        }
        profile = profile.with_exception_approvals(approvals);
    }
    Ok(profile)
}

fn decode_transcript(inputs: &FrozenInputs) -> Result<TranscriptSnapshot, AuditError> {
    let total = index_count(required_integer(inputs, "transcript.count")?)?;
    let mut entries = Vec::with_capacity(total);
    for index in 0..total {
        let prefix = format!("transcript.{index:03}");
        let attempt: AttemptId = required_reference(inputs, &format!("{prefix}.attempt"))?
            .parse()
            .map_err(|_| AuditError::MalformedEngineInput("transcript attempt id"))?;
        let course: CourseId = required_reference(inputs, &format!("{prefix}.course"))?
            .parse()
            .map_err(|_| AuditError::MalformedEngineInput("transcript course id"))?;
        let reason = decode_reason(&required_reference(inputs, &format!("{prefix}.reason"))?)?;
        let credits = required_integer(inputs, &format!("{prefix}.credits"))?;
        let credits = u16::try_from(credits)
            .map_err(|_| AuditError::MalformedEngineInput("transcript credits"))?;
        let admission = match required_reference(inputs, &format!("{prefix}.admission"))?.as_str() {
            "COUNTED" => EntryAdmission::Counted {
                credits: CreditAmount::new(credits)?,
                reason,
            },
            "EXCLUDED" => EntryAdmission::Excluded { reason },
            "PENDING" => EntryAdmission::Pending { reason },
            _ => return Err(AuditError::MalformedEngineInput("transcript admission")),
        };
        let categories_total = index_count(required_integer(
            inputs,
            &format!("{prefix}.categories.count"),
        )?)?;
        let mut categories = Vec::with_capacity(categories_total);
        for position in 0..categories_total {
            categories.push(CreditCategory::new(&required_reference(
                inputs,
                &format!("{prefix}.categories.{position:03}"),
            )?)?);
        }
        let record_status = RecordAttemptStatus::parse(&required_reference(
            inputs,
            &format!("{prefix}.record_status"),
        )?)
        .ok_or(AuditError::MalformedEngineInput("transcript record status"))?;
        entries.push(TranscriptEntry::decoded(
            attempt,
            required_reference(inputs, &format!("{prefix}.course_code"))?,
            course,
            TermKey::parse(&required_reference(inputs, &format!("{prefix}.term"))?)?,
            record_status,
            admission,
            categories,
            optional_reference(inputs, &format!("{prefix}.area"))?
                .map(|area| AreaId::new(&area))
                .transpose()?,
            required_integer(inputs, &format!("{prefix}.major"))? != 0,
            decode_language(&required_reference(inputs, &format!("{prefix}.language"))?)?,
        ));
    }

    let readings_total = index_count(required_integer(inputs, "gpa.count")?)?;
    let mut readings: BTreeMap<String, GpaReading> = BTreeMap::new();
    for index in 0..readings_total {
        let prefix = format!("gpa.{index:03}");
        let denominator = required_integer(inputs, &format!("{prefix}.denominator"))?;
        readings.insert(
            required_reference(inputs, &format!("{prefix}.scope"))?,
            GpaReading {
                weighted_points: required_decimal(inputs, &format!("{prefix}.weighted_points"))?,
                denominator_credits: u32::try_from(denominator)
                    .map_err(|_| AuditError::MalformedEngineInput("gpa denominator"))?,
            },
        );
    }

    Ok(TranscriptSnapshot::decoded(entries, readings))
}

fn decode_reason(token: &str) -> Result<DispositionReason, AuditError> {
    DispositionReason::ALL
        .into_iter()
        .find(|reason| crate::transcript::reason_token(*reason) == token)
        .ok_or(AuditError::MalformedEngineInput("disposition reason"))
}

fn decode_language(token: &str) -> Result<LanguageEvidence, AuditError> {
    match token {
        "UNVERIFIED" => Ok(LanguageEvidence::Unverified),
        "KOREAN" => Ok(LanguageEvidence::Verified(InstructionLanguage::Korean)),
        "FOREIGN" => Ok(LanguageEvidence::Verified(InstructionLanguage::Foreign)),
        _ => Err(AuditError::MalformedEngineInput("language evidence")),
    }
}

fn reference(value: Option<&str>) -> InputValue {
    value.map_or(InputValue::Unknown, |value| {
        InputValue::Reference(value.to_owned())
    })
}

fn count(value: usize) -> Result<i64, AuditError> {
    i64::try_from(value).map_err(|_| AuditError::MalformedEngineInput("list length"))
}

fn index_count(value: i64) -> Result<usize, AuditError> {
    usize::try_from(value).map_err(|_| AuditError::MalformedEngineInput("list length"))
}

fn value_of<'inputs>(
    inputs: &'inputs FrozenInputs,
    key: &str,
) -> Result<&'inputs InputValue, AuditError> {
    let key = InputKey::new(key)?;
    inputs
        .get(&key)
        .ok_or(AuditError::MissingEngineInput("frozen input"))
}

fn required_integer(inputs: &FrozenInputs, key: &str) -> Result<i64, AuditError> {
    match value_of(inputs, key)? {
        InputValue::Integer(value) => Ok(*value),
        _ => Err(AuditError::MalformedEngineInput("expected an integer")),
    }
}

fn optional_integer(inputs: &FrozenInputs, key: &str) -> Result<Option<i64>, AuditError> {
    match value_of(inputs, key)? {
        InputValue::Integer(value) => Ok(Some(*value)),
        InputValue::Unknown => Ok(None),
        _ => Err(AuditError::MalformedEngineInput("expected an integer")),
    }
}

fn required_reference(inputs: &FrozenInputs, key: &str) -> Result<String, AuditError> {
    match value_of(inputs, key)? {
        InputValue::Reference(value) => Ok(value.clone()),
        _ => Err(AuditError::MalformedEngineInput("expected a reference")),
    }
}

fn optional_reference(inputs: &FrozenInputs, key: &str) -> Result<Option<String>, AuditError> {
    match value_of(inputs, key)? {
        InputValue::Reference(value) => Ok(Some(value.clone())),
        InputValue::Unknown => Ok(None),
        _ => Err(AuditError::MalformedEngineInput("expected a reference")),
    }
}

fn required_decimal(inputs: &FrozenInputs, key: &str) -> Result<Decimal, AuditError> {
    match value_of(inputs, key)? {
        InputValue::Decimal(value) => Ok(*value),
        _ => Err(AuditError::MalformedEngineInput("expected a decimal")),
    }
}

/// The frozen-input keys one transcript entry occupies.
///
/// A proof leaf declares the keys it read, and `ProofNode::validate` refuses a
/// key the frozen inputs do not carry, so this list and [`encode`] are the same
/// list written once. It is derived from the index rather than from the entry,
/// which is what lets a leaf name the keys of an attempt it *excluded* as well
/// as the ones it counted.
#[must_use]
pub fn entry_keys(index: usize) -> Vec<String> {
    let prefix = format!("transcript.{index:03}");
    vec![
        format!("{prefix}.admission"),
        format!("{prefix}.attempt"),
        format!("{prefix}.course"),
        format!("{prefix}.credits"),
    ]
}
