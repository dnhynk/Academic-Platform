//! Comparing two providers' transcriptions of the same audio, without ranking
//! them.
//!
//! Section 12.3 requires the raw response and the exact model version to be
//! kept so a re-transcription can be *compared*. `P2-M1` requires something
//! else in the same breath: a provider's raw number and another provider's raw
//! number mean different things, so nothing may order them. Those two are
//! compatible, and this module is where the line runs.
//!
//! **What is reported** is where the two disagree, symmetrically. Neither side
//! is a baseline: they are [`Side::Left`] and [`Side::Right`], and
//! [`RetranscriptionComparison::divergence_digest`] is computed over the two
//! runs sorted by their own identity, so `compare(a, b)` and `compare(b, a)`
//! carry the same digest. `provider_retranscription_compare` observes that
//! equality, which is what makes "the comparison is not an order" executable
//! rather than asserted.
//!
//! **What is not reported** is which one is better. [`ProviderRun`] implements
//! neither `PartialOrd` nor `Ord`, [`RetranscriptionComparison`] implements
//! neither, and there is no accessor named for a winner, a preference, a rank
//! or a score. `crates/transcription/tests/compile_fail/two_runs_are_not_ordered.rs`
//! is the compiler's half.
//!
//! **What this does not claim.** It does not claim a caller cannot invent an
//! order of their own out of the counts below; `academic_model_run::RawScore`'s
//! contract makes the same distinction, and the narrower true statement is the
//! one both pages make. What it claims is that this crate offers none, and that
//! a provider's own confidence number cannot be read out at all.

use academic_domain::ContentDigest;
use academic_model_run::{ModelVersion, ProviderId};

use crate::{authorize::be_len, response::RawResponseId, transcript::RawTranscript};

/// Why two transcripts could not be compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompareFault {
    /// The two runs read different inputs, so a difference between them says
    /// nothing about the providers.
    #[error("the two runs read different inputs")]
    DifferentInputs,
    /// The two runs are the same provider and the same model version.
    #[error("a run compared against itself measures nothing")]
    SameRun,
}

/// Which of the two runs a divergence came from.
///
/// Deliberately not `Ord`: naming a side is not ranking it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The run passed first.
    Left,
    /// The run passed second.
    Right,
}

impl Side {
    /// Exhaustive order of the two names, which is a listing and not a
    /// comparison.
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
        }
    }
}

/// One run's identity in a comparison.
///
/// Implements no ordering. Two runs are things to diff, not things to sort.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderRun {
    provider: ProviderId,
    model_version: ModelVersion,
    raw_response: RawResponseId,
}

impl ProviderRun {
    /// Which provider.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// The archived raw response it was decoded from, which is still there.
    #[must_use]
    pub const fn raw_response(&self) -> RawResponseId {
        self.raw_response
    }

    /// The identity bytes the symmetric digest sorts on.
    fn identity(&self) -> Vec<u8> {
        let mut material = Vec::new();
        material.extend_from_slice(&be_len(self.provider.as_str().len()));
        material.extend_from_slice(self.provider.as_str().as_bytes());
        material.extend_from_slice(&be_len(self.model_version.as_str().len()));
        material.extend_from_slice(self.model_version.as_str().as_bytes());
        material.extend_from_slice(&self.raw_response.value().to_be_bytes());
        material
    }
}

/// One place the two runs disagree.
///
/// The alignment is positional: segment *i* against segment *i*, token *j*
/// against token *j*. That is deterministic and it is not a sequence aligner --
/// an insertion at the front of a segment reports every token after it as
/// divergent. What the report is for is telling a reader **where to listen**,
/// and `P2-L4`'s coverage validator is where mapping quality is judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Divergence {
    /// The two runs returned different numbers of segments.
    SegmentCount {
        /// How many the left run returned.
        left: usize,
        /// How many the right run returned.
        right: usize,
    },
    /// One segment covers a different interval on the two sides.
    SegmentInterval {
        /// Which segment.
        segment: usize,
    },
    /// One segment attributes speech to a different speaker.
    Speaker {
        /// Which segment.
        segment: usize,
    },
    /// One segment holds a different number of tokens.
    TokenCount {
        /// Which segment.
        segment: usize,
        /// How many the left run returned.
        left: usize,
        /// How many the right run returned.
        right: usize,
    },
    /// Two tokens at the same position read differently.
    TokenText {
        /// Which segment.
        segment: usize,
        /// Which position inside it.
        position: usize,
    },
}

impl Divergence {
    /// Stable spelling of the kind.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SegmentCount { .. } => "SEGMENT_COUNT",
            Self::SegmentInterval { .. } => "SEGMENT_INTERVAL",
            Self::Speaker { .. } => "SPEAKER",
            Self::TokenCount { .. } => "TOKEN_COUNT",
            Self::TokenText { .. } => "TOKEN_TEXT",
        }
    }

    /// The bytes the symmetric digest covers.
    ///
    /// A count carries both sides' numbers, so the two are sorted before
    /// hashing: swapping the arguments to [`compare`] must not move the digest.
    fn digest_material(&self) -> Vec<u8> {
        let mut material = Vec::new();
        material.extend_from_slice(self.as_str().as_bytes());
        material.push(0);
        match self {
            Self::SegmentCount { left, right } => {
                let (low, high) = if left <= right {
                    (left, right)
                } else {
                    (right, left)
                };
                material.extend_from_slice(&be_len(*low));
                material.extend_from_slice(&be_len(*high));
            }
            Self::SegmentInterval { segment } | Self::Speaker { segment } => {
                material.extend_from_slice(&be_len(*segment));
            }
            Self::TokenCount {
                segment,
                left,
                right,
            } => {
                material.extend_from_slice(&be_len(*segment));
                let (low, high) = if left <= right {
                    (left, right)
                } else {
                    (right, left)
                };
                material.extend_from_slice(&be_len(*low));
                material.extend_from_slice(&be_len(*high));
            }
            Self::TokenText { segment, position } => {
                material.extend_from_slice(&be_len(*segment));
                material.extend_from_slice(&be_len(*position));
            }
        }
        material
    }
}

