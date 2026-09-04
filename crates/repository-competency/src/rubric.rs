//! Section 17.6's second bullet: `단순 scaffold가 아닌 이해가 필요한 선택·수정`.
//!
//! Deciding which of those a change is, is a judgement, and this crate does not
//! make it. [`ScaffoldRubric`] is **configuration**: a value with an identifier
//! and a version that the caller supplies and a personal claim records. The
//! answer to *was this scaffold* is therefore always relative to a named
//! version of a named rubric, and two rubric versions may disagree about one
//! change without either being a bug.
//!
//! ## Why it is not a constant
//!
//! A threshold compiled into this crate would be a product decision made where
//! nobody can see it, revised by editing code, and invisible to the reader of a
//! claim it decided. There is therefore no [`Default`] for [`ScaffoldRubric`],
//! no constructor that fills any part in, and no numeric literal in
//! [`ScaffoldRubric::judge`]: every threshold it compares against is a field of
//! the value it was handed. `the_rubric_is_configuration_and_not_a_constant`
//! holds all three over the source.
//!
//! ## What a rubric says
//!
//! Three parts, and each answers a different way a change can be scaffold.
//!
//! | Part | Question |
//! |---|---|
//! | [`ScaffoldRubric::scaffold_change_kinds`] | is this *kind* of edit one that needed a choice? |
//! | [`ScaffoldRubric::scaffold_path_classes`] | is the file one this repository is the author of? |
//! | [`ScaffoldRubric::minimum_bearing_sites`] | is there enough of it to be a contribution? |
//!
//! The second reuses `P2-R2`'s [`PathClass`] rather than introducing a second
//! vocabulary for the same fact: `VENDORED` and `GENERATED` are already that
//! crate's names for source this repository did not write, and `P2-R2` already
//! refuses to let evidence at either raise a tier.

use std::collections::BTreeSet;

use academic_repository_analysis::{Locator, PathClass};

use crate::{CompetencyError, identity::validated};

/// What one changed site is an edit to.
///
/// A closed vocabulary, because a rubric that named kinds it did not enumerate
/// could not be compared with another rubric. The connector that reports a
/// change classifies each site into one of these; this crate reads the answer
/// and never derives it from a path or from the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeKind {
    /// A dependency's version in a manifest or a lockfile.
    DependencyPin,
    /// A value in a configuration or environment file.
    ConfigurationValue,
    /// Output a tool wrote from something else in the tree.
    GeneratedArtifact,
    /// Whitespace, an import order, a rename with no other effect.
    Formatting,
    /// A template's own files, unedited past the placeholders.
    ProjectScaffold,
    /// A branch, a loop, an early return: what the program decides.
    ControlFlow,
    /// A type, a schema, an index: how the data is shaped.
    DataStructure,
    /// A failure path: what happens when something does not hold.
    ErrorHandling,
    /// A lock, an isolation level, an ordering constraint.
    ConcurrencyControl,
    /// A test that exercises a behaviour or a failure.
    TestBehaviour,
}

impl ChangeKind {
    /// Exhaustive order.
    pub const ALL: [Self; 10] = [
        Self::DependencyPin,
        Self::ConfigurationValue,
        Self::GeneratedArtifact,
        Self::Formatting,
        Self::ProjectScaffold,
        Self::ControlFlow,
        Self::DataStructure,
        Self::ErrorHandling,
        Self::ConcurrencyControl,
        Self::TestBehaviour,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependencyPin => "DEPENDENCY_PIN",
            Self::ConfigurationValue => "CONFIGURATION_VALUE",
            Self::GeneratedArtifact => "GENERATED_ARTIFACT",
            Self::Formatting => "FORMATTING",
            Self::ProjectScaffold => "PROJECT_SCAFFOLD",
            Self::ControlFlow => "CONTROL_FLOW",
            Self::DataStructure => "DATA_STRUCTURE",
            Self::ErrorHandling => "ERROR_HANDLING",
            Self::ConcurrencyControl => "CONCURRENCY_CONTROL",
            Self::TestBehaviour => "TEST_BEHAVIOUR",
        }
    }
}

/// One place a change touched, with what kind of edit it was.
///
/// The locator is `P2-R2`'s own, so a changed site and an observed use are
/// described in one vocabulary and can be compared without a translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedSite {
    locator: Locator,
    kind: ChangeKind,
}

impl ChangedSite {
    /// Names a site and what kind of edit it was.
    #[must_use]
    pub const fn new(locator: Locator, kind: ChangeKind) -> Self {
        Self { locator, kind }
    }

    /// Where it is.
    #[must_use]
    pub const fn locator(&self) -> &Locator {
        &self.locator
    }

    /// What kind of edit it was.
    #[must_use]
    pub const fn kind(&self) -> ChangeKind {
        self.kind
    }
}

/// Names one rubric.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RubricId {
    identifier: String,
}

impl RubricId {
    /// Validates and takes a rubric identifier.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when it is empty, over 64 bytes,
    /// or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self {
            identifier: validated(value.into(), "rubric")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// The rubric that separates a scaffold change from one that needed
/// understanding, at one version.
///
/// Private fields, one constructor taking every part, and no [`Default`]. See
/// the module documentation for why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldRubric {
    id: RubricId,
    version: u64,
    scaffold_change_kinds: BTreeSet<ChangeKind>,
    scaffold_path_classes: BTreeSet<PathClass>,
    minimum_bearing_sites: u32,
}

