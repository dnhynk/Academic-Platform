//! The §3.9 deterministic engine contract and the §28 engine registry.
//!
//! [`schemas/registry/engine-registry-v1.json`] is the single source of truth.
//! [`generated`] is rendered from it by `tools/engine-registry.mjs` and is
//! compared byte-for-byte against a fresh render by `pnpm verify:contracts`, so
//! a hand edit to either side fails the build.
//!
//! What this module fixes:
//!
//! - **The engine signature.** A deterministic engine is
//!   `(frozen_inputs, rule_set_hash, engine_version) -> (result, proof_tree,
//!   explanation_snapshot)`. [`DeterministicEngine::evaluate`] takes `&self`
//!   and every other input by value or shared reference, so an engine has no
//!   place to keep ambient state between two calls.
//! - **No clock, RNG, network, or model.** Nothing here reads a clock, draws
//!   randomness, opens a socket, or calls a model, and the prohibition is
//!   enforced by a source and dependency scan rather than by convention. See
//!   `docs/contracts/engine-harness.md`.
//! - **The proof-tree node shape.** [`ProofNode`] is exactly `{node_id,
//!   rule_id, status, inputs, source_locators, children}` with the fixed
//!   five-value [`ProofStatus`].
//! - **Typed errors, never a panic.** [`FrozenInputs::parse`] and
//!   [`ProofNode::validate`] return [`EngineError`] for every malformed input.
//!   An engine that panics on a malformed input violates §2.3-11.
//! - **Byte equality.** [`EngineOutcome::canonical_bytes`] is a total,
//!   order-independent encoding of the whole output, so "same inputs and rule
//!   hash yield the same result" is a byte comparison rather than an absence of
//!   errors.
//!
//! The registry names the thirteen §28 engines and none of them is implemented
//! yet. [`EngineLifecycle::Planned`] records that, and the harness audit
//! ([`audit_engine_harness`]) fails a planned entry that has acquired harness
//! artifacts as well as an implemented entry that is missing any.

mod generated;

pub use generated::{ENGINE_REGISTRY, ENGINE_REGISTRY_VERSION, EngineName, HARNESS_ROOT, SpecRow};

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use thiserror::Error;

use crate::{ArtifactId, ContentDigest, Decimal, EvidenceLocator};

/// The four adverse paths a high-impact engine must additionally cover.
///
/// §28 requires the high-impact engines to test more than a successful
/// computation. [`AdversePath`] is the enumeration the harness audit counts, so
/// "four domains times three adverse modes" is executable rather than declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdversePath {
    /// An input the engine needs is not known.
    Unknown,
    /// Two admitted sources disagree about an input the engine used.
    Conflict,
    /// Some rules could not be evaluated; the rest still were.
    PartialFailure,
}

impl AdversePath {
    /// Every adverse path, in registry order.
    pub const ALL: [Self; 3] = [Self::Unknown, Self::Conflict, Self::PartialFailure];

    /// Returns the registry spelling of the adverse path.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Conflict => "CONFLICT",
            Self::PartialFailure => "PARTIAL_FAILURE",
        }
    }

    /// Returns the harness subdirectory an adverse fixture set lives in.
    #[must_use]
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Conflict => "conflict",
            Self::PartialFailure => "partial_failure",
        }
    }
}

/// The four artifact classes every registered engine must ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactClass {
    /// Frozen input and output pairs replayed byte-for-byte.
    GoldenFixtures,
    /// Generated-input tests over the engine's declared invariants.
    PropertyTests,
    /// Fixtures produced under an earlier engine version and replayed here.
    VersionCompatFixtures,
    /// The normalized explanation rendering compared for semantic drift.
    ExplanationSnapshot,
}

impl ArtifactClass {
    /// Every artifact class, in registry order.
    pub const ALL: [Self; 4] = [
        Self::GoldenFixtures,
        Self::PropertyTests,
        Self::VersionCompatFixtures,
        Self::ExplanationSnapshot,
    ];

    /// Returns the registry spelling of the artifact class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GoldenFixtures => "GOLDEN_FIXTURES",
            Self::PropertyTests => "PROPERTY_TESTS",
            Self::VersionCompatFixtures => "VERSION_COMPAT_FIXTURES",
            Self::ExplanationSnapshot => "EXPLANATION_SNAPSHOT",
        }
    }

    /// Returns the harness path the class occupies under an engine directory.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::GoldenFixtures => "golden",
            Self::PropertyTests => "property",
            Self::VersionCompatFixtures => "version-compat",
            Self::ExplanationSnapshot => "explanation.snapshot",
        }
    }
}

