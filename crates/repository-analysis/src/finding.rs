//! Section 17.4's finding, and the two things it cannot be.
//!
//! ## It cannot be repository-wide
//!
//! Section 34.4's fourth row is *code snippet을 architecture 전체로 과대해석*,
//! and its prevention column is *finding scope를 symbol/component로 시작*.
//! `REQ-34-091` states the acceptance as *a new AI finding cannot default to
//! repository-wide scope*.
//!
//! Three things hold that, and they are the three `P2-K6`'s verified-admission
//! receipt and `P2-M2`'s `Proposed<T>` use. (That receipt type is not spelled
//! here: `no_environment_or_flag_override_exists` counts its name as an
//! admission-authority token and allows it to `academic-admission` alone, and a
//! doc comment is source text to a scan that reads raw bytes.)
//!
//! * **[`FindingScope`] has no repository variant.** There is no value to
//!   select, so the widest scope is not reachable by naming it.
//! * **[`ComponentId`] refuses the root.** The empty string, `.`, `/` and `./`
//!   are each refused, so the widest scope is not reachable by naming the whole
//!   tree as a component either.
//! * **[`Finding`] has no public constructor and no public field.** Its one
//!   producer is [`crate::ladder::EvidenceLadder::classify`], which derives the
//!   scope from the evidence rather than taking it as a defaultable argument,
//!   and which emits one finding per component rather than one finding spanning
//!   several. Evidence in three components is three findings.
//!
//! `crates/scenario/tests/compile_fail/` holds the compiled half: a struct
//! literal, a `Default`, a `Finding::new`, and a `FindingScope::Repository`
//! each fail to compile with a committed diagnostic.
//!
//! ## It cannot show a number nothing calibrated
//!
//! Section 17.3's third row is *가능, confidence 표시*, and `P2-M1`'s contract
//! is that only `CalibrationRegistry::interpret` issues a `CalibratedConfidence`
//! and only a `CalibratedConfidence` reaches `DisplayedConfidence::of`. So the
//! `OBSERVED` arm of the private tier value *holds* a [`DisplayedConfidence`]:
//! an observed finding without a calibrated number is not a value this crate
//! can build, and a finding that would be observed without one is refused
//! rather than shown with a bare score.

use academic_model_run::DisplayedConfidence;
use academic_policy::ContentDigest;

use crate::{
    index::{SourceSpan, SymbolFingerprint, SymbolKind},
    paths::{ArtifactScope, ComponentId, PathClass},
};

/// Section 34.4's `PRESENT_ONLY/POSSIBLE/OBSERVED`.
///
/// `REQ-34-081` names these three exact values as what a reader is shown.
/// Section 17.3's ladder has five rungs and they fold onto these three; which
/// rung became which value is [`LadderRung`], carried beside the tier so the
/// fold is readable rather than lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceTier {
    /// The subject is named somewhere and nothing more is known.
    PresentOnly,
    /// There is a use that nothing shows running.
    Possible,
    /// There is direct evidence of use, at the finding's scope.
    Observed,
}

impl EvidenceTier {
    /// Exhaustive order, weakest first.
    pub const ALL: [Self; 3] = [Self::PresentOnly, Self::Possible, Self::Observed];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PresentOnly => "PRESENT_ONLY",
            Self::Possible => "POSSIBLE",
            Self::Observed => "OBSERVED",
        }
    }
}

/// Which of section 17.3's five observations produced a tier.
///
/// The section 17.3 table is the authority for the fold, row for row:
///
/// | Section 17.3 observation | `OBSERVED` 가능 여부 | Rung | Tier |
/// |---|---|---|---|
/// | `manifest에 dependency만 있음` | 불가 | [`LadderRung::ManifestPresence`] | `PRESENT_ONLY` |
/// | `import만 있고 reachable use 없음` | 보류 | [`LadderRung::UnreachableImport`] | `POSSIBLE` |
/// | `reachable call + config 존재` | 가능, confidence 표시 | [`LadderRung::ReachableCallWithConfig`] | `OBSERVED` |
/// | `test에서만 사용` | scope를 제한해 가능 | [`LadderRung::TestScopedUse`] | `OBSERVED`, scope `TEST` |
/// | `runtime trace/production config와 일치` | 가능 | [`LadderRung::RuntimeAndProductionConfig`] | `OBSERVED`, strong |
///
/// The fourth row restricts a scope rather than raising a strength, which is
/// why the rungs are not ordered by strength: rung four is not above rung
/// three, it is rung three with the answer narrowed to tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LadderRung {
    /// A manifest or lock file names the subject and nothing else does.
    ManifestPresence,
    /// Source imports the subject, and no reachable call uses it.
    UnreachableImport,
    /// A call reachable from an entry point, plus configuration.
    ReachableCallWithConfig,
    /// Every use of the subject sits at test scope.
    TestScopedUse,
    /// A runtime trace of this snapshot agrees with production configuration.
    RuntimeAndProductionConfig,
}

