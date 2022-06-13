//!
//! The `solc --standard-json` contract Yul deploy code.
//!

use crate::yul::parser::statement::object::Object;

///
/// The `solc --standard-json` contract Yul deploy code.
///
#[derive(Debug)]
pub struct Yul {
    /// The Yul AST object.
    pub object: Object,
    /// The runtime code hash. Must be set before compiling.
    pub runtime_code_hash: Option<String>,
}

impl Yul {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(object: Object) -> Self {
        Self {
            object,
            runtime_code_hash: None,
        }
    }
}

impl<D> compiler_llvm_context::WriteLLVM<D> for Yul
where
    D: compiler_llvm_context::Dependency,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.object.declare(context)
    }

    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.object.into_llvm(context)
    }
}