/// The high-impact paths §3.9 names: GPA, graduation, deletion, and egress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HighImpactPath {
    /// The GPA computation a grade or a repeat decision depends on.
    Gpa,
    /// The graduation verdict.
    Graduation,
    /// The deletion plan and its execution.
    Deletion,
    /// The decision that lets data leave the device.
    Egress,
}

impl HighImpactPath {
    /// Every high-impact path, in registry order.
    pub const ALL: [Self; 4] = [Self::Gpa, Self::Graduation, Self::Deletion, Self::Egress];

    /// Returns the registry spelling of the high-impact path.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gpa => "GPA",
            Self::Graduation => "GRADUATION",
            Self::Deletion => "DELETION",
            Self::Egress => "EGRESS",
        }
    }
}

/// Whether a registered engine has an implementation yet.
///
/// Registration is the contract; implementation arrives with the task that owns
/// the engine. A `Planned` entry is not a placeholder engine — no engine is
/// invented here — it is the registry stating that the engine is named, its
/// harness obligations are fixed, and nothing implements it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EngineLifecycle {
    /// Named and obligated; no implementation and no harness artifacts exist.
    Planned,
    /// Implemented; every artifact class must be present and executable.
    Implemented,
}

impl EngineLifecycle {
    /// Returns the registry spelling of the lifecycle state.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "PLANNED",
            Self::Implemented => "IMPLEMENTED",
        }
    }
}

/// Everything the registry fixes about one §28 engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineDescriptor {
    /// The registry name, which is the §28 table cell in screaming snake case.
    pub name: EngineName,
    /// The stable identifier an engine's outputs and audit rows are keyed by.
    pub engine_id: &'static str,
    /// The §28 requirement this engine closes.
    pub requirement_id: &'static str,
    /// The registry version that introduced the entry.
    pub since_registry_version: u16,
    /// The §28 table row, verbatim, for the twelve tabulated engines.
    pub spec_row: Option<SpecRow>,
    /// The §28 prose sentence, verbatim, for the engine with no table row.
    pub spec_sentence: Option<&'static str>,
    /// The high-impact path this engine decides, when it decides one.
    pub high_impact_path: Option<HighImpactPath>,
    /// Whether an implementation exists yet.
    pub lifecycle: EngineLifecycle,
    /// The engine's directory under [`HARNESS_ROOT`].
    pub harness_dir: &'static str,
}

/// A rule identifier carried by every proof-tree node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(String);

/// A proof-tree node identifier, unique within one tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(String);

/// A key into [`FrozenInputs`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputKey(String);

/// The characters admitted in a rule, node, or input identifier.
///
/// Deliberately narrow: the canonical encodings below separate fields with
/// `=`, `:`, and newline, so an identifier that could contain one of those
/// would make the encoding ambiguous and the byte comparison meaningless.
fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

