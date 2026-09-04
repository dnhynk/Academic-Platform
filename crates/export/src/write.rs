//! Writing one graduation bundle.
//!
//! # The clock is a parameter
//!
//! Nothing here reads a clock, an environment variable, a host name or a
//! random number. `generated_at_unix_ms` is a field of the request, so two
//! bundles of one watermark are byte-identical **whole-file**, manifest
//! included, rather than identical except for one integer nobody can compare.
//! The only ambient value the writer touches is the staging directory's own
//! name, which is removed by the publish rename and is never inside the bundle.
//!
//! # Content files are written per security domain
//!
//! A file carries one sensitivity label, one sharing restriction and one source
//! copyright notice, so it may draw on one security domain. Rows are partitioned
//! by the domain they belong to and each partition becomes its own file, which
//! is why a bundle has `canonical/<domain>/claims.jsonl` rather than one
//! `claims.jsonl` whose terms would have to be a compromise between two sets of
//! terms nobody reconciled.
//!
//! # The originals question is answered before anything is written
//!
//! [`crate::OriginalInclusion`] is taken by value and consulted once, and the
//! two branches produce different manifests: a carried original has a path and
//! no reason, a withheld one has a reason and no path. There is no third state
//! where a record names a file the directory does not hold.

use std::path::{Path, PathBuf};

use academic_audit::{DegreeAudit, GRADUATION_ENGINE_ID, RuleSetScope};
use academic_domain::{
    ContentDigest,
    engines::{EngineVersion, FrozenInputs},
};
use academic_requirement::RuleSet;

use crate::{
    BUNDLE_ENCRYPTED, BUNDLE_MANIFEST_SCHEMA, ExportError, ExportResult, FORMAT_MARKER_BYTES,
    FORMAT_MARKER_FILE, GRADUATION_EXPORT_FORMAT, GRADUATION_EXPORT_GENERATOR,
    GRADUATION_EXPORT_MANIFEST_VERSION, INVENTORY_FILE, MANIFEST_FILE, MANIFEST_SCHEMA_FILE,
    OPEN_FORMATS, PARTS_DIRECTORY, PDF_RENDERING_ABSENCE, PROJECTIONS_INCLUDED,
    audit::{
        AuditRecord, CatalogScopeRecord, FROZEN_INPUTS_FILE, OUTCOME_FILE, PROOF_TREE_FILE,
        RULE_SET_FILE,
    },
    bundle::{
        BundleCounts, BundleManifest, BundleSemantic, FileRecord, ManifestAttributes, ObjectRecord,
        PartRecord, PostureBlock, encode_hex,
    },
    directory, graph,
    label::{SensitivityLabel, SharingRestriction, TermsRegister},
    part::BundlePart,
    source::{
        ArtifactSource, ClaimSource, DomainRecord, OriginalInclusion, SourceView, WithheldReason,
    },
};

/// The graduation audit the caller already computed, as the writer records it.
///
/// The writer does not evaluate the engine: it records what an evaluation
/// produced, and [`crate::rerun_audit`] is the half that evaluates. Keeping the
/// two apart is what makes the re-run evidence — a writer that computed the
/// expected bytes and a reader that recomputed them with the same call would
/// have compared one function with itself.
#[derive(Debug, Clone, Copy)]
pub struct RecordedAudit<'a> {
    /// The engine version the audit ran at.
    pub engine_version: EngineVersion,
    /// The frozen inputs it ran over.
    pub inputs: &'a FrozenInputs,
    /// The published rule set it ran under.
    pub rules: &'a RuleSet,
    /// The scope that rule set declares, which the selector matched.
    pub scope: &'a RuleSetScope,
    /// The audit itself.
    pub audit: &'a DegreeAudit,
}

/// Everything one bundle is written from.
#[derive(Debug, Clone, Copy)]
pub struct BundleRequest<'a> {
    /// The canonical state of one committed watermark.
    pub source: &'a SourceView,
    /// The posture the source profile was under.
    pub posture: &'a PostureBlock,
    /// The label and notice covering each security domain.
    pub terms: &'a TermsRegister,
    /// Whether original bytes travel. The user's choice, with no default.
    pub originals: OriginalInclusion,
    /// The graduation audit to record.
    pub audit: RecordedAudit<'a>,
    /// The instant the caller records for this generation.
    pub generated_at_unix_ms: i64,
}

/// One published bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleReceipt {
    /// Where it was published.
    pub destination: PathBuf,
    /// Its manifest.
    pub manifest: BundleManifest,
}

