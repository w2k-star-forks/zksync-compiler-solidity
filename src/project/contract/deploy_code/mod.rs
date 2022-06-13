//!
//! The `solc --standard-json` contract deploy code.
//!

pub mod evm;
pub mod yul;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

use compiler_llvm_context::Dependency;
use compiler_llvm_context::WriteLLVM;

use crate::dump_flag::DumpFlag;
use crate::evm::ethereal_ir::EtherealIR;
use crate::project::contract::state::State as ContractBuildState;
use crate::project::Project;
use crate::yul::parser::statement::object::Object;

use self::evm::EVM;
use self::yul::Yul;

///
/// The `solc --standard-json` contract deploy code.
///
#[derive(Debug)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum DeployCode {
    /// The Yul deploy code representation.
    Yul(Yul),
    /// The EVM legacy assembly deploy code representation.
    EVM(EVM),
}

impl DeployCode {
    ///
    /// A shortcut constructor.
    ///
    pub fn new_yul(object: Object) -> Self {
        Self::Yul(Yul::new(object))
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn new_evm(ethereal_ir: EtherealIR) -> Self {
        Self::EVM(EVM::new(ethereal_ir))
    }

    ///
    /// Sets the runtime contract hash.
    ///
    pub fn set_runtime_code_hash(&mut self, hash: String) {
        match self {
            Self::Yul(inner) => inner.runtime_code_hash = Some(hash),
            Self::EVM(inner) => inner.runtime_code_hash = Some(hash),
        }
    }

    ///
    /// Takes the runtime code hash.
    ///
    /// # Panics
    /// If the hash has not been set.
    ///
    pub fn runtime_code_hash(&self) -> &str {
        match self {
            Self::Yul(inner) => inner.runtime_code_hash.as_deref(),
            Self::EVM(inner) => inner.runtime_code_hash.as_deref(),
        }
        .expect("The runtime code hash must be set before compiling")
    }

    ///
    /// Compiles the specified contract part, setting its build artifacts.
    ///
    pub fn compile(
        mut self,
        project: Arc<RwLock<Project>>,
        optimizer_settings: compiler_llvm_context::OptimizerSettings,
        dump_flags: &[DumpFlag],
    ) -> anyhow::Result<compiler_llvm_context::Build> {
        let llvm = inkwell::context::Context::create();
        let optimizer = compiler_llvm_context::Optimizer::new(optimizer_settings)?;
        let dump_flags = compiler_llvm_context::DumpFlag::initialize(
            dump_flags.contains(&DumpFlag::Yul),
            dump_flags.contains(&DumpFlag::EthIR),
            dump_flags.contains(&DumpFlag::EVM),
            false,
            dump_flags.contains(&DumpFlag::LLVM),
            dump_flags.contains(&DumpFlag::Assembly),
        );
        let full_path = project
            .read()
            .expect("Sync")
            .resolve_path(self.identifier())?;
        let mut context = match self {
            Self::Yul(_) => compiler_llvm_context::Context::new(
                &llvm,
                full_path.as_str(),
                compiler_llvm_context::CodeType::Deploy,
                optimizer,
                Some(project.clone()),
                dump_flags,
            ),
            Self::EVM(_) => {
                let version = project.read().expect("Sync").version.to_owned();
                compiler_llvm_context::Context::new_evm(
                    &llvm,
                    full_path.as_str(),
                    compiler_llvm_context::CodeType::Deploy,
                    optimizer,
                    Some(project.clone()),
                    dump_flags,
                    compiler_llvm_context::ContextEVMData::new(version),
                )
            }
        };

        let factory_dependencies = self.drain_factory_dependencies();

        self.declare(&mut context).map_err(|error| {
            anyhow::anyhow!(
                "The contract `{}` LLVM IR generator declaration pass error: {}",
                full_path,
                error
            )
        })?;
        self.into_llvm(&mut context).map_err(|error| {
            anyhow::anyhow!(
                "The contract `{}` LLVM IR generator definition pass error: {}",
                full_path,
                error
            )
        })?;

        let mut build = context.build(full_path.as_str())?;
        for dependency in factory_dependencies.into_iter() {
            let full_path = project
                .read()
                .expect("Sync")
                .identifier_paths
                .get(dependency.as_str())
                .cloned()
                .unwrap_or_else(|| panic!("Dependency `{}` full path not found", dependency));
            let hash = match project
                .read()
                .expect("Sync")
                .contract_states
                .get(full_path.as_str())
            {
                Some(ContractBuildState::Build(build)) => build.deploy_build.hash.to_owned(),
                Some(_) => {
                    panic!("Dependency `{}` must be built at this point", full_path)
                }
                None => anyhow::bail!(
                    "Dependency contract `{}` not found in the project",
                    full_path
                ),
            };
            build.factory_dependencies.insert(hash, full_path);
        }
        Ok(build)
    }

    ///
    /// Returns the contract identifier, which is:
    /// - the Yul object identifier for Yul
    /// - the full contract path for the EVM legacy assembly
    ///
    pub fn identifier(&self) -> &str {
        match self {
            Self::Yul(ref yul) => yul.object.identifier.as_str(),
            Self::EVM(ref evm) => evm.ethereal_ir.full_path.as_str(),
        }
    }

    ///
    /// Extract factory dependencies.
    ///
    pub fn drain_factory_dependencies(&mut self) -> HashSet<String> {
        match self {
            Self::Yul(ref mut yul) => yul.object.factory_dependencies.drain(),
            Self::EVM(ref mut evm) => evm.ethereal_ir.factory_dependencies.drain(),
        }
        .collect()
    }
}

impl<D> WriteLLVM<D> for DeployCode
where
    D: Dependency,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        compiler_llvm_context::DeployCodeFunction::new(
            compiler_llvm_context::DummyLLVMWritable::default(),
            self.runtime_code_hash().to_owned(),
        )
        .declare(context)
    }

    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        let runtime_code_hash = self.runtime_code_hash().to_owned();
        match self {
            Self::Yul(inner) => {
                compiler_llvm_context::DeployCodeFunction::new(inner, runtime_code_hash)
                    .into_llvm(context)
            }
            Self::EVM(inner) => {
                compiler_llvm_context::DeployCodeFunction::new(inner, runtime_code_hash)
                    .into_llvm(context)
            }
        }
    }
}
