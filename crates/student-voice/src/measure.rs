//! The scorer, the number it produces, and the witness that number can become.
//!
//! # The number has one producer and it walks a corpus
//!
//! [`DiarizationMeasurement`] has private fields, no setter, no `Default` and
//! no constructor taking a figure. Its one producer is [`measure`], which reads
//! a [`DiarizationCorpus`](crate::DiarizationCorpus) and nothing else. That is
//! `P2-L4`'s `CompletenessWitness` shape applied to an accuracy: the value with
//! no measurement behind it is not representable, rather than being a default
//! somebody is expected not to use.
//!
//! # The partition is the check that the scorer is not lying
//!
//! Every millisecond of reference time lands in exactly one of five buckets:
//! agreed, student-labelled-instructor, instructor-labelled-student,
//! unattributed, and uncovered. [`DiarizationMeasurement::partition_reconciles`]
//! is that statement, and the acceptance suite asserts it over every case and
//! over the fold. A scorer that double-counts an overlap or drops a hole fails
//! it rather than reporting a slightly wrong ratio nobody can see.
//!
//! # Two axes, because they are two failures
//!
//! Attribution accuracy is one number and the fraction of student speech
//! labelled instructor is another, and a corpus can be good at the first while
//! being bad at the second -- a lecture is mostly the instructor, so mislabeling
//! every student utterance costs a few permille of accuracy and costs all of the
//! privacy. [`DiarizationThreshold`] therefore has two fields and
//! [`AccuracyRefusal`] has two variants.
//!
//! # Configuration cannot empty the guard
//!
//! The threshold is configuration -- which number is enough is a user and
//! institution decision, and `GATE-38-026` says so. But a configuration that
//! can be set to zero is a guard a profile can delete, and the subject here is
//! somebody who never consented. So [`DiarizationThreshold::new`] refuses
//! anything below [`ABSOLUTE_ACCURACY_FLOOR`] or above
//! [`ABSOLUTE_MISSED_STUDENT_CEILING`]. Inside that band the number is the
//! user's; outside it there is no value at all.
//!
//! # No floating point
//!
//! Every ratio here is permille computed in `u64`. `academic-record` fixed that
//! rule for money and it holds for the same reason here: a number that decides
//! whether somebody's voice may be processed automatically should not depend on
//! a rounding mode.

use academic_domain::ContentDigest;

use crate::{
    corpus::{DiarizationCase, DiarizationCorpus, VoiceClass, VoiceSpan},
    fault::{AccuracyRefusal, ThresholdFault},
};

/// The version of the scoring rule this build implements.
///
/// It is part of every measurement, because a number taken under one scoring
/// rule and compared against a threshold chosen for another is not a
/// comparison. Bumping it invalidates every committed `.expected` file.
pub const SCORER_VERSION: u32 = 1;

/// The lowest attribution accuracy any configuration may require.
///
/// Below this there is no configuration, because there is nobody in the loop:
/// the people whose speech an automatic redaction is about did not choose the
/// profile.
pub const ABSOLUTE_ACCURACY_FLOOR: u64 = 900;

/// The most student speech any configuration may allow to be labelled
/// instructor.
pub const ABSOLUTE_MISSED_STUDENT_CEILING: u64 = 50;

/// How good a diarizer has to be before it may choose the spans.
///
/// Versioned configuration with recorded defaults, in `P2-L4`'s sense: a
/// threshold that can be superseded and dated is a decision a user makes per
/// profile. What is not configuration is the band it may move in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiarizationThreshold {
    version: u32,
    min_accuracy_permille: u64,
    max_missed_student_permille: u64,
}

impl DiarizationThreshold {
    /// States a threshold.
    ///
    /// # Errors
    ///
    /// [`ThresholdFault::AccuracyFloorIsBinding`] below
    /// [`ABSOLUTE_ACCURACY_FLOOR`], [`ThresholdFault::MissedStudentCeilingIsBinding`]
    /// above [`ABSOLUTE_MISSED_STUDENT_CEILING`], and
    /// [`ThresholdFault::AccuracyIsNotAPermille`] above 1000.
    pub const fn new(
        version: u32,
        min_accuracy_permille: u64,
        max_missed_student_permille: u64,
    ) -> Result<Self, ThresholdFault> {
        if min_accuracy_permille > 1000 {
            return Err(ThresholdFault::AccuracyIsNotAPermille {
                stated: min_accuracy_permille,
            });
        }
        if min_accuracy_permille < ABSOLUTE_ACCURACY_FLOOR {
            return Err(ThresholdFault::AccuracyFloorIsBinding {
                stated: min_accuracy_permille,
                floor: ABSOLUTE_ACCURACY_FLOOR,
            });
        }
        if max_missed_student_permille > ABSOLUTE_MISSED_STUDENT_CEILING {
            return Err(ThresholdFault::MissedStudentCeilingIsBinding {
                stated: max_missed_student_permille,
                ceiling: ABSOLUTE_MISSED_STUDENT_CEILING,
            });
        }
        Ok(Self {
            version,
            min_accuracy_permille,
            max_missed_student_permille,
        })
    }

