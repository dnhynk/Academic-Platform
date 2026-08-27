//! Coordinate and policy authority bound into disposable projection generations.
//!
//! Canonical row loading and resolution are owned by `academic-store`; this
//! module only owns the versioned registry identity persisted by projections.

use std::collections::BTreeMap;

use academic_domain::{AuthorityClass, EpistemicStatus, PredicateId};
pub use academic_store::queries::{
    AuthorityPolicy, PROJECTION_RESOLVER_VERSION as CANONICAL_RESOLVER_VERSION,
};

use crate::{
    checksum::append_field,
    runner::{ProjectionError, ProjectionResult},
};

/// Versioned, deterministic predicate-to-authority-policy registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredicatePolicies {
    version: String,
    entries: BTreeMap<PredicateId, AuthorityPolicy>,
    canonical_hash: academic_domain::ContentDigest,
}

impl PredicatePolicies {
    /// Constructs a closed policy registry. Duplicate predicates and empty
    /// versions fail closed rather than making the effective policy ambiguous.
    pub fn new(
        version: impl Into<String>,
        entries: impl IntoIterator<Item = (PredicateId, AuthorityPolicy)>,
    ) -> ProjectionResult<Self> {
        let version = version.into();
        if version.trim().is_empty() || version.contains('\0') {
            return Err(ProjectionError::InvalidPolicyRegistry(
                "policy registry version must be non-empty and contain no NUL".to_owned(),
            ));
        }
        let mut policies = BTreeMap::new();
        for (predicate, policy) in entries {
            if policies.insert(predicate.clone(), policy).is_some() {
                return Err(ProjectionError::InvalidPolicyRegistry(format!(
                    "predicate {} occurs more than once",
                    predicate.as_str()
                )));
            }
        }
        let mut canonical = Vec::new();
        append_field(&mut canonical, b"ACADEMIC_PREDICATE_POLICIES_V1");
        append_field(&mut canonical, version.as_bytes());
        for (predicate, policy) in &policies {
            append_field(&mut canonical, predicate.as_str().as_bytes());
            append_field(&mut canonical, authority_policy_name(*policy).as_bytes());
        }
        Ok(Self {
            version,
            entries: policies,
            canonical_hash: academic_domain::ContentDigest::sha256(&canonical),
        })
    }

    /// Convenience constructor for a one-predicate projection policy.
    pub fn single(
        version: impl Into<String>,
        predicate: PredicateId,
        policy: AuthorityPolicy,
    ) -> ProjectionResult<Self> {
        Self::new(version, [(predicate, policy)])
    }

    /// Stable policy registry version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Hash of the version and every sorted predicate-policy pair.
    #[must_use]
    pub const fn canonical_hash(&self) -> academic_domain::ContentDigest {
        self.canonical_hash
    }

    /// Returns the explicit policy for a predicate, failing closed when absent.
    pub fn policy_for(&self, predicate: &PredicateId) -> ProjectionResult<AuthorityPolicy> {
        self.entries
            .get(predicate)
            .copied()
            .ok_or_else(|| ProjectionError::MissingPredicatePolicy(predicate.as_str().to_owned()))
    }

    pub(crate) fn entries(&self) -> &BTreeMap<PredicateId, AuthorityPolicy> {
        &self.entries
    }
}

pub(crate) const fn authority_policy_name(policy: AuthorityPolicy) -> &'static str {
    match policy {
        AuthorityPolicy::UserOwned => "USER_OWNED",
        AuthorityPolicy::OfficialFact => "OFFICIAL_FACT",
        AuthorityPolicy::ImplementationObservation => "IMPLEMENTATION_OBSERVATION",
        AuthorityPolicy::CuratedRelation => "CURATED_RELATION",
    }
}

pub(crate) fn parse_authority_policy(value: &str) -> ProjectionResult<AuthorityPolicy> {
    match value {
        "USER_OWNED" => Ok(AuthorityPolicy::UserOwned),
        "OFFICIAL_FACT" => Ok(AuthorityPolicy::OfficialFact),
        "IMPLEMENTATION_OBSERVATION" => Ok(AuthorityPolicy::ImplementationObservation),
        "CURATED_RELATION" => Ok(AuthorityPolicy::CuratedRelation),
        _ => Err(ProjectionError::Corrupt(
            "authority policy is invalid".to_owned(),
        )),
    }
}

pub(crate) const fn authority_name(value: AuthorityClass) -> &'static str {
    match value {
        AuthorityClass::Official => "OFFICIAL",
        AuthorityClass::UserExplicit => "USER_EXPLICIT",
        AuthorityClass::DirectObservation => "DIRECT_OBSERVATION",
        AuthorityClass::DeterministicEngine => "DETERMINISTIC_ENGINE",
        AuthorityClass::Curated => "CURATED",
        AuthorityClass::ModelInference => "MODEL_INFERENCE",
        AuthorityClass::Prediction => "PREDICTION",
        AuthorityClass::Unknown => "UNKNOWN",
    }
}

pub(crate) fn parse_authority(value: &str) -> ProjectionResult<AuthorityClass> {
    match value {
        "OFFICIAL" => Ok(AuthorityClass::Official),
        "USER_EXPLICIT" => Ok(AuthorityClass::UserExplicit),
        "DIRECT_OBSERVATION" => Ok(AuthorityClass::DirectObservation),
        "DETERMINISTIC_ENGINE" => Ok(AuthorityClass::DeterministicEngine),
        "CURATED" => Ok(AuthorityClass::Curated),
        "MODEL_INFERENCE" => Ok(AuthorityClass::ModelInference),
        "PREDICTION" => Ok(AuthorityClass::Prediction),
        "UNKNOWN" => Ok(AuthorityClass::Unknown),
        _ => Err(ProjectionError::Corrupt(
            "authority class is invalid".to_owned(),
        )),
    }
}

pub(crate) const fn epistemic_name(value: EpistemicStatus) -> &'static str {
    match value {
        EpistemicStatus::OfficialConfirmed => "OFFICIAL_CONFIRMED",
        EpistemicStatus::UserConfirmed => "USER_CONFIRMED",
        EpistemicStatus::CodeObserved => "CODE_OBSERVED",
        EpistemicStatus::DeterministicDerived => "DETERMINISTIC_DERIVED",
        EpistemicStatus::AiInferred => "AI_INFERRED",
        EpistemicStatus::Prediction => "PREDICTION",
        EpistemicStatus::Disputed => "DISPUTED",
        EpistemicStatus::Superseded => "SUPERSEDED",
        EpistemicStatus::Unknown => "UNKNOWN",
    }
}

pub(crate) fn parse_epistemic(value: &str) -> ProjectionResult<EpistemicStatus> {
    match value {
        "OFFICIAL_CONFIRMED" => Ok(EpistemicStatus::OfficialConfirmed),
        "USER_CONFIRMED" => Ok(EpistemicStatus::UserConfirmed),
        "CODE_OBSERVED" => Ok(EpistemicStatus::CodeObserved),
        "DETERMINISTIC_DERIVED" => Ok(EpistemicStatus::DeterministicDerived),
        "AI_INFERRED" => Ok(EpistemicStatus::AiInferred),
        "PREDICTION" => Ok(EpistemicStatus::Prediction),
        "DISPUTED" => Ok(EpistemicStatus::Disputed),
        "SUPERSEDED" => Ok(EpistemicStatus::Superseded),
        "UNKNOWN" => Ok(EpistemicStatus::Unknown),
        _ => Err(ProjectionError::Corrupt(
            "epistemic status is invalid".to_owned(),
        )),
    }
}
