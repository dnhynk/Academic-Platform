//! What a path is, before anything is concluded from what is inside it.
//!
//! Section 34.4's first row names four things that make a repository stack a
//! false positive — vendored code, example code, generated code, and one part
//! of a monorepo — and its *prevention* column is `generated/vendor/test 분리`.
//! So the separation is a classification of the path, computed before the bytes
//! are read, and it is two independent axes rather than one:
//!
//! * [`PathClass`] answers *may evidence here raise a tier at all*. Vendored,
//!   generated and example trees answer no. That is the promotion axis.
//! * [`ArtifactScope`] answers *what kind of use this would be* — section
//!   18.1's five values. A test-scope use is real evidence; it is evidence
//!   about tests.
//!
//! They are separate because they disagree. `crates/x/tests/a.rs` is
//! first-party code that promotes, at test scope; `vendor/y/src/main.rs` is
//! production-shaped code that promotes nothing. Collapsing them into one
//! enumeration would force one of those two answers to be wrong.
//!
//! The monorepo half of section 34.4's row is [`PackageMap`], and it is derived
//! from the snapshot rather than configured: a package is a directory holding a
//! manifest, so what counts as "another part of the monorepo" is what the
//! frozen manifest actually shows.

use std::collections::BTreeSet;

/// Whether evidence found at a path may raise a subject's evidence tier.
///
/// A closed four-value vocabulary with no `Unknown`: a path this analyzer has
/// no rule for is [`PathClass::FirstParty`], which is the answer that lets
/// evidence count. That direction is deliberate. An unclassified path treated
/// as non-promoting would silently drop evidence and the drop would look like
/// an absence of use; an unclassified path treated as first-party shows the
/// evidence, and the tier it produces is the one the ladder computes from the
/// rest of the corroboration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathClass {
    /// Code this repository is the author of.
    FirstParty,
    /// A dependency's own source, checked in.
    Vendored,
    /// Written by a tool from something else in the tree.
    Generated,
    /// A sample, benchmark or probe that ships beside the code it demonstrates.
    Example,
}

impl PathClass {
    /// Exhaustive order.
    pub const ALL: [Self; 4] = [
        Self::FirstParty,
        Self::Vendored,
        Self::Generated,
        Self::Example,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstParty => "FIRST_PARTY",
            Self::Vendored => "VENDORED",
            Self::Generated => "GENERATED",
            Self::Example => "EXAMPLE",
        }
    }

    /// Whether evidence at a path of this class may raise a tier.
    ///
    /// Total over the enumeration and written as a `match` with one arm per
    /// variant rather than as `self == Self::FirstParty`, so a fifth class
    /// added later has no arm and the crate stops compiling rather than
    /// defaulting to promoting.
    #[must_use]
    pub const fn promotes(self) -> bool {
        match self {
            Self::FirstParty => true,
            Self::Vendored | Self::Generated | Self::Example => false,
        }
    }
}

/// Section 18.1's `scope`: production, test, build, migration, development.
///
/// Every analyzed path has exactly one. There is no unscoped value, which is
/// `REQ-18-003`'s second half — *the API does not collapse to unscoped
/// observed* — held as the absence of a variant rather than as a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactScope {
    /// Ships and runs.
    Production,
    /// Exercises something else.
    Test,
    /// Produces the thing that ships.
    Build,
    /// Changes a schema forward.
    Migration,
    /// Used while developing and not shipped.
    Development,
}

impl ArtifactScope {
    /// Exhaustive order, in section 18.1's own order.
    pub const ALL: [Self; 5] = [
        Self::Production,
        Self::Test,
        Self::Build,
        Self::Migration,
        Self::Development,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "PRODUCTION",
            Self::Test => "TEST",
            Self::Build => "BUILD",
            Self::Migration => "MIGRATION",
            Self::Development => "DEVELOPMENT",
        }
    }

    /// How strongly a use at this scope speaks about what the system runs.
    ///
    /// Used to reduce a finding's several sites to one scope. Production wins,
    /// then migration — which runs against the production database — then
    /// build, then development, then test. A subject used in production *and*
    /// in tests is a production use; only a subject used **nowhere but** tests
    /// is section 17.3's fourth row.
    const fn rank(self) -> u8 {
        match self {
            Self::Production => 4,
            Self::Migration => 3,
            Self::Build => 2,
            Self::Development => 1,
            Self::Test => 0,
        }
    }

    /// The stronger of two scopes.
    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }
}

/// One path's classification: promotion class, scope, and owning package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathClassification {
    class: PathClass,
    scope: ArtifactScope,
    package: Option<PackageId>,
}

impl PathClassification {
    /// The promotion class.
    #[must_use]
    pub const fn class(&self) -> PathClass {
        self.class
    }

    /// Section 18.1's scope.
    #[must_use]
    pub const fn scope(&self) -> ArtifactScope {
        self.scope
    }

