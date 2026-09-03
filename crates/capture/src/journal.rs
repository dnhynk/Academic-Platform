//! The device-local chunk journal: what survives a lost connection, a stopped
//! capture, and a killed process.
//!
//! # What it is
//!
//! One append-only file of chain-digested frames. Section 12.2 asks that
//! "연결이 끊겨도 장치 로컬 chunk에 계속 기록한다" and section 34.1's omission
//! row asks prevention to be a "local chunk journal"; `t001`'s `REQ-12-017` row
//! asks the evidence to be a "contiguous local chunk timeline/hashes and later
//! resumable processing". So a frame carries the chunk's own digest, the digest
//! of the frame before it, and its own digest over both — the contiguity and
//! the hashes are the same structure, and neither is a claim a reader has to
//! take on trust.
//!
//! Nothing here opens a socket, and there is no upload path to lose: the
//! journal is what a capture writes, always, and connectivity is a property of
//! whatever reads it later. That is why `offline_capture_continuity` needs no
//! network to sever.
//!
//! # Append-only, and what recovery is allowed to remove
//!
//! [`ChunkJournal`] has one mutating operation that reaches the file,
//! [`ChunkJournal::append`], and it only ever extends. Recovery removes exactly
//! one thing: a trailing partial frame, which is bytes no frame digest ever
//! covered, left behind by a process that died between `write` and `sync`.
//! `the_journal_appends_and_never_rewrites` holds the whole set of public
//! `&mut self` methods against a table with a written reason for each and pins
//! the one place the file is shortened.
//!
//! # A resumed session is a second clock and the file says so
//!
//! A file header fixes the domain the first frames were minted under. A
//! [`RecordBody::Gap`] with [`GapCause::SessionResumed`] carries the domain
//! every frame after it is minted under, so replay knows which clock each tick
//! belongs to and a resumed capture is never re-timestamped onto the old one.
//!
//! # What this is not evidence for
//!
//! The frames are plaintext on disk. The profile's `storage_encryption` is
//! `NONE`, `production_data_allowed` is `false`, ADR-002 is unaccepted, and
//! every byte in this crate's fixtures is a committed literal. Sealing the
//! journal under `AEAD_CHUNKED_V2` is open item `C-8` in
//! [the capture subsystem contract](../../../docs/contracts/capture-subsystem.md).
//! The chain detects truncation and corruption; it is not a signature and it
//! does not defend against a writer who can already edit files inside the
//! profile.

use std::{
    fs::{File, OpenOptions},
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
};

use academic_domain::ContentDigest;

use crate::{
    align::{AlignmentConfidence, Anchor, DriftEstimate, MappingVersion},
    capture::{CaptureBytes, Orientation},
    clock::{SessionClockDomain, SessionTick},
    fault::{self, FaultPoint},
    mark::MarkLabelKind,
    preflight::{FailureKind, SignalDelivery},
};

/// The eight bytes every journal file opens with.
pub const JOURNAL_MAGIC: &[u8; 8] = b"ACJRNL01";

/// The file header: magic, clock domain, policy row digest, token identifier.
const HEADER_LEN: usize = 8 + 32 + 32 + 32;

/// A frame header: sequence, kind, tick sequence, elapsed, body length, parent.
const FRAME_HEADER_LEN: usize = 4 + 1 + 4 + 8 + 4 + 32;

