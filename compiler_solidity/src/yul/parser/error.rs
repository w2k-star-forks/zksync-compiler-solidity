//!
//! The Yul IR parser error.
//!

use crate::yul::lexer::token::location::Location;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("{location} Expected one of {expected:?}, found `{found}`")]
    InvalidToken {
        /// The invalid token location.
        location: Location,
        /// The list of expected tokens.
        expected: Vec<&'static str>,
        /// The invalid token.
        found: String,
    },
    #[error("{location} The identifier `{identifier}` is reserved")]
    ReservedIdentifier {
        /// The invalid token location.
        location: Location,
        /// The invalid identifier.
        identifier: String,
    },
    #[error("{location} Function `{identifier}` must have {expected} arguments, found {found}")]
    InvalidNumberOfArguments {
        /// The invalid function location.
        location: Location,
        /// The invalid function name.
        identifier: String,
        /// The expected number of arguments.
        expected: usize,
        /// The actual number of arguments.
        found: usize,
    },
}