/// Writes one graduation bundle into an absent destination.
///
/// Everything is built in a sibling staging directory and published with one
/// rename, so an interrupted write leaves an unpublished staging root and no
/// partial destination.
pub fn write_bundle(
    request: &BundleRequest<'_>,
    destination: &Path,
) -> ExportResult<BundleReceipt> {
    directory::require_absent(destination)?;
    request.source.validate()?;
    request.posture.require_phase2_posture()?;
    let domains = require_terms_for_every_domain(request)?;

    let staging = directory::reserve_staging_path(destination)?;
    directory::create_new_directory(&staging)?;
    let built = build(request, &staging, &domains);
    let receipt = match built {
        Ok(manifest) => manifest,
        Err(error) => {
            directory::remove_staging(&staging)?;
            return Err(error);
        }
    };
    directory::sync_tree(&staging)?;
    directory::publish(&staging, destination)?;
    Ok(BundleReceipt {
        destination: destination.to_path_buf(),
        manifest: receipt,
    })
}

/// Refuses a source whose domains the register does not cover.
///
/// Also refuses a recorded label weaker than what the ledger says: a domain
/// holding a `SECRET` artifact cannot be declared `PERSONAL`, because the
/// declaration would then be the thing a recipient reads and the record would
/// be the thing nobody does.
fn require_terms_for_every_domain(request: &BundleRequest<'_>) -> ExportResult<Vec<String>> {
    let domains = request.source.domains();
    for domain in &domains {
        let terms = request.terms.for_domain(domain)?;
        let observed = request.source.domain_label(domain);
        if terms.sensitivity().rank() < observed.rank() {
            return Err(ExportError::mismatch(
                "recorded domain sensitivity",
                format!("at least {}", observed.as_str()),
                terms.sensitivity().as_str(),
            ));
        }
    }
    Ok(domains)
}

/// One file the writer has produced, with everything the manifest needs.
struct WrittenFile {
    path: String,
    byte_length: u64,
    sha256: String,
    sensitivity: SensitivityLabel,
}

/// Accumulates written files so the manifest is a report of what happened.
struct Writer<'a> {
    staging: &'a Path,
    files: Vec<WrittenFile>,
}

impl<'a> Writer<'a> {
    const fn new(staging: &'a Path) -> Self {
        Self {
            staging,
            files: Vec::new(),
        }
    }

    fn write(
        &mut self,
        relative: &str,
        bytes: &[u8],
        sensitivity: SensitivityLabel,
    ) -> ExportResult<()> {
        directory::write_new_file(self.staging, relative, bytes)?;
        self.files.push(WrittenFile {
            path: relative.to_owned(),
            byte_length: bytes.len() as u64,
            sha256: encode_hex(ContentDigest::sha256(bytes).as_bytes().as_slice()),
            sensitivity,
        });
        Ok(())
    }

    fn copy(
        &mut self,
        relative: &str,
        source_path: &Path,
        sensitivity: SensitivityLabel,
    ) -> ExportResult<(ContentDigest, u64)> {
        let (digest, byte_length) = directory::copy_new_file(self.staging, relative, source_path)?;
        self.files.push(WrittenFile {
            path: relative.to_owned(),
            byte_length,
            sha256: encode_hex(digest.as_bytes().as_slice()),
            sensitivity,
        });
        Ok((digest, byte_length))
    }

    fn paths_below(&self, prefix: &str) -> Vec<String> {
        let mut paths: Vec<String> = self
            .files
            .iter()
            .filter(|file| file.path.starts_with(prefix))
            .map(|file| file.path.clone())
            .collect();
        paths.sort();
        paths
    }

    fn label_of(&self, prefix: &str) -> SensitivityLabel {
        SensitivityLabel::strongest_of(
            self.files
                .iter()
                .filter(|file| file.path.starts_with(prefix))
                .map(|file| file.sensitivity),
        )
    }
}

