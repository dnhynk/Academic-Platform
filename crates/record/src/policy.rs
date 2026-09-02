//! Effective-dated policy rows, and the rule book their hash identifies.
//!
//! Two facts in section 10 are dated, and neither may be a constant:
//!
//! - the undergraduate repeat ceiling applies "2015학년도 1학기 이수 교과목부터";
//! - grades earned at another university "2004학년도 이후" are not counted in
//!   the 본교 평점평균.
//!
//! A constant spelled `2015` in an `if` cannot be superseded, cannot be dated
//! against an attempt that predates it, and cannot say which published notice
//! it came from. Both are therefore rows in a [`PolicyBook`], selected by the
//! attempt's own [`TermKey`], and `repeat_ceiling_effective_date` moves a row's
//! date and observes the published average move with it.
//!
//! What the specification does **not** state, this crate does not invent. It
//! fixes the ceiling but says the repeat *eligibility* rule, the 경과조치 for
//! old courses, and the 동일·대체 mapping are "별도 versioned policy로 관리하고
//! 최신 원문을 확인한다" — a separate policy whose current original has not been
//! confirmed. Which of two attempts is the recognized one is likewise unstated.
//! Those fields are therefore [`RepeatRecognition::Unknown`] and
//! [`RecognitionDecision::Undecided`] in the shipped book, and an engine that
//! meets one reports `UNKNOWN` rather than choosing. That is `GATE-38-006` and
//! `GATE-38-016` staying open, expressed as a value.

use academic_domain::ContentDigest;

use crate::{RecordError, grade::GradeSymbol, grade::GradingScheme, term::TermKey};

/// Where an attempt's credits were earned.
///
/// The distinction is what the external-grade policy row keys on: a grade
/// earned at this institution is never subject to it, whatever the term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttemptOrigin {
    /// Taken at this institution.
    Internal,
    /// 교환학생 — taken abroad under an exchange agreement.
    Exchange,
    /// 편입 — carried in on transfer.
    Transfer,
    /// 인정학점 — recognized from another programme or 학점교류.
    Recognized,
}

impl AttemptOrigin {
    /// Every origin.
    pub const ALL: [Self; 4] = [
        Self::Internal,
        Self::Exchange,
        Self::Transfer,
        Self::Recognized,
    ];

    /// Returns the contract spelling, which is also the frozen-input token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "INTERNAL",
            Self::Exchange => "EXCHANGE",
            Self::Transfer => "TRANSFER",
            Self::Recognized => "RECOGNIZED",
        }
    }

    /// Resolves an origin from its spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|origin| origin.as_str() == text)
    }

    /// Whether the origin is outside this institution.
    #[must_use]
    pub const fn is_external(self) -> bool {
        !matches!(self, Self::Internal)
    }
}

/// Whether an external attempt's credits count toward the earned total.
///
/// `GATE-38-006` — "Recognition decisions per credit, or they stay excluded
/// from definitive proof". [`Self::Undecided`] is the shipped value and is not
/// a synonym for "no": an undecided credit is *known to be undecided*, and the
/// engine reports it as `UNKNOWN` rather than silently dropping it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognitionDecision {
    /// The user confirmed the credits are recognized.
    Recognized,
    /// The user confirmed the credits are not recognized.
    NotRecognized,
    /// No decision has been recorded. The default, and never resolved here.
    Undecided,
}

impl RecognitionDecision {
    /// Every decision.
    pub const ALL: [Self; 3] = [Self::Recognized, Self::NotRecognized, Self::Undecided];

    /// Returns the contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recognized => "RECOGNIZED",
            Self::NotRecognized => "NOT_RECOGNIZED",
            Self::Undecided => "UNDECIDED",
        }
    }

    /// Resolves a decision from its spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|decision| decision.as_str() == text)
    }
}

