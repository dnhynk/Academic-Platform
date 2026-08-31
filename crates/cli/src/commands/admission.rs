//! `academic admission verify|show`.

use std::path::Path;

use academic_admission::AdmissionVerifier;
use serde_json::json;

use crate::output::{CliFailure, CommandResult, ExitClass};

fn report(profile_root: &Path) -> (serde_json::Value, Option<CliFailure>) {
    match AdmissionVerifier::verify(profile_root) {
        Ok(verified) => (
            json!({
                "verification": "VERIFIED",
                "reason": null,
                "receipt_digest": verified.receipt_digest(),
                "platforms": verified.platforms(),
            }),
            None,
        ),
        Err(error) => {
            let value = json!({
                "verification": "DENIED",
                "reason": error.code(),
                "detail": error.to_string(),
                "receipt_digest": null,
                "platforms": [],
            });
            let failure = CliFailure::new(ExitClass::PolicyDenied, error.code(), error.to_string())
                .with_result(value.clone());
            (value, Some(failure))
        }
    }
}

/// Verifies the receipt and exits with a policy denial when it is not admitted.
pub fn verify(profile_root: &Path) -> CommandResult {
    let (value, failure) = report(profile_root);
    match failure {
        Some(failure) => Err(failure),
        None => Ok(value),
    }
}

/// Shows the current posture and denial reason without turning denial into an error.
pub fn show(profile_root: &Path) -> serde_json::Value {
    let (value, _failure) = report(profile_root);
    value
}

/// Human rendering shared by `verify` and `show`.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    vec![
        format!("admission verification: {}", value["verification"]),
        format!("reason: {}", value["reason"]),
    ]
}
