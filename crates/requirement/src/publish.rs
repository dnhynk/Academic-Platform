//! Section 11.1's `DegreeRequirementSet`, published immutably and replaced only
//! by supersession.
//!
//! Section 11.4: *변경은 기존 RuleSet을 수정하지 않고 새 버전을 publish한다*
//! and *과거 audit은 당시 입력과 rule hash로 재현한다*.
//!
//! # What immutable is executed as
//!
//! [`RuleSet`] has private fields, no `&mut self` method, and no setter. That
//! makes one value immutable. What makes the *history* immutable is
//! [`RuleSetLedger`]: `publish` refuses a version the ledger already holds, and
//! the only way to change what a rule says is to publish a later version that
//! names the earlier one in `supersedes`. Both versions stay in the ledger and
//! both stay addressable by their own hash, which is what lets a historical
//! audit be replayed against the rules that were live when it ran.
//!
//! # The official source binding
//!
//! [`RuleSetDraft::from_official_source`] takes an
//! `academic_ingestion::PublishedRules`, whose fields are private and whose
//! only producer is `P2-U6`'s stage nine -- which takes a `PublishableRules`
//! that `Reconciled::publishable` returns `None` for on an undated document. A
//! rule set founded on an `UNSCOPED_OFFICIAL_SOURCE` is therefore not a value
//! that exists, for the same reason `P2-U1`'s curriculum version is not.
//!
//! # The release gate runs its fixtures
//!
//! Section 11.4: *새 rule은 공식 예시와 synthetic transcript fixture로 회귀
//! 검증한다*. [`RuleSetDraft::include`] takes the two fixture classes as **two
//! parameters**, so a release with only one is not a call that can be written,
//! and it **evaluates** every case in both against the rule being included,
//! requiring the observed status to be the one the case declares. A fixture
//! that merely exists proves nothing, which is the lesson
//! `docs/contracts/engine-harness.md` records about adverse fixture
//! directories; here the fixture has to agree with the rule or the publication
//! fails.

use std::collections::BTreeMap;

use academic_domain::{
    ContentDigest, CurriculumVersionId, RequirementSetId,
    engines::{ProofStatus, RuleId as SourceRuleId},
};
use academic_ingestion::{
    dating::EffectiveDate,
    identifier::ConnectorId,
    manifest::{ParserVersion, RetrievalInstant},
    publish::PublishedRules,
};

use crate::{
    candidate::ReviewedRule,
    dsl::{RuleBody, RuleId},
    error::RequirementError,
    evaluate::{RuleOutcome, evaluate},
    facts::AcademicFacts,
};

/// A published rule set's version number inside its own identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleSetVersion(u32);

impl RuleSetVersion {
    /// The first version of a rule set.
    pub const FIRST: Self = Self(1);

    /// Constructs a version.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the version number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The version that follows this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl core::fmt::Display for RuleSetVersion {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "v{}", self.0)
    }
}

/// Where the rules came from, bound at publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialSourceBinding {
    connector: ConnectorId,
    effective: EffectiveDate,
    retrieved_at: RetrievalInstant,
    parser_version: ParserVersion,
}

impl OfficialSourceBinding {
    /// Which connector's document.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// When the source's rules start to apply.
    #[must_use]
    pub const fn effective(&self) -> EffectiveDate {
        self.effective
    }

    /// When the source was retrieved.
    #[must_use]
    pub const fn retrieved_at(&self) -> RetrievalInstant {
        self.retrieved_at
    }

    /// Which parser read it.
    #[must_use]
    pub const fn parser_version(&self) -> ParserVersion {
        self.parser_version
    }
}

/// A rule that has passed the review gate and been admitted to a draft.
///
/// Private fields and no public constructor. The one expression in this crate
/// that builds it is inside [`RuleSetDraft::include`], which takes a
/// [`ReviewedRule`] by value -- and [`ReviewedRule`] is itself only produced by
/// [`crate::candidate::ReviewGate::admit`]. There is no path from a
/// [`crate::candidate::RuleCandidate`] to a value of this type.
///
/// It carries no quoted source text. The candidate's `quoted_source` is not
/// forwarded here and there is no field it would fit in, so the audit path has
/// no sentence to interpret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableRule {
    id: RuleId,
    source_rule: SourceRuleId,
    body: RuleBody,
    source_digest: ContentDigest,
}

impl ExecutableRule {
    /// The rule's identifier.
    #[must_use]
    pub const fn id(&self) -> &RuleId {
        &self.id
    }