/// The largest body a single frame may carry.
///
/// Sixteen mebibytes. A capture chunk is seconds of audio or one photograph;
/// the bound exists so a corrupt length field cannot make recovery allocate an
/// arbitrary buffer, and it is checked on the way in as well as on the way out.
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Why the journal refused, or what recovery found.
#[derive(Debug, thiserror::Error)]
pub enum JournalFault {
    /// The filesystem refused.
    #[error("journal i/o: {0}")]
    Io(#[from] std::io::Error),
    /// A journal already exists at that path.
    #[error("a journal already exists at {0}")]
    AlreadyExists(PathBuf),
    /// The file does not open with [`JOURNAL_MAGIC`].
    #[error("the file is not a capture journal")]
    NotAJournal,
    /// The file is shorter than a header.
    #[error("the journal header is incomplete")]
    HeaderIncomplete,
    /// A frame's digest does not cover its bytes.
    #[error("frame {seq} does not match its digest")]
    FrameCorrupt {
        /// Which frame.
        seq: u32,
    },
    /// A frame's parent digest is not the previous frame's digest.
    #[error("frame {seq} does not chain to the frame before it")]
    ChainBroken {
        /// Which frame.
        seq: u32,
    },
    /// A frame names a body shape this build does not know.
    #[error("frame {seq} carries an unknown body")]
    UnknownBody {
        /// Which frame.
        seq: u32,
    },
    /// The body is larger than [`MAX_BODY_BYTES`].
    #[error("a frame body of {len} bytes is over the bound")]
    BodyTooLarge {
        /// How large.
        len: usize,
    },
    /// The frame's instant is below the instant of the frame before it, on the
    /// same clock.
    ///
    /// [`SessionClock::tick`](crate::clock::SessionClock::tick) refuses a
    /// reading below one it accepted, which orders the ticks a clock *mints*.
    /// It says nothing about the order they are *appended* in, and
    /// [`ChunkJournal::append`] is public and takes a tick rather than a
    /// reading. This is the second half.
    #[error("frame instant {offered} is below the recorded {recorded}")]
    FrameOutOfOrder {
        /// The instant the frame offered.
        offered: u64,
        /// The instant of the last frame from the same clock.
        recorded: u64,
    },
}

/// Why a gap opened in the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GapCause {
    /// A preflight failure stopped the capture. Faults `CP02` and `CP03`.
    ResourceFailure,
    /// The section 3.7 permission stopped covering the capture. Fault `CP01`.
    PermissionRefused,
    /// The host offered a reading below one the clock had accepted.
    ClockWentBackwards,
    /// A killed process was recovered and a new clock started. Fault `CP05`.
    SessionResumed,
}

impl GapCause {
    /// Every cause.
    pub const ALL: [Self; 4] = [
        Self::ResourceFailure,
        Self::PermissionRefused,
        Self::ClockWentBackwards,
        Self::SessionResumed,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceFailure => "RESOURCE_FAILURE",
            Self::PermissionRefused => "PERMISSION_REFUSED",
            Self::ClockWentBackwards => "CLOCK_WENT_BACKWARDS",
            Self::SessionResumed => "SESSION_RESUMED",
        }
    }

    /// The frame byte.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::ResourceFailure => 1,
            Self::PermissionRefused => 2,
            Self::ClockWentBackwards => 3,
            Self::SessionResumed => 4,
        }
    }

    /// Resolves a cause from its frame byte.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.code() == code)
    }
}

/// What one frame holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordBody {
    /// Audio, as it arrived.
    AudioChunk {
        /// The bytes.
        bytes: CaptureBytes,
    },
    /// One image, its orientation, and its offset from the audio start.
    ///
    /// The offset is derived from two ticks on the same clock rather than
    /// measured between two clocks, which is what makes section 34.1's
    /// prevention cell a structure instead of a promise.
    ImageCapture {
        /// The original bytes, untransformed.
        bytes: CaptureBytes,
        /// Which way up, as a field beside the bytes.
        orientation: Orientation,
        /// Nanoseconds from the first audio instant of the session.
        audio_clock_offset_nanos: i64,
    },
    /// A Mark Moment, with no label.
    Mark {
        /// Its position among the session's marks.
        mark_seq: u32,
    },
    /// A label applied to a mark that is already in the file.
    MarkLabel {
        /// Which mark.
        mark_seq: u32,
        /// Which label.
        kind: MarkLabelKind,
    },
    /// A non-intrusive failure signal.
    FailureSignal {
        /// Which failure.
        kind: FailureKind,
        /// How it was delivered.
        delivery: SignalDelivery,
        /// The session instant the reading was taken at.
        observed_at_nanos: u64,
    },
    /// An explicit gap.
    Gap {
        /// Why it opened.
        cause: GapCause,
        /// The clock every later frame is minted under, for a resume.
        resumed_domain: Option<SessionClockDomain>,
    },
    /// A mapping version appended by a two-anchor realignment.
    MappingVersion {
        /// Its number, from one.
        version: u32,
        /// The earlier anchor's tick sequence and elapsed nanoseconds.
        first: (u32, u64, u64),
        /// The later anchor's.
        second: (u32, u64, u64),
        /// The offset the first anchor fixes.
        offset_nanos: i64,
        /// How far it moved by the second.
        drift_nanos: i64,
        /// The ± range, zero while the confidence is normal.
        plus_minus_nanos: u64,
    },
}

