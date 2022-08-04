//!
//! Translates the CODECOPY use cases.
//!

///
/// Translates the contract hash copying.
///
pub fn contract_hash<'ctx, D>(
    context: &mut compiler_llvm_context::Context<'ctx, D>,
    offset: inkwell::values::IntValue<'ctx>,
    value: inkwell::values::IntValue<'ctx>,
) -> anyhow::Result<Option<inkwell::values::BasicValueEnum<'ctx>>>
where
    D: compiler_llvm_context::Dependency,
{
    let offset = context.builder().build_int_add(
        offset,
        context.field_const((compiler_common::SIZE_X32 + compiler_common::SIZE_FIELD) as u64),
        "datacopy_contract_hash_offset",
    );

    compiler_llvm_context::memory::store(context, offset, value)?;

    Ok(None)
}

///
/// Translates the library marker copying.
///
pub fn library_marker<'ctx, D>(
    context: &mut compiler_llvm_context::Context<'ctx, D>,
    offset: &str,
    value: &str,
) -> anyhow::Result<Option<inkwell::values::BasicValueEnum<'ctx>>>
where
    D: compiler_llvm_context::Dependency,
{
    compiler_llvm_context::memory::store_byte(
        context,
        context.field_const_str_hex(offset),
        context.field_const_str_hex(value),
    )?;

    Ok(None)
}

///
/// Translates the static data copying.
///
pub fn static_data<'ctx, D>(
    context: &mut compiler_llvm_context::Context<'ctx, D>,
    destination: inkwell::values::IntValue<'ctx>,
    source: &str,
) -> anyhow::Result<Option<inkwell::values::BasicValueEnum<'ctx>>>
where
    D: compiler_llvm_context::Dependency,
{
    let mut offset = 0;
    for (index, chunk) in source
        .chars()
        .collect::<Vec<char>>()
        .chunks(compiler_common::SIZE_FIELD * 2)
        .enumerate()
    {
        let mut value_string = chunk.iter().collect::<String>();
        value_string.push_str(
            "0".repeat((compiler_common::SIZE_FIELD * 2) - chunk.len())
                .as_str(),
        );

        let datacopy_destination = context.builder().build_int_add(
            destination,
            context.field_const(offset as u64),
            format!("datacopy_destination_index_{}", index).as_str(),
        );
        let datacopy_value = context.field_const_str(value_string.as_str());
        compiler_llvm_context::memory::store(context, datacopy_destination, datacopy_value)?;
        offset += chunk.len() / 2;
    }

    Ok(None)
}
