//! `INV-C-009` from the writer's side of the boundary.
//!
//! `academic-scenario` has no dependency on the canonical writer, so nothing in
//! that crate *can* write. This test asks the harder question from the other
//! side: with a real canonical store open and writable in the same process,
//! does driving the projection engine hard change anything?
//!
//! The answer has to be a hash comparison. "No error was returned" is not
//! evidence — a projection that quietly appended a row would return no error
//! either — so the canonical content is digested before and after and the two
//! digests must be identical byte for byte.

mod support;

use std::{error::Error, fs, path::Path};

use academic_domain::{
    AuthorityClass, ContentDigest, EntityId, EpistemicStatus, MasteryLevel, ModelRunId, OfferingId,
    TimestampMillis,
};
use academic_scenario::{
    OpportunityBasis, ProjectedMastery, ProjectionCalibration, ProjectionEnvelope,
    ProposalProvenance, Proposed, ScenarioAssumption, ScenarioChoice, ScenarioInputs,
    SyllabusConceptSignal, WorkloadHoursRange, admit_projection_payload, project,
};
use academic_store::queries::canonical_snapshot;
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use support::{
    Fixture, TestResult, claim_id, entity, importer_actor, observed_entity_claim, text_claim,
};

/// Iterations of the fuzz loop.
///
/// Large enough to walk every branch of the engine and the admission checks
/// many times over, small enough to stay inside the Rust job's budget on the
/// slowest hosted runner.
const FUZZ_ITERATIONS: u32 = 512;

/// `simulation_fuzz_leaves_actual_state_hash_identical`.
#[test]
fn simulation_fuzz_leaves_actual_state_hash_identical() -> TestResult {
    let mut fixture = Fixture::new("scenario-isolation")?;
    seed_actual_state(&mut fixture)?;

    let before = actual_state_digest(&fixture)?;
    let observed = fuzz_the_simulator()?;
    let after = actual_state_digest(&fixture)?;

    assert_eq!(
        before.as_bytes(),
        after.as_bytes(),
        "the canonical state digest changed while only the projection engine ran"
    );
    // A digest that never covered anything would also compare equal, so the
    // seeded state has to be non-empty and the fuzz has to have done work.
    assert_ne!(
        before.as_bytes(),
        empty_profile_digest()?.as_bytes(),
        "the digest must cover a non-empty canonical state"
    );
    // The generator draws offerings and concepts from small pools so that
    // duplicates occur, which means a healthy run both projects and rejects.
    // Requiring both keeps a generator that degenerated into one branch from
    // passing as a thorough one.
    assert_eq!(
        observed.projected + observed.rejected_inputs,
        FUZZ_ITERATIONS
    );
    assert!(
        observed.projected > FUZZ_ITERATIONS / 8 && observed.rejected_inputs > 0,
        "the fuzz must both project and reject, saw {observed:?}"
    );
    assert!(
        observed.admitted > 0 && observed.refused > 0,
        "the fuzz must exercise both admission outcomes, saw {observed:?}"
    );
    Ok(())
}

/// What the fuzz loop actually did, so an empty run cannot pass as a quiet one.
#[derive(Debug, Default)]
struct FuzzObservations {
    projected: u32,
    rejected_inputs: u32,
    admitted: u32,
    refused: u32,
    calibrations: u32,
}

