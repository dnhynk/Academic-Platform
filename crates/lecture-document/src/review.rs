//! Section 12.6's review queue: equations, code, and low-confidence spans, each
//! with the audio it came from.
//!
//! # Three classes, and the queue is not the whole transcript
//!
//! `REQ-04-005`'s failure mode has two sides — a hard error nobody looked at,
//! and a queue so large nobody looks at any of it — so the classification is a
//! closed set of three and a paragraph that is none of them does not enter.
//!
//! # A queue item without audio is not a value
//!
//! [`ReviewItem`] carries an [`AudioLocator`] by value, and an
//! [`AudioLocator`] with no chunk is refused at construction. "Never orphaned
//! text only" is therefore a shape rather than an assertion: there is no review
//! item that does not name the audio it came from.
//!
//! # Why a raw confidence is not compared here
//!
//! `P2-M1` says a provider's raw number has no readable units and no ordering.
//! So a low-confidence span is decided by
//! `CalibrationRegistry::interpret` against a configured permille, and a token
//! whose score has *no* usable dataset enters the queue rather than passing it:
//! having a number nobody can read is a reason to look, not a reason to trust.
//! A provider that declared segment-level confidence produces no token score at
//! all, and that is not a low-confidence signal — there is no number.

use academic_domain::ConfidencePermille;
use academic_model_run::{CalibrationRegistry, Purpose};
use academic_transcription::TranscriptLineage;

use crate::{
    config::CoverageConfig,
    document::{LectureDocument, NodeId, NodeKind},
    fault::CoverageFault,
};

/// Why one span is in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskClass {
    /// Section 34.1's equation row.
    Equation,
    /// Section 34.1's code row.
    Code,
    /// A calibrated confidence at or below the configured permille, or a
    /// provider number no registered dataset can read.
    LowConfidence,
}

impl RiskClass {
    /// Every class, in the order section 12.6 lists them.
    pub const ALL: [Self; 3] = [Self::Equation, Self::Code, Self::LowConfidence];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Equation => "EQUATION",
            Self::Code => "CODE",
            Self::LowConfidence => "LOW_CONFIDENCE",
        }
    }
}

/// Where in the original recording a span came from.
///
/// One producer, and it refuses an empty chunk list. Section 12.6 wants the
/// original audio beside the span, and a locator naming no chunk is a locator
/// that does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioLocator {
    chunk_frame_seqs: Vec<u32>,
    start_nanos: u64,
    end_nanos: u64,
}

impl AudioLocator {
    /// The journal frames the span was transcribed from.
    #[must_use]
    pub fn chunk_frame_seqs(&self) -> &[u32] {
        &self.chunk_frame_seqs
    }

    /// When the span starts.
    #[must_use]
    pub const fn start_nanos(&self) -> u64 {
        self.start_nanos
    }

    /// When it ends.
    #[must_use]
    pub const fn end_nanos(&self) -> u64 {
        self.end_nanos
    }
}

/// One span in front of a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewItem {
    node: NodeId,
    class: RiskClass,
    segment_index: usize,
    audio: AudioLocator,
    nearby_captures: Vec<u32>,
    calibrated_permille: Option<u16>,
}

impl ReviewItem {
    /// Which node.
    #[must_use]
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Why it is here.
    #[must_use]
    pub const fn class(&self) -> RiskClass {
        self.class
    }

    /// Which segment.
    #[must_use]
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    /// The original audio. Always present.
    #[must_use]
    pub const fn audio(&self) -> &AudioLocator {
        &self.audio
    }

    /// The captures placed beside it, where there are any.
    #[must_use]
    pub fn nearby_captures(&self) -> &[u32] {
        &self.nearby_captures
    }

    /// The calibrated confidence that put it here, when one could be read.
    #[must_use]
    pub const fn calibrated_permille(&self) -> Option<u16> {
        self.calibrated_permille
    }
}

/// Section 12.6's review queue over one document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewQueue {
    items: Vec<ReviewItem>,
}

impl ReviewQueue {
    /// Every item, in document order.
    #[must_use]
    pub fn items(&self) -> &[ReviewItem] {
        &self.items
    }

    /// The items of one class.
    #[must_use]
    pub fn of(&self, class: RiskClass) -> Vec<&ReviewItem> {
        self.items
            .iter()
            .filter(|item| item.class == class)
            .collect()
    }

    /// Builds the queue for one document.
    ///
    /// `now` is an argument because this crate reads no clock: the caller names
    /// the instant a calibration dataset's freshness is judged against, exactly
    /// as `academic-capture` and `academic-consent` do.
    ///
    /// # Errors
    ///
    /// [`CoverageFault`] when a mapped segment does not exist at the document's
    /// version, or when a risky span's segment names no audio chunk — the
    /// second is the fail-closed half of "never orphaned text only".
    pub fn build(
        document: &LectureDocument,
        lineage: &TranscriptLineage,
        calibration: &CalibrationRegistry,
        purpose: &Purpose,
        now: u64,
        config: CoverageConfig,
    ) -> Result<Self, CoverageFault> {
        let threshold = config.low_confidence_at_or_below_permille();
        let mut items = Vec::new();
        for node in document.nodes() {
            for mapping in node.mappings() {
                let segment_index = mapping.segment_index();
                let segment = lineage
                    .segment_at(document.version(), segment_index)
                    .ok_or(CoverageFault::DispositionForNoSuchSegment(segment_index))?;
                let mut lowest: Option<u16> = None;
                let mut unreadable = false;
                for position in mapping.covered_tokens() {
                    let Some(token) = segment.tokens().get(*position) else {
                        continue;
                    };
                    let Some(score) = token.raw().confidence() else {
                        continue;
                    };
                    match calibration.interpret(score, purpose, now) {
                        Ok(calibrated) => {
                            let permille = ConfidencePermille::value(calibrated.confidence());
                            lowest = Some(match lowest {
                                Some(previous) => previous.min(permille),
                                None => permille,
                            });
                        }
                        Err(_) => unreadable = true,
                    }
                }
                let low = unreadable || lowest.is_some_and(|permille| permille <= threshold);
                let class = match node.kind() {
                    NodeKind::Equation => Some(RiskClass::Equation),
                    NodeKind::CodeBlock => Some(RiskClass::Code),
                    NodeKind::Section | NodeKind::Paragraph | NodeKind::CapturePlacement => {
                        low.then_some(RiskClass::LowConfidence)
                    }
                };
                let Some(class) = class else {
                    continue;
                };
                if segment.source_audio_chunks().is_empty() {
                    return Err(CoverageFault::DispositionForNoSuchSegment(segment_index));
                }
                items.push(ReviewItem {
                    node: node.id().clone(),
                    class,
                    segment_index,
                    audio: AudioLocator {
                        chunk_frame_seqs: segment.source_audio_chunks().to_vec(),
                        start_nanos: segment.start_nanos(),
                        end_nanos: segment.end_nanos(),
                    },
                    nearby_captures: node.nearby_captures().to_vec(),
                    calibrated_permille: if unreadable { None } else { lowest },
                });
            }
        }
        Ok(Self { items })
    }
}