fn build(
    request: &BundleRequest<'_>,
    staging: &Path,
    domains: &[String],
) -> ExportResult<BundleManifest> {
    let source = request.source;
    let mut writer = Writer::new(staging);

    // The marker and the embedded schema describe no artifact and are this
    // repository's own constants, so they are `PUBLIC` and carry the bundle's
    // own notice. Every other generated file is labelled by what it describes.
    writer.write(
        FORMAT_MARKER_FILE,
        FORMAT_MARKER_BYTES.as_bytes(),
        SensitivityLabel::Public,
    )?;
    writer.write(
        MANIFEST_SCHEMA_FILE,
        BUNDLE_MANIFEST_SCHEMA.as_bytes(),
        SensitivityLabel::Public,
    )?;

    let objects = write_part_two(request, &mut writer, domains)?;
    let audit_record = write_part_one(request, &mut writer, domains)?;
    write_topical_part(
        BundlePart::ConceptAndCompetencyEvidence,
        request,
        &mut writer,
        domains,
    )?;
    write_part_four(request, &mut writer, domains)?;
    write_part_five(request, &mut writer, domains)?;
    write_part_six(request, &mut writer, domains)?;

    // Written after every part so each `part.json` reports files that exist.
    let mut parts = Vec::with_capacity(BundlePart::ALL.len());
    for part in BundlePart::ALL {
        let prefix = format!("{PARTS_DIRECTORY}/{}/", part.directory());
        let files = writer.paths_below(&prefix);
        let label = writer.label_of(&prefix);
        let record = PartRecord {
            part: part.as_str().to_owned(),
            directory: part.directory().to_owned(),
            specification_sentence: part.specification_sentence().to_owned(),
            files,
        };
        let mut bytes = serde_json::to_vec_pretty(&record).map_err(|source| ExportError::Json {
            operation: "render part record",
            source,
        })?;
        bytes.push(b'\n');
        let relative = format!("{prefix}part.json");
        writer.write(&relative, &bytes, label)?;
        let mut published = record;
        published.files.push(relative);
        published.files.sort();
        parts.push(published);
    }

    let inventory = inventory_text(request, &parts, &objects);
    let content_label =
        SensitivityLabel::strongest_of(writer.files.iter().map(|file| file.sensitivity));
    writer.write(INVENTORY_FILE, inventory.as_bytes(), content_label)?;

    let mut files: Vec<FileRecord> = Vec::with_capacity(writer.files.len());
    for written in &writer.files {
        directory::check_relative_path(&written.path)?;
        let notice = notice_for_path(request, &written.path)?;
        files.push(FileRecord::new(
            written.path.clone(),
            written.byte_length,
            written.sha256.clone(),
            written.sensitivity,
            notice,
        ));
    }
    files.sort_by(|left, right| left.path().cmp(right.path()));

    let manifest_label = SensitivityLabel::strongest_of(files.iter().map(FileRecord::sensitivity));
    let semantic = BundleSemantic {
        format: GRADUATION_EXPORT_FORMAT.to_owned(),
        manifest_version: GRADUATION_EXPORT_MANIFEST_VERSION,
        generator: GRADUATION_EXPORT_GENERATOR.to_owned(),
        policy: request.posture.clone(),
        encrypted: BUNDLE_ENCRYPTED,
        projections_included: PROJECTIONS_INCLUDED,
        originals_included: request.originals.includes_originals(),
        store: source.store.clone(),
        watermark: source.watermark,
        device_heads: source.device_heads.clone(),
        canonical_semantic_digest: source.canonical_semantic_digest.clone(),
        counts: BundleCounts {
            batches: source.batches.len() as u64,
            events: source.events.len() as u64,
            scopes: source.scopes.len() as u64,
            artifacts: source.artifacts.len() as u64,
            evidence: source.evidence.len() as u64,
            claims: source.claims.len() as u64,
            relations: source.relations.len() as u64,
            decisions: source.decisions.len() as u64,
        },
        parts,
        objects,
        git_refs: source.git_refs.clone(),
        audit: audit_record,
        manifest_attributes: ManifestAttributes {
            sensitivity: manifest_label,
            sharing_restriction: SharingRestriction::of(manifest_label),
            copyright_notice: request.terms.bundle_notice().clone(),
        },
        files,
    };
    let manifest = BundleManifest::seal(semantic, request.generated_at_unix_ms)?;
    manifest.require_v2_contract()?;
    directory::write_new_file(staging, MANIFEST_FILE, &manifest.to_json_bytes()?)?;
    Ok(manifest)
}