macro_rules! identifier_newtype {
    ($name:ident, $kind:literal) => {
        impl $name {
            /// Validates and constructs the identifier.
            pub fn new(value: &str) -> Result<Self, EngineError> {
                if is_identifier(value) {
                    Ok(Self(value.to_owned()))
                } else {
                    Err(EngineError::InvalidIdentifier {
                        kind: $kind,
                        value: value.to_owned(),
                    })
                }
            }

            /// Returns the identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier_newtype!(RuleId, "rule id");
identifier_newtype!(NodeId, "node id");
identifier_newtype!(InputKey, "input key");

/// The published rule set an evaluation is pinned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleSetHash(ContentDigest);

impl RuleSetHash {
    /// Wraps a digest of the published rule set bytes.
    #[must_use]
    pub const fn new(digest: ContentDigest) -> Self {
        Self(digest)
    }

    /// Returns the wrapped digest.
    #[must_use]
    pub const fn digest(self) -> ContentDigest {
        self.0
    }
}

impl fmt::Display for RuleSetHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The engine's own version, which changes when its computation changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineVersion(u16);

impl EngineVersion {
    /// The lowest admitted engine version.
    pub const MIN: Self = Self(1);

    /// Validates and constructs a version; zero is not a version.
    pub fn new(value: u16) -> Result<Self, EngineError> {
        if value == 0 {
            return Err(EngineError::InvalidEngineVersion);
        }
        Ok(Self(value))
    }

    /// Returns the version number.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for EngineVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One frozen input value.
///
/// [`InputValue::Unknown`] is a first-class value, not a missing key. An input
/// the user has not supplied and an official fact nobody has confirmed are both
/// known to be unknown, and an engine that folds them into a default would
/// manufacture a pass or a fail out of nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValue {
    /// An exact integer.
    Integer(i64),
    /// An exact base-10 decimal.
    Decimal(Decimal),
    /// An identifier-shaped reference; never a sentence.
    Reference(String),
    /// The value is declared and is not known.
    Unknown,
}

/// The frozen inputs half of the engine signature.
///
/// The map is ordered and the canonical encoding is a total function of the
/// entries, so two independently built input sets with the same content have
/// the same [`FrozenInputs::digest`] regardless of insertion order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenInputs {
    entries: BTreeMap<InputKey, InputValue>,
}

impl FrozenInputs {
    /// Builds a frozen input set from key/value pairs, rejecting duplicates.
    pub fn new(
        entries: impl IntoIterator<Item = (InputKey, InputValue)>,
    ) -> Result<Self, EngineError> {
        let mut map = BTreeMap::new();
        for (key, value) in entries {
            if let InputValue::Reference(reference) = &value
                && !is_identifier(reference)
            {
                return Err(EngineError::InvalidIdentifier {
                    kind: "input reference",
                    value: reference.clone(),
                });
            }
            if map.insert(key.clone(), value).is_some() {
                return Err(EngineError::DuplicateInputKey(key.0));
            }
        }
        Ok(Self { entries: map })
    }

    /// Parses the canonical encoding, returning a typed error for every
    /// malformed input rather than panicking (§2.3-11).
    pub fn parse(input: &str) -> Result<Self, EngineError> {
        let mut entries = Vec::new();
        if input.is_empty() {
            return Self::new(entries);
        }
        let Some(body) = input.strip_suffix('\n') else {
            return Err(EngineError::MalformedInput("input must end with a newline"));
        };
        let mut previous: Option<&str> = None;
        for line in body.split('\n') {
            let Some((key, encoded)) = line.split_once('=') else {
                return Err(EngineError::MalformedInput(
                    "input line has no '=' separator",
                ));
            };
            if previous.is_some_and(|earlier| earlier >= key) {
                return Err(EngineError::MalformedInput(
                    "input keys must be strictly ascending",
                ));
            }
            previous = Some(key);
            entries.push((InputKey::new(key)?, parse_input_value(encoded)?));
        }
        Self::new(entries)
    }

    /// Renders the canonical encoding [`FrozenInputs::parse`] accepts.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = String::new();
        for (key, value) in &self.entries {
            rendered.push_str(key.as_str());
            rendered.push('=');
            rendered.push_str(&render_input_value(value));
            rendered.push('\n');
        }
        rendered
    }

    /// Returns the digest of the canonical encoding.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(self.canonical_text().as_bytes())
    }

    /// Returns a value, or `None` when the key is not declared at all.
    #[must_use]
    pub fn get(&self, key: &InputKey) -> Option<&InputValue> {
        self.entries.get(key)
    }

    /// Returns every declared key, in canonical order.
    pub fn keys(&self) -> impl Iterator<Item = &InputKey> {
        self.entries.keys()
    }
}