    /// The identifier the official document this rule was read from gives it.
    ///
    /// Not the same namespace as [`ExecutableRule::id`], which is the
    /// identifier the reviewer chose inside the set. `RuleSetDraft::include`
    /// refuses a rule whose `source_rule` the official source did not publish,
    /// so this is a value bound to the document rather than a second spelling
    /// somebody typed -- which is what `academic-audit`'s conflict gate needs
    /// in order to say whether a conflict case is about this set at all.
    #[must_use]
    pub const fn source_rule(&self) -> &SourceRuleId {
        &self.source_rule
    }

    /// The compiled body.
    #[must_use]
    pub const fn body(&self) -> &RuleBody {
        &self.body
    }

    /// The digest of the official snapshot this rule was read out of.
    #[must_use]
    pub const fn source_digest(&self) -> ContentDigest {
        self.source_digest
    }
}

/// One regression case: a fact set and the verdict the rule must reach on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureCase {
    facts: AcademicFacts,
    expected: ProofStatus,
}

impl FixtureCase {
    /// Declares a case.
    #[must_use]
    pub const fn new(facts: AcademicFacts, expected: ProofStatus) -> Self {
        Self { facts, expected }
    }
}

macro_rules! fixture_class {
    ($name:ident, $doc:literal, $missing:literal) => {
        #[doc = $doc]
        ///
        /// Non-empty by construction: a class with no case would pass a loop
        /// over it and prove nothing, which is the empty-guard shape this
        /// repository has found five times.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            cases: Vec<FixtureCase>,
        }

        impl $name {
            /// Builds the class, refusing an empty one.
            pub fn new(
                cases: impl IntoIterator<Item = FixtureCase>,
                rule: &RuleId,
            ) -> Result<Self, RequirementError> {
                let cases: Vec<FixtureCase> = cases.into_iter().collect();
                if cases.is_empty() {
                    return Err(RequirementError::ReleaseFixturesMissing {
                        rule: rule.as_str().to_owned(),
                        missing: $missing,
                    });
                }
                Ok(Self { cases })
            }

            /// The declared cases.
            #[must_use]
            pub fn cases(&self) -> &[FixtureCase] {
                &self.cases
            }
        }
    };
}

fixture_class!(
    OfficialExampleFixtures,
    "The worked examples the official source itself states.",
    "no official example fixture"
);
fixture_class!(
    SyntheticTranscriptFixtures,
    "Synthetic transcripts written to exercise the rule's boundaries.",
    "no synthetic transcript fixture"
);

/// A rule set under construction.
///
/// Nothing here is published until [`RuleSetDraft::publish`] is called, and a
/// draft is consumed by it, so a draft cannot be published twice.
#[derive(Debug, Clone)]
pub struct RuleSetDraft {
    set_id: RequirementSetId,
    curriculum_version: CurriculumVersionId,
    version: RuleSetVersion,
    supersedes: Option<RuleSetVersion>,
    source: OfficialSourceBinding,
    /// The rule identifiers the official source published, kept for the whole
    /// life of the draft so `include` can bind each admitted rule to one. It
    /// does not travel onto [`RuleSet`]: what the published set needs is the
    /// binding on each rule, not the document's index.
    source_rules: Vec<SourceRuleId>,
    rules: Vec<ExecutableRule>,
}

impl RuleSetDraft {
    /// Starts a draft bound to an official source `P2-U6` published.
    #[must_use]
    pub fn from_official_source(
        published: &PublishedRules,
        set_id: RequirementSetId,
        curriculum_version: CurriculumVersionId,
        version: RuleSetVersion,
        supersedes: Option<RuleSetVersion>,
    ) -> Self {
        Self {
            set_id,
            curriculum_version,
            version,
            supersedes,
            source: OfficialSourceBinding {
                connector: published.connector().clone(),
                effective: published.effective(),
                retrieved_at: published.retrieved_at(),
                parser_version: published.parser_version(),
            },
            source_rules: published.rules().to_vec(),
            rules: Vec::new(),
        }
    }

