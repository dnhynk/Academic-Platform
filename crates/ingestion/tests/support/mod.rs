//! Synthetic fixtures. Nothing here reaches a network, and nothing is real.
//!
//! `CONTRIBUTING.md`'s first rule is synthetic fixtures only, and its fourth is
//! no runtime network dependency. The transport below answers from bytes
//! composed in this file; the crate under test implements no transport at all,
//! which is why supplying one here is the whole of what a "connector run" is in
//! Phase 2. `GATE-38-020` is open and no live connector runs behind it.

use std::cell::RefCell;

use academic_domain::ContentDigest;
use academic_ingestion::{
    AllowedFrequency, AuthenticationMethod, Completeness, ConditionalFetch, ConditionalRequest,
    ConnectorId, ConnectorManifest, Corpus, DeclaredTarget, FetchOutcome, HeaderValue,
    HttpMetadata, LastSuccess, ManifestDraft, NextVerification, ParserVersion, PersonalDataClass,
    ProgramKey, RetrievalInstant, SourceCategory, SourceOwnership, TermsLedger, TermsStatus,
};

/// The declared document every fixture connector retrieves.
pub const CATALOGUE: DeclaredTarget =
    DeclaredTarget::declared("official/cse/graduation-requirements");

/// A second declared document, for the two-source cases.
pub const BYLAW: DeclaredTarget = DeclaredTarget::declared("official/cse/department-bylaw");

/// A document the fixture connector does not declare.
pub const UNDECLARED: DeclaredTarget = DeclaredTarget::declared("official/cse/not-declared");

/// The wall-clock second every fixture retrieval reports.
pub const RETRIEVED_AT: RetrievalInstant = RetrievalInstant::at(1_772_000_000);

/// The parser version every fixture manifest declares.
pub const PARSER: ParserVersion = ParserVersion::new(3);

/// A connector identifier.
pub fn connector(name: &str) -> Result<ConnectorId, Box<dyn std::error::Error>> {
    Ok(ConnectorId::new(name)?)
}

/// A manifest with every section 29.1 field answered.
pub fn manifest(name: &str) -> Result<ConnectorManifest, Box<dyn std::error::Error>> {
    Ok(draft(name)?.build()?)
}

/// The same manifest, still a draft, so a test can drop one field.
pub fn draft(name: &str) -> Result<ManifestDraft, Box<dyn std::error::Error>> {
    Ok(
        ManifestDraft::for_connector(connector(name)?, SourceCategory::DepartmentPage)
            .declaring(CATALOGUE)
            .declaring(BYLAW)
            .source_ownership(SourceOwnership::CollegeOrDepartment)
            .authentication_method(AuthenticationMethod::PublicNoCredential)
            .allowed_frequency(AllowedFrequency::Weekly)
            .terms_status(TermsStatus::PermittedForDeclaredMethod)
            .personal_data_class(PersonalDataClass::Public)
            .completeness(Completeness::Partial)
            .last_success(LastSuccess::Never)
            .next_verification(NextVerification::due_at(RetrievalInstant::at(
                RETRIEVED_AT.seconds() + 86_400,
            )))
            .parser_version(PARSER),
    )
}

/// A ledger in which one connector's terms were reviewed and permit a fetch.
pub fn permitting_ledger(name: &str) -> Result<TermsLedger, Box<dyn std::error::Error>> {
    let mut ledger = TermsLedger::new();
    ledger.record(connector(name)?, TermsStatus::PermittedForDeclaredMethod);
    Ok(ledger)
}

/// A corpus that knows the fixture programme and nothing else.
pub fn corpus() -> Result<Corpus, Box<dyn std::error::Error>> {
    Ok(Corpus::new().knowing(ProgramKey::new("cse")?))
}

