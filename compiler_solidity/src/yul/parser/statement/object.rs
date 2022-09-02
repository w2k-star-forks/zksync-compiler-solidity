//!
//! The YUL object.
//!

use std::collections::HashSet;

use crate::yul::error::Error;
use crate::yul::lexer::token::lexeme::keyword::Keyword;
use crate::yul::lexer::token::lexeme::literal::Literal;
use crate::yul::lexer::token::lexeme::symbol::Symbol;
use crate::yul::lexer::token::lexeme::Lexeme;
use crate::yul::lexer::token::location::Location;
use crate::yul::lexer::token::Token;
use crate::yul::lexer::Lexer;
use crate::yul::parser::error::Error as ParserError;
use crate::yul::parser::statement::code::Code;

///
/// The YUL object.
///
#[derive(Debug, PartialEq, Clone)]
pub struct Object {
    /// The location.
    pub location: Location,
    /// The identifier.
    pub identifier: String,
    /// The code.
    pub code: Code,
    /// The optional inner object.
    pub inner_object: Option<Box<Self>>,
    /// The factory dependency objects.
    pub factory_dependencies: HashSet<String>,
}

impl Object {
    ///
    /// The element parser.
    ///
    pub fn parse(lexer: &mut Lexer, initial: Option<Token>) -> Result<Self, Error> {
        let token = crate::yul::parser::take_or_next(initial, lexer)?;

        let location = match token {
            Token {
                lexeme: Lexeme::Keyword(Keyword::Object),
                location,
                ..
            } => location,
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec!["object"],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        };

        let identifier = match lexer.next()? {
            Token {
                lexeme: Lexeme::Literal(Literal::String(literal)),
                ..
            } => literal.inner,
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec!["{string}"],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        };
        let is_runtime_code = identifier.ends_with("_deployed");

        match lexer.next()? {
            Token {
                lexeme: Lexeme::Symbol(Symbol::BracketCurlyLeft),
                ..
            } => {}
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec!["{"],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        }

        let code = Code::parse(lexer, None)?;
        let mut inner_object = None;
        let mut factory_dependencies = HashSet::new();

        if !is_runtime_code {
            inner_object = match lexer.peek()? {
                Token {
                    lexeme: Lexeme::Keyword(Keyword::Object),
                    ..
                } => {
                    let mut object = Self::parse(lexer, None)?;
                    factory_dependencies.extend(object.factory_dependencies.drain());
                    Some(Box::new(object))
                }
                _ => None,
            };

            if let Token {
                lexeme: Lexeme::Identifier(identifier),
                ..
            } = lexer.peek()?
            {
                if identifier.inner.as_str() == "data" {
                    let _data = lexer.next()?;
                    let _identifier = lexer.next()?;
                    let _metadata = lexer.next()?;
                }
            };
        }

        loop {
            match lexer.next()? {
                Token {
                    lexeme: Lexeme::Symbol(Symbol::BracketCurlyRight),
                    ..
                } => break,
                token @ Token {
                    lexeme: Lexeme::Keyword(Keyword::Object),
                    ..
                } => {
                    let dependency = Self::parse(lexer, Some(token))?;
                    factory_dependencies.insert(dependency.identifier);
                }
                Token {
                    lexeme: Lexeme::Identifier(identifier),
                    ..
                } if identifier.inner.as_str() == "data" => {
                    let _identifier = lexer.next()?;
                    let _metadata = lexer.next()?;
                }
                token => {
                    return Err(ParserError::InvalidToken {
                        location: token.location,
                        expected: vec!["object", "}"],
                        found: token.lexeme.to_string(),
                    }
                    .into());
                }
            }
        }

        Ok(Self {
            location,
            identifier,
            code,
            inner_object,
            factory_dependencies,
        })
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for Object
where
    D: compiler_llvm_context::Dependency,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        let mut entry = compiler_llvm_context::EntryFunction::default();
        entry.declare(context)?;

        compiler_llvm_context::DeployCodeFunction::new(
            compiler_llvm_context::DummyLLVMWritable::default(),
        )
        .declare(context)?;
        compiler_llvm_context::RuntimeCodeFunction::new(
            compiler_llvm_context::DummyLLVMWritable::default(),
        )
        .declare(context)?;

        entry.into_llvm(context)?;

        Ok(())
    }

    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        if self.identifier.ends_with("_deployed") {
            compiler_llvm_context::RuntimeCodeFunction::new(self.code).into_llvm(context)?;
        } else {
            compiler_llvm_context::DeployCodeFunction::new(self.code).into_llvm(context)?;
        }

        if let Some(object) = self.inner_object {
            object.into_llvm(context)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::yul::lexer::token::location::Location;
    use crate::yul::lexer::Lexer;
    use crate::yul::parser::error::Error;
    use crate::yul::parser::statement::object::Object;

    #[test]
    fn error_invalid_token_object() {
        let input = r#"
class "Test" {
    code {
        {
            return(0, 0)
        }
    }
    object "Test_deployed" {
        code {
            {
                return(0, 0)
            }
        }
    }
}
    "#;

        let mut lexer = Lexer::new(input.to_owned());
        let result = Object::parse(&mut lexer, None);
        assert_eq!(
            result,
            Err(Error::InvalidToken {
                location: Location::new(2, 1),
                expected: vec!["object"],
                found: "class".to_owned(),
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_token_identifier() {
        let input = r#"
object 256 {
    code {
        {
            return(0, 0)
        }
    }
    object "Test_deployed" {
        code {
            {
                return(0, 0)
            }
        }
    }
}
    "#;

        let mut lexer = Lexer::new(input.to_owned());
        let result = Object::parse(&mut lexer, None);
        assert_eq!(
            result,
            Err(Error::InvalidToken {
                location: Location::new(2, 8),
                expected: vec!["{string}"],
                found: "256".to_owned(),
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_token_bracket_curly_left() {
        let input = r#"
object "Test" (
    code {
        {
            return(0, 0)
        }
    }
    object "Test_deployed" {
        code {
            {
                return(0, 0)
            }
        }
    }
}
    "#;

        let mut lexer = Lexer::new(input.to_owned());
        let result = Object::parse(&mut lexer, None);
        assert_eq!(
            result,
            Err(Error::InvalidToken {
                location: Location::new(2, 15),
                expected: vec!["{"],
                found: "(".to_owned(),
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_token_object_inner() {
        let input = r#"
object "Test" {
    code {
        {
            return(0, 0)
        }
    }
    class "Test_deployed" {
        code {
            {
                return(0, 0)
            }
        }
    }
}
    "#;

        let mut lexer = Lexer::new(input.to_owned());
        let result = Object::parse(&mut lexer, None);
        assert_eq!(
            result,
            Err(Error::InvalidToken {
                location: Location::new(8, 5),
                expected: vec!["object", "}"],
                found: "class".to_owned(),
            }
            .into())
        );
    }
}
