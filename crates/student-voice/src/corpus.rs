//! The named, versioned diarization corpus, and why it is synthetic.
//!
//! # A number without a corpus is an estimate
//!
//! `GATE-38-026` asks whether diarization accuracy is sufficient to remove
//! student voices. The only answer this repository accepts is a measurement,
//! so the number this crate publishes is a function of committed bytes: a
//! corpus with an identifier, a version and a digest over its whole content.
//! Change one millisecond of one case and the digest changes, so a measurement
//! carrying that digest names exactly the corpus it was taken on.
//!
//! There is no constant holding an accuracy, no field a caller sets and no
//! literature figure anywhere in this crate. [`crate::DiarizationMeasurement`]
//! has one producer and it walks a corpus.
//!
//! # The corpus is synthetic and has to be
//!
//! `CONTRIBUTING.md` rule 1 forbids lecture media in this repository, so every
//! case here is a pair of committed timelines rather than audio. That bounds
//! what the number is evidence for and the contract page says so: it measures
//! the **scorer** against a stated ground truth, not a speech engine against a
//! room. What it does support is the fail-closed rule, because the rule is
//! about what happens when a number is low, and a low number is exactly what a
//! synthetic corpus can state exactly.
//!
//! # A reference cannot be unresolved
//!
//! The reference timeline is what was actually said. `Speaker::Unresolved` is
//! a *provider's* refusal to attribute, so admitting it into the reference
//! would score every hypothesis as correct on that span -- the `P2-L3` shape
//! where an oracle reads its expected value out of the thing it is checking.
//! [`CorpusFault::UnresolvedInReference`] refuses it.
//!
//! # A corpus with no student speech is not a corpus
//!
//! The figure that governs the fail-closed rule is how much student speech an
//! automatic redaction would leave in. Its denominator is the reference's
//! student milliseconds, so a corpus without any would divide by zero and, on
//! any convention that treats an empty ratio as zero, would report a perfect
//! score. [`CorpusFault::NoStudentSpeech`] refuses the whole corpus rather than
//! the case, because the property is about the corpus.

use academic_domain::ContentDigest;
use academic_transcription::Speaker;

use crate::fault::CorpusFault;

/// This corpus's stable identity. It is part of every measurement.
pub const CORPUS_ID: &str = "student-voice-diarization";

/// The version of [`CORPUS_ID`] this build ships.
///
/// A version is bumped when the case set changes. The digest catches the change
/// either way; the version is what a person reads.
pub const CORPUS_VERSION: u32 = 1;

/// The corpus root every version's directory sits under.
pub const CORPUS_ROOT: &str = "testdata/diarization";

/// A half-open span of a recording attributed to one voice.
///
/// Milliseconds, because this crate holds no clock and every number in it is a
/// committed literal. The end is exclusive, which is `academic-domain`'s
/// half-open interval rule applied to a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoiceSpan {
    start_ms: u64,
    end_ms: u64,
    speaker: Speaker,
}

impl VoiceSpan {
    /// States one span. Validation belongs to the timeline, which is where
    /// order and overlap can be seen.
    #[must_use]
    pub const fn new(start_ms: u64, end_ms: u64, speaker: Speaker) -> Self {
        Self {
            start_ms,
            end_ms,
            speaker,
        }
    }

    /// Inclusive start.
    #[must_use]
    pub const fn start_ms(self) -> u64 {
        self.start_ms
    }

    /// Exclusive end.
    #[must_use]
    pub const fn end_ms(self) -> u64 {
        self.end_ms
    }

    /// Who this span attributes the speech to.
    #[must_use]
    pub const fn speaker(self) -> Speaker {
        self.speaker
    }

