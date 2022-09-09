//!
//! The Yul IR error.
//!

use crate::yul::lexer::error::Error as LexerError;
use crate::yul::parser::error::Error as ParserError;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    /// The lexer error.
    #[error("Lexical error: {0}")]
    Lexer(#[from] LexerError),
    /// The parser error.
    #[error("Syntax error: {0}")]
    Parser(#[from] ParserError),
}