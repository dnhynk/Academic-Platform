//! What a transcription job is allowed to read, and why a caller cannot widen
//! it.
//!
//! Section 12.3's first line is the whole input set: *authorized audio chunks +
//! captures + supplied materials*. There are therefore three admitted kinds and
//! no fourth, and each is a type with private fields whose one producer is a
//! method on [`InputManifest`].
//!
//! **The comparison is a journal header, not a caller's word.** An audio chunk
//! is admitted out of a [`JournalRecovery`] whose header names the capability
//! token and the policy row the capture began under; `P2-L2`'s
//! `academic_capture::begin` is the only thing that writes such a header, and
//! it writes one only after `mint_capture_capability` returned a token. So a
//! buffer that was never captured under a live section 3.7 permission is not a
//! value this module will admit -- not because it is inspected and rejected,
//! but because there is no journal to take it out of.

use academic_capture::{JournalRecovery, RecordBody};
use academic_domain::{Actor, ContentDigest, LectureSessionId};

use crate::fault::InputFault;

/// The authorization one manifest is bound to.
///
/// Built from a journal's own header, so the manifest cannot name an
/// authorization the journal does not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationBinding {
    lecture: LectureSessionId,
    token_id: ContentDigest,
    policy_digest: ContentDigest,
}

impl AuthorizationBinding {
    /// Reads the binding out of the journal the capture wrote.
    #[must_use]
    pub fn of(lecture: LectureSessionId, recovery: &JournalRecovery) -> Self {
        let header = recovery.header();
        Self {
            lecture,
            token_id: *header.token_id(),
            policy_digest: *header.policy_digest(),
        }
    }

    /// The lecture session the capture was authorized for.
    #[must_use]
    pub const fn lecture(&self) -> LectureSessionId {
        self.lecture
    }

    /// The capability token the capture was recorded under.
    #[must_use]
    pub const fn token_id(&self) -> &ContentDigest {
        &self.token_id
    }

    /// The effective capture policy row the capture began under.
    #[must_use]
    pub const fn policy_digest(&self) -> &ContentDigest {
        &self.policy_digest
    }

    /// Whether `recovery` was written under this exact authorization.
    fn covers(&self, recovery: &JournalRecovery) -> bool {
        let header = recovery.header();
        header.token_id() == &self.token_id && header.policy_digest() == &self.policy_digest
    }
}

/// One audio chunk a job may read.
///
/// Private fields and no public constructor: the only producer is
/// [`InputManifest::admit_audio_chunk`], which compares the journal's header
/// against the manifest's binding first.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthorizedChunk {
    frame_seq: u32,
    elapsed_nanos: u64,
    chunk_bytes: Vec<u8>,
    digest: ContentDigest,
}

impl AuthorizedChunk {
    /// Its frame position in the journal it came out of.
    #[must_use]
    pub const fn frame_seq(&self) -> u32 {
        self.frame_seq
    }

    /// The session instant the frame was recorded at.
    #[must_use]
    pub const fn elapsed_nanos(&self) -> u64 {
        self.elapsed_nanos
    }

    /// SHA-256 over the chunk as the journal holds it.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes it carries.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.chunk_bytes.len()
    }

    /// The bytes, for a provider that is about to transcribe them.
    ///
    /// Public, because a provider needs them and a provider is an
    /// implementation of [`crate::SttProvider`] outside this crate. It adds no
    /// reach: `academic_capture::CaptureBytes::as_slice` already returns the
    /// same buffer one crate over, and what this crate's guards are about is
    /// the *provider response* -- see [`crate::ProviderResponse`]. The
    /// hand-written `Debug` below is still worth having: it stops a lecture
    /// recording reaching a log through a format string, which is a different
    /// question from whether a caller may deliberately read it.
    #[must_use]
    pub fn audio(&self) -> &[u8] {
        &self.chunk_bytes
    }
}

// The bytes are a lecture recording. `S-10`'s decision for this crate is made
// in the strengthening direction: a hand-written `Debug` that reaches the
// buffer through a length only, registered in `SECRET_BEARING_TYPES`, rather
// than a `PUBLIC_BYTES` entry written into somebody else's contract.
impl core::fmt::Debug for AuthorizedChunk {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AuthorizedChunk")
            .field("frame_seq", &self.frame_seq)
            .field("elapsed_nanos", &self.elapsed_nanos)
            .field("byte_len", &self.chunk_bytes.len())
            .finish()
    }
}

