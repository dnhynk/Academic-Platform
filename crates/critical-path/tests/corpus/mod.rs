//! The deterministic builder for this crate's `P2-C5` corpus.
//!
//! One builder, reached by two callers: `tests/critical_path_harness.rs`
//! re-renders it and byte-compares the committed files, and
//! `examples/emit_corpus.rs` writes them. That is `CONTRIBUTING.md` rule 5 --
//! a golden fixture changes only through the builder -- with the additional
//! property that the two halves cannot drift, because there is only one.
//!
//! It lives in the test tree because a case needs a real `P2-N5` `GapCase`, and
//! the only producer of one is `academic_gap::search` over the fixture chain in
//! `tests/common/mod.rs`. An example is compiled with dev-dependencies, so it
//! reaches the same module.

// Two targets include this module by `#[path]` -- the harness suite and the
// corpus example -- and each uses a different subset.
#![allow(
    dead_code,
    unused_imports,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]

#[path = "../common/mod.rs"]
pub mod common;

use std::error::Error;

use academic_critical_path::{
    BENEFIT_COMPONENTS, CONSTRAINTS, COST_COMPONENTS, CRITICAL_PATH_CORPUS_ROOT, ConceptEstimate,
    ConstraintInputs, CostComponent, DISCLOSURE_GROUPS, MAX_SATISFYING_SETS, NAMED_STRATEGIES,
    PATH_ROLES, PlanRequest, PreferenceSlider, PrerequisiteHypergraph, STAGE_RULES,
    UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE, frozen_inputs, outcome, plan,
};
use academic_domain::{
    FreshnessBand,
    engines::{EngineVersion, FrozenInputs, RuleSetHash},
};
use academic_gap::GapCase;

use common::{
    all_concepts, buffer_pool, cost_except, disk_page, fan_out, flat_benefit, flat_estimates,
    page_layout, permissive_constraints, random_io, reading_for, rule_set, section_16_1_graph,
    section_36_4_gap, spec_order_slider, storage_hierarchy, unmeasured, with_estimate,
};

/// The engine version every committed case is evaluated under.
pub const ENGINE_VERSION: u16 = 1;

/// The identifier the canonical bytes are keyed by.
///
/// **Not** a `P2-C5` registry name. Section 28's table has twelve rows and none
/// of them is a critical path engine, so this crate claims no registered
/// `engine_id`; this string names the corpus and appears in no registry.
/// `the_registry_does_not_hold_a_critical_path_engine` asserts both halves.
pub const CORPUS_ENGINE_LABEL: &str = "SECTION_16_CRITICAL_PATH";

/// One committed corpus file.
pub struct CorpusFile {
    /// Path relative to the repository root.
    pub path: String,
    /// Exact bytes.
    pub bytes: Vec<u8>,
}

/// One case the corpus ships.
pub struct Case {
    /// Its committed name.
    pub name: &'static str,
    /// The hypergraph it is solved over.
    pub graph: PrerequisiteHypergraph,
    /// One estimate per concept.
    pub estimates: Vec<ConceptEstimate>,
    /// Section 16.3's inputs.
    pub constraints: ConstraintInputs,
    /// The preference in force.
    pub slider: PreferenceSlider,
}

/// The cases, in committed order.
///
/// Four, and each reaches a different one of section 16's answers: two routes
/// offered, one route left after an exclusion, no route feasible at all, and a
/// route whose relations are all uncertain and one of whose costs rests on
/// nothing. `the_corpus_cases_are_not_all_the_same_shape` measures that they
/// differ rather than asserting it here.
pub fn cases() -> Result<Vec<Case>, Box<dyn Error>> {
    let sole = {
        let mut inputs = permissive_constraints();
        inputs.user_excluded_concepts = vec![storage_hierarchy()];
        inputs
    };
    let none = {
        let mut inputs = permissive_constraints();
        inputs.horizon_days = 0;
        inputs
    };
    let stale = {
        let mut inputs = permissive_constraints();
        inputs.bands = all_concepts()
            .into_iter()
            .map(|concept| {
                (
                    concept,
                    if concept == disk_page() {
                        FreshnessBand::Stale
                    } else {
                        FreshnessBand::High
                    },
                )
            })
            .collect();
        inputs
    };
    let unmeasured_estimates = with_estimate(
        flat_estimates()?,
        ConceptEstimate {
            concept: disk_page(),
            cost: cost_except(
                10,
                CostComponent::LearningEffort,
                unmeasured(CostComponent::LearningEffort, 30, 90)?,
            )?,
            benefit: flat_benefit(10)?,
            options: vec![reading_for(disk_page(), "dp")?],
        },
    );
    Ok(vec![
        Case {
            name: "two_routes",
            graph: section_16_1_graph(&[])?,
            estimates: flat_estimates()?,
            constraints: permissive_constraints(),
            slider: spec_order_slider()?,
        },
        Case {
            name: "sole_route",
            graph: section_16_1_graph(&[])?,
            estimates: flat_estimates()?,
            constraints: sole,
            slider: spec_order_slider()?,
        },
        Case {
            name: "no_feasible_route",
            graph: section_16_1_graph(&[])?,
            estimates: flat_estimates()?,
            constraints: none,
            slider: spec_order_slider()?,
        },
        Case {
            name: "uncertain_and_stale",
            graph: section_16_1_graph(&[
                (buffer_pool(), disk_page()),
                (buffer_pool(), random_io()),
                (disk_page(), storage_hierarchy()),
                (disk_page(), fan_out()),
                (disk_page(), page_layout()),
            ])?,
            estimates: unmeasured_estimates,
            constraints: stale,
            slider: spec_order_slider()?,
        },
    ])
}