    /// Admits one reviewed rule, after running both fixture classes against it.
    ///
    /// The two classes are two parameters, so there is no call that supplies
    /// one of them. Each case is evaluated against the rule as it will execute
    /// -- through the same [`evaluate`] the audit uses, over the rules admitted
    /// so far -- and the observed status must be the one the case declares. A
    /// rule whose fixtures do not agree with it does not enter the set.
    pub fn include(
        mut self,
        reviewed: ReviewedRule,
        official: &OfficialExampleFixtures,
        synthetic: &SyntheticTranscriptFixtures,
    ) -> Result<Self, RequirementError> {
        if self
            .rules
            .iter()
            .any(|existing| existing.id() == reviewed.id())
        {
            return Err(RequirementError::DuplicateRule {
                rule: reviewed.id().as_str().to_owned(),
            });
        }
        if !self
            .source_rules
            .iter()
            .any(|published| published == reviewed.source_rule())
        {
            return Err(RequirementError::SourceRuleNotPublished {
                rule: reviewed.id().as_str().to_owned(),
                source_rule: reviewed.source_rule().as_str().to_owned(),
            });
        }
        let rule = ExecutableRule {
            id: reviewed.id().clone(),
            source_rule: reviewed.source_rule().clone(),
            body: reviewed.body().clone(),
            source_digest: reviewed.source_digest(),
        };
        rule.body.compile(&rule.id)?;

        // Evaluate against the set as it stands, which is what the rule will
        // see once published: an `ALL_OF` with a `COURSE_OR_EQUIVALENT` operand
        // resolves through the `EQUIVALENCY` rules already admitted.
        //
        // The rule under test is evaluated by body rather than through the
        // staged set, and is pushed only after every case agrees. A rule that
        // was pushed first and popped on failure would leave the draft's state
        // depending on where the loop stopped.
        let staged = self.as_evaluable();
        for case in official.cases().iter().chain(synthetic.cases()) {
            let outcome = evaluate(&staged, &rule.id, &rule.body, &case.facts)?;
            if outcome.status != case.expected {
                return Err(RequirementError::ReleaseFixturesMissing {
                    rule: rule.id.as_str().to_owned(),
                    missing: "a regression fixture disagrees with the rule",
                });
            }
        }
        self.rules.push(rule);
        Ok(self)
    }

    /// A read-only view of the rules admitted so far, for fixture evaluation.
    fn as_evaluable(&self) -> RuleSet {
        RuleSet {
            set_id: self.set_id,
            curriculum_version: self.curriculum_version,
            version: self.version,
            supersedes: self.supersedes,
            source: self.source.clone(),
            rules: self.rules.clone(),
        }
    }

    /// Freezes the draft into an immutable rule set.
    ///
    /// # A set with no rule is refused
    ///
    /// Section 11.4 makes `DETERMINATE` conditional on *rule coverage 100%*,
    /// and coverage over no rule is the vacuous witness
    /// `academic_audit::CoverageWitness::establish` refuses eleven lines above
    /// its own conflict counterpart. A published set with no rule was
    /// accepted here, selected by `academic-audit`, and audited: the tree had
    /// no leaf, so the coverage gate refused, and the audit had **no
    /// outstanding check to name** -- it reported
    /// `SOURCE_FRESHNESS_POLICY_ABSENT` to a user who had recorded the
    /// freshness criterion, telling them to record what they had already
    /// recorded. Deleting the two guards that stop the same set answering
    /// 졸업 가능 outright left the whole `academic-audit` suite green.
    ///
    /// The reason code was the symptom. This is the state: a requirement set
    /// that requires nothing is not a lenient set, it is not a set.
    pub fn publish(self) -> Result<RuleSet, RequirementError> {
        if self.rules.is_empty() {
            return Err(RequirementError::EmptyRuleSet {
                set: self.set_id.to_string(),
                version: self.version.to_string(),
            });
        }
        Ok(RuleSet {
            set_id: self.set_id,
            curriculum_version: self.curriculum_version,
            version: self.version,
            supersedes: self.supersedes,
            source: self.source,
            rules: self.rules,
        })
    }
}

/// A published rule set. Immutable.
///
/// No `&mut self` method, no setter, and no public field. A change is a new
/// version through [`RuleSetLedger::publish`], never an edit to this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    set_id: RequirementSetId,
    curriculum_version: CurriculumVersionId,
    version: RuleSetVersion,
    supersedes: Option<RuleSetVersion>,
    source: OfficialSourceBinding,
    rules: Vec<ExecutableRule>,
}

impl RuleSet {
    /// The durable identity of the requirement set.
    #[must_use]
    pub const fn set_id(&self) -> RequirementSetId {
        self.set_id
    }

    /// The curriculum version the set hangs from.
    #[must_use]
    pub const fn curriculum_version(&self) -> CurriculumVersionId {
        self.curriculum_version
    }

    /// This set's version.
    #[must_use]
    pub const fn version(&self) -> RuleSetVersion {
        self.version
    }

