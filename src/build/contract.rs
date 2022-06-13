//!
//! The Solidity contract build.
//!

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::solc::combined_json::contract::Contract as CombinedJsonContract;

///
/// The Solidity contract build.
///
#[derive(Debug)]
pub struct Contract {
    /// The contract path.
    pub path: String,
    /// The auxiliary identifier. Used to identify Yul objects.
    pub identifier: String,
    /// The deploy code build.
    pub deploy_build: compiler_llvm_context::Build,
    /// The runtime code build.
    pub runtime_build: compiler_llvm_context::Build,
    /// The ABI specification JSON.
    pub abi: Option<serde_json::Value>,
}

impl Contract {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        path: String,
        identifier: String,
        deploy_build: compiler_llvm_context::Build,
        runtime_build: compiler_llvm_context::Build,
        abi: Option<serde_json::Value>,
    ) -> Self {
        Self {
            path,
            identifier,
            deploy_build,
            runtime_build,
            abi,
        }
    }

    ///
    /// Writes the contract text assembly and bytecode to files.
    ///
    pub fn write_to_directory(
        self,
        path: &Path,
        output_assembly: bool,
        output_binary: bool,
        output_abi: bool,
        overwrite: bool,
    ) -> anyhow::Result<()> {
        let file_name = Self::short_path(self.path.as_str());

        if output_assembly {
            {
                let file_name = format!(
                    "{}.{}.{}",
                    file_name,
                    compiler_llvm_context::CodeType::Deploy,
                    compiler_common::EXTENSION_ZKEVM_ASSEMBLY
                );
                let mut file_path = path.to_owned();
                file_path.push(file_name);
                if file_path.exists() && !overwrite {
                    eprintln!(
                        "Refusing to overwrite an existing file {:?} (use --overwrite to force).",
                        file_path
                    );
                } else {
                    File::create(&file_path)
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} creating error: {}", file_path, error)
                        })?
                        .write_all(self.deploy_build.assembly_text.as_bytes())
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} writing error: {}", file_path, error)
                        })?;
                }
            }

            {
                let file_name = format!(
                    "{}.{}.{}",
                    file_name,
                    compiler_llvm_context::CodeType::Runtime,
                    compiler_common::EXTENSION_ZKEVM_ASSEMBLY
                );
                let mut file_path = path.to_owned();
                file_path.push(file_name);
                if file_path.exists() && !overwrite {
                    eprintln!(
                        "Refusing to overwrite an existing file {:?} (use --overwrite to force).",
                        file_path
                    );
                } else {
                    File::create(&file_path)
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} creating error: {}", file_path, error)
                        })?
                        .write_all(self.runtime_build.assembly_text.as_bytes())
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} writing error: {}", file_path, error)
                        })?;
                }
            }
        }

        if output_binary {
            {
                let file_name = format!(
                    "{}.{}.{}",
                    file_name,
                    compiler_llvm_context::CodeType::Deploy,
                    compiler_common::EXTENSION_ZKEVM_BINARY
                );
                let mut file_path = path.to_owned();
                file_path.push(file_name);
                if file_path.exists() && !overwrite {
                    eprintln!(
                        "Refusing to overwrite an existing file {:?} (use --overwrite to force).",
                        file_path
                    );
                } else {
                    File::create(&file_path)
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} creating error: {}", file_path, error)
                        })?
                        .write_all(self.deploy_build.bytecode.as_slice())
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} writing error: {}", file_path, error)
                        })?;
                }
            }

            {
                let file_name = format!(
                    "{}.{}.{}",
                    file_name,
                    compiler_llvm_context::CodeType::Runtime,
                    compiler_common::EXTENSION_ZKEVM_BINARY
                );
                let mut file_path = path.to_owned();
                file_path.push(file_name);
                if file_path.exists() && !overwrite {
                    eprintln!(
                        "Refusing to overwrite an existing file {:?} (use --overwrite to force).",
                        file_path
                    );
                } else {
                    File::create(&file_path)
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} creating error: {}", file_path, error)
                        })?
                        .write_all(self.runtime_build.bytecode.as_slice())
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} writing error: {}", file_path, error)
                        })?;
                }
            }
        }

        if let Some(abi) = self.abi {
            if output_abi {
                let file_name = format!("{}.{}", file_name, compiler_common::EXTENSION_ABI);
                let mut file_path = path.to_owned();
                file_path.push(file_name);

                if file_path.exists() && !overwrite {
                    eprintln!(
                        "Refusing to overwrite an existing file {:?} (use --overwrite to force).",
                        file_path
                    );
                } else {
                    File::create(&file_path)
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} creating error: {}", file_path, error)
                        })?
                        .write_all(abi.to_string().as_bytes())
                        .map_err(|error| {
                            anyhow::anyhow!("File {:?} writing error: {}", file_path, error)
                        })?;
                }
            }
        }

        Ok(())
    }

    ///
    /// Writes the contract text assembly and bytecode to the combined JSON.
    ///
    pub fn write_to_combined_json(
        self,
        combined_json_contract: &mut CombinedJsonContract,
    ) -> anyhow::Result<()> {
        combined_json_contract.bin = Some(hex::encode(self.deploy_build.bytecode));
        combined_json_contract.bin_runtime = Some(hex::encode(self.runtime_build.bytecode));

        combined_json_contract.deploy_factory_deps = Some(self.deploy_build.factory_dependencies);
        combined_json_contract.runtime_factory_deps = Some(self.runtime_build.factory_dependencies);

        Ok(())
    }

    ///
    /// Converts the full path to a short one.
    ///
    pub fn short_path(path: &str) -> &str {
        path.rfind('/')
            .map(|last_slash| &path[last_slash + 1..])
            .unwrap_or_else(|| path)
    }
}
