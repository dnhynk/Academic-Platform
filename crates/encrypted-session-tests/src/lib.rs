//! Non-shipping integration target for the explicitly selected encrypted graph.
//! Only shared test vocabulary is exported; fixture material stays in the test.

/// Result shared by the real encrypted-session integration cases.
pub type TestResult = Result<(), Box<dyn std::error::Error>>;
