//! Section 16.2: `Course 수강은 concept 획득 그 자체가 아니라 여러
//! exposure/practice 기회를 묶은 acquisition option이다`. An option hands out
//! occasions and answers no state question, so there is no function from one to
//! a mastery level.

use academic_critical_path::AcquisitionOption;
use academic_domain::MasteryLevel;

fn acquired(option: &AcquisitionOption) -> MasteryLevel {
    option.mastery()
}

fn main() {}
