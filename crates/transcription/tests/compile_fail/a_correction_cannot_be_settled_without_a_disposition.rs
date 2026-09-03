//! A settled correction is one `P2-M2` recorded, and there is no fourth
//! disposition.
//!
//! `SettledCorrection` has private fields and two constructors, one per
//! appending disposition. A rejection produces no value at all, and a caller
//! cannot assemble one to stand in for it.

use academic_transcription::{
    CorrectionAuthor, CorrectionCandidate, SettledCorrection, TokenAddress,
};

fn forge(candidate: CorrectionCandidate) -> SettledCorrection {
    SettledCorrection {
        proposal: academic_proposal::ProposalId::new(1),
        candidate,
        author: CorrectionAuthor::ConfirmedModelCandidate,
    }
}

fn forge_candidate() -> CorrectionCandidate {
    CorrectionCandidate {
        address: TokenAddress::new(0, 0),
        replacement_text: "anything".to_owned(),
    }
}

fn main() {}
