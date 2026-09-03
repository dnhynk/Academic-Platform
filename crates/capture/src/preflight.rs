//! Storage, battery and microphone preflight, and the non-intrusive signal a
//! failure raises.
//!
//! # A reading is an argument
//!
//! Nothing here queries a device. [`PreflightReading`] is handed in — by the
//! host at `begin`, and again whenever the host observes a change — for the
//! same reason every instant in this crate is an argument: the acceptance rows
//! can then name the readings they assert against, and no test depends on the
//! machine it ran on. `capture_failure_notifications` injects each of the three
//! failures as a reading.
//!
//! # Non-intrusive is a closed vocabulary, not a comment
//!
//! Section 12.2 asks for the failure to be signalled "즉시 비침습적으로", and
//! 12.2's line above it asks the surface to support 무음 haptic.
//! [`SignalDelivery`] therefore has exactly two variants, both silent, and
//! there is no audible, modal or blocking form to select. A guard that read a
//! boolean named `intrusive` would be a guard that could be flipped;
//! `the_signal_vocabulary_has_no_intrusive_form` compares the whole variant set
//! instead, so a third variant fails whatever it is called.
//!
//! # Immediately is a bound from the policy book
//!
//! "즉시" is measured against [`crate::policy::CapturePolicyRow::notification_within_nanos`],
//! not against a constant. The signal carries the session instant it was raised
//! at and the instant the reading was observed at, so the latency is in the
//! record rather than asserted by the code that wrote it.

use crate::{clock::SessionTick, policy::CapturePolicyRow};

/// What the host sees of the microphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MicrophoneState {
    /// A device is present and the capture holds it.
    Held,
    /// The device the capture held is gone — unplugged, or taken.
    Lost,
}

/// One reading of the three resources a capture depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreflightReading {
    free_storage_bytes: u64,
    battery_percent: u8,
    battery_charging: bool,
    microphone: MicrophoneState,
}

impl PreflightReading {
    /// Records one reading.
    #[must_use]
    pub const fn observed(
        free_storage_bytes: u64,
        battery_percent: u8,
        battery_charging: bool,
        microphone: MicrophoneState,
    ) -> Self {
        Self {
            free_storage_bytes,
            battery_percent,
            battery_charging,
            microphone,
        }
    }

    /// Free space on the volume the journal is on.
    #[must_use]
    pub const fn free_storage_bytes(self) -> u64 {
        self.free_storage_bytes
    }

    /// Battery charge, as a percentage.
    #[must_use]
    pub const fn battery_percent(self) -> u8 {
        self.battery_percent
    }

    /// Whether the battery is charging.
    #[must_use]
    pub const fn battery_charging(self) -> bool {
        self.battery_charging
    }

    /// What the host sees of the microphone.
    #[must_use]
    pub const fn microphone(self) -> MicrophoneState {
        self.microphone
    }

    /// Every failure this reading holds, against one policy row.
    ///
    /// The battery row is not raised while charging: a capture on mains power
    /// at three percent is not about to stop, and stopping it would be the
    /// intrusive failure the row exists to avoid.
    #[must_use]
    pub fn failures(self, policy: CapturePolicyRow) -> Vec<FailureKind> {
        let mut found = Vec::new();
        if self.free_storage_bytes < policy.storage_floor_bytes() {
            found.push(FailureKind::StorageExhausted);
        }
        if !self.battery_charging && self.battery_percent < policy.battery_floor_percent() {
            found.push(FailureKind::BatteryCritical);
        }
        if self.microphone == MicrophoneState::Lost {
            found.push(FailureKind::MicrophoneLost);
        }
        found
    }
}

/// The three failures section 12.2 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailureKind {
    /// Free space is below the effective floor. Fault `CP02`.
    StorageExhausted,
    /// The battery is below the effective floor and not charging.
    BatteryCritical,
    /// The microphone is gone. Fault `CP03`.
    MicrophoneLost,
}

impl FailureKind {
    /// Every failure, in the order section 12.2 lists them.
    pub const ALL: [Self; 3] = [
        Self::StorageExhausted,
        Self::BatteryCritical,
        Self::MicrophoneLost,
    ];

    /// The contract spelling, which is also the journal frame's token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StorageExhausted => "STORAGE_EXHAUSTED",
            Self::BatteryCritical => "BATTERY_CRITICAL",
            Self::MicrophoneLost => "MICROPHONE_LOST",
        }
    }

    /// The frame byte.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::StorageExhausted => 1,
            Self::BatteryCritical => 2,
            Self::MicrophoneLost => 3,
        }
    }

    /// Resolves a failure from its frame byte.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.code() == code)
    }

    /// How this failure is delivered.
    ///
    /// A total function over the enum rather than a field, so a fourth failure
    /// has to state its delivery at the compiler rather than default to one.
    #[must_use]
    pub const fn delivery(self) -> SignalDelivery {
        match self {
            Self::StorageExhausted | Self::MicrophoneLost => SignalDelivery::SilentBanner,
            Self::BatteryCritical => SignalDelivery::SilentHaptic,
        }
    }
}

/// How a failure reaches the person holding the device.
///
/// Both variants are silent and neither takes focus. There is no audible form,
/// no modal, and no variant that blocks the capture surface, because section
/// 12.2 asks for the signal to be non-intrusive and a surface that could be
/// told to interrupt is a surface that will be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignalDelivery {
    /// A persistent line on the capture surface. No focus change, no sound.
    SilentBanner,
    /// A haptic pulse with the ringer untouched.
    SilentHaptic,
}

impl SignalDelivery {
    /// Every delivery. Compared whole by
    /// `the_signal_vocabulary_has_no_intrusive_form`.
    pub const ALL: [Self; 2] = [Self::SilentBanner, Self::SilentHaptic];

    /// The contract spelling, which is also the journal frame's token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SilentBanner => "SILENT_BANNER",
            Self::SilentHaptic => "SILENT_HAPTIC",
        }
    }

    /// The frame byte.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::SilentBanner => 1,
            Self::SilentHaptic => 2,
        }
    }

    /// Resolves a delivery from its frame byte.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.code() == code)
    }
}

/// One raised failure, as it sits in the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureSignal {
    kind: FailureKind,
    delivery: SignalDelivery,
    raised_at: SessionTick,
    observed_at_nanos: u64,
}

impl FailureSignal {
    pub(crate) const fn raised(
        kind: FailureKind,
        raised_at: SessionTick,
        observed_at_nanos: u64,
    ) -> Self {
        Self {
            kind,
            delivery: kind.delivery(),
            raised_at,
            observed_at_nanos,
        }
    }

    /// Which failure.
    #[must_use]
    pub const fn kind(self) -> FailureKind {
        self.kind
    }

    /// How it is delivered.
    #[must_use]
    pub const fn delivery(self) -> SignalDelivery {
        self.delivery
    }

    /// The session instant it reached the timeline at.
    #[must_use]
    pub const fn raised_at(self) -> SessionTick {
        self.raised_at
    }

    /// The reading's own instant, on the same session clock.
    #[must_use]
    pub const fn observed_at_nanos(self) -> u64 {
        self.observed_at_nanos
    }

    /// How long the signal took to reach the timeline, in nanoseconds.
    #[must_use]
    pub const fn latency_nanos(self) -> u64 {
        self.raised_at
            .elapsed_nanos()
            .saturating_sub(self.observed_at_nanos)
    }

    /// Whether it arrived inside the effective bound.
    #[must_use]
    pub const fn within(self, policy: CapturePolicyRow) -> bool {
        self.latency_nanos() <= policy.notification_within_nanos()
    }
}