    /// How long it is.
    #[must_use]
    pub const fn duration_ms(self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// How many milliseconds this span shares with `other`.
    #[must_use]
    pub const fn overlap_ms(self, other: Self) -> u64 {
        let start = if self.start_ms > other.start_ms {
            self.start_ms
        } else {
            other.start_ms
        };
        let end = if self.end_ms < other.end_ms {
            self.end_ms
        } else {
            other.end_ms
        };
        end.saturating_sub(start)
    }
}

/// Which of the two classes a redaction cares about a speaker falls in.
///
/// The redaction question is binary -- is this the instructor or is it somebody
/// who did not consent -- and `Speaker` has three shapes, so the fold is
/// written once here rather than at each comparison. `Unresolved` is neither
/// class: it is the provider declining, and a fail-closed reader treats it as
/// unattributed rather than as instructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VoiceClass {
    /// The instructor.
    Instructor,
    /// A student, whichever ordinal distinguished them.
    Student,
    /// The provider attributed the speech to nobody.
    Unattributed,
}

impl VoiceClass {
    /// Every class, in the order a measurement reports them.
    pub const ALL: [Self; 3] = [Self::Instructor, Self::Student, Self::Unattributed];

    /// The class a section 12.4 speaker falls in.
    ///
    /// A total `match` over `Speaker`, so a fourth speaker shape stops this
    /// crate compiling until it is classified.
    #[must_use]
    pub const fn of(speaker: Speaker) -> Self {
        match speaker {
            Speaker::Instructor => Self::Instructor,
            Speaker::StudentUnknown(_) => Self::Student,
            Speaker::Unresolved => Self::Unattributed,
        }
    }

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instructor => "INSTRUCTOR",
            Self::Student => "STUDENT",
            Self::Unattributed => "UNATTRIBUTED",
        }
    }
}

/// One case: what was said, and what a diarizer said about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiarizationCase {
    name: String,
    reference: Vec<VoiceSpan>,
    hypothesis: Vec<VoiceSpan>,
}

impl DiarizationCase {
    /// Admits one case, or says why it is not one.
    ///
    /// # Errors
    ///
    /// Every variant of [`CorpusFault`] except [`CorpusFault::NoStudentSpeech`],
    /// [`CorpusFault::DuplicateCase`] and [`CorpusFault::EmptyCorpus`], which
    /// are properties of a corpus rather than of a case.
    pub fn new(
        name: &str,
        reference: Vec<VoiceSpan>,
        hypothesis: Vec<VoiceSpan>,
    ) -> Result<Self, CorpusFault> {
        check_timeline(name, "reference", &reference)?;
        check_timeline(name, "hypothesis", &hypothesis)?;
        if reference
            .iter()
            .any(|span| VoiceClass::of(span.speaker) == VoiceClass::Unattributed)
        {
            return Err(CorpusFault::UnresolvedInReference {
                case: name.to_owned(),
            });
        }
        let reference_end = reference.last().map_or(0, |span| span.end_ms);
        let reference_start = reference.first().map_or(0, |span| span.start_ms);
        let outside = hypothesis
            .first()
            .is_some_and(|span| span.start_ms < reference_start)
            || hypothesis
                .last()
                .is_some_and(|span| span.end_ms > reference_end);
        if outside {
            return Err(CorpusFault::HypothesisOutsideReference {
                case: name.to_owned(),
            });
        }
        Ok(Self {
            name: name.to_owned(),
            reference,
            hypothesis,
        })
    }

    /// The case name, which is also its `.input`/`.expected` stem.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What was actually said.
    #[must_use]
    pub fn reference(&self) -> &[VoiceSpan] {
        &self.reference
    }

    /// What the diarizer said about it.
    #[must_use]
    pub fn hypothesis(&self) -> &[VoiceSpan] {
        &self.hypothesis
    }

    /// How much reference time this case is scored over.
    #[must_use]
    pub fn reference_ms(&self) -> u64 {
        self.reference
            .iter()
            .map(|span| span.duration_ms())
            .fold(0, u64::saturating_add)
    }

    /// How much of it is student speech.
    #[must_use]
    pub fn reference_student_ms(&self) -> u64 {
        self.reference
            .iter()
            .filter(|span| VoiceClass::of(span.speaker) == VoiceClass::Student)
            .map(|span| span.duration_ms())
            .fold(0, u64::saturating_add)
    }

