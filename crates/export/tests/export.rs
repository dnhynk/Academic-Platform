//! `t068` section 5's seven named `P2-P1` tests.
//!
//! Each one carries the half that stops it being vacuous, and that half is
//! stated beside the assertion rather than left to a reader to reconstruct.
//! The pattern this suite follows is `T177`'s: what is compared is what is on
//! disk, enumerated, rather than a list somebody wrote down.

mod support;

use std::{collections::BTreeSet, fs, path::Path};

use academic_domain::ContentDigest;
use academic_export::{
    BundleManifest, BundlePart, BundleRequest, FORMAT_MARKER_FILE, MANIFEST_FILE,
    OriginalInclusion, PDF_RENDERING_ABSENCE, SensitivityLabel, SharingRestriction,
    bundle::encode_hex, read_bundle, rerun_audit, write_bundle,
};

use support::{
    DOMAIN_NOTICE, Fixture, GENERATED_AT_UNIX_MS, Graduation, TestResult, WITH_ORIGINALS,
    WITHOUT_ORIGINALS, hex_lower, list_files,
};

/// The design document the six parts are read out of.
const SPECIFICATION: &str =
    include_str!("../../../PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md");

// ---------------------------------------------------------------------------
// 1. open_export_round_trip
// ---------------------------------------------------------------------------

