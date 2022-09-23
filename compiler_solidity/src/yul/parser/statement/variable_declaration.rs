//!
//! The variable declaration statement.
//!

use inkwell::types::BasicType;
use inkwell::values::BasicValue;

use crate::yul::error::Error;
use crate::yul::lexer::token::lexeme::symbol::Symbol;
use crate::yul::lexer::token::lexeme::Lexeme;
use crate::yul::lexer::token::location::Location;
use crate::yul::lexer::token::Token;
use crate::yul::lexer::Lexer;
use crate::yul::parser::error::Error as ParserError;
use crate::yul::parser::identifier::Identifier;
use crate::yul::parser::statement::expression::function_call::name::Name as FunctionName;
use crate::yul::parser::statement::expression::Expression;

///
/// The Yul variable declaration statement.
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableDeclaration {
    /// The location.
    pub location: Location,
    /// The variable bindings list.
    pub bindings: Vec<Identifier>,
    /// The variable initializing expression.
    pub expression: Option<Expression>,
}

impl VariableDeclaration {
    ///
    /// The element parser.
    ///
    pub fn parse(
        lexer: &mut Lexer,
        initial: Option<Token>,
    ) -> Result<(Self, Option<Token>), Error> {
        let token = crate::yul::parser::take_or_next(initial, lexer)?;
        let location = token.location;

        let (bindings, next) = Identifier::parse_typed_list(lexer, Some(token))?;
        for binding in bindings.iter() {
            match FunctionName::from(binding.inner.as_str()) {
                FunctionName::UserDefined(_) => continue,
                _function_name => {
                    return Err(ParserError::ReservedIdentifier {
                        location: binding.location,
                        identifier: binding.inner.to_owned(),
                    }
                    .into())
                }
            }
        }

        match crate::yul::parser::take_or_next(next, lexer)? {
            Token {
                lexeme: Lexeme::Symbol(Symbol::Assignment),
                ..
            } => {}
            token => {
                return Ok((
                    Self {
                        location,
                        bindings,
                        expression: None,
                    },
                    Some(token),
                ))
            }
        }

        let expression = Expression::parse(lexer, None)?;

        Ok((
            Self {
                location,
                bindings,
                expression: Some(expression),
            },
            None,
        ))
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for VariableDeclaration
where
    D: compiler_llvm_context::Dependency,
{
    fn into_llvm(mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        if self.bindings.len() == 1 {
            let identifier = self.bindings.remove(0);
            let r#type = identifier.r#type.unwrap_or_default().into_llvm(context);
            let pointer = context.build_alloca(r#type, identifier.inner.as_str());
            context
                .function_mut()
                .stack
                .insert(identifier.inner.clone(), pointer);

            let value = if let Some(expression) = self.expression {
                match expression.into_llvm(context)? {
                    Some(mut value) => {
                        if let Some(constant) = value.constant.take() {
                            context
                                .function_mut()
                                .constants
                                .insert(identifier.inner, constant);
                        }

                        value.to_llvm()
                    }
                    None => r#type.const_zero().as_basic_value_enum(),
                }
            } else {
                r#type.const_zero().as_basic_value_enum()
            };
            context.build_store(pointer, value);
            return Ok(());
        }

        let llvm_type = context.structure_type(
            self.bindings
                .iter()
                .map(|binding| {
                    binding
                        .r#type
                        .to_owned()
                        .unwrap_or_default()
                        .into_llvm(context)
                        .as_basic_type_enum()
                })
                .collect(),
        );
        let pointer = context.build_alloca(llvm_type, "bindings_pointer");
        for (index, binding) in self.bindings.iter().enumerate() {
            let yul_type = binding
                .r#type
                .to_owned()
                .unwrap_or_default()
                .into_llvm(context);
            let pointer = context.build_alloca(
                yul_type.as_basic_type_enum(),
                format!("binding_{}_pointer", index).as_str(),
            );
            context
                .function_mut()
                .stack
                .insert(binding.inner.to_owned(), pointer);
        }

        match self.expression.take() {
            Some(expression) => {
                let location = expression.location();

                if let Some(value) = expression.into_llvm(context)? {
                    if value
                        .value
                        .get_type()
                        .ptr_type(compiler_llvm_context::AddressSpace::Stack.into())
                        != pointer.get_type()
                    {
                        anyhow::bail!(
                            "{} Assignment to {:?} received an invalid number of arguments",
                            location,
                            self.bindings
                        );
                    }

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
                                format!("binding_{}_gep_pointer", index).as_str(),
                            )
                        };

                        let value = context
                            .build_load(pointer, format!("binding_{}_value", index).as_str());
                        let pointer = context
                            .function_mut()
                            .stack
                            .get(binding.inner.as_str())
                            .copied()
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "{} Assignment to an undeclared variable `{}`",
                                    binding.location,
                                    binding.inner
                                )
                            })?;
                        context.build_store(pointer, value);
                    }
                }
            }
            None => {
                context.build_store(pointer, llvm_type.const_zero());
            }
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
    fn error_reserved_identifier() {
        let input = r#"
object "Test" {
    code {
        {
            return(0, 0)
        }
    }
    object "Test_deployed" {
        code {
            {
                let basefee := 42
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
            Err(Error::ReservedIdentifier {
                location: Location::new(11, 21),
                identifier: "basefee".to_owned()
            }
            .into())
        );
    }
}
