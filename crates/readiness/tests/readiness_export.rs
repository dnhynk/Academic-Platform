//! `non_guarantee_disclaimer_survives_export`, measured against `P2-P1` itself.
//!
//! The claim is not that a readiness view *carries* a notice. It is that
//! somebody holding only a `P2-P1` graduation bundle — no product, no key, no
//! vendor, no school account — cannot read the matrix without the notice.
//!
//! So this file does not simulate an export. It builds a real
//! `academic_export::BundleRequest`, writes a real bundle with
//! `write_bundle`, and reads it back with `read_bundle`, which takes a path and
//! **no key, no token, no host and no account**. The readiness view travels as
//! the canonical JSON of a claim under a `career.` predicate, which is the
//! namespace section 37's `role 관심 변화와 alternative paths` part selects, so
//! it is routed and written by that crate's own code rather than by a copy of
//! it here.
//!
//! # Three things are measured, and the third is `P2-P1`'s and not this crate's
//!
//! 1. **The bytes arrive.** The restored claim's canonical JSON is compared
//!    byte for byte with what was handed to the writer, and
//!    `academic_readiness::published_notice` reads the notice out of the
//!    restored bytes.
//! 2. **A document without the notice is refused by the reader that reads it.**
//!    `published_notice` is run over the restored bytes with the notice removed
//!    and with the notice altered by one character, and refuses both.
//! 3. **A recipient cannot quietly remove it from a sealed bundle.** The
//!    notice is deleted from the published file on disk and `read_bundle` is
//!    run again: it refuses the bundle, because `P2-P1` digests every file and
//!    seals the manifest. That refusal is that crate's, and this test is where
//!    the two contracts meet.
//!
//! # What this does not claim
//!
//! It does not claim that no program anywhere could parse the matrix rows out
//! of the JSON while ignoring the notice key. Nothing in an open format can
//! claim that. What is claimed is that the notice is *in* the bytes a recipient
//! receives, that this repository's own reader refuses a document that has lost
//! it, and that removing it from a published bundle breaks that bundle.

