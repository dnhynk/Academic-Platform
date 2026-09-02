//! The two §28 engines this task implements.
//!
//! | registry name | engine id | what it decides |
//! |---|---|---|
//! | `GPA` | `engine.gpa` | the grade-point average for one scope, plus the inclusion proof |
//! | `CREDIT_ACCOUNTING` | `engine.credit.accounting` | per-category credit totals, and every credit counted twice |
//!
//! Both are pure functions of `(frozen_inputs, rule_set_hash, engine_version)`.
//! Neither reads a clock, an RNG, a socket, or a model, and neither holds
//! mutable state: the rule book is `&self` and the attempt set arrives entirely
//! through the frozen inputs. The `rule_set_hash` argument is checked against
//! the book's own digest before anything is computed, so an average can never
//! be attributed to a rule set that did not produce it.
//!
//! `GPA` is one of the four high-impact paths, which is why its harness carries
//! `adverse/unknown`, `adverse/conflict`, and `adverse/partial_failure` fixture
//! sets. Each of those is a state the shipped rules actually reach:
//!
//! - **unknown** — a repeat group whose recognition rule no confirmed source
//!   states, or an external attempt whose term no dated row reaches.
//! - **conflict** — two settled attempts at one course in one term, neither
//!   marked as a repeat of the other. The record disagrees with itself about
//!   what happened, and no average over it is meaningful.
//! - **partial failure** — a scope naming a term or a programme the attempt set
//!   has nothing in. The average rule is left unevaluated rather than answered
//!   with a zero.

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::{
    Decimal,
    engines::{
        DeterministicEngine, EngineError, EngineOutcome, EngineResult, EngineVersion, FrozenInputs,
        InputKey, NodeId, ProofNode, ProofStatus, RuleId, RuleSetHash,
    },
};

use crate::{
    RecordError,
    classify::{ProgramId, RequirementCategory},
    decimal,
    facts::{AttemptFacts, GpaScope, decode},
    policy::RuleBook,
    views::{AverageContribution, CreditContribution, GpaValue, RecordViews},
};

/// The registry identifier of the GPA engine.
pub const GPA_ENGINE_ID: &str = "engine.gpa";
/// The registry identifier of the credit-accounting engine.
pub const CREDIT_ENGINE_ID: &str = "engine.credit.accounting";

/// The rule the GPA engine folds every attempt into.
pub const RULE_GPA_AVERAGE: &str = "rule.gpa.average";
/// The rule that decides one attempt's disposition.
pub const RULE_GPA_DISPOSITION: &str = "rule.gpa.attempt.disposition";
/// The rule that refuses two settled attempts at one course in one term.
pub const RULE_GPA_SINGLE_RECORD: &str = "rule.gpa.single.record.per.course.term";
/// The rule the credit engine folds every category total into.
pub const RULE_CREDIT_TOTALS: &str = "rule.credit.category.totals";
/// The rule that traces a credit recognized under more than one programme.
pub const RULE_CREDIT_DOUBLE_COUNT: &str = "rule.credit.double.recognition.traced";

/// The `GPA` engine.
#[derive(Debug, Clone)]
pub struct GpaEngine {
    rules: RuleBook,
    version: EngineVersion,
}

impl GpaEngine {
    /// Binds an engine to one published rule book.
    #[must_use]
    pub const fn new(rules: RuleBook, version: EngineVersion) -> Self {
        Self { rules, version }
    }

    /// Returns the rule book this engine evaluates under.
    #[must_use]
    pub const fn rules(&self) -> &RuleBook {
        &self.rules
    }

    /// Returns the hash every evaluation must present.
    #[must_use]
    pub fn rule_set_hash(&self) -> RuleSetHash {
        RuleSetHash::new(self.rules.digest())
    }

