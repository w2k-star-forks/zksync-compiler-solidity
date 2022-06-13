//!
//! The `solc --standard-json` output contract EVM data.
//!

pub mod bytecode;

use serde::Deserialize;
use serde::Serialize;

use crate::evm::assembly::Assembly;

use self::bytecode::Bytecode;

///
/// The `solc --standard-json` output contract EVM data.
///
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EVM {
    /// The contract assembly code.
    #[serde(rename = "legacyAssembly")]
    pub assembly: Option<Assembly>,
    /// The contract deploy bytecode.
    /// Is reset by that of zkEVM before yielding the compiled project artifacts.
    #[serde(rename = "bytecode")]
    pub deploy_bytecode: Option<Bytecode>,
    /// The contract runtime bytecode.
    /// Is reset by that of zkEVM before yielding the compiled project artifacts.
    #[serde(rename = "deployedBytecode")]
    pub runtime_bytecode: Option<Bytecode>,
}

impl EVM {
    ///
    /// A shortcut constructor for the zkEVM bytecode.
    ///
    pub fn new_zkevm_bytecode(deploy_bytecode: String, runtime_bytecode: String) -> Self {
        Self {
            assembly: None,
            deploy_bytecode: Some(Bytecode::new(deploy_bytecode)),
            runtime_bytecode: Some(Bytecode::new(runtime_bytecode)),
        }
    }
}
