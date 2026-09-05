//! Grade symbols and the versioned [`GradingScheme`] that gives them meaning.
//!
//! A grade symbol on its own says nothing arithmetic. `A0` is four grade points
//! under one published table and something else under another, and `S` is
//! outside the average entirely. What a symbol *does* to a grade-point average
//! is therefore a property of a **versioned scheme**, never of the symbol, and
//! never a constant in the engine — which is what lets
//! `gpa_policy_version_matrix` hold one attempt set fixed and observe the
//! published average move.
//!
//! Section 10 fixes one table: "SNU 공식 표는 A+ 4.3, A0 4.0, …, D- 0.7, F
//! 0이며 S/U 교과목은 평점 계산에서 제외한다". [`GradingScheme::snu_4_3_v1`] is
//! that table and cites it. Every other scheme in this crate is labelled
//! synthetic and exists so the version axis has a second point on it.

use std::collections::BTreeMap;

use academic_domain::Decimal;

use crate::CanonicalIdentifier;

use crate::{RecordError, decimal};

/// Every grade symbol this crate admits.
///
/// The set is closed. A transcript row spelling anything else is refused at the
/// boundary rather than mapped to a nearest neighbour, because the symbol is
/// what the whole calculation keys on and a silent coercion would be a wrong
/// average rather than an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GradeSymbol {
    /// A+.
    APlus,
    /// A0.
    AZero,
    /// A-.
    AMinus,
    /// B+.
    BPlus,
    /// B0.
    BZero,
    /// B-.
    BMinus,
    /// C+.
    CPlus,
    /// C0.
    CZero,
    /// C-.
    CMinus,
    /// D+.
    DPlus,
    /// D0.
    DZero,
    /// D-.
    DMinus,
    /// F.
    F,
    /// S — satisfactory, outside the average.
    S,
    /// U — unsatisfactory, outside the average.
    U,
    /// W — withdrawn.
    W,
    /// I — incomplete, not yet resolved into a grade.
    I,
}

impl GradeSymbol {
    /// Every symbol, best first, then the four non-numeric ones.
    pub const ALL: [Self; 17] = [
        Self::APlus,
        Self::AZero,
        Self::AMinus,
        Self::BPlus,
        Self::BZero,
        Self::BMinus,
        Self::CPlus,
        Self::CZero,
        Self::CMinus,
        Self::DPlus,
        Self::DZero,
        Self::DMinus,
        Self::F,
        Self::S,
        Self::U,
        Self::W,
        Self::I,
    ];

    /// Returns the symbol as an official transcript spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::APlus => "A+",
            Self::AZero => "A0",
            Self::AMinus => "A-",
            Self::BPlus => "B+",
            Self::BZero => "B0",
            Self::BMinus => "B-",
            Self::CPlus => "C+",
            Self::CZero => "C0",
            Self::CMinus => "C-",
            Self::DPlus => "D+",
            Self::DZero => "D0",
            Self::DMinus => "D-",
            Self::F => "F",
            Self::S => "S",
            Self::U => "U",
            Self::W => "W",
            Self::I => "I",
        }
    }

    /// Returns the identifier-shaped spelling a frozen engine input carries.
    ///
    /// `A+` is not an identifier, and the engine input encoding admits only
    /// ASCII alphanumerics, `.`, `_`, and `-`. This is the one re-spelling, and
    /// it is total and reversible.
    #[must_use]
    pub const fn as_token(self) -> &'static str {
        match self {
            Self::APlus => "A_PLUS",
            Self::AZero => "A_ZERO",
            Self::AMinus => "A_MINUS",
            Self::BPlus => "B_PLUS",
            Self::BZero => "B_ZERO",
            Self::BMinus => "B_MINUS",
            Self::CPlus => "C_PLUS",
            Self::CZero => "C_ZERO",
            Self::CMinus => "C_MINUS",
            Self::DPlus => "D_PLUS",
            Self::DZero => "D_ZERO",
            Self::DMinus => "D_MINUS",
            Self::F => "F",
            Self::S => "S",
            Self::U => "U",
            Self::W => "W",
            Self::I => "I",
        }
    }

    /// Resolves a symbol from an official transcript spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|symbol| symbol.as_str() == text)
    }

    /// Resolves a symbol from its frozen-input token.
    #[must_use]
    pub fn parse_token(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|symbol| symbol.as_token() == text)
    }
}