    /// The package this path belongs to, when the snapshot showed one.
    #[must_use]
    pub const fn package(&self) -> Option<&PackageId> {
        self.package.as_ref()
    }
}

/// A directory holding a manifest, named by that directory's path.
///
/// The repository root is spelled `.`, so a single-package repository has one
/// package and the identifier is still a real value rather than an empty
/// string. That matters because [`ComponentId`] refuses the empty string, and
/// the two would otherwise be confusable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageId {
    directory: String,
}

impl PackageId {
    /// The package directory, relative and forward-slashed, or `.` for a root.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.directory
    }
}

/// Which package each path belongs to, derived from a frozen manifest.
///
/// Section 34.4 names "monorepo 일부" as a source of stack false positives:
/// evidence in one package says nothing about another. This is the map that
/// makes that answerable, and it is built from the snapshot's own manifest
/// rows, so a package exists here exactly when the frozen tree holds its
/// manifest file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageMap {
    directories: BTreeSet<String>,
}

/// The file names that mark a directory as a package root.
const PACKAGE_MANIFESTS: [&str; 4] = ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"];

impl PackageMap {
    /// Builds the map from every path in a frozen manifest.
    #[must_use]
    pub fn of_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        let mut directories = BTreeSet::new();
        for path in paths {
            let (directory, name) = split_parent(path);
            if PACKAGE_MANIFESTS.contains(&name) {
                directories.insert(directory.to_owned());
            }
        }
        Self { directories }
    }

    /// Every package directory, sorted.
    #[must_use]
    pub fn packages(&self) -> Vec<PackageId> {
        self.directories
            .iter()
            .map(|directory| PackageId {
                directory: directory.clone(),
            })
            .collect()
    }

    /// The package owning a path: the longest package directory that is a
    /// prefix of it, or `None` when no manifest covers the path.
    #[must_use]
    pub fn package_of(&self, path: &str) -> Option<PackageId> {
        let mut best: Option<&String> = None;
        for directory in &self.directories {
            let covers = directory == "." || path.starts_with(&format!("{directory}/"));
            if covers && best.is_none_or(|current| directory.len() > current.len()) {
                best = Some(directory);
            }
        }
        best.map(|directory| PackageId {
            directory: directory.clone(),
        })
    }
}

/// A named part of the repository, which is the coarsest scope a finding may
/// have.
///
/// Section 34.4's prevention column for over-generalised snippets is *finding
/// scope를 symbol/component로 시작*. A component is a path inside the
/// repository — a directory, or a file at the root that has no directory to
/// widen to — and the constructor refuses every spelling of the root itself:
/// the empty string, `.`, `/`, `./`, and any path holding a `..`. So "the whole
/// repository" is not a value this type has. That refusal is the runtime half
/// of `new_finding_cannot_default_to_repository_scope`; the type half is that
/// [`crate::FindingScope`] has no repository variant to select.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId {
    directory: String,
}

/// Why a component identifier was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ComponentError {
    /// The identifier named the repository root rather than a directory in it.
    #[error("the component {0:?} is the repository root, which is not a component")]
    RepositoryRoot(String),
    /// The identifier was not a relative forward-slashed directory path.
    #[error("the component {0:?} is not a relative forward-slashed directory")]
    Malformed(String),
}

impl ComponentId {
    /// Names a directory inside the repository.
    ///
    /// # Errors
    ///
    /// [`ComponentError::RepositoryRoot`] for every spelling of the root, and
    /// [`ComponentError::Malformed`] for a path that is absolute, backslashed,
    /// or holds an empty, `.` or `..` segment.
    pub fn new(value: impl Into<String>) -> Result<Self, ComponentError> {
        let value = value.into();
        if value.is_empty() || value == "." || value == "/" || value == "./" {
            return Err(ComponentError::RepositoryRoot(value));
        }
        let malformed = value.starts_with('/')
            || value.contains('\\')
            || value.contains(':')
            || value
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..");
        if malformed {
            return Err(ComponentError::Malformed(value));
        }
        Ok(Self { directory: value })
    }

    /// The component a file path sits in.
    ///
    /// The containing directory, or — for a file at the repository root, which
    /// has no directory below the root to widen to — the file itself. The root
    /// is never the answer: widening a root-level manifest to `.` would be
    /// exactly the repository-wide scope this type exists to refuse.
    ///
    /// # Errors
    ///
    /// [`ComponentError::Malformed`] when the path is not a relative
    /// forward-slashed path, and [`ComponentError::RepositoryRoot`] when the
    /// path is a spelling of the root itself.
    pub fn containing(path: &str) -> Result<Self, ComponentError> {
        let (parent, _) = split_parent(path);
        if parent == "." {
            return Self::new(path);
        }
        Self::new(parent)
    }

    /// The path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.directory
    }
}

