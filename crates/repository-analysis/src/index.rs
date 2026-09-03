//! Section 17.3's third stage: `AST, symbols, call/data flow, schema, config,
//! IaC`, and the coverage gap that stands where one of them cannot be answered.
//!
//! ## The index is total, by construction
//!
//! `REQ-17-011`'s acceptance is *each listed index kind emits typed locator;
//! unsupported kind explicitly reports coverage gap*. The failure that sentence
//! is written against is a silent skip: an analyzer that returns nothing for a
//! file it did not understand is indistinguishable from one that understood the
//! file and found nothing.
//!
//! So [`PathCoverage`] holds a fixed-size array with one slot per
//! [`IndexKind`], indexed by [`IndexKind::position`]. There is no path through
//! this module that leaves a slot unfilled, because the array has no unfilled
//! state: `[CoverageOutcome; IndexKind::COUNT]` is built by mapping over
//! [`IndexKind::ALL`]. A kind added to the enumeration changes `COUNT`, and
//! every construction site fails to compile until it answers for the new kind.
//!
//! ## Three outcomes, and why "not applicable" is not a gap
//!
//! [`CoverageOutcome::Analyzed`] is *this analyzer read the file for this kind*
//! and carries how many facts came out, including zero.
//! [`CoverageOutcome::NotApplicable`] is *this kind is not a question about
//! this file* — a Rust source file has no infrastructure-as-code facts, and
//! reporting a gap there would make the gap list noise rather than a list of
//! things the analyzer cannot do. [`CoverageOutcome::Gap`] is the honest
//! absence: the analyzer has no reader for this file at all.
//!
//! The three are separated by [`support`], which is a total function over
//! [`FileKind`] × [`IndexKind`] with no default arm, so the day a file kind is
//! added the matrix is what has to be extended.
//!
//! ## Nothing derived from a file's bytes is text here
//!
//! A symbol name read out of a repository is untrusted content, and this crate
//! cannot seal one — `Untrusted::seal` is private to `academic-untrusted-
//! content`. So a symbol is identified by [`SymbolFingerprint`], which is a
//! digest, which is section 17.4's own word: *blob hash, symbol fingerprint,
//! syntax span과 commit을 함께 저장하고*. Paths are text and stay text, because
//! `academic-repository`'s own manifest already hands those out and the gate
//! classified them before anything opened a file.

use academic_policy::ContentDigest;

use crate::paths::{ArtifactScope, PathClass};

/// One of section 17.3's six index kinds, plus the data-flow half of its
/// fourth.
///
/// Section 17.3 draws `AST, symbols, call/data flow, schema, config, IaC`.
/// Call flow and data flow are one phrase there and two questions here: a
/// reachable call and a value that reaches a sink are different evidence, and
/// section 17.3's own tier table asks about the first without asking about the
/// second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndexKind {
    /// The syntactic shape of the file.
    Ast,
    /// The named things it declares.
    Symbol,
    /// Which declaration calls which.
    CallFlow,
    /// Which declared value reaches which use.
    DataFlow,
    /// Tables, columns and indexes.
    Schema,
    /// Keys a running system reads.
    Config,
    /// Infrastructure, deployment and pipeline definitions.
    Iac,
}

impl IndexKind {
    /// Exhaustive order, in section 17.3's own order.
    pub const ALL: [Self; 7] = [
        Self::Ast,
        Self::Symbol,
        Self::CallFlow,
        Self::DataFlow,
        Self::Schema,
        Self::Config,
        Self::Iac,
    ];

    /// How many kinds there are. The width of every coverage array.
    pub const COUNT: usize = Self::ALL.len();

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ast => "AST",
            Self::Symbol => "SYMBOL",
            Self::CallFlow => "CALL_FLOW",
            Self::DataFlow => "DATA_FLOW",
            Self::Schema => "SCHEMA",
            Self::Config => "CONFIG",
            Self::Iac => "IAC",
        }
    }

    /// This kind's slot in a coverage array.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::Ast => 0,
            Self::Symbol => 1,
            Self::CallFlow => 2,
            Self::DataFlow => 3,
            Self::Schema => 4,
            Self::Config => 5,
            Self::Iac => 6,
        }
    }
}