impl LadderRung {
    /// Exhaustive order, in section 17.3's own row order.
    pub const ALL: [Self; 5] = [
        Self::ManifestPresence,
        Self::UnreachableImport,
        Self::ReachableCallWithConfig,
        Self::TestScopedUse,
        Self::RuntimeAndProductionConfig,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ManifestPresence => "MANIFEST_PRESENCE",
            Self::UnreachableImport => "UNREACHABLE_IMPORT",
            Self::ReachableCallWithConfig => "REACHABLE_CALL_WITH_CONFIG",
            Self::TestScopedUse => "TEST_SCOPED_USE",
            Self::RuntimeAndProductionConfig => "RUNTIME_AND_PRODUCTION_CONFIG",
        }
    }

    /// The tier this rung folds onto. The table above, as a total function.
    #[must_use]
    pub const fn tier(self) -> EvidenceTier {
        match self {
            Self::ManifestPresence => EvidenceTier::PresentOnly,
            Self::UnreachableImport => EvidenceTier::Possible,
            Self::ReachableCallWithConfig
            | Self::TestScopedUse
            | Self::RuntimeAndProductionConfig => EvidenceTier::Observed,
        }
    }
}

/// How strongly the evidence speaks about execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceStrength {
    /// Read from the source tree alone.
    Static,
    /// Section 17.3's fifth row: a runtime trace agreeing with production
    /// configuration, which that row calls `실행 사용의 강한 근거`.
    Strong,
}

impl EvidenceStrength {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Static, Self::Strong];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "STATIC",
            Self::Strong => "STRONG",
        }
    }
}

/// Section 17.4's locator: path, symbol fingerprint, line span, blob hash.
///
/// Section 17.4's own sentence is that a path alone breaks when a line moves,
/// so `blob hash, symbol fingerprint, syntax span과 commit을 함께 저장`. The
/// commit is on the snapshot the finding names; the other three are here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locator {
    path: String,
    symbol: Option<SymbolFingerprint>,
    symbol_kind: Option<SymbolKind>,
    span: SourceSpan,
    blob_digest: ContentDigest,
    class: PathClass,
    scope: ArtifactScope,
}

impl Locator {
    /// Builds a locator. Crate-private: a locator names a place in a frozen
    /// snapshot, and the only thing that knows the frozen bytes is the analysis.
    pub(crate) const fn new(
        path: String,
        symbol: Option<SymbolFingerprint>,
        symbol_kind: Option<SymbolKind>,
        span: SourceSpan,
        blob_digest: ContentDigest,
        class: PathClass,
        scope: ArtifactScope,
    ) -> Self {
        Self {
            path,
            symbol,
            symbol_kind,
            span,
            blob_digest,
            class,
            scope,
        }
    }

    /// The path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The symbol, when the evidence sits inside one.
    #[must_use]
    pub const fn symbol(&self) -> Option<&SymbolFingerprint> {
        self.symbol.as_ref()
    }

    /// What kind of symbol it is.
    #[must_use]
    pub const fn symbol_kind(&self) -> Option<SymbolKind> {
        self.symbol_kind
    }

    /// Section 17.4's `lineSpan`, with the byte range beside it.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// The digest of the file as the snapshot froze it.
    #[must_use]
    pub const fn blob_digest(&self) -> &ContentDigest {
        &self.blob_digest
    }

    /// The promotion class of the path.
    #[must_use]
    pub const fn class(&self) -> PathClass {
        self.class
    }

    /// The section 18.1 scope of the path.
    #[must_use]
    pub const fn scope(&self) -> ArtifactScope {
        self.scope
    }
}

/// Where a finding applies. Symbol or component, and nothing wider.
///
/// There is no `Repository` variant and no `All`. The type is `non_exhaustive`
/// so a caller outside this crate cannot write an exhaustive `match` that a
/// third variant would break, and — more to the point here — cannot construct
/// one either, so this enumeration is not a route around
/// [`Finding`]'s private constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FindingScope {
    /// One declaration, named by its fingerprint.
    Symbol {
        /// Which component the symbol sits in.
        component: ComponentId,
        /// The declaration.
        symbol: SymbolFingerprint,
    },
    /// One directory of the repository.
    Component {
        /// The directory.
        component: ComponentId,
    },
}

impl FindingScope {
    /// The component, which both arms have.
    #[must_use]
    pub const fn component(&self) -> &ComponentId {
        match self {
            Self::Symbol { component, .. } | Self::Component { component } => component,
        }
    }

    /// Stable spelling of which arm this is.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Symbol { .. } => "SYMBOL",
            Self::Component { .. } => "COMPONENT",
        }
    }
}

/// `REQ-34-093`'s "only observed in this component" denominator.
///
/// The denominator is the number of components this run actually analyzed —
/// components holding at least one path the analyzer had a reader for — rather
/// than the number of components in the tree. A component that is entirely a
/// coverage gap is not in the denominator, because including it would report a
/// coverage percentage against a tree the analyzer never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentCoverage {
    observed: u32,
    analyzed: u32,
}

impl ComponentCoverage {
    pub(crate) const fn new(observed: u32, analyzed: u32) -> Self {
        Self { observed, analyzed }
    }