/// Part 1: 공식 성적/요건과 계산 proof.
fn write_part_one(
    request: &BundleRequest<'_>,
    writer: &mut Writer<'_>,
    domains: &[String],
) -> ExportResult<AuditRecord> {
    let part = BundlePart::OfficialRecordAndProof;
    write_part_claims(part, request, writer, domains)?;

    let recorded = request.audit;
    let prefix = format!("{PARTS_DIRECTORY}/{}/", part.directory());
    let rule_set_text = recorded.rules.canonical_text();
    let rule_set_digest = ContentDigest::sha256(rule_set_text.as_bytes());
    let frozen_text = recorded.inputs.canonical_text();
    let outcome = recorded.audit.outcome().canonical_bytes(
        GRADUATION_ENGINE_ID,
        academic_domain::engines::RuleSetHash::new(rule_set_digest),
        recorded.engine_version,
        recorded.inputs,
    );
    let proof_tree = render_proof_tree(recorded.audit);

    // The audit's own label is the strongest the bundle carries: a proof tree
    // cites the artifacts a verdict rests on, so it is not weaker than they are.
    let label = SensitivityLabel::strongest_of(
        request
            .source
            .artifacts
            .iter()
            .map(ArtifactSource::label)
            .chain(domains.iter().filter_map(|domain| {
                request
                    .terms
                    .for_domain(domain)
                    .ok()
                    .map(|terms| terms.sensitivity())
            })),
    );

    let frozen_path = format!("{prefix}{FROZEN_INPUTS_FILE}");
    let rule_set_path = format!("{prefix}{RULE_SET_FILE}");
    let outcome_path = format!("{prefix}{OUTCOME_FILE}");
    let proof_tree_path = format!("{prefix}{PROOF_TREE_FILE}");
    writer.write(&frozen_path, frozen_text.as_bytes(), label)?;
    writer.write(&rule_set_path, rule_set_text.as_bytes(), label)?;
    writer.write(&outcome_path, &outcome, label)?;
    writer.write(&proof_tree_path, proof_tree.as_bytes(), label)?;

    let markdown = official_record_markdown(recorded, &rule_set_digest, &proof_tree);
    writer.write(&format!("{prefix}record.md"), markdown.as_bytes(), label)?;

    Ok(AuditRecord {
        engine_id: GRADUATION_ENGINE_ID.to_owned(),
        engine_version: recorded.engine_version.get(),
        rule_set_hash: encode_hex(rule_set_digest.as_bytes().as_slice()),
        rule_set_version: recorded.rules.version().get(),
        frozen_inputs_sha256: encode_hex(
            ContentDigest::sha256(frozen_text.as_bytes())
                .as_bytes()
                .as_slice(),
        ),
        outcome_sha256: encode_hex(ContentDigest::sha256(&outcome).as_bytes().as_slice()),
        selected_scope: CatalogScopeRecord::of(recorded.scope),
        frozen_inputs_path: frozen_path,
        rule_set_path,
        outcome_path,
        proof_tree_path,
    })
}

/// Part 2: 원본을 포함하거나 제외할 수 있는 강의·질문 archive.
fn write_part_two(
    request: &BundleRequest<'_>,
    writer: &mut Writer<'_>,
    domains: &[String],
) -> ExportResult<Vec<ObjectRecord>> {
    let part = BundlePart::LectureAndQuestionArchive;
    write_part_claims(part, request, writer, domains)?;
    let prefix = format!("{PARTS_DIRECTORY}/{}/", part.directory());

    let mut objects = Vec::with_capacity(request.source.artifacts.len());
    for artifact in &request.source.artifacts {
        let terms = request.terms.for_domain(artifact.domain_id())?;
        let label = terms.sensitivity().strongest(artifact.label());
        let (path, withheld) = match request.originals {
            OriginalInclusion::Included => {
                // Addressed by the artifact identifier. Two artifacts with
                // identical bytes share a vault locator, so a path derived from
                // the locator would publish one file for both and lose one.
                let relative = format!(
                    "{prefix}originals/{}/{}.bin",
                    artifact.domain_id(),
                    artifact.artifact_id()
                );
                let (digest, byte_length) =
                    writer.copy(&relative, artifact.original_path(), label)?;
                let observed = encode_hex(digest.as_bytes().as_slice());
                if observed != artifact.content_sha256() || byte_length != artifact.byte_length() {
                    return Err(ExportError::mismatch(
                        "exported original artifact",
                        artifact.content_sha256(),
                        observed,
                    ));
                }
                (Some(relative), None)
            }
            OriginalInclusion::Withheld => (None, Some(WithheldReason::UserExcludedOriginals)),
        };
        let record = ObjectRecord {
            artifact_id: artifact.artifact_id().to_owned(),
            domain_id: artifact.domain_id().to_owned(),
            media_type: artifact.media_type().to_owned(),
            plaintext_sha256: artifact.content_sha256().to_owned(),
            byte_length: artifact.byte_length(),
            vault_locator: artifact.vault_locator().to_owned(),
            sensitivity: label,
            path,
            withheld,
        };
        record.validate()?;
        objects.push(record);
    }

    let label = SensitivityLabel::strongest_of(objects.iter().map(|object| object.sensitivity));
    let markdown = archive_markdown(request, &objects);
    writer.write(&format!("{prefix}archive.md"), markdown.as_bytes(), label)?;
    Ok(objects)
}