    /// Evaluates, returning this crate's error type rather than the domain's.
    pub fn evaluate_record(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
    ) -> Result<EngineOutcome, RecordError> {
        if rule_set_hash != self.rule_set_hash() {
            return Err(RecordError::RuleSetHashMismatch);
        }
        let (facts, scope) = decode(inputs)?;
        let views = RecordViews::from_facts(&facts, &self.rules)?;

        let selected: Vec<&AttemptFacts> = match &scope {
            GpaScope::Cumulative => facts.iter().collect(),
            GpaScope::Term(term) => facts.iter().filter(|entry| entry.term == *term).collect(),
            GpaScope::Major(program) => facts
                .iter()
                .filter(|entry| entry.is_major_for(program))
                .collect(),
        };
        let selected_ids: BTreeSet<_> = selected.iter().map(|entry| entry.id).collect();

        let conflicts = duplicate_records(&facts);
        let mut children = Vec::new();
        let mut worst = ProofStatus::Satisfied;

        // One node per selected attempt, naming the exact inputs it read.
        for (index, attempt) in selected.iter().enumerate() {
            let disposition = views
                .dispositions()
                .iter()
                .find(|disposition| disposition.attempt_id() == attempt.id)
                .ok_or(RecordError::DispositionMissing)?;
            let status = if conflicts.contains(&attempt.id) {
                ProofStatus::Conflict
            } else if matches!(disposition.average(), AverageContribution::Unknown) {
                ProofStatus::Unknown
            } else {
                ProofStatus::Satisfied
            };
            worst = worsen(worst, status);
            let rule = if conflicts.contains(&attempt.id) {
                RULE_GPA_SINGLE_RECORD
            } else {
                RULE_GPA_DISPOSITION
            };
            children.push(ProofNode {
                node_id: NodeId::new(&format!("n.attempt.{index:03}"))?,
                rule_id: RuleId::new(rule)?,
                status,
                inputs: attempt_input_keys(inputs, attempt)?,
                source_locators: Vec::new(),
                children: Vec::new(),
            });
        }

        let mut values: BTreeMap<String, Decimal> = BTreeMap::new();
        let mut unevaluated = Vec::new();

        let average = match &scope {
            GpaScope::Cumulative => views.cumulative_gpa()?,
            GpaScope::Term(term) => views.term_gpa(*term)?,
            GpaScope::Major(program) => views.major_gpa(program)?,
        };

        // A scope the attempt set has nothing in leaves the average rule
        // unevaluated. That is a partial failure, not an average of zero.
        if selected.is_empty() {
            unevaluated.push(RuleId::new(RULE_GPA_AVERAGE)?);
            worst = worsen(worst, ProofStatus::Needs);
        } else {
            match &average {
                GpaValue::Known(_) => {}
                GpaValue::NoGradedAttempts => worst = worsen(worst, ProofStatus::Needs),
                GpaValue::Unknown(_) => worst = worsen(worst, ProofStatus::Unknown),
            }
        }

        // A value is published only when it is fully determined.
        //
        // The temptation here is to publish the arithmetic that ran and let the
        // status carry the caveat. That is the quiet failure this engine exists
        // to avoid: a reader who sees `gpa=2.65` beside a `CONFLICT` has been
        // handed an average over a record that disagrees with itself, and the
        // number is what survives into a screenshot. So a conflicted evaluation
        // publishes no derived value at all, and an unknown one publishes only
        // the totals no unknown attempt touched.
        let denominator = views.gpa_denominator()?;
        let earned = views.earned_credits()?;
        let conflicted = worst == ProofStatus::Conflict;
        if !conflicted {
            if let GpaValue::Known(value) = &average {
                values.insert("gpa".to_owned(), *value);
                values.insert("gpa.quality.points".to_owned(), views.quality_points()?);
            }
            // The two totals, always both, always separately. Reporting one
            // without the other is the collapse `credits_vs_denominator` exists
            // to prevent, and an engine result is a place it could happen
            // unobserved.
            if let Some(total) = denominator.complete() {
                values.insert("gpa.denominator.credits".to_owned(), total);
            }
            if let Some(total) = earned.complete() {
                values.insert("credits.earned".to_owned(), total);
            }
        }
        values.insert(
            "attempts.in.scope".to_owned(),
            decimal::integer(i128::try_from(selected_ids.len()).unwrap_or(i128::MAX))?,
        );
        values.insert(
            "attempts.pending.disposition".to_owned(),
            decimal::integer(i128::try_from(denominator.unknown().len()).unwrap_or(i128::MAX))?,
        );

        let root = ProofNode {
            node_id: NodeId::new("n.gpa")?,
            rule_id: RuleId::new(RULE_GPA_AVERAGE)?,
            status: worst,
            inputs: scope_input_keys(inputs)?,
            source_locators: Vec::new(),
            children,
        };
        let result = EngineResult {
            status: worst,
            values,
            unevaluated,
        };
        Ok(EngineOutcome::new(result, root, inputs)?)
    }
}

