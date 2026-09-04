//! `P2-U3`'s named acceptance evidence, less the compile failures and the
//! source scans.
//!
//! The absences -- that a plan has no route into an audit, that a determinate
//! verdict has no constructor taking two witnesses, that a leaf has no shorter
//! form -- are in `tests/compile_fail/`, because a running test cannot observe
//! a route that does not exist. `tests/audit_scans.rs` holds the halves that
//! read the specification's own text back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`.
//!
//! Every audit below is evaluated through frozen inputs, so the values a test
//! asserts on went through the same encoding a golden fixture does.

mod support;

use std::collections::BTreeSet;

use academic_domain::engines::{EngineVersion, ProofStatus};
use academic_record::views::DispositionReason;
use academic_requirement::{Measure, OpenGate as RuleGate, RuleId, RuleSet, RuleSetLedger};

use academic_audit::{
    AttemptUsage, AuditError, CommonRuleExamples, CourseFactsIndex, CreditVerdict, DegreeAudit,
    DegreeVerdict, EntryAdmission, EquivalencyDecision, GraduationAuditEngine, GraduationOutcome,
    MissingCheck, OpenGate, PlanAnnotatedView, PlannedCoursework, ProfileField, RuleSetScope,
    RuleSourceIndex, Selection, SourceFreshnessPolicy, StudentProfile, TranscriptSnapshot, encode,
    select, verdict::ConflictReference,
};
use support::{
    FRESHNESS, STALE_FRESHNESS, TestResult, audit_facts, catalog, course, profile, scope, sources,
    sources_missing,
};

/// Builds one audit end to end: select, freeze, evaluate.
fn audit(
    rules: &RuleSet,
    transcript: TranscriptSnapshot,
    sources: RuleSourceIndex,
    conflicts: Vec<ConflictReference>,
    freshness: Option<SourceFreshnessPolicy>,
) -> Result<DegreeAudit, Box<dyn std::error::Error>> {
    audit_for(
        &profile()?,
        rules,
        transcript,
        sources,
        conflicts,
        freshness,
    )
}

fn audit_for(
    profile: &StudentProfile,
    rules: &RuleSet,
    transcript: TranscriptSnapshot,
    sources: RuleSourceIndex,
    conflicts: Vec<ConflictReference>,
    freshness: Option<SourceFreshnessPolicy>,
) -> Result<DegreeAudit, Box<dyn std::error::Error>> {
    let selection = select(profile, &catalog(rules)?);
    let selected = selection
        .selected()
        .ok_or("the fixture profile selected no rule set")?
        .clone();
    let engine = GraduationAuditEngine::new(selected, EngineVersion::MIN);
    let mut facts = audit_facts(transcript, sources, conflicts, freshness)?;
    facts.profile = profile.clone();
    Ok(DegreeAudit::evaluate(&engine, &encode(&facts)?)?)
}

fn status_of(audit: &DegreeAudit, rule: &str) -> Option<ProofStatus> {
    audit
        .nodes()
        .iter()
        .find(|node| node.leaf().rule().as_str() == rule)
        .map(|node| node.leaf().status())
}

// ---------------------------------------------------------------------------
// selector_dimension_matrix -- REQ-11-002
// ---------------------------------------------------------------------------

/// One dimension varied at a time, over a catalogue that really discriminates.
///
/// Section 11.1's yaml declares a scope field for six of the sentence's eight
/// inputs. For each of those six the catalogue below holds two entries that
/// differ **only** in that dimension, and the profile has to pick the one that
/// matches; changing the profile's value to a third one has to pick neither.
/// For the two the yaml declares no field for, the assertion is the other one:
/// varying them removes no candidate, and omitting them is `INDETERMINATE`.
#[test]
fn selector_dimension_matrix() -> TestResult {
    use academic_audit::{
        CatalogEntry, DegreeMode, GraduationStandard, InstitutionId, RuleSetCatalog, RuleSetScope,
        SelectorDimension,
    };
    use academic_requirement::AdmissionYear;

    let rules = support::baseline_rules()?;
    let base = scope()?;
    let other = InstitutionId::new("OTHER")?;

    // The six scope fields, each varied on its own.
    let variants: Vec<(SelectorDimension, RuleSetScope)> = vec![
        (
            SelectorDimension::University,
            RuleSetScope::new(
                other.clone(),
                support::college()?,
                support::department()?,
                support::admission_year()?,
                support::standard()?,
                support::standard()?,
                DegreeMode::SingleMajor,
            )?,
        ),
        (
            SelectorDimension::College,
            RuleSetScope::new(
                support::university()?,
                other.clone(),
                support::department()?,
                support::admission_year()?,
                support::standard()?,
                support::standard()?,
                DegreeMode::SingleMajor,
            )?,
        ),
        (
            SelectorDimension::Department,
            RuleSetScope::new(
                support::university()?,
                support::college()?,
                other.clone(),
                support::admission_year()?,
                support::standard()?,
                support::standard()?,
                DegreeMode::SingleMajor,
            )?,
        ),
        (
            SelectorDimension::AdmissionYear,
            RuleSetScope::new(
                support::university()?,
                support::college()?,
                support::department()?,
                AdmissionYear::new(2025)?,
                support::standard()?,
                support::standard()?,
                DegreeMode::SingleMajor,
            )?,
        ),
        (
            SelectorDimension::GraduationStandard,
            RuleSetScope::new(
                support::university()?,
                support::college()?,
                support::department()?,
                support::admission_year()?,
                GraduationStandard::new("2024")?,
                GraduationStandard::new("2024")?,
                DegreeMode::SingleMajor,
            )?,
        ),
        (
            SelectorDimension::MajorMode,
            RuleSetScope::new(
                support::university()?,
                support::college()?,
                support::department()?,
                support::admission_year()?,
                support::standard()?,
                support::standard()?,
                DegreeMode::DoubleMajor,
            )?,
        ),
    ];

    let mut narrowing_seen = BTreeSet::new();
    for (dimension, rival) in &variants {
        assert!(
            dimension.narrows_the_catalogue(),
            "{dimension:?} is not one of the yaml's scope fields"
        );
        narrowing_seen.insert(*dimension);
        let catalogue = RuleSetCatalog::new()
            .with(CatalogEntry::new(base.clone(), rules.clone()))
            .with(CatalogEntry::new(rival.clone(), rules.clone()));
        let selection = select(&profile()?, &catalogue);
        let selected = selection
            .selected()
            .ok_or_else(|| format!("varying {dimension:?} lost the matching set"))?;
        assert_eq!(
            selected.scope(),
            &base,
            "varying {dimension:?} selected the wrong scope"
        );

        // The rival alone covers nothing this profile is.
        let only_rival =
            RuleSetCatalog::new().with(CatalogEntry::new(rival.clone(), rules.clone()));
        assert!(
            matches!(
                select(&profile()?, &only_rival).missing().first(),
                Some(MissingCheck::NoRuleSetCovers { .. })
            ),
            "the {dimension:?} rival covered a profile it does not scope"
        );
    }

    // Every dimension the split calls narrowing was exercised above, and every
    // one it does not is exercised below. The two halves together are section
    // 11.1's eight, and neither list is written twice.
    let narrowing: BTreeSet<SelectorDimension> = SelectorDimension::ALL
        .into_iter()
        .filter(|dimension| dimension.narrows_the_catalogue())
        .collect();
    assert_eq!(narrowing, narrowing_seen);

    for dimension in SelectorDimension::ALL {
        if dimension.narrows_the_catalogue() {
            continue;
        }
        // Varying it removes no candidate: the same set is still selected with
        // a different recorded value.
        let varied = match dimension {
            SelectorDimension::ExchangeOrTransfer => profile()?,
            _ => profile()?.with_exception_approvals(Vec::new()),
        };
        let selection = select(&varied, &catalog(&rules)?);
        assert!(
            selection.selected().is_some(),
            "{dimension:?} narrowed the catalogue"
        );

        // Omitting it is INDETERMINATE, so it is required even though it
        // narrows nothing.
        let omitted = omit_dimension(dimension)?;
        assert!(
            select(&omitted, &catalog(&rules)?).selected().is_none(),
            "omitting {dimension:?} still selected a set"
        );
    }
    Ok(())
}