/// One image capture a job may read.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthorizedCapture {
    frame_seq: u32,
    audio_clock_offset_nanos: i64,
    orientation_code: u8,
    chunk_bytes: Vec<u8>,
    digest: ContentDigest,
}

impl AuthorizedCapture {
    /// Its frame position in the journal it came out of.
    #[must_use]
    pub const fn frame_seq(&self) -> u32 {
        self.frame_seq
    }

    /// Nanoseconds from the session's first audio instant.
    #[must_use]
    pub const fn audio_clock_offset_nanos(&self) -> i64 {
        self.audio_clock_offset_nanos
    }

    /// The EXIF orientation the capture stated, as its code.
    #[must_use]
    pub const fn orientation_code(&self) -> u8 {
        self.orientation_code
    }

    /// SHA-256 over the original image bytes.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes it carries.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.chunk_bytes.len()
    }
}

// A photograph of a board is the user's private content, for the same reason
// the audio is.
impl core::fmt::Debug for AuthorizedCapture {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AuthorizedCapture")
            .field("frame_seq", &self.frame_seq)
            .field("audio_clock_offset_nanos", &self.audio_clock_offset_nanos)
            .field("orientation_code", &self.orientation_code)
            .field("byte_len", &self.chunk_bytes.len())
            .finish()
    }
}

/// Something the user handed the pipeline explicitly: a slide deck, a reading,
/// a vocabulary list.
///
/// It is not read out of a journal, because nothing captured it. What stands in
/// for the journal header is the actor: `academic_domain::Actor::User` and no
/// other arm, so an importer or a model run cannot supply material on the
/// user's behalf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuppliedMaterial {
    identifier: String,
    digest: ContentDigest,
    byte_len: usize,
}

impl SuppliedMaterial {
    /// The caller-chosen identifier, restricted to `[A-Za-z0-9._-]`.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// SHA-256 over the supplied bytes.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes were supplied.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }
}

/// Everything one transcription job is allowed to read.
///
/// Private fields and no `Default`. A manifest is opened against one
/// [`AuthorizationBinding`] and grows only through the three `admit_` methods,
/// each of which refuses an input the binding does not cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputManifest {
    binding: AuthorizationBinding,
    chunks: Vec<AuthorizedChunk>,
    captures: Vec<AuthorizedCapture>,
    materials: Vec<SuppliedMaterial>,
}

impl InputManifest {
    /// Opens an empty manifest against one authorization.
    #[must_use]
    pub const fn for_binding(binding: AuthorizationBinding) -> Self {
        Self {
            binding,
            chunks: Vec::new(),
            captures: Vec::new(),
            materials: Vec::new(),
        }
    }

    /// The authorization every admitted input was compared against.
    #[must_use]
    pub const fn binding(&self) -> &AuthorizationBinding {
        &self.binding
    }

    /// Admits one audio frame out of a journal.
    ///
    /// # Errors
    ///
    /// [`InputFault::ForeignJournal`] when the journal's header names another
    /// capability token or another policy row; [`InputFault::NoSuchFrame`] when
    /// no frame carries that sequence; [`InputFault::WrongFrameKind`] when the
    /// frame is not an audio chunk; [`InputFault::DuplicateInput`] when the
    /// frame is already admitted.
    pub fn admit_audio_chunk(
        &mut self,
        recovery: &JournalRecovery,
        frame_seq: u32,
    ) -> Result<(), InputFault> {
        if !self.binding.covers(recovery) {
            return Err(InputFault::ForeignJournal);
        }
        let record = recovery
            .records()
            .iter()
            .find(|record| record.seq() == frame_seq)
            .ok_or(InputFault::NoSuchFrame { frame_seq })?;
        let RecordBody::AudioChunk { bytes } = record.body() else {
            return Err(InputFault::WrongFrameKind { frame_seq });
        };
        if self.chunks.iter().any(|chunk| chunk.frame_seq == frame_seq) {
            return Err(InputFault::DuplicateInput);
        }
        self.chunks.push(AuthorizedChunk {
            frame_seq,
            elapsed_nanos: record.at().elapsed_nanos(),
            chunk_bytes: bytes.as_slice().to_vec(),
            digest: bytes.digest(),
        });
        Ok(())
    }