/// What this analyzer recognises a file as, from its path alone.
///
/// A closed vocabulary of the file shapes this analyzer has a reader for, plus
/// [`FileKind::Unsupported`] for everything else. It is keyed on the file name
/// as well as the extension because two of section 17.1's listed artefact kinds
/// — `Dockerfile` and a CI workflow — are identified by name and by directory
/// rather than by suffix.
///
/// This is a second classification beside `academic-repository`'s `Language`
/// and not a replacement for it. That one answers *what language are these
/// bytes*, which is what a manifest row records; this one answers *which reader
/// does this analyzer have*, which is what decides a coverage gap. They agree
/// where they overlap and `file_kind_and_manifest_language_agree` is what says
/// so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileKind {
    /// A Rust source file.
    RustSource,
    /// A TypeScript or JavaScript source file.
    TypeScriptSource,
    /// A Python source file.
    PythonSource,
    /// A SQL script.
    SqlScript,
    /// `Cargo.toml`.
    CargoManifest,
    /// `package.json`.
    NodeManifest,
    /// `pyproject.toml` or `requirements.txt`.
    PythonManifest,
    /// A dependency lock file.
    LockFile,
    /// A TOML, YAML or JSON document that is not a manifest or a lock file.
    ConfigDocument,
    /// A `Dockerfile` or `Containerfile`.
    ContainerFile,
    /// A container compose file.
    ComposeFile,
    /// A continuous-integration workflow.
    CiWorkflow,
    /// Prose: Markdown or plain text.
    Prose,
    /// A file this analyzer has no reader for.
    Unsupported,
}

impl FileKind {
    /// Exhaustive order.
    pub const ALL: [Self; 14] = [
        Self::RustSource,
        Self::TypeScriptSource,
        Self::PythonSource,
        Self::SqlScript,
        Self::CargoManifest,
        Self::NodeManifest,
        Self::PythonManifest,
        Self::LockFile,
        Self::ConfigDocument,
        Self::ContainerFile,
        Self::ComposeFile,
        Self::CiWorkflow,
        Self::Prose,
        Self::Unsupported,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustSource => "RUST_SOURCE",
            Self::TypeScriptSource => "TYPESCRIPT_SOURCE",
            Self::PythonSource => "PYTHON_SOURCE",
            Self::SqlScript => "SQL_SCRIPT",
            Self::CargoManifest => "CARGO_MANIFEST",
            Self::NodeManifest => "NODE_MANIFEST",
            Self::PythonManifest => "PYTHON_MANIFEST",
            Self::LockFile => "LOCK_FILE",
            Self::ConfigDocument => "CONFIG_DOCUMENT",
            Self::ContainerFile => "CONTAINER_FILE",
            Self::ComposeFile => "COMPOSE_FILE",
            Self::CiWorkflow => "CI_WORKFLOW",
            Self::Prose => "PROSE",
            Self::Unsupported => "UNSUPPORTED",
        }
    }

    /// The languages this analyzer supports, as the list a coverage report
    /// prints. Every other file kind is a manifest, a document or unsupported.
    pub const SUPPORTED_LANGUAGES: [Self; 4] = [
        Self::RustSource,
        Self::TypeScriptSource,
        Self::PythonSource,
        Self::SqlScript,
    ];

    /// Recognises a file from its relative forward-slashed path.
    #[must_use]
    pub fn of_path(path: &str) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path);
        let extension = name.rsplit_once('.').map_or("", |(_, tail)| tail);
        let lowered = name.to_ascii_lowercase();
        if lowered.starts_with("dockerfile") || lowered.starts_with("containerfile") {
            return Self::ContainerFile;
        }
        if lowered.starts_with("docker-compose") || lowered.starts_with("compose.") {
            return Self::ComposeFile;
        }
        if path.contains(".github/workflows/") && matches!(extension, "yml" | "yaml") {
            return Self::CiWorkflow;
        }
        match name {
            "Cargo.toml" => return Self::CargoManifest,
            "package.json" => return Self::NodeManifest,
            "pyproject.toml" | "requirements.txt" => return Self::PythonManifest,
            "Cargo.lock" | "package-lock.json" | "pnpm-lock.yaml" | "poetry.lock" => {
                return Self::LockFile;
            }
            _ => (),
        }
        match extension {
            "rs" => Self::RustSource,
            "ts" | "tsx" | "js" | "mjs" | "cjs" => Self::TypeScriptSource,
            "py" => Self::PythonSource,
            "sql" => Self::SqlScript,
            "toml" | "yaml" | "yml" | "json" => Self::ConfigDocument,
            "md" | "txt" => Self::Prose,
            _ => Self::Unsupported,
        }
    }
}