    /// Which published configuration.
    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }

    /// The attribution accuracy it requires.
    #[must_use]
    pub const fn min_accuracy_permille(self) -> u64 {
        self.min_accuracy_permille
    }

    /// The student speech it allows to be labelled instructor.
    #[must_use]
    pub const fn max_missed_student_permille(self) -> u64 {
        self.max_missed_student_permille
    }
}

/// The recorded default.
///
/// `990` and `0`. The contract page carries the reasoning and
/// `the_recorded_defaults_are_the_documented_ones` reads this constant back out
/// of it, so a number changed here and left undocumented fails.
pub const DIARIZATION_THRESHOLD_V1: DiarizationThreshold = DiarizationThreshold {
    version: 1,
    min_accuracy_permille: 990,
    max_missed_student_permille: 0,
};

/// One case's score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseMeasurement {
    case: String,
    scored_ms: u64,
    agreed_ms: u64,
    student_as_instructor_ms: u64,
    instructor_as_student_ms: u64,
    unattributed_ms: u64,
    uncovered_ms: u64,
    reference_student_ms: u64,
    student_agreed_ms: u64,
}

impl CaseMeasurement {
    /// Which case.
    #[must_use]
    pub fn case(&self) -> &str {
        &self.case
    }

    /// Reference milliseconds scored.
    #[must_use]
    pub const fn scored_ms(&self) -> u64 {
        self.scored_ms
    }

    /// Milliseconds the hypothesis put in the reference's class.
    #[must_use]
    pub const fn agreed_ms(&self) -> u64 {
        self.agreed_ms
    }

    /// Student milliseconds the hypothesis called instructor.
    #[must_use]
    pub const fn student_as_instructor_ms(&self) -> u64 {
        self.student_as_instructor_ms
    }

    /// Instructor milliseconds the hypothesis called student.
    #[must_use]
    pub const fn instructor_as_student_ms(&self) -> u64 {
        self.instructor_as_student_ms
    }

    /// Milliseconds the hypothesis covered and declined to attribute.
    #[must_use]
    pub const fn unattributed_ms(&self) -> u64 {
        self.unattributed_ms
    }

    /// Milliseconds no hypothesis span covers at all.
    #[must_use]
    pub const fn uncovered_ms(&self) -> u64 {
        self.uncovered_ms
    }

    /// Student milliseconds in the reference.
    #[must_use]
    pub const fn reference_student_ms(&self) -> u64 {
        self.reference_student_ms
    }

    /// Student milliseconds the hypothesis also called student.
    #[must_use]
    pub const fn student_agreed_ms(&self) -> u64 {
        self.student_agreed_ms
    }

    /// Whether the five buckets add up to the scored time.
    #[must_use]
    pub const fn partition_reconciles(&self) -> bool {
        self.agreed_ms
            + self.student_as_instructor_ms
            + self.instructor_as_student_ms
            + self.unattributed_ms
            + self.uncovered_ms
            == self.scored_ms
    }

    /// The case's line in the corpus grammar.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut text = format!("diarization-measurement/1 {}\n", self.case);
        for (key, value) in [
            ("scored_ms", self.scored_ms),
            ("agreed_ms", self.agreed_ms),
            ("student_as_instructor_ms", self.student_as_instructor_ms),
            ("instructor_as_student_ms", self.instructor_as_student_ms),
            ("unattributed_ms", self.unattributed_ms),
            ("uncovered_ms", self.uncovered_ms),
            ("reference_student_ms", self.reference_student_ms),
            ("student_agreed_ms", self.student_agreed_ms),
        ] {
            text.push_str(key);
            text.push('=');
            text.push_str(&value.to_string());
            text.push('\n');
        }
        text.into_bytes()
    }
}