/// Which attempt in a repeat group is the recognized one.
///
/// Section 10 does not state this. It states the *ceiling* and says the rest is
/// a separate versioned policy whose current original must be confirmed, so the
/// shipped book carries [`Self::Unknown`] and the engine refuses to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatRecognition {
    /// The latest completed attempt is the recognized one.
    LatestAttempt,
    /// The highest-graded completed attempt is the recognized one.
    HighestAttempt,
    /// No confirmed official source says. The engine reports `UNKNOWN`.
    Unknown,
}

impl RepeatRecognition {
    /// Every rule.
    pub const ALL: [Self; 3] = [Self::LatestAttempt, Self::HighestAttempt, Self::Unknown];

    /// Returns the contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LatestAttempt => "LATEST_ATTEMPT",
            Self::HighestAttempt => "HIGHEST_ATTEMPT",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Resolves a rule from its spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| rule.as_str() == text)
    }
}

/// One dated repeat-policy row.
///
/// `effective_from` is inclusive and is compared against the term the attempt
/// was *taken in*, which is what the notice says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatPolicyRow {
    /// Row identity, carried into the proof tree.
    pub row_id: String,
    /// The first term the row governs, inclusive.
    pub effective_from: TermKey,
    /// The highest grade a repeat attempt may be recorded as earning.
    ///
    /// `None` means the row states no ceiling, which is different from a row
    /// not existing: one is a confirmed absence, the other is silence.
    pub ceiling: Option<GradeSymbol>,
    /// Which attempt of a repeat group is recognized.
    pub recognition: RepeatRecognition,
    /// The published source this row was transcribed from.
    pub citation: String,
}

/// One dated external-grade row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalGradePolicyRow {
    /// Row identity, carried into the proof tree.
    pub row_id: String,
    /// The first term the row governs, inclusive.
    pub effective_from: TermKey,
    /// Whether a grade from outside this institution stays out of the average.
    pub excluded_from_average: bool,
    /// The published source this row was transcribed from.
    pub citation: String,
}

/// The effective-dated rows a rule book selects from.
///
/// Rows are held sorted and a lookup returns the last row whose
/// `effective_from` is at or before the attempt's term. A term no row reaches
/// resolves to `None`, and the engine reports that as `UNKNOWN` — never as
/// "the rule does not apply", which would be a policy claim about a period no
/// source in this repository covers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PolicyBook {
    repeat_rows: Vec<RepeatPolicyRow>,
    external_rows: Vec<ExternalGradePolicyRow>,
}

impl PolicyBook {
    /// Builds a book, refusing two rows that share an effective term.
    ///
    /// Two rows on one date have no order between them, so the lookup would
    /// depend on insertion order — a silent dependence on how the book was
    /// assembled rather than on what it says.
    pub fn new(
        mut repeat_rows: Vec<RepeatPolicyRow>,
        mut external_rows: Vec<ExternalGradePolicyRow>,
    ) -> Result<Self, RecordError> {
        repeat_rows.sort_by_key(|row| row.effective_from);
        external_rows.sort_by_key(|row| row.effective_from);
        for window in repeat_rows.windows(2) {
            if let [earlier, later] = window
                && earlier.effective_from == later.effective_from
            {
                return Err(RecordError::DuplicatePolicyEffectiveTerm(
                    earlier.effective_from.canonical_text(),
                ));
            }
        }
        for window in external_rows.windows(2) {
            if let [earlier, later] = window
                && earlier.effective_from == later.effective_from
            {
                return Err(RecordError::DuplicatePolicyEffectiveTerm(
                    earlier.effective_from.canonical_text(),
                ));
            }
        }
        Ok(Self {
            repeat_rows,
            external_rows,
        })
    }

    /// Returns the repeat row governing `term`, or `None` if none reaches it.
    #[must_use]
    pub fn repeat_row_at(&self, term: TermKey) -> Option<&RepeatPolicyRow> {
        self.repeat_rows
            .iter()
            .rev()
            .find(|row| row.effective_from <= term)
    }

    /// Returns the external-grade row governing `term`, or `None`.
    #[must_use]
    pub fn external_row_at(&self, term: TermKey) -> Option<&ExternalGradePolicyRow> {
        self.external_rows
            .iter()
            .rev()
            .find(|row| row.effective_from <= term)
    }

