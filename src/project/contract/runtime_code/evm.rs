//!
//! The `solc --standard-json` contract runtime code EVM legacy assembly.
//!

use crate::evm::ethereal_ir::EtherealIR;

///
/// The `solc --standard-json` contract runtime code EVM legacy assembly.
///
#[derive(Debug)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub struct EVM {
    /// The EVM legacy assembly Ethereal IR.
    pub ethereal_ir: EtherealIR,
}

impl EVM {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(ethereal_ir: EtherealIR) -> Self {
        Self { ethereal_ir }
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for EVM
where
    D: compiler_llvm_context::Dependency,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.ethereal_ir.declare(context)
    }

    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.ethereal_ir.into_llvm(context)
    }
}