fn parse_input_value(encoded: &str) -> Result<InputValue, EngineError> {
    if encoded == "unknown" {
        return Ok(InputValue::Unknown);
    }
    let Some((tag, body)) = encoded.split_once(':') else {
        return Err(EngineError::MalformedInput("input value has no type tag"));
    };
    match tag {
        "int" => {
            let parsed: i64 = body
                .parse()
                .map_err(|_| EngineError::MalformedInput("input integer is not an i64"))?;
            if parsed.to_string() != body {
                return Err(EngineError::MalformedInput(
                    "input integer is not canonically spelled",
                ));
            }
            Ok(InputValue::Integer(parsed))
        }
        "dec" => {
            let Some((coefficient_text, scale_text)) = body.split_once('/') else {
                return Err(EngineError::MalformedInput(
                    "input decimal needs a coefficient and a scale",
                ));
            };
            let coefficient: i128 = coefficient_text.parse().map_err(|_| {
                EngineError::MalformedInput("input decimal coefficient is not an i128")
            })?;
            let scale: u8 = scale_text
                .parse()
                .map_err(|_| EngineError::MalformedInput("input decimal scale is not a u8"))?;
            if coefficient.to_string() != coefficient_text || scale.to_string() != scale_text {
                return Err(EngineError::MalformedInput(
                    "input decimal is not canonically spelled",
                ));
            }
            Ok(InputValue::Decimal(Decimal::new(coefficient, scale)?))
        }
        "ref" => {
            if !is_identifier(body) {
                return Err(EngineError::InvalidIdentifier {
                    kind: "input reference",
                    value: body.to_owned(),
                });
            }
            Ok(InputValue::Reference(body.to_owned()))
        }
        _ => Err(EngineError::MalformedInput("unknown input type tag")),
    }
}

fn render_input_value(value: &InputValue) -> String {
    match value {
        InputValue::Integer(integer) => format!("int:{integer}"),
        InputValue::Decimal(decimal) => {
            format!("dec:{}/{}", decimal.coefficient(), decimal.scale())
        }
        InputValue::Reference(reference) => format!("ref:{reference}"),
        InputValue::Unknown => "unknown".to_owned(),
    }
}

/// The fixed five-value proof-tree node status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofStatus {
    /// The rule holds on the frozen inputs.
    Satisfied,
    /// The rule does not hold yet and the shortfall is quantified.
    Needs,
    /// The rule does not hold and no admitted path closes it.
    NotSatisfied,
    /// An input the rule needs is not known.
    Unknown,
    /// Two admitted sources disagree about an input the rule used.
    Conflict,
}

impl ProofStatus {
    /// Every status, in the §3.9 order.
    pub const ALL: [Self; 5] = [
        Self::Satisfied,
        Self::Needs,
        Self::NotSatisfied,
        Self::Unknown,
        Self::Conflict,
    ];

    /// Returns the contract spelling of the status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "SATISFIED",
            Self::Needs => "NEEDS",
            Self::NotSatisfied => "NOT_SATISFIED",
            Self::Unknown => "UNKNOWN",
            Self::Conflict => "CONFLICT",
        }
    }
}

impl fmt::Display for ProofStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// An immutable location inside a named artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocator {
    /// The artifact the span belongs to.
    pub artifact_id: ArtifactId,
    /// The exact span inside it.
    pub locator: EvidenceLocator,
}

impl SourceLocator {
    /// Renders the locator deterministically for the canonical encodings.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let span = match &self.locator {
            EvidenceLocator::Page { page_number } => format!("page/{page_number}"),
            EvidenceLocator::TextBytes {
                source_digest,
                start,
                end,
            } => format!("text/{source_digest}/{start}-{end}"),
            EvidenceLocator::TranscriptTime { start_ms, end_ms } => {
                format!("transcript/{start_ms}-{end_ms}")
            }
            EvidenceLocator::RepositoryBytes {
                snapshot_digest,
                path,
                start,
                end,
            } => format!("repo/{snapshot_digest}/{}/{start}-{end}", path.as_str()),
        };
        format!("{}@{span}", self.artifact_id)
    }
}

/// The fixed §3.9 proof-tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofNode {
    /// Unique within one tree.
    pub node_id: NodeId,
    /// The published rule this node evaluated.
    pub rule_id: RuleId,
    /// The node's verdict.
    pub status: ProofStatus,
    /// The frozen input keys this node read.
    pub inputs: Vec<InputKey>,
    /// The immutable spans this node's verdict rests on.
    pub source_locators: Vec<SourceLocator>,
    /// Sub-rules, ordered by `node_id`.
    pub children: Vec<ProofNode>,
}

impl ProofNode {
    /// Validates the whole subtree against the frozen inputs it claims to read.
    ///
    /// Every failure is a typed [`EngineError`]; a malformed tree never panics.
    pub fn validate(&self, inputs: &FrozenInputs) -> Result<(), EngineError> {
        let mut seen = BTreeSet::new();
        self.validate_into(inputs, &mut seen)
    }