/// What one symbol does to credits and to the average, under one scheme.
///
/// The two questions are separate fields because they have separate answers,
/// and collapsing them is exactly the defect `credits_vs_denominator` exists to
/// catch. `S` earns credit and is outside the average. `F` earns none and is
/// *inside* it — an F that left the denominator would raise the average of a
/// student who failed a course, which is the wrong direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradeTreatment {
    /// The grade points one credit earns, or `None` when the symbol is outside
    /// the average entirely.
    grade_points: Option<Decimal>,
    /// Whether an attempt with this symbol earns its credits.
    earns_credit: bool,
    /// Whether the attempt is still open — an `I` that no later grade replaced.
    unresolved: bool,
}

impl GradeTreatment {
    /// A symbol inside the average, worth `grade_points` per credit.
    ///
    /// `earns_credit` is separate because `F` is graded and earns nothing.
    pub fn graded(grade_points: Decimal, earns_credit: bool) -> Self {
        Self {
            grade_points: Some(grade_points),
            earns_credit,
            unresolved: false,
        }
    }

    /// A symbol outside the average that still earns its credits (`S`).
    #[must_use]
    pub const fn earned_not_graded() -> Self {
        Self {
            grade_points: None,
            earns_credit: true,
            unresolved: false,
        }
    }

    /// A symbol outside the average that earns nothing (`U`, `W`).
    #[must_use]
    pub const fn not_earned_not_graded() -> Self {
        Self {
            grade_points: None,
            earns_credit: false,
            unresolved: false,
        }
    }

    /// An attempt whose grade is not yet decided (`I`).
    ///
    /// Distinct from [`Self::not_earned_not_graded`] because the two mean
    /// different things to a reader: a `W` is a settled outcome and an `I` is a
    /// value that is *known to be unknown*. The engine reports the second as
    /// `UNKNOWN` rather than folding it into a zero.
    #[must_use]
    pub const fn unresolved() -> Self {
        Self {
            grade_points: None,
            earns_credit: false,
            unresolved: true,
        }
    }

    /// Returns the grade points, or `None` when outside the average.
    #[must_use]
    pub const fn grade_points(&self) -> Option<Decimal> {
        self.grade_points
    }

    /// Whether the attempt earns its credits.
    #[must_use]
    pub const fn earns_credit(&self) -> bool {
        self.earns_credit
    }

    /// Whether the attempt participates in the grade-point average.
    #[must_use]
    pub const fn participates_in_average(&self) -> bool {
        self.grade_points.is_some()
    }

    /// Whether the grade is still undecided.
    #[must_use]
    pub const fn is_unresolved(&self) -> bool {
        self.unresolved
    }
}

/// A published grade table under a version identifier.
///
/// The version is part of the identity, not a label beside it: two schemes that
/// disagree about one symbol are two schemes, and every average this crate
/// publishes names the scheme it was computed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradingScheme {
    id: CanonicalIdentifier,
    treatments: BTreeMap<GradeSymbol, GradeTreatment>,
    published_scale: u8,
    citation: String,
}

impl GradingScheme {
    /// Builds a scheme, refusing one that leaves a symbol unmapped.
    ///
    /// Totality is the point. A scheme with a gap would make an average depend
    /// on whether a particular symbol happened to appear in the attempt set,
    /// and the gap would surface as a wrong number rather than as an error.
    ///
    /// `id` is rendered into [`GradingScheme::canonical_text`], so it is a
    /// [`CanonicalIdentifier`] rather than a `String`.
    pub fn new(
        id: impl Into<String>,
        treatments: BTreeMap<GradeSymbol, GradeTreatment>,
        published_scale: u8,
        citation: impl Into<String>,
    ) -> Result<Self, RecordError> {
        if published_scale > decimal::MAX_SCALE {
            return Err(RecordError::DecimalScaleTooLarge(published_scale));
        }
        for symbol in GradeSymbol::ALL {
            if !treatments.contains_key(&symbol) {
                return Err(RecordError::GradingSchemeIncomplete(symbol));
            }
        }
        Ok(Self {
            id: CanonicalIdentifier::new(id)?,
            treatments,
            published_scale,
            citation: citation.into(),
        })
    }

    /// Returns the scheme's version identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    /// Returns the number of digits an average is published to.
    #[must_use]
    pub const fn published_scale(&self) -> u8 {
        self.published_scale
    }

    /// Returns the source this table was transcribed from.
    #[must_use]
    pub fn citation(&self) -> &str {
        &self.citation
    }

