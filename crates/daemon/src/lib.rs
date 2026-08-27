//! Compileable Phase 1 daemon shell.
//!
//! There is no listener, profile, database handle, writer, reader, transport,
//! process singleton, or background task in F0.

/// Product binary name reserved for the local-core daemon.
pub const DAEMON_BINARY_NAME: &str = "academicd";
/// Reversible Phase 1 bounded-writer queue default.
pub const WRITER_QUEUE_CAPACITY: usize = 64;
/// Human-readable scaffold notice. It deliberately makes no readiness claim.
pub const F0_SCAFFOLD_NOTICE: &str =
    "academicd Phase 1 F0 contract scaffold — no profile was opened";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_scaffold_has_no_runtime_claim() {
        assert_eq!(DAEMON_BINARY_NAME, "academicd");
        assert_eq!(WRITER_QUEUE_CAPACITY, 64);
        assert!(F0_SCAFFOLD_NOTICE.contains("no profile was opened"));
    }
}
