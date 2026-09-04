//! The recorded graduation audit, and the re-run that reproduces it offline.
//!
//! # A recorded verdict is not evidence; a reproduced one is
//!
//! A bundle that carried "you may graduate" as text would prove only that
//! somebody once computed it. [`rerun_audit`] instead re-performs `P2-U3`'s
//! work from what the directory carries:
//!
//! 1. parse the frozen inputs out of `frozen-inputs.txt`;
//! 2. take SHA-256 over `rule-set.txt` and require it to equal the recorded
//!    `rule_set_hash` — section 37's *과거 audit은 당시 rule hash로 재현된다*;
//! 3. find, among the rule sets the **caller** supplies, the one whose
//!    canonical text is those exact bytes;
//! 4. rebuild the catalogue scope from [`CatalogScopeRecord`], decode the
//!    student profile out of the frozen inputs, and re-run section 11.1's
//!    selector;
//! 5. evaluate the engine and byte-compare
//!    [`EngineOutcome::canonical_bytes`] with `outcome.expected`.
//!
//! Step 4 is what makes this a re-run rather than a re-read.
//! [`academic_audit::SelectedRuleSet`] has private fields and exactly one
//! producer, inside [`academic_audit::select`], so the decision about which
//! published rules apply is genuinely taken again.
//!
//! # The rule set comes from the caller, never from the bundle
//!
//! `P2-U2` puts a published rule behind a two-attestation review gate. A bundle
//! that could mint a `RuleSet` would be a way around that gate, so this module
//! has no parser: it carries the rule set's canonical text, whose SHA-256 *is*
//! the `rule_set_hash`, and matches it against sets the caller already holds.
//! A bundle whose rules nobody still has is not silently evaluated under
//! different ones — it fails with [`ExportError::Absent`].
//!
//! # Nothing here takes a credential
//!
//! No parameter of [`rerun_audit`] is a key, a token, a host, an account or a
//! session. That is the type-level half of
//! `restore_without_vendor_or_school_account_succeeds`: there is no argument to
//! pass a vendor endpoint as.

use academic_audit::{
    CatalogEntry, DegreeAudit, GRADUATION_ENGINE_ID, GraduationAuditEngine, RuleSetCatalog,
    RuleSetScope, Selection,
    profile::{DegreeMode, GraduationStandard, InstitutionId},
    select,
};
use academic_domain::{
    ContentDigest,
    engines::{EngineVersion, FrozenInputs, RuleSetHash},
};
use academic_requirement::{AdmissionYear, RuleSet};
use serde::{Deserialize, Serialize};

use crate::{ExportError, ExportResult, bundle::encode_hex, read::ClaimedBundle};

/// Relative path of the frozen inputs, below the official-record part.
pub const FROZEN_INPUTS_FILE: &str = "audit/frozen-inputs.txt";
/// Relative path of the published rule set's canonical text.
pub const RULE_SET_FILE: &str = "audit/rule-set.txt";
/// Relative path of the recorded outcome bytes.
pub const OUTCOME_FILE: &str = "audit/outcome.expected";
/// Relative path of the rendered proof tree.
pub const PROOF_TREE_FILE: &str = "audit/proof-tree.txt";

/// Section 11.1's declared scope of the rule set an audit selected.
///
/// Seven fields, one per field section 11.1's yaml declares. They are recorded
/// rather than derived because the selector must be re-run over a catalogue,
/// and a catalogue entry is a scope paired with a published set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogScopeRecord {
    /// The university node of `institutionPath`.
    pub university: String,
    /// The college node.
    pub college: String,
    /// The department node.
    pub department: String,
    /// The admission year the scope covers.
    pub admission_year: u16,
    /// The lower end of `selectedGraduationStandardRange`.
    pub standard_from: String,
    /// The upper end.
    pub standard_to: String,
    /// The `majorMode` the scope covers.
    pub major_mode: String,
}

impl CatalogScopeRecord {
    /// Records the scope a selected rule set declares.
    pub fn of(scope: &RuleSetScope) -> Self {
        let (standard_from, standard_to) = scope.standard_range();
        Self {
            university: scope.university().as_str().to_owned(),
            college: scope.college().as_str().to_owned(),
            department: scope.department().as_str().to_owned(),
            admission_year: scope.admission_year().get(),
            standard_from: standard_from.as_str().to_owned(),
            standard_to: standard_to.as_str().to_owned(),
            major_mode: scope.major_mode().as_str().to_owned(),
        }
    }

