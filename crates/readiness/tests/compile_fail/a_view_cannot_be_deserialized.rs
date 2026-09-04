//! A readiness view is a derivation over three crates' frozen values.
//!
//! A view read back out of a document would be a filled matrix and a published
//! score that ran no producer, and the notice beside them would be whatever the
//! document said. What a recipient gets instead is `published_notice`.

use academic_readiness::ReadinessView;

fn shape(document: &str) -> Result<ReadinessView, serde_json::Error> {
    serde_json::from_str(document)
}

fn main() {
    let _ = shape;
}
