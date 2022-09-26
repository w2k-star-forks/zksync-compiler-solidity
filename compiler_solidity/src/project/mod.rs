//!
//! The processed input data representation.
//!

pub mod contract;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;

use crate::build::contract::Contract as ContractBuild;
use crate::build::Build;
use crate::dump_flag::DumpFlag;
use crate::project::contract::source::Source;
use crate::project::contract::state::State;
use crate::yul::lexer::Lexer;
use crate::yul::parser::statement::object::Object;

use self::contract::state::State as ContractState;
use self::contract::Contract;

///
/// The processes input data representation.
///
#[derive(Debug)]
pub struct Project {
    /// The Solidity project version.
    pub version: semver::Version,
    /// The contract data,
    pub contract_states: BTreeMap<String, ContractState>,
    /// The mapping of auxiliary identifiers, e.g. Yul object names, to full contract paths.
    pub identifier_paths: BTreeMap<String, String>,
    /// The library addresses.
    pub libraries: BTreeMap<String, BTreeMap<String, String>>,
}

impl Project {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        version: semver::Version,
        contracts: BTreeMap<String, Contract>,
        libraries: BTreeMap<String, BTreeMap<String, String>>,
    ) -> Self {
        let mut identifier_paths = BTreeMap::new();
        for (path, contract) in contracts.iter() {
            identifier_paths.insert(contract.identifier().to_owned(), path.to_owned());
        }

        Self {
            version,
            contract_states: contracts
                .into_iter()
                .map(|(path, contract)| (path, ContractState::Source(contract)))
                .collect(),
            identifier_paths,
            libraries,
        }
    }

    ///
    /// Compiles the specified contract, setting its build artifacts.
    ///
    pub fn compile(
        project: Arc<RwLock<Self>>,
        contract_path: &str,
        target_machine: compiler_llvm_context::TargetMachine,
        optimizer_settings: compiler_llvm_context::OptimizerSettings,
        dump_flags: Vec<DumpFlag>,
    ) {
        let mut project_guard = project.write().expect("Sync");
        match project_guard
            .contract_states
            .remove(contract_path)
            .expect("Always exists")
        {
            ContractState::Source(mut contract) => {
                let waiter = ContractState::waiter();
                project_guard.contract_states.insert(
                    contract_path.to_owned(),
                    ContractState::Waiter(waiter.clone()),
                );
                std::mem::drop(project_guard);

                let identifier = contract.identifier().to_owned();
                let abi = contract.abi.take();
                match contract.compile(
                    project.clone(),
                    target_machine,
                    optimizer_settings,
                    dump_flags,
                ) {
                    Ok(build) => {
                        let build =
                            ContractBuild::new(contract_path.to_owned(), identifier, build, abi);
                        project
                            .write()
                            .expect("Sync")
                            .contract_states
                            .insert(contract_path.to_owned(), ContractState::Build(build));
                        waiter.1.notify_all();
                    }
                    Err(error) => {
                        project
                            .write()
                            .expect("Sync")
                            .contract_states
                            .insert(contract_path.to_owned(), ContractState::Error(error));
                        waiter.1.notify_all();
                    }
                }
            }
            ContractState::Waiter(waiter) => {
                project_guard.contract_states.insert(
                    contract_path.to_owned(),
                    ContractState::Waiter(waiter.clone()),
                );
                std::mem::drop(project_guard);

                let _guard = waiter.1.wait(waiter.0.lock().expect("Sync"));
            }
            ContractState::Build(build) => {
                project_guard
                    .contract_states
                    .insert(contract_path.to_owned(), ContractState::Build(build));
            }
            ContractState::Error(error) => {
                project_guard
                    .contract_states
                    .insert(contract_path.to_owned(), ContractState::Error(error));
            }
        }
    }

    ///
    /// Compiles all contracts, returning their build artifacts.
    ///
    #[allow(clippy::needless_collect)]
    pub fn compile_all(
        self,
        target_machine: compiler_llvm_context::TargetMachine,
        optimizer_settings: compiler_llvm_context::OptimizerSettings,
        dump_flags: Vec<DumpFlag>,
    ) -> anyhow::Result<Build> {
        let project = Arc::new(RwLock::new(self));

        let contract_paths: Vec<String> = project
            .read()
            .expect("Sync")
            .contract_states
            .keys()
            .cloned()
            .collect();
        let _: Vec<()> = contract_paths
            .into_par_iter()
            .map(|contract_path| {
                Self::compile(
                    project.clone(),
                    contract_path.as_str(),
                    target_machine.clone(),
                    optimizer_settings.clone(),
                    dump_flags.clone(),
                );
            })
            .collect();

        let project = Arc::try_unwrap(project)
            .expect("No other references must exist at this point")
            .into_inner()
            .expect("Sync");
        let mut build = Build::default();
        for (path, state) in project.contract_states.into_iter() {
            match state {
                State::Build(contract_build) => {
                    build.contracts.insert(path, contract_build);
                }
                State::Error(error) => return Err(error),
                _ => panic!("Contract `{}` must be built at this point", path),
            }
        }
        Ok(build)
    }

    ///
    /// Parses the default Yul source code and returns the source data.
    ///
    pub fn try_from_default_yul(path: &Path, version: &semver::Version) -> anyhow::Result<Self> {
        let yul = std::fs::read_to_string(path)
            .map_err(|error| anyhow::anyhow!("Yul file {:?} reading error: {}", path, error))?;
        let mut lexer = Lexer::new(yul.clone());
        let path = path.to_string_lossy().to_string();
        let object = Object::parse(&mut lexer, None)
            .map_err(|error| anyhow::anyhow!("Yul object `{}` parsing error: {}", path, error,))?;

        let mut project_contracts = BTreeMap::new();
        project_contracts.insert(
            path.clone(),
            Contract::new(path, Source::new_yul(yul, object), None),
        );
        Ok(Self::new(
            version.to_owned(),
            project_contracts,
            BTreeMap::new(),
        ))
    }

    ///
    /// Parses the test Yul source code and returns the source data.
    ///
    /// Only for integration testing purposes.
    ///
    pub fn try_from_test_yul(yul: &str, version: &semver::Version) -> anyhow::Result<Self> {
        let mut lexer = Lexer::new(yul.to_owned());
        let path = "Test".to_owned();
        let object = Object::parse(&mut lexer, None)
            .map_err(|error| anyhow::anyhow!("Yul object `{}` parsing error: {}", path, error,))?;

        let mut project_contracts = BTreeMap::new();
        project_contracts.insert(
            path.clone(),
            Contract::new(path, Source::new_yul(yul.to_owned(), object), None),
        );
        Ok(Self::new(
            version.to_owned(),
            project_contracts,
            BTreeMap::new(),
        ))
    }
}