/// A bundle written from a committed watermark reads back into the same
/// identifiers, digests, lineage and decisions the store still holds.
///
/// **What stops this being vacuous.** The side the bundle is compared against
/// is the profile's own database, read through `academic-portability` after the
/// bundle was written — not the value the bundle was written from. Every record
/// class is additionally required to be non-empty, so a writer that dropped one
/// stream would fail here rather than pass with nothing to compare.
#[test]
fn open_export_round_trip() -> TestResult {
    let fixture = Fixture::new("round-trip")?;
    let graduation = Graduation::baseline()?;
    let view = fixture.source_view()?;
    let terms = support::terms()?;
    let posture = support::posture();
    let destination = fixture.work_path("bundle");
    write_bundle(
        &BundleRequest {
            source: &view,
            posture: &posture,
            terms: &terms,
            originals: WITH_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS,
        },
        &destination,
    )?;

    let bundle = read_bundle(&destination)?;
    let rows = fixture.canonical_rows()?;

    assert!(!rows.artifacts.is_empty());
    assert!(!rows.evidence.is_empty());
    assert!(!rows.claims.is_empty());
    assert!(!rows.relations.is_empty());
    assert!(!rows.decisions.is_empty());
    assert!(!rows.scopes.is_empty());
    assert!(!rows.events.is_empty());
    assert!(!rows.batches.is_empty());

    let domain = support::domain_id()?.to_string();
    let graph = format!(
        "parts/{}/canonical",
        BundlePart::MachineReadableGraph.directory()
    );

    // Every stream is read across **every** security domain the bundle carries,
    // by enumerating the files the manifest lists under each prefix rather than
    // naming one domain. The corpus has two, so a comparison against one would
    // have silently dropped a domain's rows.
    assert!(
        support::domain_id()? != support::second_domain_id()?,
        "the corpus holds one security domain, so per-domain partitioning is unobserved"
    );

    // Identifiers and hashes.
    assert_eq!(
        read_stream(&bundle, &format!("{graph}/artifacts/"))?,
        sorted(expected_lines(&rows.artifacts)?)
    );
    assert_eq!(
        read_stream(&bundle, &format!("{graph}/evidence/"))?,
        sorted(expected_lines(&rows.evidence)?)
    );
    assert_eq!(
        read_stream(&bundle, &format!("{graph}/claims/"))?,
        sorted(expected_lines(&rows.claims)?)
    );

    // Lineage: the relations between claims, and the accepted events.
    assert_eq!(
        read_stream(&bundle, &format!("{graph}/relations/"))?,
        sorted(expected_lines(&rows.relations)?)
    );
    assert_eq!(
        read_stream(&bundle, &format!("{graph}/events/"))?,
        sorted(expected_lines(&rows.events)?)
    );

    // Decisions.
    assert_eq!(
        read_stream(&bundle, &format!("{graph}/decisions/"))?,
        sorted(expected_lines(&rows.decisions)?)
    );

    // The JSON-LD graph carries one node per artifact. A graph keyed by the
    // vault locator would have merged the two that share one into a single
    // node, and every JSONL comparison above would still have passed.
    let mut document = String::new();
    for record in &bundle.manifest().semantic.files {
        if record.path().starts_with(&format!(
            "parts/{}/graph/",
            BundlePart::MachineReadableGraph.directory()
        )) {
            document.push_str(&String::from_utf8(bundle.read_bytes(record.path())?)?);
        }
    }
    assert!(!document.is_empty(), "the bundle carries no graph document");
    for artifact in &rows.artifacts {
        assert!(
            document.contains(&format!("urn:academic:artifact:{}", artifact.artifact_id)),
            "{} has no node in the exported graph",
            artifact.artifact_id
        );
    }
    assert_eq!(
        document.matches("\"Artifact\"").count(),
        rows.artifacts.len(),
        "the exported graph holds a different number of artifact nodes than the ledger"
    );
    let _ = &domain;

    // The whole canonical state agrees, not only the streams compared above.
    assert_eq!(
        bundle.manifest().semantic.canonical_semantic_digest,
        hex_lower(rows.semantic_digest()?.as_bytes().as_slice())
    );
    assert_eq!(
        bundle.manifest().semantic.counts.claims,
        rows.claims.len() as u64
    );
    assert_eq!(
        bundle.manifest().semantic.counts.decisions,
        rows.decisions.len() as u64
    );

    // The original signed envelopes are the ledger's own bytes.
    for batch in &rows.batches {
        let relative = format!(
            "parts/{}/ledger/batches/{}.cbor",
            BundlePart::MachineReadableGraph.directory(),
            batch.batch_id
        );
        let bytes = bundle.read_bytes(&relative)?;
        assert_eq!(
            encode_hex(ContentDigest::sha256(&bytes).as_bytes().as_slice()),
            batch.envelope_sha256,
            "the exported envelope for {} is not the accepted bytes",
            batch.batch_id
        );
        assert_eq!(bytes.len() as u64, batch.envelope_byte_length);
    }

    // The exact originals.
    for artifact in &rows.artifacts {
        let object = bundle
            .manifest()
            .semantic
            .objects
            .iter()
            .find(|object| object.artifact_id == artifact.artifact_id)
            .ok_or("an artifact row has no object record")?;
        let path = object
            .path
            .clone()
            .ok_or("an included original has no path")?;
        let bytes = bundle.read_bytes(&path)?;
        assert_eq!(
            encode_hex(ContentDigest::sha256(&bytes).as_bytes().as_slice()),
            artifact.content_digest
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. export_is_deterministic_at_a_fixed_watermark
// ---------------------------------------------------------------------------

/// Two bundles of one committed watermark are byte-identical, file for file.
///
/// **What stops this being vacuous.** The two runs read the source twice —
/// `source_view` opens the database again — so the comparison exercises the
/// read path as well as the writer, and the assertion is over **every** file
/// including `manifest.json`, not over a digest one of them computed. The
/// second half then moves the one value that is allowed to differ and requires
/// the semantic digest to stay put, so "identical" is not being obtained by
/// having nothing that varies.
#[test]
fn export_is_deterministic_at_a_fixed_watermark() -> TestResult {
    let fixture = Fixture::new("deterministic")?;
    let graduation = Graduation::baseline()?;
    let terms = support::terms()?;
    let posture = support::posture();

    let first_view = fixture.source_view()?;
    let first = fixture.work_path("bundle-a");
    write_bundle(
        &BundleRequest {
            source: &first_view,
            posture: &posture,
            terms: &terms,
            originals: WITH_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS,
        },
        &first,
    )?;

    let second_view = fixture.source_view()?;
    let second = fixture.work_path("bundle-b");
    write_bundle(
        &BundleRequest {
            source: &second_view,
            posture: &posture,
            terms: &terms,
            originals: WITH_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS,
        },
        &second,
    )?;

    let left = list_files(&first)?;
    let right = list_files(&second)?;
    assert_eq!(left, right, "the two bundles hold different files");
    assert!(left.len() > 20, "only {} files were compared", left.len());
    assert!(left.iter().any(|path| path == MANIFEST_FILE));
    for relative in &left {
        let a = fs::read(first.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)))?;
        let b = fs::read(second.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)))?;
        assert_eq!(a, b, "{relative} differs between two exports");
    }

    // The recorded instant is the one value outside the digest, and moving it
    // must move the manifest bytes and leave the semantic digest alone.
    let third = fixture.work_path("bundle-c");
    let receipt = write_bundle(
        &BundleRequest {
            source: &second_view,
            posture: &posture,
            terms: &terms,
            originals: WITH_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS + 1,
        },
        &third,
    )?;
    let first_manifest = fs::read(first.join(MANIFEST_FILE))?;
    let third_manifest = fs::read(third.join(MANIFEST_FILE))?;
    assert_ne!(first_manifest, third_manifest);
    let first_bundle = read_bundle(&first)?;
    assert_eq!(
        first_bundle.manifest().semantic_digest,
        receipt.manifest.semantic_digest
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. export_carries_labels_restrictions_and_notices
// ---------------------------------------------------------------------------

/// Every file in a published bundle carries section 32.10's three attributes.
///
/// **What stops this being vacuous.** The set walked is the set on disk, read
/// recursively, and it is compared with the recorded inventory in **both**
/// directions. A file the manifest forgot fails as an unlisted file, and a
/// record with no file fails as a dangling one. This is `T177`'s move: not a
/// hand-written pair list, an enumeration of what is really there.
#[test]
fn export_carries_labels_restrictions_and_notices() -> TestResult {
    let fixture = Fixture::new("labels")?;
    let graduation = Graduation::baseline()?;
    let view = fixture.source_view()?;
    let terms = support::terms()?;
    let posture = support::posture();
    let destination = fixture.work_path("bundle");
    write_bundle(
        &BundleRequest {
            source: &view,
            posture: &posture,
            terms: &terms,
            originals: WITH_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS,
        },
        &destination,
    )?;
    let bundle = read_bundle(&destination)?;

    let on_disk = list_files(&destination)?;
    assert!(on_disk.len() > 20, "only {} files exist", on_disk.len());

    let mut covered: BTreeSet<String> = BTreeSet::new();
    for relative in &on_disk {
        if relative == MANIFEST_FILE {
            let attributes = &bundle.manifest().semantic.manifest_attributes;
            assert_eq!(
                attributes.sharing_restriction,
                SharingRestriction::of(attributes.sensitivity)
            );
            assert!(!attributes.copyright_notice.as_str().trim().is_empty());
            covered.insert(relative.clone());
            continue;
        }
        let record = bundle.file(relative)?;
        assert_eq!(
            record.sharing_restriction(),
            SharingRestriction::of(record.sensitivity()),
            "{relative} carries a restriction its label does not produce"
        );
        assert!(
            !record.copyright_notice().as_str().trim().is_empty(),
            "{relative} carries no source copyright notice"
        );
        let bytes =
            fs::read(destination.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)))?;
        assert_eq!(
            encode_hex(ContentDigest::sha256(&bytes).as_bytes().as_slice()),
            record.sha256()
        );
        covered.insert(relative.clone());
    }

    // Both directions: every file is covered, and every record has a file.
    let recorded: BTreeSet<String> = bundle
        .manifest()
        .semantic
        .files
        .iter()
        .map(|file| file.path().to_owned())
        .chain(std::iter::once(MANIFEST_FILE.to_owned()))
        .collect();
    assert_eq!(covered, recorded);
    assert_eq!(covered.len(), on_disk.len());

    // A file carrying one security domain's rows carries that domain's notice
    // and its recorded label, so the attributes are the domain's rather than a
    // constant the writer repeats.
    let domain = support::domain_id()?.to_string();
    let domain_files: Vec<&str> = bundle
        .manifest()
        .semantic
        .files
        .iter()
        .filter(|file| file.path().contains(&domain))
        .map(|file| file.path())
        .collect();
    assert!(
        domain_files.len() >= 8,
        "only {} per-domain files were found",
        domain_files.len()
    );
    for path in domain_files {
        let record = bundle.file(path)?;
        assert_eq!(record.copyright_notice().as_str(), DOMAIN_NOTICE, "{path}");
        assert_eq!(record.sensitivity(), SensitivityLabel::Restricted, "{path}");
        assert_eq!(
            record.sharing_restriction(),
            SharingRestriction::NoRedistributionWithoutSourcePermission,
            "{path}"
        );
    }

    // Two security domains, two labels, two restrictions and two notices. A
    // corpus with one domain would have let a writer that ignored the register
    // and repeated one value everywhere pass every assertion above.
    let second = support::second_domain_id()?.to_string();
    let mut first_seen = 0_usize;
    let mut second_seen = 0_usize;
    for record in &bundle.manifest().semantic.files {
        if record.path().contains(&second) {
            assert_eq!(
                record.sensitivity(),
                SensitivityLabel::Personal,
                "{}",
                record.path()
            );
            assert_eq!(
                record.sharing_restriction(),
                SharingRestriction::PersonalUseOnly,
                "{}",
                record.path()
            );
            assert_eq!(
                record.copyright_notice().as_str(),
                support::SECOND_DOMAIN_NOTICE,
                "{}",
                record.path()
            );
            second_seen += 1;
        } else if record.path().contains(&domain) {
            assert_eq!(
                record.sensitivity(),
                SensitivityLabel::Restricted,
                "{}",
                record.path()
            );
            first_seen += 1;
        }
    }
    assert!(
        first_seen >= 8,
        "only {first_seen} first-domain files were seen"
    );
    assert!(
        second_seen >= 4,
        "only {second_seen} second-domain files were seen"
    );

    // A recorded label weaker than the ledger is refused, so the label a
    // recipient reads cannot be the declaration while the record says
    // something stronger.
    let understated = support::terms_understating_the_domain()?;
    assert!(
        write_bundle(
            &BundleRequest {
                source: &view,
                posture: &posture,
                terms: &understated,
                originals: WITH_ORIGINALS,
                audit: graduation.recorded(),
                generated_at_unix_ms: GENERATED_AT_UNIX_MS,
            },
            &fixture.work_path("understated"),
        )
        .is_err(),
        "a domain holding RESTRICTED artifacts was exported as PERSONAL"
    );

    // A file with no domain carries the bundle's own notice, which is a
    // different string, so "every file has a notice" is not one notice
    // everywhere.
    let marker = bundle.file(FORMAT_MARKER_FILE)?;
    assert_ne!(marker.copyright_notice().as_str(), DOMAIN_NOTICE);
    assert_eq!(marker.sensitivity(), SensitivityLabel::Public);

    // An unlisted file is refused rather than ignored.
    fs::write(destination.join("stray.txt"), b"unlisted\n")?;
    let refused = read_bundle(&destination);
    assert!(
        refused.is_err(),
        "a bundle with an unlisted file was accepted"
    );
    fs::remove_file(destination.join("stray.txt"))?;
    read_bundle(&destination)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. original_inclusion_is_user_selected_with_no_dangling_locator