/// Part 4: repository snapshot과 architecture evolution.
fn write_part_four(
    request: &BundleRequest<'_>,
    writer: &mut Writer<'_>,
    domains: &[String],
) -> ExportResult<()> {
    let part = BundlePart::RepositorySnapshotAndEvolution;
    write_part_claims(part, request, writer, domains)?;
    let prefix = format!("{PARTS_DIRECTORY}/{}/", part.directory());

    for domain in domains {
        let references: Vec<&crate::source::GitRef> = request
            .source
            .git_refs
            .iter()
            .filter(|reference| reference.domain_id == *domain)
            .collect();
        if references.is_empty() {
            continue;
        }
        let terms = request.terms.for_domain(domain)?;
        let mut bytes = Vec::new();
        for reference in &references {
            let line = serde_json::to_vec(reference).map_err(|source| ExportError::Json {
                operation: "render git reference",
                source,
            })?;
            bytes.extend_from_slice(&line);
            bytes.push(b'\n');
        }
        writer.write(
            &format!("{prefix}git-refs/{domain}.jsonl"),
            &bytes,
            terms.sensitivity(),
        )?;
    }

    let label = writer.label_of(&prefix);
    let markdown = evolution_markdown(request);
    writer.write(&format!("{prefix}evolution.md"), markdown.as_bytes(), label)?;
    Ok(())
}

/// Part 5: role 관심 변화와 alternative paths.
fn write_part_five(
    request: &BundleRequest<'_>,
    writer: &mut Writer<'_>,
    domains: &[String],
) -> ExportResult<()> {
    let part = BundlePart::RoleInterestAndAlternativePaths;
    write_part_claims(part, request, writer, domains)?;
    let prefix = format!("{PARTS_DIRECTORY}/{}/", part.directory());

    // A change of interest is recorded as a decision over a claim, so the part
    // that is about changes carries the decisions as well as the claims.
    write_domain_stream(
        writer,
        request,
        domains,
        &format!("{prefix}decisions"),
        &request.source.decisions,
    )?;

    let label = writer.label_of(&prefix);
    let markdown = paths_markdown(request);
    writer.write(&format!("{prefix}paths.md"), markdown.as_bytes(), label)?;
    Ok(())
}

/// Part 6: machine-readable graph와 open formats.
///
/// Not a selection. This part carries the canonical state of the exported
/// watermark whole, which is what makes the assignment total without inventing
/// a seventh part for the rows no section 37 topic names.
fn write_part_six(
    request: &BundleRequest<'_>,
    writer: &mut Writer<'_>,
    domains: &[String],
) -> ExportResult<()> {
    let part = BundlePart::MachineReadableGraph;
    let prefix = format!("{PARTS_DIRECTORY}/{}/", part.directory());
    let source = request.source;

    let canonical = format!("{prefix}canonical");
    write_domain_stream(
        writer,
        request,
        domains,
        &format!("{canonical}/events"),
        &source.events,
    )?;
    write_domain_stream(
        writer,
        request,
        domains,
        &format!("{canonical}/scopes"),
        &source.scopes,
    )?;
    write_domain_stream(
        writer,
        request,
        domains,
        &format!("{canonical}/evidence"),
        &source.evidence,
    )?;
    write_domain_stream(
        writer,
        request,
        domains,
        &format!("{canonical}/relations"),
        &source.relations,
    )?;
    write_domain_stream(
        writer,
        request,
        domains,
        &format!("{canonical}/decisions"),
        &source.decisions,
    )?;

    for domain in domains {
        let terms = request.terms.for_domain(domain)?;
        let artifacts: Vec<&ArtifactSource> = source
            .artifacts
            .iter()
            .filter(|artifact| artifact.domain_id() == domain)
            .collect();
        if !artifacts.is_empty() {
            let mut bytes = Vec::new();
            for artifact in &artifacts {
                bytes.extend_from_slice(artifact.canonical_json().as_bytes());
                bytes.push(b'\n');
            }
            let label = SensitivityLabel::strongest_of(
                artifacts
                    .iter()
                    .map(|artifact| artifact.label())
                    .chain([terms.sensitivity()]),
            );
            writer.write(
                &format!("{canonical}/artifacts/{domain}.jsonl"),
                &bytes,
                label,
            )?;
        }

        let claims: Vec<&ClaimSource> = source
            .claims
            .iter()
            .filter(|claim| claim.record().domain_id() == domain)
            .collect();
        if !claims.is_empty() {
            let mut bytes = Vec::new();
            for claim in &claims {
                bytes.extend_from_slice(claim.record().canonical_json().as_bytes());
                bytes.push(b'\n');
            }
            writer.write(
                &format!("{canonical}/claims/{domain}.jsonl"),
                &bytes,
                terms.sensitivity(),
            )?;
        }

        let scopes: Vec<&DomainRecord> = source
            .scopes
            .iter()
            .filter(|record| record.domain_id() == domain)
            .collect();
        let evidence: Vec<&DomainRecord> = source
            .evidence
            .iter()
            .filter(|record| record.domain_id() == domain)
            .collect();
        let decisions: Vec<&DomainRecord> = source
            .decisions
            .iter()
            .filter(|record| record.domain_id() == domain)
            .collect();
        let document = graph::render(domain, &scopes, &artifacts, &evidence, &claims, &decisions)?;
        let label = SensitivityLabel::strongest_of(
            artifacts
                .iter()
                .map(|artifact| artifact.label())
                .chain([terms.sensitivity()]),
        );
        writer.write(&format!("{prefix}graph/{domain}.jsonld"), &document, label)?;
    }

    // The original signed envelopes, copied byte-for-byte. Re-serialising one
    // would change signed historical bytes, and every signature in the bundle
    // would then verify only against this build.
    for batch in &source.batches {
        let terms = request.terms.for_domain(batch.domain_id())?;
        let relative = format!("{prefix}ledger/batches/{}.cbor", batch.batch_id());
        let (digest, byte_length) =
            writer.copy(&relative, batch.envelope_path(), terms.sensitivity())?;
        let observed = encode_hex(digest.as_bytes().as_slice());
        if observed != batch.envelope_sha256() || byte_length != batch.envelope_byte_length() {
            return Err(ExportError::mismatch(
                "exported signed envelope",
                batch.envelope_sha256(),
                observed,
            ));
        }
    }

    let label = writer.label_of(&prefix);
    let markdown = formats_markdown(request);
    writer.write(&format!("{prefix}formats.md"), markdown.as_bytes(), label)?;
    Ok(())
}