    /// The case's own bytes, in the corpus grammar.
    ///
    /// One line per span, `timeline start end speaker`, so the file a reader
    /// opens is the value the scorer read.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut text = format!("diarization-case/1 {}\n", self.name);
        for span in &self.reference {
            push_span(&mut text, "reference", *span);
        }
        for span in &self.hypothesis {
            push_span(&mut text, "hypothesis", *span);
        }
        text.into_bytes()
    }
}

fn push_span(text: &mut String, timeline: &str, span: VoiceSpan) {
    text.push_str(timeline);
    text.push(' ');
    text.push_str(&span.start_ms.to_string());
    text.push(' ');
    text.push_str(&span.end_ms.to_string());
    text.push(' ');
    text.push_str(&span.speaker.spelling());
    text.push('\n');
}

fn check_timeline(
    case: &str,
    timeline: &'static str,
    spans: &[VoiceSpan],
) -> Result<(), CorpusFault> {
    if spans.is_empty() {
        return Err(CorpusFault::EmptyTimeline {
            case: case.to_owned(),
            timeline,
        });
    }
    let mut previous_end = None;
    for (index, span) in spans.iter().enumerate() {
        if span.end_ms <= span.start_ms {
            return Err(CorpusFault::EmptySpan {
                case: case.to_owned(),
                timeline,
                index,
            });
        }
        if previous_end.is_some_and(|end| span.start_ms < end) {
            return Err(CorpusFault::OverlappingSpan {
                case: case.to_owned(),
                timeline,
                index,
            });
        }
        previous_end = Some(span.end_ms);
    }
    Ok(())
}

/// A named, versioned set of cases.
///
/// The identity a measurement carries is `(id, version, digest)`. The first two
/// are what a person names it by and the third is what makes the naming
/// checkable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiarizationCorpus {
    id: String,
    version: u32,
    cases: Vec<DiarizationCase>,
}

impl DiarizationCorpus {
    /// Admits a corpus, or says why it is not one.
    ///
    /// # Errors
    ///
    /// [`CorpusFault::EmptyCorpus`] for no cases,
    /// [`CorpusFault::DuplicateCase`] for a repeated name, and
    /// [`CorpusFault::NoStudentSpeech`] when no case's reference holds student
    /// speech -- the measurement whose denominator that is could not be taken.
    pub fn new(id: &str, version: u32, cases: Vec<DiarizationCase>) -> Result<Self, CorpusFault> {
        if cases.is_empty() {
            return Err(CorpusFault::EmptyCorpus);
        }
        for (index, case) in cases.iter().enumerate() {
            if cases[..index].iter().any(|other| other.name == case.name) {
                return Err(CorpusFault::DuplicateCase {
                    case: case.name.clone(),
                });
            }
        }
        let student_ms = cases
            .iter()
            .map(DiarizationCase::reference_student_ms)
            .fold(0, u64::saturating_add);
        if student_ms == 0 {
            return Err(CorpusFault::NoStudentSpeech);
        }
        Ok(Self {
            id: id.to_owned(),
            version,
            cases,
        })
    }

    /// Which corpus.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Which version of it.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Its cases, in the order they are scored.
    #[must_use]
    pub fn cases(&self) -> &[DiarizationCase] {
        &self.cases
    }

    /// The whole corpus as bytes: the banner, then every case in order.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("diarization-corpus/1 {} {}\n", self.id, self.version).into_bytes();
        for case in &self.cases {
            bytes.extend_from_slice(&case.canonical_bytes());
        }
        bytes
    }

    /// The digest a measurement carries.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(&self.canonical_bytes())
    }
}