    /// Rebuilds the scope, refusing every value the audit crate refuses.
    pub fn rebuild(&self) -> ExportResult<RuleSetScope> {
        let mode = DegreeMode::ALL
            .into_iter()
            .find(|mode| mode.as_str() == self.major_mode)
            .ok_or_else(|| ExportError::Malformed {
                item: "recorded major mode",
                value: self.major_mode.clone(),
            })?;
        Ok(RuleSetScope::new(
            InstitutionId::new(&self.university)?,
            InstitutionId::new(&self.college)?,
            InstitutionId::new(&self.department)?,
            AdmissionYear::new(self.admission_year).map_err(|_| ExportError::Malformed {
                item: "recorded admission year",
                value: self.admission_year.to_string(),
            })?,
            GraduationStandard::new(&self.standard_from)?,
            GraduationStandard::new(&self.standard_to)?,
            mode,
        )?)
    }
}

/// The graduation audit a bundle records, as identities and paths.
///
/// No verdict text. What is recorded is what a re-run needs and what a re-run
/// is compared against: the engine identity and version, the rule-set hash, the
/// digests of the two inputs, the selected scope, and the four files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRecord {
    /// The registered engine identifier.
    pub engine_id: String,
    /// The engine version the recorded outcome was produced by.
    pub engine_version: u16,
    /// SHA-256 of the published rule set's canonical text, lowercase hex.
    pub rule_set_hash: String,
    /// The published rule set's version number.
    pub rule_set_version: u32,
    /// SHA-256 of the frozen input encoding, lowercase hex.
    pub frozen_inputs_sha256: String,
    /// SHA-256 of the recorded outcome bytes, lowercase hex.
    pub outcome_sha256: String,
    /// The scope of the rule set the selector chose.
    pub selected_scope: CatalogScopeRecord,
    /// The bundle-relative path of the frozen input encoding.
    pub frozen_inputs_path: String,
    /// The bundle-relative path of the published rule set's canonical text.
    pub rule_set_path: String,
    /// The bundle-relative path of the recorded outcome bytes.
    pub outcome_path: String,
    /// The bundle-relative path of the rendered proof tree.
    pub proof_tree_path: String,
}

impl AuditRecord {
    /// Every bundle-relative path this record references.
    ///
    /// The reader walks this so a recorded audit cannot point at a file the
    /// bundle does not list, which is the same dangling-locator rule the
    /// objects obey.
    #[must_use]
    pub fn referenced_paths(&self) -> [&str; 4] {
        [
            &self.frozen_inputs_path,
            &self.rule_set_path,
            &self.outcome_path,
            &self.proof_tree_path,
        ]
    }
}

/// What a re-run established.
///
/// Private fields and one producer. There is no constructor taking a boolean,
/// so a value of this type is a re-run that reproduced: [`rerun_audit`] returns
/// [`ExportError::AuditNotReproduced`] otherwise rather than a value saying so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRerun {
    engine_id: String,
    engine_version: EngineVersion,
    rule_set_hash: String,
    outcome_sha256: String,
    selected_scope_text: String,
    outcome_byte_length: usize,
}

impl AuditRerun {
    /// The engine that was re-run.
    #[must_use]
    pub fn engine_id(&self) -> &str {
        &self.engine_id
    }

    /// The version it was re-run at.
    #[must_use]
    pub const fn engine_version(&self) -> EngineVersion {
        self.engine_version
    }

    /// The rule-set hash both runs presented.
    #[must_use]
    pub fn rule_set_hash(&self) -> &str {
        &self.rule_set_hash
    }

    /// The digest of the outcome bytes both runs produced.
    #[must_use]
    pub fn outcome_sha256(&self) -> &str {
        &self.outcome_sha256
    }

    /// The scope the re-run selector chose, in the audit crate's own rendering.
    #[must_use]
    pub fn selected_scope_text(&self) -> &str {
        &self.selected_scope_text
    }

    /// How many bytes the reproduced outcome is.
    #[must_use]
    pub const fn outcome_byte_length(&self) -> usize {
        self.outcome_byte_length
    }
}