/// Part 3 and the claim half of the other topical parts.
fn write_topical_part(
    part: BundlePart,
    request: &BundleRequest<'_>,
    writer: &mut Writer<'_>,
    domains: &[String],
) -> ExportResult<()> {
    write_part_claims(part, request, writer, domains)?;
    let prefix = format!("{PARTS_DIRECTORY}/{}/", part.directory());
    let label = writer.label_of(&prefix);
    let markdown = topical_markdown(part, request);
    writer.write(&format!("{prefix}history.md"), markdown.as_bytes(), label)?;
    Ok(())
}

/// Writes one topical part's claims, partitioned by security domain.
fn write_part_claims(
    part: BundlePart,
    request: &BundleRequest<'_>,
    writer: &mut Writer<'_>,
    domains: &[String],
) -> ExportResult<()> {
    let prefix = format!("{PARTS_DIRECTORY}/{}/claims", part.directory());
    for domain in domains {
        let selected: Vec<&ClaimSource> = request
            .source
            .claims
            .iter()
            .filter(|claim| {
                claim.record().domain_id() == domain
                    && BundlePart::for_predicate(claim.predicate_id()) == Some(part)
            })
            .collect();
        if selected.is_empty() {
            continue;
        }
        let terms = request.terms.for_domain(domain)?;
        let mut bytes = Vec::new();
        for claim in selected {
            bytes.extend_from_slice(claim.record().canonical_json().as_bytes());
            bytes.push(b'\n');
        }
        writer.write(
            &format!("{prefix}/{domain}.jsonl"),
            &bytes,
            terms.sensitivity(),
        )?;
    }
    Ok(())
}

/// Writes one canonical stream partitioned by security domain.
fn write_domain_stream(
    writer: &mut Writer<'_>,
    request: &BundleRequest<'_>,
    domains: &[String],
    prefix: &str,
    records: &[DomainRecord],
) -> ExportResult<()> {
    for domain in domains {
        let selected: Vec<&DomainRecord> = records
            .iter()
            .filter(|record| record.domain_id() == domain)
            .collect();
        if selected.is_empty() {
            continue;
        }
        let terms = request.terms.for_domain(domain)?;
        let mut bytes = Vec::new();
        for record in selected {
            bytes.extend_from_slice(record.canonical_json().as_bytes());
            bytes.push(b'\n');
        }
        writer.write(
            &format!("{prefix}/{domain}.jsonl"),
            &bytes,
            terms.sensitivity(),
        )?;
    }
    Ok(())
}

/// The notice covering one written file.
fn notice_for_path(
    request: &BundleRequest<'_>,
    path: &str,
) -> ExportResult<crate::label::CopyrightNotice> {
    for domain in request.source.domains() {
        if path.contains(&format!("/{domain}/"))
            || path.ends_with(&format!("/{domain}.jsonl"))
            || path.ends_with(&format!("/{domain}.jsonld"))
        {
            return Ok(request.terms.for_domain(&domain)?.notice().clone());
        }
    }
    Ok(request.terms.bundle_notice().clone())
}

