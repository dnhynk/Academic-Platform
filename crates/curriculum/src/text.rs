//! The bounded strings section 8.2 puts on the three aggregates.
//!
//! Each is restricted the way `academic_ingestion::identifier` restricts a name
//! read out of an official document, and for the same reason: these values are
//! transcribed from a university catalogue, and a name lifted out of one must
//! not be able to carry a directive or a separator.
//!
//! There is no `Default` and no `From<String>` on any of them. A value arrives
//! through `parse` or it does not arrive.

use crate::error::CurriculumError;

/// Longest value any of these types admits.
const MAX_LEN: usize = 128;

fn bounded(field: &'static str, value: &str) -> Result<(), CurriculumError> {
    if value.is_empty() {
        return Err(CurriculumError::Malformed {
            field,
            reason: "it is empty",
        });
    }
    if value.len() > MAX_LEN {
        return Err(CurriculumError::Malformed {
            field,
            reason: "it is longer than 128 bytes",
        });
    }
    Ok(())
}

macro_rules! restricted_text {
    ($name:ident, $field:literal, $doc:literal, $admits:expr, $reason:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The only constructor.
            pub fn parse(value: &str) -> Result<Self, CurriculumError> {
                bounded($field, value)?;
                let admits: fn(char) -> bool = $admits;
                if !value.chars().all(admits) {
                    return Err(CurriculumError::Malformed {
                        field: $field,
                        reason: $reason,
                    });
                }
                Ok(Self(value.to_owned()))
            }

            /// The stored value.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

restricted_text!(
    CourseCode,
    "course code",
    "Section 8.2's `courseCode`, as the catalogue prints it (`M1522.001800`).\n\n\
     A course code is a label, never an identity: two occurrences of one code\n\
     are the same course only when [`crate::relation::IdentityDecision`] says\n\
     so. See [`crate::relation::CourseCodeReuse`].",
    |character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'),
    "a course code admits only ASCII letters, digits, '.', '-' and '_'"
);

restricted_text!(
    CourseTitle,
    "course title",
    "Section 8.2's `titleKo`. Korean and Latin letters, digits, spaces, and a\n\
     small punctuation set; no control characters and no line separators.",
    |character| {
        character.is_alphanumeric()
            || matches!(
                character,
                ' ' | '(' | ')' | ',' | '·' | '-' | '/' | ':' | '.'
            )
    },
    "a course title admits letters, digits, spaces and ( ) , · - / : ."
);

restricted_text!(
    TermCode,
    "term code",
    "Section 8.2's `term` (`2026_FALL`). The academic term a section runs in.",
    |character| character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_',
    "a term code admits only ASCII upper-case letters, digits and '_'"
);

restricted_text!(
    SectionCode,
    "section code",
    "Section 8.2's `section`. The division within one term's offering.",
    |character| character.is_ascii_alphanumeric() || character == '-',
    "a section code admits only ASCII letters, digits and '-'"
);

restricted_text!(
    InstructorName,
    "instructor name",
    "One entry of section 8.2's `instructors`. This is offering reality: it is\n\
     admitted on [`crate::offering::CourseOffering`] and on nothing else.",
    |character| character.is_alphanumeric() || matches!(character, ' ' | '.' | '-'),
    "an instructor name admits letters, digits, spaces, '.' and '-'"
);

restricted_text!(
    AdmissionCohort,
    "admission cohort",
    "A 학번 as section 8.1 writes it (`2026`). The unit a curriculum version\n\
     applies to and the unit a transition arrangement moves.",
    |character| character.is_ascii_digit(),
    "an admission cohort admits only ASCII digits"
);