    fn validate_into(
        &self,
        inputs: &FrozenInputs,
        seen: &mut BTreeSet<NodeId>,
    ) -> Result<(), EngineError> {
        if !seen.insert(self.node_id.clone()) {
            return Err(EngineError::DuplicateNodeId(self.node_id.0.clone()));
        }
        for key in &self.inputs {
            if inputs.get(key).is_none() {
                return Err(EngineError::UndeclaredInput(key.0.clone()));
            }
        }
        let mut sorted = self.inputs.clone();
        sorted.sort();
        sorted.dedup();
        if sorted != self.inputs {
            return Err(EngineError::UnorderedProofField {
                node_id: self.node_id.0.clone(),
                field: "inputs",
            });
        }
        let locators: Vec<String> = self
            .source_locators
            .iter()
            .map(SourceLocator::canonical_text)
            .collect();
        let mut sorted_locators = locators.clone();
        sorted_locators.sort();
        sorted_locators.dedup();
        if sorted_locators != locators {
            return Err(EngineError::UnorderedProofField {
                node_id: self.node_id.0.clone(),
                field: "source_locators",
            });
        }
        for locator in &self.source_locators {
            locator.locator.validate()?;
        }
        let mut previous: Option<&NodeId> = None;
        for child in &self.children {
            if previous.is_some_and(|earlier| earlier >= &child.node_id) {
                return Err(EngineError::UnorderedProofField {
                    node_id: self.node_id.0.clone(),
                    field: "children",
                });
            }
            previous = Some(&child.node_id);
            child.validate_into(inputs, seen)?;
        }
        Ok(())
    }

    /// Returns every node in the subtree, root first.
    #[must_use]
    pub fn walk(&self) -> Vec<&Self> {
        let mut nodes = vec![self];
        for child in &self.children {
            nodes.extend(child.walk());
        }
        nodes
    }

    fn render_into(&self, depth: usize, rendered: &mut String) {
        for _ in 0..depth {
            rendered.push_str("  ");
        }
        rendered.push_str(self.status.as_str());
        rendered.push(' ');
        rendered.push_str(self.rule_id.as_str());
        rendered.push_str(" [");
        rendered.push_str(self.node_id.as_str());
        rendered.push(']');
        if !self.inputs.is_empty() {
            rendered.push_str(" inputs=");
            for (index, key) in self.inputs.iter().enumerate() {
                if index > 0 {
                    rendered.push(',');
                }
                rendered.push_str(key.as_str());
            }
        }
        if !self.source_locators.is_empty() {
            rendered.push_str(" locators=");
            for (index, locator) in self.source_locators.iter().enumerate() {
                if index > 0 {
                    rendered.push(',');
                }
                rendered.push_str(&locator.canonical_text());
            }
        }
        rendered.push('\n');
        for child in &self.children {
            child.render_into(depth + 1, rendered);
        }
    }
}

/// The result half of an engine's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineResult {
    /// The engine's overall verdict.
    pub status: ProofStatus,
    /// Typed engine outputs. Nothing structured is smuggled through free text.
    pub values: BTreeMap<String, Decimal>,
    /// Rules the engine could not evaluate. Non-empty is a partial failure.
    pub unevaluated: Vec<RuleId>,
}

impl EngineResult {
    /// Whether some declared rules were not evaluated.
    #[must_use]
    pub fn is_partial_failure(&self) -> bool {
        !self.unevaluated.is_empty()
    }
}

/// The normalized explanation rendering compared for semantic drift.
///
/// Normalization is total and locale-free: LF line endings, two spaces of
/// indentation per proof depth, statuses spelled as the contract spells them,
/// and no trailing whitespace. Nothing time- or host-dependent can enter it,
/// which is what lets a snapshot be committed and byte-compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationSnapshot(String);

impl ExplanationSnapshot {
    /// Renders the normalized explanation for one outcome.
    #[must_use]
    pub fn render(result: &EngineResult, proof_tree: &ProofNode) -> Self {
        let mut rendered = String::new();
        rendered.push_str("result ");
        rendered.push_str(result.status.as_str());
        rendered.push('\n');
        for (key, value) in &result.values {
            rendered.push_str("value ");
            rendered.push_str(key);
            rendered.push('=');
            rendered.push_str(&format!("{}/{}", value.coefficient(), value.scale()));
            rendered.push('\n');
        }
        for rule in &result.unevaluated {
            rendered.push_str("unevaluated ");
            rendered.push_str(rule.as_str());
            rendered.push('\n');
        }
        rendered.push_str("proof\n");
        proof_tree.render_into(1, &mut rendered);
        Self(rendered)
    }