/// A profile with every field but the ones under `dimension` recorded.
fn omit_dimension(
    dimension: academic_audit::SelectorDimension,
) -> Result<StudentProfile, Box<dyn std::error::Error>> {
    let mut profile = StudentProfile::unrecorded();
    for field in ProfileField::ALL {
        if field.dimension() == dimension {
            continue;
        }
        profile = record(profile, field)?;
    }
    Ok(profile)
}

/// A profile with every field but `omitted` recorded.
fn omit_field(omitted: ProfileField) -> Result<StudentProfile, Box<dyn std::error::Error>> {
    let mut profile = StudentProfile::unrecorded();
    for field in ProfileField::ALL {
        if field == omitted {
            continue;
        }
        profile = record(profile, field)?;
    }
    Ok(profile)
}

fn record(
    profile: StudentProfile,
    field: ProfileField,
) -> Result<StudentProfile, Box<dyn std::error::Error>> {
    use academic_audit::{DegreeMode, ExchangeOrTransfer};
    Ok(match field {
        ProfileField::University => profile.with_university(support::university()?),
        ProfileField::College => profile.with_college(support::college()?),
        ProfileField::Department => profile.with_department(support::department()?),
        ProfileField::AdmissionYear => profile.with_admission_year(support::admission_year()?),
        ProfileField::GraduationStandard => profile.with_graduation_standard(support::standard()?),
        ProfileField::DegreeMode => profile.with_degree_mode(DegreeMode::SingleMajor),
        ProfileField::AdditionalMajors => profile.with_additional_majors(Vec::new()),
        ProfileField::ExchangeOrTransferCredits => {
            profile.with_exchange_or_transfer(ExchangeOrTransfer::Declared)
        }
        ProfileField::ExceptionApprovals => profile.with_exception_approvals(Vec::new()),
    })
}

// ---------------------------------------------------------------------------
// selector_fail_closed -- REQ-11-003
// ---------------------------------------------------------------------------

