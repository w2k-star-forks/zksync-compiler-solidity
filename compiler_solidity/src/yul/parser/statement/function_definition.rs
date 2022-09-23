//!
//! The function definition statement.
//!

use inkwell::types::BasicType;

use crate::yul::error::Error;
use crate::yul::lexer::token::lexeme::symbol::Symbol;
use crate::yul::lexer::token::lexeme::Lexeme;
use crate::yul::lexer::token::location::Location;
use crate::yul::lexer::token::Token;
use crate::yul::lexer::Lexer;
use crate::yul::parser::error::Error as ParserError;
use crate::yul::parser::identifier::Identifier;
use crate::yul::parser::statement::block::Block;
use crate::yul::parser::statement::expression::function_call::name::Name as FunctionName;

///
/// The function definition statement.
///
/// All functions are translated in two steps:
/// 1. The hoisted declaration
/// 2. The definition, which now has the access to all function signatures
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDefinition {
    /// The location.
    pub location: Location,
    /// The function identifier.
    pub identifier: String,
    /// The function formal arguments.
    pub arguments: Vec<Identifier>,
    /// The function return variables.
    pub result: Vec<Identifier>,
    /// The function body block.
    pub body: Block,
}

impl FunctionDefinition {
    ///
    /// The element parser.
    ///
    pub fn parse(lexer: &mut Lexer, initial: Option<Token>) -> Result<Self, Error> {
        let token = crate::yul::parser::take_or_next(initial, lexer)?;

        let (location, identifier) = match token {
            Token {
                lexeme: Lexeme::Identifier(identifier),
                location,
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

        match FunctionName::from(identifier.inner.as_str()) {
            FunctionName::UserDefined(_) => {}
            _function_name => {
                return Err(ParserError::ReservedIdentifier {
                    location,
                    identifier: identifier.inner,
                }
                .into())
            }
        }

        match lexer.next()? {
            Token {
                lexeme: Lexeme::Symbol(Symbol::ParenthesisLeft),
                ..
            } => {}
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec!["("],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        }

        let (mut arguments, next) = Identifier::parse_typed_list(lexer, None)?;
        if identifier
            .inner
            .contains(compiler_llvm_context::Function::ZKSYNC_NEAR_CALL_ABI_PREFIX)
        {
            if arguments.is_empty() {
                return Err(ParserError::InvalidNumberOfArguments {
                    location,
                    identifier: identifier.inner,
                    expected: 1,
                    found: arguments.len(),
                }
                .into());
            }

            arguments.remove(0);
        }
        if identifier
            .inner
            .contains(compiler_llvm_context::Function::ZKSYNC_NEAR_CALL_ABI_EXCEPTION_HANDLER)
            && !arguments.is_empty()
        {
            return Err(ParserError::InvalidNumberOfArguments {
                location,
                identifier: identifier.inner,
                expected: 0,
                found: arguments.len(),
            }
            .into());
        }

        match crate::yul::parser::take_or_next(next, lexer)? {
            Token {
                lexeme: Lexeme::Symbol(Symbol::ParenthesisRight),
                ..
            } => {}
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec![")"],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        }

        let (result, next) = match lexer.peek()? {
            Token {
                lexeme: Lexeme::Symbol(Symbol::Arrow),
                ..
            } => {
                lexer.next()?;
                Identifier::parse_typed_list(lexer, None)?
            }
            Token {
                lexeme: Lexeme::Symbol(Symbol::BracketCurlyLeft),
                ..
            } => (vec![], None),
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec!["->", "{"],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        };

        let body = Block::parse(lexer, next)?;

        Ok(Self {
            location,
            identifier: identifier.inner,
            arguments,
            result,
            body,
        })
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for FunctionDefinition
where
    D: compiler_llvm_context::Dependency,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        let argument_types: Vec<_> = self
            .arguments
            .iter()
            .map(|argument| {
                let yul_type = argument.r#type.to_owned().unwrap_or_default();
                yul_type.into_llvm(context).as_basic_type_enum()
            })
            .collect();

        let function_type = context.function_type(self.result.len(), argument_types);

        context.add_function(
            self.identifier.as_str(),
            function_type,
            Some(inkwell::module::Linkage::Private),
        );

        if self.result.len() > 1 {
            let function = context
                .functions
                .get(self.identifier.as_str())
                .cloned()
                .expect("Always exists");
            let pointer = function
                .value
                .get_first_param()
                .expect("Always exists")
                .into_pointer_value();
            context.set_function(function);
            context.set_function_return(compiler_llvm_context::FunctionReturn::compound(
                pointer,
                self.result.len(),
            ));
        }

        Ok(())
    }

    fn into_llvm(mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        let function = context
            .functions
            .get(self.identifier.as_str())
            .cloned()
            .expect("Function always exists");
        context.set_function(function.clone());

        context.set_basic_block(function.entry_block);
        let r#return = match function.r#return {
            Some(r#return) => {
                for (index, identifier) in self.result.into_iter().enumerate() {
                    let pointer = r#return.return_pointer().expect("Always exists");
                    let pointer = unsafe {
                        context.builder().build_gep(
                            pointer,
                            &[
                                context.field_const(0),
                                context
                                    .integer_type(compiler_common::BITLENGTH_X32)
                                    .const_int(index as u64, false),
                            ],
                            format!("return_{}_gep_pointer", index).as_str(),
                        )
                    };
                    let pointer = context.builder().build_pointer_cast(
                        pointer,
                        context
                            .field_type()
                            .ptr_type(compiler_llvm_context::AddressSpace::Stack.into()),
                        format!("return_{}_gep_pointer_field", index).as_str(),
                    );
                    context
                        .function_mut()
                        .stack
                        .insert(identifier.inner.clone(), pointer);
                }
                r#return
            }
            None => {
                if let Some(identifier) = self.result.pop() {
                    let r#type = identifier.r#type.unwrap_or_default();
                    let pointer =
                        context.build_alloca(r#type.clone().into_llvm(context), "return_pointer");
                    context.build_store(pointer, r#type.into_llvm(context).const_zero());
                    context
                        .function_mut()
                        .stack
                        .insert(identifier.inner, pointer);
                    compiler_llvm_context::FunctionReturn::primitive(pointer)
                } else {
                    compiler_llvm_context::FunctionReturn::none()
                }
            }
        };

        let argument_types: Vec<_> = self
            .arguments
            .iter()
            .map(|argument| {
                let yul_type = argument.r#type.to_owned().unwrap_or_default();
                yul_type.into_llvm(context)
            })
            .collect();
        for (mut index, argument) in self.arguments.iter().enumerate() {
            let pointer = context.build_alloca(argument_types[index], argument.inner.as_str());
            context
                .function_mut()
                .stack
                .insert(argument.inner.clone(), pointer);
            if let Some(compiler_llvm_context::FunctionReturn::Compound { .. }) =
                context.function().r#return
            {
                index += 1;
            }
            context.build_store(
                pointer,
                context
                    .function()
                    .value
                    .get_nth_param(index as u32)
                    .expect("Always exists"),
            );
        }

        self.body.into_llvm(context)?;
        match context
            .basic_block()
            .get_last_instruction()
            .map(|instruction| instruction.get_opcode())
        {
            Some(inkwell::values::InstructionOpcode::Br) => {}
            Some(inkwell::values::InstructionOpcode::Switch) => {}
            _ => context.build_unconditional_branch(context.function().return_block),
        }

        match r#return {
            compiler_llvm_context::FunctionReturn::None => {
                context.set_basic_block(context.function().return_block);
                context.build_return(None);
            }
            compiler_llvm_context::FunctionReturn::Primitive { pointer } => {
                context.set_basic_block(context.function().return_block);
                let return_value = context.build_load(pointer, "return_value");
                context.build_return(Some(&return_value));
            }
            compiler_llvm_context::FunctionReturn::Compound {
                pointer: return_pointer,
                ..
            } => {
                context.set_basic_block(context.function().return_block);
                context.build_return(Some(&return_pointer));
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
    fn error_invalid_token_identifier() {
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
                return(0, 0)
            }

            function 256() -> result {
                result := 42
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
                location: Location::new(14, 22),
                expected: vec!["{identifier}"],
                found: "256".to_owned(),
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_token_parenthesis_left() {
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
                return(0, 0)
            }

            function test{) -> result {
                result := 42
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
                location: Location::new(14, 26),
                expected: vec!["("],
                found: "{".to_owned(),
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_token_parenthesis_right() {
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
                return(0, 0)
            }

            function test(} -> result {
                result := 42
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
                location: Location::new(14, 27),
                expected: vec![")"],
                found: "}".to_owned(),
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_token_arrow_or_bracket_curly_left() {
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
                return(0, 0)
            }

            function test() := result {
                result := 42
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
                location: Location::new(14, 29),
                expected: vec!["->", "{"],
                found: ":=".to_owned(),
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_number_of_arguments_near_call_abi() {
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
                return(0, 0)
            }

            function ZKSYNC_NEAR_CALL_test() -> result {
                result := 42
            }
        }
    }
}
    "#;

        let mut lexer = Lexer::new(input.to_owned());
        let result = Object::parse(&mut lexer, None);
        assert_eq!(
            result,
            Err(Error::InvalidNumberOfArguments {
                location: Location::new(14, 22),
                identifier: "ZKSYNC_NEAR_CALL_test".to_owned(),
                expected: 1,
                found: 0,
            }
            .into())
        );
    }

    #[test]
    fn error_invalid_number_of_arguments_near_call_abi_catch() {
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
                return(0, 0)
            }

            function ZKSYNC_CATCH_NEAR_CALL(length) {
                revert(0, length)
            }
        }
    }
}
    "#;

        let mut lexer = Lexer::new(input.to_owned());
        let result = Object::parse(&mut lexer, None);
        assert_eq!(
            result,
            Err(Error::InvalidNumberOfArguments {
                location: Location::new(14, 22),
                identifier: "ZKSYNC_CATCH_NEAR_CALL".to_owned(),
                expected: 0,
                found: 1,
            }
            .into())
        );
    }

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
                return(0, 0)
            }

            function basefee() -> result {
                result := 42
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
                location: Location::new(14, 22),
                identifier: "basefee".to_owned()
            }
            .into())
        );
    }
}
