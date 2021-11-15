//!
//! Translates the contract creation instructions.
//!

use inkwell::values::BasicValue;

use crate::generator::llvm::argument::Argument;
use crate::generator::llvm::intrinsic::Intrinsic;
use crate::generator::llvm::Context as LLVMContext;

///
/// Translates the contract `create` instruction.
///
pub fn create<'ctx, 'src>(
    context: &mut LLVMContext<'ctx, 'src>,
    arguments: [inkwell::values::BasicValueEnum<'ctx>; 3],
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    create2(
        context,
        [
            arguments[0],
            arguments[1],
            arguments[2],
            context.field_const(0).as_basic_value_enum(),
        ],
    )
}

///
/// Translates the contract `create2` instruction.
///
pub fn create2<'ctx, 'src>(
    context: &mut LLVMContext<'ctx, 'src>,
    arguments: [inkwell::values::BasicValueEnum<'ctx>; 4],
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    let input_offset = context.builder.build_int_add(
        arguments[1].into_int_value(),
        context.field_const(compiler_common::size::FIELD as u64),
        "create_input_offset",
    );
    let input_size = context.builder.build_int_sub(
        arguments[2].into_int_value(),
        context.field_const(compiler_common::size::FIELD as u64),
        "create_input_size",
    );

    let intrinsic = context.get_intrinsic_function(Intrinsic::SwitchContext);
    context.build_call(intrinsic, &[], "create_switch_context");

    let child_pointer_input = context.access_memory(
        context.field_const(
            (compiler_common::abi::OFFSET_CALLDATA_SIZE * compiler_common::size::FIELD) as u64,
        ),
        compiler_common::AddressSpace::Child,
        "create_child_pointer_input",
    );
    context.build_store(child_pointer_input, input_size);

    let child_pointer_entry_data = context.access_memory(
        context.field_const(
            (compiler_common::abi::OFFSET_ENTRY_DATA * compiler_common::size::FIELD) as u64,
        ),
        compiler_common::AddressSpace::Child,
        "create_child_pointer_entry_data",
    );
    context.build_store(
        child_pointer_entry_data,
        context.field_const(1).as_basic_value_enum(),
    );

    let destination = context.access_memory(
        context.field_const(
            (compiler_common::abi::OFFSET_CALL_RETURN_DATA * compiler_common::size::FIELD) as u64,
        ),
        compiler_common::AddressSpace::Child,
        "create_child_input_destination",
    );
    let source = context.access_memory(
        input_offset,
        compiler_common::AddressSpace::Heap,
        "create_child_input_source",
    );

    context.build_memcpy(
        Intrinsic::MemoryCopyToChild,
        destination,
        source,
        input_size,
        "create_memcpy_to_child",
    );

    let intrinsic = context.get_intrinsic_function(Intrinsic::FarCall);
    let address = context
        .field_type()
        .const_int_from_string(
            "1234567812345678123456781234567812345678", // TODO: get from the special event call
            inkwell::types::StringRadix::Hexadecimal,
        )
        .expect("Always valid");
    let call_definition = context.builder.build_left_shift(
        address,
        context.field_const((compiler_common::bitlength::BYTE * 4) as u64),
        "",
    );
    let is_call_successful = context
        .build_call(
            intrinsic,
            &[call_definition.as_basic_value_enum()],
            "create_call",
        )
        .expect("Intrinsic always returns a flag");

    Some(is_call_successful)
}

///
/// Translates the `datasize` instruction, which is actually used to set the hash of the contract
/// being created, or other related auxiliary data.
///
pub fn datasize<'ctx, 'src>(
    context: &mut LLVMContext<'ctx, 'src>,
    mut arguments: [Argument<'ctx>; 1],
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    let literal = arguments[0].original.take().expect("Always exists");

    if literal.ends_with("_deployed") || literal.as_str() == context.object() {
        return Some(context.field_const(0).as_basic_value_enum());
    }

    Some(
        context
            .field_const(compiler_common::size::FIELD as u64)
            .as_basic_value_enum(),
    )
}

///
/// Translates the `dataoffset` instruction, which is actually used to set the hash of the contract
/// being created, or other related auxiliary data.
///
pub fn dataoffset<'ctx, 'src>(
    context: &mut LLVMContext<'ctx, 'src>,
    mut arguments: [Argument<'ctx>; 1],
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    let literal = arguments[0].original.take().expect("Always exists");

    if literal.ends_with("_deployed") {
        return Some(context.field_const(0).as_basic_value_enum());
    }

    let dependency_bytecode = context.compile_dependency(literal.as_str());
    let dependency_hash_str = compiler_common::hashes::keccak256(dependency_bytecode.as_slice());
    let dependency_hash_value = context
        .field_type()
        .const_int_from_string(
            dependency_hash_str.as_str(),
            inkwell::types::StringRadix::Hexadecimal,
        )
        .expect("Always valid");

    Some(dependency_hash_value.as_basic_value_enum())
}

///
/// Translates the `datacopy` instruction, which is actually used to set the hash of the contract
/// being created, or other related auxiliary data.
///
pub fn datacopy<'ctx, 'src>(
    context: &mut LLVMContext<'ctx, 'src>,
    arguments: [inkwell::values::BasicValueEnum<'ctx>; 3],
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    let pointer = context.access_memory(
        arguments[0].into_int_value(),
        compiler_common::AddressSpace::Heap,
        "datacopy_pointer",
    );
    context.build_store(pointer, arguments[1]);

    None
}