/// One official document, composed line by line.
#[derive(Debug, Clone)]
pub struct DocumentFixture {
    authority: &'static str,
    issued: Option<&'static str>,
    effective: Option<&'static str>,
    program: &'static str,
    cohorts: &'static str,
    transition: &'static str,
    sections: Vec<(&'static str, Vec<(&'static str, String)>)>,
}

impl DocumentFixture {
    /// A dated department rule for the `cse` programme, cohorts 2023 onwards.
    pub fn dated() -> Self {
        Self {
            authority: "DEPARTMENT_RULE",
            issued: Some("2026-01-15"),
            effective: Some("2026-03-01"),
            program: "cse",
            cohorts: "2023-",
            transition: "PRIOR_COHORT_KEEPS_PREVIOUS_RULE",
            sections: vec![
                (
                    "art-12",
                    vec![
                        (
                            "r-12-1",
                            "major electives require thirty credits".to_owned(),
                        ),
                        ("r-12-2", "a capstone project is required".to_owned()),
                    ],
                ),
                (
                    "art-13",
                    vec![("r-13-1", "a thesis substitutes for the capstone".to_owned())],
                ),
            ],
        }
    }

    /// The same document with no `EFFECTIVE:` line. `IN02`.
    pub fn undated() -> Self {
        Self {
            effective: None,
            ..Self::dated()
        }
    }

    /// Replaces one rule's text.
    pub fn with_rule_text(mut self, rule: &str, text: &str) -> Self {
        for (_, rules) in &mut self.sections {
            for (id, body) in rules.iter_mut() {
                if *id == rule {
                    *body = text.to_owned();
                }
            }
        }
        self
    }

    /// Moves one rule into another section.
    pub fn moving_rule(mut self, rule: &str, into: &'static str) -> Self {
        let mut carried = None;
        for (_, rules) in &mut self.sections {
            if let Some(index) = rules.iter().position(|(id, _)| *id == rule) {
                carried = Some(rules.remove(index));
            }
        }
        if let Some(entry) = carried {
            for (section, rules) in &mut self.sections {
                if *section == into {
                    rules.push(entry);
                    return self;
                }
            }
            self.sections.push((into, vec![entry]));
        }
        self
    }

    /// Drops one rule.
    pub fn without_rule(mut self, rule: &str) -> Self {
        for (_, rules) in &mut self.sections {
            rules.retain(|(id, _)| *id != rule);
        }
        self
    }

    /// Adds one rule to a section.
    pub fn with_extra_rule(mut self, section: &'static str, rule: &'static str) -> Self {
        for (existing, rules) in &mut self.sections {
            if *existing == section {
                rules.push((rule, format!("added rule {rule}")));
                return self;
            }
        }
        self.sections
            .push((section, vec![(rule, format!("added rule {rule}"))]));
        self
    }

    /// Sets the effective date.
    pub fn effective_on(mut self, date: &'static str) -> Self {
        self.effective = Some(date);
        self
    }

    /// Sets the issuing authority.
    pub fn issued_by(mut self, authority: &'static str) -> Self {
        self.authority = authority;
        self
    }

    /// Sets the cohort range.
    pub fn for_cohorts(mut self, cohorts: &'static str) -> Self {
        self.cohorts = cohorts;
        self
    }

    /// Sets the transitional measures.
    pub fn transitioning(mut self, transition: &'static str) -> Self {
        self.transition = transition;
        self
    }

    /// The bytes.
    pub fn bytes(&self) -> Vec<u8> {
        let mut text = format!("AUTHORITY: {}\n", self.authority);
        if let Some(issued) = self.issued {
            text.push_str(&format!("ISSUED: {issued}\n"));
        }
        if let Some(effective) = self.effective {
            text.push_str(&format!("EFFECTIVE: {effective}\n"));
        }
        text.push_str(&format!("PROGRAM: {}\n", self.program));
        text.push_str(&format!("COHORTS: {}\n", self.cohorts));
        text.push_str(&format!("TRANSITION: {}\n", self.transition));
        for (section, rules) in &self.sections {
            text.push_str(&format!("SECTION: {section}\n"));
            for (id, body) in rules {
                text.push_str(&format!("RULE: {id} | {body}\n"));
            }
        }
        text.into_bytes()
    }
}

