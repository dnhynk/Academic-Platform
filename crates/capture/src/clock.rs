//! The one session clock, and the instants it is the only source of.
//!
//! # Why "shared" has to be a type
//!
//! Section 34.1's misalignment row prevents the defect with "capture와 audio
//! process 공통 session clock". A test that reads an audio instant and an image
//! instant and compares them passes whether they came from one clock or from
//! two that happen to agree, so the name would be right and the meaning wrong.
//!
//! So the instant is [`SessionTick`] and it has no public constructor.
//! [`SessionClock::tick`] is the only thing that builds one, every tick carries
//! the [`SessionClockDomain`] of the clock that minted it, and every seam in
//! this crate that accepts an instant from outside compares that domain against
//! the recorder's own through [`SessionClock::admit`]. A second clock cannot
//! produce a tick the first one's recorder accepts, because
//! [`SessionClock::start`] mixes a per-instance ordinal into the domain: two
//! clocks over the same lecture and the same token still differ.
//!
//! **What that separates, and what it does not.** Two clocks started in one
//! process differ by the ordinal; a clock that continues a journal another
//! process wrote differs by that journal's tail digest, which the resume passes
//! as its predecessor. Two independent processes that each start a *first*
//! clock for the same lecture under the same token produce the same domain, and
//! nothing here needs to tell those apart: they write different journals and a
//! journal's header names its own domain. There is no wall clock and no random
//! source in this derivation, so the domain of a given session is reproducible.
//!
//! # This crate reads no clock
//!
//! `elapsed_nanos` arrives as an argument, exactly as every instant in
//! `academic-consent` and `academic-capture-gate` does, so the acceptance rows
//! can name the instants they assert against. What this module owns is not the
//! reading but the refusal: [`SessionClock::tick`] rejects a reading below the
//! last one it accepted, so no tick sequence this crate mints goes backwards
//! and a wall-clock rollback cannot be recorded as elapsed time.
//! `no_wall_clock_reaches_the_session_clock` is the source half of that.
//!
//! # A resume is a new clock and says so
//!
//! A process that was killed mid-capture comes back with no monotonic origin it
//! can prove is the old one. Continuing on a fresh clock while pretending the
//! domain never changed is the silent re-timestamping section 34.1 forbids, so
//! the domain changes and [`crate::journal::RecordBody::Gap`] carries both.

use std::sync::atomic::{AtomicU64, Ordering};

use academic_domain::{ContentDigest, LectureSessionId};

/// How many clocks this process has started.
///
/// It is what makes two clocks distinguishable without a random source and
/// without reading a clock: the ordinal is mixed into the domain, so
/// [`SessionClock::start`] twice over the same lecture and the same token
/// yields two domains that compare unequal. Nothing asserts its value; only
/// that two clocks differ and that one clock is stable.
static CLOCKS_STARTED: AtomicU64 = AtomicU64::new(0);

/// The domain separator the clock identity is derived under.
const CLOCK_DOMAIN_INFO: &[u8] = b"academic.capture.session-clock/v1";

/// The identity of one session clock.
///
/// Opaque. It is compared, never parsed, and there is no constructor that takes
/// a digest, so a caller cannot name a domain it did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionClockDomain(ContentDigest);

impl SessionClockDomain {
    /// The digest form, for a journal frame and for a gap record.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.0
    }

    /// Rebuilds a domain read back out of a journal file header or a gap frame.
    ///
    /// Crate-private, and it is not a way to name a running clock's domain: a
    /// journal file inside the profile is what a session already wrote, so this
    /// reads back an identity rather than minting one. What it makes possible
    /// is the resume story — a recovered journal's ticks keep the domain of the
    /// clock that minted them and therefore compare unequal to the new one.
    pub(crate) const fn recorded(digest: ContentDigest) -> Self {
        Self(digest)
    }
}

/// One instant on one session clock.
///
/// There is no public constructor. [`SessionClock::tick`] is the only producer
/// in this crate, and `the_only_instant_type_comes_from_one_clock` compares the
/// whole set of its construction sites against one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTick {
    domain: SessionClockDomain,
    seq: u32,
    elapsed_nanos: u64,
}

impl SessionTick {
    /// Rebuilds a tick read back out of a journal frame.
    ///
    /// The second and last construction site of this type, and the reason it
    /// exists is that a journal outlives the clock that wrote it: a recovered
    /// record has to carry the instant it was recorded at, and that instant is
    /// this type. It is crate-private, its domain comes from the file the frame
    /// was read out of rather than from a caller, and
    /// `the_only_instant_type_comes_from_one_clock` holds the whole inventory
    /// of both sites with a written reason for each.
    pub(crate) const fn recorded(domain: SessionClockDomain, seq: u32, elapsed_nanos: u64) -> Self {
        Self {
            domain,
            seq,
            elapsed_nanos,
        }
    }

    /// Which clock minted it.
    #[must_use]
    pub const fn domain(&self) -> SessionClockDomain {
        self.domain
    }

    /// Its position in that clock's sequence, from zero.
    #[must_use]
    pub const fn seq(&self) -> u32 {
        self.seq
    }