/// The shipped corpus.
///
/// Six cases. Three are diarizers that behave; three are the ways one fails,
/// and each failure is a different consequence: a student labelled instructor
/// is speech a redaction would leave in, an instructor labelled student is
/// speech a redaction would take out, and an unattributed span is the provider
/// declining to say. Only the first is a privacy failure and the measurement
/// reports it on its own axis for that reason.
///
/// # Errors
///
/// [`CorpusFault`] if a literal below is edited into an invalid timeline.
pub fn corpus_v1() -> Result<DiarizationCorpus, CorpusFault> {
    let cases = vec![
        // A clean two-speaker lecture: the diarizer agrees everywhere.
        DiarizationCase::new(
            "clean_two_speaker",
            vec![
                VoiceSpan::new(0, 60_000, Speaker::Instructor),
                VoiceSpan::new(60_000, 72_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(72_000, 150_000, Speaker::Instructor),
            ],
            vec![
                VoiceSpan::new(0, 60_000, Speaker::Instructor),
                VoiceSpan::new(60_000, 72_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(72_000, 150_000, Speaker::Instructor),
            ],
        )?,
        // Two students the diarizer keeps apart. The ordinals differ from the
        // reference's on purpose: an ordinal is not an identity, so swapping
        // two students is not an error the redaction question cares about.
        DiarizationCase::new(
            "two_students_distinguished",
            vec![
                VoiceSpan::new(0, 30_000, Speaker::Instructor),
                VoiceSpan::new(30_000, 38_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(38_000, 44_000, Speaker::StudentUnknown(2)),
                VoiceSpan::new(44_000, 90_000, Speaker::Instructor),
            ],
            vec![
                VoiceSpan::new(0, 30_000, Speaker::Instructor),
                VoiceSpan::new(30_000, 38_000, Speaker::StudentUnknown(2)),
                VoiceSpan::new(38_000, 44_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(44_000, 90_000, Speaker::Instructor),
            ],
        )?,
        // A boundary the diarizer sets two seconds late: two seconds of student
        // speech land in the instructor's span. This is the missed redaction.
        DiarizationCase::new(
            "late_boundary_misses_student",
            vec![
                VoiceSpan::new(0, 40_000, Speaker::Instructor),
                VoiceSpan::new(40_000, 52_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(52_000, 100_000, Speaker::Instructor),
            ],
            vec![
                VoiceSpan::new(0, 42_000, Speaker::Instructor),
                VoiceSpan::new(42_000, 52_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(52_000, 100_000, Speaker::Instructor),
            ],
        )?,
        // A diarizer that takes four seconds of the instructor for a student:
        // over-redaction, which costs losslessness rather than privacy.
        DiarizationCase::new(
            "early_boundary_over_redacts",
            vec![
                VoiceSpan::new(0, 40_000, Speaker::Instructor),
                VoiceSpan::new(40_000, 50_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(50_000, 80_000, Speaker::Instructor),
            ],
            vec![
                VoiceSpan::new(0, 36_000, Speaker::Instructor),
                VoiceSpan::new(36_000, 50_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(50_000, 80_000, Speaker::Instructor),
            ],
        )?,
        // A crowded room: the diarizer declines to attribute the overlap.
        DiarizationCase::new(
            "declines_to_attribute",
            vec![
                VoiceSpan::new(0, 20_000, Speaker::Instructor),
                VoiceSpan::new(20_000, 26_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(26_000, 60_000, Speaker::Instructor),
            ],
            vec![
                VoiceSpan::new(0, 20_000, Speaker::Instructor),
                VoiceSpan::new(20_000, 26_000, Speaker::Unresolved),
                VoiceSpan::new(26_000, 60_000, Speaker::Instructor),
            ],
        )?,
        // A question the diarizer does not notice at all: the hypothesis has a
        // hole where the reference has student speech.
        DiarizationCase::new(
            "unheard_question",
            vec![
                VoiceSpan::new(0, 25_000, Speaker::Instructor),
                VoiceSpan::new(25_000, 31_000, Speaker::StudentUnknown(1)),
                VoiceSpan::new(31_000, 70_000, Speaker::Instructor),
            ],
            vec![
                VoiceSpan::new(0, 25_000, Speaker::Instructor),
                VoiceSpan::new(31_000, 70_000, Speaker::Instructor),
            ],
        )?,
    ];
    DiarizationCorpus::new(CORPUS_ID, CORPUS_VERSION, cases)
}
