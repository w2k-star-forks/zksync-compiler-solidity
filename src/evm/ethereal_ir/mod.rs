//!
//! The Ethereal IR representation of the EVM bytecode.
//!

pub mod function;

use std::collections::HashMap;
use std::collections::HashSet;

use crate::evm::assembly::instruction::Instruction;

use self::function::block::Block;
use self::function::Function;

///
/// The Ethereal IR representation of the EVM bytecode.
///
#[derive(Debug)]
pub struct EtherealIR {
    /// The Solidity compiler version.
    pub solc_version: semver::Version,
    /// The contract full path.
    pub full_path: String,
    /// The all-inlined function representation.
    pub function: Function,
    /// The factory dependencies.
    pub factory_dependencies: HashSet<String>,
}

impl EtherealIR {
    /// The default entry function name.
    pub const DEFAULT_ENTRY_FUNCTION_NAME: &'static str = "function_main";

    /// The `deploy address` immutable storage key.
    pub const DEPLOY_ADDRESS_STORAGE_KEY: &'static str = "deploy_address";

    /// The blocks hashmap initial capacity.
    pub const BLOCKS_HASHMAP_DEFAULT_CAPACITY: usize = 64;

    ///
    /// Assembles a sequence of functions from the sequence of instructions.
    ///
    pub fn new(
        solc_version: semver::Version,
        full_path: String,
        blocks: HashMap<compiler_llvm_context::FunctionBlockKey, Block>,
        factory_dependencies: HashSet<String>,
    ) -> anyhow::Result<Self> {
        let mut visited = HashSet::with_capacity(blocks.len());
        let function = Function::new(solc_version.clone(), &blocks, &mut visited)?;

        Ok(Self {
            solc_version,
            full_path,
            function,
            factory_dependencies,
        })
    }

    ///
    /// Gets blocks for the specified type of the contract code.
    ///
    pub fn get_blocks(
        solc_version: semver::Version,
        code_type: compiler_llvm_context::CodeType,
        instructions: &[Instruction],
    ) -> anyhow::Result<HashMap<compiler_llvm_context::FunctionBlockKey, Block>> {
        let mut blocks = HashMap::with_capacity(Self::BLOCKS_HASHMAP_DEFAULT_CAPACITY);
        let mut offset = 0;

        while offset < instructions.len() {
            let (block, size) = Block::try_from_instructions(
                solc_version.clone(),
                code_type,
                &instructions[offset..],
            )?;
            blocks.insert(
                compiler_llvm_context::FunctionBlockKey::new(code_type, block.key.tag.clone()),
                block,
            );
            offset += size;
        }

        Ok(blocks)
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for EtherealIR
where
    D: compiler_llvm_context::Dependency,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.function.declare(context)?;

        Ok(())
    }

    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        context.evm_mut().stack = vec![];

        self.function.into_llvm(context)?;

        Ok(())
    }
}

impl std::fmt::Display for EtherealIR {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.function)?;

        Ok(())
    }
}
