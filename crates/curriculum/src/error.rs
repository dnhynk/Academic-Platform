//! Every way a curriculum value refuses to be built or published.

use thiserror::Error;

/// A refusal from this crate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CurriculumError {
    /// A course code, title, term, or section string is outside its grammar.
    #[error("{field} is not a well-formed {field}: {reason}")]
    Malformed {
        /// Which value.
        field: &'static str,
        /// Why it was refused.
        reason: &'static str,
    },
    /// A draft reached `build` with a required attribute still unset.
    #[error("{aggregate} draft is missing {field}")]
    Missing {
        /// Which aggregate's draft.
        aggregate: &'static str,
        /// Which attribute is unset.
        field: &'static str,
    },
    /// A relation names the same course on both ends.
    #[error("a {relation} relation names one course on both ends")]
    Reflexive {
        /// Which relation.
        relation: &'static str,
    },
    /// A published aggregate names a parent that was not published beside it.
    #[error("{child} names {parent}, which this publication does not carry")]
    Dangling {
        /// The aggregate that names a missing parent.
        child: &'static str,
        /// The parent kind that is absent.
        parent: &'static str,
    },
    /// The same aggregate identity was published twice in one publication.
    #[error("{aggregate} is published twice in one publication")]
    Duplicate {
        /// Which aggregate kind.
        aggregate: &'static str,
    },
    /// A publish checkpoint was failed by an injected fault.
    #[error("injected publish fault at {0}")]
    InjectedFault(&'static str),
}