    /// Nanoseconds since the clock started.
    #[must_use]
    pub const fn elapsed_nanos(&self) -> u64 {
        self.elapsed_nanos
    }

    /// The signed distance from `earlier` to this tick, in nanoseconds.
    ///
    /// This is how an audio-clock offset is produced: both ticks come from the
    /// same clock, so the offset is a difference inside one domain rather than
    /// a comparison between two clocks. Across domains it is `None` rather than
    /// a number, because there is no defined distance between two clocks.
    #[must_use]
    pub fn offset_from(self, earlier: Self) -> Option<i64> {
        if earlier.domain != self.domain {
            return None;
        }
        let now = i64::try_from(self.elapsed_nanos).ok()?;
        let then = i64::try_from(earlier.elapsed_nanos).ok()?;
        now.checked_sub(then)
    }
}

/// Why a reading or a tick was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ClockFault {
    /// The reading is below one this clock already accepted.
    ///
    /// A monotonic source cannot produce it, so the caller handed over a wall
    /// clock that stepped back. It is refused rather than clamped: clamping
    /// would put two different real instants on one tick.
    #[error("the reading {offered} is below the accepted {accepted}")]
    WentBackwards {
        /// What the caller offered.
        offered: u64,
        /// The highest reading this clock has accepted.
        accepted: u64,
    },
    /// This clock has minted every sequence number it has.
    #[error("the session clock has no sequence left")]
    SequenceExhausted,
    /// The tick was minted by another clock.
    #[error("the tick belongs to another session clock")]
    ForeignDomain,
}

/// The single monotonic clock a capture session derives every instant from.
#[derive(Debug)]
pub struct SessionClock {
    domain: SessionClockDomain,
    accepted_nanos: u64,
    next_seq: u32,
}

impl SessionClock {
    /// Starts a clock for one lecture under one capability token.
    ///
    /// `predecessor` is the tail digest of the journal this session continues,
    /// and `None` for a session that starts an empty one. It is what separates
    /// a resumed clock from the killed clock whose frames are already in the
    /// file: those frames were written before that digest existed, so a clock
    /// derived from it cannot collide with the one that produced them.
    ///
    /// The only constructor. `the_only_instant_type_comes_from_one_clock`
    /// counts its call sites in this crate's product source and requires
    /// exactly one, so a second clock cannot be started beside the recorder's.
    #[must_use]
    pub fn start(
        lecture_id: LectureSessionId,
        token_id: &ContentDigest,
        predecessor: Option<&ContentDigest>,
    ) -> Self {
        let ordinal = CLOCKS_STARTED.fetch_add(1, Ordering::Relaxed);
        let mut material = Vec::with_capacity(CLOCK_DOMAIN_INFO.len() + 96);
        material.extend_from_slice(CLOCK_DOMAIN_INFO);
        material.extend_from_slice(lecture_id.as_bytes());
        material.extend_from_slice(token_id.as_bytes());
        match predecessor {
            Some(digest) => {
                material.push(1);
                material.extend_from_slice(digest.as_bytes());
            }
            None => material.push(0),
        }
        material.extend_from_slice(&ordinal.to_be_bytes());
        Self {
            domain: SessionClockDomain(ContentDigest::sha256(&material)),
            accepted_nanos: 0,
            next_seq: 0,
        }
    }

    /// This clock's identity.
    #[must_use]
    pub const fn domain(&self) -> SessionClockDomain {
        self.domain
    }

    /// The highest reading accepted so far.
    #[must_use]
    pub const fn accepted_nanos(&self) -> u64 {
        self.accepted_nanos
    }

    /// Turns one monotonic reading into a tick, or refuses it.
    ///
    /// Equal readings are accepted and get their own sequence: two events can
    /// share a nanosecond and still need an order. A lower reading is refused,
    /// which is the whole of this crate's monotonicity guarantee — it reads no
    /// clock, so it cannot promise the host's source is monotonic; it promises
    /// that no tick it minted is below one it already minted.
    pub fn tick(&mut self, elapsed_nanos: u64) -> Result<SessionTick, ClockFault> {
        if elapsed_nanos < self.accepted_nanos {
            return Err(ClockFault::WentBackwards {
                offered: elapsed_nanos,
                accepted: self.accepted_nanos,
            });
        }
        let seq = self.next_seq;
        self.next_seq = seq.checked_add(1).ok_or(ClockFault::SequenceExhausted)?;
        self.accepted_nanos = elapsed_nanos;
        Ok(SessionTick {
            domain: self.domain,
            seq,
            elapsed_nanos,
        })
    }

    /// Refuses a tick minted by another clock.
    ///
    /// The seams that take an instant from outside — a realignment anchor, a
    /// label offered against a mark — run this. A tick is unforgeable, so the
    /// only way to hold one from another domain is to have started a second
    /// clock, and this is where that is observed.
    pub fn admit(&self, tick: SessionTick) -> Result<SessionTick, ClockFault> {
        if tick.domain == self.domain {
            Ok(tick)
        } else {
            Err(ClockFault::ForeignDomain)
        }
    }
}