impl DeterministicEngine for GpaEngine {
    fn engine_id(&self) -> &'static str {
        GPA_ENGINE_ID
    }

    fn engine_version(&self) -> EngineVersion {
        self.version
    }

    fn evaluate(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
        _engine_version: EngineVersion,
    ) -> Result<EngineOutcome, EngineError> {
        self.evaluate_record(inputs, rule_set_hash)
            .map_err(RecordError::into_engine_error)
    }
}

/// The `CREDIT_ACCOUNTING` engine.
///
/// Its §28 invariant is "한 학점의 중복 인정 근거 추적" — trace the basis on
/// which one credit is recognized twice. It therefore does **not** resolve
/// double counting: `GATE-38-015` (multi-major double-counting rules) is open,
/// and choosing a rule here would close it by invention. What it does is name
/// every credit that reached two programmes' totals, with both categories, so
/// the double recognition is visible rather than silently summed.
#[derive(Debug, Clone)]
pub struct CreditAccountingEngine {
    rules: RuleBook,
    version: EngineVersion,
}

impl CreditAccountingEngine {
    /// Binds an engine to one published rule book.
    #[must_use]
    pub const fn new(rules: RuleBook, version: EngineVersion) -> Self {
        Self { rules, version }
    }

    /// Returns the hash every evaluation must present.
    #[must_use]
    pub fn rule_set_hash(&self) -> RuleSetHash {
        RuleSetHash::new(self.rules.digest())
    }

    /// Evaluates, returning this crate's error type rather than the domain's.
    pub fn evaluate_record(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
    ) -> Result<EngineOutcome, RecordError> {
        if rule_set_hash != self.rule_set_hash() {
            return Err(RecordError::RuleSetHashMismatch);
        }
        let (facts, _) = decode(inputs)?;
        let views = RecordViews::from_facts(&facts, &self.rules)?;

        let mut totals: BTreeMap<(ProgramId, RequirementCategory), Decimal> = BTreeMap::new();
        let mut double_counted: Vec<&AttemptFacts> = Vec::new();
        let mut unknown = Vec::new();

        for attempt in &facts {
            let disposition = views
                .dispositions()
                .iter()
                .find(|disposition| disposition.attempt_id() == attempt.id)
                .ok_or(RecordError::DispositionMissing)?;
            let credits = match disposition.credit() {
                CreditContribution::Earned(credits) => credits,
                CreditContribution::NotEarned => continue,
                CreditContribution::Unknown => {
                    unknown.push(attempt.id);
                    continue;
                }
            };
            if attempt.categories.len() > 1 {
                double_counted.push(attempt);
            }
            for (program, category) in &attempt.categories {
                let slot = totals
                    .entry((program.clone(), *category))
                    .or_insert(decimal::zero()?);
                *slot = decimal::add(*slot, credits)?;
            }
        }

        let mut children = Vec::new();
        for (index, attempt) in double_counted.iter().enumerate() {
            children.push(ProofNode {
                node_id: NodeId::new(&format!("n.double.{index:03}"))?,
                rule_id: RuleId::new(RULE_CREDIT_DOUBLE_COUNT)?,
                // Traced, not resolved: `GATE-38-015` decides whether a credit
                // may count twice, and it is open.
                status: ProofStatus::Unknown,
                inputs: attempt_input_keys(inputs, attempt)?,
                source_locators: Vec::new(),
                children: Vec::new(),
            });
        }

        let mut values: BTreeMap<String, Decimal> = BTreeMap::new();
        for ((program, category), total) in &totals {
            values.insert(
                format!("credits.{}.{}", program.as_str(), category.as_str()),
                *total,
            );
        }
        // As in the GPA engine: an earned total is published only when no
        // attempt's contribution to it is unknown. A total that silently
        // omitted an undecided external credit would read as a complete one.
        let earned = views.earned_credits()?;
        if let Some(total) = earned.complete() {
            values.insert("credits.earned".to_owned(), total);
        }
        values.insert(
            "credits.pending.recognition".to_owned(),
            decimal::integer(i128::try_from(earned.unknown().len()).unwrap_or(i128::MAX))?,
        );
        values.insert(
            "credits.double.recognized.attempts".to_owned(),
            decimal::integer(i128::try_from(double_counted.len()).unwrap_or(i128::MAX))?,
        );

        let status = if !unknown.is_empty() || !double_counted.is_empty() {
            ProofStatus::Unknown
        } else {
            ProofStatus::Satisfied
        };
        let root = ProofNode {
            node_id: NodeId::new("n.credit")?,
            rule_id: RuleId::new(RULE_CREDIT_TOTALS)?,
            status,
            inputs: scope_input_keys(inputs)?,
            source_locators: Vec::new(),
            children,
        };
        let result = EngineResult {
            status,
            values,
            unevaluated: Vec::new(),
        };
        Ok(EngineOutcome::new(result, root, inputs)?)
    }
}