    /// The version this one replaces, when it replaces one.
    #[must_use]
    pub const fn supersedes(&self) -> Option<RuleSetVersion> {
        self.supersedes
    }

    /// Where the rules came from.
    #[must_use]
    pub const fn source(&self) -> &OfficialSourceBinding {
        &self.source
    }

    /// Every rule, as identifier and body pairs.
    pub fn rules(&self) -> impl Iterator<Item = (&RuleId, &RuleBody)> {
        self.rules.iter().map(|rule| (rule.id(), rule.body()))
    }

    /// Every rule whole, including the document identifier it is bound to.
    ///
    /// [`RuleSet::rules`] is the pair an evaluation needs; this is what a
    /// caller needs in order to ask whether something said about the official
    /// document is about a rule in this set.
    pub fn executable_rules(&self) -> impl Iterator<Item = &ExecutableRule> {
        self.rules.iter()
    }

    /// One rule by identifier.
    #[must_use]
    pub fn rule(&self, id: &RuleId) -> Option<&ExecutableRule> {
        self.rules.iter().find(|rule| rule.id() == id)
    }

    /// Evaluates one rule of this set against the frozen facts.
    ///
    /// The only entry point an audit uses. It takes an identifier rather than a
    /// body, so a body that is not in this set cannot be evaluated as though it
    /// were -- which is what binds an audit to a published, reviewed rule.
    pub fn evaluate(
        &self,
        id: &RuleId,
        facts: &AcademicFacts,
    ) -> Result<RuleOutcome, RequirementError> {
        let rule = self
            .rule(id)
            .ok_or_else(|| RequirementError::UndeclaredFact {
                rule: id.as_str().to_owned(),
                fact: "the rule is not published in this set".to_owned(),
            })?;
        evaluate(self, rule.id(), rule.body(), facts)
    }

    /// The canonical text this set's hash is taken over.
    ///
    /// A total function of the set's content, in a fixed order, with no clock
    /// and no host in it, so two independently constructed sets with the same
    /// content have the same hash.
    ///
    /// # It is every field, not the ones that look like identity
    ///
    /// This rendering used to carry the set's identifiers, each rule's
    /// identifier, rule type and source digest, and the source's effective
    /// date -- and nothing else. Two things a verdict depends on were outside
    /// it, and both were measured:
    ///
    /// * **The rule bodies.** Two sets differing only in `CREDIT_MINIMUM`'s
    ///   threshold hashed the same and answered 졸업 불가 and 졸업 가능. The
    ///   stricter audit's recorded hash replayed against the laxer bodies and
    ///   was accepted.
    /// * **The rest of the source binding.** `retrieved_at` is what
    ///   `academic-audit`'s freshness gate reads. Two sets differing only in it
    ///   hashed the same and answered `DETERMINATE POSSIBLE` and
    ///   `INDETERMINATE [SOURCE_NOT_FRESH]`, and the fresh audit's recorded
    ///   hash replayed against the stale source and was accepted.
    ///
    /// So the rule is not "the fields that decide a verdict today" -- that is a
    /// judgement a later reader has to make again, and it was made wrongly
    /// twice here. The rule is **every field of [`RuleSet`]**: its six own
    /// fields, all four of [`OfficialSourceBinding`]'s, and all three of
    /// [`ExecutableRule`]'s including the body, which
    /// [`RuleBody::canonical_text`] renders totally.
    /// `every_rule_set_field_moves_the_hash` moves each of them in turn, and
    /// `the_canonical_text_covers_every_field_of_the_rule_set` compares the
    /// field sets themselves so a field added to any of the three types has to
    /// arrive here.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str("requirement_set ");
        rendered.push_str(&self.set_id.to_string());
        rendered.push('\n');
        rendered.push_str("curriculum_version ");
        rendered.push_str(&self.curriculum_version.to_string());
        rendered.push('\n');
        rendered.push_str("version ");
        rendered.push_str(&self.version.to_string());
        rendered.push('\n');
        rendered.push_str("supersedes ");
        match self.supersedes {
            Some(version) => rendered.push_str(&version.to_string()),
            None => rendered.push_str("none"),
        }
        rendered.push('\n');
        let OfficialSourceBinding {
            connector,
            effective,
            retrieved_at,
            parser_version,
        } = &self.source;
        rendered.push_str("connector ");
        rendered.push_str(connector.as_str());
        rendered.push('\n');
        rendered.push_str("effective ");
        rendered.push_str(&effective.to_string());
        rendered.push('\n');
        rendered.push_str("retrieved_at ");
        rendered.push_str(&retrieved_at.seconds().to_string());
        rendered.push('\n');
        rendered.push_str("parser_version ");
        rendered.push_str(&parser_version.get().to_string());
        rendered.push('\n');
        for rule in &self.rules {
            let ExecutableRule {
                id,
                source_rule,
                body,
                source_digest,
            } = rule;
            rendered.push_str("rule ");
            rendered.push_str(id.as_str());
            rendered.push(' ');
            rendered.push_str(source_rule.as_str());
            rendered.push(' ');
            rendered.push_str(&source_digest.to_string());
            rendered.push(' ');
            rendered.push_str(&body.canonical_text());
            rendered.push('\n');
        }
        rendered
    }

    /// The hash a historical audit replays against.
    #[must_use]
    pub fn rule_set_hash(&self) -> ContentDigest {
        ContentDigest::sha256(self.canonical_text().as_bytes())
    }
}