/// What a corpus measured, and which corpus it was.
///
/// Private fields, no setter, one producer. There is no constructor that takes
/// an accuracy: a figure quoted from anywhere but a run over committed bytes
/// has no value of this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiarizationMeasurement {
    corpus_id: String,
    corpus_version: u32,
    corpus_digest: ContentDigest,
    scorer_version: u32,
    cases: Vec<CaseMeasurement>,
    scored_ms: u64,
    agreed_ms: u64,
    student_as_instructor_ms: u64,
    instructor_as_student_ms: u64,
    unattributed_ms: u64,
    uncovered_ms: u64,
    reference_student_ms: u64,
    student_agreed_ms: u64,
}

impl DiarizationMeasurement {
    /// Which corpus.
    #[must_use]
    pub fn corpus_id(&self) -> &str {
        &self.corpus_id
    }

    /// Which version of it.
    #[must_use]
    pub const fn corpus_version(&self) -> u32 {
        self.corpus_version
    }

    /// The digest of the bytes that were scored.
    #[must_use]
    pub const fn corpus_digest(&self) -> &ContentDigest {
        &self.corpus_digest
    }

    /// Which scoring rule produced it.
    #[must_use]
    pub const fn scorer_version(&self) -> u32 {
        self.scorer_version
    }

    /// Every case's score, in corpus order.
    #[must_use]
    pub fn cases(&self) -> &[CaseMeasurement] {
        &self.cases
    }

    /// Reference milliseconds scored.
    #[must_use]
    pub const fn scored_ms(&self) -> u64 {
        self.scored_ms
    }

    /// Milliseconds the hypothesis put in the reference's class.
    #[must_use]
    pub const fn agreed_ms(&self) -> u64 {
        self.agreed_ms
    }

    /// Student milliseconds the hypothesis called instructor.
    #[must_use]
    pub const fn student_as_instructor_ms(&self) -> u64 {
        self.student_as_instructor_ms
    }

    /// Instructor milliseconds the hypothesis called student.
    #[must_use]
    pub const fn instructor_as_student_ms(&self) -> u64 {
        self.instructor_as_student_ms
    }

    /// Milliseconds the hypothesis covered and declined to attribute.
    #[must_use]
    pub const fn unattributed_ms(&self) -> u64 {
        self.unattributed_ms
    }

    /// Milliseconds no hypothesis span covers at all.
    #[must_use]
    pub const fn uncovered_ms(&self) -> u64 {
        self.uncovered_ms
    }

    /// Student milliseconds in the reference. The privacy axis's denominator.
    #[must_use]
    pub const fn reference_student_ms(&self) -> u64 {
        self.reference_student_ms
    }

    /// Student milliseconds the hypothesis also called student.
    #[must_use]
    pub const fn student_agreed_ms(&self) -> u64 {
        self.student_agreed_ms
    }

    /// Attribution accuracy, in permille, floored.
    #[must_use]
    pub const fn accuracy_permille(&self) -> u64 {
        permille(self.agreed_ms, self.scored_ms)
    }

    /// The fraction of student speech an automatic redaction would leave in.
    ///
    /// The denominator is the reference's student milliseconds, which a corpus
    /// cannot have none of.
    #[must_use]
    pub const fn missed_student_permille(&self) -> u64 {
        permille(self.student_as_instructor_ms, self.reference_student_ms)
    }

    /// The fraction of student speech the hypothesis also called student.
    #[must_use]
    pub const fn student_recall_permille(&self) -> u64 {
        permille(self.student_agreed_ms, self.reference_student_ms)
    }

    /// Whether the five buckets add up to the scored time.
    #[must_use]
    pub const fn partition_reconciles(&self) -> bool {
        self.agreed_ms
            + self.student_as_instructor_ms
            + self.instructor_as_student_ms
            + self.unattributed_ms
            + self.uncovered_ms
            == self.scored_ms
    }