/// What one evaluated case yields: its frozen inputs, the canonical bytes of its
/// outcome, and its normalized explanation snapshot.
///
/// A named type rather than a tuple, so the three halves the determinism
/// contract compares are distinguishable at every call site.
pub struct Evaluated {
    /// `P2-C5`'s frozen inputs for the run.
    pub inputs: FrozenInputs,
    /// `EngineOutcome::canonical_bytes` for it.
    pub bytes: Vec<u8>,
    /// The normalized explanation, as bytes.
    pub snapshot: Vec<u8>,
}

/// Evaluates one case, returning its frozen inputs, its canonical bytes and its
/// explanation snapshot.
pub fn evaluate(
    gap_case: &GapCase,
    case: &Case,
    hash: RuleSetHash,
    version: EngineVersion,
) -> Result<Evaluated, Box<dyn Error>> {
    let request = PlanRequest {
        gap_case,
        graph: &case.graph,
        estimates: &case.estimates,
        constraints: &case.constraints,
        slider: &case.slider,
        rule_set_hash: rule_set(),
        engine_version: ENGINE_VERSION,
    };
    let result = plan(&request)?;
    let inputs = frozen_inputs(&request)?;
    let answer = outcome(&result, &inputs)?;
    let bytes = answer.canonical_bytes(CORPUS_ENGINE_LABEL, hash, version, &inputs);
    let snapshot = answer.explanation_snapshot.as_str().as_bytes().to_vec();
    Ok(Evaluated {
        inputs,
        bytes,
        snapshot,
    })
}

/// The whole corpus, rendered from the engine.
pub fn corpus_files() -> Result<Vec<CorpusFile>, Box<dyn Error>> {
    let gap_case = section_36_4_gap()?;
    let hash = RuleSetHash::new(rule_set());
    let version = EngineVersion::new(ENGINE_VERSION)?;

    let mut files = vec![
        CorpusFile {
            path: format!("{CRITICAL_PATH_CORPUS_ROOT}/ruleset.txt"),
            bytes: ruleset_text().into_bytes(),
        },
        CorpusFile {
            path: format!("{CRITICAL_PATH_CORPUS_ROOT}/property/bounds.txt"),
            bytes: bounds_text().into_bytes(),
        },
    ];

    let mut explanation: Option<Vec<u8>> = None;
    for case in cases()? {
        let evaluated = evaluate(&gap_case, &case, hash, version)?;
        files.push(CorpusFile {
            path: format!("{CRITICAL_PATH_CORPUS_ROOT}/golden/{}.input", case.name),
            bytes: evaluated.inputs.canonical_text().into_bytes(),
        });
        files.push(CorpusFile {
            path: format!("{CRITICAL_PATH_CORPUS_ROOT}/golden/{}.expected", case.name),
            bytes: evaluated.bytes,
        });
        if explanation.is_none() {
            explanation = Some(evaluated.snapshot);
        }
    }
    files.push(CorpusFile {
        path: format!("{CRITICAL_PATH_CORPUS_ROOT}/explanation.snapshot"),
        bytes: explanation.ok_or("the corpus has no case")?,
    });
    Ok(files)
}

/// The published rule set this corpus is pinned to.
///
/// One line per section 16 stage plus one per section 16.3 constraint, derived
/// from the crate's own arrays rather than typed out, so a constraint added
/// there changes the rule set here and every `.expected` file with it.
#[must_use]
pub fn ruleset_text() -> String {
    let mut rendered = String::new();
    for rule in STAGE_RULES {
        rendered.push_str(rule);
        rendered.push('\n');
    }
    for constraint in CONSTRAINTS {
        rendered.push_str(constraint.as_str());
        rendered.push('\n');
    }
    rendered
}

/// The bounds a property test over this engine is driven from.
///
/// Every number is read out of the crate, so the file is a measurement of the
/// engine rather than a second place a count is written.
#[must_use]
pub fn bounds_text() -> String {
    format!(
        "benefit_axes={}\nconstraints={}\ncost_axes={}\ndisclosure_groups={}\n\
         max_satisfying_sets={}\nnamed_strategies={}\npath_roles={}\n\
         uncertain_edge_threshold_permille={}\n",
        BENEFIT_COMPONENTS.len(),
        CONSTRAINTS.len(),
        COST_COMPONENTS.len(),
        DISCLOSURE_GROUPS.len(),
        MAX_SATISFYING_SETS,
        NAMED_STRATEGIES.len(),
        PATH_ROLES.len(),
        UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE,
    )
}