impl RecordBody {
    /// The frame byte naming this shape.
    #[must_use]
    pub const fn kind_code(&self) -> u8 {
        match self {
            Self::AudioChunk { .. } => 1,
            Self::ImageCapture { .. } => 2,
            Self::Mark { .. } => 3,
            Self::MarkLabel { .. } => 4,
            Self::FailureSignal { .. } => 5,
            Self::Gap { .. } => 6,
            Self::MappingVersion { .. } => 7,
        }
    }

    /// The contract spelling.
    #[must_use]
    pub const fn kind_str(&self) -> &'static str {
        match self {
            Self::AudioChunk { .. } => "AUDIO_CHUNK",
            Self::ImageCapture { .. } => "IMAGE_CAPTURE",
            Self::Mark { .. } => "MARK",
            Self::MarkLabel { .. } => "MARK_LABEL",
            Self::FailureSignal { .. } => "FAILURE_SIGNAL",
            Self::Gap { .. } => "GAP",
            Self::MappingVersion { .. } => "MAPPING_VERSION",
        }
    }

    /// The captured bytes, for the two shapes that carry any.
    #[must_use]
    pub const fn bytes(&self) -> Option<&CaptureBytes> {
        match self {
            Self::AudioChunk { bytes } | Self::ImageCapture { bytes, .. } => Some(bytes),
            _ => None,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Self::AudioChunk { bytes } => out.extend_from_slice(bytes.as_slice()),
            Self::ImageCapture {
                bytes,
                orientation,
                audio_clock_offset_nanos,
            } => {
                out.push(orientation.code());
                out.extend_from_slice(&audio_clock_offset_nanos.to_be_bytes());
                out.extend_from_slice(bytes.as_slice());
            }
            Self::Mark { mark_seq } => out.extend_from_slice(&mark_seq.to_be_bytes()),
            Self::MarkLabel { mark_seq, kind } => {
                out.extend_from_slice(&mark_seq.to_be_bytes());
                out.push(kind.code());
            }
            Self::FailureSignal {
                kind,
                delivery,
                observed_at_nanos,
            } => {
                out.push(kind.code());
                out.push(delivery.code());
                out.extend_from_slice(&observed_at_nanos.to_be_bytes());
            }
            Self::Gap {
                cause,
                resumed_domain,
            } => {
                out.push(cause.code());
                match resumed_domain {
                    Some(domain) => {
                        out.push(1);
                        out.extend_from_slice(domain.digest().as_bytes());
                    }
                    None => out.push(0),
                }
            }
            Self::MappingVersion {
                version,
                first,
                second,
                offset_nanos,
                drift_nanos,
                plus_minus_nanos,
            } => {
                out.extend_from_slice(&version.to_be_bytes());
                for anchor in [first, second] {
                    out.extend_from_slice(&anchor.0.to_be_bytes());
                    out.extend_from_slice(&anchor.1.to_be_bytes());
                    out.extend_from_slice(&anchor.2.to_be_bytes());
                }
                out.extend_from_slice(&offset_nanos.to_be_bytes());
                out.extend_from_slice(&drift_nanos.to_be_bytes());
                out.extend_from_slice(&plus_minus_nanos.to_be_bytes());
            }
        }
        out
    }

    fn decode(kind: u8, body: &[u8]) -> Option<Self> {
        let mut cursor = Cursor::over(body);
        let decoded = match kind {
            1 => Self::AudioChunk {
                bytes: CaptureBytes::of(body.to_vec()),
            },
            2 => {
                let orientation = Orientation::from_code(cursor.byte()?)?;
                let audio_clock_offset_nanos = i64::from_be_bytes(cursor.array::<8>()?);
                Self::ImageCapture {
                    bytes: CaptureBytes::of(cursor.rest().to_vec()),
                    orientation,
                    audio_clock_offset_nanos,
                }
            }
            3 => Self::Mark {
                mark_seq: u32::from_be_bytes(cursor.array::<4>()?),
            },
            4 => Self::MarkLabel {
                mark_seq: u32::from_be_bytes(cursor.array::<4>()?),
                kind: MarkLabelKind::from_code(cursor.byte()?)?,
            },
            5 => Self::FailureSignal {
                kind: FailureKind::from_code(cursor.byte()?)?,
                delivery: SignalDelivery::from_code(cursor.byte()?)?,
                observed_at_nanos: u64::from_be_bytes(cursor.array::<8>()?),
            },
            6 => {
                let cause = GapCause::from_code(cursor.byte()?)?;
                let resumed_domain = match cursor.byte()? {
                    0 => None,
                    1 => Some(SessionClockDomain::recorded(
                        ContentDigest::from_sha256_bytes(cursor.array::<32>()?),
                    )),
                    _ => return None,
                };
                Self::Gap {
                    cause,
                    resumed_domain,
                }
            }
            7 => {
                let version = u32::from_be_bytes(cursor.array::<4>()?);
                let first = cursor.anchor()?;
                let second = cursor.anchor()?;
                Self::MappingVersion {
                    version,
                    first,
                    second,
                    offset_nanos: i64::from_be_bytes(cursor.array::<8>()?),
                    drift_nanos: i64::from_be_bytes(cursor.array::<8>()?),
                    plus_minus_nanos: u64::from_be_bytes(cursor.array::<8>()?),
                }
            }
            _ => return None,
        };
        Some(decoded)
    }
}

