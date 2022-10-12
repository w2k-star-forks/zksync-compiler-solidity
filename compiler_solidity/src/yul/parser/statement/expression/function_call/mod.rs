//!
//! The function call subexpression.
//!

pub mod name;

use num::ToPrimitive;

use inkwell::types::BasicType;
use inkwell::values::BasicValue;

use crate::yul::error::Error;
use crate::yul::lexer::token::lexeme::symbol::Symbol;
use crate::yul::lexer::token::lexeme::Lexeme;
use crate::yul::lexer::token::location::Location;
use crate::yul::lexer::token::Token;
use crate::yul::lexer::Lexer;
use crate::yul::parser::error::Error as ParserError;
use crate::yul::parser::statement::expression::Expression;

use self::name::Name;

///
/// The Yul function call subexpression.
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCall {
    /// The location.
    pub location: Location,
    /// The function name.
    pub name: Name,
    /// The function arguments expression list.
    pub arguments: Vec<Expression>,
}

impl FunctionCall {
    ///
    /// The element parser.
    ///
    pub fn parse(lexer: &mut Lexer, initial: Option<Token>) -> Result<Self, Error> {
        let token = crate::yul::parser::take_or_next(initial, lexer)?;

        let (location, name) = match token {
            Token {
                lexeme: Lexeme::Identifier(identifier),
                location,
                ..
            } => (location, Name::from(identifier.inner.as_str())),
            token => {
                return Err(ParserError::InvalidToken {
                    location: token.location,
                    expected: vec!["{identifier}"],
                    found: token.lexeme.to_string(),
                }
                .into());
            }
        };

        let mut arguments = Vec::new();
        loop {
            let argument = match lexer.next()? {
                Token {
                    lexeme: Lexeme::Symbol(Symbol::ParenthesisRight),
                    ..
                } => break,
                token => Expression::parse(lexer, Some(token))?,
            };

            arguments.push(argument);

            match lexer.peek()? {
                Token {
                    lexeme: Lexeme::Symbol(Symbol::Comma),
                    ..
                } => {
                    lexer.next()?;
                    continue;
                }
                Token {
                    lexeme: Lexeme::Symbol(Symbol::ParenthesisRight),
                    ..
                } => {
                    lexer.next()?;
                    break;
                }
                _ => break,
            }
        }

        Ok(Self {
            location,
            name,
            arguments,
        })
    }

