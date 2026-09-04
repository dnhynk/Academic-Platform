//! `P2-U8`'s behavioural acceptance evidence.
//!
//! Six of the eight named tests are here. Two are absences a behavioural test
//! cannot observe -- `no_login_bypass_or_evasion_module_exists` and
//! `raw_review_text_is_excluded_from_export_and_share` -- and they are in
//! `tests/review_scans.rs`.
//!
//! Every one of these is per-item rather than per-count: it drops one thing at
//! a time and requires the exact refusal for it, or it compares a whole set
//! both ways. There is no assertion in this file of the form "there are six".

mod support;

use std::error::Error;

use academic_curriculum::{InstructorName, TermCode};
use academic_domain::EpistemicStatus;
use academic_ingestion::{DenialReason, DenialRoute, Fallback, TermsStatus};
use academic_untrusted_content::{SourceId, SourceKind};

use academic_review::{
    AggregationClaim, AggregationMethod, BiasDimension, BiasStrength, CourseAggregate,
    CourseReading, DimensionBand, DimensionReading, OfferingAggregate, ReviewDimension,
    ReviewError, ReviewExtraction, ReviewRecord, ReviewScope, SampleBias, ScopeDimension,
    SimilarityPermille, SourceAccessMode, SourceTermsLedger, duplicate_findings,
    duplicated_record_count, permit, similarity,
};

use support::{
    SOURCE, collection, disclosure, draft_disclosing, extraction_at, extraction_with, offering_id,
    review, scope, source,
};

type TestResult = Result<(), Box<dyn Error>>;

/// The specification, as text.
fn specification() -> Result<String, Box<dyn Error>> {
    repository_file("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md")
}

/// Section 29.5, from its heading to the next one.
fn section_29_5() -> Result<String, Box<dyn Error>> {
    let text = specification()?;
    let start = text
        .find("### 29.5 강의평")
        .ok_or("section 29.5 is not in the specification")?;
    let rest = &text[start..];
    let end = rest[1..]
        .find("\n### ")
        .map_or(rest.len(), |offset| offset + 1);
    Ok(rest[..end].to_owned())
}

// ---------------------------------------------------------------------------
// review_default_scope_is_offering_instructor_term_source
// ---------------------------------------------------------------------------

