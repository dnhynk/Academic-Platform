//! Effective-dated capture thresholds, and the book they are rows in.
//!
//! # Why none of these is a constant
//!
//! Four numbers decide what this crate does: how much clock drift is still
//! alignable, how little free storage stops a capture, how low a battery stops
//! one, and how long a failure signal may take to appear. The specification
//! fixes none of them — section 34.1's row says only "drift estimation" and
//! `±초 오차 범위`, and `t001` lists a threshold as an open gate candidate under
//! `REQ-12-017`, `REQ-12-018` and `REQ-34-021`.
//!
//! A number spelled in an `if` cannot be superseded, cannot be dated against a
//! capture that predates it, and cannot say which decision it came from. So all
//! four are fields of a [`CapturePolicyRow`] selected by the capture's own
//! instant, exactly as `academic-record`'s `PolicyBook` selects a repeat ceiling
//! by the attempt's own term, and
//! `drift_beyond_tolerance_is_alignment_low_confidence` moves the effective
//! instant and observes the verdict move with it.
//!
//! # An instant no row reaches is `None`
//!
//! [`CapturePolicyBook::effective_at`] returns the last row at or before the
//! instant, and `None` when no row reaches it. `None` is reported as a refusal
//! to begin — never as "no threshold applies", which would be a claim about a
//! period no decision here covers.
//!
//! # Which time axis
//!
//! `effective_from` is on the permission axis: the same `u64` that
//! `academic-consent` compares a grant's lifetime against, and the same one
//! [`crate::recorder::begin`] takes as `now`. It is deliberately not a
//! [`crate::clock::SessionTick`], which measures elapsed time inside one
//! session and cannot order two sessions.

use academic_domain::ContentDigest;

/// One dated set of capture thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapturePolicyRow {
    id: &'static str,
    effective_from: u64,
    drift_tolerance_nanos: u64,
    storage_floor_bytes: u64,
    battery_floor_percent: u8,
    notification_within_nanos: u64,
}

impl CapturePolicyRow {
    /// Declares one row.
    ///
    /// Public so a caller can date a decision of its own — moving the effective
    /// instant is what `drift_beyond_tolerance_is_alignment_low_confidence`
    /// does, and a book that could only be the shipped one would make that row
    /// untestable.
    #[must_use]
    pub const fn declare(
        id: &'static str,
        effective_from: u64,
        drift_tolerance_nanos: u64,
        storage_floor_bytes: u64,
        battery_floor_percent: u8,
        notification_within_nanos: u64,
    ) -> Self {
        Self {
            id,
            effective_from,
            drift_tolerance_nanos,
            storage_floor_bytes,
            battery_floor_percent,
            notification_within_nanos,
        }
    }

    /// The row's identifier, which is what a record cites rather than a number.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// The instant this row starts applying at.
    #[must_use]
    pub const fn effective_from(&self) -> u64 {
        self.effective_from
    }

    /// How far a drift estimate may reach before alignment is low confidence.
    #[must_use]
    pub const fn drift_tolerance_nanos(&self) -> u64 {
        self.drift_tolerance_nanos
    }

    /// The free space below which a capture stops.
    #[must_use]
    pub const fn storage_floor_bytes(&self) -> u64 {
        self.storage_floor_bytes
    }

    /// The battery charge below which an uncharging capture stops.
    #[must_use]
    pub const fn battery_floor_percent(&self) -> u8 {
        self.battery_floor_percent
    }

    /// How long a failure signal may take to reach the timeline.
    #[must_use]
    pub const fn notification_within_nanos(&self) -> u64 {
        self.notification_within_nanos
    }

    /// The digest a journal frame cites this row by.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut material = Vec::with_capacity(64);
        material.extend_from_slice(b"academic.capture.policy-row/v1");
        material.extend_from_slice(self.id.as_bytes());
        material.extend_from_slice(&self.effective_from.to_be_bytes());
        material.extend_from_slice(&self.drift_tolerance_nanos.to_be_bytes());
        material.extend_from_slice(&self.storage_floor_bytes.to_be_bytes());
        material.push(self.battery_floor_percent);
        material.extend_from_slice(&self.notification_within_nanos.to_be_bytes());
        ContentDigest::sha256(&material)
    }
}

/// The dated rows a capture selects its thresholds from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturePolicyBook {
    rows: Vec<CapturePolicyRow>,
}

impl CapturePolicyBook {
    /// Builds a book from rows in any order.
    #[must_use]
    pub fn of(rows: Vec<CapturePolicyRow>) -> Self {
        let mut rows = rows;
        rows.sort_by_key(CapturePolicyRow::effective_from);
        Self { rows }
    }

    /// The shipped book.
    ///
    /// One row, and its numbers are this repository's decision rather than the
    /// specification's — the specification names none. Each is written so a
    /// synthetic corpus can straddle it: a two-second drift tolerance, a
    /// sixty-four mebibyte storage floor, a five percent battery floor, and a
    /// two-second signal latency.
    #[must_use]
    pub fn published() -> Self {
        Self::of(vec![CapturePolicyRow::declare(
            "capture.thresholds.2026_first",
            PUBLISHED_EFFECTIVE_FROM,
            2_000_000_000,
            67_108_864,
            5,
            2_000_000_000,
        )])
    }

    /// Every row, earliest first.
    #[must_use]
    pub fn rows(&self) -> &[CapturePolicyRow] {
        &self.rows
    }

    /// The last row at or before `at`, or `None` when no row reaches it.
    #[must_use]
    pub fn effective_at(&self, at: u64) -> Option<CapturePolicyRow> {
        self.rows
            .iter()
            .rev()
            .find(|row| row.effective_from <= at)
            .copied()
    }
}

/// When the shipped row starts applying.
///
/// A named constant rather than a literal inside [`CapturePolicyBook::published`]
/// so a fixture can sit on either side of it without restating the number.
pub const PUBLISHED_EFFECTIVE_FROM: u64 = 1_000_000;