/// Splits a forward-slashed path into its parent directory and its file name.
///
/// A path with no `/` has parent `.`, which every constructor above refuses as
/// a component and accepts as a package.
fn split_parent(path: &str) -> (&str, &str) {
    path.rsplit_once('/').map_or((".", path), |(head, tail)| {
        (if head.is_empty() { "." } else { head }, tail)
    })
}

/// Directory names whose subtree is a checked-in copy of somebody else's code.
const VENDORED_DIRECTORIES: [&str; 6] = [
    "node_modules",
    "vendor",
    "vendored",
    "third_party",
    "thirdparty",
    "external",
];

/// Directory names whose subtree a tool wrote.
const GENERATED_DIRECTORIES: [&str; 6] = ["target", "dist", "build", "generated", "gen", "out"];

/// Directory names whose subtree demonstrates rather than ships.
///
/// `benches` and `probes` are here beside `examples` for the reason `S-12`
/// records: they are compiled by `cargo clippy --workspace --all-targets` and
/// look exactly like product code to a walk, so a scan that names only
/// `examples` reads the other two as first-party.
const EXAMPLE_DIRECTORIES: [&str; 6] = [
    "examples", "example", "benches", "probes", "samples", "fixtures",
];

/// Directory names whose subtree exists to exercise something else.
const TEST_DIRECTORIES: [&str; 5] = ["tests", "test", "__tests__", "spec", "testdata"];

/// Directory names whose subtree changes a schema forward.
const MIGRATION_DIRECTORIES: [&str; 2] = ["migrations", "migration"];

/// Directory names whose subtree produces the thing that ships.
const BUILD_DIRECTORIES: [&str; 4] = [".github", ".gitlab", "ci", ".circleci"];

/// Directory names whose subtree is used while developing and does not ship.
const DEVELOPMENT_DIRECTORIES: [&str; 5] = ["tools", "scripts", "docs", "doc", ".devcontainer"];

/// File names that are build inputs wherever they sit.
const BUILD_FILES: [&str; 3] = ["build.rs", "Makefile", "makefile"];

/// Whether any segment of `path` is one of `names`.
fn has_segment(path: &str, names: &[&str]) -> bool {
    path.split('/').any(|segment| names.contains(&segment))
}

/// Whether the file name marks the file as a test regardless of directory.
///
/// `a.test.ts`, `a.spec.ts` and `a_test.py` are the three shapes this
/// repository's own ecosystems use for a test file that sits beside the code
/// it tests rather than under a `tests` tree.
fn is_test_file_name(name: &str) -> bool {
    let stem = name.split_once('.').map_or(name, |(head, _)| head);
    name.contains(".test.")
        || name.contains(".spec.")
        || name.ends_with(".test.mjs")
        || stem.ends_with("_test")
        || stem.starts_with("test_")
}

/// Whether the file name marks the file as generated regardless of directory.
fn is_generated_file_name(name: &str) -> bool {
    name.contains(".generated.") || name.contains(".g.") || name.ends_with(".min.js")
}

/// The promotion class of a path.
///
/// Vendored is decided first: a `vendor/x/examples/` tree is vendored, and a
/// vendored dependency's generated output is somebody else's generated output.
/// Generated is decided next for the same reason against examples.
#[must_use]
pub fn class_of(path: &str) -> PathClass {
    let (_, name) = split_parent(path);
    if has_segment(path, &VENDORED_DIRECTORIES) {
        return PathClass::Vendored;
    }
    if has_segment(path, &GENERATED_DIRECTORIES) || is_generated_file_name(name) {
        return PathClass::Generated;
    }
    if has_segment(path, &EXAMPLE_DIRECTORIES) {
        return PathClass::Example;
    }
    PathClass::FirstParty
}

/// The section 18.1 scope of a path.
///
/// Ordered most specific first. A file under `migrations/` is a migration even
/// though the repository ships it; a file under `tests/` is a test even inside
/// `tools/`; and a path matching none of the rules is production, which is the
/// answer that makes an unclassified path count against the strongest claim
/// rather than hide from it.
#[must_use]
pub fn scope_of(path: &str) -> ArtifactScope {
    let (_, name) = split_parent(path);
    if has_segment(path, &MIGRATION_DIRECTORIES) {
        return ArtifactScope::Migration;
    }
    if has_segment(path, &TEST_DIRECTORIES) || is_test_file_name(name) {
        return ArtifactScope::Test;
    }
    if has_segment(path, &BUILD_DIRECTORIES) || BUILD_FILES.contains(&name) {
        return ArtifactScope::Build;
    }
    if has_segment(path, &DEVELOPMENT_DIRECTORIES) {
        return ArtifactScope::Development;
    }
    ArtifactScope::Production
}

/// Classifies one path on both axes and attributes it to a package.
#[must_use]
pub fn classify_path(path: &str, packages: &PackageMap) -> PathClassification {
    PathClassification {
        class: class_of(path),
        scope: scope_of(path),
        package: packages.package_of(path),
    }
}