/// Renders section 11.3's proof tree as one line per node.
fn render_proof_tree(audit: &DegreeAudit) -> String {
    let mut rendered = String::new();
    render_node(&audit.outcome().proof_tree, 0, &mut rendered);
    rendered
}

fn render_node(node: &academic_domain::engines::ProofNode, depth: usize, rendered: &mut String) {
    for _ in 0..depth {
        rendered.push_str("  ");
    }
    rendered.push_str(node.node_id.as_str());
    rendered.push(' ');
    rendered.push_str(node.rule_id.as_str());
    rendered.push(' ');
    rendered.push_str(node.status.as_str());
    for input in &node.inputs {
        rendered.push_str(" input=");
        rendered.push_str(input.as_str());
    }
    for locator in &node.source_locators {
        rendered.push_str(" source=");
        rendered.push_str(&locator.canonical_text());
    }
    rendered.push('\n');
    for child in &node.children {
        render_node(child, depth + 1, rendered);
    }
}

fn official_record_markdown(
    recorded: RecordedAudit<'_>,
    rule_set_digest: &ContentDigest,
    proof_tree: &str,
) -> String {
    let mut text = String::new();
    text.push_str("# 공식 성적/요건과 계산 proof\n\n");
    text.push_str(
        "This part carries the graduation audit as inputs, rules and proof, not as a verdict \
         somebody may quote. A reader reproduces it by re-running the engine over \
         `audit/frozen-inputs.txt` under the rule set whose canonical text is \
         `audit/rule-set.txt`, and comparing the bytes with `audit/outcome.expected`.\n\n",
    );
    text.push_str("## Binding\n\n| field | value |\n|---|---|\n");
    text.push_str(&format!("| engine | `{GRADUATION_ENGINE_ID}` |\n"));
    text.push_str(&format!(
        "| engine version | {} |\n",
        recorded.engine_version.get()
    ));
    text.push_str(&format!("| rule set | {} |\n", recorded.rules.version()));
    text.push_str(&format!("| rule set hash | `{rule_set_digest}` |\n"));
    text.push_str(&format!(
        "| frozen inputs | `{}` |\n",
        recorded.inputs.digest()
    ));
    text.push_str(&format!(
        "| selected scope | `{}` |\n\n",
        recorded.scope.canonical_text()
    ));
    text.push_str("## Proof tree\n\n```text\n");
    text.push_str(proof_tree);
    text.push_str("```\n");
    text
}

fn archive_markdown(request: &BundleRequest<'_>, objects: &[ObjectRecord]) -> String {
    let mut text = String::new();
    text.push_str("# 원본을 포함하거나 제외할 수 있는 강의·질문 archive\n\n");
    text.push_str(&format!(
        "Originals in this bundle: **{}**.\n\n",
        request.originals.as_str()
    ));
    text.push_str(
        "Every artifact is listed with its exact plaintext digest whether or not its bytes \
         travel, so a withheld original is still identifiable and still verifiable against a \
         copy held elsewhere. A withheld original names no path: nothing in this bundle points \
         at a file the bundle does not carry.\n\n",
    );
    text.push_str("| artifact | media type | bytes | sensitivity | original |\n");
    text.push_str("|---|---|---:|---|---|\n");
    for object in objects {
        let original = match (&object.path, object.withheld) {
            (Some(path), _) => format!("`{path}`"),
            (None, Some(reason)) => format!("withheld ({})", reason.as_str()),
            (None, None) => "unstated".to_owned(),
        };
        text.push_str(&format!(
            "| `{}` | {} | {} | {} | {original} |\n",
            object.artifact_id,
            object.media_type,
            object.byte_length,
            object.sensitivity.as_str()
        ));
    }
    text
}

fn topical_markdown(part: BundlePart, request: &BundleRequest<'_>) -> String {
    let mut text = String::new();
    text.push_str("# ");
    text.push_str(part.specification_sentence());
    text.push_str("\n\n");
    text.push_str(
        "Claims are selected into this part by the first segment of their predicate \
         identifier. Every claim in this bundle also appears whole under \
         `parts/machine-readable-graph/`, which carries the canonical state without \
         selection.\n\n",
    );
    text.push_str("Predicate namespaces: ");
    text.push_str(&part.predicate_namespaces().join(", "));
    text.push_str("\n\n| claim | predicate | rests on |\n|---|---|---:|\n");
    for claim in &request.source.claims {
        if BundlePart::for_predicate(claim.predicate_id()) != Some(part) {
            continue;
        }
        text.push_str(&format!(
            "| `{}` | `{}` | {} |\n",
            claim.record().id(),
            claim.predicate_id(),
            claim.evidence_ids().len()
        ));
    }
    text
}

