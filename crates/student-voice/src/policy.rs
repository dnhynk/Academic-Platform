//! The redaction policy `P2-L4` cites and this crate resolves, and the scope
//! it has no value for.
//!
//! # `D-3`, closed
//!
//! `docs/contracts/lecture-document.md` leaves `D-3` open: a
//! `RedactionPolicyRef` holds a digest that crate does not resolve, and "what
//! the digest names, and whether the redaction it authorises actually happened,
//! is `P2-L5`'s". [`RedactionPolicy::digest`] is what that digest is, and
//! [`RedactionPolicy::resolves`] is the comparison. A reference citing anything
//! else is refused at the point a derivative would be built, so a redaction
//! carrying a policy nobody can produce is not a value.
//!
//! # `GATE-38-026` is open, and the shape of it is that there is no variant
//!
//! Whether student voices may be removed from an **original** is an open user
//! and institution decision. This crate does not answer it, and the way it does
//! not answer it is that [`RedactionScope`] has one variant. There is no
//! `Original`, so a policy authorising removal from an original has no
//! spelling here at all -- the same shape as `AutomaticLevel` having no
//! `FLUENT` in `P2-N2` and `AuthorshipMode` having no review value in `P2-R5`.
//!
//! `academic-retention` holds the *mechanism* for a voice-scoped deletion of an
//! original, behind an `OriginalVoiceAuthority` a caller has to state. This
//! crate never produces one, never names one in product source, and
//! `no_original_voice_authority_is_produced_here` measures both directions.
//!
//! # Who may decide
//!
//! A redaction is a judgement about what was said and who said it, and section
//! 27.2 does not let a model make one. Every constructor here matches
//! `academic-domain`'s closed `Actor` exhaustively, so a fifth actor class
//! stops this crate compiling until it is classified. That is `P2-L4`'s
//! `NonSpeechEvidence::declared` reused rather than restated as a comparison.

use academic_domain::{Actor, ContentDigest};
use academic_lecture_document::{RedactionBasis, RedactionPolicyRef};
use academic_transcription::Speaker;

use crate::{corpus::VoiceClass, fault::RedactionFault};

/// What a redaction is allowed to produce.
///
/// One variant. `GATE-38-026` stays open because there is nothing here that
/// could close it: a policy that removes speech from an original is not a value
/// this crate can build, whatever a caller writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RedactionScope {
    /// A local derivative beside the original, which is unchanged.
    DerivativeOnly,
}

impl RedactionScope {
    /// Every scope this build has.
    pub const ALL: [Self; 1] = [Self::DerivativeOnly];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DerivativeOnly => "DERIVATIVE_ONLY",
        }
    }
}

/// What `GATE-38-026` leaves open, stated where the policy lives.
///
/// `academic-retention`'s `GATE_38_026_STATEMENT` says the same thing where the
/// mechanism lives. `the_open_gate_is_stated_on_both_sides` compares the two so
/// neither can quietly start claiming the question is settled.
pub const GATE_38_026_OPEN: &str = "whether student voices may be removed from an original lecture recording is an open \
     decision for the user and the institution (GATE-38-026); this build measures diarization \
     accuracy, redacts into a derivative only, and selects no policy for originals";

/// Which speakers a policy targets.
///
/// Two shapes and no third. `NonInstructorVoices` is section 32.5's own rule --
/// "비교수자 음성" -- and `NamedSpeakers` is a rights request about particular
/// people. There is no "all voices" value, because a derivative excluding the
/// instructor as well is not a lecture derivative, and no "no voices" value,
/// because [`RedactionFault::NoTargets`] is what an empty list is.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpeakerTargeting {
    /// Everyone who is not the instructor, including speech nobody attributed.
    ///
    /// `Unresolved` is targeted deliberately: a fail-closed reader treats an
    /// unattributed span as possibly a student, so it leaves the derivative.
    /// That costs losslessness and it is the direction this task errs in.
    NonInstructorVoices,
    /// The speakers a rights request named.
    NamedSpeakers(Vec<Speaker>),
}

impl SpeakerTargeting {
    /// Whether this targeting covers `speaker`.
    #[must_use]
    pub fn targets(&self, speaker: Speaker) -> bool {
        match self {
            Self::NonInstructorVoices => VoiceClass::of(speaker) != VoiceClass::Instructor,
            Self::NamedSpeakers(named) => named.contains(&speaker),
        }
    }

    /// The stable external spelling of the shape.
    #[must_use]
    pub const fn kind_str(&self) -> &'static str {
        match self {
            Self::NonInstructorVoices => "NON_INSTRUCTOR_VOICES",
            Self::NamedSpeakers(_) => "NAMED_SPEAKERS",
        }
    }
}

