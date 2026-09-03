//! The two thresholds section 12.6 leaves to configuration, and the version
//! they carry.
//!
//! The plan's `P2-L4` row says the confidence and gap thresholds are versioned
//! configuration with recorded defaults, and this module is what makes the
//! second half of that sentence checkable: [`COVERAGE_CONFIG_V1`] is the
//! default, `the_recorded_defaults_are_the_documented_ones` reads the table out
//! of `docs/contracts/lecture-document.md` and compares it against these
//! constants, so a threshold changed in code and left undocumented fails.
//!
//! A configuration is a value a caller supplies and a
//! [`crate::CoverageReport`] carries: two runs under different thresholds are
//! two different answers, and the report says which one it was.

/// The versioned thresholds a coverage run is evaluated under.
///
/// Private fields and one constructor, so a threshold cannot be moved on a
/// report that already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoverageConfig {
    version: u32,
    gap_threshold_nanos: u64,
    low_confidence_at_or_below_permille: u16,
}

impl CoverageConfig {
    /// Builds a configuration.
    ///
    /// # Errors
    ///
    /// [`ConfigFault`] when the version is zero or the permille is above one
    /// thousand.
    pub const fn new(
        version: u32,
        gap_threshold_nanos: u64,
        low_confidence_at_or_below_permille: u16,
    ) -> Result<Self, ConfigFault> {
        if version == 0 {
            return Err(ConfigFault::VersionIsZero);
        }
        if low_confidence_at_or_below_permille > 1000 {
            return Err(ConfigFault::PermilleOutOfRange);
        }
        Ok(Self {
            version,
            gap_threshold_nanos,
            low_confidence_at_or_below_permille,
        })
    }

    /// Which version of the configuration this is.
    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }

    /// The elapsed distance between two consecutive audio frames above which an
    /// unexplained hole is a finding.
    #[must_use]
    pub const fn gap_threshold_nanos(self) -> u64 {
        self.gap_threshold_nanos
    }

    /// The calibrated confidence at or below which a span enters the review
    /// queue.
    #[must_use]
    pub const fn low_confidence_at_or_below_permille(self) -> u16 {
        self.low_confidence_at_or_below_permille
    }
}

/// The recorded defaults.
///
/// `gap_threshold_nanos` is two seconds. `P2-L2` records a frame's session
/// instant and not its duration, so what is measurable is the elapsed distance
/// between two consecutive audio frames; two seconds is above any chunk cadence
/// the capture subsystem writes and below the shortest hole a listener would
/// call a hole. `low_confidence_at_or_below_permille` is 700: a calibrated
/// seven-in-ten is where section 12.6 wants a span in front of a person rather
/// than in a document.
///
/// Both are here because they are configuration, not because they are right.
/// The contract page carries the same two numbers and the same reasons, and a
/// test compares the two.
pub const COVERAGE_CONFIG_V1: CoverageConfig = CoverageConfig {
    version: 1,
    gap_threshold_nanos: 2_000_000_000,
    low_confidence_at_or_below_permille: 700,
};

/// What a malformed configuration is refused with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigFault {
    /// A configuration has to be able to name itself.
    #[error("a coverage configuration version starts at one")]
    VersionIsZero,
    /// Permille is per thousand.
    #[error("a confidence permille is at most one thousand")]
    PermilleOutOfRange,
}