impl ScaffoldRubric {
    /// Takes a whole rubric.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::RubricAdmitsNothing`] when the rubric would call
    /// every possible change meaningful *and* require no site — a rubric that
    /// answers `MEANINGFUL` for a change with no sites at all is not a rubric,
    /// it is the absence of one, and the whole of section 17.6's second bullet
    /// is that such a thing does not decide a personal claim.
    pub fn of(
        id: RubricId,
        version: u64,
        scaffold_change_kinds: Vec<ChangeKind>,
        scaffold_path_classes: Vec<PathClass>,
        minimum_bearing_sites: u32,
    ) -> Result<Self, CompetencyError> {
        if minimum_bearing_sites == 0 {
            return Err(CompetencyError::RubricAdmitsNothing(
                id.as_str().to_owned(),
                version,
            ));
        }
        Ok(Self {
            id,
            version,
            scaffold_change_kinds: scaffold_change_kinds.into_iter().collect(),
            scaffold_path_classes: scaffold_path_classes.into_iter().collect(),
            minimum_bearing_sites,
        })
    }

    /// Which rubric.
    #[must_use]
    pub const fn id(&self) -> &RubricId {
        &self.id
    }

    /// Which version of it. A personal claim records this.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// The kinds of edit this rubric calls scaffold, in enumeration order.
    #[must_use]
    pub fn scaffold_change_kinds(&self) -> Vec<ChangeKind> {
        self.scaffold_change_kinds.iter().copied().collect()
    }

    /// The path classes this rubric calls scaffold, in enumeration order.
    #[must_use]
    pub fn scaffold_path_classes(&self) -> Vec<PathClass> {
        self.scaffold_path_classes.iter().copied().collect()
    }

    /// How many understanding-bearing sites a change needs.
    #[must_use]
    pub const fn minimum_bearing_sites(&self) -> u32 {
        self.minimum_bearing_sites
    }

    /// Whether one site is one this rubric counts.
    ///
    /// Both halves have to hold: a `CONTROL_FLOW` edit inside vendored source
    /// is somebody else's control flow, and a `FORMATTING` edit inside
    /// first-party source is still formatting.
    #[must_use]
    pub fn bears_understanding(&self, site: &ChangedSite) -> bool {
        !self.scaffold_change_kinds.contains(&site.kind())
            && !self.scaffold_path_classes.contains(&site.locator().class())
    }

    /// This rubric's answer about a whole change.
    #[must_use]
    pub fn judge(&self, sites: &[ChangedSite]) -> ChangeVerdict {
        let bearing: Vec<&ChangedSite> = sites
            .iter()
            .filter(|site| self.bears_understanding(site))
            .collect();
        let counted = u32::try_from(bearing.len()).unwrap_or(u32::MAX);
        if counted >= self.minimum_bearing_sites {
            ChangeVerdict::Meaningful {
                rubric: self.id.clone(),
                version: self.version,
                bearing_sites: bearing.into_iter().cloned().collect(),
            }
        } else {
            ChangeVerdict::ScaffoldOnly {
                rubric: self.id.clone(),
                version: self.version,
                bearing_sites: counted,
                required: self.minimum_bearing_sites,
            }
        }
    }
}

/// What a rubric said about one change.
///
/// Both arms carry the rubric and the version that produced them, so a verdict
/// read later says which configuration decided it rather than being read as an
/// absolute fact about the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeVerdict {
    /// Section 17.6's `이해가 필요한 선택·수정`, with the sites that carry it.
    Meaningful {
        /// Which rubric decided.
        rubric: RubricId,
        /// Which version of it.
        version: u64,
        /// The sites the rubric counted, in the change's own order.
        bearing_sites: Vec<ChangedSite>,
    },
    /// Section 17.6's `단순 scaffold`.
    ScaffoldOnly {
        /// Which rubric decided.
        rubric: RubricId,
        /// Which version of it.
        version: u64,
        /// How many sites the rubric counted.
        bearing_sites: u32,
        /// How many it wanted.
        required: u32,
    },
}

impl ChangeVerdict {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Meaningful { .. } => "MEANINGFUL",
            Self::ScaffoldOnly { .. } => "SCAFFOLD_ONLY",
        }
    }

    /// Which rubric decided.
    #[must_use]
    pub const fn rubric(&self) -> &RubricId {
        match self {
            Self::Meaningful { rubric, .. } | Self::ScaffoldOnly { rubric, .. } => rubric,
        }
    }

    /// Which version of it.
    #[must_use]
    pub const fn version(&self) -> u64 {
        match self {
            Self::Meaningful { version, .. } | Self::ScaffoldOnly { version, .. } => *version,
        }
    }

    /// The sites a `MEANINGFUL` verdict counted.
    #[must_use]
    pub fn bearing_sites(&self) -> &[ChangedSite] {
        match self {
            Self::Meaningful { bearing_sites, .. } => bearing_sites,
            Self::ScaffoldOnly { .. } => &[],
        }
    }
}
