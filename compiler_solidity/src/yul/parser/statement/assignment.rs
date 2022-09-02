//!
//! The assignment expression statement.
//!

use crate::yul::error::Error;
use crate::yul::lexer::token::lexeme::symbol::Symbol;
use crate::yul::lexer::token::lexeme::Lexeme;
use crate::yul::lexer::token::location::Location;
use crate::yul::lexer::token::Token;
use crate::yul::lexer::Lexer;
use crate::yul::parser::error::Error as ParserError;
use crate::yul::parser::identifier::Identifier;
use crate::yul::parser::statement::expression::Expression;

///
/// The assignment expression statement.
///
#[derive(Debug, PartialEq, Clone)]
pub struct Assignment {
    /// The location.
    pub location: Location,
    /// The variable bindings.
    pub bindings: Vec<Identifier>,
    /// The initializing expression.
    pub initializer: Expression,
}

impl Assignment {
    ///
    /// The element parser.
    ///
    pub fn parse(lexer: &mut Lexer, initial: Option<Token>) -> Result<Self, Error> {
        let token = crate::yul::parser::take_or_next(initial, lexer)?;

        let (location, identifier) = match token {
            Token {
                location,
                lexeme: Lexeme::Identifier(identifier),
                ..
            } => (location, identifier),
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec!["{identifier}"],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        };
        let length = identifier.inner.len();

        match lexer.peek()? {
            Token {
                lexeme: Lexeme::Symbol(Symbol::Assignment),
                ..
            } => {
                lexer.next()?;

                Ok(Self {
                    location,
                    bindings: vec![Identifier::new(location, identifier.inner)],
                    initializer: Expression::parse(lexer, None)?,
                })
            }
            Token {
                lexeme: Lexeme::Symbol(Symbol::Comma),
                ..
            } => {
                let (identifiers, next) = Identifier::parse_list(
                    lexer,
                    Some(Token::new(location, Lexeme::Identifier(identifier), length)),
                )?;

                match crate::yul::parser::take_or_next(next, lexer)? {
                    Token {
                        lexeme: Lexeme::Symbol(Symbol::Assignment),
                        ..
                    } => {}
                    token => {
                        return Err(ParserError::InvalidToken {
                            location: token.location,
                            expected: vec![":="],
                            found: token.lexeme.to_string(),
                        }
                        .into());
                    }
                }

                Ok(Self {
                    location,
                    bindings: identifiers,
                    initializer: Expression::parse(lexer, None)?,
                })
            }
            token => Err(ParserError::InvalidToken {
                location: token.location,
                expected: vec![":=", ","],
                found: token.lexeme.to_string(),
            }
            .into()),
        }
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for Assignment
where
    D: compiler_llvm_context::Dependency,
{
    fn into_llvm(mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        let value = match self.initializer.into_llvm(context)? {
            Some(value) => value,
            None => return Ok(()),
        };

        if self.bindings.len() == 1 {
            let identifier = self.bindings.remove(0).inner;
            context.build_store(
                context.function().stack[identifier.as_str()],
                value.to_llvm(),
            );
            return Ok(());
        }

        let llvm_type = value.to_llvm().into_struct_value().get_type();
        let pointer = context.build_alloca(llvm_type, "assignment_pointer");
        context.build_store(pointer, value.to_llvm());

        for (index, binding) in self.bindings.into_iter().enumerate() {
            let pointer = unsafe {
                context.builder().build_gep(
                    pointer,
                    &[
                        context.field_const(0),
                        context
                            .integer_type(compiler_common::BITLENGTH_X32)
                            .const_int(index as u64, false),
                    ],
                    format!("assignment_binding_{}_gep_pointer", index).as_str(),
                )
            };

            let value = context.build_load(
                pointer,
                format!("assignment_binding_{}_value", index).as_str(),
            );

            context.build_store(context.function().stack[binding.inner.as_str()], value);
        }

        Ok(())
    }
}
