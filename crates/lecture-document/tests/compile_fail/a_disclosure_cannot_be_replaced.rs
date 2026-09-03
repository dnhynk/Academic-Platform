//! The disclosure is a constant a study index carries, not a value it was
//! given. There is no setter and no constructor parameter.

use academic_lecture_document::StudyIndex;

fn replace(index: &mut StudyIndex) {
    index.set_disclosure("this is the lecture document");
}

fn main() {}