    /// Returns every repeat row, in effective order.
    #[must_use]
    pub fn repeat_rows(&self) -> &[RepeatPolicyRow] {
        &self.repeat_rows
    }

    /// Returns every external-grade row, in effective order.
    #[must_use]
    pub fn external_rows(&self) -> &[ExternalGradePolicyRow] {
        &self.external_rows
    }

    /// Renders the book as the rule-set text its hash is taken over.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = String::new();
        for row in &self.repeat_rows {
            rendered.push_str(&format!(
                "repeat-policy {} from={} ceiling={} recognition={}\n",
                row.row_id,
                row.effective_from.canonical_text(),
                row.ceiling
                    .map_or_else(|| "none".to_owned(), |symbol| symbol.as_token().to_owned()),
                row.recognition.as_str(),
            ));
        }
        for row in &self.external_rows {
            rendered.push_str(&format!(
                "external-grade-policy {} from={} excluded_from_average={}\n",
                row.row_id,
                row.effective_from.canonical_text(),
                row.excluded_from_average,
            ));
        }
        rendered
    }

    /// The two dated rows section 10 states, and nothing else.
    ///
    /// The repeat row's `recognition` is [`RepeatRecognition::Unknown`]: the
    /// ceiling is sourced and the recognition rule is not. A caller that needs
    /// a definite average over a repeat group supplies its own confirmed row.
    pub fn published_v1() -> Result<Self, RecordError> {
        Self::new(
            vec![RepeatPolicyRow {
                row_id: "repeat.ceiling.2015_spring".to_owned(),
                effective_from: TermKey::parse("2015_SPRING")?,
                ceiling: Some(GradeSymbol::AZero),
                recognition: RepeatRecognition::Unknown,
                citation: "2026학년도 2학기 수강신청 안내, as quoted in section 10".to_owned(),
            }],
            vec![ExternalGradePolicyRow {
                row_id: "external.excluded.2004_spring".to_owned(),
                effective_from: TermKey::parse("2004_SPRING")?,
                excluded_from_average: true,
                citation: "서울대학교 성적등급 및 평점환산기준표 유의사항, as quoted in section 10"
                    .to_owned(),
            }],
        )
    }
}

/// A grading scheme and a policy book, together, under one hash.
///
/// This is the `rule_set_hash` half of the deterministic engine signature. An
/// engine refuses a hash that is not its own book's, so an average can never be
/// attributed to a rule set that did not produce it, and two evaluations that
/// agree on bytes agree on the rules as well as on the inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleBook {
    scheme: GradingScheme,
    policies: PolicyBook,
    classification_ruleset_id: String,
}

impl RuleBook {
    /// Assembles a rule book.
    #[must_use]
    pub fn new(
        scheme: GradingScheme,
        policies: PolicyBook,
        classification_ruleset_id: impl Into<String>,
    ) -> Self {
        Self {
            scheme,
            policies,
            classification_ruleset_id: classification_ruleset_id.into(),
        }
    }

    /// Returns the grading scheme.
    #[must_use]
    pub const fn scheme(&self) -> &GradingScheme {
        &self.scheme
    }

    /// Returns the policy book.
    #[must_use]
    pub const fn policies(&self) -> &PolicyBook {
        &self.policies
    }

    /// Returns the classification rule set these results are classified under.
    #[must_use]
    pub fn classification_ruleset_id(&self) -> &str {
        &self.classification_ruleset_id
    }

    /// Renders the whole book. This is the byte form `ruleset.txt` holds.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = String::from("academic-record-ruleset v1\n");
        rendered.push_str(&self.scheme.canonical_text());
        rendered.push_str(&self.policies.canonical_text());
        rendered.push_str(&format!(
            "classification-ruleset {}\n",
            self.classification_ruleset_id
        ));
        rendered
    }

    /// Returns the digest every evaluation under this book is keyed by.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(self.canonical_text().as_bytes())
    }
}
