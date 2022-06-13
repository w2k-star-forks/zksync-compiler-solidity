//!
//! The contract data representation.
//!

pub mod deploy_code;
pub mod runtime_code;
pub mod state;

use std::sync::Arc;
use std::sync::RwLock;

use crate::build::contract::Contract as ContractBuild;
use crate::dump_flag::DumpFlag;
use crate::evm::assembly::data::Data as AssemblyData;
use crate::evm::assembly::Assembly;
use crate::evm::ethereal_ir::EtherealIR;
use crate::project::Project;
use crate::yul::parser::statement::object::Object;

use self::deploy_code::DeployCode;
use self::runtime_code::RuntimeCode;

///
/// The contract data representation.
///
#[derive(Debug)]
pub struct Contract {
    /// The absolute file path.
    pub path: String,
    /// The auxiliary contract identifier. Used to identify Yul objects.
    pub identifier: String,
    /// The deploy code data.
    pub deploy_code: DeployCode,
    /// The runtime code data.
    pub runtime_code: RuntimeCode,
    /// The ABI specification JSON.
    pub abi: Option<serde_json::Value>,
}

impl Contract {
    ///
    /// A shortcut constructor for Yul.
    ///
    pub fn try_from_yul(
        path: String,
        object: Object,
        abi: Option<serde_json::Value>,
    ) -> anyhow::Result<Self> {
        let mut deploy_object = object;
        let runtime_object = *deploy_object
            .inner_object
            .take()
            .ok_or_else(|| anyhow::anyhow!("The runtime object must be always"))?;

        let identifier = deploy_object.identifier.clone();

        let deploy_code = DeployCode::new_yul(deploy_object);
        let runtime_code = RuntimeCode::new_yul(runtime_object);

        Ok(Self {
            path,
            identifier,
            deploy_code,
            runtime_code,
            abi,
        })
    }

    ///
    /// A shortcut constructor for the EVM legacy assembly.
    ///
    pub fn try_from_evm(
        path: String,
        version: &semver::Version,
        assembly: Assembly,
        abi: Option<serde_json::Value>,
        dump_flags: &[DumpFlag],
    ) -> anyhow::Result<Self> {
        if dump_flags.contains(&DumpFlag::EVM) {
            println!(
                "Contract `{}` {} code EVM:\n\n{}",
                compiler_llvm_context::CodeType::Deploy,
                path,
                assembly
            );
        }
        let deploy_code_blocks = EtherealIR::get_blocks(
            version.to_owned(),
            compiler_llvm_context::CodeType::Deploy,
            assembly
                .code
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Deploy code instructions not found"))?,
        )?;

        let data = assembly
            .data
            .ok_or_else(|| anyhow::anyhow!("Runtime code data not found"))?
            .remove("0")
            .expect("Always exists");
        if dump_flags.contains(&DumpFlag::EVM) {
            println!(
                "Contract `{}` {} code EVM:\n\n{}",
                path,
                compiler_llvm_context::CodeType::Runtime,
                data
            );
        };
        let runtime_code_instructions = match data {
            AssemblyData::Assembly(assembly) => assembly
                .code
                .ok_or_else(|| anyhow::anyhow!("Runtime code instructions not found"))?,
            AssemblyData::Hash(hash) => {
                anyhow::bail!("Expected runtime code instructions, found hash `{}`", hash)
            }
            AssemblyData::Path(path) => {
                anyhow::bail!("Expected runtime code instructions, found path `{}`", path)
            }
        };
        let runtime_code_blocks = EtherealIR::get_blocks(
            version.to_owned(),
            compiler_llvm_context::CodeType::Runtime,
            runtime_code_instructions.as_slice(),
        )?;

        let deploy_ethereal_ir = EtherealIR::new(
            version.to_owned(),
            path.clone(),
            deploy_code_blocks,
            assembly.deploy_factory_dependencies,
        )?;
        if dump_flags.contains(&DumpFlag::EthIR) {
            println!(
                "Contract `{}` {} code Ethereal IR:\n\n{}",
                path,
                compiler_llvm_context::CodeType::Deploy,
                deploy_ethereal_ir
            );
        }
        let deploy_code = DeployCode::new_evm(deploy_ethereal_ir);

        let runtime_ethereal_ir = EtherealIR::new(
            version.to_owned(),
            path.clone(),
            runtime_code_blocks,
            assembly.runtime_factory_dependencies,
        )?;
        if dump_flags.contains(&DumpFlag::EthIR) {
            println!(
                "Contract `{}` {} code Ethereal IR:\n\n{}",
                path,
                compiler_llvm_context::CodeType::Runtime,
                runtime_ethereal_ir
            );
        }
        let runtime_code = RuntimeCode::new_evm(runtime_ethereal_ir);

        Ok(Self {
            path: path.clone(),
            identifier: path,
            deploy_code,
            runtime_code,
            abi,
        })
    }

    ///
    /// Compiles the specified contract, setting its build artifacts.
    ///
    pub fn compile(
        mut self,
        project: Arc<RwLock<Project>>,
        optimizer_settings: compiler_llvm_context::OptimizerSettings,
        dump_flags: Vec<DumpFlag>,
    ) -> anyhow::Result<ContractBuild> {
        let runtime_build = self.runtime_code.compile(
            project.clone(),
            optimizer_settings.clone(),
            dump_flags.as_slice(),
        )?;

        self.deploy_code
            .set_runtime_code_hash(runtime_build.hash.clone());
        let deploy_build =
            self.deploy_code
                .compile(project, optimizer_settings, dump_flags.as_slice())?;

        Ok(ContractBuild::new(
            self.path,
            self.identifier,
            deploy_build,
            runtime_build,
            self.abi,
        ))
    }
}
