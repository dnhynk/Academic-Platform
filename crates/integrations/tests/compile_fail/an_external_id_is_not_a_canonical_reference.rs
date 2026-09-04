//! `an_external_id_is_not_a_canonical_reference`.
//!
//! Section 33 says an external identifier is stored as an `ExternalIdentity`
//! mapping rather than as a canonical identifier. Every route from the external
//! half into the canonical one is tried here.

use std::str::FromStr;

use academic_integrations::{CanonicalRef, ExternalId, ExternalIdentity};

fn main() {
    let external = ExternalId::new("MDEwOlJlcG9zaXRvcnkx").unwrap();

    // There is no `From<ExternalId>`.
    let _converted = CanonicalRef::from(external.clone());

    // Nor a `TryFrom<&str>`.
    let _tried = CanonicalRef::try_from("01900000-0000-7000-8000-000000000004");

    // Nor a `FromStr`, so nor a `parse` through it.
    let _parsed = CanonicalRef::from_str("01900000-0000-7000-8000-000000000004");
    let _turbofished = "01900000-0000-7000-8000-000000000004".parse::<CanonicalRef>();

    // Nor an arm taking text: every arm carries a domain identifier.
    let _arm = CanonicalRef::Entity(external.as_str());

    // Nor the sixteen bytes back the other way: `as_bytes` reads in one
    // direction only.
    let _backwards: CanonicalRef = CanonicalRef::as_bytes(b"0123456789abcdef");

    // And an `ExternalIdentity` cannot be written as a literal to smuggle one
    // in beside the mapping, because its fields are private.
    let _literal = ExternalIdentity {
        external_id: external,
    };
}