    /// How many components hold evidence for this subject.
    #[must_use]
    pub const fn observed_components(&self) -> u32 {
        self.observed
    }

    /// How many components this run read at all. The denominator.
    #[must_use]
    pub const fn analyzed_components(&self) -> u32 {
        self.analyzed
    }
}

/// The tier and the number that has to accompany it.
///
/// Private, and the whole reason the pairing is not an `Option` field with a
/// checked invariant: the `Observed` arm carries the `DisplayedConfidence`, so
/// an observed finding without a calibrated number has no representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TierEvidence {
    PresentOnly,
    Possible,
    Observed(DisplayedConfidence),
}

impl TierEvidence {
    const fn tier(&self) -> EvidenceTier {
        match self {
            Self::PresentOnly => EvidenceTier::PresentOnly,
            Self::Possible => EvidenceTier::Possible,
            Self::Observed(_) => EvidenceTier::Observed,
        }
    }
}

/// A site that was found and deliberately did not count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedSite {
    locator: Locator,
    reason: ExclusionReason,
}

impl ExcludedSite {
    pub(crate) const fn new(locator: Locator, reason: ExclusionReason) -> Self {
        Self { locator, reason }
    }

    /// Where it was.
    #[must_use]
    pub const fn locator(&self) -> &Locator {
        &self.locator
    }

    /// Why it did not raise the tier.
    #[must_use]
    pub const fn reason(&self) -> ExclusionReason {
        self.reason
    }
}

/// Why a site was kept but not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExclusionReason {
    /// The path's class does not promote: vendored, generated or example.
    NonPromotingPath,
    /// The site is in another package of this monorepo.
    OtherPackage,
}

impl ExclusionReason {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::NonPromotingPath, Self::OtherPackage];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NonPromotingPath => "NON_PROMOTING_PATH",
            Self::OtherPackage => "OTHER_PACKAGE",
        }
    }
}

/// Section 17.4's `ProjectFinding`, at the strength this task fixes.
///
/// Every field is private, there is no `Default`, and the only constructor is
/// crate-private and called from one place. See the module documentation for
/// why that is what `new_finding_cannot_default_to_repository_scope` means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    snapshot_id: String,
    subject: String,
    scope: FindingScope,
    tier: TierEvidence,
    rung: LadderRung,
    artifact_scope: ArtifactScope,
    strength: EvidenceStrength,
    locators: Vec<Locator>,
    excluded: Vec<ExcludedSite>,
    coverage: ComponentCoverage,
}

impl Finding {
    /// The one constructor. Crate-private and called only from the ladder.
    #[expect(
        clippy::too_many_arguments,
        reason = "every field of section 17.4's finding is required at construction; a builder \
                  would reintroduce the partially-built value this type exists to refuse"
    )]
    pub(crate) const fn seal(
        snapshot_id: String,
        subject: String,
        scope: FindingScope,
        tier: TierEvidence,
        rung: LadderRung,
        artifact_scope: ArtifactScope,
        strength: EvidenceStrength,
        locators: Vec<Locator>,
        excluded: Vec<ExcludedSite>,
        coverage: ComponentCoverage,
    ) -> Self {
        Self {
            snapshot_id,
            subject,
            scope,
            tier,
            rung,
            artifact_scope,
            strength,
            locators,
            excluded,
            coverage,
        }
    }

    /// Which snapshot this finding is about.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// The caller's own identifier for what the finding is about.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Symbol or component. Never the repository.
    #[must_use]
    pub const fn scope(&self) -> &FindingScope {
        &self.scope
    }

    /// `PRESENT_ONLY`, `POSSIBLE` or `OBSERVED`.
    #[must_use]
    pub const fn tier(&self) -> EvidenceTier {
        self.tier.tier()
    }

    /// Which of section 17.3's five observations produced the tier.
    #[must_use]
    pub const fn rung(&self) -> LadderRung {
        self.rung
    }

    /// Section 18.1's scope of the use.
    #[must_use]
    pub const fn artifact_scope(&self) -> ArtifactScope {
        self.artifact_scope
    }

    /// Static or strong.
    #[must_use]
    pub const fn strength(&self) -> EvidenceStrength {
        self.strength
    }

    /// The calibrated confidence, which every `OBSERVED` finding has and no
    /// other finding has.
    #[must_use]
    pub const fn confidence(&self) -> Option<&DisplayedConfidence> {
        match &self.tier {
            TierEvidence::Observed(confidence) => Some(confidence),
            TierEvidence::PresentOnly | TierEvidence::Possible => None,
        }
    }

    /// The sites that produced the tier.
    #[must_use]
    pub fn locators(&self) -> &[Locator] {
        &self.locators
    }

    /// The sites that were found and did not count, with why.
    #[must_use]
    pub fn excluded_sites(&self) -> &[ExcludedSite] {
        &self.excluded
    }

    /// `REQ-34-093`'s coverage numerator and denominator.
    #[must_use]
    pub const fn coverage(&self) -> ComponentCoverage {
        self.coverage
    }
}