    /// Returns one symbol's treatment. Total by construction.
    #[must_use]
    pub fn treatment(&self, symbol: GradeSymbol) -> GradeTreatment {
        self.treatments
            .get(&symbol)
            .copied()
            .unwrap_or_else(GradeTreatment::unresolved)
    }

    /// Renders the scheme as the rule-set text its hash is taken over.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = format!("grading-scheme {}\n", self.id);
        rendered.push_str(&format!("published-scale {}\n", self.published_scale));
        for symbol in GradeSymbol::ALL {
            let treatment = self.treatment(symbol);
            let points = treatment.grade_points().map_or_else(
                || "none".to_owned(),
                |value| format!("{}/{}", value.coefficient(), value.scale()),
            );
            rendered.push_str(&format!(
                "grade {} points={} earns_credit={} unresolved={}\n",
                symbol.as_token(),
                points,
                treatment.earns_credit(),
                treatment.is_unresolved(),
            ));
        }
        rendered
    }

    /// The section 10 table: A+ 4.3, A0 4.0, …, D- 0.7, F 0, S/U excluded.
    ///
    /// The endpoints and the S/U exclusion are the specification's own words;
    /// the interior steps are the published 서울대학교 성적등급 및 평점환산기준표
    /// the same sentence cites. `snu_grade_mapping_gpa` pins every row.
    ///
    /// `W` earns nothing and is outside the average. `I` is *unresolved*: the
    /// engine reports it as `UNKNOWN` rather than deciding it, because an
    /// incomplete that has not been resolved is a value the record does not
    /// have, and section 38 forbids inventing one.
    pub fn snu_4_3_v1() -> Result<Self, RecordError> {
        Self::snu_table("snu_4_3_v1", 2)
    }

    /// The same table published to three digits instead of two.
    ///
    /// The point table is byte-identical to [`Self::snu_4_3_v1`]; only the
    /// published scale differs. It is a second scheme *version* rather than a
    /// second set of facts, which is what makes it usable in
    /// `gpa_policy_version_matrix` without asserting a grade fact no source
    /// states.
    pub fn snu_4_3_v2_scale3() -> Result<Self, RecordError> {
        Self::snu_table("snu_4_3_v2_scale3", 3)
    }

    fn snu_table(id: &str, published_scale: u8) -> Result<Self, RecordError> {
        let point = |tenths: i128| -> Result<Decimal, RecordError> {
            Ok(academic_domain::Decimal::new(tenths, 1)?)
        };
        let mut treatments = BTreeMap::new();
        treatments.insert(GradeSymbol::APlus, GradeTreatment::graded(point(43)?, true));
        treatments.insert(GradeSymbol::AZero, GradeTreatment::graded(point(40)?, true));
        treatments.insert(
            GradeSymbol::AMinus,
            GradeTreatment::graded(point(37)?, true),
        );
        treatments.insert(GradeSymbol::BPlus, GradeTreatment::graded(point(33)?, true));
        treatments.insert(GradeSymbol::BZero, GradeTreatment::graded(point(30)?, true));
        treatments.insert(
            GradeSymbol::BMinus,
            GradeTreatment::graded(point(27)?, true),
        );
        treatments.insert(GradeSymbol::CPlus, GradeTreatment::graded(point(23)?, true));
        treatments.insert(GradeSymbol::CZero, GradeTreatment::graded(point(20)?, true));
        treatments.insert(
            GradeSymbol::CMinus,
            GradeTreatment::graded(point(17)?, true),
        );
        treatments.insert(GradeSymbol::DPlus, GradeTreatment::graded(point(13)?, true));
        treatments.insert(GradeSymbol::DZero, GradeTreatment::graded(point(10)?, true));
        treatments.insert(GradeSymbol::DMinus, GradeTreatment::graded(point(7)?, true));
        // F is inside the average and earns nothing. Dropping it from the
        // denominator would raise the average of a student who failed.
        treatments.insert(GradeSymbol::F, GradeTreatment::graded(point(0)?, false));
        treatments.insert(GradeSymbol::S, GradeTreatment::earned_not_graded());
        treatments.insert(GradeSymbol::U, GradeTreatment::not_earned_not_graded());
        treatments.insert(GradeSymbol::W, GradeTreatment::not_earned_not_graded());
        treatments.insert(GradeSymbol::I, GradeTreatment::unresolved());
        Self::new(
            id,
            treatments,
            published_scale,
            "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md section 10, \
             citing the published 서울대학교 성적등급 및 평점환산기준표",
        )
    }
}