use std::{
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use academic_domain::{DomainId, FreshnessBand};
use academic_export::{
    BundleRequest, ClaimSource, CopyrightNotice, DeviceHead, DomainTerms, OriginalInclusion,
    PostureBlock, RecordedAudit, SensitivityLabel, SourceView, StoreIdentity, TermsRegister,
    Watermark, read_bundle, write_bundle,
};
use academic_readiness::{
    CompetencyInput, NOTICE_KEY, NonGuaranteeNotice, ReadinessAxis, ReadinessView,
    published_notice, take,
};

mod support;

#[path = "../../audit/tests/support/mod.rs"]
mod audit_support;

use support::{TestResult, bundle, competency_about, entry, knowledge_record, ontology, placed};

/// The predicate the readiness view is recorded under.
///
/// Its first segment is `career`, which `academic_export::BundlePart` routes to
/// `ROLE_INTEREST_AND_ALTERNATIVE_PATHS`. The routing is that crate's, read
/// rather than restated.
const READINESS_PREDICATE: &str = "career.readiness_matrix";

const GENERATED_AT_UNIX_MS: i64 = 1_772_200_000_000;
const BUNDLE_NOTICE: &str =
    "Synthetic fixture bundle. Generated records are the exporting build's own.";
const DOMAIN_NOTICE: &str = "Synthetic career material. Held under the fixture's own terms.";

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

/// The temporary directory, resolved.
///
/// **Canonicalized on unix.** `academic_export::directory` walks every ancestor
/// of a bundle destination and refuses one that is not a directory, and on
/// macOS `env::temp_dir()` sits under `/var`, which is a symlink to
/// `private/var` -- so `write_bundle` refuses the path with
/// `Malformed { item: "bundle directory", value: "/var" }`. That is that
/// crate's contract and not a fault to work around here, so the base is
/// resolved before a name is built on it, exactly as
/// `crates/export/tests/support/mod.rs` does for the same reason.
#[cfg(unix)]
fn temporary_base() -> io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

#[cfg(windows)]
fn temporary_base() -> io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

/// A scratch directory under the build lane, removed when the test ends.
///
/// The name carries the process id, the wall clock and a counter, and the
/// directory is **reserved** with `create_dir` rather than cleared: two lanes
/// sharing one machine's temporary directory would otherwise delete each
/// other's tree, and `tools/shared-name-isolation.test.mjs` refuses a name that
/// separates no two processes. `crates/export`'s own suite is where this shape
/// comes from.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> TestResult<Self> {
        for _ in 0..64 {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "system clock is before the Unix epoch")?
                .as_nanos();
            let root = temporary_base()?.join(format!(
                "acad-y3-{label}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self { root }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not reserve a unique readiness export test root".into())
    }

    fn child(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The readiness view this test exports, serialized.
fn readiness_document() -> TestResult<(ReadinessView, String)> {
    let concept = ontology("RELATIONAL_DATABASE");
    let competency = competency_about(
        "relational-database-diagnosis",
        &concept,
        &["chooses-an-index"],
        &academic_competency::EvidenceStage::ALL,
    )?;
    let profile = bundle(
        "export-fixture-profile",
        vec![entry(
            &competency,
            academic_role_profile::BundleImportance::Core,
        )],
    )?;
    let record = knowledge_record(
        "explained",
        academic_competency::EvidenceStage::Used,
        support::entity("RELATIONAL_DATABASE"),
    )?;
    let placements = vec![placed(
        ReadinessAxis::AcademicLearning,
        "chooses-an-index",
        "lecture.db.03",
        &record,
    )?];
    let matrix = take(
        &profile,
        &[CompetencyInput::of(
            &competency,
            &placements,
            FreshnessBand::High,
        )],
    );
    let view = ReadinessView::of(matrix, &[&competency])?;
    let document = serde_json::to_string(&view)?;
    Ok((view, document))
}

fn domain() -> DomainId {
    support::domain_id("readiness-export-domain")
}

fn source_view(document: &str) -> TestResult<SourceView> {
    let domain = domain().to_string();
    Ok(SourceView {
        store: StoreIdentity {
            format_uuid: "0f7b6d2e-4c3a-4f19-9a5d-6b1c2e3f4a5b".to_owned(),
            schema_version: 1,
            schema_semver: "1.0.0".to_owned(),
        },
        watermark: Watermark {
            next_accept_seq: 2,
            profile_revision: 1,
            accept_seq_head: 1,
            outbox_head: 0,
        },
        device_heads: vec![DeviceHead {
            device_id: "synthetic-device-0001".to_owned(),
            next_origin_seq: 2,
            head_envelope_sha256: "0".repeat(64),
        }],
        canonical_semantic_digest: "0".repeat(64),
        batches: Vec::new(),
        events: Vec::new(),
        scopes: Vec::new(),
        artifacts: Vec::new(),
        evidence: Vec::new(),
        claims: vec![ClaimSource::new(
            "claim-readiness-0001",
            domain.clone(),
            READINESS_PREDICATE,
            Vec::new(),
            document,
        )?],
        relations: Vec::new(),
        decisions: Vec::new(),
        git_refs: Vec::new(),
    })
}

fn terms() -> TestResult<TermsRegister> {
    Ok(
        TermsRegister::new(CopyrightNotice::new(BUNDLE_NOTICE)?).with_domain(
            domain().to_string(),
            DomainTerms::new(
                SensitivityLabel::Personal,
                CopyrightNotice::new(DOMAIN_NOTICE)?,
            ),
        ),
    )
}

fn posture() -> PostureBlock {
    PostureBlock {
        data_policy: "SYNTHETIC_ONLY".to_owned(),
        storage_mode: "PLAINTEXT_SQLITE".to_owned(),
        storage_encryption: "NONE".to_owned(),
        production_data_allowed: false,
        product_network: "OFFLINE".to_owned(),
    }
}

/// The graduation audit a bundle records, run through `P2-U3`'s own engine.
struct Graduation {
    rules: academic_requirement::RuleSet,
    inputs: academic_domain::engines::FrozenInputs,
    scope: academic_audit::RuleSetScope,
    audit: academic_audit::DegreeAudit,
    engine_version: academic_domain::engines::EngineVersion,
}

impl Graduation {
    fn baseline() -> TestResult<Self> {
        let rules = audit_support::baseline_rules()?;
        let facts = audit_support::audit_facts(
            audit_support::transcript()?,
            audit_support::sources(&rules)?,
            Vec::new(),
            Some(audit_support::FRESHNESS),
        )?;
        let inputs = academic_audit::encode(&facts)?;
        let scope = audit_support::scope()?;
        let catalog = audit_support::catalog(&rules)?;
        let selection = academic_audit::select(&facts.profile, &catalog);
        let selected = selection
            .selected()
            .ok_or("the baseline profile selected no rule set")?
            .clone();
        let engine_version = academic_domain::engines::EngineVersion::new(1)?;
        let engine = academic_audit::GraduationAuditEngine::new(selected, engine_version);
        let audit = academic_audit::DegreeAudit::evaluate(&engine, &inputs)?;
        Ok(Self {
            rules,
            inputs,
            scope,
            audit,
            engine_version,
        })
    }

    fn recorded(&self) -> RecordedAudit<'_> {
        RecordedAudit {
            engine_version: self.engine_version,
            inputs: &self.inputs,
            rules: &self.rules,
            scope: &self.scope,
            audit: &self.audit,
        }
    }
}

/// The notice is in the bytes a bundle-only recipient receives.
#[test]
fn non_guarantee_disclaimer_survives_export() -> TestResult {
    let (view, document) = readiness_document()?;
    let notice = NonGuaranteeNotice::rendered().text();

    // The notice is in the serialized view before anything exports it, and the
    // reader that will be used on the far side accepts it here.
    assert!(document.contains(NOTICE_KEY));
    assert!(document.contains(&notice));
    assert_eq!(published_notice(&document)?, view.notice());

    let graduation = Graduation::baseline()?;
    let source = source_view(&document)?;
    let terms = terms()?;
    let posture = posture();
    let request = BundleRequest {
        source: &source,
        posture: &posture,
        terms: &terms,
        originals: OriginalInclusion::Withheld,
        audit: graduation.recorded(),
        generated_at_unix_ms: GENERATED_AT_UNIX_MS,
    };

    let scratch = Scratch::new("survives")?;
    let destination = scratch.child("bundle");
    let receipt = write_bundle(&request, &destination)?;
    assert_eq!(receipt.destination, destination);

    // 1. Read it back with no key, no token, no host and no account. The
    //    reader verifies the format marker, the manifest's semantic digest and
    //    every file's own digest before returning, so what is on disk after it
    //    returns is what the manifest says it is.
    let restored = read_bundle(&destination)?;
    assert_eq!(restored.root(), destination);

    // Every published file that carries the readiness document carries the
    // notice with it. The set is found by searching the tree rather than by a
    // path this test wrote down, so a bundle layout change moves the search
    // instead of pointing it at a file that no longer holds the claim.
    let carrying = files_carrying(&destination, &notice)?;
    assert!(
        carrying.len() >= 2,
        "the readiness document reached {} published files, and the writer puts it in a topical part and in the canonical stream",
        carrying.len()
    );
    let mut lines = 0_usize;
    for path in &carrying {
        let published = fs::read_to_string(path)?;
        for line in published.lines() {
            if !line.contains(NOTICE_KEY) {
                continue;
            }
            lines += 1;
            assert_eq!(
                line, document,
                "the exported readiness document is not the one that was written"
            );
            assert_eq!(published_notice(line)?, view.notice());
        }
    }
    assert!(lines >= 2, "no published line carried the document whole");
    let carried = document.clone();

    // 2. The reader refuses a document that lost the notice, and one that
    //    changed it. Both are built out of the restored bytes, so neither is a
    //    string this test typed.
    let stripped = carried.replacen(&format!("\"{NOTICE_KEY}\":\"{notice}\","), "", 1);
    assert_ne!(stripped, carried);
    assert!(
        !stripped.contains(NOTICE_KEY),
        "the notice key survived the strip, so the refusal below would be about the wrong thing"
    );
    assert!(matches!(
        published_notice(&stripped),
        Err(academic_readiness::ReadinessError::NoticeIsMissing)
    ));

    let altered = carried.replacen(&notice, &notice.replace('·', "-"), 1);
    assert_ne!(altered, carried);
    assert!(matches!(
        published_notice(&altered),
        Err(academic_readiness::ReadinessError::NoticeDoesNotMatch)
    ));

    // 3. Removing the notice from the published bundle breaks the bundle. This
    //    refusal is `P2-P1`'s: every file is digested and the manifest is
    //    sealed, so a recipient cannot quietly publish a matrix without one.
    for path in &carrying {
        let published = fs::read_to_string(path)?;
        fs::write(path, published.replace(&notice, ""))?;
        let refused = read_bundle(&destination);
        assert!(
            refused.is_err(),
            "a bundle whose readiness notice was deleted from {} was still read: {refused:?}",
            path.display()
        );
        fs::write(path, &published)?;
        read_bundle(&destination)?;
    }
    Ok(())
}

/// Every published file whose text carries `needle`.
fn files_carrying(root: &std::path::Path, needle: &str) -> TestResult<Vec<PathBuf>> {
    let mut found = Vec::new();
    walk(root, &mut found)?;
    found.sort();
    Ok(found
        .into_iter()
        .filter(|path| fs::read_to_string(path).is_ok_and(|text| text.contains(needle)))
        .collect())
}

fn walk(root: &std::path::Path, found: &mut Vec<PathBuf>) -> TestResult {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}
