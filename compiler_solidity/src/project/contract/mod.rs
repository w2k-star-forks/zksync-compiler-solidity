//!
//! The contract data representation.
//!

pub mod source;
pub mod state;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

use compiler_llvm_context::WriteLLVM;

use crate::dump_flag::DumpFlag;
use crate::project::Project;

use self::source::Source;
use self::state::State;

///
/// The contract data representation.
///
#[derive(Debug, Clone)]
pub struct Contract {
    /// The absolute file path.
    pub path: String,
    /// The source code data.
    pub source: Source,
    /// The ABI specification JSON.
    pub abi: Option<serde_json::Value>,
}

impl Contract {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(path: String, source: Source, abi: Option<serde_json::Value>) -> Self {
        Self { path, source, abi }
    }

    ///
    /// Returns the contract identifier, which is:
    /// - the Yul object identifier for Yul
    /// - the full contract path for EVM
    ///
    pub fn identifier(&self) -> &str {
        match self.source {
            Source::Yul(ref yul) => yul.object.identifier.as_str(),
            Source::EVM(ref evm) => evm.assembly.full_path(),
        }
    }

    ///
    /// Extract factory dependencies.
    ///
    pub fn drain_factory_dependencies(&mut self) -> HashSet<String> {
        match self.source {
            Source::Yul(ref mut yul) => yul.object.factory_dependencies.drain(),
            Source::EVM(ref mut evm) => evm.assembly.factory_dependencies.drain(),
        }
        .collect()
    }

    ///
    /// Compiles the specified contract, setting its build artifacts.
    ///
    pub fn compile(
        mut self,
        project: Arc<RwLock<Project>>,
        target_machine: compiler_llvm_context::TargetMachine,
        optimizer_settings: compiler_llvm_context::OptimizerSettings,
        dump_flags: Vec<DumpFlag>,
    ) -> anyhow::Result<compiler_llvm_context::Build> {
        let llvm = inkwell::context::Context::create();
        let optimizer = compiler_llvm_context::Optimizer::new(target_machine, optimizer_settings);
        let dump_flags = compiler_llvm_context::DumpFlag::initialize(
            dump_flags.contains(&DumpFlag::Yul),
            dump_flags.contains(&DumpFlag::EthIR),
            dump_flags.contains(&DumpFlag::EVM),
            false,
            dump_flags.contains(&DumpFlag::LLVM),
            dump_flags.contains(&DumpFlag::Assembly),
        );
        let mut context = compiler_llvm_context::Context::new(
            &llvm,
            self.path.as_str(),
            optimizer,
            Some(project.clone()),
            dump_flags,
        );
        context.set_solidity_data(compiler_llvm_context::ContextSolidityData::default());
        if let Source::EVM(_) = self.source {
            let version = project.read().expect("Sync").version.to_owned();
            let evmla_data = compiler_llvm_context::ContextEVMLAData::new(version);
            context.set_evmla_data(evmla_data);
        }

        let factory_dependencies = self.drain_factory_dependencies();

        self.source.declare(&mut context).map_err(|error| {
            anyhow::anyhow!(
                "The contract `{}` LLVM IR generator declaration pass error: {}",
                self.path,
                error
            )
        })?;
        self.source.into_llvm(&mut context).map_err(|error| {
            anyhow::anyhow!(
                "The contract `{}` LLVM IR generator definition pass error: {}",
                self.path,
                error
            )
        })?;

        let mut build = context.build(self.path.as_str())?;
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
                Some(State::Build(build)) => build.build.hash.to_owned(),
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
}

impl<D> WriteLLVM<D> for Contract
where
    D: compiler_llvm_context::Dependency,
{
    fn declare(&mut self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.source.declare(context)
    }

    fn into_llvm(self, context: &mut compiler_llvm_context::Context<D>) -> anyhow::Result<()> {
        self.source.into_llvm(context)
    }
}
