use academic_consent::{
    AttestationKind, AttestationRecord, AuthorityGrant, PermittedUse, RetentionBound,
    RetentionTerms,
};
use academic_domain::ContentDigest;

fn main() {
    let heard = AttestationRecord::file(
        AttestationKind::OralInstructorPermission,
        0,
        ContentDigest::sha256(b"the instructor said it was fine"),
    );
    let _grant = AuthorityGrant::record(
        heard,
        PermittedUse::new(Vec::new(), Vec::new(), false, false),
        RetentionTerms::new(RetentionBound::Prohibited, RetentionBound::Prohibited),
        Vec::new(),
        1,
    );
}