/// Builds the body a realignment writes, from the version it produced.
#[must_use]
pub fn mapping_version_body(version: MappingVersion) -> RecordBody {
    let anchor = |value: Anchor| {
        (
            value.session_tick().seq(),
            value.session_tick().elapsed_nanos(),
            value.reference_nanos(),
        )
    };
    let estimate: DriftEstimate = version.estimate();
    RecordBody::MappingVersion {
        version: version.version(),
        first: anchor(version.first()),
        second: anchor(version.second()),
        offset_nanos: estimate.offset_nanos(),
        drift_nanos: estimate.drift_nanos(),
        plus_minus_nanos: match estimate.confidence() {
            AlignmentConfidence::Normal => 0,
            AlignmentConfidence::Low { plus_minus_nanos } => plus_minus_nanos,
        },
    }
}

/// A byte reader that returns `None` rather than panicking on a short body.
struct Cursor<'body> {
    body: &'body [u8],
    at: usize,
}

impl<'body> Cursor<'body> {
    const fn over(body: &'body [u8]) -> Self {
        Self { body, at: 0 }
    }

    fn byte(&mut self) -> Option<u8> {
        let value = *self.body.get(self.at)?;
        self.at = self.at.checked_add(1)?;
        Some(value)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.at.checked_add(N)?;
        let slice = self.body.get(self.at..end)?;
        self.at = end;
        slice.try_into().ok()
    }

    fn anchor(&mut self) -> Option<(u32, u64, u64)> {
        Some((
            u32::from_be_bytes(self.array::<4>()?),
            u64::from_be_bytes(self.array::<8>()?),
            u64::from_be_bytes(self.array::<8>()?),
        ))
    }

    fn rest(&self) -> &'body [u8] {
        self.body.get(self.at..).unwrap_or(&[])
    }
}

/// One frame, as it sits in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRecord {
    seq: u32,
    at: SessionTick,
    body: RecordBody,
    parent: ContentDigest,
    digest: ContentDigest,
}

impl JournalRecord {
    /// Its position in the file, from zero.
    #[must_use]
    pub const fn seq(&self) -> u32 {
        self.seq
    }

    /// The session instant it was recorded at.
    #[must_use]
    pub const fn at(&self) -> SessionTick {
        self.at
    }

    /// What it holds.
    #[must_use]
    pub const fn body(&self) -> &RecordBody {
        &self.body
    }

    /// The digest of the frame before it.
    #[must_use]
    pub const fn parent(&self) -> &ContentDigest {
        &self.parent
    }

    /// Its own digest, over its header and its body.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }
}

/// What a journal file opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalHeader {
    domain: SessionClockDomain,
    policy_digest: ContentDigest,
    token_id: ContentDigest,
}

