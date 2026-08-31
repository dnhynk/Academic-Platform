use academic_admission::{Posture, VerifiedAdmission};

fn main() {
    let forged = VerifiedAdmission {};
    let _posture = Posture::from_verified(&forged);
}
