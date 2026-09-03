//! A raw token is what a provider said, and only the decoder builds one.
//!
//! Every field of `RawToken` is private, so a struct literal for it is a
//! compile error outside the module that declares it -- which is the language
//! rule `raw_token_write_protection` rests on when it says a raw value is built
//! in one file.

use academic_transcription::RawToken;

fn token() -> RawToken {
    RawToken {
        text: String::new(),
        start_nanos: None,
        confidence: None,
    }
}

fn main() {}