/// What two runs of the same audio said, and where they differ.
///
/// Implements no ordering, holds no verdict, and has no accessor naming a
/// winner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetranscriptionComparison {
    left: ProviderRun,
    right: ProviderRun,
    input_digest: ContentDigest,
    divergences: Vec<Divergence>,
    agreeing_tokens: usize,
    compared_tokens: usize,
}

impl RetranscriptionComparison {
    /// The run passed first.
    #[must_use]
    pub const fn left(&self) -> &ProviderRun {
        &self.left
    }

    /// The run passed second.
    #[must_use]
    pub const fn right(&self) -> &ProviderRun {
        &self.right
    }

    /// The input both runs read. Equal by construction.
    #[must_use]
    pub const fn input_digest(&self) -> &ContentDigest {
        &self.input_digest
    }

    /// Every place the two disagree, in segment then position order.
    #[must_use]
    pub fn divergences(&self) -> &[Divergence] {
        &self.divergences
    }

    /// How many aligned token positions read the same on both sides.
    #[must_use]
    pub const fn agreeing_tokens(&self) -> usize {
        self.agreeing_tokens
    }

    /// How many token positions were aligned at all.
    #[must_use]
    pub const fn compared_tokens(&self) -> usize {
        self.compared_tokens
    }

    /// A digest over the comparison that does not depend on which run was
    /// passed first.
    ///
    /// The two runs' identities are sorted before hashing and every divergence
    /// that carries both sides' numbers sorts them too. This is what
    /// `provider_retranscription_compare` uses to observe that the comparison
    /// carries no order.
    #[must_use]
    pub fn divergence_digest(&self) -> ContentDigest {
        let mut identities = [self.left.identity(), self.right.identity()];
        identities.sort_unstable();
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-transcription-retranscription-v1\0");
        for identity in &identities {
            material.extend_from_slice(&be_len(identity.len()));
            material.extend_from_slice(identity);
        }
        material.extend_from_slice(self.input_digest.as_bytes());
        material.extend_from_slice(&be_len(self.divergences.len()));
        for divergence in &self.divergences {
            let bytes = divergence.digest_material();
            material.extend_from_slice(&be_len(bytes.len()));
            material.extend_from_slice(&bytes);
        }
        ContentDigest::sha256(&material)
    }
}

/// Compares two transcriptions of the same audio.
///
/// # Errors
///
/// [`CompareFault::DifferentInputs`] when the two runs did not read the same
/// input manifest, and [`CompareFault::SameRun`] when they name the same
/// provider and model version.
pub fn compare(
    left: &RawTranscript,
    right: &RawTranscript,
) -> Result<RetranscriptionComparison, CompareFault> {
    if left.input_digest() != right.input_digest() {
        return Err(CompareFault::DifferentInputs);
    }
    if left.provider() == right.provider() && left.model_version() == right.model_version() {
        return Err(CompareFault::SameRun);
    }

    let mut divergences = Vec::new();
    if left.segments().len() != right.segments().len() {
        divergences.push(Divergence::SegmentCount {
            left: left.segments().len(),
            right: right.segments().len(),
        });
    }
    let mut agreeing_tokens = 0_usize;
    let mut compared_tokens = 0_usize;
    let shared_segments = left.segments().len().min(right.segments().len());
    for segment in 0..shared_segments {
        let (Some(one), Some(other)) =
            (left.segments().get(segment), right.segments().get(segment))
        else {
            continue;
        };
        if one.start_nanos() != other.start_nanos() || one.end_nanos() != other.end_nanos() {
            divergences.push(Divergence::SegmentInterval { segment });
        }
        if one.speaker() != other.speaker() {
            divergences.push(Divergence::Speaker { segment });
        }
        if one.tokens().len() != other.tokens().len() {
            divergences.push(Divergence::TokenCount {
                segment,
                left: one.tokens().len(),
                right: other.tokens().len(),
            });
        }
        let shared_tokens = one.tokens().len().min(other.tokens().len());
        for position in 0..shared_tokens {
            let (Some(a), Some(b)) = (one.tokens().get(position), other.tokens().get(position))
            else {
                continue;
            };
            compared_tokens = compared_tokens.saturating_add(1);
            if a.text() == b.text() {
                agreeing_tokens = agreeing_tokens.saturating_add(1);
            } else {
                divergences.push(Divergence::TokenText { segment, position });
            }
        }
    }

    Ok(RetranscriptionComparison {
        left: ProviderRun {
            provider: left.provider().clone(),
            model_version: left.model_version().clone(),
            raw_response: left.raw_response(),
        },
        right: ProviderRun {
            provider: right.provider().clone(),
            model_version: right.model_version().clone(),
            raw_response: right.raw_response(),
        },
        input_digest: *left.input_digest(),
        divergences,
        agreeing_tokens,
        compared_tokens,
    })
}
