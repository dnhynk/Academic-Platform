//! The bytes of a raw provider response do not leave this crate.
//!
//! The accessor is `pub(crate)`, the struct's fields are private, and an
//! archived response hands back the `P2-G5` label rather than a payload --
//! which implements no `Deref`, no `Display`, and no `Into<String>`.

use academic_transcription::{ArchivedResponse, ProviderResponse};

fn read(response: &ProviderResponse) -> &[u8] {
    response.response_bytes()
}

fn field(response: &ProviderResponse) -> &[u8] {
    &response.provider_response_bytes
}

fn unwrap_label(entry: &ArchivedResponse) -> String {
    entry.labelled().to_string()
}

fn main() {}