// ---------------------------------------------------------------------------

/// Both choices produce a readable bundle, and neither leaves a reference to a
/// file the bundle does not carry.
///
/// **What stops this being vacuous.** Two of the fixture's artifacts hold
/// identical bytes under one media type in one security domain, so the vault
/// derives **one** locator for both. The included case requires both to survive
/// at two distinct paths with two distinct identifiers — which is exactly what
/// a bundle keyed by locator would lose, the shape `P2-A1` found as a P1. The
/// withheld case then requires no `originals/` file to exist at all and every
/// record to state a reason, and a hand-made dangling reference is required to
/// be refused.
#[test]
fn original_inclusion_is_user_selected_with_no_dangling_locator() -> TestResult {
    let fixture = Fixture::new("originals")?;
    let graduation = Graduation::baseline()?;
    let view = fixture.source_view()?;
    let terms = support::terms()?;
    let posture = support::posture();

    // The corpus really does hold the collision this test is about.
    let rows = fixture.canonical_rows()?;
    let mut locators: Vec<&str> = rows
        .artifacts
        .iter()
        .map(|artifact| artifact.vault_locator.as_str())
        .collect();
    locators.sort_unstable();
    let distinct: BTreeSet<&&str> = locators.iter().collect();
    assert!(
        distinct.len() < locators.len(),
        "the corpus holds no two artifacts sharing a vault locator, so this test would \
         pass over a bundle keyed by locator"
    );

    let included = fixture.work_path("with-originals");
    write_bundle(
        &BundleRequest {
            source: &view,
            posture: &posture,
            terms: &terms,
            originals: WITH_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS,
        },
        &included,
    )?;
    let bundle = read_bundle(&included)?;
    assert!(bundle.originals_included());

    let mut paths: Vec<String> = Vec::new();
    for object in &bundle.manifest().semantic.objects {
        let path = object
            .path
            .clone()
            .ok_or("an object in an including bundle has no path")?;
        assert!(object.withheld.is_none());
        assert!(
            path.contains(&object.artifact_id),
            "{path} does not address the artifact by its own identifier"
        );
        assert!(
            !path.contains(&object.vault_locator),
            "{path} addresses an original by its vault locator"
        );
        let bytes = bundle.read_bytes(&path)?;
        assert_eq!(
            encode_hex(ContentDigest::sha256(&bytes).as_bytes().as_slice()),
            object.plaintext_sha256
        );
        paths.push(path);
    }
    let distinct_paths: BTreeSet<&String> = paths.iter().collect();
    assert_eq!(
        distinct_paths.len(),
        paths.len(),
        "two artifacts were published at one path"
    );
    assert_eq!(paths.len(), rows.artifacts.len());

    // The two sharing a locator are both here, with the same bytes.
    let colliding: Vec<&academic_export::bundle::ObjectRecord> = bundle
        .manifest()
        .semantic
        .objects
        .iter()
        .filter(|object| {
            bundle.manifest().semantic.objects.iter().any(|other| {
                other.artifact_id != object.artifact_id
                    && other.vault_locator == object.vault_locator
            })
        })
        .collect();
    assert_eq!(colliding.len(), 2, "the locator collision did not survive");
    assert_eq!(colliding[0].plaintext_sha256, colliding[1].plaintext_sha256);
    assert_ne!(colliding[0].artifact_id, colliding[1].artifact_id);

    // The withheld choice.
    let withheld = fixture.work_path("without-originals");
    write_bundle(
        &BundleRequest {
            source: &view,
            posture: &posture,
            terms: &terms,
            originals: WITHOUT_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS,
        },
        &withheld,
    )?;
    let lean = read_bundle(&withheld)?;
    assert!(!lean.originals_included());
    assert!(!lean.manifest().semantic.objects.is_empty());
    for object in &lean.manifest().semantic.objects {
        assert!(object.path.is_none(), "a withheld original names a path");
        assert!(object.withheld.is_some());
        assert!(!object.plaintext_sha256.is_empty());
    }
    for path in list_files(&withheld)? {
        assert!(
            !path.contains("/originals/"),
            "{path} exists in a bundle that withheld originals"
        );
    }
    assert!(
        list_files(&withheld)?.len() < list_files(&included)?.len(),
        "withholding originals removed no file"
    );

    // A reference the inventory does not list is refused, and the manifest is
    // **re-sealed** after it is broken.
    //
    // Editing the JSON in place and leaving the recorded digest behind measures
    // the digest check and nothing else: `P1-I10` deleted one of the four
    // audit-path checks and this case still passed, because `read_bundle`
    // refuses on the digest long before it reaches the locator rule. Re-sealing
    // produces a manifest that is internally consistent and names a file the
    // bundle does not carry, which is the only shape that can reach the rule.
    let manifest_path = included.join(MANIFEST_FILE);
    let original = fs::read(&manifest_path)?;
    for (item, break_it) in dangling_cases() {
        let mut manifest = BundleManifest::from_json_bytes(&original)?;
        break_it(&mut manifest.semantic);
        let resealed = BundleManifest::seal(manifest.semantic, GENERATED_AT_UNIX_MS)?;
        resealed.verify_semantic_digest()?;
        fs::write(&manifest_path, resealed.to_json_bytes()?)?;
        let refused = read_bundle(&included);
        assert!(
            refused.is_err(),
            "{item}: a re-sealed manifest naming an absent file was accepted"
        );
        if let Err(error) = refused {
            assert!(
                matches!(error, academic_export::ExportError::DanglingLocator { .. }),
                "{item}: a re-sealed manifest naming an absent file was refused for a
                 different reason: {error}"
            );
        }
    }
    fs::write(&manifest_path, &original)?;
    read_bundle(&included)?;
    Ok(())
}