/// A response carrying a body, with a matching observed digest.
pub fn body(bytes: Vec<u8>, entity_tag: &str) -> Result<FetchOutcome, Box<dyn std::error::Error>> {
    let observed = ContentDigest::sha256(&bytes);
    Ok(FetchOutcome::Body {
        at: RETRIEVED_AT,
        http: metadata(entity_tag)?,
        source_bytes: bytes,
        observed,
    })
}

/// A response carrying a body whose observed digest describes other bytes.
///
/// `IN01`: the source changed between the read and the store.
pub fn torn_body(
    bytes: Vec<u8>,
    read_instead: &[u8],
    entity_tag: &str,
) -> Result<FetchOutcome, Box<dyn std::error::Error>> {
    Ok(FetchOutcome::Body {
        at: RETRIEVED_AT,
        http: metadata(entity_tag)?,
        source_bytes: bytes,
        observed: ContentDigest::sha256(read_instead),
    })
}

/// A `304`.
pub fn not_modified(entity_tag: &str) -> Result<FetchOutcome, Box<dyn std::error::Error>> {
    Ok(FetchOutcome::NotModified {
        at: RETRIEVED_AT,
        http: HttpMetadata::new(Some(304), Some(HeaderValue::new(entity_tag)?), None, None),
    })
}

/// Response metadata with an entity tag.
pub fn metadata(entity_tag: &str) -> Result<HttpMetadata, Box<dyn std::error::Error>> {
    Ok(HttpMetadata::new(
        Some(200),
        Some(HeaderValue::new(entity_tag)?),
        Some(HeaderValue::new("Thu, 15 Jan 2026 00:00:00 GMT")?),
        Some(HeaderValue::new("text/plain; charset=utf-8")?),
    ))
}

/// A transport that answers from committed bytes.
///
/// It is conditional: a request whose entity tag matches the one this source
/// holds is answered `304`, which is what makes
/// `conditional_fetch_and_hash_diff` a test of the request rather than of the
/// fixture.
pub struct FixtureSource {
    entity_tag: String,
    bytes: Vec<u8>,
    error: Option<String>,
    requests: RefCell<Vec<bool>>,
}

impl FixtureSource {
    /// A source holding one document under one entity tag.
    pub fn holding(bytes: Vec<u8>, entity_tag: &str) -> Self {
        Self {
            entity_tag: entity_tag.to_owned(),
            bytes,
            error: None,
            requests: RefCell::new(Vec::new()),
        }
    }

    /// A source whose transport fails.
    pub fn failing(detail: &str) -> Self {
        Self {
            entity_tag: String::new(),
            bytes: Vec::new(),
            error: Some(detail.to_owned()),
            requests: RefCell::new(Vec::new()),
        }
    }

    /// Whether each request so far was conditional.
    pub fn conditional_requests(&self) -> Vec<bool> {
        self.requests.borrow().clone()
    }
}

impl ConditionalFetch for FixtureSource {
    fn fetch(&self, request: &ConditionalRequest) -> Result<FetchOutcome, String> {
        self.requests
            .borrow_mut()
            .push(request.validators().is_conditional());
        if let Some(detail) = &self.error {
            return Err(detail.clone());
        }
        let matched = request
            .validators()
            .entity_tag()
            .is_some_and(|tag| tag.as_str() == self.entity_tag);
        if matched {
            return not_modified(&self.entity_tag).map_err(|error| error.to_string());
        }
        body(self.bytes.clone(), &self.entity_tag).map_err(|error| error.to_string())
    }
}

impl core::fmt::Debug for FixtureSource {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FixtureSource")
            .field("entity_tag", &self.entity_tag)
            .field("byte_len", &self.bytes.len())
            .finish()
    }
}
