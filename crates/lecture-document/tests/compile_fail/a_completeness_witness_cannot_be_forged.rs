//! The witness has private fields and one producer,
//! `CoverageReport::completeness_witness`, so "the document is complete" is not
//! a claim a caller can make.

use academic_domain::ContentDigest;
use academic_lecture_document::CompletenessWitness;

fn main() {
    let _ = CompletenessWitness {
        report_digest: ContentDigest::sha256(b"anything"),
    };
}
