//! The three channels, and why ingested bytes reach only one of them.
//!
//! A prompt this crate renders has four kinds of region and one rule: ingested
//! bytes occupy [`ChannelKind::Data`] regions and nothing else.
//!
//! # Why the instruction channels take `&'static str`
//!
//! [`SystemDirective::new`] and [`ToolDirective::new`] take `&'static str`.
//! Ingested content arrives at run time as owned bytes, and [`crate::Untrusted`]
//! hands out no `&str` and no `String` outside this crate, so there is no value
//! a caller could turn into the `&'static str` those constructors want. The
//! remaining route -- leaking an allocation to get a `'static` borrow -- needs a
//! `String` to leak, which is the same thing the wrapper does not give up.
//!
//! ```compile_fail
//! # use academic_untrusted_content::{IngestedDocument, SystemDirective, Untrusted};
//! fn promote(document: &Untrusted<IngestedDocument>) -> SystemDirective {
//!     SystemDirective::new(document)
//! }
//! ```
//!
//! # Why a data record is one line of ASCII
//!
//! A fenced block can be closed early by content that spells the fence, and a
//! delimiter chosen to avoid today's content is chosen against the wrong
//! adversary. So a record is not fenced: it is escaped. Every byte outside
//! printable ASCII, and both of `"` and `\`, is rewritten as a `\uXXXX` escape,
//! which leaves a record that
//!
//! - contains no line terminator, so it cannot open a line of its own;
//! - contains no unescaped quote, so it cannot close the field it sits in; and
//! - is pure ASCII, so a bidirectional override, a zero-width joiner, or a
//!   homoglyph is visible as an escape rather than as the character it imitates.
//!
//! All three are properties of [`escape`]'s `match`, which is a total case
//! analysis over `char`, rather than of any particular input.
//! `taint_flow_test_keeps_untrusted_spans_in_data_channel` observes them over
//! the whole injection corpus, and observes that the bytes before the first data
//! record are identical for every corpus entry; `WHOLE_ESCAPE` in
//! `tests/trust_scans.rs` is what stops the analysis changing quietly.

use core::fmt;

use crate::{
    ingest::IngestedDocument,
    label::{Provenance, Untrusted},
};

/// The format identifier a rendered prompt opens with.
pub const PROMPT_FORMAT: &str = "academic-prompt/1";

/// Which channel a region of a rendered prompt belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChannelKind {
    /// The format line and the three channel labels. Trusted, and fixed.
    Structure,
    /// A system instruction. Trusted; `&'static str` by construction.
    System,
    /// A tool instruction. Trusted; `&'static str` by construction.
    ToolInstruction,
    /// One quoted document. The only channel ingested bytes reach.
    Data,
}

impl ChannelKind {
    /// Exhaustive order.
    pub const ALL: [Self; 4] = [
        Self::Structure,
        Self::System,
        Self::ToolInstruction,
        Self::Data,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Structure => "STRUCTURE",
            Self::System => "SYSTEM",
            Self::ToolInstruction => "TOOL_INSTRUCTION",
            Self::Data => "DATA",
        }
    }
}

/// A trusted system instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemDirective(&'static str);

impl SystemDirective {
    /// Takes a compile-time constant.
    #[must_use]
    pub const fn new(text: &'static str) -> Self {
        Self(text)
    }

    /// The constant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// A trusted tool instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDirective(&'static str);

impl ToolDirective {
    /// Takes a compile-time constant.
    #[must_use]
    pub const fn new(text: &'static str) -> Self {
        Self(text)
    }

    /// The constant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// One quoted document, ready to render.
#[derive(Clone, PartialEq, Eq)]
pub struct QuotedDocument {
    provenance: Provenance,
    digest: String,
    byte_len: usize,
    escaped: String,
}

impl fmt::Debug for QuotedDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotedDocument")
            .field("provenance", &self.provenance)
            .field("digest", &self.digest)
            .field("byte_len", &self.byte_len)
            .field(
                "escaped",
                &format_args!("<untrusted:{} bytes>", self.escaped.len()),
            )
            .finish()
    }
}

impl QuotedDocument {
    /// Where the quoted bytes came from.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Digest of the unescaped source bytes.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Rewrites `text` so it can occupy one ASCII line inside a quoted field.
///
/// Printable ASCII other than `"` and `\` passes through. Everything else --
/// every control byte, every non-ASCII scalar, and the two structural
/// characters -- becomes a `\uXXXX` escape, astral scalars as a surrogate pair.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            ' '..='~' => escaped.push(character),
            other => {
                let mut units = [0_u16; 2];
                for unit in other.encode_utf16(&mut units) {
                    escaped.push_str("\\u");
                    for shift in [12_u32, 8, 4, 0] {
                        const DIGITS: &[u8; 16] = b"0123456789abcdef";
                        let nibble = usize::from((*unit >> shift) & 0x000f);
                        escaped.push(char::from(DIGITS[nibble]));
                    }
                }
            }
        }
    }
    escaped
}

/// One region of a rendered prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    kind: ChannelKind,
    start: usize,
    end: usize,
    provenance: Option<Provenance>,
}

impl Segment {
    /// Which channel.
    #[must_use]
    pub const fn kind(&self) -> ChannelKind {
        self.kind
    }

    /// Inclusive start offset in the rendered text.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Exclusive end offset in the rendered text.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// The document this region quotes, when it quotes one.
    #[must_use]
    pub const fn provenance(&self) -> Option<&Provenance> {
        self.provenance.as_ref()
    }
}

