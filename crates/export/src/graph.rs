//! The JSON-LD half of section 32.10's *machine-readable JSON/JSON-LD*.
//!
//! The bundle already carries the canonical records as JSON, one compact record
//! per line, byte-for-byte as the ledger holds them. JSON-LD adds the part JSON
//! alone does not: a node identified by an IRI, and a vocabulary term for every
//! edge, so a reader that has never seen this repository can follow a claim to
//! its evidence, its evidence to an artifact, and an artifact to its digest
//! without being told the column names.
//!
//! # Why this is built from typed values and not from a map
//!
//! Every node below is a struct whose fields serialise in declaration order,
//! and every list is sorted by `@id` before it is written. A `serde_json::Map`
//! would have made the byte order a property of the map implementation and its
//! feature flags rather than of this file, and byte-identical output at a fixed
//! watermark is the whole claim.
//!
//! # The IRI scheme
//!
//! `urn:academic:<kind>:<identifier>`, where the identifier is the canonical
//! one the ledger holds. An artifact is addressed by its artifact identifier,
//! never by its vault locator: two artifacts with identical bytes in one
//! security domain share a locator, and a graph keyed by it would silently
//! merge two nodes into one.

use serde::Serialize;

use crate::{
    ExportError, ExportResult,
    label::SensitivityLabel,
    source::{ArtifactSource, ClaimSource, DomainRecord},
};

/// The fixed vocabulary IRI every bundle declares.
pub const VOCABULARY: &str = "https://academic-os.invalid/vocabulary/graduation-export/v2#";

/// The JSON-LD version a bundle declares, as text.
///
/// Written as a string rather than as the number JSON-LD spells it, because
/// every canonical encoding in this repository forbids a floating-point value:
/// two hosts may render `1.1` differently and the bundle's byte equality is the
/// whole claim.
pub const JSON_LD_VERSION: &str = "1.1";

/// The `@context` a bundle writes, term by term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Context {
    #[serde(rename = "@vocab")]
    vocab: &'static str,
    #[serde(rename = "@version")]
    version: &'static str,
}

/// One node of the exported graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
enum Node {
    Scope {
        #[serde(rename = "@id")]
        id: String,
        #[serde(rename = "@type")]
        node_type: &'static str,
        #[serde(rename = "securityDomain")]
        security_domain: String,
    },
    Artifact {
        #[serde(rename = "@id")]
        id: String,
        #[serde(rename = "@type")]
        node_type: &'static str,
        #[serde(rename = "securityDomain")]
        security_domain: String,
        #[serde(rename = "mediaType")]
        media_type: String,
        #[serde(rename = "contentDigest")]
        content_digest: String,
        #[serde(rename = "byteLength")]
        byte_length: u64,
        sensitivity: SensitivityLabel,
    },
    Evidence {
        #[serde(rename = "@id")]
        id: String,
        #[serde(rename = "@type")]
        node_type: &'static str,
        #[serde(rename = "securityDomain")]
        security_domain: String,
    },
    Claim {
        #[serde(rename = "@id")]
        id: String,
        #[serde(rename = "@type")]
        node_type: &'static str,
        #[serde(rename = "securityDomain")]
        security_domain: String,
        predicate: String,
        #[serde(rename = "restsOn")]
        rests_on: Vec<String>,
    },
    Decision {
        #[serde(rename = "@id")]
        id: String,
        #[serde(rename = "@type")]
        node_type: &'static str,
        #[serde(rename = "securityDomain")]
        security_domain: String,
    },
}

impl Node {
    fn id(&self) -> &str {
        match self {
            Self::Scope { id, .. }
            | Self::Artifact { id, .. }
            | Self::Evidence { id, .. }
            | Self::Claim { id, .. }
            | Self::Decision { id, .. } => id,
        }
    }
}

/// One security domain's slice of the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct GraphDocument {
    #[serde(rename = "@context")]
    context: Context,
    #[serde(rename = "@graph")]
    graph: Vec<Node>,
}

/// Renders one security domain's JSON-LD document.
///
/// The rows handed in are already this domain's; the caller partitions, because
/// a file that mixed domains could not carry one source copyright notice.
pub fn render(
    domain_id: &str,
    scopes: &[&DomainRecord],
    artifacts: &[&ArtifactSource],
    evidence: &[&DomainRecord],
    claims: &[&ClaimSource],
    decisions: &[&DomainRecord],
) -> ExportResult<Vec<u8>> {
    let mut graph: Vec<Node> = Vec::new();
    for scope in scopes {
        graph.push(Node::Scope {
            id: iri("scope", scope.id()),
            node_type: "Scope",
            security_domain: domain_id.to_owned(),
        });
    }
    for artifact in artifacts {
        graph.push(Node::Artifact {
            id: iri("artifact", artifact.artifact_id()),
            node_type: "Artifact",
            security_domain: domain_id.to_owned(),
            media_type: artifact.media_type().to_owned(),
            content_digest: artifact.content_sha256().to_owned(),
            byte_length: artifact.byte_length(),
            sensitivity: artifact.label(),
        });
    }
    for item in evidence {
        graph.push(Node::Evidence {
            id: iri("evidence", item.id()),
            node_type: "Evidence",
            security_domain: domain_id.to_owned(),
        });
    }
    for claim in claims {
        let mut rests_on: Vec<String> = claim
            .evidence_ids()
            .iter()
            .map(|evidence_id| iri("evidence", evidence_id))
            .collect();
        rests_on.sort();
        graph.push(Node::Claim {
            id: iri("claim", claim.record().id()),
            node_type: "Claim",
            security_domain: domain_id.to_owned(),
            predicate: claim.predicate_id().to_owned(),
            rests_on,
        });
    }
    for decision in decisions {
        graph.push(Node::Decision {
            id: iri("decision", decision.id()),
            node_type: "Decision",
            security_domain: domain_id.to_owned(),
        });
    }
    graph.sort_by(|left, right| left.id().cmp(right.id()));

    let document = GraphDocument {
        context: Context {
            vocab: VOCABULARY,
            version: JSON_LD_VERSION,
        },
        graph,
    };
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|source| ExportError::Json {
        operation: "render bundle graph document",
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn iri(kind: &str, identifier: &str) -> String {
    format!("urn:academic:{kind}:{identifier}")
}

#[cfg(test)]
mod tests {
    use super::{iri, render};

    #[test]
    fn an_empty_domain_still_renders_a_document_with_a_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes = render("domain-1", &[], &[], &[], &[], &[])?;
        let text = String::from_utf8(bytes)?;
        assert!(text.contains("\"@context\""));
        assert!(text.contains("\"@graph\": []"));
        assert!(text.ends_with('\n'));
        Ok(())
    }

    #[test]
    fn an_iri_names_the_canonical_identifier_and_not_a_locator() {
        assert_eq!(iri("artifact", "abc"), "urn:academic:artifact:abc");
        assert_eq!(iri("claim", "def"), "urn:academic:claim:def");
    }
}