fn evolution_markdown(request: &BundleRequest<'_>) -> String {
    let mut text = topical_markdown(BundlePart::RepositorySnapshotAndEvolution, request);
    text.push_str("\n## Version-control references\n\n");
    text.push_str(
        "| snapshot | repository | branch | commit | parents |\n|---|---|---|---|---:|\n",
    );
    for reference in &request.source.git_refs {
        text.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            reference.snapshot_id,
            reference.repository_id,
            reference.branch.as_deref().unwrap_or("—"),
            reference.commit.as_deref().unwrap_or("—"),
            reference.parent_snapshots.len()
        ));
    }
    text
}

fn paths_markdown(request: &BundleRequest<'_>) -> String {
    let mut text = topical_markdown(BundlePart::RoleInterestAndAlternativePaths, request);
    text.push_str(
        "\nA change of interest is a decision over a claim, never a deletion of the claim it \
         replaces. The decisions this bundle carries are beside the claims for that reason: an \
         abandoned path stays readable.\n",
    );
    text
}

fn formats_markdown(request: &BundleRequest<'_>) -> String {
    let mut text = String::new();
    text.push_str("# machine-readable graph와 open formats\n\n");
    text.push_str(
        "This part carries the canonical state of the exported watermark whole: every accepted \
         event, registered scope, registered artifact, evidence item, asserted claim, claim \
         relation and recorded decision, as the ledger holds them, plus the original signed \
         envelopes byte-for-byte.\n\n",
    );
    text.push_str("## Formats carried\n\n");
    for format in OPEN_FORMATS {
        text.push_str(&format!("- `{format}`\n"));
    }
    text.push_str("\n## What section 32.10 names that this bundle does not carry\n\n");
    text.push_str(PDF_RENDERING_ABSENCE);
    text.push_str("\n\n## Counts\n\n| record | count |\n|---|---:|\n");
    for (label, count) in [
        ("batches", request.source.batches.len()),
        ("events", request.source.events.len()),
        ("scopes", request.source.scopes.len()),
        ("artifacts", request.source.artifacts.len()),
        ("evidence", request.source.evidence.len()),
        ("claims", request.source.claims.len()),
        ("relations", request.source.relations.len()),
        ("decisions", request.source.decisions.len()),
    ] {
        text.push_str(&format!("| {label} | {count} |\n"));
    }
    text
}

fn inventory_text(
    request: &BundleRequest<'_>,
    parts: &[PartRecord],
    objects: &[ObjectRecord],
) -> String {
    let mut text = String::new();
    text.push_str("# Graduation export bundle\n\n");
    text.push_str(&format!("- format: `{GRADUATION_EXPORT_FORMAT}`\n"));
    text.push_str(&format!(
        "- manifest version: {GRADUATION_EXPORT_MANIFEST_VERSION}\n"
    ));
    text.push_str(&format!("- encrypted: {BUNDLE_ENCRYPTED}\n"));
    text.push_str(&format!("- projections included: {PROJECTIONS_INCLUDED}\n"));
    text.push_str(&format!("- originals: {}\n", request.originals.as_str()));
    text.push_str(&format!(
        "- canonical semantic digest: `{}`\n\n",
        request.source.canonical_semantic_digest
    ));
    text.push_str(
        "This directory is readable without this product. Every record is JSON, JSON-LD, \
         Markdown or CBOR; the signed envelopes are the original bytes the ledger accepted; and \
         the graduation audit under `parts/official-record-and-proof/` is re-runnable from the \
         inputs and the rule text carried beside it.\n\n",
    );
    text.push_str("## The six parts section 37 names\n\n");
    text.push_str("| part | directory | files |\n|---|---|---:|\n");
    for part in parts {
        text.push_str(&format!(
            "| {} | `{}` | {} |\n",
            part.specification_sentence,
            part.directory,
            part.files.len()
        ));
    }
    text.push_str("\n## Originals\n\n");
    let carried = objects
        .iter()
        .filter(|object| object.path.is_some())
        .count();
    text.push_str(&format!(
        "{carried} of {} registered artifacts carry their exact bytes.\n\n",
        objects.len()
    ));
    text.push_str("## Formats\n\n");
    for format in OPEN_FORMATS {
        text.push_str(&format!("- `{format}`\n"));
    }
    text.push('\n');
    text.push_str(PDF_RENDERING_ABSENCE);
    text.push('\n');
    text
}