    /// The whole measurement as bytes: the identity, then every case, then the
    /// fold and the two ratios.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut text = format!(
            "diarization-measurement-set/1 {} {} {} {}\n",
            self.corpus_id, self.corpus_version, self.corpus_digest, self.scorer_version
        );
        for case in &self.cases {
            text.push_str(&String::from_utf8_lossy(&case.canonical_bytes()));
        }
        for (key, value) in [
            ("total_scored_ms", self.scored_ms),
            ("total_agreed_ms", self.agreed_ms),
            (
                "total_student_as_instructor_ms",
                self.student_as_instructor_ms,
            ),
            (
                "total_instructor_as_student_ms",
                self.instructor_as_student_ms,
            ),
            ("total_unattributed_ms", self.unattributed_ms),
            ("total_uncovered_ms", self.uncovered_ms),
            ("total_reference_student_ms", self.reference_student_ms),
            ("total_student_agreed_ms", self.student_agreed_ms),
            ("accuracy_permille", self.accuracy_permille()),
            ("missed_student_permille", self.missed_student_permille()),
            ("student_recall_permille", self.student_recall_permille()),
        ] {
            text.push_str(key);
            text.push('=');
            text.push_str(&value.to_string());
            text.push('\n');
        }
        text.into_bytes()
    }

    /// The witness this measurement is, if it clears `threshold`.
    ///
    /// The one producer of an [`AccuracyWitness`]. Both axes are checked and
    /// the accuracy one is checked first, so a measurement that fails both
    /// names the accuracy.
    ///
    /// # Errors
    ///
    /// [`AccuracyRefusal::AccuracyBelowThreshold`] or
    /// [`AccuracyRefusal::MissedStudentSpeechAboveThreshold`].
    pub fn witness(
        &self,
        threshold: DiarizationThreshold,
    ) -> Result<AccuracyWitness, AccuracyRefusal> {
        let accuracy = self.accuracy_permille();
        if accuracy < threshold.min_accuracy_permille {
            return Err(AccuracyRefusal::AccuracyBelowThreshold {
                measured: accuracy,
                required: threshold.min_accuracy_permille,
            });
        }
        let missed = self.missed_student_permille();
        if missed > threshold.max_missed_student_permille {
            return Err(AccuracyRefusal::MissedStudentSpeechAboveThreshold {
                measured: missed,
                allowed: threshold.max_missed_student_permille,
            });
        }
        Ok(AccuracyWitness {
            corpus_id: self.corpus_id.clone(),
            corpus_version: self.corpus_version,
            corpus_digest: self.corpus_digest,
            scorer_version: self.scorer_version,
            threshold,
            accuracy_permille: accuracy,
            missed_student_permille: missed,
        })
    }
}

/// A measurement that cleared a threshold.
///
/// Private fields, no public constructor, no `Default`. Its one producer is
/// [`DiarizationMeasurement::witness`], and the whole `impl` block is pinned by
/// `a_witness_has_one_producer`, so a second route is an edit to a constant.
/// [`crate::RedactionMode::Automatic`] takes one **by value**: an automatic
/// redaction claim without a measurement is not a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccuracyWitness {
    corpus_id: String,
    corpus_version: u32,
    corpus_digest: ContentDigest,
    scorer_version: u32,
    threshold: DiarizationThreshold,
    accuracy_permille: u64,
    missed_student_permille: u64,
}

impl AccuracyWitness {
    /// Which corpus the number came from.
    #[must_use]
    pub fn corpus_id(&self) -> &str {
        &self.corpus_id
    }

    /// Which version of it.
    #[must_use]
    pub const fn corpus_version(&self) -> u32 {
        self.corpus_version
    }

    /// The digest of the bytes that were scored.
    #[must_use]
    pub const fn corpus_digest(&self) -> &ContentDigest {
        &self.corpus_digest
    }

    /// Which scoring rule.
    #[must_use]
    pub const fn scorer_version(&self) -> u32 {
        self.scorer_version
    }

    /// The threshold it cleared, whole, so a weak configuration is visible.
    #[must_use]
    pub const fn threshold(&self) -> DiarizationThreshold {
        self.threshold
    }

    /// The measured attribution accuracy.
    #[must_use]
    pub const fn accuracy_permille(&self) -> u64 {
        self.accuracy_permille
    }

    /// The measured missed-student fraction.
    #[must_use]
    pub const fn missed_student_permille(&self) -> u64 {
        self.missed_student_permille
    }
}