/// Whether this analyzer answers one index kind for one file kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Support {
    /// There is a reader; the answer may still be zero facts.
    Analyzed,
    /// The question does not apply to this file kind.
    NotApplicable,
    /// There is no reader. This is what becomes a coverage gap.
    Unsupported,
}

/// The support matrix, as a total function with no default arm.
///
/// Written as a `match` over both enumerations rather than as a table lookup,
/// so a new [`FileKind`] or a new [`IndexKind`] is a compile error here before
/// it is a hole in a report.
#[must_use]
pub const fn support(file: FileKind, index: IndexKind) -> Support {
    match file {
        FileKind::Unsupported => Support::Unsupported,
        FileKind::RustSource | FileKind::TypeScriptSource | FileKind::PythonSource => match index {
            IndexKind::Ast | IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow => {
                Support::Analyzed
            }
            IndexKind::Config => Support::Analyzed,
            IndexKind::Schema | IndexKind::Iac => Support::NotApplicable,
        },
        FileKind::SqlScript => match index {
            IndexKind::Ast | IndexKind::Schema => Support::Analyzed,
            IndexKind::Symbol
            | IndexKind::CallFlow
            | IndexKind::DataFlow
            | IndexKind::Config
            | IndexKind::Iac => Support::NotApplicable,
        },
        FileKind::CargoManifest
        | FileKind::NodeManifest
        | FileKind::PythonManifest
        | FileKind::LockFile => match index {
            IndexKind::Ast | IndexKind::Config => Support::Analyzed,
            IndexKind::Symbol
            | IndexKind::CallFlow
            | IndexKind::DataFlow
            | IndexKind::Schema
            | IndexKind::Iac => Support::NotApplicable,
        },
        FileKind::ConfigDocument => match index {
            IndexKind::Ast | IndexKind::Config => Support::Analyzed,
            IndexKind::Symbol
            | IndexKind::CallFlow
            | IndexKind::DataFlow
            | IndexKind::Schema
            | IndexKind::Iac => Support::NotApplicable,
        },
        FileKind::ContainerFile | FileKind::ComposeFile | FileKind::CiWorkflow => match index {
            IndexKind::Ast | IndexKind::Config | IndexKind::Iac => Support::Analyzed,
            IndexKind::Symbol | IndexKind::CallFlow | IndexKind::DataFlow | IndexKind::Schema => {
                Support::NotApplicable
            }
        },
        FileKind::Prose => match index {
            IndexKind::Ast
            | IndexKind::Symbol
            | IndexKind::CallFlow
            | IndexKind::DataFlow
            | IndexKind::Schema
            | IndexKind::Config
            | IndexKind::Iac => Support::NotApplicable,
        },
    }
}

/// Why this analyzer cannot answer an index kind for a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoverageGapReason {
    /// The file's language or format has no reader in this analyzer.
    UnsupportedLanguage,
    /// No bytes reached the analyzer for this path. `academic-repository`
    /// manifests a file it cannot read as bounded text by digest and does not
    /// ingest it, so a manifest row can exist with nothing to analyze.
    BytesNotIngested,
}

impl CoverageGapReason {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::UnsupportedLanguage, Self::BytesNotIngested];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedLanguage => "UNSUPPORTED_LANGUAGE",
            Self::BytesNotIngested => "BYTES_NOT_INGESTED",
        }
    }
}

/// What this analyzer did about one index kind for one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageOutcome {
    /// A reader ran. The count is how many facts it produced, possibly zero.
    Analyzed(u32),
    /// The question does not apply to this file kind.
    NotApplicable,
    /// No reader exists. This is the coverage gap section 17.3 requires.
    Gap(CoverageGapReason),
}

impl CoverageOutcome {
    /// Whether this outcome is a coverage gap.
    #[must_use]
    pub const fn is_gap(self) -> bool {
        matches!(self, Self::Gap(_))
    }
}

/// One path's answer for every index kind.
///
/// The array is the totality: there is one slot per [`IndexKind`] and no
/// construction that leaves one empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCoverage {
    path: String,
    file_kind: FileKind,
    class: PathClass,
    scope: ArtifactScope,
    outcomes: [CoverageOutcome; IndexKind::COUNT],
}