impl JournalHeader {
    /// The clock the first frames were minted under.
    #[must_use]
    pub const fn domain(&self) -> SessionClockDomain {
        self.domain
    }

    /// The policy row the capture began under.
    #[must_use]
    pub const fn policy_digest(&self) -> &ContentDigest {
        &self.policy_digest
    }

    /// The capability token the capture began under.
    #[must_use]
    pub const fn token_id(&self) -> &ContentDigest {
        &self.token_id
    }
}

/// What was found in a journal on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRecovery {
    header: JournalHeader,
    records: Vec<JournalRecord>,
    partial_tail_bytes: u64,
}

impl JournalRecovery {
    /// The file header.
    #[must_use]
    pub const fn header(&self) -> JournalHeader {
        self.header
    }

    /// Every complete frame, in file order.
    #[must_use]
    pub fn records(&self) -> &[JournalRecord] {
        &self.records
    }

    /// How many trailing bytes belonged to no complete frame.
    ///
    /// Non-zero after a process died between `write` and `sync`. Those bytes
    /// are what recovery truncates, and nothing else is.
    #[must_use]
    pub const fn partial_tail_bytes(&self) -> u64 {
        self.partial_tail_bytes
    }

    /// The last complete frame, which is what `CP05` calls the last synced
    /// chunk.
    #[must_use]
    pub fn last_synced(&self) -> Option<&JournalRecord> {
        self.records.last()
    }
}

/// An open journal.
#[derive(Debug)]
pub struct ChunkJournal {
    path: PathBuf,
    file: File,
    header: JournalHeader,
    records: Vec<JournalRecord>,
    tail: ContentDigest,
}

/// The parent digest of the first frame: thirty-two zero bytes.
fn genesis() -> ContentDigest {
    ContentDigest::from_sha256_bytes([0_u8; 32])
}

fn frame_digest(header: &[u8], body: &[u8]) -> ContentDigest {
    let mut material = Vec::with_capacity(header.len().saturating_add(body.len()));
    material.extend_from_slice(header);
    material.extend_from_slice(body);
    ContentDigest::sha256(&material)
}

