#![allow(dead_code)]
//! Synthetic corpus and broker fixtures shared by the egress suites.
//!
//! Every secret-shaped string here is composed at run time from a fixed
//! sixty-four-bit seed. No token literal appears in this repository, which is
//! both what `CONTRIBUTING` rule 1 requires and what keeps the corpus from
//! being mistaken for a leak by a scanner reading these files.

use std::error::Error;

use academic_egress_boundary::{IdentifierPolicy, SourceDocument, StagedPayload, StagingRequest};
use academic_policy::{
    BrokerError, CapabilityToken, ContentDigest, DecisionOutcome, EgressRule, ObjectRange,
    PermissionBroker, PermissionRequest, PolicySnapshot, ProcessClass, ProviderIdentity,
    ProviderPolicyDraft, ProviderPolicySnapshot, ProviderSurface,
};

/// The one process class `P2-G7` admits for an outbound socket capability.
pub const EGRESS_ACTOR: &str = "synthetic-egress-proxy";
pub const EGRESS_CLASS: ProcessClass = ProcessClass::EgressProxy;

pub type TestResult = Result<(), Box<dyn Error>>;

/// Deterministic byte source. A linear congruential generator, not entropy.
///
/// The tests need tokens that a Shannon-entropy rule scores high and that are
/// byte-identical on every run and every platform. A seeded generator gives
/// both; an operating-system random source would give neither.
pub struct Lcg {
    state: u64,
}

impl Lcg {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state >> 17
    }

    /// A `len`-byte token drawn from `alphabet`.
    pub fn token(&mut self, len: usize, alphabet: &[u8]) -> String {
        let mut token = String::with_capacity(len);
        for _ in 0..len {
            let index = usize::try_from(self.next()).unwrap_or(0) % alphabet.len();
            token.push(char::from(alphabet[index]));
        }
        token
    }
}

pub const UPPER_ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
pub const ALNUM: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
pub const BASE64URL: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
pub const HEX: &[u8] = b"0123456789abcdef";

/// One corpus entry: what it is, and the rule that must find it.
pub struct CorpusEntry {
    pub label: &'static str,
    pub rule_id: &'static str,
    pub text: String,
}

/// Secret-shaped entries, one per pattern class the task names.
#[must_use]
pub fn secret_corpus() -> Vec<CorpusEntry> {
    let mut rng = Lcg::new(0x5EC0_0D15_5EED_0001);
    vec![
        CorpusEntry {
            label: "aws access key id",
            rule_id: "aws-access-key-id",
            text: format!("AKIA{}", rng.token(16, UPPER_ALNUM)),
        },
        CorpusEntry {
            label: "aws session key id",
            rule_id: "aws-session-key-id",
            text: format!("ASIA{}", rng.token(16, UPPER_ALNUM)),
        },
        CorpusEntry {
            label: "github personal token",
            rule_id: "github-token",
            text: format!("ghp_{}", rng.token(36, ALNUM)),
        },
        CorpusEntry {
            label: "slack bot token",
            rule_id: "slack-bot-token",
            text: format!("xoxb-{}", rng.token(30, ALNUM)),
        },
        CorpusEntry {
            label: "google api key",
            rule_id: "google-api-key",
            text: format!("AIza{}", rng.token(35, BASE64URL)),
        },
        CorpusEntry {
            label: "bearer style key",
            rule_id: "bearer-key-prefix",
            text: format!("sk-{}", rng.token(40, ALNUM)),
        },
        CorpusEntry {
            label: "pem private key header",
            rule_id: "pem-private-key",
            text: format!("-----{} RSA PRIVATE KEY-----", "BEGIN"),
        },
        CorpusEntry {
            label: "json web token",
            rule_id: "json-web-token",
            text: format!("eyJ{}", rng.token(60, BASE64URL)),
        },
        CorpusEntry {
            label: "cloud credential assignment",
            rule_id: "cloud-credential-assignment",
            text: format!("aws_secret_access_key = {}", rng.token(40, BASE64URL)),
        },
        CorpusEntry {
            label: "azure account key assignment",
            rule_id: "cloud-credential-assignment",
            text: format!("AccountKey={}", rng.token(44, BASE64URL)),
        },
        CorpusEntry {
            label: "generic credential assignment",
            rule_id: "generic-credential-assignment",
            text: format!("database_password = {}", rng.token(24, ALNUM)),
        },
        CorpusEntry {
            label: "postgres connection string",
            rule_id: "credential-connection-string",
            text: format!(
                "postgres://svc:{}@db.invalid:5432/records",
                rng.token(20, ALNUM)
            ),
        },
        CorpusEntry {
            label: "mongodb connection string",
            rule_id: "credential-connection-string",
            text: format!(
                "mongodb+srv://svc:{}@cluster.invalid/records",
                rng.token(20, ALNUM)
            ),
        },
    ]
}

