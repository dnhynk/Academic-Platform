//! A credential binding comes from a manifest that declares it holds one.
//!
//! `ConnectorManifest::credential_binding` returns `None` for every
//! authentication method that holds no credential. A struct literal would let a
//! public-page connector present one anyway, which is exactly what section
//! 29.2 refuses.

use academic_ingestion::{ConnectorId, CredentialBinding};

fn main() {
    let connector = ConnectorId::new("snu.cse.official").expect("a valid identifier");
    let _forged = CredentialBinding { connector };
}