/// Drives the whole projection surface with pseudo-random inputs.
///
/// The generator is a fixed-seed xorshift rather than a real RNG: a failure
/// here has to be reproducible from the source alone, and section 3.10 keeps
/// ambient randomness out of the engine's own inputs.
fn fuzz_the_simulator() -> Result<FuzzObservations, Box<dyn Error>> {
    let mut random = Xorshift64::new(0x5345_4e41_5249_4f31);
    let mut observed = FuzzObservations::default();
    for iteration in 0..FUZZ_ITERATIONS {
        let inputs = random_inputs(&mut random, iteration)?;
        let Ok(projection) = project(&inputs) else {
            // A rejected input set is a real outcome of the fuzz. It still must
            // not have touched anything, which is what the digest proves.
            observed.rejected_inputs += 1;
            continue;
        };
        observed.projected += 1;

        // Every projection is the shape section 22.3 permits and no other.
        let encoded = serde_json::to_value(&projection)?;
        assert_no_actual_state_field(&encoded);
        assert_eq!(
            projection,
            project(&inputs)?,
            "the engine must be deterministic for identical inputs"
        );

        let payload = serde_json::to_vec(&ProjectionEnvelope::seal(projection))?;
        match admit_projection_payload(&payload) {
            Ok(admitted) => {
                observed.admitted += 1;
                assert_no_actual_state_field(&serde_json::to_value(&admitted)?);
            }
            Err(error) => return Err(format!("a sealed projection was refused: {error}").into()),
        }

        let forged = forge(&mut random, &serde_json::from_slice::<Value>(&payload)?)?;
        if admit_projection_payload(&serde_json::to_vec(&forged)?).is_err() {
            observed.refused += 1;
        }

        // The calibration path is the only read of a sealed value that exists,
        // so the fuzz drives it too.
        let proposed: ProjectedMastery = Proposed::new(
            mastery(random.next_u32()),
            ProposalProvenance::new(
                identifier::<ModelRunId>(0x900 + u16::try_from(iteration % 64).unwrap_or(0))?,
                digest(0x5a)?,
                1,
                TimestampMillis::new(i64::from(iteration)),
            ),
        );
        let calibration = proposed.calibrate(&mastery(random.next_u32()));
        assert!(matches!(
            calibration,
            ProjectionCalibration::Underprojected
                | ProjectionCalibration::Matched
                | ProjectionCalibration::Overprojected
        ));
        observed.calibrations += 1;
    }
    assert_eq!(observed.calibrations, observed.projected);
    Ok(observed)
}

/// Fails if a projection ever grows a field that names actual state.
fn assert_no_actual_state_field(value: &Value) {
    const FORBIDDEN: [&str; 6] = [
        "mastery",
        "mastery_level",
        "freshness",
        "freshness_band",
        "claim",
        "claim_object",
    ];
    match value {
        Value::Object(fields) => {
            for (name, nested) in fields {
                assert!(
                    !FORBIDDEN.contains(&name.as_str()),
                    "a projection carried the actual-state field {name}"
                );
                assert_no_actual_state_field(nested);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_no_actual_state_field),
        _ => {}
    }
}

/// Applies one pseudo-random forgery to a genuine payload.
fn forge(random: &mut Xorshift64, genuine: &Value) -> Result<Value, Box<dyn Error>> {
    let mut forged = genuine.clone();
    match random.next_u32() % 8 {
        0 => forged["hypothetical"] = json!(false),
        1 => forged["authority_class"] = json!("DIRECT_OBSERVATION"),
        2 => forged["epistemic_status"] = json!("USER_CONFIRMED"),
        3 => forged["projection"]["mastery_level"] = json!("FLUENT"),
        4 => forged["projection"]["workload"]["range"]["high_hours"] = json!(167),
        5 => forged["projection"]["inputs_digest"] = json!(digest(0x77)?.to_string()),
        6 => forged["envelope_version"] = json!(2),
        _ => forged["binding_digest"] = json!(digest(0x88)?.to_string()),
    }
    Ok(forged)
}

/// Writes a small but real canonical state: evidence, an observed claim, and a
/// model-inferred one, so the digest below covers more than an empty schema.
fn seed_actual_state(fixture: &mut Fixture) -> TestResult {
    let evidence = fixture.register_scope_evidence(3, 0x21, b"scenario-isolation-evidence")?;
    let subject = entity(0x2001)?;
    let target = entity(0x2002)?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        observed_entity_claim(
            claim_id(0x2101)?,
            subject,
            "academic.course.concept",
            target,
            evidence.scope_id,
            evidence.evidence_id,
            1_000,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        text_claim(
            claim_id(0x2102)?,
            subject,
            "academic.course.title",
            "Synthetic Networks",
            evidence.scope_id,
            evidence.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            1_000,
            None,
        )?,
    )?;
    Ok(())
}

/// Digests the canonical state: every row of every canonical table, the
/// acceptance counts and heads, and the on-disk bytes of the database and its
/// write-ahead log.
///
/// Rows are digested rather than counted. A count would miss an in-place
/// rewrite of a claim object, which is exactly the failure a projection lane
/// could introduce; the append-only triggers already forbid it, and this
/// digest is what would notice if they ever stopped.
fn actual_state_digest(fixture: &Fixture) -> Result<ContentDigest, Box<dyn Error>> {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"academic-core/actual-state-digest/v1");

    let snapshot = canonical_snapshot(&fixture.store_reader()?)?;
    for value in [
        snapshot.next_accept_seq,
        snapshot.profile_revision,
        snapshot.batch_count,
        snapshot.event_count,
        snapshot.scope_count,
        snapshot.artifact_count,
        snapshot.evidence_count,
        snapshot.claim_count,
        snapshot.relation_count,
        snapshot.decision_count,
        snapshot.outbox_count,
        snapshot.receipt_count,
        snapshot.device_count,
        snapshot.accept_seq_head,
        snapshot.outbox_head,
    ] {
        hash_field(&mut hasher, &value.to_be_bytes());
    }

    let path = fixture.canonical_path();
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    for table in canonical_tables(&connection)? {
        hash_field(&mut hasher, table.as_bytes());
        for row in table_rows(&connection, &table)? {
            hash_field(&mut hasher, &row);
        }
    }
    drop(connection);

    // The logical rows are the state; the file bytes catch anything that
    // changed the database without changing a row this query can see.
    hash_field(&mut hasher, &file_bytes_digest(path)?);
    hash_field(&mut hasher, &file_bytes_digest(&with_suffix(path, "-wal"))?);
    Ok(ContentDigest::from_sha256_bytes(hasher.finalize().into()))
}

