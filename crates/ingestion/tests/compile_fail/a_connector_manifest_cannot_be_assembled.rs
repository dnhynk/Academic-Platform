//! Every section 29.1 field is answered because the draft is the only route.
//!
//! A struct literal would let a caller skip the builder and leave a field at
//! whatever `Default` would have given it, which is the shape
//! `connector_manifest_requires_every_field` exists to refuse.

use academic_ingestion::ConnectorManifest;

fn main() {
    let _forged = ConnectorManifest {};
}