/// Entries with no vendor prefix at all, found only by entropy.
#[must_use]
pub fn entropy_corpus() -> Vec<CorpusEntry> {
    let mut rng = Lcg::new(0x5EC0_0D15_5EED_0002);
    vec![
        CorpusEntry {
            label: "unprefixed base64url token",
            rule_id: "high-entropy-token",
            text: rng.token(44, BASE64URL),
        },
        CorpusEntry {
            label: "unprefixed hexadecimal token",
            rule_id: "high-entropy-hex-token",
            text: rng.token(64, HEX),
        },
    ]
}

/// Personal identifiers, in the shapes a syllabus or a fixture file carries.
#[must_use]
pub fn pii_corpus() -> Vec<CorpusEntry> {
    vec![
        CorpusEntry {
            label: "email address",
            rule_id: "email-address",
            text: "j.doe@students.invalid".to_owned(),
        },
        CorpusEntry {
            label: "resident registration number",
            rule_id: "resident-registration-number",
            text: "900101-1234567".to_owned(),
        },
        CorpusEntry {
            label: "telephone number",
            rule_id: "telephone-number",
            text: "010-5555-0142".to_owned(),
        },
        CorpusEntry {
            label: "student number",
            rule_id: "student-number",
            text: "2021-12345".to_owned(),
        },
    ]
}

/// A clean multi-item document. Nothing in it trips a rule.
#[must_use]
pub fn clean_document() -> String {
    [
        "//! A synthetic module used by the egress acceptance suite.",
        "",
        "/// Adds up the weights of a schedule.",
        "pub fn total_weight(weights: &[u32]) -> u32 {",
        "    let mut total = 0;",
        "    for weight in weights {",
        "        total += weight;",
        "    }",
        "    total",
        "}",
        "",
        "/// Names the term a lecture belongs to.",
        "pub fn term_label(year: u32, season: &str) -> String {",
        "    format!(\"{year} {season}\")",
        "}",
        "",
        "/// Rounds a credit value to the nearest half credit.",
        "pub fn round_credit(raw: u32) -> u32 {",
        "    raw.div_ceil(2) * 2",
        "}",
        "",
    ]
    .join("\n")
}

/// The clean document with `insert` spliced into `total_weight`'s body.
///
/// The insert becomes its own line rather than being appended to an existing
/// one, so a corpus entry that is itself a comment or a literal keeps the span
/// kind it is meant to test.
#[must_use]
pub fn document_with(insert: &str) -> String {
    clean_document().replace(
        "    let mut total = 0;",
        &format!(
            "    let mut total = 0;
    {insert}"
        ),
    )
}

/// A staging request over `document`, focused on `total_weight`.
pub fn staging_request<'a>(
    document: &'a SourceDocument,
    focus: &'a [String],
    policy: &'a IdentifierPolicy,
    max_bytes: u64,
) -> StagingRequest<'a> {
    StagingRequest {
        document,
        focus,
        identifier_policy: policy,
        max_bytes,
    }
}

/// The default focus: one declaration.
#[must_use]
pub fn focus_total_weight() -> Vec<String> {
    vec!["total_weight".to_owned()]
}

fn digest(label: &str) -> ContentDigest {
    ContentDigest::of(label.as_bytes())
}

/// The synthetic provider used by every egress test.
pub fn provider_draft(maximum_input_bytes: u64) -> Result<ProviderPolicyDraft, BrokerError> {
    Ok(ProviderPolicyDraft {
        identity: Some(ProviderIdentity::new(
            "synthetic-provider",
            ProviderSurface::EnterpriseApi,
        )?),
        training_use_enabled: Some(false),
        training_opt_out_applied: Some(false),
        server_retention_millis: Some(0),
        abuse_logging_enabled: Some(false),
        residency_regions: Some(vec!["kr".to_owned()]),
        subprocessors: Some(Vec::new()),
        transit_encryption_declared: Some(true),
        at_rest_encryption_declared: Some(true),
        deletion_api_available: Some(true),
        deletion_receipt_capable: Some(true),
        maximum_input_bytes: Some(maximum_input_bytes),
        logging_configuration: Some("content-logging-disabled".to_owned()),
        policy_source_digest: Some(digest("synthetic-provider-policy-source")),
        last_verified_at: Some(0),
        ttl_millis: Some(1_000_000),
    })
}