    /// Returns the snapshot text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExplanationSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The whole output half of the engine signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOutcome {
    /// The engine's result.
    pub result: EngineResult,
    /// The proof tree that produced it.
    pub proof_tree: ProofNode,
    /// The normalized explanation rendering of both.
    pub explanation_snapshot: ExplanationSnapshot,
}

impl EngineOutcome {
    /// Assembles an outcome, rendering the explanation from the other two
    /// halves so a caller cannot supply an explanation that disagrees.
    pub fn new(
        result: EngineResult,
        proof_tree: ProofNode,
        inputs: &FrozenInputs,
    ) -> Result<Self, EngineError> {
        proof_tree.validate(inputs)?;
        if result.status == ProofStatus::Satisfied
            && proof_tree
                .walk()
                .iter()
                .any(|node| node.status == ProofStatus::Conflict)
        {
            return Err(EngineError::SatisfiedOverConflict);
        }
        let explanation_snapshot = ExplanationSnapshot::render(&result, &proof_tree);
        Ok(Self {
            result,
            proof_tree,
            explanation_snapshot,
        })
    }

    /// The total canonical encoding of the whole output.
    ///
    /// This is what "byte-equal results" means: two evaluations agree when
    /// these bytes agree, not when neither returned an error.
    #[must_use]
    pub fn canonical_bytes(
        &self,
        engine_id: &str,
        rule_set_hash: RuleSetHash,
        engine_version: EngineVersion,
        inputs: &FrozenInputs,
    ) -> Vec<u8> {
        let mut rendered = String::new();
        rendered.push_str("engine ");
        rendered.push_str(engine_id);
        rendered.push('\n');
        rendered.push_str("engine_version ");
        rendered.push_str(&engine_version.to_string());
        rendered.push('\n');
        rendered.push_str("rule_set_hash ");
        rendered.push_str(&rule_set_hash.to_string());
        rendered.push('\n');
        rendered.push_str("frozen_inputs ");
        rendered.push_str(&inputs.digest().to_string());
        rendered.push('\n');
        rendered.push_str(self.explanation_snapshot.as_str());
        rendered.into_bytes()
    }
}

/// A deterministic engine: a pure function over frozen inputs.
///
/// `&self` and no interior mutability is the whole ambient-state prohibition at
/// type level. The clock, RNG, network, and model prohibitions cannot be
/// expressed in the type system and are enforced by the scan named in
/// `docs/contracts/engine-harness.md`.
pub trait DeterministicEngine {
    /// The registry identifier this engine's outputs are keyed by.
    fn engine_id(&self) -> &'static str;

    /// The engine's own version.
    fn engine_version(&self) -> EngineVersion;

    /// Evaluates the frozen inputs under one published rule set.
    fn evaluate(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
        engine_version: EngineVersion,
    ) -> Result<EngineOutcome, EngineError>;
}

/// The harness artifacts discovered on disk for one engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EngineHarnessArtifacts {
    /// Artifact classes present and non-empty.
    pub classes: BTreeSet<ArtifactClass>,
    /// Adverse paths with an executable fixture set.
    pub adverse: BTreeSet<AdversePath>,
    /// Whether anything at all exists under the engine's harness directory.
    pub directory_exists: bool,
    /// Workspace source files outside the registry that name the engine id.
    pub implementation_sites: Vec<String>,
}

/// A harness obligation a registered engine does not meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessViolation {
    /// An implemented engine is missing one of the four artifact classes.
    MissingArtifactClass {
        /// The engine that is missing it.
        engine: EngineName,
        /// The class that is missing.
        class: ArtifactClass,
    },
    /// A high-impact engine is missing an adverse fixture set.
    MissingAdversePath {
        /// The engine that is missing it.
        engine: EngineName,
        /// The adverse path that is missing.
        path: AdversePath,
    },
    /// A planned engine has acquired harness artifacts without being flipped.
    PlannedEngineHasArtifacts {
        /// The engine that acquired them.
        engine: EngineName,
    },
    /// A planned engine is named by workspace source, so it is implemented.
    PlannedEngineHasImplementation {
        /// The engine that is named.
        engine: EngineName,
        /// The first source file naming it.
        site: String,
    },
}