/// The digest of a profile that was created and never written to.
///
/// Used only to prove the digest above is not returning the same value for
/// every input, which would make the equality assertion meaningless.
fn empty_profile_digest() -> Result<ContentDigest, Box<dyn Error>> {
    let fixture = Fixture::new("scenario-isolation-empty")?;
    actual_state_digest(&fixture)
}

fn canonical_tables(connection: &Connection) -> Result<Vec<String>, Box<dyn Error>> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

/// Reads every row of one table into sorted, self-delimiting byte strings.
///
/// Sorting the encoded rows makes the digest independent of the order SQLite
/// happens to return them in, so a page reorganisation that preserves content
/// does not read as a change.
fn table_rows(connection: &Connection, table: &str) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    let mut statement = connection.prepare(&format!("SELECT * FROM \"{table}\""))?;
    let column_count = statement.column_count();
    let mut rows = statement.query([])?;
    let mut encoded = Vec::new();
    while let Some(row) = rows.next()? {
        let mut buffer = Vec::new();
        for index in 0..column_count {
            match row.get_ref(index)? {
                ValueRef::Null => buffer.push(0),
                ValueRef::Integer(value) => {
                    buffer.push(1);
                    buffer.extend_from_slice(&value.to_be_bytes());
                }
                ValueRef::Real(value) => {
                    buffer.push(2);
                    buffer.extend_from_slice(&value.to_be_bytes());
                }
                ValueRef::Text(value) => {
                    buffer.push(3);
                    push_length_delimited(&mut buffer, value);
                }
                ValueRef::Blob(value) => {
                    buffer.push(4);
                    push_length_delimited(&mut buffer, value);
                }
            }
        }
        encoded.push(buffer);
    }
    encoded.sort_unstable();
    Ok(encoded)
}