/// A broker with the synthetic provider registered and no egress rule yet.
pub fn broker_with_provider(
    maximum_input_bytes: u64,
) -> Result<(PermissionBroker, ProviderPolicySnapshot), BrokerError> {
    let broker = PermissionBroker::new_profile_with_ttl(600_000)?;
    let provider = broker.register_provider_policy(provider_draft(maximum_input_bytes)?, 0)?;
    Ok((broker, provider))
}

/// The exact per-tuple rule that admits one staged payload.
///
/// `redaction_policy_hash` is the rulepack digest, which is what makes the
/// versioned pack identity a recorded property of the grant rather than a claim
/// about the code that produced it.
pub fn rule_for(
    staged: &StagedPayload,
    provider: &ProviderPolicySnapshot,
    rulepack_hash: ContentDigest,
) -> Result<EgressRule, BrokerError> {
    Ok(EgressRule {
        actor_id: EGRESS_ACTOR.to_owned(),
        process_class: EGRESS_CLASS,
        data_class: "synthetic-private-code".to_owned(),
        operation: "classify".to_owned(),
        purpose_id: "architecture-classification".to_owned(),
        destination_id: provider.destination_id().to_owned(),
        retention_terms_hash: provider.retention_terms_hash(),
        consent_evidence_id: "synthetic-consent-event".to_owned(),
        valid_from: 0,
        valid_until: 1_000_000,
        minimal_ranges: vec![staged.object_range()?],
        payload_digest: staged.preview().digest(),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        training_use_allowed: false,
        redaction_policy_hash: rulepack_hash,
    })
}

/// The matching complete request tuple.
pub fn request_for(
    staged: &StagedPayload,
    provider: &ProviderPolicySnapshot,
    policy_version: academic_policy::PolicyVersion,
    requested_at: u64,
) -> Result<PermissionRequest, BrokerError> {
    Ok(PermissionRequest {
        actor_id: Some(EGRESS_ACTOR.to_owned()),
        process_class: EGRESS_CLASS,
        data_class: Some("synthetic-private-code".to_owned()),
        object_range_digest_set: Some(vec![staged.object_range()?]),
        operation: Some("classify".to_owned()),
        purpose_id: Some("architecture-classification".to_owned()),
        destination_id: Some(provider.destination_id().to_owned()),
        retention_terms_hash: Some(provider.retention_terms_hash()),
        requested_at: Some(requested_at),
        consent_evidence_id: Some("synthetic-consent-event".to_owned()),
        policy_version: Some(policy_version),
    })
}

/// Installs the rule for `staged` and mints one capability over it.
pub fn capability_for(
    broker: &PermissionBroker,
    staged: &StagedPayload,
    provider: &ProviderPolicySnapshot,
    rulepack_hash: ContentDigest,
    issued_at: u64,
) -> Result<DecisionOutcome, BrokerError> {
    let rule = rule_for(staged, provider, rulepack_hash)?;
    let version = broker.install_policy(PolicySnapshot::from_rules(vec![rule])?)?;
    let request = request_for(staged, provider, version, issued_at)?;
    broker.evaluate(request, issued_at)
}

/// The capability token out of a decision, or a test failure.
pub fn token(outcome: DecisionOutcome) -> Result<(CapabilityToken, String), Box<dyn Error>> {
    let grant_id = outcome
        .receipt
        .grant_id()
        .ok_or("the broker allowed without minting a grant")?
        .to_owned();
    let capability = outcome
        .capability
        .ok_or("the broker allowed without a capability")?;
    Ok((capability, grant_id))
}

/// A range over the staged payload, widened by one byte.
pub fn widened(staged: &StagedPayload) -> Result<ObjectRange, BrokerError> {
    let range = staged.object_range()?;
    ObjectRange::new(
        range.object_id(),
        range.start(),
        range.end().saturating_add(1),
        range.content_digest().clone(),
    )
}