impl PathCoverage {
    /// Builds one path's coverage by asking `answer` for every index kind.
    pub(crate) fn build(
        path: String,
        file_kind: FileKind,
        class: PathClass,
        scope: ArtifactScope,
        mut answer: impl FnMut(IndexKind) -> CoverageOutcome,
    ) -> Self {
        Self {
            path,
            file_kind,
            class,
            scope,
            outcomes: IndexKind::ALL.map(&mut answer),
        }
    }

    /// The path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// What this analyzer recognised the file as.
    #[must_use]
    pub const fn file_kind(&self) -> FileKind {
        self.file_kind
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

    /// What happened for one index kind.
    #[must_use]
    pub fn outcome(&self, kind: IndexKind) -> CoverageOutcome {
        self.outcomes[kind.position()]
    }

    /// Every index kind this path is a gap for.
    #[must_use]
    pub fn gaps(&self) -> Vec<(IndexKind, CoverageGapReason)> {
        IndexKind::ALL
            .iter()
            .filter_map(|&kind| match self.outcome(kind) {
                CoverageOutcome::Gap(reason) => Some((kind, reason)),
                CoverageOutcome::Analyzed(_) | CoverageOutcome::NotApplicable => None,
            })
            .collect()
    }
}

/// What a symbol is called, without saying what it is called.
///
/// Section 17.4 asks a locator to carry a *symbol fingerprint* beside the blob
/// hash and the syntax span. A fingerprint rather than a name is what this
/// crate can hold: a name read out of a repository is untrusted content, and
/// `academic-untrusted-content`'s wrapper cannot be constructed here, so a
/// value carrying the name would be an unlabelled copy of it.
///
/// The digest covers the path, the kind and the name, so the same name in two
/// files is two fingerprints and a rename is a new one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolFingerprint {
    digest: ContentDigest,
}

impl SymbolFingerprint {
    /// Fingerprints one declaration.
    #[must_use]
    pub fn of(path: &str, kind: SymbolKind, name: &str) -> Self {
        let mut preimage = b"academic-repository-analysis-symbol-v1\0".to_vec();
        preimage.extend_from_slice(path.as_bytes());
        preimage.push(0);
        preimage.extend_from_slice(kind.as_str().as_bytes());
        preimage.push(0);
        preimage.extend_from_slice(name.as_bytes());
        Self {
            digest: ContentDigest::of(&preimage),
        }
    }

    /// The digest, as the hexadecimal a locator records.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.digest.as_str()
    }
}

/// What kind of thing a symbol is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolKind {
    /// A free function.
    Function,
    /// A method or an associated function.
    Method,
    /// A type declaration.
    Type,
    /// A constant or a module-level binding.
    Constant,
    /// A test function.
    TestFunction,
    /// A schema object: a table, a view or an index.
    SchemaObject,
}

impl SymbolKind {
    /// Exhaustive order.
    pub const ALL: [Self; 6] = [
        Self::Function,
        Self::Method,
        Self::Type,
        Self::Constant,
        Self::TestFunction,
        Self::SchemaObject,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "FUNCTION",
            Self::Method => "METHOD",
            Self::Type => "TYPE",
            Self::Constant => "CONSTANT",
            Self::TestFunction => "TEST_FUNCTION",
            Self::SchemaObject => "SCHEMA_OBJECT",
        }
    }
}

/// A half-open byte range inside one file, and the lines it spans.
///
/// Section 17.4 asks for a `lineSpan`; the byte range is beside it because a
/// line number moves when a line above it changes and a byte range is what a
/// digest can be recomputed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceSpan {
    start: u32,
    end: u32,
    first_line: u32,
    last_line: u32,
}

impl SourceSpan {
    /// Builds a span from a byte range and the line numbers it covers.
    #[must_use]
    pub const fn new(start: u32, end: u32, first_line: u32, last_line: u32) -> Self {
        Self {
            start,
            end,
            first_line,
            last_line,
        }
    }

    /// First byte offset.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// One past the last byte offset.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }

    /// Section 17.4's `lineSpan`, one-based and inclusive.
    #[must_use]
    pub const fn line_span(self) -> (u32, u32) {
        (self.first_line, self.last_line)
    }
}