/// The half-open range of one document's escaped bytes in a rendered prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntrustedSpan {
    provenance: Provenance,
    start: usize,
    end: usize,
}

impl UntrustedSpan {
    /// Which document.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Inclusive start offset in the rendered text.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Exclusive end offset in the rendered text.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }
}

/// A rendered prompt and the map of what came from where.
#[derive(Clone, PartialEq, Eq)]
pub struct RenderedPrompt {
    text: String,
    segments: Vec<Segment>,
    untrusted: Vec<UntrustedSpan>,
    instruction_end: usize,
}

impl fmt::Debug for RenderedPrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RenderedPrompt")
            .field("segments", &self.segments)
            .field("untrusted", &self.untrusted)
            .field("instruction_end", &self.instruction_end)
            .field(
                "text",
                &format_args!("<holds untrusted spans:{} bytes>", self.text.len()),
            )
            .finish()
    }
}

impl RenderedPrompt {
    /// The rendered bytes.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Every region, in order, partitioning the rendered text.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The range of each quoted document's escaped bytes.
    #[must_use]
    pub fn untrusted_spans(&self) -> &[UntrustedSpan] {
        &self.untrusted
    }

    /// The exclusive end of everything before the first data record.
    ///
    /// This is the region a promotion would have to reach to become an
    /// instruction, and it is what
    /// `taint_flow_test_keeps_untrusted_spans_in_data_channel` compares across
    /// corpus entries.
    #[must_use]
    pub const fn instruction_end(&self) -> usize {
        self.instruction_end
    }

    /// The bytes before the first data record.
    #[must_use]
    pub fn instruction_region(&self) -> &str {
        self.text.get(..self.instruction_end).unwrap_or_default()
    }
}

/// The channels of one prompt, before rendering.
#[derive(Debug, Default)]
pub struct PromptEnvelope {
    system: Vec<SystemDirective>,
    tools: Vec<ToolDirective>,
    data: Vec<QuotedDocument>,
}

impl PromptEnvelope {
    /// An envelope with no directive and no data.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            system: Vec::new(),
            tools: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Appends a trusted system instruction.
    pub fn push_system(&mut self, directive: SystemDirective) {
        self.system.push(directive);
    }

    /// Appends a trusted tool instruction.
    pub fn push_tool(&mut self, directive: ToolDirective) {
        self.tools.push(directive);
    }

    /// Appends a document to the data channel.
    ///
    /// This is the crate's first exposure site: it reads the wrapped text so it
    /// can escape it. What comes back out is escaped, one line, and pure ASCII,
    /// and its range in the rendered prompt is recorded as untrusted.
    pub fn quote(&mut self, document: &Untrusted<IngestedDocument>) {
        let inner = document.expose();
        self.data.push(QuotedDocument {
            provenance: document.provenance().clone(),
            digest: document.digest().to_owned(),
            byte_len: inner.byte_len(),
            escaped: escape(inner.text()),
        });
    }

    /// How many documents are quoted.
    #[must_use]
    pub fn quoted_len(&self) -> usize {
        self.data.len()
    }

    /// Renders the prompt and the map of what came from where.
    #[must_use]
    pub fn render(&self) -> RenderedPrompt {
        let mut text = String::new();
        let mut segments = Vec::new();
        let mut untrusted = Vec::new();
        let push = |text: &mut String, segments: &mut Vec<Segment>, kind, line: &str| {
            let start = text.len();
            text.push_str(line);
            text.push('\n');
            segments.push(Segment {
                kind,
                start,
                end: text.len(),
                provenance: None,
            });
        };

        push(
            &mut text,
            &mut segments,
            ChannelKind::Structure,
            PROMPT_FORMAT,
        );
        push(&mut text, &mut segments, ChannelKind::Structure, "[SYSTEM]");
        for directive in &self.system {
            push(
                &mut text,
                &mut segments,
                ChannelKind::System,
                directive.as_str(),
            );
        }
        push(&mut text, &mut segments, ChannelKind::Structure, "[TOOLS]");
        for directive in &self.tools {
            push(
                &mut text,
                &mut segments,
                ChannelKind::ToolInstruction,
                directive.as_str(),
            );
        }
        push(&mut text, &mut segments, ChannelKind::Structure, "[DATA]");
        let instruction_end = text.len();

        for document in &self.data {
            let start = text.len();
            text.push_str("{\"id\":\"");
            text.push_str(document.provenance.source_id().as_str());
            text.push_str("\",\"kind\":\"");
            text.push_str(document.provenance.kind().as_str());
            text.push_str("\",\"seq\":");
            text.push_str(&document.provenance.ingest_seq().to_string());
            text.push_str(",\"sha256\":\"");
            text.push_str(&document.digest);
            text.push_str("\",\"bytes\":");
            text.push_str(&document.byte_len.to_string());
            text.push_str(",\"content\":\"");
            let content_start = text.len();
            text.push_str(&document.escaped);
            let content_end = text.len();
            text.push_str("\"}");
            text.push('\n');
            segments.push(Segment {
                kind: ChannelKind::Data,
                start,
                end: text.len(),
                provenance: Some(document.provenance.clone()),
            });
            untrusted.push(UntrustedSpan {
                provenance: document.provenance.clone(),
                start: content_start,
                end: content_end,
            });
        }

        RenderedPrompt {
            text,
            segments,
            untrusted,
            instruction_end,
        }
    }
}