    /// Admits one image capture out of a journal.
    ///
    /// # Errors
    ///
    /// As [`InputManifest::admit_audio_chunk`], with
    /// [`InputFault::WrongFrameKind`] when the frame is not an image capture.
    pub fn admit_capture(
        &mut self,
        recovery: &JournalRecovery,
        frame_seq: u32,
    ) -> Result<(), InputFault> {
        if !self.binding.covers(recovery) {
            return Err(InputFault::ForeignJournal);
        }
        let record = recovery
            .records()
            .iter()
            .find(|record| record.seq() == frame_seq)
            .ok_or(InputFault::NoSuchFrame { frame_seq })?;
        let RecordBody::ImageCapture {
            bytes,
            orientation,
            audio_clock_offset_nanos,
        } = record.body()
        else {
            return Err(InputFault::WrongFrameKind { frame_seq });
        };
        if self
            .captures
            .iter()
            .any(|capture| capture.frame_seq == frame_seq)
        {
            return Err(InputFault::DuplicateInput);
        }
        self.captures.push(AuthorizedCapture {
            frame_seq,
            audio_clock_offset_nanos: *audio_clock_offset_nanos,
            orientation_code: orientation.code(),
            chunk_bytes: bytes.as_slice().to_vec(),
            digest: bytes.digest(),
        });
        Ok(())
    }

    /// Admits material the user supplied.
    ///
    /// # Errors
    ///
    /// [`InputFault::MaterialNotUserSupplied`] for any actor other than
    /// `Actor::User`; [`InputFault::MaterialIdentifier`] for an identifier that
    /// is empty, over 64 bytes, or outside `[A-Za-z0-9._-]`;
    /// [`InputFault::DuplicateInput`] for an identifier already admitted.
    pub fn admit_supplied_material(
        &mut self,
        supplied_by: &Actor,
        identifier: &str,
        bytes: &[u8],
    ) -> Result<(), InputFault> {
        // `Actor` is `academic-domain`'s closed enum, matched exhaustively, so
        // a fifth actor class stops this crate compiling until it is
        // classified here too.
        match supplied_by {
            Actor::User { .. } => {}
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                return Err(InputFault::MaterialNotUserSupplied);
            }
        }
        if identifier.is_empty() || identifier.len() > 64 {
            return Err(InputFault::MaterialIdentifier);
        }
        if !identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(InputFault::MaterialIdentifier);
        }
        if self
            .materials
            .iter()
            .any(|material| material.identifier == identifier)
        {
            return Err(InputFault::DuplicateInput);
        }
        self.materials.push(SuppliedMaterial {
            identifier: identifier.to_owned(),
            digest: ContentDigest::sha256(bytes),
            byte_len: bytes.len(),
        });
        Ok(())
    }

    /// Every admitted audio chunk, in admission order.
    #[must_use]
    pub fn chunks(&self) -> &[AuthorizedChunk] {
        &self.chunks
    }

    /// Every admitted image capture, in admission order.
    #[must_use]
    pub fn captures(&self) -> &[AuthorizedCapture] {
        &self.captures
    }

    /// Every admitted supplied material, in admission order.
    #[must_use]
    pub fn materials(&self) -> &[SuppliedMaterial] {
        &self.materials
    }

    /// Whether the manifest holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.captures.is_empty() && self.materials.is_empty()
    }

    /// One digest over every admitted input, in admission order.
    ///
    /// Length-prefixed rather than delimited, so an identifier cannot spell a
    /// separator. This is the "shared input hash" every downstream job of
    /// section 12.3 cites.
    #[must_use]
    pub fn input_digest(&self) -> ContentDigest {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-transcription-input-manifest-v1\0");
        material.extend_from_slice(self.binding.token_id.as_bytes());
        material.extend_from_slice(self.binding.policy_digest.as_bytes());
        material.extend_from_slice(&be_len(self.chunks.len()));
        for chunk in &self.chunks {
            material.extend_from_slice(&chunk.frame_seq.to_be_bytes());
            material.extend_from_slice(chunk.digest.as_bytes());
        }
        material.extend_from_slice(&be_len(self.captures.len()));
        for capture in &self.captures {
            material.extend_from_slice(&capture.frame_seq.to_be_bytes());
            material.extend_from_slice(capture.digest.as_bytes());
        }
        material.extend_from_slice(&be_len(self.materials.len()));
        for supplied in &self.materials {
            material.extend_from_slice(&be_len(supplied.identifier.len()));
            material.extend_from_slice(supplied.identifier.as_bytes());
            material.extend_from_slice(supplied.digest.as_bytes());
        }
        ContentDigest::sha256(&material)
    }
}

/// A length as eight big-endian bytes.
pub(crate) fn be_len(value: usize) -> [u8; 8] {
    u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes()
}