/// Re-runs the recorded graduation audit from a bundle and published rules.
///
/// Fails closed at every step. A bundle whose frozen inputs were edited, whose
/// rule-set text no longer hashes to the recorded hash, whose recorded scope
/// selects nothing or selects two sets, or whose recorded outcome differs by
/// one byte is refused rather than reported as a partial success.
pub fn rerun_audit(bundle: &ClaimedBundle, published: &[RuleSet]) -> ExportResult<AuditRerun> {
    let record = &bundle.manifest().semantic.audit;
    if record.engine_id != GRADUATION_ENGINE_ID {
        return Err(ExportError::mismatch(
            "recorded audit engine",
            GRADUATION_ENGINE_ID,
            &record.engine_id,
        ));
    }
    let engine_version = EngineVersion::new(record.engine_version)?;

    let frozen_text = bundle.read_text(&record.frozen_inputs_path)?;
    let observed_inputs_digest = encode_hex(
        ContentDigest::sha256(frozen_text.as_bytes())
            .as_bytes()
            .as_slice(),
    );
    if observed_inputs_digest != record.frozen_inputs_sha256 {
        return Err(ExportError::mismatch(
            "recorded frozen input digest",
            &record.frozen_inputs_sha256,
            observed_inputs_digest,
        ));
    }
    let inputs = FrozenInputs::parse(&frozen_text)?;

    let rule_set_text = bundle.read_text(&record.rule_set_path)?;
    let rule_set_digest = ContentDigest::sha256(rule_set_text.as_bytes());
    let observed_rule_set_hash = encode_hex(rule_set_digest.as_bytes().as_slice());
    if observed_rule_set_hash != record.rule_set_hash {
        return Err(ExportError::mismatch(
            "recorded rule set hash",
            &record.rule_set_hash,
            observed_rule_set_hash,
        ));
    }

    let rules = published
        .iter()
        .find(|candidate| candidate.canonical_text() == rule_set_text)
        .ok_or_else(|| ExportError::Absent {
            item: "a published rule set whose canonical text the bundle records",
            value: record.rule_set_hash.clone(),
        })?;
    if rules.version().get() != record.rule_set_version {
        return Err(ExportError::mismatch(
            "recorded rule set version",
            record.rule_set_version,
            rules.version().get(),
        ));
    }

    let scope = record.selected_scope.rebuild()?;
    let catalog = RuleSetCatalog::new().with(CatalogEntry::new(scope, rules.clone()));
    let facts = academic_audit::decode(&inputs)?;
    let selection = select(&facts.profile, &catalog);
    let selected = match selection {
        Selection::Selected(selected) => *selected,
        Selection::Indeterminate(_) => {
            return Err(ExportError::AuditNotReproduced(
                "the recorded scope selects no published rule set for the recorded profile",
            ));
        }
    };
    let selected_scope_text = selected.scope().canonical_text();

    let engine = GraduationAuditEngine::new(selected, engine_version);
    let rule_set_hash = RuleSetHash::new(rule_set_digest);
    if engine.rule_set_hash() != rule_set_hash {
        return Err(ExportError::AuditNotReproduced(
            "the published rule set does not hash to the recorded rule set hash",
        ));
    }
    let audit = DegreeAudit::evaluate(&engine, &inputs)?;
    let reproduced = audit.outcome().canonical_bytes(
        GRADUATION_ENGINE_ID,
        rule_set_hash,
        engine_version,
        &inputs,
    );

    let recorded = bundle.read_bytes(&record.outcome_path)?;
    if recorded != reproduced {
        return Err(ExportError::AuditNotReproduced(
            "the re-run produced different outcome bytes",
        ));
    }
    let outcome_sha256 = encode_hex(ContentDigest::sha256(&reproduced).as_bytes().as_slice());
    if outcome_sha256 != record.outcome_sha256 {
        return Err(ExportError::mismatch(
            "recorded outcome digest",
            &record.outcome_sha256,
            outcome_sha256,
        ));
    }

    Ok(AuditRerun {
        engine_id: GRADUATION_ENGINE_ID.to_owned(),
        engine_version,
        rule_set_hash: record.rule_set_hash.clone(),
        outcome_sha256,
        selected_scope_text,
        outcome_byte_length: reproduced.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::{FROZEN_INPUTS_FILE, OUTCOME_FILE, PROOF_TREE_FILE, RULE_SET_FILE};

    #[test]
    fn the_four_audit_files_are_distinct_and_live_under_one_directory() {
        let paths = [
            FROZEN_INPUTS_FILE,
            RULE_SET_FILE,
            OUTCOME_FILE,
            PROOF_TREE_FILE,
        ];
        let mut sorted = paths;
        sorted.sort_unstable();
        sorted.iter().reduce(|left, right| {
            assert_ne!(left, right);
            right
        });
        for path in paths {
            assert!(path.starts_with("audit/"), "{path} is outside audit/");
        }
    }
}