impl compiler_llvm_context::Dependency for Project {
    fn compile(
        project: Arc<RwLock<Self>>,
        identifier: &str,
        target_machine: compiler_llvm_context::TargetMachine,
        optimizer_settings: compiler_llvm_context::OptimizerSettings,
        dump_flags: Vec<compiler_llvm_context::DumpFlag>,
    ) -> anyhow::Result<String> {
        let contract_path = project.read().expect("Lock").resolve_path(identifier)?;

        Self::compile(
            project.clone(),
            contract_path.as_str(),
            target_machine,
            optimizer_settings,
            DumpFlag::from_context(dump_flags.as_slice()),
        );

        match project
            .read()
            .expect("Lock")
            .contract_states
            .get(contract_path.as_str())
        {
            Some(ContractState::Build(build)) => Ok(build.build.hash.to_owned()),
            Some(ContractState::Error(error)) => anyhow::bail!(
                "Dependency contract `{}` compiling error: {}",
                identifier,
                error
            ),
            Some(_) => panic!(
                "Dependency contract `{}` must be built at this point",
                contract_path
            ),
            None => anyhow::bail!(
                "Dependency contract `{}` not found in the project",
                contract_path
            ),
        }
    }

    fn resolve_path(&self, identifier: &str) -> anyhow::Result<String> {
        self.identifier_paths
            .get(identifier.strip_suffix("_deployed").unwrap_or(identifier))
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Contract with identifier `{}` not found in the project",
                    identifier
                )
            })
    }

    fn resolve_library(&self, path: &str) -> anyhow::Result<String> {
        for (file_path, contracts) in self.libraries.iter() {
            for (contract_name, address) in contracts.iter() {
                let key = format!("{}:{}", file_path, contract_name);
                if key.as_str() == path {
                    return Ok(address["0x".len()..].to_owned());
                }
            }
        }

        anyhow::bail!("Library `{}` not found in the project", path);
    }
}