impl fmt::Display for HarnessViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingArtifactClass { engine, class } => write!(
                formatter,
                "{} is IMPLEMENTED and ships no {}",
                engine.as_str(),
                class.as_str()
            ),
            Self::MissingAdversePath { engine, path } => write!(
                formatter,
                "{} is high impact and has no executable {} fixture",
                engine.as_str(),
                path.as_str()
            ),
            Self::PlannedEngineHasArtifacts { engine } => write!(
                formatter,
                "{} is PLANNED and has harness artifacts; flip it to IMPLEMENTED",
                engine.as_str()
            ),
            Self::PlannedEngineHasImplementation { engine, site } => write!(
                formatter,
                "{} is PLANNED and is named by {}; flip it to IMPLEMENTED",
                engine.as_str(),
                site
            ),
        }
    }
}

/// Audits one registry against the harness artifacts discovered for it.
///
/// Pure: the caller collects the inventory, so the same audit runs against the
/// real tree and against an injected violation. An empty result is the only
/// passing outcome.
#[must_use]
pub fn audit_engine_harness(
    registry: &[EngineDescriptor],
    discovered: &BTreeMap<EngineName, EngineHarnessArtifacts>,
) -> Vec<HarnessViolation> {
    let mut violations = Vec::new();
    let empty = EngineHarnessArtifacts::default();
    for descriptor in registry {
        let artifacts = discovered.get(&descriptor.name).unwrap_or(&empty);
        match descriptor.lifecycle {
            EngineLifecycle::Planned => {
                if artifacts.directory_exists || !artifacts.classes.is_empty() {
                    violations.push(HarnessViolation::PlannedEngineHasArtifacts {
                        engine: descriptor.name,
                    });
                }
                if let Some(site) = artifacts.implementation_sites.first() {
                    violations.push(HarnessViolation::PlannedEngineHasImplementation {
                        engine: descriptor.name,
                        site: site.clone(),
                    });
                }
            }
            EngineLifecycle::Implemented => {
                for class in ArtifactClass::ALL {
                    if !artifacts.classes.contains(&class) {
                        violations.push(HarnessViolation::MissingArtifactClass {
                            engine: descriptor.name,
                            class,
                        });
                    }
                }
                if descriptor.high_impact_path.is_some() {
                    for path in AdversePath::ALL {
                        if !artifacts.adverse.contains(&path) {
                            violations.push(HarnessViolation::MissingAdversePath {
                                engine: descriptor.name,
                                path,
                            });
                        }
                    }
                }
            }
        }
    }
    violations
}

/// Failures an engine or the harness raises instead of panicking.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// An identifier was empty, too long, or used a character the canonical
    /// encoding reserves.
    #[error("invalid {kind}: {value}")]
    InvalidIdentifier {
        /// Which identifier kind was rejected.
        kind: &'static str,
        /// The rejected text.
        value: String,
    },
    /// The canonical frozen-input encoding did not parse.
    #[error("malformed frozen inputs: {0}")]
    MalformedInput(&'static str),
    /// The same input key was supplied twice.
    #[error("duplicate frozen input key: {0}")]
    DuplicateInputKey(String),
    /// A proof tree reused a node identifier.
    #[error("duplicate proof node id: {0}")]
    DuplicateNodeId(String),
    /// A proof node read an input the frozen set does not declare.
    #[error("proof node reads undeclared input: {0}")]
    UndeclaredInput(String),
    /// A proof node's repeated field was unordered or contained a duplicate.
    #[error("proof node {node_id} has an unordered or duplicated {field}")]
    UnorderedProofField {
        /// The offending node.
        node_id: String,
        /// The field that was unordered.
        field: &'static str,
    },
    /// A result claimed `SATISFIED` over a proof tree containing a conflict.
    #[error("a SATISFIED result cannot rest on a proof tree containing a CONFLICT")]
    SatisfiedOverConflict,
    /// Version zero is not an engine version.
    #[error("engine version must be greater than zero")]
    InvalidEngineVersion,
    /// A domain value the engine depends on was invalid.
    #[error(transparent)]
    Domain(#[from] crate::DomainError),
}
