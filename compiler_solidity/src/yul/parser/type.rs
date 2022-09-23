//!
//! The YUL source code type.
//!

use crate::yul::error::Error;
use crate::yul::lexer::token::lexeme::keyword::Keyword;
use crate::yul::lexer::token::lexeme::Lexeme;
use crate::yul::lexer::token::Token;
use crate::yul::lexer::Lexer;
use crate::yul::parser::error::Error as ParserError;

///
/// The YUL source code type.
///
/// The type is not currently in use, so all values have the `uint256` type by default.
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// The `bool` type.
    Bool,
    /// The `int{N}` type.
    Int(usize),
    /// The `uint{N}` type.
    UInt(usize),
    /// The custom user-defined type.
    Custom(String),
}

impl Default for Type {
    fn default() -> Self {
        Self::UInt(compiler_common::BITLENGTH_FIELD)
    }
}

impl Type {
    ///
    /// The element parser.
    ///
    pub fn parse(lexer: &mut Lexer, initial: Option<Token>) -> Result<Self, Error> {
        let token = crate::yul::parser::take_or_next(initial, lexer)?;

        match token {
            Token {
                lexeme: Lexeme::Keyword(Keyword::Bool),
                ..
            } => Ok(Self::Bool),
            Token {
                lexeme: Lexeme::Keyword(Keyword::Int(bitlength)),
                ..
            } => Ok(Self::Int(bitlength)),
            Token {
                lexeme: Lexeme::Keyword(Keyword::Uint(bitlength)),
                ..
            } => Ok(Self::UInt(bitlength)),
            Token {
                lexeme: Lexeme::Identifier(identifier),
                ..
            } => Ok(Self::Custom(identifier.inner)),
            token => Err(ParserError::InvalidToken {
                location: token.location,
                expected: vec!["{type}"],
                found: token.lexeme.to_string(),
            }
            .into()),
        }
    }

    ///
    /// Converts the type into its LLVM representation.
    ///
    pub fn into_llvm<'ctx, D>(
        self,
        context: &compiler_llvm_context::Context<'ctx, D>,
    ) -> inkwell::types::IntType<'ctx>
    where
        D: compiler_llvm_context::Dependency,
    {
        match self {
            Self::Bool => context.integer_type(compiler_common::BITLENGTH_BOOLEAN),
            Self::Int(bitlength) => context.integer_type(bitlength),
            Self::UInt(bitlength) => context.integer_type(bitlength),
            Self::Custom(_) => context.field_type(),
        }
    }
}