/// Scores a whole corpus.
///
/// The only producer of a [`DiarizationMeasurement`].
#[must_use]
pub fn measure(corpus: &DiarizationCorpus) -> DiarizationMeasurement {
    let cases: Vec<CaseMeasurement> = corpus.cases().iter().map(measure_case).collect();
    let mut measurement = DiarizationMeasurement {
        corpus_id: corpus.id().to_owned(),
        corpus_version: corpus.version(),
        corpus_digest: corpus.digest(),
        scorer_version: SCORER_VERSION,
        scored_ms: 0,
        agreed_ms: 0,
        student_as_instructor_ms: 0,
        instructor_as_student_ms: 0,
        unattributed_ms: 0,
        uncovered_ms: 0,
        reference_student_ms: 0,
        student_agreed_ms: 0,
        cases,
    };
    for case in &measurement.cases {
        measurement.scored_ms = measurement.scored_ms.saturating_add(case.scored_ms);
        measurement.agreed_ms = measurement.agreed_ms.saturating_add(case.agreed_ms);
        measurement.student_as_instructor_ms = measurement
            .student_as_instructor_ms
            .saturating_add(case.student_as_instructor_ms);
        measurement.instructor_as_student_ms = measurement
            .instructor_as_student_ms
            .saturating_add(case.instructor_as_student_ms);
        measurement.unattributed_ms = measurement
            .unattributed_ms
            .saturating_add(case.unattributed_ms);
        measurement.uncovered_ms = measurement.uncovered_ms.saturating_add(case.uncovered_ms);
        measurement.reference_student_ms = measurement
            .reference_student_ms
            .saturating_add(case.reference_student_ms);
        measurement.student_agreed_ms = measurement
            .student_agreed_ms
            .saturating_add(case.student_agreed_ms);
    }
    measurement
}

/// Scores one case.
///
/// Every reference millisecond is attributed to exactly one bucket: the
/// hypothesis spans it overlaps decide the first four, and whatever is left
/// over is uncovered. That residual is why a diarizer that simply says nothing
/// cannot score well.
#[must_use]
pub fn measure_case(case: &DiarizationCase) -> CaseMeasurement {
    let mut measured = CaseMeasurement {
        case: case.name().to_owned(),
        scored_ms: 0,
        agreed_ms: 0,
        student_as_instructor_ms: 0,
        instructor_as_student_ms: 0,
        unattributed_ms: 0,
        uncovered_ms: 0,
        reference_student_ms: 0,
        student_agreed_ms: 0,
    };
    for reference in case.reference() {
        let reference_class = VoiceClass::of(reference.speaker());
        let duration = reference.duration_ms();
        measured.scored_ms = measured.scored_ms.saturating_add(duration);
        if reference_class == VoiceClass::Student {
            measured.reference_student_ms = measured.reference_student_ms.saturating_add(duration);
        }
        let mut covered = 0_u64;
        for hypothesis in case.hypothesis() {
            let overlap = reference.overlap_ms(*hypothesis);
            if overlap == 0 {
                continue;
            }
            covered = covered.saturating_add(overlap);
            attribute(&mut measured, reference_class, *hypothesis, overlap);
        }
        measured.uncovered_ms = measured.uncovered_ms.saturating_add(duration - covered);
    }
    measured
}

/// Puts one overlap in one bucket.
///
/// A total `match` over the pair of classes. The `(Unattributed, _)` reference
/// arm is unreachable because a corpus refuses an unresolved reference, and it
/// is spelled rather than defaulted so the compiler still enumerates the shape.
fn attribute(
    measured: &mut CaseMeasurement,
    reference_class: VoiceClass,
    hypothesis: VoiceSpan,
    overlap: u64,
) {
    match (reference_class, VoiceClass::of(hypothesis.speaker())) {
        (VoiceClass::Instructor, VoiceClass::Instructor) => {
            measured.agreed_ms = measured.agreed_ms.saturating_add(overlap);
        }
        (VoiceClass::Student, VoiceClass::Student) => {
            measured.agreed_ms = measured.agreed_ms.saturating_add(overlap);
            measured.student_agreed_ms = measured.student_agreed_ms.saturating_add(overlap);
        }
        (VoiceClass::Student, VoiceClass::Instructor) => {
            measured.student_as_instructor_ms =
                measured.student_as_instructor_ms.saturating_add(overlap);
        }
        (VoiceClass::Instructor, VoiceClass::Student) => {
            measured.instructor_as_student_ms =
                measured.instructor_as_student_ms.saturating_add(overlap);
        }
        (VoiceClass::Instructor | VoiceClass::Student, VoiceClass::Unattributed) => {
            measured.unattributed_ms = measured.unattributed_ms.saturating_add(overlap);
        }
        (VoiceClass::Unattributed, _) => {
            measured.uncovered_ms = measured.uncovered_ms.saturating_add(overlap);
        }
    }
}

/// `numerator / denominator` in permille, floored, with a zero denominator
/// answering zero.
///
/// A zero denominator is unreachable for the two ratios this crate publishes --
/// a corpus with no reference time has no cases and one with no student speech
/// is refused -- and it answers zero rather than panicking because a panic in a
/// pure function is the one behaviour the engine discipline forbids.
const fn permille(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    numerator.saturating_mul(1000) / denominator
}
