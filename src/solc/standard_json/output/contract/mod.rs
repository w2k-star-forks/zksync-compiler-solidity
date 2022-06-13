//!
//! The `solc --standard-json` output contract.
//!

pub mod evm;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use self::evm::EVM;

///
/// The `solc --standard-json` output contract.
///
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    /// The contract optimized IR code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir_optimized: Option<String>,
    /// The contract ABI representation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<serde_json::Value>,
    /// Contract's bytecode and related objects
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evm: Option<EVM>,

    /// The contract's zkEVM deploy bytecode hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy_hash: Option<String>,
    /// The contracts factory dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy_factory_dependencies: Option<BTreeMap<String, String>>,

    /// The contract's zkEVM deploy bytecode hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_hash: Option<String>,
    /// The contracts factory dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_factory_dependencies: Option<BTreeMap<String, String>>,
}