/// Every published version of one requirement set, oldest first.
///
/// Append-only. `publish` is the only mutator and it appends; there is no
/// `remove`, no `replace` and no `&mut` accessor into a stored version.
#[derive(Debug, Clone, Default)]
pub struct RuleSetLedger {
    versions: Vec<RuleSet>,
}

impl RuleSetLedger {
    /// An empty ledger. The only `Default` in this crate, and it is emptiness
    /// rather than a value.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            versions: Vec::new(),
        }
    }

    /// Publishes a version.
    ///
    /// Refuses a version number the ledger already holds, and refuses a
    /// supersession that names anything but the current head. Both refusals are
    /// what make "a change publishes a new version" the only available move:
    /// republishing an existing number is the edit that section 11.4 forbids,
    /// and superseding a version that is not current would fork the history a
    /// replay walks.
    pub fn publish(&mut self, set: RuleSet) -> Result<(), RequirementError> {
        if self
            .versions
            .iter()
            .any(|existing| existing.version() == set.version())
        {
            return Err(RequirementError::VersionAlreadyPublished {
                version: set.version().to_string(),
            });
        }
        let head = self.versions.last().map(RuleSet::version);
        if set.supersedes() != head {
            return Err(RequirementError::SupersedesTheWrongVersion {
                claimed: set
                    .supersedes()
                    .map_or_else(|| "none".to_owned(), |version| version.to_string()),
                actual: head.map_or_else(|| "none".to_owned(), |version| version.to_string()),
            });
        }
        self.versions.push(set);
        Ok(())
    }

    /// The version currently in force, when one has been published.
    #[must_use]
    pub fn current(&self) -> Option<&RuleSet> {
        self.versions.last()
    }

    /// One version by number. A superseded version stays addressable, which is
    /// what a historical replay needs.
    #[must_use]
    pub fn version(&self, version: RuleSetVersion) -> Option<&RuleSet> {
        self.versions
            .iter()
            .find(|existing| existing.version() == version)
    }

    /// One version by its rule-set hash.
    #[must_use]
    pub fn by_hash(&self, hash: ContentDigest) -> Option<&RuleSet> {
        self.versions
            .iter()
            .find(|existing| existing.rule_set_hash() == hash)
    }

    /// Every published version, oldest first.
    #[must_use]
    pub fn versions(&self) -> &[RuleSet] {
        &self.versions
    }

    /// Which rules changed between two versions, by identifier.
    ///
    /// Section 11.4's *영향받는 rules를 표시한다*, over the published sets
    /// rather than over the source documents: a rule whose body differs, one
    /// only the later version has, and one only the earlier version had.
    #[must_use]
    pub fn changed_rules(&self, earlier: RuleSetVersion, later: RuleSetVersion) -> Vec<RuleId> {
        let (Some(earlier), Some(later)) = (self.version(earlier), self.version(later)) else {
            return Vec::new();
        };
        let earlier_rules: BTreeMap<&RuleId, &RuleBody> = earlier.rules().collect();
        let later_rules: BTreeMap<&RuleId, &RuleBody> = later.rules().collect();
        let mut changed: Vec<RuleId> = Vec::new();
        for (id, body) in &later_rules {
            if earlier_rules.get(id).is_none_or(|earlier| earlier != body) {
                changed.push((*id).clone());
            }
        }
        for id in earlier_rules.keys() {
            if !later_rules.contains_key(id) {
                changed.push((*id).clone());
            }
        }
        changed.sort();
        changed.dedup();
        changed
    }
}