impl DeterministicEngine for CreditAccountingEngine {
    fn engine_id(&self) -> &'static str {
        CREDIT_ENGINE_ID
    }

    fn engine_version(&self) -> EngineVersion {
        self.version
    }

    fn evaluate(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
        _engine_version: EngineVersion,
    ) -> Result<EngineOutcome, EngineError> {
        self.evaluate_record(inputs, rule_set_hash)
            .map_err(RecordError::into_engine_error)
    }
}

/// Returns every attempt settled twice for one course in one term.
///
/// Two rows for one slot with neither marked as a repeat of the other is the
/// record disagreeing with itself, which is what `CONFLICT` means in the proof
/// vocabulary: two admitted sources for a fact one rule read.
fn duplicate_records(facts: &[AttemptFacts]) -> BTreeSet<academic_domain::AttemptId> {
    let mut slots: BTreeMap<(&str, crate::term::TermKey), Vec<&AttemptFacts>> = BTreeMap::new();
    for attempt in facts {
        if attempt.status.is_settled()
            && matches!(
                attempt.repeat_status,
                crate::attempt::RepeatStatus::Original
                    | crate::attempt::RepeatStatus::NotApplicable
            )
        {
            slots
                .entry((attempt.course_code.as_str(), attempt.term))
                .or_default()
                .push(attempt);
        }
    }
    slots
        .into_values()
        .filter(|group| group.len() > 1)
        .flatten()
        .map(|attempt| attempt.id)
        .collect()
}

/// The frozen-input keys one attempt's node declares it read.
///
/// Every key is checked against the declared set, sorted, and deduplicated
/// before it reaches the node, because `ProofNode::validate` refuses an
/// undeclared or unordered `inputs` list — and a node that claimed to read a
/// key that is not there would make the proof tree a description rather than a
/// record.
fn attempt_input_keys(
    inputs: &FrozenInputs,
    attempt: &AttemptFacts,
) -> Result<Vec<InputKey>, RecordError> {
    let id = attempt.id.to_string();
    let mut prefix = None;
    for key in inputs.keys() {
        if key.as_str().ends_with(".id")
            && let Some(academic_domain::engines::InputValue::Reference(value)) = inputs.get(key)
            && *value == id
        {
            prefix = key.as_str().strip_suffix(".id").map(str::to_owned);
        }
    }
    let prefix = prefix.ok_or(RecordError::DispositionMissing)?;
    let mut keys: Vec<InputKey> = inputs
        .keys()
        .filter(|key| key.as_str().starts_with(&format!("{prefix}.")))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// The frozen-input keys the root node declares it read.
fn scope_input_keys(inputs: &FrozenInputs) -> Result<Vec<InputKey>, RecordError> {
    let mut keys: Vec<InputKey> = inputs
        .keys()
        .filter(|key| key.as_str() == "attempt.count" || key.as_str().starts_with("scope"))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// Folds one status into the worst seen so far.
///
/// `CONFLICT` dominates, then `UNKNOWN`, then `NOT_SATISFIED`, then `NEEDS`.
/// The order is deliberate and is not a fold from children to parent in
/// general — the harness contract leaves that to each engine — but for an
/// average it is exactly right: one attempt whose place in the denominator is
/// disputed makes the whole average disputed.
const fn worsen(current: ProofStatus, candidate: ProofStatus) -> ProofStatus {
    if rank(candidate) > rank(current) {
        candidate
    } else {
        current
    }
}

/// The severity order the fold above uses.
const fn rank(status: ProofStatus) -> u8 {
    match status {
        ProofStatus::Satisfied => 0,
        ProofStatus::Needs => 1,
        ProofStatus::NotSatisfied => 2,
        ProofStatus::Unknown => 3,
        ProofStatus::Conflict => 4,
    }
}
