//! The bytes a capture holds, and the metadata that travels beside them.
//!
//! # The original bytes are the ones that are kept
//!
//! Section 12.2 asks capture to store "원본 image, orientation, timestamp,
//! audio clock offset". All four are separate: the orientation is a field, not
//! a transform applied to the bytes, and `capture_metadata_integrity` compares
//! the stored digest against the digest of the bytes the caller handed in. No
//! function in this crate rotates, re-encodes, strips or re-compresses a
//! capture, and there is no accessor that returns anything but the whole of
//! what came in.
//!
//! # Why the bytes are behind a type with a hand-written `Debug`
//!
//! A lecture recording and a photograph of a board are the user's private
//! content. `tools/secret-debug-policy.test.mjs` refuses a derived `Debug` over
//! a field named from its vocabulary — `chunk_bytes` is on that list — because
//! `format!("{value:?}")` in a log line or a panic message would print them.
//! [`CaptureBytes`] is registered there and its formatter reaches the buffer
//! only through a length.

use academic_domain::ContentDigest;
use std::fmt;

/// Captured bytes, as they arrived.
///
/// No transform, no truncation, and no accessor that yields part of them.
#[derive(Clone, PartialEq, Eq)]
pub struct CaptureBytes {
    chunk_bytes: Vec<u8>,
}

impl CaptureBytes {
    /// Takes the bytes exactly as the caller supplies them.
    #[must_use]
    pub const fn of(chunk_bytes: Vec<u8>) -> Self {
        Self { chunk_bytes }
    }

    /// The bytes, whole.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.chunk_bytes
    }

    /// How many there are.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.chunk_bytes.len()
    }

    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.chunk_bytes.is_empty()
    }

    /// The digest over the whole of them.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(&self.chunk_bytes)
    }
}

impl fmt::Debug for CaptureBytes {
    /// Redacting: the buffer reaches the formatter only as a length.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CaptureBytes")
            .field("len", &self.chunk_bytes.len())
            .finish_non_exhaustive()
    }
}

/// How the captured image is oriented relative to its stored bytes.
///
/// The eight EXIF orientation values, declared here rather than read out of the
/// file. `t001`'s `REQ-12-015` row asks for an "exact EXIF-independent
/// orientation", and this is what that means: the caller states the orientation
/// and the bytes are never opened to look for one, so a capture whose bytes
/// carry no EXIF block still has an exact orientation and a capture whose EXIF
/// block disagrees with the device does not silently win.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    /// Row 0 top, column 0 left. Upright.
    TopLeft,
    /// Row 0 top, column 0 right. Mirrored horizontally.
    TopRight,
    /// Row 0 bottom, column 0 right. Rotated 180 degrees.
    BottomRight,
    /// Row 0 bottom, column 0 left. Mirrored vertically.
    BottomLeft,
    /// Row 0 left, column 0 top. Mirrored and rotated 270 degrees.
    LeftTop,
    /// Row 0 right, column 0 top. Rotated 90 degrees clockwise.
    RightTop,
    /// Row 0 right, column 0 bottom. Mirrored and rotated 90 degrees.
    RightBottom,
    /// Row 0 left, column 0 bottom. Rotated 270 degrees clockwise.
    LeftBottom,
}

impl Orientation {
    /// Every orientation, in EXIF order.
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomRight,
        Self::BottomLeft,
        Self::LeftTop,
        Self::RightTop,
        Self::RightBottom,
        Self::LeftBottom,
    ];

    /// The EXIF code, which is also the journal frame's byte.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::TopLeft => 1,
            Self::TopRight => 2,
            Self::BottomRight => 3,
            Self::BottomLeft => 4,
            Self::LeftTop => 5,
            Self::RightTop => 6,
            Self::RightBottom => 7,
            Self::LeftBottom => 8,
        }
    }

    /// Resolves an orientation from its EXIF code.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.code() == code)
    }
}