    ///
    /// Converts the function call into an LLVM value.
    ///
    pub fn into_llvm<'ctx, D>(
        mut self,
        context: &mut compiler_llvm_context::Context<'ctx, D>,
    ) -> anyhow::Result<Option<inkwell::values::BasicValueEnum<'ctx>>>
    where
        D: compiler_llvm_context::Dependency,
    {
        let location = self.location;

        match self.name {
            Name::UserDefined(name)
                if name.contains(compiler_llvm_context::Function::ZKSYNC_NEAR_CALL_ABI_PREFIX) =>
            {
                let mut values = Vec::with_capacity(self.arguments.len());
                for argument in self.arguments.into_iter() {
                    let value = argument.into_llvm(context)?.expect("Always exists").value;
                    values.push(value);
                }
                let function = context.get_function(name.as_str()).ok_or_else(|| {
                    anyhow::anyhow!("{} Undeclared function `{}`", location, name)
                })?;
                let r#return = function.borrow().r#return();

                if let compiler_llvm_context::FunctionReturn::Compound { size, .. } = r#return {
                    let r#type =
                        context
                            .structure_type(vec![context.field_type().as_basic_type_enum(); size]);
                    let pointer = context.build_alloca(
                        r#type,
                        format!("{}_near_call_return_pointer_argument", name).as_str(),
                    );
                    context.build_store(pointer, r#type.const_zero());
                    values.insert(1, pointer.as_basic_value_enum());
                }

                let function_pointer = context.builder().build_bitcast(
                    function.borrow().inner(),
                    context
                        .field_type()
                        .ptr_type(compiler_llvm_context::AddressSpace::Stack.into()),
                    format!("{}_near_call_function_pointer", name).as_str(),
                );
                values.insert(
                    0,
                    function_pointer.into_pointer_value().as_basic_value_enum(),
                );

                let expected_arguments_count = function.borrow().inner().count_params() as usize;
                if expected_arguments_count != (values.len() - 2) {
                    anyhow::bail!(
                        "{} Function `{}` expected {} arguments, found {}",
                        location,
                        name,
                        expected_arguments_count,
                        values.len()
                    );
                }

                let return_value = context.build_invoke_near_call_abi(
                    function.borrow().inner(),
                    values,
                    format!("{}_near_call", name).as_str(),
                );

                if let compiler_llvm_context::FunctionReturn::Compound { .. } = r#return {
                    let return_pointer = return_value.expect("Always exists").into_pointer_value();
                    let return_value = context.build_load(
                        return_pointer,
                        format!("{}_near_call_return_value", name).as_str(),
                    );
                    Ok(Some(return_value))
                } else {
                    Ok(return_value)
                }
            }
            Name::UserDefined(name) => {
                let mut values = Vec::with_capacity(self.arguments.len());
                for argument in self.arguments.into_iter() {
                    let value = argument.into_llvm(context)?.expect("Always exists").value;
                    values.push(value);
                }
                let function = context.get_function(name.as_str()).ok_or_else(|| {
                    anyhow::anyhow!("{} Undeclared function `{}`", location, name)
                })?;
                let r#return = function.borrow().r#return();

                if let compiler_llvm_context::FunctionReturn::Compound { size, .. } = r#return {
                    let r#type =
                        context
                            .structure_type(vec![context.field_type().as_basic_type_enum(); size]);
                    let pointer = context
                        .build_alloca(r#type, format!("{}_return_pointer_argument", name).as_str());
                    context.build_store(pointer, r#type.const_zero());
                    values.insert(0, pointer.as_basic_value_enum());
                }

                let expected_arguments_count = function.borrow().inner().count_params() as usize;
                if expected_arguments_count != values.len() {
                    anyhow::bail!(
                        "{} Function `{}` expected {} arguments, found {}",
                        location,
                        name,
                        expected_arguments_count,
                        values.len()
                    );
                }

                let return_value = context.build_invoke(
                    function.borrow().inner(),
                    values.as_slice(),
                    format!("{}_call", name).as_str(),
                );

                if let compiler_llvm_context::FunctionReturn::Compound { .. } = r#return {
                    let return_pointer = return_value.expect("Always exists").into_pointer_value();
                    let return_value = context
                        .build_load(return_pointer, format!("{}_return_value", name).as_str());
                    Ok(Some(return_value))
                } else {
                    Ok(return_value)
                }
            }

            Name::Add => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::arithmetic::addition(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Sub => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::arithmetic::subtraction(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Mul => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::arithmetic::multiplication(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Div => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::arithmetic::division(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Mod => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::arithmetic::remainder(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Sdiv => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::arithmetic::division_signed(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Smod => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::arithmetic::remainder_signed(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }

            Name::Lt => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::comparison::compare(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    inkwell::IntPredicate::ULT,
                )
            }
            Name::Gt => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::comparison::compare(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    inkwell::IntPredicate::UGT,
                )
            }
            Name::Eq => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::comparison::compare(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    inkwell::IntPredicate::EQ,
                )
            }
            Name::IsZero => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                compiler_llvm_context::comparison::compare(
                    context,
                    arguments[0].into_int_value(),
                    context.field_const(0),
                    inkwell::IntPredicate::EQ,
                )
            }
            Name::Slt => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::comparison::compare(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    inkwell::IntPredicate::SLT,
                )
            }
            Name::Sgt => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::comparison::compare(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    inkwell::IntPredicate::SGT,
                )
            }

            Name::Or => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::bitwise::or(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Xor => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::bitwise::xor(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Not => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                compiler_llvm_context::bitwise::xor(
                    context,
                    arguments[0].into_int_value(),
                    context.field_type().const_all_ones(),
                )
            }
            Name::And => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::bitwise::and(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Shl => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::bitwise::shift_left(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Shr => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::bitwise::shift_right(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Sar => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::bitwise::shift_right_arithmetic(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Byte => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::bitwise::byte(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Pop => {
                let _arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                Ok(None)
            }

            Name::AddMod => {
                let arguments = self.pop_arguments_llvm::<D, 3>(context)?;
                compiler_llvm_context::math::add_mod(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2].into_int_value(),
                )
            }
            Name::MulMod => {
                let arguments = self.pop_arguments_llvm::<D, 3>(context)?;
                compiler_llvm_context::math::mul_mod(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2].into_int_value(),
                )
            }
            Name::Exp => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::math::exponent(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::SignExtend => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::math::sign_extend(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }

            Name::Keccak256 => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::hash::keccak256(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }

            Name::MLoad => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                compiler_llvm_context::memory::load(context, arguments[0].into_int_value())
            }
            Name::MStore => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::memory::store(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::MStore8 => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::memory::store_byte(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }

            Name::SLoad => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                compiler_llvm_context::storage::load(context, arguments[0].into_int_value())
            }
            Name::SStore => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::storage::store(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::LoadImmutable => {
                let mut arguments = self.pop_arguments::<D, 1>(context)?;
                let key = arguments[0].original.take().ok_or_else(|| {
                    anyhow::anyhow!("{} `load_immutable` literal is missing", location)
                })?;

                if key.as_str() == "library_deploy_address" {
                    return Ok(context.build_call(
                        context.get_intrinsic_function(
                            compiler_llvm_context::IntrinsicFunction::CodeSource,
                        ),
                        &[],
                        "library_deploy_address",
                    ));
                }

                let offset = context
                    .solidity_mut()
                    .get_or_allocate_immutable(key.as_str());

                let index = context.field_const(offset as u64);

                compiler_llvm_context::immutable::load(context, index)
            }
            Name::SetImmutable => {
                let mut arguments = self.pop_arguments::<D, 3>(context)?;
                let key = arguments[1].original.take().ok_or_else(|| {
                    anyhow::anyhow!("{} `load_immutable` literal is missing", location)
                })?;

                if key.as_str() == "library_deploy_address" {
                    return Ok(None);
                }

                let offset = context.solidity_mut().allocate_immutable(key.as_str());

                let index = context.field_const(offset as u64);
                let value = arguments[2].value.into_int_value();
                compiler_llvm_context::immutable::store(context, index, value)
            }

            Name::CallDataLoad => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                compiler_llvm_context::calldata::load(context, arguments[0].into_int_value())
            }
            Name::CallDataSize => compiler_llvm_context::calldata::size(context),
            Name::CallDataCopy => {
                let arguments = self.pop_arguments_llvm::<D, 3>(context)?;
                compiler_llvm_context::calldata::copy(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2].into_int_value(),
                )
            }
            Name::CodeSize => compiler_llvm_context::calldata::size(context),
            Name::CodeCopy => {
                if let compiler_llvm_context::CodeType::Runtime = context.code_type() {
                    anyhow::bail!(
                        "{} The `CODECOPY` instruction is not supported in the runtime code",
                        location,
                    );
                }

                let arguments = self.pop_arguments_llvm::<D, 3>(context)?;
                compiler_llvm_context::calldata::copy(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2].into_int_value(),
                )
            }
            Name::ReturnDataSize => compiler_llvm_context::return_data::size(context),
            Name::ReturnDataCopy => {
                let arguments = self.pop_arguments_llvm::<D, 3>(context)?;
                compiler_llvm_context::return_data::copy(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2].into_int_value(),
                )
            }
            Name::ExtCodeSize => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                compiler_llvm_context::ext_code::size(context, arguments[0].into_int_value())
            }
            Name::ExtCodeHash => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                compiler_llvm_context::ext_code::hash(context, arguments[0].into_int_value())
            }

            Name::Return => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::r#return::r#return(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Revert => {
                let arguments = self.pop_arguments_llvm::<D, 2>(context)?;
                compiler_llvm_context::r#return::revert(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                )
            }
            Name::Stop => compiler_llvm_context::r#return::stop(context),
            Name::Invalid => compiler_llvm_context::r#return::invalid(context),

            Name::Log0 => {
                let arguments = self.pop_arguments_llvm_log::<D, 2>(context)?;
                compiler_llvm_context::event::log(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    vec![],
                )
            }
            Name::Log1 => {
                let arguments = self.pop_arguments_llvm_log::<D, 3>(context)?;
                compiler_llvm_context::event::log(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2..]
                        .iter()
                        .map(|argument| argument.into_int_value())
                        .collect(),
                )
            }
            Name::Log2 => {
                let arguments = self.pop_arguments_llvm_log::<D, 4>(context)?;
                compiler_llvm_context::event::log(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2..]
                        .iter()
                        .map(|argument| argument.into_int_value())
                        .collect(),
                )
            }
            Name::Log3 => {
                let arguments = self.pop_arguments_llvm_log::<D, 5>(context)?;
                compiler_llvm_context::event::log(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2..]
                        .iter()
                        .map(|argument| argument.into_int_value())
                        .collect(),
                )
            }
            Name::Log4 => {
                let arguments = self.pop_arguments_llvm_log::<D, 6>(context)?;
                compiler_llvm_context::event::log(
                    context,
                    arguments[0].into_int_value(),
                    arguments[1].into_int_value(),
                    arguments[2..]
                        .iter()
                        .map(|argument| argument.into_int_value())
                        .collect(),
                )
            }

            Name::Call => {
                let mut arguments = self.pop_arguments::<D, 7>(context)?;

                let gas = arguments[0].value.into_int_value();
                let address = arguments[1].value.into_int_value();
                let value = arguments[2].value.into_int_value();
                let input_offset = arguments[3].value.into_int_value();
                let input_size = arguments[4].value.into_int_value();
                let output_offset = arguments[5].value.into_int_value();
                let output_size = arguments[6].value.into_int_value();

                let simulation_address = arguments[1]
                    .constant
                    .take()
                    .and_then(|value| value.to_u16());

                compiler_llvm_context::contract::call(
                    context,
                    context.runtime().far_call,
                    gas,
                    address,
                    Some(value),
                    input_offset,
                    input_size,
                    output_offset,
                    output_size,
                    simulation_address,
                )
            }
            Name::CallCode => {
                let _arguments = self.pop_arguments_llvm::<D, 7>(context)?;
                Ok(Some(context.field_const(0).as_basic_value_enum()))
            }
            Name::StaticCall => {
                let mut arguments = self.pop_arguments::<D, 6>(context)?;

                let gas = arguments[0].value.into_int_value();
                let address = arguments[1].value.into_int_value();
                let input_offset = arguments[2].value.into_int_value();
                let input_size = arguments[3].value.into_int_value();
                let output_offset = arguments[4].value.into_int_value();
                let output_size = arguments[5].value.into_int_value();

                let simulation_address = arguments[1]
                    .constant
                    .take()
                    .and_then(|value| value.to_u16());

                compiler_llvm_context::contract::call(
                    context,
                    context.runtime().static_call,
                    gas,
                    address,
                    None,
                    input_offset,
                    input_size,
                    output_offset,
                    output_size,
                    simulation_address,
                )
            }
            Name::DelegateCall => {
                let mut arguments = self.pop_arguments::<D, 6>(context)?;

                let gas = arguments[0].value.into_int_value();
                let address = arguments[1].value.into_int_value();
                let input_offset = arguments[2].value.into_int_value();
                let input_size = arguments[3].value.into_int_value();
                let output_offset = arguments[4].value.into_int_value();
                let output_size = arguments[5].value.into_int_value();

                let simulation_address = arguments[1]
                    .constant
                    .take()
                    .and_then(|value| value.to_u16());

                compiler_llvm_context::contract::call(
                    context,
                    context.runtime().delegate_call,
                    gas,
                    address,
                    None,
                    input_offset,
                    input_size,
                    output_offset,
                    output_size,
                    simulation_address,
                )
            }

            Name::Create => {
                let arguments = self.pop_arguments_llvm::<D, 3>(context)?;

                let value = arguments[0].into_int_value();
                let input_offset = arguments[1].into_int_value();
                let input_size = arguments[2].into_int_value();

                compiler_llvm_context::create::create(
                    context,
                    value,
                    input_offset,
                    input_size,
                    compiler_llvm_context::AddressSpace::Heap,
                )
            }
            Name::Create2 => {
                let arguments = self.pop_arguments_llvm::<D, 4>(context)?;

                let value = arguments[0].into_int_value();
                let input_offset = arguments[1].into_int_value();
                let input_size = arguments[2].into_int_value();
                let salt = arguments[3].into_int_value();

                compiler_llvm_context::create::create2(
                    context,
                    value,
                    input_offset,
                    input_size,
                    Some(salt),
                    compiler_llvm_context::AddressSpace::Heap,
                )
            }
            Name::DataOffset => {
                let mut arguments = self.pop_arguments::<D, 1>(context)?;
                let identifier = arguments[0].original.take().ok_or_else(|| {
                    anyhow::anyhow!("{} `dataoffset` object identifier is missing", location)
                })?;
                compiler_llvm_context::create::contract_hash(context, identifier)
            }
            Name::DataSize => {
                let mut arguments = self.pop_arguments::<D, 1>(context)?;
                let identifier = arguments[0].original.take().ok_or_else(|| {
                    anyhow::anyhow!("{} `dataoffset` object identifier is missing", location)
                })?;
                compiler_llvm_context::create::header_size(context, identifier)
            }
            Name::DataCopy => {
                let arguments = self.pop_arguments_llvm::<D, 3>(context)?;
                let offset = context.builder().build_int_add(
                    arguments[0].into_int_value(),
                    context.field_const(
                        (compiler_common::SIZE_X32 + compiler_common::SIZE_FIELD) as u64,
                    ),
                    "datacopy_contract_hash_offset",
                );
                compiler_llvm_context::memory::store(context, offset, arguments[1].into_int_value())
            }

            Name::LinkerSymbol => {
                let mut arguments = self.pop_arguments::<D, 1>(context)?;
                let path = arguments[0].original.take().ok_or_else(|| {
                    anyhow::anyhow!("{} Linker symbol literal is missing", location)
                })?;

                Ok(Some(
                    context
                        .resolve_library(path.as_str())?
                        .as_basic_value_enum(),
                ))
            }
            Name::MemoryGuard => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                Ok(Some(arguments[0]))
            }

            Name::Address => Ok(context.build_call(
                context.get_intrinsic_function(compiler_llvm_context::IntrinsicFunction::Address),
                &[],
                "address",
            )),
            Name::Caller => Ok(context.build_call(
                context.get_intrinsic_function(compiler_llvm_context::IntrinsicFunction::Caller),
                &[],
                "caller",
            )),

            Name::CallValue => compiler_llvm_context::ether_gas::value(context),
            Name::Gas => compiler_llvm_context::ether_gas::gas(context),
            Name::Balance => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;

                let address = arguments[0].into_int_value();
                compiler_llvm_context::ether_gas::balance(context, address)
            }
            Name::SelfBalance => {
                let address = context
                    .build_call(
                        context.get_intrinsic_function(
                            compiler_llvm_context::IntrinsicFunction::Address,
                        ),
                        &[],
                        "self_balance_address",
                    )
                    .expect("Always exists")
                    .into_int_value();

                compiler_llvm_context::ether_gas::balance(context, address)
            }

            Name::GasLimit => compiler_llvm_context::contract_context::gas_limit(context),
            Name::GasPrice => compiler_llvm_context::contract_context::gas_price(context),
            Name::Origin => compiler_llvm_context::contract_context::origin(context),
            Name::ChainId => compiler_llvm_context::contract_context::chain_id(context),
            Name::Timestamp => compiler_llvm_context::contract_context::block_timestamp(context),
            Name::Number => compiler_llvm_context::contract_context::block_number(context),
            Name::BlockHash => {
                let arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                let index = arguments[0].into_int_value();

                compiler_llvm_context::contract_context::block_hash(context, index)
            }
            Name::Difficulty => compiler_llvm_context::contract_context::difficulty(context),
            Name::CoinBase => compiler_llvm_context::contract_context::coinbase(context),
            Name::BaseFee => compiler_llvm_context::contract_context::basefee(context),
            Name::MSize => compiler_llvm_context::contract_context::msize(context),

            Name::Verbatim {
                input_size,
                output_size,
            } => {
                if output_size > 1 {
                    anyhow::bail!(
                        "{} Verbatim instructions with multiple return values are not supported",
                        location
                    );
                }

                let mut arguments = self.pop_arguments::<D, 1>(context)?;
                let identifier = arguments[0]
                    .original
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("{} Verbatim literal is missing", location))?;
                match identifier.as_str() {
                    identifier @ "to_l1" => {
                        const ARGUMENTS_COUNT: usize = 3;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::to_l1(
                            context,
                            arguments[0].into_int_value(),
                            arguments[1].into_int_value(),
                            arguments[2].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "code_source" => {
                        const ARGUMENTS_COUNT: usize = 0;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        compiler_llvm_context::contract::simulation::code_source(context).map(Some)
                    }
                    identifier @ "precompile" => {
                        const ARGUMENTS_COUNT: usize = 2;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::precompile(
                            context,
                            arguments[0].into_int_value(),
                            arguments[1].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "meta" => {
                        const ARGUMENTS_COUNT: usize = 0;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        compiler_llvm_context::contract::simulation::meta(context).map(Some)
                    }
                    identifier @ "mimic_call" => {
                        const ARGUMENTS_COUNT: usize = 3;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::mimic_call(
                            context,
                            context.runtime().mimic_call,
                            arguments[0].into_int_value(),
                            arguments[1].into_int_value(),
                            arguments[2],
                            [context.field_const(0), context.field_const(0)],
                        )
                        .map(Some)
                    }
                    identifier @ "mimic_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 2;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::mimic_call(
                            context,
                            context.runtime().mimic_call_byref,
                            arguments[0].into_int_value(),
                            arguments[1].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            [context.field_const(0), context.field_const(0)],
                        )
                        .map(Some)
                    }
                    identifier @ "system_mimic_call" => {
                        const ARGUMENTS_COUNT: usize = 5;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::mimic_call(
                            context,
                            context.runtime().system_mimic_call,
                            arguments[0].into_int_value(),
                            arguments[1].into_int_value(),
                            arguments[2],
                            [arguments[3].into_int_value(), arguments[4].into_int_value()],
                        )
                        .map(Some)
                    }
                    identifier @ "system_mimic_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 4;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::mimic_call(
                            context,
                            context.runtime().system_mimic_call_byref,
                            arguments[0].into_int_value(),
                            arguments[1].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            [arguments[2].into_int_value(), arguments[3].into_int_value()],
                        )
                        .map(Some)
                    }
                    identifier @ "raw_call" => {
                        const ARGUMENTS_COUNT: usize = 4;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::raw_far_call(
                            context,
                            context.runtime().far_call,
                            arguments[0].into_int_value(),
                            arguments[1],
                            arguments[2].into_int_value(),
                            arguments[3].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "raw_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 3;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::raw_far_call(
                            context,
                            context.runtime().far_call_byref,
                            arguments[0].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            arguments[1].into_int_value(),
                            arguments[2].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "system_call" => {
                        const ARGUMENTS_COUNT: usize = 6;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::system_call(
                            context,
                            context.runtime().system_far_call,
                            arguments[0].into_int_value(),
                            arguments[1],
                            arguments[4].into_int_value(),
                            arguments[5].into_int_value(),
                            arguments[2].into_int_value(),
                            arguments[3].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "system_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 5;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::system_call(
                            context,
                            context.runtime().system_far_call_byref,
                            arguments[0].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            arguments[3].into_int_value(),
                            arguments[4].into_int_value(),
                            arguments[1].into_int_value(),
                            arguments[2].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "raw_static_call" => {
                        const ARGUMENTS_COUNT: usize = 4;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::raw_far_call(
                            context,
                            context.runtime().static_call,
                            arguments[0].into_int_value(),
                            arguments[1],
                            arguments[2].into_int_value(),
                            arguments[3].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "raw_static_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 3;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::raw_far_call(
                            context,
                            context.runtime().static_call_byref,
                            arguments[0].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            arguments[1].into_int_value(),
                            arguments[2].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "system_static_call" => {
                        const ARGUMENTS_COUNT: usize = 6;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::system_call(
                            context,
                            context.runtime().system_static_call,
                            arguments[0].into_int_value(),
                            arguments[1],
                            arguments[4].into_int_value(),
                            arguments[5].into_int_value(),
                            arguments[2].into_int_value(),
                            arguments[3].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "system_static_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 5;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::system_call(
                            context,
                            context.runtime().system_static_call_byref,
                            arguments[0].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            arguments[3].into_int_value(),
                            arguments[4].into_int_value(),
                            arguments[1].into_int_value(),
                            arguments[2].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "raw_delegate_call" => {
                        const ARGUMENTS_COUNT: usize = 4;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::raw_far_call(
                            context,
                            context.runtime().delegate_call,
                            arguments[0].into_int_value(),
                            arguments[1],
                            arguments[2].into_int_value(),
                            arguments[3].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "raw_delegate_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 3;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::raw_far_call(
                            context,
                            context.runtime().delegate_call_byref,
                            arguments[0].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            arguments[1].into_int_value(),
                            arguments[2].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "system_delegate_call" => {
                        const ARGUMENTS_COUNT: usize = 6;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::system_call(
                            context,
                            context.runtime().system_delegate_call,
                            arguments[0].into_int_value(),
                            arguments[1],
                            arguments[4].into_int_value(),
                            arguments[5].into_int_value(),
                            arguments[2].into_int_value(),
                            arguments[3].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "system_delegate_call_byref" => {
                        const ARGUMENTS_COUNT: usize = 5;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::system_call(
                            context,
                            context.runtime().system_delegate_call_byref,
                            arguments[0].into_int_value(),
                            context.get_global(compiler_llvm_context::GLOBAL_ACTIVE_POINTER)?,
                            arguments[3].into_int_value(),
                            arguments[4].into_int_value(),
                            arguments[1].into_int_value(),
                            arguments[2].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "set_context_u128" => {
                        const ARGUMENTS_COUNT: usize = 1;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::set_context_value(
                            context,
                            arguments[0].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "set_pubdata_price" => {
                        const ARGUMENTS_COUNT: usize = 1;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::set_pubdata_price(
                            context,
                            arguments[0].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "increment_tx_counter" => {
                        const ARGUMENTS_COUNT: usize = 0;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        compiler_llvm_context::contract::simulation::increment_tx_counter(context)
                            .map(Some)
                    }
                    identifier
                        if identifier
                            .starts_with(compiler_llvm_context::verbatim::GLOBAL_GETTER_PREFIX) =>
                    {
                        const ARGUMENTS_COUNT: usize = 0;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let index = match identifier
                            .strip_prefix(compiler_llvm_context::verbatim::GLOBAL_GETTER_PREFIX)
                        {
                            Some(identifier)
                                if identifier == compiler_llvm_context::GLOBAL_CALLDATA_POINTER =>
                            {
                                compiler_llvm_context::GLOBAL_INDEX_CALLDATA_ABI
                            }
                            Some(identifier)
                                if identifier == compiler_llvm_context::GLOBAL_CALL_FLAGS =>
                            {
                                compiler_llvm_context::GLOBAL_INDEX_CALL_FLAGS
                            }
                            Some(identifier)
                                if identifier
                                    .starts_with(compiler_llvm_context::GLOBAL_EXTRA_ABI_DATA) =>
                            {
                                match identifier
                                    .strip_prefix(compiler_llvm_context::GLOBAL_EXTRA_ABI_DATA)
                                {
                                    Some("_1") => {
                                        compiler_llvm_context::GLOBAL_INDEX_EXTRA_ABI_DATA_1
                                    }

                                    Some("_2") => {
                                        compiler_llvm_context::GLOBAL_INDEX_EXTRA_ABI_DATA_2
                                    }

                                    suffix => anyhow::bail!(
                                        "{} Invalid extra ABI data suffix `{:?}`",
                                        location,
                                        suffix
                                    ),
                                }
                            }
                            Some(identifier)
                                if identifier
                                    == compiler_llvm_context::GLOBAL_RETURN_DATA_POINTER =>
                            {
                                compiler_llvm_context::GLOBAL_INDEX_RETURN_DATA_ABI
                            }
                            identifier => anyhow::bail!(
                                "{} Invalid global variable identifier `{:?}`",
                                location,
                                identifier
                            ),
                        };

                        compiler_llvm_context::contract::simulation::get_global(context, index)
                            .map(Some)
                    }
                    identifier @ "calldata_ptr_to_active" => {
                        const ARGUMENTS_COUNT: usize = 0;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        compiler_llvm_context::contract::simulation::calldata_ptr_to_active(context)
                            .map(Some)
                    }
                    identifier @ "return_data_ptr_to_active" => {
                        const ARGUMENTS_COUNT: usize = 0;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        compiler_llvm_context::contract::simulation::return_data_ptr_to_active(
                            context,
                        )
                        .map(Some)
                    }
                    identifier @ "active_ptr_add_assign" => {
                        const ARGUMENTS_COUNT: usize = 1;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::active_ptr_add_assign(
                            context,
                            arguments[0].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "active_ptr_shrink_assign" => {
                        const ARGUMENTS_COUNT: usize = 1;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::active_ptr_shrink_assign(
                            context,
                            arguments[0].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "active_ptr_pack_assign" => {
                        const ARGUMENTS_COUNT: usize = 1;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::active_ptr_pack_assign(
                            context,
                            arguments[0].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "mul_high" => {
                        const ARGUMENTS_COUNT: usize = 2;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        let arguments = self.pop_arguments_llvm::<D, ARGUMENTS_COUNT>(context)?;
                        compiler_llvm_context::contract::simulation::multiplication_512(
                            context,
                            arguments[0].into_int_value(),
                            arguments[1].into_int_value(),
                        )
                        .map(Some)
                    }
                    identifier @ "throw" => {
                        const ARGUMENTS_COUNT: usize = 0;
                        if input_size != ARGUMENTS_COUNT {
                            anyhow::bail!(
                                "{} Internal function `{}` expected {} arguments, found {}",
                                location,
                                identifier,
                                ARGUMENTS_COUNT,
                                input_size
                            );
                        }

                        compiler_llvm_context::verbatim::throw(context)
                    }
                    identifier => anyhow::bail!(
                        "{} Found unknown internal function `{}`",
                        location,
                        identifier
                    ),
                }
            }

            Name::Pc => anyhow::bail!("{} The `PC` instruction is not supported", location),
            Name::ExtCodeCopy => {
                let _arguments = self.pop_arguments_llvm::<D, 4>(context)?;
                anyhow::bail!(
                    "{} The `EXTCODECOPY` instruction is not supported",
                    location
                )
            }
            Name::SelfDestruct => {
                let _arguments = self.pop_arguments_llvm::<D, 1>(context)?;
                anyhow::bail!(
                    "{} The `SELFDESTRUCT` instruction is not supported",
                    location
                )
            }
        }
    }

    ///
    /// Pops the specified number of arguments, converted into their LLVM values.
    ///
    fn pop_arguments_llvm<'ctx, D, const N: usize>(
        &mut self,
        context: &mut compiler_llvm_context::Context<'ctx, D>,
    ) -> anyhow::Result<[inkwell::values::BasicValueEnum<'ctx>; N]>
    where
        D: compiler_llvm_context::Dependency,
    {
        let mut arguments = Vec::with_capacity(N);
        for expression in self.arguments.drain(0..N) {
            arguments.push(expression.into_llvm(context)?.expect("Always exists").value);
        }

        Ok(arguments.try_into().expect("Always successful"))
    }

    ///
    /// Pops the specified number of arguments.
    ///
    fn pop_arguments<'ctx, D, const N: usize>(
        &mut self,
        context: &mut compiler_llvm_context::Context<'ctx, D>,
    ) -> anyhow::Result<[compiler_llvm_context::Argument<'ctx>; N]>
    where
        D: compiler_llvm_context::Dependency,
    {
        let mut arguments = Vec::with_capacity(N);
        for expression in self.arguments.drain(0..N) {
            arguments.push(expression.into_llvm(context)?.expect("Always exists"));
        }

        Ok(arguments.try_into().expect("Always successful"))
    }

    ///
    /// Pops the specified number of arguments, converted into their LLVM values.
    ///
    /// This function inverts the order of event topics, taking into account its behavior in EVM.
    ///
    fn pop_arguments_llvm_log<'ctx, D, const N: usize>(
        &mut self,
        context: &mut compiler_llvm_context::Context<'ctx, D>,
    ) -> anyhow::Result<[inkwell::values::BasicValueEnum<'ctx>; N]>
    where
        D: compiler_llvm_context::Dependency,
    {
        self.arguments[2..].reverse();
        let mut arguments = Vec::with_capacity(N);
        for expression in self.arguments.drain(0..N) {
            arguments.push(expression.into_llvm(context)?.expect("Always exists").value);
        }
        arguments[2..].reverse();

        Ok(arguments.try_into().expect("Always successful"))
    }
}