/// Every omitted field, and two sets that compete, each refused by name.
#[test]
fn selector_fail_closed() -> TestResult {
    let rules = support::baseline_rules()?;

    // Every field, omitted on its own.
    for field in ProfileField::ALL {
        let selection = select(&omit_field(field)?, &catalog(&rules)?);
        assert!(
            selection.selected().is_none(),
            "omitting {field:?} still selected a rule set"
        );
        let named = selection.missing().iter().any(|check| {
            matches!(check, MissingCheck::ProfileField { field: named, gate }
                if *named == field && *gate == field.gate())
        });
        assert!(
            named,
            "omitting {field:?} did not name it, or named the wrong section 38 cell: {:?}",
            selection.missing()
        );
        assert_eq!(
            selection.missing().len(),
            1,
            "omitting one field reported more than one check"
        );
        // The action is specific to the dimension rather than a generic
        // sentence, which is what section 11.1's 필요한 확인 항목 asks for.
        let action = selection
            .missing()
            .first()
            .map(MissingCheck::action)
            .unwrap_or_default();
        assert!(
            action.contains("record"),
            "the action for {field:?} does not say what to do: {action}"
        );
    }

    // Every field omitted at once reports every field, not the first.
    let all_missing = select(&StudentProfile::unrecorded(), &catalog(&rules)?);
    assert_eq!(all_missing.missing().len(), ProfileField::ALL.len());

    // Two sets that both cover the profile: neither is chosen, both are named.
    use academic_audit::{CatalogEntry, RuleSetCatalog};
    let competing = RuleSetCatalog::new()
        .with(CatalogEntry::new(scope()?, rules.clone()))
        .with(CatalogEntry::new(scope()?, support::revised_rules()?));
    let selection = select(&profile()?, &competing);
    assert!(selection.selected().is_none());
    let versions = match selection.missing().first() {
        Some(MissingCheck::CompetingRuleSets { versions }) => versions.clone(),
        other => return Err(format!("two competing sets reported {other:?}").into()),
    };
    assert_eq!(versions.len(), 2, "both competing versions must be named");

    // Nothing covers the profile at all.
    let empty = select(&profile()?, &RuleSetCatalog::new());
    assert!(matches!(
        empty.missing().first(),
        Some(MissingCheck::NoRuleSetCovers { .. })
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// mixed_proof_tree -- REQ-11-020
// ---------------------------------------------------------------------------

/// One tree carrying every reading section 11.3's example carries at once.
///
/// Section 11.3's tree has a root that reads `INDETERMINATE` over children that
/// pass, fall short, are not satisfied, and cannot be evaluated. This asserts
/// exactly that shape, and additionally that a `CONFLICT` leaf can stand beside
/// them -- which section 11.4 requires to be possible and section 11.3's
/// example does not print.
#[test]
fn mixed_proof_tree() -> TestResult {
    let rules = support::mixed_rules()?;
    let audit = audit(
        &rules,
        support::transcript_with_conflicting_records()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;

    let present: BTreeSet<ProofStatus> = audit
        .walk()
        .iter()
        .map(|node| node.leaf().status())
        .collect();
    for status in ProofStatus::ALL {
        assert!(
            present.contains(&status),
            "the mixed tree carries no {status} leaf; it carries {present:?}"
        );
    }

    // The root is INDETERMINATE and says why, one entry per outstanding thing.
    assert_eq!(audit.verdict().as_str(), "INDETERMINATE");
    assert!(!audit.verdict().missing().is_empty());

    // A `CONFLICT` anywhere makes the root `CONFLICT`, and nothing derived is
    // published under it.
    assert_eq!(audit.root_status(), ProofStatus::Conflict);
    assert!(
        audit.outcome().result.values.is_empty(),
        "a CONFLICT result published a derived value"
    );

    // The unknown leaf names its section 38 cell rather than a number.
    let thesis = audit
        .walk()
        .into_iter()
        .find(|node| node.leaf().rule().as_str() == "thesis_research")
        .ok_or("the mixed set has no thesis rule")?;
    assert_eq!(thesis.leaf().status(), ProofStatus::Unknown);
    assert_eq!(thesis.leaf().open_gate(), Some(OpenGate::RuleThesisScope));

    // Planned-only work reads NOT_SATISFIED, and the annotation says which.
    let plan = PlannedCoursework::from_scenario(&support::plan()?);
    let view = PlanAnnotatedView::new(&audit, &plan);
    assert_eq!(
        view.planned_only(),
        vec![support::PLANNED_COURSE_CODE],
        "the plan's course is not labelled planned-only"
    );
    assert_eq!(
        status_of(&audit, "required_course_set"),
        Some(ProofStatus::NotSatisfied)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// proof_leaf_completeness -- REQ-11-021, INV-C-001 graduation half
// ---------------------------------------------------------------------------

/// Every node below the root carries all four of section 11.3's parts, and
/// every one of them resolves.
#[test]
fn proof_leaf_completeness() -> TestResult {
    let rules = support::mixed_rules()?;
    let complete = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;

    let published: BTreeSet<String> = rules
        .rules()
        .map(|(rule, _)| rule.as_str().to_owned())
        .collect();
    let attempts: BTreeSet<_> = complete
        .transcript()
        .entries()
        .iter()
        .map(|entry| entry.attempt())
        .collect();

    let mut with_attempts = 0_usize;
    let mut with_equivalency = 0_usize;
    let nodes = complete.walk();
    assert!(
        nodes.len() >= published.len(),
        "the tree has fewer nodes than the set has rules"
    );
    for node in &nodes {
        let leaf = node.leaf();
        assert!(
            leaf.is_complete(),
            "{:?} is not a complete leaf",
            leaf.rule()
        );

        // 1. The applied rule ID resolves to a published rule.
        assert!(
            published.contains(leaf.rule().as_str()),
            "{} is not a rule of the published set",
            leaf.rule()
        );

        // 2. The source page and paragraph are a real span.
        assert!(leaf.source().page() > 0);
        let (start, end) = leaf.source().paragraph();
        assert!(start < end, "the paragraph span is empty");
        assert_eq!(
            leaf.source().locators().len(),
            2,
            "a leaf carries a page and a paragraph, not one of them"
        );

        // 3. The attempts used resolve to attempts of the bound transcript, or
        //    the reason no attempt was used is stated.
        match leaf.attempts() {
            AttemptUsage::Used(used) => {
                assert!(!used.is_empty());
                for attempt in used {
                    assert!(
                        attempts.contains(attempt),
                        "{attempt} is not an attempt of the bound transcript"
                    );
                }
                with_attempts += 1;
            }
            AttemptUsage::NoneUsed(reason) => {
                assert!(!reason.as_str().is_empty());
            }
        }

        // 4. The equivalency decision is present either way.
        match leaf.equivalency() {
            EquivalencyDecision::Applied(applied) => {
                assert!(!applied.is_empty());
                for rule in applied {
                    assert!(published.contains(rule.as_str()));
                }
                with_equivalency += 1;
            }
            EquivalencyDecision::NoneApplied => {}
        }
    }

    // Without these two the walk above would pass over a tree in which nothing
    // ever named an attempt and nothing ever applied a substitution -- the
    // vacuous shape this repository has found in ten consecutive tasks.
    assert!(
        with_attempts > 0,
        "no leaf in the tree named an attempt at all"
    );
    assert!(
        with_equivalency > 0,
        "no leaf in the tree applied an equivalency at all"
    );

    // A rule the source index does not place produces no leaf and is reported.
    let partial = audit(
        &rules,
        support::transcript()?,
        sources_missing(&rules, "total_credits")?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    assert!(
        partial
            .walk()
            .iter()
            .all(|node| node.leaf().rule().as_str() != "total_credits"),
        "a rule with no recorded page produced a leaf"
    );
    assert!(
        partial
            .unevaluated()
            .iter()
            .any(|rule| rule.as_str() == "total_credits")
    );
    assert!(partial.outcome().result.is_partial_failure());
    assert!(partial.verdict().missing().iter().any(|check| matches!(
        check,
        MissingCheck::RuleSourceSpanAbsent { rule } if rule.as_str() == "total_credits"
    )));
    Ok(())
}

// ---------------------------------------------------------------------------
// credit_explanation_drilldown -- REQ-11-022
// ---------------------------------------------------------------------------

/// Opening a credit number reaches every attempt, included or excluded.
#[test]
fn credit_explanation_drilldown() -> TestResult {
    let rules = support::baseline_rules()?;
    let audit = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;

    let transcript: Vec<_> = audit
        .transcript()
        .entries()
        .iter()
        .map(|entry| entry.attempt())
        .collect();
    assert!(transcript.len() >= 8, "the corpus transcript shrank");

    let explanations = audit.credit_explanations();
    assert!(
        explanations.len() >= 2,
        "the baseline set has two credit floors and produced {} drilldowns",
        explanations.len()
    );

    for explanation in explanations {
        // Total over the transcript: one line per attempt, none missing and
        // none repeated. A drilldown that listed only the included attempts
        // would answer the opposite of the question a user asks.
        let named: Vec<_> = explanation
            .lines()
            .iter()
            .map(academic_audit::CreditLine::attempt)
            .collect();
        assert_eq!(named, transcript, "the drilldown is not total");

        let included: u32 = explanation.included_credits();
        let measure = audit
            .nodes()
            .iter()
            .find(|node| node.leaf().rule() == explanation.rule())
            .and_then(|node| node.leaf().measure());
        match measure {
            Some(Measure::Credits { attained, .. }) => assert_eq!(
                included,
                attained,
                "the drilldown for {} does not add up to its own number",
                explanation.rule()
            ),
            other => return Err(format!("a credit rule measured {other:?}").into()),
        }

        // Every line says why, and every exclusion names the record engine's
        // own reason or this crate's one category reason.
        for line in explanation.lines() {
            let reason = line.verdict().reason_text();
            assert!(!reason.is_empty());
            assert!(explanation.source().page() > 0);
        }
        // Both sides are present, so the partition is a real one.
        assert!(
            explanation
                .lines()
                .iter()
                .any(|line| line.verdict().is_included())
        );
        assert!(
            explanation
                .lines()
                .iter()
                .any(|line| !line.verdict().is_included())
        );
    }

    // The failed attempt is excluded for the record engine's reason, not this
    // crate's.
    let major = audit
        .credit_explanation(&RuleId::new("cse_major_total")?)
        .ok_or("no drilldown for cse_major_total")?;
    let failed = major
        .lines()
        .iter()
        .find(|line| line.course_code() == academic_record::corpus::COURSE_FAILED)
        .ok_or("the failed attempt is not in the drilldown")?;
    assert_eq!(
        failed.verdict(),
        CreditVerdict::NoCreditEarned {
            reason: DispositionReason::FailedInDenominator
        }
    );

    // An attempt that earned credit under no category this rule counts is
    // excluded for this crate's one reason.
    let outside = major
        .lines()
        .iter()
        .find(|line| line.course_code() == academic_record::corpus::COURSE_ADDITIONAL)
        .ok_or("the additional-programme attempt is not in the drilldown")?;
    assert_eq!(outside.verdict(), CreditVerdict::OutsideCategory);
    Ok(())
}

// ---------------------------------------------------------------------------
// unknown_profile_audit -- REQ-03-015
// ---------------------------------------------------------------------------

/// Section 3's current profile audits to `INDETERMINATE` with the whole list.
#[test]
fn unknown_profile_audit() -> TestResult {
    let rules = support::baseline_rules()?;
    let selection = select(&StudentProfile::unrecorded(), &catalog(&rules)?);

    assert!(
        selection.selected().is_none(),
        "an entirely unrecorded profile selected a rule set"
    );
    assert!(matches!(selection, Selection::Indeterminate(_)));

    let named: BTreeSet<ProfileField> = selection
        .missing()
        .iter()
        .filter_map(|check| match check {
            MissingCheck::ProfileField { field, .. } => Some(*field),
            _ => None,
        })
        .collect();
    let expected: BTreeSet<ProfileField> = ProfileField::ALL.into_iter().collect();
    assert_eq!(named, expected, "not every unrecorded field was reported");

    // Each field that section 38 asks the user for names its cell.
    for check in selection.missing() {
        if let MissingCheck::ProfileField { field, gate } = check {
            assert_eq!(*gate, field.gate());
            if let Some(gate) = gate {
                assert!(gate.identifier().starts_with("GATE-38-"));
                assert!(!gate.statement().is_empty());
            }
        }
    }

    // The five section 38.1 cells this task leaves open all appear.
    let gates: BTreeSet<OpenGate> = selection
        .missing()
        .iter()
        .filter_map(|check| match check {
            MissingCheck::ProfileField { gate, .. } => *gate,
            _ => None,
        })
        .collect();
    for gate in [
        OpenGate::ProfileAdmissionYear,
        OpenGate::ProfileGraduationStandard,
        OpenGate::ProfileDegreeMode,
        OpenGate::ProfileAdditionalMajor,
        OpenGate::ProfileExchangeOrTransfer,
    ] {
        assert!(
            gates.contains(&gate),
            "{} was not reported",
            gate.identifier()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// missing_admission_no_remaining -- REQ-11-030
// ---------------------------------------------------------------------------

/// With no admission year there is no personal audit, and the public example
/// carries no personal number.
#[test]
fn missing_admission_no_remaining() -> TestResult {
    let rules = support::baseline_rules()?;
    let selection = select(&omit_field(ProfileField::AdmissionYear)?, &catalog(&rules)?);

    // No set is selected, so there is no audit, so there is no remaining figure.
    assert!(selection.selected().is_none());
    assert!(selection.missing().iter().any(|check| matches!(
        check,
        MissingCheck::ProfileField {
            field: ProfileField::AdmissionYear,
            gate: Some(OpenGate::ProfileAdmissionYear)
        }
    )));

    // The published common facts are still readable, and they are thresholds
    // only: no attained figure and no remaining figure exists on the value.
    let examples = CommonRuleExamples::of(&rules)?;
    assert_eq!(CommonRuleExamples::LABEL, "NOT_PERSONALIZED");
    let thresholds: Vec<u16> = examples
        .floors()
        .iter()
        .map(academic_audit::CommonRuleExample::threshold)
        .collect();
    assert!(
        thresholds.contains(&130),
        "the public floor is not readable: {thresholds:?}"
    );

    // Nothing in the outstanding list quotes a threshold as though it were a
    // personal figure.
    for check in selection.missing() {
        let action = check.action();
        assert!(
            !action.contains("130"),
            "an outstanding check quoted a credit threshold: {action}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// plan_excluded_from_actual_audit -- REQ-22-017 shape, section 11.3
// ---------------------------------------------------------------------------

/// A plan changes no measure, no status and no byte of the audit.
#[test]
fn plan_excluded_from_actual_audit() -> TestResult {
    let rules = support::baseline_rules()?;
    let without = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;

    // The same evaluation, run again while a plan exists. `DegreeAudit::evaluate`
    // has no plan parameter, so "with a plan" is a statement about the world
    // and not about the call -- which is the point.
    let with = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    let plan = PlannedCoursework::from_scenario(&support::plan()?);
    assert!(!plan.is_empty(), "the fixture plan is empty");

    assert_eq!(
        without.outcome(),
        with.outcome(),
        "the audit is not the same evaluation"
    );
    assert_eq!(without.binding(), with.binding());

    // The annotation finds the planned course. Without this half the assertion
    // above would pass on a plan that named nothing.
    let view = PlanAnnotatedView::new(&with, &plan);
    assert_eq!(view.planned_only(), vec![support::PLANNED_COURSE_CODE]);

    // A course that is planned *and* already earned is not planned-only: a
    // completed attempt is not a proposal.
    use academic_record::term::{Semester, TermKey};
    let both = PlannedCoursework::from_scenario(&academic_record::plan::PlanScenario::new(
        support::entity(6_002)?,
        "already done",
        vec![academic_record::plan::PlanScenarioChoice::new(
            academic_record::corpus::COURSE_SHARED,
            TermKey::new(2027, Semester::Spring)?,
        )?],
    )?);
    assert!(
        PlanAnnotatedView::new(&with, &both)
            .planned_only()
            .is_empty(),
        "a completed course was labelled planned-only"
    );

    // And the record layer's own exclusion is what keeps the registered
    // attempt out of every measure: `P2-U4` reports it as earning nothing.
    let registered = with
        .transcript()
        .entries()
        .iter()
        .find(|entry| entry.course_code() == academic_record::corpus::COURSE_REGISTERED)
        .ok_or("the registered attempt is not in the transcript")?;
    assert_eq!(
        registered.admission(),
        EntryAdmission::Excluded {
            reason: DispositionReason::NotSettled
        }
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// historic_audit_replay -- REQ-37-018, REQ-34-060
// ---------------------------------------------------------------------------

/// A past audit replays byte-identically under the rule hash it ran on.
#[test]
fn historic_audit_replay() -> TestResult {
    let first = support::baseline_rules()?;
    let second = support::revised_rules()?;

    let mut ledger = RuleSetLedger::new();
    ledger.publish(first.clone())?;
    let recorded = audit(
        &first,
        support::transcript()?,
        sources(&first)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    let recorded_hash = recorded.binding().rule_set_hash();

    // The curriculum changes: a second version is published beside the first.
    ledger.publish(second.clone())?;
    assert_eq!(ledger.versions().len(), 2);

    // The replay walks the ledger by the hash the recorded audit carries.
    let replayed_set = ledger
        .by_hash(recorded_hash.digest())
        .ok_or("the recorded rule hash is no longer in the ledger")?
        .clone();
    let replayed = audit(
        &replayed_set,
        support::transcript()?,
        sources(&replayed_set)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;

    assert_eq!(
        recorded.outcome().canonical_bytes(
            academic_audit::GRADUATION_ENGINE_ID,
            recorded_hash,
            EngineVersion::MIN,
            &encode(&audit_facts(
                support::transcript()?,
                sources(&first)?,
                Vec::new(),
                Some(FRESHNESS)
            )?)?
        ),
        replayed.outcome().canonical_bytes(
            academic_audit::GRADUATION_ENGINE_ID,
            recorded_hash,
            EngineVersion::MIN,
            &encode(&audit_facts(
                support::transcript()?,
                sources(&replayed_set)?,
                Vec::new(),
                Some(FRESHNESS)
            )?)?
        ),
        "the replay does not reproduce the recorded audit"
    );
    assert_eq!(recorded.binding(), replayed.binding());

    // The latest audit applies the new version, and it is a different audit.
    let latest = audit(
        &second,
        support::transcript()?,
        sources(&second)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    assert_ne!(
        latest.binding().rule_set_hash(),
        recorded_hash,
        "the published version did not move the hash"
    );

    // The assertion above is **not** the one its old message named. `first` and
    // `second` differ in the version number, the supersession and the rule list
    // as well as in the threshold, so it passed for years against a
    // `rule_set_hash` that rendered no rule body at all: two sets differing
    // only in a credit threshold hashed identically, produced a byte-identical
    // `AuditInputBinding`, and answered 졸업 불가 and 졸업 가능.
    //
    // So the threshold is varied on its own here, and what the hash is *for* is
    // driven with it: a replay presenting the strict set's recorded hash
    // against the lax set's rules is refused rather than answered.
    let strict = support::credit_floor_rules(130)?;
    let lax = support::credit_floor_rules(12)?;
    assert_eq!(strict.version(), lax.version());
    assert_eq!(strict.supersedes(), lax.supersedes());
    assert_eq!(strict.rules().count(), lax.rules().count());
    assert_eq!(
        strict
            .rules()
            .map(|(rule, _)| rule.clone())
            .collect::<Vec<_>>(),
        lax.rules()
            .map(|(rule, _)| rule.clone())
            .collect::<Vec<_>>(),
        "the two sets must differ in the threshold and in nothing else"
    );
    assert_ne!(
        strict.rule_set_hash(),
        lax.rule_set_hash(),
        "two sets differing only in a credit threshold hashed the same"
    );

    let strict_audit = audit(
        &strict,
        support::transcript()?,
        sources(&strict)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    let lax_audit = audit(
        &lax,
        support::transcript()?,
        sources(&lax)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    // The conclusions really are opposite, so the refusal below is
    // load-bearing rather than a formality about two digests.
    assert_eq!(
        strict_audit
            .verdict()
            .determinate()
            .map(|verdict| verdict.outcome()),
        Some(GraduationOutcome::NotPossible)
    );
    assert_eq!(
        lax_audit
            .verdict()
            .determinate()
            .map(|verdict| verdict.outcome()),
        Some(GraduationOutcome::Possible)
    );
    assert_ne!(
        strict_audit.binding(),
        lax_audit.binding(),
        "two audits reaching opposite verdicts shared an input binding"
    );

    let selection = select(&profile()?, &catalog(&lax)?);
    let lax_engine = GraduationAuditEngine::new(
        selection
            .selected()
            .ok_or("the lax set was not selected")?
            .clone(),
        EngineVersion::MIN,
    );
    let lax_inputs = encode(&audit_facts(
        support::transcript()?,
        sources(&lax)?,
        Vec::new(),
        Some(FRESHNESS),
    )?)?;
    assert!(
        matches!(
            lax_engine.evaluate_audit(&lax_inputs, strict_audit.binding().rule_set_hash()),
            Err(AuditError::RuleSetHashMismatch)
        ),
        "a replay under a foreign rule-set hash was answered instead of refused"
    );
    // And the same engine under its own hash is answered, so the refusal is
    // about the hash rather than about the inputs.
    assert!(
        lax_engine
            .evaluate_audit(&lax_inputs, lax_engine.rule_set_hash())
            .is_ok()
    );
    assert_ne!(
        latest.outcome().explanation_snapshot,
        recorded.outcome().explanation_snapshot,
        "the new version produced the old explanation"
    );
    // Without this the comparison above would pass on two audits that merely
    // differ; the point is that the *conclusion* moved.
    assert_eq!(
        status_of(&latest, "total_credits"),
        Some(ProofStatus::Satisfied)
    );
    assert_eq!(
        status_of(&recorded, "total_credits"),
        Some(ProofStatus::Needs)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// determinate_three_gate -- REQ-11-029
// ---------------------------------------------------------------------------

/// All three gates, then each falsified on its own.
#[test]
fn determinate_three_gate() -> TestResult {
    let rules = support::satisfiable_rules()?;
    let all_three = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    let determinate = match all_three.verdict() {
        DegreeVerdict::Determinate(verdict) => *verdict,
        DegreeVerdict::Indeterminate(verdict) => {
            return Err(format!(
                "all three gates hold and the audit is {:?}",
                verdict.missing()
            )
            .into());
        }
    };
    assert_eq!(determinate.outcome(), GraduationOutcome::Possible);
    assert!(determinate.coverage().rules_covered() > 0);
    assert!(determinate.freshness().age_seconds() > 0);

    // Gate one: a rule the source index does not place is not evaluated.
    let no_coverage = audit(
        &rules,
        support::transcript()?,
        sources_missing(&rules, "total_credits")?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    assert!(no_coverage.verdict().determinate().is_none());
    assert!(
        no_coverage
            .verdict()
            .missing()
            .iter()
            .any(|check| matches!(check, MissingCheck::RuleSourceSpanAbsent { .. }))
    );

    // Gate two: an unresolved conflict case on a rule of this set.
    let contested = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        vec![support::unresolved_conflict()?],
        Some(FRESHNESS),
    )?;
    assert!(contested.verdict().determinate().is_none());
    assert!(
        contested
            .verdict()
            .missing()
            .iter()
            .any(|check| matches!(check, MissingCheck::UnresolvedSourceConflict { .. }))
    );

    // Gate three, twice: no criterion at all, and a criterion the source fails.
    let no_policy = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        None,
    )?;
    assert!(no_policy.verdict().determinate().is_none());
    assert!(
        no_policy
            .verdict()
            .missing()
            .contains(&MissingCheck::SourceFreshnessPolicyAbsent)
    );

    let stale = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(STALE_FRESHNESS),
    )?;
    assert!(stale.verdict().determinate().is_none());
    assert!(
        stale
            .verdict()
            .missing()
            .iter()
            .any(|check| matches!(check, MissingCheck::SourceNotFresh { .. }))
    );

    // An UNKNOWN leaf defeats coverage even with every rule placed.
    let with_open_gate = support::mixed_rules()?;
    let unknown = audit(
        &with_open_gate,
        support::transcript()?,
        sources(&with_open_gate)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    assert!(unknown.verdict().determinate().is_none());
    Ok(())
}

// ---------------------------------------------------------------------------
// graduation_conflict_fail_closed -- REQ-08-037
// ---------------------------------------------------------------------------

/// An unresolved source conflict keeps a graduation result `INDETERMINATE`.
#[test]
fn graduation_conflict_fail_closed() -> TestResult {
    let rules = support::satisfiable_rules()?;
    let case = support::conflict_case()?;
    assert_eq!(
        case.disposition(),
        academic_ingestion::AuditDisposition::Indeterminate,
        "a freshly detected case is unresolved"
    );

    let blocked = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        vec![ConflictReference::of(&case)],
        Some(FRESHNESS),
    )?;
    assert!(blocked.verdict().determinate().is_none());

    // The reference is actionable: it names the rule and both connectors.
    let reference = blocked
        .verdict()
        .missing()
        .iter()
        .find_map(|check| match check {
            MissingCheck::UnresolvedSourceConflict {
                rule,
                left_connector,
                right_connector,
            } => Some((
                rule.clone(),
                left_connector.clone(),
                right_connector.clone(),
            )),
            _ => None,
        })
        .ok_or("the blocked audit named no conflict")?;
    assert_eq!(reference.0, support::CONTESTED_RULE);
    assert_ne!(
        reference.1, reference.2,
        "both sides are the same connector"
    );
    assert!(
        blocked
            .verdict()
            .missing()
            .iter()
            .any(|check| check.action().contains("resolve the source conflict"))
    );

    // Resolving it lets the determination through, which is what says the
    // refusal above was the conflict rather than anything else.
    let mut resolved = case;
    resolved.resolve(academic_ingestion::UserResolution::recorded(
        academic_ingestion::Side::Left,
        academic_ingestion::DependentId::new("registrar-decision")?,
    ));
    let allowed = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        vec![ConflictReference::of(&resolved)],
        Some(FRESHNESS),
    )?;
    assert!(allowed.verdict().determinate().is_some());

    // And a rule that itself concludes `CONFLICT` blocks it too, with no
    // derived value published beside the disagreement.
    let baseline = support::baseline_rules()?;
    let record_conflict = audit(
        &baseline,
        support::transcript_with_conflicting_records()?,
        sources(&baseline)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    assert_eq!(record_conflict.root_status(), ProofStatus::Conflict);
    assert!(record_conflict.verdict().determinate().is_none());
    assert!(record_conflict.outcome().result.values.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// degree_audit_input_binding -- REQ-06-025
// ---------------------------------------------------------------------------

/// Each bound input moved on its own moves exactly its own digest.
#[test]
fn degree_audit_input_binding() -> TestResult {
    let rules = support::baseline_rules()?;
    let recorded = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    let base = recorded.binding();

    // The transcript moves.
    let other_transcript = audit(
        &rules,
        support::transcript_with_conflicting_records()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?
    .binding();
    assert_ne!(
        base.transcript_digest(),
        other_transcript.transcript_digest()
    );
    assert_eq!(base.profile_digest(), other_transcript.profile_digest());
    assert_eq!(base.rule_set_hash(), other_transcript.rule_set_hash());
    assert_eq!(
        base.source_index_digest(),
        other_transcript.source_index_digest()
    );

    // The rule set moves.
    let revised = support::revised_rules()?;
    let other_rules = audit(
        &revised,
        support::transcript()?,
        sources(&revised)?,
        Vec::new(),
        Some(FRESHNESS),
    )?
    .binding();
    assert_ne!(base.rule_set_hash(), other_rules.rule_set_hash());
    assert_eq!(base.transcript_digest(), other_rules.transcript_digest());

    // `revised` differs from `rules` in its version, its supersession and its
    // rule list, so the assertion above holds even when the rendering behind
    // `rule_set_hash` reaches no rule body -- which is what it did. The pair
    // below differs in a credit threshold and in nothing else, so the binding
    // has to separate them on the rules alone.
    let strict = support::credit_floor_rules(130)?;
    let lax = support::credit_floor_rules(12)?;
    let strict_binding = audit(
        &strict,
        support::transcript()?,
        sources(&strict)?,
        Vec::new(),
        Some(FRESHNESS),
    )?
    .binding();
    let lax_binding = audit(
        &lax,
        support::transcript()?,
        sources(&lax)?,
        Vec::new(),
        Some(FRESHNESS),
    )?
    .binding();
    assert_ne!(
        strict_binding.rule_set_hash(),
        lax_binding.rule_set_hash(),
        "two sets differing only in a credit threshold bound the same hash"
    );
    assert_ne!(
        strict_binding.digest(),
        lax_binding.digest(),
        "two sets differing only in a credit threshold bound the same digest"
    );
    assert_eq!(
        strict_binding.transcript_digest(),
        lax_binding.transcript_digest()
    );
    assert_eq!(
        strict_binding.profile_digest(),
        lax_binding.profile_digest()
    );

    // The source placements move.
    let moved_sources = audit(
        &rules,
        support::transcript()?,
        sources_missing(&rules, "major_exclusive")?,
        Vec::new(),
        Some(FRESHNESS),
    )?
    .binding();
    assert_ne!(
        base.source_index_digest(),
        moved_sources.source_index_digest()
    );
    assert_eq!(base.transcript_digest(), moved_sources.transcript_digest());
    assert_eq!(base.rule_set_hash(), moved_sources.rule_set_hash());

    // The profile moves. A profile that differs only in a field the selector
    // still admits is still a different profile.
    let with_major =
        profile()?.with_additional_majors(vec![academic_audit::ProgrammeId::new("stat")?]);
    let other_profile = audit_for(
        &with_major,
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?
    .binding();
    assert_ne!(base.profile_digest(), other_profile.profile_digest());
    assert_eq!(base.transcript_digest(), other_profile.transcript_digest());

    // Every one of the four moves the whole binding, and the recorded audit is
    // unchanged by any of them.
    for moved in [other_transcript, other_rules, moved_sources, other_profile] {
        assert_ne!(base.digest(), moved.digest());
    }
    assert_eq!(recorded.binding(), base);
    Ok(())
}

// ---------------------------------------------------------------------------
// thesis_determinate_gate -- GATE-38-012
// ---------------------------------------------------------------------------

/// A completed thesis does not resolve an unresolved thesis rule.
#[test]
fn thesis_determinate_gate() -> TestResult {
    let rules = support::open_gate_rules()?;
    let audit = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;

    // The record really does hold a completed, credited attempt at the course
    // the thesis rule names. Without this the assertion below would be about an
    // absent attempt, which is a weaker claim.
    let thesis_course = course(support::COURSE_THESIS)?;
    let attempt = audit
        .transcript()
        .entries()
        .iter()
        .find(|entry| entry.course() == thesis_course)
        .ok_or("the transcript holds no attempt at the thesis course")?;
    assert!(matches!(
        attempt.admission(),
        EntryAdmission::Counted { .. }
    ));

    let leaf = audit
        .nodes()
        .iter()
        .find(|node| node.leaf().rule().as_str() == "thesis_research")
        .ok_or("the set has no thesis rule")?
        .leaf();
    assert_eq!(leaf.status(), ProofStatus::Unknown);
    assert_eq!(leaf.open_gate(), Some(OpenGate::RuleThesisScope));
    assert_eq!(leaf.rule_gate(), Some(RuleGate::ThesisRuleScope));
    assert!(audit.verdict().determinate().is_none());
    assert!(audit.verdict().missing().iter().any(|check| matches!(
        check,
        MissingCheck::OpenOfficialFact {
            gate: Some(OpenGate::RuleThesisScope),
            ..
        }
    )));

    // `GATE-38-015` and `GATE-38-016` stay `academic-requirement`'s: the leaf
    // carries the rule crate's cell and this crate names none of its own.
    let cap = audit
        .nodes()
        .iter()
        .find(|node| node.leaf().rule().as_str() == "external_recognition_cap")
        .ok_or("the set has no external-recognition rule")?
        .leaf();
    assert_eq!(cap.status(), ProofStatus::Unknown);
    assert_eq!(cap.open_gate(), None);
    assert_eq!(cap.rule_gate(), Some(RuleGate::ExternalCreditRecognition));
    Ok(())
}

/// The course-facts index refuses a transcript it does not cover.
///
/// Not one of the thirteen: it is the fail-closed half of the transcript
/// boundary, and without it every assertion above would hold over a transcript
/// the engine had silently read only part of.
#[test]
fn a_transcript_the_curriculum_has_not_placed_is_refused() -> TestResult {
    let refused = TranscriptSnapshot::from_record(
        &academic_record::corpus::baseline_history()?,
        &support::classification()?,
        &support::record_rules()?,
        &support::primary_program()?,
        &CourseFactsIndex::new(),
    );
    assert!(
        refused.is_err(),
        "an empty course index produced a transcript"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// a_gate_that_refuses_always_names_a_check -- REQ-11-024
// ---------------------------------------------------------------------------

/// Every route to a refused gate names the thing that is actually outstanding.
///
/// `DegreeAudit::assemble` used to end its `INDETERMINATE` arm with
/// `from_checks(missing).unwrap_or_else(|| … SourceFreshnessPolicyAbsent …)`,
/// and a published set with **no rule** reached it: the tree had no leaf, the
/// coverage gate refused, and there was no rule for a check to be about. The
/// audit then told a user who had recorded the source-freshness criterion to
/// record the source-freshness criterion. `verdict.rs`'s own contract is that
/// "every arm names the exact cell, rule, attempt or dimension that is
/// outstanding", and that arm named a different one.
///
/// Two things close it, and this drives both. `RuleSetDraft::publish` refuses
/// a set with no rule, so the state does not exist; and the fallback is now
/// `AuditError::RefusedWithNoCheck` rather than an invented check, so if the
/// state ever does exist the audit refuses instead of lying. What is left to
/// observe is the property the fallback was hiding: on every route that
/// refuses a gate, the check list is non-empty **and** does not name freshness
/// unless freshness is what is wrong.
#[test]
fn a_gate_that_refuses_always_names_a_check() -> TestResult {
    let baseline = support::baseline_rules()?;
    let open_gate = support::open_gate_rules()?;

    // Each row refuses a gate a different way. The freshness criterion is
    // supplied in every row but the fifth.
    let rows: Vec<(&str, DegreeAudit, &str)> = vec![
        (
            "a rule with no recorded page",
            audit(
                &baseline,
                support::transcript()?,
                sources_missing(&baseline, "total_credits")?,
                Vec::new(),
                Some(FRESHNESS),
            )?,
            "RULE_SOURCE_SPAN_ABSENT",
        ),
        (
            "a rule whose official fact is unconfirmed",
            audit(
                &open_gate,
                support::transcript()?,
                sources(&open_gate)?,
                Vec::new(),
                Some(FRESHNESS),
            )?,
            "OPEN_OFFICIAL_FACT",
        ),
        (
            "an unresolved conflict between two official sources",
            audit(
                &baseline,
                support::transcript()?,
                sources(&baseline)?,
                vec![support::unresolved_conflict()?],
                Some(FRESHNESS),
            )?,
            "UNRESOLVED_SOURCE_CONFLICT",
        ),
        (
            "a source older than the recorded criterion",
            audit(
                &baseline,
                support::transcript()?,
                sources(&baseline)?,
                Vec::new(),
                Some(STALE_FRESHNESS),
            )?,
            "SOURCE_NOT_FRESH",
        ),
        (
            "no recorded freshness criterion",
            audit(
                &baseline,
                support::transcript()?,
                sources(&baseline)?,
                Vec::new(),
                None,
            )?,
            "SOURCE_FRESHNESS_POLICY_ABSENT",
        ),
    ];

    for (route, audited, expected) in &rows {
        let checks: Vec<&'static str> = audited
            .verdict()
            .missing()
            .iter()
            .map(MissingCheck::kind)
            .collect();
        assert!(
            matches!(audited.verdict(), DegreeVerdict::Indeterminate(_)),
            "{route} reached a determination"
        );
        assert!(
            !checks.is_empty(),
            "{route} refused a gate and named nothing"
        );
        assert!(
            checks.contains(expected),
            "{route} named {checks:?} and not {expected}"
        );
        // The lie the fallback told: freshness reported absent by an audit
        // that was handed a criterion.
        if *expected != "SOURCE_FRESHNESS_POLICY_ABSENT" {
            assert!(
                !checks.contains(&"SOURCE_FRESHNESS_POLICY_ABSENT"),
                "{route} reported the freshness criterion absent; it was supplied"
            );
        }
        // And every check names a subject, so `action()` is something a person
        // can do rather than a sentence about missing information.
        for check in audited.verdict().missing() {
            assert!(
                !check.action().is_empty(),
                "{route}: {} carries no action",
                check.kind()
            );
        }
    }

    // Each row's own reason really is its own: the five check lists are not
    // the same list five times.
    let rendered: BTreeSet<Vec<&'static str>> = rows
        .iter()
        .map(|(_, audited, _)| {
            audited
                .verdict()
                .missing()
                .iter()
                .map(MissingCheck::kind)
                .collect()
        })
        .collect();
    assert_eq!(rendered.len(), rows.len(), "two routes reported alike");
    Ok(())
}

// ---------------------------------------------------------------------------
// a_source_conflict_is_applicable_by_the_document_identifier -- REQ-08-014
// ---------------------------------------------------------------------------

/// The same unresolved conflict blocks the same requirement under any set-local
/// spelling, and does not block a set the document rule is not in.
///
/// Applicability used to be `rule.as_str() == case.rule()` -- the identifier a
/// **reviewer typed** into a `RuleCandidate` against the identifier the
/// **official document** carries. Nothing bound the two, so one unresolved
/// conflict over `total_credits` made a set that happened to publish the rule
/// under that name `INDETERMINATE` and left an identical set published under
/// `credit_floor` `DETERMINATE POSSIBLE`, with `conflict cases examined = 0` on
/// the determination.
///
/// `RuleSetDraft::include` now refuses a rule whose `source_rule` the published
/// document does not carry, and the gate compares that bound identifier. Both
/// halves are here: the same conflict applies across two set-local spellings,
/// and a conflict about a document rule this set publishes nothing from does
/// not.
#[test]
fn a_source_conflict_is_applicable_by_the_document_identifier() -> TestResult {
    let case = support::conflict_case()?;
    assert_eq!(
        case.disposition(),
        academic_ingestion::AuditDisposition::Indeterminate,
        "the fixture case has to be unresolved for this to say anything"
    );
    let reference = ConflictReference::of(&case);
    assert_eq!(
        reference.rule(),
        support::CONTESTED_RULE,
        "the case is about the document rule these sets are published from"
    );

    // The same requirement, published under two different set-local
    // identifiers, both read from the document rule the conflict is about.
    let named_alike = support::credit_floor_rules(12)?;
    let named_apart = support::credit_floor_named("credit_floor", support::CONTESTED_RULE, 12)?;
    assert_ne!(
        named_alike
            .rules()
            .map(|(rule, _)| rule.as_str().to_owned())
            .collect::<Vec<_>>(),
        named_apart
            .rules()
            .map(|(rule, _)| rule.as_str().to_owned())
            .collect::<Vec<_>>(),
        "the two sets must differ in the set-local spelling"
    );

    for (label, rules) in [
        ("as the document spells it", &named_alike),
        ("under another name", &named_apart),
    ] {
        let audited = audit(
            rules,
            support::transcript()?,
            sources(rules)?,
            vec![reference.clone()],
            Some(FRESHNESS),
        )?;
        assert!(
            audited
                .verdict()
                .missing()
                .iter()
                .any(|check| matches!(check, MissingCheck::UnresolvedSourceConflict { .. })),
            "{label}: the unresolved conflict did not block the determination"
        );
    }

    // And it still narrows: a set published from a different document rule is
    // not blocked by this case, so the gate is a binding rather than a refusal
    // to conclude at all.
    let elsewhere = support::credit_floor_named("total_credits", "seminar_choice", 12)?;
    let unaffected = audit(
        &elsewhere,
        support::transcript()?,
        sources(&elsewhere)?,
        vec![reference],
        Some(FRESHNESS),
    )?;
    assert!(
        !unaffected
            .verdict()
            .missing()
            .iter()
            .any(|check| matches!(check, MissingCheck::UnresolvedSourceConflict { .. })),
        "a conflict about a document rule this set publishes nothing from blocked it"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// an_unread_conflict_store_is_not_an_absence_of_conflict -- REQ-08-014
// ---------------------------------------------------------------------------

/// Nobody having read the conflict store is not the same as no conflict.
///
/// `ConflictFreeWitness::establish` took a bare slice and issued a witness over
/// an empty one -- the vacuous witness `CoverageWitness::establish` refuses
/// eleven lines above it, in the same file, in a comment naming this exact
/// failure mode. A caller who simply passed no cases got `DETERMINATE POSSIBLE`
/// with `conflict cases examined = 0`.
#[test]
fn an_unread_conflict_store_is_not_an_absence_of_conflict() -> TestResult {
    let rules = support::satisfiable_rules()?;

    // The store was read and held nothing. A real observation, and the witness
    // records that it examined none.
    let read = audit(
        &rules,
        support::transcript()?,
        sources(&rules)?,
        Vec::new(),
        Some(FRESHNESS),
    )?;
    let determinate = read
        .verdict()
        .determinate()
        .ok_or("a read-and-empty store did not reach a determination")?;
    assert_eq!(determinate.conflict_free().cases_examined(), 0);

    // Nobody read it. Same everything else.
    let selection = select(&profile()?, &catalog(&rules)?);
    let engine = GraduationAuditEngine::new(
        selection.selected().ok_or("no rule set selected")?.clone(),
        EngineVersion::MIN,
    );
    let unread_facts = support::surveyed_facts(
        support::transcript()?,
        sources(&rules)?,
        None,
        Some(FRESHNESS),
    )?;
    let unread = DegreeAudit::evaluate(&engine, &encode(&unread_facts)?)?;
    assert!(
        matches!(unread.verdict(), DegreeVerdict::Indeterminate(_)),
        "an audit whose conflict store nobody read reached a determination"
    );
    assert!(
        unread
            .verdict()
            .missing()
            .contains(&MissingCheck::SourceConflictSurveyAbsent),
        "it did not name the survey it never had: {:?}",
        unread
            .verdict()
            .missing()
            .iter()
            .map(MissingCheck::kind)
            .collect::<Vec<_>>()
    );

    // The two are different audits by their frozen inputs as well as by their
    // verdicts, so the distinction survives into the recorded binding.
    assert_ne!(
        read.binding().frozen_inputs_digest(),
        unread.binding().frozen_inputs_digest(),
        "a read-and-empty store and an unread one froze the same inputs"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// the_scope_range_refuses_a_range_it_cannot_compare -- REQ-11-021
// ---------------------------------------------------------------------------

/// A graduation-standard range that no lexicographic comparison can read.
///
/// `RuleSetScope::new` refuses two shapes: a range whose ends differ in width,
/// because `"9" > "10"` lexicographically and a ragged range therefore covers
/// the wrong set; and a reversed range, which covers nothing while looking like
/// a scope. `P2-A3` deleted each of them in turn and the whole `academic-audit`
/// suite passed both times -- the selector's own matrix never builds a
/// malformed range, so nothing drove either refusal.
#[test]
fn the_scope_range_refuses_a_range_it_cannot_compare() -> TestResult {
    // The result of `RuleSetScope::new` itself, so an accepted range is
    // observable as `Ok`. Written as a closure returning `Err` on the accepting
    // path once, it passed with **both** refusals deleted -- an empty guard of
    // exactly the shape this repair exists to remove.
    let build = |from: &str,
                 to: &str|
     -> Result<Result<RuleSetScope, AuditError>, Box<dyn std::error::Error>> {
        Ok(RuleSetScope::new(
            support::university()?,
            support::college()?,
            support::department()?,
            support::admission_year()?,
            academic_audit::GraduationStandard::new(from)?,
            academic_audit::GraduationStandard::new(to)?,
            academic_audit::DegreeMode::SingleMajor,
        ))
    };

    // The well-formed range this is measured against. Without it, a
    // `RuleSetScope::new` that refused everything would satisfy the three
    // refusals below.
    assert!(
        build("2020", "2026")?.is_ok(),
        "a range with equal-width ends in order was refused"
    );

    // Ragged: `"9"` and `"2026"` cannot be compared as text -- `"9" > "2026"`
    // lexicographically -- so the range covers the complement of what it says.
    assert!(
        matches!(
            build("9", "2026")?,
            Err(AuditError::InvalidIdentifier { .. })
        ),
        "a range whose ends differ in width was accepted"
    );
    assert!(
        matches!(
            build("2020", "20260")?,
            Err(AuditError::InvalidIdentifier { .. })
        ),
        "a range whose ends differ in width was accepted"
    );

    // Reversed: equal width, wrong order, covers nothing while looking like a
    // scope.
    assert!(
        matches!(
            build("2026", "2020")?,
            Err(AuditError::InvalidIdentifier { .. })
        ),
        "a reversed range was accepted"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// an_audit_with_no_evaluated_rule_folds_to_unknown -- REQ-11-016
// ---------------------------------------------------------------------------

/// A tree with no leaf reads `UNKNOWN`, never `SATISFIED`.
///
/// `fold` returns `UNKNOWN` for an empty leaf list before it looks for any
/// status, and `P2-A3` deleted that arm with the whole suite still green: every
/// corpus in the crate produces at least one leaf, so nothing drove it. Removed
/// together with the coverage gate's empty-leaf refusal, a rule set with no
/// evaluated rule answered `DETERMINATE POSSIBLE` from an empty tree.
///
/// A published set now always has a rule, so the way to an empty tree is that
/// **every** rule's source page is unrecorded -- which section 11.3 refuses to
/// evaluate on rather than evaluating into a leaf with no citation.
#[test]
fn an_audit_with_no_evaluated_rule_folds_to_unknown() -> TestResult {
    let rules = support::baseline_rules()?;
    assert!(rules.rules().count() >= 8);
    let audited = audit(
        &rules,
        support::transcript()?,
        RuleSourceIndex::new(),
        Vec::new(),
        Some(FRESHNESS),
    )?;

    assert_eq!(
        audited.nodes().len(),
        0,
        "the tree is not empty; this drives nothing"
    );
    assert_eq!(
        audited.root_status(),
        ProofStatus::Unknown,
        "an empty tree folded to something other than UNKNOWN"
    );
    assert!(
        matches!(audited.verdict(), DegreeVerdict::Indeterminate(_)),
        "an audit that evaluated no rule reached a determination"
    );
    // And it says why, once per rule, rather than reporting one thing.
    let unplaced: Vec<&RuleId> = audited
        .verdict()
        .missing()
        .iter()
        .filter_map(|check| match check {
            MissingCheck::RuleSourceSpanAbsent { rule } => Some(rule),
            _ => None,
        })
        .collect();
    assert_eq!(
        unplaced.len(),
        rules.rules().count(),
        "the audit did not name every rule it could not place"
    );
    Ok(())
}