fn push_length_delimited(buffer: &mut Vec<u8>, bytes: &[u8]) {
    buffer.extend_from_slice(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    buffer.extend_from_slice(bytes);
}

/// Digests a file, treating an absent one as distinct from an empty one.
fn file_bytes_digest(path: &Path) -> Result<[u8; 32], Box<dyn Error>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Sha256::digest(&bytes).into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Sha256::digest(b"<absent>").into())
        }
        Err(error) => Err(error.into()),
    }
}

fn with_suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut spelling = path.as_os_str().to_owned();
    spelling.push(suffix);
    std::path::PathBuf::from(spelling)
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn random_inputs(
    random: &mut Xorshift64,
    iteration: u32,
) -> Result<ScenarioInputs, Box<dyn Error>> {
    let choice_count = random.next_u32() % 4;
    let mut choices = Vec::new();
    for index in 0..choice_count {
        let concept_count = random.next_u32() % 4;
        let mut syllabus_concepts = Vec::new();
        for concept in 0..concept_count {
            syllabus_concepts.push(SyllabusConceptSignal {
                // The modulus is deliberately small so duplicate concepts and
                // duplicate offerings occur, driving the rejection paths.
                concept_entity_id: identifier::<EntityId>(
                    0x300
                        + u16::try_from(random.next_u32() % 6).unwrap_or(0)
                        + u16::try_from(concept).unwrap_or(0),
                )?,
                basis: basis(random.next_u32()),
                coverage_permille: u16::try_from(random.next_u32() % 1_200).unwrap_or(0),
                assessed: random.next_u32().is_multiple_of(2),
            });
        }
        let low = u16::try_from(random.next_u32() % 30).unwrap_or(0);
        let span = u16::try_from(random.next_u32() % 20).unwrap_or(0);
        choices.push(ScenarioChoice {
            offering_id: identifier::<OfferingId>(
                0x400
                    + u16::try_from(random.next_u32() % 5).unwrap_or(0)
                    + u16::try_from(index).unwrap_or(0),
            )?,
            credit_units: u16::try_from(random.next_u32() % 6).unwrap_or(0),
            assumed_weekly_hours: WorkloadHoursRange::new(low, low.saturating_add(span))?,
            syllabus_concepts,
        });
    }
    Ok(ScenarioInputs {
        scenario_id: identifier::<EntityId>(0x500 + u16::try_from(iteration % 32).unwrap_or(0))?,
        model_run_id: identifier::<ModelRunId>(0x600 + u16::try_from(iteration % 16).unwrap_or(0))?,
        knowledge_state_as_of: TimestampMillis::new(i64::from(random.next_u32())),
        requirement_set_digest: digest(u8::try_from(random.next_u32() % 256).unwrap_or(0))?,
        offering_catalog_digest: digest(u8::try_from(random.next_u32() % 256).unwrap_or(0))?,
        choices,
        assumptions: vec![ScenarioAssumption {
            name: "completionStatus".to_owned(),
            value: "HYPOTHETICAL".to_owned(),
        }],
    })
}

const fn basis(value: u32) -> OpportunityBasis {
    match value % 4 {
        0 => OpportunityBasis::Syllabus,
        1 => OpportunityBasis::AssignmentBrief,
        2 => OpportunityBasis::AssessmentPlan,
        _ => OpportunityBasis::HistoricalOffering,
    }
}

const fn mastery(value: u32) -> MasteryLevel {
    match value % 6 {
        0 => MasteryLevel::Unseen,
        1 => MasteryLevel::Exposed,
        2 => MasteryLevel::Understood,
        3 => MasteryLevel::Practiced,
        4 => MasteryLevel::Applied,
        _ => MasteryLevel::Fluent,
    }
}

fn identifier<T: std::str::FromStr>(suffix: u16) -> Result<T, T::Err> {
    format!("01936f2a-0000-7000-8000-{suffix:012x}").parse()
}

fn digest(seed: u8) -> Result<ContentDigest, academic_domain::DomainError> {
    format!("sha256:{}", format!("{seed:02x}").repeat(32)).parse()
}

/// A fixed-seed xorshift, so a failing iteration is reproducible from source.
struct Xorshift64(u64);

impl Xorshift64 {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    const fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
}