/// One way of pointing a manifest at a file its inventory does not list.
///
/// Every `require_listed` site in the reader is here: the audit's four paths,
/// a part record's file list, and an object's path. A single case would have
/// left the other two unexecuted.
type DanglingCase = (&'static str, fn(&mut academic_export::BundleSemantic));

fn dangling_cases() -> Vec<DanglingCase> {
    vec![
        ("the audit's first path", |semantic| {
            semantic.audit.frozen_inputs_path =
                "parts/official-record-and-proof/audit/absent.txt".to_owned();
        }),
        ("the audit's last path", |semantic| {
            semantic.audit.proof_tree_path =
                "parts/official-record-and-proof/audit/absent.txt".to_owned();
        }),
        ("a part record's file list", |semantic| {
            if let Some(part) = semantic.parts.first_mut() {
                part.files
                    .push("parts/official-record-and-proof/absent.md".to_owned());
            }
        }),
        ("an included original's path", |semantic| {
            if let Some(object) = semantic.objects.first_mut() {
                object.path =
                    Some("parts/lecture-and-question-archive/originals/absent.bin".to_owned());
            }
        }),
    ]
}

// ---------------------------------------------------------------------------
// 5. clean_offline_restore_reruns_deterministic_audit
// ---------------------------------------------------------------------------

/// The bundle alone, on a machine that no longer holds the profile, reproduces
/// the graduation audit byte for byte.
///
/// **What stops this being vacuous.** The source profile and the Phase 1 export
/// it was built from are **deleted** before the bundle is read, so nothing the
/// re-run reads could have come from them. The engine is then re-run rather than
/// re-read: the selector chooses the rule set again from the recorded scope and
/// the profile decoded out of the frozen inputs. And the last block edits one
/// byte of the frozen inputs and requires the re-run to refuse, so agreement is
/// a measurement and not the absence of a comparison.
#[test]
fn clean_offline_restore_reruns_deterministic_audit() -> TestResult {
    let clean_room = support::TestRoot::new("clean-room")?;
    let destination = clean_room.child("bundle");
    let (rule_sets, recorded_outcome, source_root) = {
        let fixture = Fixture::new("offline-audit")?;
        let graduation = Graduation::baseline()?;
        let view = fixture.source_view()?;
        let terms = support::terms()?;
        let posture = support::posture();
        write_bundle(
            &BundleRequest {
                source: &view,
                posture: &posture,
                terms: &terms,
                originals: WITH_ORIGINALS,
                audit: graduation.recorded(),
                generated_at_unix_ms: GENERATED_AT_UNIX_MS,
            },
            &destination,
        )?;
        let outcome = graduation.audit.outcome().canonical_bytes(
            academic_audit::GRADUATION_ENGINE_ID,
            academic_domain::engines::RuleSetHash::new(ContentDigest::sha256(
                graduation.rules.canonical_text().as_bytes(),
            )),
            graduation.engine_version,
            &graduation.inputs,
        );
        let source_root = fixture.root().to_path_buf();
        assert!(source_root.join("profile").exists());
        assert!(source_root.join("phase1-export").exists());
        (vec![graduation.rules.clone()], outcome, source_root)
    };
    // The profile, its database, its vault and its Phase 1 export are gone:
    // the fixture's whole tree was removed when it left scope above, and the
    // two lines inside that scope observed it existing first, so this is a
    // deletion rather than a path that was never there.
    assert!(
        !source_root.exists(),
        "{} still exists, so the re-run below could still be reading it",
        source_root.display()
    );
    assert!(
        !destination.starts_with(&source_root),
        "the bundle is inside the tree that was deleted"
    );

    let bundle = read_bundle(&destination)?;
    let rerun = rerun_audit(&bundle, &rule_sets)?;
    assert_eq!(rerun.engine_id(), academic_audit::GRADUATION_ENGINE_ID);
    assert_eq!(
        rerun.outcome_sha256(),
        encode_hex(
            ContentDigest::sha256(&recorded_outcome)
                .as_bytes()
                .as_slice()
        ),
        "the re-run produced different bytes from the audit that was recorded"
    );
    assert!(rerun.outcome_byte_length() > 0);
    assert!(
        rerun.selected_scope_text().contains("SNU"),
        "the selector did not re-choose the recorded scope: {}",
        rerun.selected_scope_text()
    );

    // Nothing but the directory was needed, and it is still complete.
    assert_eq!(
        bundle.manifest().semantic.audit.rule_set_hash,
        hex_lower(
            ContentDigest::sha256(rule_sets[0].canonical_text().as_bytes())
                .as_bytes()
                .as_slice()
        )
    );

    // A frozen input the bundle no longer carries faithfully is refused.
    let audit_path = bundle.manifest().semantic.audit.frozen_inputs_path.clone();
    let full = destination.join(audit_path.replace('/', std::path::MAIN_SEPARATOR_STR));
    let original = fs::read_to_string(&full)?;
    let tampered = original.replacen("=int:", "=int:9", 1);
    assert_ne!(tampered, original, "no integer input was found to move");
    fs::write(&full, &tampered)?;
    assert!(
        read_bundle(&destination).is_err(),
        "an edited frozen input passed the file digest check"
    );
    fs::write(&full, &original)?;
    let restored = read_bundle(&destination)?;
    rerun_audit(&restored, &rule_sets)?;

    // A caller who no longer holds the published rules is told so rather than
    // given a verdict under different ones.
    assert!(
        rerun_audit(&restored, &[]).is_err(),
        "an audit was reproduced with no published rule set"
    );

    // And the byte comparison is load-bearing rather than decorative. This
    // bundle is internally consistent -- every file hashes as recorded, the
    // reader accepts it whole -- and its recorded outcome was produced from a
    // different transcript than the frozen inputs beside it. Nothing but the
    // re-run can notice, so a re-run that only re-read would pass here.
    let mismatched_room = support::TestRoot::new("mismatched-audit")?;
    let mismatched = mismatched_room.child("bundle");
    {
        let fixture = Fixture::new("mismatched-source")?;
        let graduation = Graduation::baseline()?;
        let other = graduation.with_other_inputs()?;
        let view = fixture.source_view()?;
        let terms = support::terms()?;
        let posture = support::posture();
        let mut recorded = graduation.recorded();
        recorded.audit = &other.audit;
        assert_ne!(
            other.audit.outcome().canonical_bytes(
                academic_audit::GRADUATION_ENGINE_ID,
                academic_domain::engines::RuleSetHash::new(ContentDigest::sha256(
                    graduation.rules.canonical_text().as_bytes(),
                )),
                graduation.engine_version,
                &graduation.inputs,
            ),
            recorded_outcome,
            "the alternate corpus reaches the same outcome, so this case measures nothing"
        );
        write_bundle(
            &BundleRequest {
                source: &view,
                posture: &posture,
                terms: &terms,
                originals: WITH_ORIGINALS,
                audit: recorded,
                generated_at_unix_ms: GENERATED_AT_UNIX_MS,
            },
            &mismatched,
        )?;
    }
    let readable = read_bundle(&mismatched)?;
    let refused = rerun_audit(&readable, &rule_sets);
    assert!(
        refused.is_err(),
        "a bundle whose recorded outcome belongs to other inputs was reproduced"
    );
    // The **arm** matters, not only the refusal. `P1-I8` weakened the byte
    // comparison to a length comparison and this case still failed, because the
    // recorded-digest check two lines further down caught the same thing: a
    // second guard was masking the first. Matching the arm tells them apart.
    if let Err(error) = refused {
        assert!(
            matches!(
                error,
                academic_export::ExportError::AuditNotReproduced(_)
                    | academic_export::ExportError::Mismatch { .. }
            ),
            "the re-run refused for a different reason: {error}"
        );
    }

    // And the byte comparison is the only thing that can catch **this** one.
    // A bundle whose recorded audit is correct, with the recorded outcome
    // edited by one byte and no more, that file's digest corrected, and the
    // manifest re-sealed. Every other check in the re-run then agrees: the
    // lengths match, the digest of the reproduced bytes matches what the audit
    // record holds, and the reader accepts the directory whole. `P1-I8`
    // weakened the comparison to a length comparison and passed every case
    // before this one for exactly that reason.
    let one_byte_room = support::TestRoot::new("one-byte")?;
    let one_byte_bundle = one_byte_room.child("bundle");
    {
        let fixture = Fixture::new("one-byte-source")?;
        let graduation = Graduation::baseline()?;
        let view = fixture.source_view()?;
        let terms = support::terms()?;
        let posture = support::posture();
        write_bundle(
            &BundleRequest {
                source: &view,
                posture: &posture,
                terms: &terms,
                originals: WITH_ORIGINALS,
                audit: graduation.recorded(),
                generated_at_unix_ms: GENERATED_AT_UNIX_MS,
            },
            &one_byte_bundle,
        )?;
    }
    let intact = read_bundle(&one_byte_bundle)?;
    rerun_audit(&intact, &rule_sets)?;

    let outcome_path = intact.manifest().semantic.audit.outcome_path.clone();
    let native = outcome_path.replace('/', std::path::MAIN_SEPARATOR_STR);
    let mut bytes = fs::read(one_byte_bundle.join(&native))?;
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    fs::write(one_byte_bundle.join(&native), &bytes)?;

    let manifest_bytes = fs::read(one_byte_bundle.join(MANIFEST_FILE))?;
    let mut semantic = BundleManifest::from_json_bytes(&manifest_bytes)?.semantic;
    let corrected = encode_hex(ContentDigest::sha256(&bytes).as_bytes().as_slice());
    semantic.files = semantic
        .files
        .into_iter()
        .map(|record| {
            if record.path() == outcome_path {
                academic_export::FileRecord::new(
                    record.path(),
                    bytes.len() as u64,
                    corrected.clone(),
                    record.sensitivity(),
                    record.copyright_notice().clone(),
                )
            } else {
                record
            }
        })
        .collect();
    let resealed = BundleManifest::seal(semantic, GENERATED_AT_UNIX_MS)?;
    fs::write(
        one_byte_bundle.join(MANIFEST_FILE),
        resealed.to_json_bytes()?,
    )?;

    let accepted = read_bundle(&one_byte_bundle)?;
    assert_eq!(
        accepted.read_bytes(&outcome_path)?.len(),
        bytes.len(),
        "the edited outcome changed length, so a length comparison would catch it"
    );
    let one_byte = rerun_audit(&accepted, &rule_sets);
    assert!(
        one_byte.is_err(),
        "a recorded outcome differing from the engine's by one byte was reproduced"
    );
    if let Err(error) = one_byte {
        assert!(
            matches!(
                error,
                academic_export::ExportError::AuditNotReproduced(
                    "the re-run produced different outcome bytes"
                )
            ),
            "the one-byte case refused for a different reason: {error}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. restore_without_vendor_or_school_account_succeeds
// ---------------------------------------------------------------------------

/// The restore path needs no vendor endpoint, no provider, and no school
/// account, whichever choice the user made about originals.
///
/// **What stops this being vacuous.** The behavioural half runs from a
/// directory whose producing profile has been deleted and whose reader is
/// handed no credential of any kind — there is no parameter to pass one as. The
/// source half is `export_scans.rs`, which reads this crate's own product files
/// and its declared closure as whole sets: neither half alone would catch a
/// dependency that reached a network from inside a call the behaviour test
/// happens not to exercise.
#[test]
fn restore_without_vendor_or_school_account_succeeds() -> TestResult {
    for originals in OriginalInclusion::ALL {
        let clean_room = support::TestRoot::new("no-vendor")?;
        let destination = clean_room.child("bundle");
        let rules = {
            let fixture = Fixture::new("no-vendor-source")?;
            let graduation = Graduation::baseline()?;
            let view = fixture.source_view()?;
            let terms = support::terms()?;
            let posture = support::posture();
            write_bundle(
                &BundleRequest {
                    source: &view,
                    posture: &posture,
                    terms: &terms,
                    originals,
                    audit: graduation.recorded(),
                    generated_at_unix_ms: GENERATED_AT_UNIX_MS,
                },
                &destination,
            )?;
            vec![graduation.rules.clone()]
        };

        // Everything the product would have provided is gone: no profile, no
        // vault, no key, no device authorization, no connector.
        let bundle = read_bundle(&destination)?;
        assert_eq!(bundle.originals_included(), originals.includes_originals());
        let rerun = rerun_audit(&bundle, &rules)?;
        assert!(!rerun.rule_set_hash().is_empty());

        // Every part is still readable from the bytes alone.
        for part in bundle.parts() {
            for path in &part.files {
                let bytes = bundle.read_bytes(path)?;
                assert!(
                    !bytes.is_empty() || path.ends_with(".jsonl"),
                    "{path} is empty"
                );
            }
        }

        // The posture a reader is told about is still the refusing one.
        assert!(!bundle.manifest().semantic.policy.production_data_allowed);
        assert!(!bundle.manifest().semantic.projections_included);
        assert!(!bundle.manifest().semantic.encrypted);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. graduation_bundle_contains_all_six_named_parts
// ---------------------------------------------------------------------------

/// The bundle's parts are exactly the ones section 37 lists, and each carries
/// content.
///
/// **What stops this being vacuous.** The six are parsed **out of the
/// specification** and compared with the crate's enumeration in both
/// directions, so nothing here asserts the number six: a bullet renamed,
/// dropped or added fails. Each part is then required to carry a content file
/// besides its own `part.json`, and the sixth is required to carry the
/// canonical state whole rather than a selection of it.
#[test]
fn graduation_bundle_contains_all_six_named_parts() -> TestResult {
    let specified = specification_export_list();
    let declared: Vec<&str> = BundlePart::ALL
        .iter()
        .map(|part| part.specification_sentence())
        .collect();
    assert_eq!(
        specified, declared,
        "section 37's export list and BundlePart::ALL disagree"
    );
    assert_eq!(
        specified.len(),
        6,
        "section 37 lists {} export items, not six",
        specified.len()
    );

    let fixture = Fixture::new("six-parts")?;
    let graduation = Graduation::baseline()?;
    let view = fixture.source_view()?;
    let terms = support::terms()?;
    let posture = support::posture();
    let destination = fixture.work_path("bundle");
    write_bundle(
        &BundleRequest {
            source: &view,
            posture: &posture,
            terms: &terms,
            originals: WITH_ORIGINALS,
            audit: graduation.recorded(),
            generated_at_unix_ms: GENERATED_AT_UNIX_MS,
        },
        &destination,
    )?;
    let bundle = read_bundle(&destination)?;

    assert_eq!(bundle.parts().len(), BundlePart::ALL.len());
    for (part, record) in BundlePart::ALL.into_iter().zip(bundle.parts()) {
        assert_eq!(record.part, part.as_str());
        assert_eq!(record.directory, part.directory());
        assert_eq!(record.specification_sentence, part.specification_sentence());
        let content: Vec<&String> = record
            .files
            .iter()
            .filter(|path| !path.ends_with("/part.json"))
            .collect();
        assert!(
            !content.is_empty(),
            "{} carries only its own part record",
            part.as_str()
        );
        for path in &record.files {
            assert!(
                path.starts_with(&format!("parts/{}/", part.directory())),
                "{path} is listed under the wrong part"
            );
            bundle.read_bytes(path)?;
        }
    }

    // Part six is not a selection: it carries the canonical state whole.
    let rows = fixture.canonical_rows()?;
    let counts = bundle.manifest().semantic.counts;
    assert_eq!(counts.batches, rows.batches.len() as u64);
    assert_eq!(counts.events, rows.events.len() as u64);
    assert_eq!(counts.scopes, rows.scopes.len() as u64);
    assert_eq!(counts.artifacts, rows.artifacts.len() as u64);
    assert_eq!(counts.evidence, rows.evidence.len() as u64);
    assert_eq!(counts.claims, rows.claims.len() as u64);
    assert_eq!(counts.relations, rows.relations.len() as u64);
    assert_eq!(counts.decisions, rows.decisions.len() as u64);

    // Every claim reaches the sixth part, including the one whose predicate no
    // section 37 topic names, and across every security domain.
    let all_claims = read_stream(
        &bundle,
        &format!(
            "parts/{}/canonical/claims/",
            BundlePart::MachineReadableGraph.directory()
        ),
    )?;
    assert_eq!(all_claims.len(), rows.claims.len());
    let untopical = rows
        .claims
        .iter()
        .filter(|claim| BundlePart::for_predicate(&claim.predicate_id).is_none())
        .count();
    assert!(
        untopical > 0,
        "every claim in the corpus names a section 37 topic, so the sixth part's \
         totality is not being measured"
    );

    // Section 32.10 names a format this build does not write, and the bundle
    // says so instead of shipping an empty one.
    for path in list_files(&destination)? {
        assert!(
            !path.ends_with(".pdf"),
            "{path} claims a format this build cannot write"
        );
    }
    let formats = String::from_utf8(bundle.read_bytes(&format!(
        "parts/{}/formats.md",
        BundlePart::MachineReadableGraph.directory()
    ))?)?;
    assert!(formats.contains(PDF_RENDERING_ABSENCE));
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Section 37's export list, parsed out of the specification.
///
/// The list is the bullets between *졸업 시 사용자는 다음을 export할 수 있다.*
/// and the blank line that ends them. Reading it rather than restating it is
/// what makes the six a measurement.
fn specification_export_list() -> Vec<&'static str> {
    let mut items = Vec::new();
    let mut inside = false;
    for line in SPECIFICATION.lines() {
        let line = line.trim_end_matches('\r');
        if line.contains("졸업 시 사용자는 다음을 export할 수 있다.") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(item) = line.strip_prefix("- ") {
            items.push(item);
        } else if !items.is_empty() {
            break;
        }
    }
    items
}

/// Every record of one canonical stream, across every security domain.
///
/// The files are enumerated out of the manifest rather than named, so a domain
/// the corpus gains is read without this helper changing.
fn read_stream(bundle: &academic_export::ClaimedBundle, prefix: &str) -> TestResult<Vec<String>> {
    let mut lines = Vec::new();
    let mut files = 0_usize;
    for record in &bundle.manifest().semantic.files {
        if !record.path().starts_with(prefix) {
            continue;
        }
        files += 1;
        let text = String::from_utf8(bundle.read_bytes(record.path())?)?;
        lines.extend(text.lines().map(str::to_owned));
    }
    assert!(files > 0, "no file in the bundle sits under {prefix}");
    lines.sort();
    Ok(lines)
}

fn sorted(mut lines: Vec<String>) -> Vec<String> {
    lines.sort();
    lines
}

fn expected_lines<T: serde::Serialize>(rows: &[T]) -> TestResult<Vec<String>> {
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        lines.push(String::from_utf8(
            academic_portability::verify::canonical_json(row)?,
        )?);
    }
    Ok(lines)
}

/// Guards the helper above: a comparison over an empty list proves nothing.
#[test]
fn the_specification_parser_finds_a_list() {
    let items = specification_export_list();
    assert!(!items.is_empty());
    assert!(items.iter().all(|item| !item.trim().is_empty()));
    let _ = Path::new(".");
}