/// The policy a redaction rests on, resolved.
///
/// `P2-L4`'s `RedactionPolicyRef` carries a digest, a basis and a deciding
/// actor. This is what that digest names: the same basis, the same actor, plus
/// the two things a reference cannot carry -- who is targeted, and what the
/// redaction is allowed to produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionPolicy {
    version: u32,
    basis: RedactionBasis,
    targeting: SpeakerTargeting,
    scope: RedactionScope,
    decided_by: Actor,
}

impl RedactionPolicy {
    /// Publishes a policy.
    ///
    /// # Errors
    ///
    /// [`RedactionFault::AutomaticActorCannotRedact`] for every automatic
    /// actor, and [`RedactionFault::NoTargets`] for a named list with nothing
    /// in it -- a policy that targets nobody would make every check below
    /// vacuous.
    pub fn published(
        version: u32,
        basis: RedactionBasis,
        targeting: SpeakerTargeting,
        scope: RedactionScope,
        decided_by: Actor,
    ) -> Result<Self, RedactionFault> {
        match &decided_by {
            Actor::User { .. } => {}
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                return Err(RedactionFault::AutomaticActorCannotRedact);
            }
        }
        if matches!(&targeting, SpeakerTargeting::NamedSpeakers(named) if named.is_empty()) {
            return Err(RedactionFault::NoTargets);
        }
        Ok(Self {
            version,
            basis,
            targeting,
            scope,
            decided_by,
        })
    }

    /// Which published policy.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// What it rests on.
    #[must_use]
    pub const fn basis(&self) -> RedactionBasis {
        self.basis
    }

    /// Who it targets.
    #[must_use]
    pub const fn targeting(&self) -> &SpeakerTargeting {
        &self.targeting
    }

    /// What it is allowed to produce.
    #[must_use]
    pub const fn scope(&self) -> RedactionScope {
        self.scope
    }

    /// Who published it.
    #[must_use]
    pub const fn decided_by(&self) -> &Actor {
        &self.decided_by
    }

    /// Whether this policy targets `speaker`.
    #[must_use]
    pub fn targets(&self, speaker: Speaker) -> bool {
        self.targeting.targets(speaker)
    }

    /// The policy's canonical bytes, which its digest is over.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut text = format!("academic-redaction-policy/1 {}\n", self.version);
        text.push_str("basis=");
        text.push_str(self.basis.as_str());
        text.push('\n');
        text.push_str("scope=");
        text.push_str(self.scope.as_str());
        text.push('\n');
        text.push_str("targeting=");
        text.push_str(self.targeting.kind_str());
        text.push('\n');
        if let SpeakerTargeting::NamedSpeakers(named) = &self.targeting {
            for speaker in named {
                text.push_str("speaker=");
                text.push_str(&speaker.spelling());
                text.push('\n');
            }
        }
        text.push_str("decided_by=");
        text.push_str(self.decided_by.kind_name());
        text.push('\n');
        text.into_bytes()
    }

    /// The digest a `P2-L4` `RedactionPolicyRef` cites.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(&self.canonical_bytes())
    }

    /// Whether `reference` cites this policy.
    ///
    /// `D-3`'s closure. The digest is the comparison; the basis comparison
    /// beside it is **defence in depth and nothing more**, and this comment
    /// used to claim otherwise.
    ///
    /// [`Self::canonical_bytes`] writes `basis=` into the digest preimage, so a
    /// reference that agrees on the digest and disagrees on the basis needs a
    /// SHA-256 collision. `P2-A4`'s F9 measured the consequence: deleting
    /// `reference.basis() == self.basis` leaves the suite green, and no test
    /// can drive it, because the state it refuses is unreachable rather than
    /// merely unvisited. It is kept because it costs one comparison and it
    /// stops being redundant the moment the preimage stops carrying the basis
    /// — which is a change to `canonical_bytes`, one function away.
    #[must_use]
    pub fn resolves(&self, reference: &RedactionPolicyRef) -> bool {
        *reference.policy_digest() == self.digest() && reference.basis() == self.basis
    }

    /// Checks that `reference` cites this policy, or says what it cites.
    ///
    /// # Errors
    ///
    /// [`RedactionFault::PolicyReferenceDoesNotResolve`].
    pub fn resolve(&self, reference: &RedactionPolicyRef) -> Result<(), RedactionFault> {
        if self.resolves(reference) {
            return Ok(());
        }
        Err(RedactionFault::PolicyReferenceDoesNotResolve {
            cited: *reference.policy_digest(),
            actual: self.digest(),
        })
    }
}