/// The four dimensions are section 29.5's own, and a review carries no course.
///
/// Three halves. The specification's own sentence is walked forwards and its
/// backticked list compared with `ScopeDimension::ALL` in both directions; the
/// record built from a permitted collection carries all four; and two reviews
/// that differ in any one of them do not aggregate together, which is checked
/// once per dimension rather than once.
#[test]
fn review_default_scope_is_offering_instructor_term_source() -> TestResult {
    let section = section_29_5()?;
    let sentence = section
        .lines()
        .find(|line| line.contains("Review는 기본적으로"))
        .ok_or("section 29.5 no longer states what a review is attached to")?;

    // The specification writes the four inside one backticked span. Reading the
    // span rather than searching for four names is what makes a fifth name in
    // it a failure instead of something nobody looked for.
    let open = sentence
        .find('`')
        .ok_or("the sentence no longer carries a backticked scope")?;
    let close = sentence[open + 1..]
        .find('`')
        .map(|offset| open + 1 + offset)
        .ok_or("the backticked scope is unterminated")?;
    let spelled: Vec<&str> = sentence[open + 1..close]
        .split('+')
        .map(str::trim)
        .collect();
    let expected: Vec<&str> = ScopeDimension::ALL
        .into_iter()
        .map(ScopeDimension::spec_name)
        .collect();
    assert_eq!(
        spelled, expected,
        "section 29.5's scope and ScopeDimension::ALL have diverged"
    );

    // The same sentence is what says a course needs an explicit aggregation, so
    // it is read here too rather than assumed.
    assert!(
        sentence.contains("Course 전체로 승격할 때 명시적 aggregation"),
        "section 29.5 no longer requires an explicit aggregation to reach a course"
    );

    let record = review(
        1,
        scope(SOURCE, 1, "Kim", "2026_1")?,
        "the lectures were slow and the assignments were long",
        SourceAccessMode::ManualPaste,
        extraction_at(DimensionBand::High)?,
    )?;
    for dimension in ScopeDimension::ALL {
        assert!(
            record.scope().carries(dimension),
            "the collected record does not carry {}",
            dimension.as_str()
        );
    }
    assert_eq!(record.source_access_mode(), SourceAccessMode::ManualPaste);
    assert_eq!(record.extraction_status(), EpistemicStatus::AiInferred);

    // One dimension changed at a time, and each one has to be enough on its own
    // to stop two reviews being one aggregate.
    for dimension in ScopeDimension::ALL {
        let other = match dimension {
            ScopeDimension::Offering => ReviewScope::new(
                source(SOURCE)?,
                Some(offering_id(2)?),
                Some(InstructorName::parse("Kim")?),
                Some(TermCode::parse("2026_1")?),
            ),
            ScopeDimension::Instructor => ReviewScope::new(
                source(SOURCE)?,
                Some(offering_id(1)?),
                Some(InstructorName::parse("Park")?),
                Some(TermCode::parse("2026_1")?),
            ),
            ScopeDimension::Term => ReviewScope::new(
                source(SOURCE)?,
                Some(offering_id(1)?),
                Some(InstructorName::parse("Kim")?),
                Some(TermCode::parse("2026_2")?),
            ),
            ScopeDimension::Source => ReviewScope::new(
                source("other.review.board")?,
                Some(offering_id(1)?),
                Some(InstructorName::parse("Kim")?),
                Some(TermCode::parse("2026_1")?),
            ),
        };
        assert!(
            !record.scope().same_scope_as(&other),
            "{} alone does not separate two scopes",
            dimension.as_str()
        );
        let second = ReviewRecord::collected(
            &collection(SOURCE, SourceAccessMode::ManualPaste)?,
            other,
            support::retained("a second review of a different thing")?,
            academic_ingestion::RetrievalInstant::at(1_700_000_100),
            support::autosaved(
                100 + dimension.as_str().len() as u64,
                extraction_at(DimensionBand::Low)?,
            )?,
            academic_review::SampleBias::none(),
        );
        let mixed = OfferingAggregate::over(
            &[
                review(
                    2,
                    scope(SOURCE, 1, "Kim", "2026_1")?,
                    "the lectures were slow and the assignments were long",
                    SourceAccessMode::ManualPaste,
                    extraction_at(DimensionBand::High)?,
                )?,
                second,
            ],
            disclosure(2)?,
        );
        assert_eq!(
            mixed.err(),
            Some(ReviewError::ScopeMixed(dimension)),
            "two reviews differing only in {} were aggregated together",
            dimension.as_str()
        );
    }

    // The scope reports the source as never nullable and the other three as
    // nullable, which is the specification's own `... | null`.
    for dimension in ScopeDimension::ALL {
        assert_eq!(
            dimension.is_nullable(),
            dimension != ScopeDimension::Source,
            "{} is on the wrong side of section 29.5's null spelling",
            dimension.as_str()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// course_promotion_requires_explicit_aggregation
// ---------------------------------------------------------------------------

/// A course-level value exists only behind a named aggregation claim.
///
/// The compile-time half -- that a claim cannot be assembled from outside and
/// that `promote` takes nothing else -- is in `tests/compile_fail/`. This is the
/// behavioural half, and it drives **both** refusal branches of the one
/// producer separately, because a branch no test reaches can be relaxed and
/// every test still passes.
#[test]
fn course_promotion_requires_explicit_aggregation() -> TestResult {
    let first = OfferingAggregate::over(
        &[review(
            10,
            scope(SOURCE, 1, "Kim", "2026_1")?,
            "the problem sets were heavy but fair",
            SourceAccessMode::Public,
            extraction_at(DimensionBand::High)?,
        )?],
        disclosure(1)?,
    )?;
    let second = OfferingAggregate::over(
        &[review(
            11,
            scope(SOURCE, 2, "Park", "2026_2")?,
            "the grading was gentle and the lectures were clear",
            SourceAccessMode::Public,
            extraction_at(DimensionBand::Low)?,
        )?],
        disclosure(1)?,
    )?;
    let course = support::course_id(7)?;

    // A claim asserted over both, spent on both, promotes.
    let claim = AggregationClaim::asserting(
        AggregationMethod::PooledBandCounts,
        course,
        &[first.clone(), second.clone()],
    );
    assert_eq!(claim.method(), AggregationMethod::PooledBandCounts);
    let promoted =
        CourseAggregate::promote(claim, &[first.clone(), second.clone()], disclosure(2)?)?;
    assert_eq!(promoted.course(), course);
    assert_eq!(promoted.method(), AggregationMethod::PooledBandCounts);
    assert_eq!(promoted.sample_size(), 2);

    // Branch one: a claim asserted over one set, spent on another.
    let mismatched = AggregationClaim::asserting(
        AggregationMethod::PooledBandCounts,
        course,
        core::slice::from_ref(&first),
    );
    assert_eq!(
        CourseAggregate::promote(mismatched, &[first.clone(), second.clone()], disclosure(2)?)
            .err(),
        Some(ReviewError::PromotionScopeMixed),
        "a claim made over one offering was spent on two"
    );

    // Branch two: the same aggregate offered twice.
    let repeated = AggregationClaim::asserting(
        AggregationMethod::PooledBandCounts,
        course,
        &[first.clone(), first.clone()],
    );
    assert_eq!(
        CourseAggregate::promote(repeated, &[first.clone(), first.clone()], disclosure(2)?).err(),
        Some(ReviewError::PromotionInputRepeated),
        "one offering aggregate was counted twice"
    );

    // Branch three: nothing to promote.
    let empty = AggregationClaim::asserting(AggregationMethod::PooledBandCounts, course, &[]);
    assert_eq!(
        CourseAggregate::promote(empty, &[], disclosure(0)?).err(),
        Some(ReviewError::NoReviews)
    );

    // The offering-level producer refuses its own two branches too, so both
    // producers are driven rather than one.
    assert_eq!(
        OfferingAggregate::over(&[], disclosure(0)?).err(),
        Some(ReviewError::NoReviews)
    );
    Ok(())
}

/// The named method decides what a course value *is*, rather than labelling it.
///
/// A method nothing reads is decoration. Both arms are run over one input and
/// the two readings have to differ in shape, and the reading's own idea of
/// which method made it has to agree with the claim's.
#[test]
fn the_named_method_decides_what_the_course_value_is() -> TestResult {
    let first = OfferingAggregate::over(
        &[review(
            20,
            scope(SOURCE, 1, "Kim", "2026_1")?,
            "a heavy term with two projects",
            SourceAccessMode::Public,
            extraction_at(DimensionBand::VeryHigh)?,
        )?],
        disclosure(1)?,
    )?;
    let second = OfferingAggregate::over(
        &[review(
            21,
            scope(SOURCE, 2, "Park", "2026_2")?,
            "a light term with one project",
            SourceAccessMode::Public,
            extraction_at(DimensionBand::VeryLow)?,
        )?],
        disclosure(1)?,
    )?;
    let course = support::course_id(8)?;
    let inputs = [first.clone(), second.clone()];

    let mut readings = Vec::new();
    for method in AggregationMethod::ALL {
        let claim = AggregationClaim::asserting(method, course, &inputs);
        let promoted = CourseAggregate::promote(claim, &inputs, disclosure(2)?)?;
        assert_eq!(
            promoted.reading().method(),
            method,
            "the {} reading does not know which method made it",
            method.as_str()
        );
        readings.push(promoted.reading().clone());
    }
    assert_eq!(readings.len(), AggregationMethod::ALL.len());
    for (position, reading) in readings.iter().enumerate() {
        for other in &readings[..position] {
            assert_ne!(
                reading, other,
                "two named methods produced the same course value from one input"
            );
        }
    }

    // The pooled arm sums; the listing arm keeps the offerings apart. Both are
    // read here so neither arm is an unvisited branch.
    match &readings[0] {
        CourseReading::Pooled { distributions } => {
            let difficulty = distributions[ReviewDimension::Difficulty.index()];
            assert_eq!(difficulty.count(DimensionBand::VeryHigh), 1);
            assert_eq!(difficulty.count(DimensionBand::VeryLow), 1);
            assert_eq!(difficulty.total(), 2);
        }
        CourseReading::PerOffering { .. } => {
            return Err("PooledBandCounts produced a per-offering reading".into());
        }
    }
    match &readings[1] {
        CourseReading::PerOffering { offerings } => {
            assert_eq!(offerings.len(), 2);
            assert_eq!(offerings[0].scope(), first.scope());
            assert_eq!(offerings[1].scope(), second.scope());
        }
        CourseReading::Pooled { .. } => {
            return Err("PerOfferingListing produced a pooled reading".into());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// scalar_is_not_a_course_property
// ---------------------------------------------------------------------------

/// A course reading is a distribution, and there is no value it reduces to.
///
/// The source-level half -- that `academic-curriculum`'s `Course` names no
/// dimension and that no `impl` here converts a band to a number -- is in
/// `tests/review_scans.rs`. This half is the behaviour: readings that a mean
/// would collapse into one number stay distinguishable, and the six
/// disclosures travel with them.
#[test]
fn scalar_is_not_a_course_property() -> TestResult {
    // Two samples whose mean difficulty would be identical and whose shapes are
    // not: one all-moderate, one split between the extremes.
    let flat = OfferingAggregate::over(
        &[
            review(
                30,
                scope(SOURCE, 1, "Kim", "2026_1")?,
                "an even term throughout",
                SourceAccessMode::Public,
                extraction_at(DimensionBand::Moderate)?,
            )?,
            review(
                31,
                scope(SOURCE, 1, "Kim", "2026_1")?,
                "nothing surprising happened at all",
                SourceAccessMode::Public,
                extraction_at(DimensionBand::Moderate)?,
            )?,
        ],
        disclosure(2)?,
    )?;
    let split = OfferingAggregate::over(
        &[
            review(
                32,
                scope(SOURCE, 1, "Kim", "2026_1")?,
                "the hardest course I have taken",
                SourceAccessMode::Public,
                extraction_at(DimensionBand::VeryHigh)?,
            )?,
            review(
                33,
                scope(SOURCE, 1, "Kim", "2026_1")?,
                "the easiest course I have taken",
                SourceAccessMode::Public,
                extraction_at(DimensionBand::VeryLow)?,
            )?,
        ],
        disclosure(2)?,
    )?;

    let flat_difficulty = flat.distribution(ReviewDimension::Difficulty);
    let split_difficulty = split.distribution(ReviewDimension::Difficulty);
    assert_eq!(flat_difficulty.total(), split_difficulty.total());
    assert_ne!(
        flat_difficulty.counts(),
        split_difficulty.counts(),
        "two samples a mean would collapse are indistinguishable here"
    );

    // Every dimension of a course value is a distribution, and every one of
    // them arrives with the whole disclosure attached.
    let course = support::course_id(9)?;
    let claim = AggregationClaim::asserting(
        AggregationMethod::PooledBandCounts,
        course,
        core::slice::from_ref(&flat),
    );
    let promoted = CourseAggregate::promote(claim, core::slice::from_ref(&flat), disclosure(2)?)?;
    let CourseReading::Pooled { distributions } = promoted.reading() else {
        return Err("PooledBandCounts produced a per-offering reading".into());
    };
    for dimension in ReviewDimension::ALL {
        let distribution = distributions[dimension.index()];
        assert_eq!(distribution.dimension(), dimension);
        assert_eq!(
            distribution.total(),
            2,
            "{} lost a reading on the way to the course",
            dimension.as_str()
        );
    }
    for dimension in BiasDimension::ALL {
        assert_eq!(
            promoted.disclosure().finding(dimension).dimension(),
            dimension
        );
    }

    // And the source half: `academic-curriculum`'s `Course` is three fields,
    // none of them a review reading, and it names nothing from here. The list
    // is compared both ways, so a field added there fails as an extra entry
    // rather than as something nobody looked for.
    let course_module = repository_file("crates/curriculum/src/course.rs")?;
    let body = course_module
        .split_once("pub struct Course {")
        .ok_or("academic-curriculum no longer declares `pub struct Course`")?
        .1
        .split_once('}')
        .ok_or("the Course declaration is unterminated")?
        .0;
    let declared: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim())
        .collect();
    assert_eq!(
        declared,
        vec!["id", "code", "canonical_identity"],
        "section 8.2's Course grew a field; check that it is not a review reading"
    );
    for dimension in ReviewDimension::ALL {
        for spelling in [dimension.spec_key(), dimension.as_str()] {
            assert!(
                !course_module.contains(spelling),
                "academic-curriculum's Course module names {spelling}"
            );
        }
    }
    for name in ["CourseAggregate", "BandDistribution", "academic_review"] {
        assert!(
            !course_module.contains(name),
            "academic-curriculum's Course module names {name}"
        );
    }
    Ok(())
}

/// A repository file, as text.
fn repository_file(path: &str) -> Result<String, Box<dyn Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or("the crate is two levels below the repository root")?;
    Ok(std::fs::read_to_string(root.join(path))?)
}

// ---------------------------------------------------------------------------
// aggregate_discloses_all_six_bias_dimensions
// ---------------------------------------------------------------------------

/// Every one of section 29.5's six is disclosed, and dropping any one refuses.
///
/// The six are read out of the specification's own sentence and compared both
/// ways, then each is dropped in turn and the exact per-dimension error is
/// required. The evidence is per-dimension; nothing here counts to six.
#[test]
fn aggregate_discloses_all_six_bias_dimensions() -> TestResult {
    let section = section_29_5()?;
    let sentence = section
        .lines()
        .find(|line| line.contains("강의평 aggregate는"))
        .ok_or("section 29.5 no longer says what an aggregate discloses")?;
    let listed: Vec<&str> = sentence
        .split_once("aggregate는")
        .ok_or("the disclosure sentence lost its subject")?
        .1
        .split_once("을 표시한다")
        .ok_or("the disclosure sentence lost its verb")?
        .0
        .split(',')
        .map(str::trim)
        .collect();
    let expected: Vec<&str> = BiasDimension::ALL
        .into_iter()
        .map(BiasDimension::spec_phrase)
        .collect();
    assert_eq!(
        listed, expected,
        "section 29.5's disclosure list and BiasDimension::ALL have diverged"
    );

    // A complete draft builds, and every dimension is present in the built
    // value in the specification's order.
    let complete = draft_disclosing(4, &BiasDimension::ALL).build()?;
    assert_eq!(complete.disclosed(), BiasDimension::ALL.to_vec());

    // One dropped at a time, each naming itself.
    for dropped in BiasDimension::ALL {
        let kept: Vec<BiasDimension> = BiasDimension::ALL
            .into_iter()
            .filter(|dimension| *dimension != dropped)
            .collect();
        assert_eq!(
            draft_disclosing(4, &kept).build().err(),
            Some(ReviewError::BiasDimensionMissing(dropped)),
            "a disclosure without {} was accepted",
            dropped.as_str()
        );
    }

    // A dimension disclosed twice is refused too, naming itself.
    for repeated in BiasDimension::ALL {
        let mut twice: Vec<BiasDimension> = BiasDimension::ALL.to_vec();
        twice.push(repeated);
        assert_eq!(
            draft_disclosing(4, &twice).build().err(),
            Some(ReviewError::BiasDimensionRepeated(repeated)),
            "a disclosure naming {} twice was accepted",
            repeated.as_str()
        );
    }

    // Both aggregate constructors take a built disclosure by value, so an
    // aggregate that exists discloses all six.
    let aggregate = OfferingAggregate::over(
        &[review(
            40,
            scope(SOURCE, 1, "Kim", "2026_1")?,
            "one review of one offering",
            SourceAccessMode::Public,
            extraction_at(DimensionBand::Moderate)?,
        )?],
        complete,
    )?;
    assert_eq!(
        aggregate.disclosure().disclosed(),
        BiasDimension::ALL.to_vec()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// denied_source_exposes_only_the_four_fallbacks
// ---------------------------------------------------------------------------

/// A refused source offers `P2-U6`'s four and nothing else, and stops.
///
/// Every unpermitting status is driven, in every access mode, so the claim is
/// about the pipeline rather than about one call. `GATE-38-021`'s own case --
/// a source nobody recorded -- is the empty ledger, which is the default.
#[test]
fn denied_source_exposes_only_the_four_fallbacks() -> TestResult {
    let refused = [
        (TermsStatus::Unreviewed, DenialReason::TermsUnreviewed),
        (TermsStatus::Refused, DenialReason::TermsRefuse),
        (TermsStatus::Revoked, DenialReason::TermsRevoked),
    ];
    for mode in SourceAccessMode::ALL {
        for (status, reason) in refused {
            let ledger = SourceTermsLedger::empty().recording(source(SOURCE)?, mode, status);
            let denial = permit(&ledger, &source(SOURCE)?, mode)
                .err()
                .ok_or("a status that permits nothing permitted a collection")?;
            assert_eq!(denial.reason(), reason);
            assert_eq!(denial.route(), DenialRoute::ManualOrStop);
            assert_eq!(
                denial.fallbacks(),
                Fallback::ALL.as_slice(),
                "a denial under {} in {} offered something other than the four",
                status.as_str(),
                mode.as_str()
            );
        }

        // The gate's own case: nothing recorded at all.
        let unconfigured = permit(&SourceTermsLedger::empty(), &source(SOURCE)?, mode)
            .err()
            .ok_or("an unconfigured source permitted a collection")?;
        assert_eq!(unconfigured.reason(), DenialReason::TermsUnreviewed);
        assert_eq!(unconfigured.fallbacks(), Fallback::ALL.as_slice());

        // A recorded permission permits that pair and no other mode.
        let ledger = support::ledger_permitting(SOURCE, mode)?;
        assert!(permit(&ledger, &source(SOURCE)?, mode).is_ok());
        for other in SourceAccessMode::ALL {
            if other != mode {
                assert!(
                    permit(&ledger, &source(SOURCE)?, other).is_err(),
                    "permitting {} also permitted {}",
                    mode.as_str(),
                    other.as_str()
                );
            }
        }

        // And a different source is not covered by this one's record.
        assert!(
            permit(&ledger, &source("another.board")?, mode).is_err(),
            "a record for one source permitted another"
        );
    }

    // Each of the four is something a person does. None of them is a module.
    for fallback in Fallback::ALL {
        assert!(
            !fallback.as_str().is_empty(),
            "a fallback with no name is not an offer"
        );
    }
    Ok(())
}

/// The three access modes are section 29.5's own union, and none authenticates.
#[test]
fn the_access_modes_are_section_29_5s_own() -> TestResult {
    let section = section_29_5()?;
    let line = section
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("sourceAccessMode:"))
        .ok_or("section 29.5's record no longer names an access mode")?;
    let spelled: Vec<&str> = line
        .trim_start_matches("sourceAccessMode:")
        .split('|')
        .map(str::trim)
        .collect();
    let expected: Vec<&str> = SourceAccessMode::ALL
        .into_iter()
        .map(SourceAccessMode::as_str)
        .collect();
    assert_eq!(
        spelled, expected,
        "section 29.5's access modes and SourceAccessMode::ALL have diverged"
    );
    for mode in SourceAccessMode::ALL {
        assert_eq!(SourceAccessMode::parse(mode.as_str()), Some(mode));
        assert!(
            !mode.presents_a_credential(),
            "{} presents a credential",
            mode.as_str()
        );
    }
    assert!(SourceAccessMode::parse("SCRAPED_WITH_SESSION").is_none());
    Ok(())
}

// ---------------------------------------------------------------------------
// duplicate_similarity_is_detected
// ---------------------------------------------------------------------------

/// Near-duplicate reviews are found, with values computed by hand.
///
/// The expected permille values below are worked out from the definition in
/// `crates/review/src/duplicate.rs` -- word trigrams, set intersection over set
/// union, times a thousand, integer division -- and the intersection and union
/// sizes each one came from are written beside it. Nothing here asks the
/// implementation what the answer is.
#[test]
fn duplicate_similarity_is_detected() -> TestResult {
    // A: "the lectures were clear and the workload was fair"
    //    words: the lectures were clear and the workload was fair   (9 words)
    //    trigrams: {the lectures were, lectures were clear, were clear and,
    //               clear and the, and the workload, the workload was,
    //               workload was fair}                              (7)
    //
    // B: "The lectures were clear, and the workload was fair!"
    //    normalises to exactly A's word list, so its trigram set is A's.
    //    |A ∩ B| = 7, |A ∪ B| = 7  ->  1000 * 7 / 7 = 1000
    //
    // C: "the lectures were clear and the projects were long"
    //    trigrams: {the lectures were, lectures were clear, were clear and,
    //               clear and the, and the projects, the projects were,
    //               projects were long}                             (7)
    //    shared with A: the first four.
    //    |A ∩ C| = 4, |A ∪ C| = 10  ->  1000 * 4 / 10 = 400
    //
    // D: "grading felt arbitrary from week to week"
    //    trigrams: {grading felt arbitrary, felt arbitrary from,
    //               arbitrary from week, from week to, week to week}  (5)
    //    |A ∩ D| = 0, |A ∪ D| = 12  ->  0
    let a = review(
        50,
        scope(SOURCE, 1, "Kim", "2026_1")?,
        "the lectures were clear and the workload was fair",
        SourceAccessMode::ManualPaste,
        extraction_at(DimensionBand::Moderate)?,
    )?;
    let b = review(
        51,
        scope(SOURCE, 1, "Kim", "2026_1")?,
        "The lectures were clear, and the workload was fair!",
        SourceAccessMode::ManualPaste,
        extraction_at(DimensionBand::Moderate)?,
    )?;
    let c = review(
        52,
        scope(SOURCE, 1, "Kim", "2026_1")?,
        "the lectures were clear and the projects were long",
        SourceAccessMode::ManualPaste,
        extraction_at(DimensionBand::Moderate)?,
    )?;
    let d = review(
        53,
        scope(SOURCE, 1, "Kim", "2026_1")?,
        "grading felt arbitrary from week to week",
        SourceAccessMode::ManualPaste,
        extraction_at(DimensionBand::Moderate)?,
    )?;

    assert_eq!(similarity(&a, &b).value(), 1000, "7 shared of 7 in union");
    assert_eq!(similarity(&a, &c).value(), 400, "4 shared of 10 in union");
    assert_eq!(similarity(&a, &d).value(), 0, "0 shared of 12 in union");

    // Symmetric in every pair, with neither side privileged.
    let records = [a, b, c, d];
    for left in 0..records.len() {
        for right in 0..records.len() {
            assert_eq!(
                similarity(&records[left], &records[right]),
                similarity(&records[right], &records[left]),
                "similarity is not symmetric at ({left}, {right})"
            );
        }
    }

    // A threshold that finds the punctuation-only copy and nothing else.
    let strict = SimilarityPermille::new(900)?;
    let findings = duplicate_findings(&records, strict);
    assert_eq!(
        findings
            .iter()
            .map(|finding| (
                finding.left(),
                finding.right(),
                finding.similarity().value()
            ))
            .collect::<Vec<_>>(),
        vec![(0, 1, 1000)],
        "at 900 permille the only duplicate pair is A and its retyped copy"
    );
    assert_eq!(duplicated_record_count(&records, strict), 2);

    // A looser threshold reaches the paraphrase as well, and the count is of
    // records involved rather than of pairs.
    let loose = SimilarityPermille::new(400)?;
    assert_eq!(
        duplicate_findings(&records, loose)
            .iter()
            .map(|finding| (finding.left(), finding.right()))
            .collect::<Vec<_>>(),
        vec![(0, 1), (0, 2), (1, 2)],
        "at 400 permille A, its copy and the paraphrase are all pairwise near"
    );
    assert_eq!(duplicated_record_count(&records, loose), 3);

    // Nothing is a duplicate of nothing: an empty set finds no pairs.
    assert!(duplicate_findings(&[], strict).is_empty());
    assert_eq!(duplicated_record_count(&[], strict), 0);

    // The bound is a bound.
    assert_eq!(
        SimilarityPermille::new(1001).err(),
        Some(ReviewError::SimilarityOutOfRange(1001))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The record, the extraction, and the two indexes
// ---------------------------------------------------------------------------

/// The record's fields are section 29.5's own keys, both ways.
#[test]
fn the_record_fields_are_section_29_5s_own() -> TestResult {
    let section = section_29_5()?;
    let start = section
        .find("ReviewRecord:")
        .ok_or("section 29.5 no longer carries the ReviewRecord block")?;
    let block = &section[start..];
    let end = block
        .find("\n```")
        .ok_or("the ReviewRecord block is unterminated")?;
    let keys: Vec<&str> = block[..end]
        .lines()
        .skip(1)
        .filter(|line| line.starts_with("  ") && !line.starts_with("    "))
        .filter_map(|line| line.trim().split_once(':'))
        .map(|(key, _)| key)
        .collect();

    // The accessor this crate answers each key with. A key the specification
    // adds fails as a missing entry; an entry the specification drops fails as
    // an extra one.
    let answered = [
        ("offering", "ReviewScope::offering"),
        ("instructor", "ReviewScope::instructor"),
        ("term", "ReviewScope::term"),
        ("rawArtifact", "ReviewRecord::raw_artifact"),
        ("sourceAccessMode", "ReviewRecord::source_access_mode"),
        ("collectedAt", "ReviewRecord::collected_at"),
        ("dimensions", "ReviewRecord::dimensions"),
        ("extractionStatus", "ReviewRecord::extraction_status"),
        ("provenanceSpans", "ReviewRecord::provenance_spans"),
        ("sampleBias", "ReviewRecord::sample_bias"),
    ];
    assert_eq!(
        keys,
        answered.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
        "section 29.5's ReviewRecord and this crate's accessors have diverged"
    );

    // And the block's nested `dimensions:` keys are this crate's nine.
    let nested: Vec<&str> = block[..end]
        .lines()
        .skip_while(|line| !line.starts_with("  dimensions:"))
        .skip(1)
        .take_while(|line| line.starts_with("    "))
        .filter_map(|line| line.trim().split_once(':'))
        .map(|(key, _)| key)
        .collect();
    assert_eq!(
        nested,
        ReviewDimension::ALL
            .into_iter()
            .map(ReviewDimension::spec_key)
            .collect::<Vec<_>>(),
        "section 29.5's dimension keys and ReviewDimension::ALL have diverged"
    );

    // The record the specification describes is the record this builds.
    let record = review(
        60,
        scope(SOURCE, 1, "Kim", "2026_1")?,
        "one review with one span over the whole of it",
        SourceAccessMode::UserProvidedExport,
        extraction_at(DimensionBand::Low)?,
    )?;
    assert_eq!(record.provenance_spans().len(), 1);
    assert_eq!(record.provenance_spans()[0].start(), 0);
    assert_eq!(
        record.provenance_spans()[0].len(),
        record.raw_artifact().byte_len()
    );
    assert!(record.sample_bias().signals().is_empty());
    assert_eq!(
        record.collected_at(),
        academic_ingestion::RetrievalInstant::at(1_700_000_060)
    );
    assert_eq!(
        ReviewRecord::EXTRACTION_STATUS,
        EpistemicStatus::AiInferred,
        "the record's status is no longer P2-M2's autosaved constant"
    );
    Ok(())
}

/// An extraction reads every dimension, once, and names the first that is
/// missing.
#[test]
fn the_extraction_reads_every_dimension() -> TestResult {
    for dropped in ReviewDimension::ALL {
        let readings: Vec<DimensionReading> = ReviewDimension::ALL
            .into_iter()
            .filter(|dimension| *dimension != dropped)
            .map(|dimension| DimensionReading::new(dimension, DimensionBand::Moderate, 0))
            .collect();
        assert_eq!(
            ReviewExtraction::read(&readings).err(),
            Some(ReviewError::DimensionMissing(dropped)),
            "an extraction without {} was accepted",
            dropped.as_str()
        );
    }
    for repeated in ReviewDimension::ALL {
        let mut readings: Vec<DimensionReading> = ReviewDimension::ALL
            .into_iter()
            .map(|dimension| DimensionReading::new(dimension, DimensionBand::Moderate, 0))
            .collect();
        readings.push(DimensionReading::new(repeated, DimensionBand::High, 0));
        assert_eq!(
            ReviewExtraction::read(&readings).err(),
            Some(ReviewError::DimensionRepeated(repeated)),
            "an extraction reading {} twice was accepted",
            repeated.as_str()
        );
    }

    // Each dimension keeps its own band through the extraction and the record.
    for moved in ReviewDimension::ALL {
        let record = review(
            70,
            scope(SOURCE, 1, "Kim", "2026_1")?,
            "one review whose bands differ by dimension",
            SourceAccessMode::Public,
            extraction_with(DimensionBand::Low, moved, DimensionBand::VeryHigh)?,
        )?;
        for dimension in ReviewDimension::ALL {
            let expected = if dimension == moved {
                DimensionBand::VeryHigh
            } else {
                DimensionBand::Low
            };
            assert_eq!(
                record.band(dimension),
                expected,
                "{} does not carry its own band when {} was moved",
                dimension.as_str(),
                moved.as_str()
            );
        }
    }
    Ok(())
}

/// Every index a direct lookup uses is that value's position in its own list.
#[test]
fn the_indexes_are_positions_in_their_own_lists() {
    for (position, dimension) in ReviewDimension::ALL.into_iter().enumerate() {
        assert_eq!(dimension.index(), position, "{}", dimension.as_str());
    }
    for (position, band) in DimensionBand::ALL.into_iter().enumerate() {
        assert_eq!(band.index(), position, "{}", band.as_str());
    }
    for (position, dimension) in BiasDimension::ALL.into_iter().enumerate() {
        assert_eq!(dimension.index(), position, "{}", dimension.as_str());
    }
}

/// A provenance span has to point at what it claims.
#[test]
fn a_provenance_span_is_checked_against_the_text() -> TestResult {
    let text = "the workload was heavy in the second half";
    let whole = academic_review::RawReviewText::digest_of(text.as_bytes());
    let part = academic_review::RawReviewText::digest_of("workload".as_bytes());

    assert!(academic_review::RawReviewText::retain(text, &[(4, 12, part.as_str())]).is_ok());
    assert_eq!(
        academic_review::RawReviewText::retain(text, &[(4, 12, whole.as_str())]).err(),
        Some(ReviewError::SpanDigestMismatch { start: 4, end: 12 }),
        "a span digesting to something else was retained"
    );
    assert_eq!(
        academic_review::RawReviewText::retain(text, &[(4, 400, part.as_str())]).err(),
        Some(ReviewError::SpanOutOfRange { start: 4, end: 400 })
    );
    assert_eq!(
        academic_review::RawReviewText::retain(text, &[(12, 4, part.as_str())]).err(),
        Some(ReviewError::SpanOutOfRange { start: 12, end: 4 })
    );
    assert_eq!(
        academic_review::RawReviewText::retain("", &[]).err(),
        Some(ReviewError::EmptyText)
    );

    // A span off a character boundary is refused rather than panicking.
    let korean = "강의가 좋았다";
    let inside = academic_review::RawReviewText::digest_of(&korean.as_bytes()[0..1]);
    assert_eq!(
        academic_review::RawReviewText::retain(korean, &[(0, 1, inside.as_str())]).err(),
        Some(ReviewError::SpanOutOfRange { start: 0, end: 1 })
    );
    Ok(())
}

/// The one section 38 cell this task leaves open says where it bites.
#[test]
fn the_open_gate_is_recorded_and_denies() -> TestResult {
    let gate = academic_review::OpenGate::PerSourceRights;
    assert_eq!(academic_review::OpenGate::ALL, [gate]);
    assert_eq!(gate.identifier(), "GATE-38-021");
    assert!(gate.to_string().contains("GATE-38-021"));
    assert!(!gate.question().is_empty());
    assert!(!gate.while_open().is_empty());

    // What "open" looks like: nothing recorded, so nothing permitted.
    for mode in SourceAccessMode::ALL {
        assert_eq!(
            permit(
                &SourceTermsLedger::empty(),
                &source("unconfigured.board")?,
                mode
            )
            .err()
            .map(|denial| denial.reason()),
            Some(DenialReason::TermsUnreviewed)
        );
    }
    Ok(())
}

/// The one public route out of a retained review is `P2-G5`'s label.
///
/// `seal` is what makes "the extraction a model performs can happen" true while
/// no `String` of somebody else's writing exists. It is driven here because an
/// accessor no test calls is an accessor whose behaviour nothing states: the
/// scan half says the route is the only one, and this says what comes back.
#[test]
fn the_one_public_route_out_is_the_untrusted_seal() -> TestResult {
    let text = "the reading list was long and the lectures were worth it";
    let retained = support::retained(text)?;
    assert_eq!(retained.byte_len(), text.len());
    assert_eq!(
        retained.digest(),
        academic_review::RawReviewText::digest_of(text.as_bytes()),
        "the retained digest is the digest of the retained bytes"
    );
    assert_eq!(retained.spans().len(), 1);
    assert!(!retained.spans()[0].is_empty());
    assert_eq!(
        retained.spans()[0].digest(),
        academic_review::RawReviewText::digest_of(text.as_bytes()),
        "the whole-text span digests to the whole text"
    );

    let sealed = retained.seal(SourceId::new("synthetic.review.1")?, 7)?;
    assert_eq!(sealed.provenance().kind(), SourceKind::ReviewText);
    assert_eq!(
        sealed.provenance().source_id().as_str(),
        "synthetic.review.1"
    );
    assert_eq!(sealed.provenance().ingest_seq(), 7);
    assert_eq!(sealed.byte_len(), text.len());
    // `P2-G5` computes its own SHA-256 over the same bytes; it is 64 hexadecimal
    // characters and is not this crate's FNV-128, which is 32. The two are
    // different answers to two different questions and the test says so rather
    // than comparing them.
    assert_eq!(sealed.digest().len(), 64);
    assert_eq!(retained.digest().len(), 32);
    assert_ne!(sealed.digest(), retained.digest());

    // The `Debug` of the retained artifact reaches the text through a length.
    let rendered = format!("{retained:?}");
    assert!(rendered.contains("<retained:"), "{rendered}");
    assert!(
        !rendered.contains("reading list"),
        "the retained Debug printed the review: {rendered}"
    );
    Ok(())
}

/// The accessors an aggregate, a disclosure and a record hand back.
///
/// Each of these is a public accessor the eight named tests do not call. An
/// accessor nothing drives is an accessor whose behaviour nothing states, and
/// this run has found guards that were exactly that.
#[test]
fn every_public_accessor_hands_back_what_it_names() -> TestResult {
    // The ledger reports what was recorded, per source and per mode.
    let ledger = support::ledger_permitting(SOURCE, SourceAccessMode::Public)?;
    assert_eq!(
        ledger.status_of(&source(SOURCE)?, SourceAccessMode::Public),
        TermsStatus::PermittedForDeclaredMethod
    );
    assert_eq!(
        ledger.status_of(&source(SOURCE)?, SourceAccessMode::ManualPaste),
        TermsStatus::Unreviewed
    );
    let permitted = collection(SOURCE, SourceAccessMode::Public)?;
    assert_eq!(permitted.mode(), SourceAccessMode::Public);
    assert_eq!(permitted.source(), &source(SOURCE)?);

    // Two of the three access modes are a person's act; the public page is not.
    for mode in SourceAccessMode::ALL {
        assert_eq!(
            mode.is_a_person_act(),
            mode != SourceAccessMode::Public,
            "{} is on the wrong side of section 29.5's fallback list",
            mode.as_str()
        );
    }

    // A record's sample bias is a set of the same six the aggregate discloses.
    let flagged = SampleBias::none()
        .flagging(BiasDimension::ExtremeExperience)
        .flagging(BiasDimension::SelfSelection)
        .flagging(BiasDimension::ExtremeExperience);
    assert_eq!(
        flagged.signals(),
        [
            BiasDimension::SelfSelection,
            BiasDimension::ExtremeExperience
        ],
        "a dimension flagged twice is recorded once, in BiasDimension::ALL order"
    );
    assert!(flagged.flags(BiasDimension::SelfSelection));
    assert!(!flagged.flags(BiasDimension::Duplication));

    // An extraction hands back one reading per dimension, in order, and each
    // reading names the span it was read from.
    let extraction = extraction_at(DimensionBand::High)?;
    assert_eq!(
        extraction
            .readings()
            .iter()
            .map(|reading| reading.dimension())
            .collect::<Vec<_>>(),
        ReviewDimension::ALL.to_vec()
    );
    for dimension in ReviewDimension::ALL {
        assert_eq!(extraction.reading(dimension).span_index(), 0);
    }

    // A disclosure hands back one finding per dimension, with what was counted.
    let disclosed = draft_disclosing(11, &BiasDimension::ALL).build()?;
    assert_eq!(
        disclosed
            .findings()
            .iter()
            .map(|finding| finding.dimension())
            .collect::<Vec<_>>(),
        BiasDimension::ALL.to_vec()
    );
    assert_eq!(disclosed.finding(BiasDimension::SampleCount).measured(), 11);
    assert_eq!(
        disclosed.finding(BiasDimension::Duplication).strength(),
        BiasStrength::Low
    );

    // An aggregate hands back a distribution per dimension, and a claim hands
    // back the scopes it was asserted over.
    let record = review(
        80,
        scope(SOURCE, 1, "Kim", "2026_1")?,
        "one review of one offering",
        SourceAccessMode::Public,
        extraction_at(DimensionBand::High)?,
    )?;
    let aggregate = OfferingAggregate::over(core::slice::from_ref(&record), disclosure(1)?)?;
    assert_eq!(
        aggregate
            .distributions()
            .iter()
            .map(|distribution| distribution.dimension())
            .collect::<Vec<_>>(),
        ReviewDimension::ALL.to_vec()
    );
    assert_eq!(
        aggregate.distribution(ReviewDimension::Difficulty).counts(),
        [0, 0, 0, 1, 0],
        "one High reading sits in the fourth band and nowhere else"
    );
    assert_eq!(aggregate.instructor(), aggregate.scope().instructor());
    assert_eq!(aggregate.term(), aggregate.scope().term());
    assert_eq!(aggregate.offering(), aggregate.scope().offering());

    let course = support::course_id(11)?;
    let claim = AggregationClaim::asserting(
        AggregationMethod::PerOfferingListing,
        course,
        core::slice::from_ref(&aggregate),
    );
    assert_eq!(claim.asserted_over(), [aggregate.scope().clone()]);
    let promoted =
        CourseAggregate::promote(claim, core::slice::from_ref(&aggregate), disclosure(1)?)?;
    assert_eq!(promoted.over(), [aggregate.scope().clone()]);
    let CourseReading::PerOffering { offerings } = promoted.reading() else {
        return Err("PerOfferingListing produced a pooled reading".into());
    };
    assert_eq!(offerings[0].sample_size(), 1);
    assert_eq!(offerings[0].distributions(), aggregate.distributions());
    Ok(())
}

// ---------------------------------------------------------------------------
// the_retained_text_bound_and_the_short_review_arm -- REQ-24-012
// ---------------------------------------------------------------------------

/// The two guards `P2-A3` found undriven in this crate.
///
/// Both were removed one at a time and the whole `academic-review` suite passed
/// each time: nothing in any corpus is longer than `MAX_REVIEW_BYTES`, and no
/// two reviews in any corpus are shorter than three words.
///
/// * `RawReviewText::retain`'s length bound is what makes the boundary's own
///   `MAX_SOURCE_BYTES` a fact about every value of this type rather than about
///   the values somebody happened to build, so `seal` can always hand one over.
/// * `shingles`' short-review arm yields one shingle holding all the words, so
///   two short reviews compare against **each other**. Without it, `windows(3)`
///   over fewer than three words yields nothing, every short review is the
///   empty set, and two identical short reviews read as 0 permille similar --
///   duplicate detection silently stops seeing exactly the reviews a person is
///   most likely to paste twice.
#[test]
fn the_retained_text_bound_and_the_short_review_arm() -> TestResult {
    // 1. The length bound. One byte under and one byte over.
    let inside = "x".repeat(academic_review::MAX_REVIEW_BYTES);
    assert!(
        academic_review::RawReviewText::retain(&inside, &[]).is_ok(),
        "a text exactly at the bound was refused"
    );
    let over = "x".repeat(academic_review::MAX_REVIEW_BYTES + 1);
    assert_eq!(
        academic_review::RawReviewText::retain(&over, &[]).err(),
        Some(ReviewError::TextTooLong(
            academic_review::MAX_REVIEW_BYTES + 1
        )),
        "a text over the bound was retained"
    );

    // 2. The short-review arm. Two identical two-word reviews are duplicates of
    //    each other, and two different ones are not.
    let short = |suffix: u64, text: &str| {
        review(
            suffix,
            scope(SOURCE, 1, "Kim", "2026_1")?,
            text,
            SourceAccessMode::ManualPaste,
            extraction_at(DimensionBand::Moderate)?,
        )
    };
    let one = short(61, "workload heavy")?;
    let same = short(62, "workload heavy")?;
    let other = short(63, "grading arbitrary")?;

    assert_eq!(
        similarity(&one, &same).value(),
        1000,
        "two identical two-word reviews did not read as duplicates"
    );
    assert_eq!(
        similarity(&one, &other).value(),
        0,
        "two different two-word reviews read as duplicates"
    );
    Ok(())
}