impl ChunkJournal {
    /// Creates a journal at `path`, refusing to overwrite one.
    pub fn create(
        path: &Path,
        domain: SessionClockDomain,
        policy_digest: ContentDigest,
        token_id: ContentDigest,
    ) -> Result<Self, JournalFault> {
        let mut file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(JournalFault::AlreadyExists(path.to_path_buf()));
            }
            Err(error) => return Err(JournalFault::Io(error)),
        };
        let mut head = Vec::with_capacity(HEADER_LEN);
        head.extend_from_slice(JOURNAL_MAGIC);
        head.extend_from_slice(domain.digest().as_bytes());
        head.extend_from_slice(policy_digest.as_bytes());
        head.extend_from_slice(token_id.as_bytes());
        file.write_all(&head)?;
        file.sync_all()?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
            header: JournalHeader {
                domain,
                policy_digest,
                token_id,
            },
            records: Vec::new(),
            tail: genesis(),
        })
    }

    /// Reads a journal file back, stopping at the first frame that is not whole.
    ///
    /// It reads and does not write. Truncation is [`ChunkJournal::reopen`]'s,
    /// which is the only place in this crate that shortens a file.
    pub fn recover(path: &Path) -> Result<JournalRecovery, JournalFault> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Self::replay(&bytes)
    }

    /// Replays journal bytes. The whole of recovery's decision, so a corpus can
    /// be replayed without a file.
    pub fn replay(bytes: &[u8]) -> Result<JournalRecovery, JournalFault> {
        let head = bytes
            .get(..HEADER_LEN)
            .ok_or(JournalFault::HeaderIncomplete)?;
        if head.get(..8) != Some(JOURNAL_MAGIC.as_slice()) {
            return Err(JournalFault::NotAJournal);
        }
        let digest_at = |from: usize| -> Result<ContentDigest, JournalFault> {
            let end = from.saturating_add(32);
            let slice = head.get(from..end).ok_or(JournalFault::HeaderIncomplete)?;
            let array: [u8; 32] = slice
                .try_into()
                .map_err(|_| JournalFault::HeaderIncomplete)?;
            Ok(ContentDigest::from_sha256_bytes(array))
        };
        let header = JournalHeader {
            domain: SessionClockDomain::recorded(digest_at(8)?),
            policy_digest: digest_at(40)?,
            token_id: digest_at(72)?,
        };

        let mut records: Vec<JournalRecord> = Vec::new();
        let mut parent = genesis();
        let mut domain = header.domain;
        let mut at = HEADER_LEN;
        while let Some(frame_header) = bytes.get(at..at.saturating_add(FRAME_HEADER_LEN)) {
            let mut cursor = Cursor::over(frame_header);
            let (
                Some(seq_bytes),
                Some(kind),
                Some(tick_seq_bytes),
                Some(elapsed_bytes),
                Some(len_bytes),
                Some(parent_bytes),
            ) = (
                cursor.array::<4>(),
                cursor.byte(),
                cursor.array::<4>(),
                cursor.array::<8>(),
                cursor.array::<4>(),
                cursor.array::<32>(),
            )
            else {
                break;
            };
            let seq = u32::from_be_bytes(seq_bytes);
            let body_len = usize::try_from(u32::from_be_bytes(len_bytes)).unwrap_or(usize::MAX);
            if body_len > MAX_BODY_BYTES {
                break;
            }
            let body_at = at.saturating_add(FRAME_HEADER_LEN);
            let body_end = body_at.saturating_add(body_len);
            let Some(body) = bytes.get(body_at..body_end) else {
                break;
            };
            let Some(trailer) = bytes.get(body_end..body_end.saturating_add(32)) else {
                break;
            };
            let Ok(trailer): Result<[u8; 32], _> = trailer.try_into() else {
                break;
            };
            let computed = frame_digest(frame_header, body);
            if computed.as_bytes() != &trailer {
                return Err(JournalFault::FrameCorrupt { seq });
            }
            if ContentDigest::from_sha256_bytes(parent_bytes) != parent {
                return Err(JournalFault::ChainBroken { seq });
            }
            let decoded =
                RecordBody::decode(kind, body).ok_or(JournalFault::UnknownBody { seq })?;
            if let RecordBody::Gap {
                cause: GapCause::SessionResumed,
                resumed_domain: Some(resumed),
            } = decoded
            {
                domain = resumed;
            }
            let record = JournalRecord {
                seq,
                at: SessionTick::recorded(
                    domain,
                    u32::from_be_bytes(tick_seq_bytes),
                    u64::from_be_bytes(elapsed_bytes),
                ),
                body: decoded,
                parent,
                digest: computed,
            };
            parent = computed;
            records.push(record);
            at = body_end.saturating_add(32);
        }
        let partial_tail_bytes = u64::try_from(bytes.len().saturating_sub(at)).unwrap_or(u64::MAX);
        Ok(JournalRecovery {
            header,
            records,
            partial_tail_bytes,
        })
    }

    /// Recovers a journal and reopens it for appending.
    ///
    /// This is the only place in this crate that shortens a file, and it
    /// shortens it to exactly the end of the last complete frame — bytes no
    /// frame digest covers, written by a process that died before `sync`
    /// returned. `the_journal_appends_and_never_rewrites` pins the whole of it.
    pub fn reopen(path: &Path) -> Result<(Self, JournalRecovery), JournalFault> {
        let recovery = Self::recover(path)?;
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let complete = path
            .metadata()?
            .len()
            .saturating_sub(recovery.partial_tail_bytes);
        if recovery.partial_tail_bytes > 0 {
            file.set_len(complete)?;
            file.sync_all()?;
        }
        let mut file = file;
        file.seek(SeekFrom::End(0))?;
        let tail = recovery
            .records
            .last()
            .map_or_else(genesis, |record| *record.digest());
        Ok((
            Self {
                path: path.to_path_buf(),
                file,
                header: recovery.header,
                records: recovery.records.clone(),
                tail,
            },
            recovery,
        ))
    }

    /// Where the file is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file header.
    #[must_use]
    pub const fn header(&self) -> JournalHeader {
        self.header
    }

    /// Every frame this journal holds, in file order.
    #[must_use]
    pub fn records(&self) -> &[JournalRecord] {
        &self.records
    }

    /// The digest of the last frame, or the genesis digest.
    #[must_use]
    pub const fn tail(&self) -> &ContentDigest {
        &self.tail
    }

    /// Appends one frame and syncs it.
    ///
    /// The frame is built whole in memory and written with one call, so a
    /// process killed during it leaves a prefix that no digest covers and that
    /// recovery drops. The three `CP05` failpoints sit around this write and
    /// are compiled only by `phase2-fault-injection`.
    ///
    /// **A frame below the one before it is refused, and the comparison is
    /// inside one clock.** `SessionClock::tick` orders the instants a clock
    /// mints; this is public, takes a tick rather than a reading, and is what
    /// orders the instants a file *holds*. The two are different claims and the
    /// second does not follow from the first: a caller holding two ticks from
    /// one clock can offer them in either order.
    ///
    /// Across domains there is nothing to compare. A resumed session starts a
    /// new clock at its own origin, so its first frame's instant is below every
    /// frame the killed clock wrote, and that discontinuity is what the
    /// [`RecordBody::Gap`] frame beside it records rather than something to
    /// refuse. It is the same reading as
    /// [`SessionTick::offset_from`](crate::clock::SessionTick::offset_from)
    /// returning `None` across domains: two clocks have no defined distance.
    pub fn append(
        &mut self,
        at: SessionTick,
        body: RecordBody,
    ) -> Result<&JournalRecord, JournalFault> {
        if let Some(previous) = self.records.last() {
            let recorded = previous.at();
            if at.domain() == recorded.domain() && at.elapsed_nanos() < recorded.elapsed_nanos() {
                return Err(JournalFault::FrameOutOfOrder {
                    offered: at.elapsed_nanos(),
                    recorded: recorded.elapsed_nanos(),
                });
            }
        }
        let encoded = body.encode();
        if encoded.len() > MAX_BODY_BYTES {
            return Err(JournalFault::BodyTooLarge { len: encoded.len() });
        }
        let seq = u32::try_from(self.records.len()).unwrap_or(u32::MAX);
        let body_len = u32::try_from(encoded.len()).unwrap_or(u32::MAX);
        let mut frame_header = Vec::with_capacity(FRAME_HEADER_LEN);
        frame_header.extend_from_slice(&seq.to_be_bytes());
        frame_header.push(body.kind_code());
        frame_header.extend_from_slice(&at.seq().to_be_bytes());
        frame_header.extend_from_slice(&at.elapsed_nanos().to_be_bytes());
        frame_header.extend_from_slice(&body_len.to_be_bytes());
        frame_header.extend_from_slice(self.tail.as_bytes());
        let digest = frame_digest(&frame_header, &encoded);

        let mut frame = Vec::with_capacity(
            FRAME_HEADER_LEN
                .saturating_add(encoded.len())
                .saturating_add(32),
        );
        frame.extend_from_slice(&frame_header);
        frame.extend_from_slice(&encoded);
        frame.extend_from_slice(digest.as_bytes());

        fault::trip(FaultPoint::BeforeFrameWrite, seq);
        let split = frame.len().saturating_sub(32);
        self.file.write_all(frame.get(..split).unwrap_or(&frame))?;
        fault::trip(FaultPoint::AfterBodyBeforeTrailer, seq);
        self.file.write_all(frame.get(split..).unwrap_or(&[]))?;
        self.file.sync_all()?;
        fault::trip(FaultPoint::AfterFrameSynced, seq);

        self.tail = digest;
        self.records.push(JournalRecord {
            seq,
            at,
            body,
            parent: ContentDigest::from_sha256_bytes(
                frame_header
                    .get(FRAME_HEADER_LEN.saturating_sub(32)..)
                    .and_then(|slice| slice.try_into().ok())
                    .unwrap_or([0_u8; 32]),
            ),
            digest,
        });
        self.records.last().ok_or(JournalFault::HeaderIncomplete)
    }

    /// Re-reads the file and checks every frame against its own digest and its
    /// parent's.
    ///
    /// The in-memory records are not consulted, so this is an independent
    /// reading of what is durable rather than a comparison of the writer with
    /// itself.
    pub fn verify_on_disk(&self) -> Result<JournalRecovery, JournalFault> {
        Self::recover(&self.path)
    }
}
