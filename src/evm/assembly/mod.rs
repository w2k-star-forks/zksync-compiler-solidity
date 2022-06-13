//!
//! The `solc --asm-json` output representation.
//!

pub mod data;
pub mod instruction;

use std::collections::BTreeMap;
use std::collections::HashSet;

use serde::Deserialize;
use serde::Serialize;

use self::data::Data;
use self::instruction::name::Name as InstructionName;
use self::instruction::Instruction;

///
/// The JSON assembly representation.
///
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Assembly {
    /// The metadata string.
    #[serde(rename = ".auxdata")]
    pub auxdata: Option<String>,
    /// The deploy code instructions.
    #[serde(rename = ".code")]
    pub code: Option<Vec<Instruction>>,
    /// The runtime code representation.
    #[serde(rename = ".data")]
    pub data: Option<BTreeMap<String, Data>>,

    /// The deploy code factory dependency paths.
    #[serde(skip)]
    pub deploy_factory_dependencies: HashSet<String>,
    /// The runtime code factory dependency paths.
    #[serde(skip)]
    pub runtime_factory_dependencies: HashSet<String>,
}

impl Assembly {
    ///
    /// Gets the contract `keccak256` hash.
    ///
    pub fn keccak256(&self) -> String {
        let json = serde_json::to_vec(self).expect("Always valid");
        compiler_llvm_context::keccak256(json.as_slice())
    }

    ///
    /// Replaces the deploy code dependencies with full contract path and returns the list.
    ///
    pub fn deploy_dependencies_pass(
        &mut self,
        full_path: &str,
        hash_data_mapping: &BTreeMap<String, String>,
    ) -> anyhow::Result<BTreeMap<String, String>> {
        let mut index_path_mapping = BTreeMap::new();
        let index = "0".repeat(compiler_common::SIZE_FIELD * 2);
        index_path_mapping.insert(index, full_path.to_owned());

        let dependencies = match self.data.as_mut() {
            Some(dependencies) => dependencies,
            None => return Ok(index_path_mapping),
        };
        for (index, data) in dependencies.iter_mut() {
            if index == "0" {
                continue;
            }

            *data = match data {
                Data::Assembly(assembly) => {
                    let hash = assembly.keccak256();
                    let full_path =
                        hash_data_mapping
                            .get(hash.as_str())
                            .cloned()
                            .ok_or_else(|| {
                                anyhow::anyhow!("Contract path not found for hash `{}`", hash)
                            })?;
                    self.deploy_factory_dependencies
                        .insert(full_path.to_owned());

                    let mut index_extended =
                        "0".repeat(compiler_common::SIZE_FIELD * 2 - index.len());
                    index_extended.push_str(index.as_str());
                    index_path_mapping.insert(index_extended, full_path.clone());

                    Data::Path(full_path)
                }
                Data::Hash(hash) => {
                    index_path_mapping.insert(index.to_owned(), hash.to_owned());
                    continue;
                }
                _ => continue,
            };
        }

        Ok(index_path_mapping)
    }

    ///
    /// Replaces the runtime code dependencies with full contract path and returns the list.
    ///
    pub fn runtime_dependencies_pass(
        &mut self,
        full_path: &str,
        hash_data_mapping: &BTreeMap<String, String>,
    ) -> anyhow::Result<BTreeMap<String, String>> {
        let mut index_path_mapping = BTreeMap::new();
        let index = "0".repeat(compiler_common::SIZE_FIELD * 2);
        index_path_mapping.insert(index, full_path.to_owned());

        let dependencies = match self
            .data
            .as_mut()
            .and_then(|data| data.get_mut("0"))
            .and_then(|data| data.get_assembly_mut())
            .and_then(|assembly| assembly.data.as_mut())
        {
            Some(dependencies) => dependencies,
            None => return Ok(index_path_mapping),
        };
        for (index, data) in dependencies.iter_mut() {
            *data = match data {
                Data::Assembly(assembly) => {
                    let hash = assembly.keccak256();
                    let full_path =
                        hash_data_mapping
                            .get(hash.as_str())
                            .cloned()
                            .ok_or_else(|| {
                                anyhow::anyhow!("Contract path not found for hash `{}`", hash)
                            })?;
                    self.runtime_factory_dependencies
                        .insert(full_path.to_owned());

                    let mut index_extended =
                        "0".repeat(compiler_common::SIZE_FIELD * 2 - index.len());
                    index_extended.push_str(index.as_str());
                    index_path_mapping.insert(index_extended, full_path.clone());

                    Data::Path(full_path)
                }
                Data::Hash(hash) => {
                    index_path_mapping.insert(index.to_owned(), hash.to_owned());
                    continue;
                }
                _ => continue,
            };
        }

        Ok(index_path_mapping)
    }
}

impl std::fmt::Display for Assembly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(instructions) = self.code.as_ref() {
            for (index, instruction) in instructions.iter().enumerate() {
                match instruction.name {
                    InstructionName::Tag => writeln!(f, "{:03} {}", index, instruction)?,
                    _ => writeln!(f, "{:03}     {}", index, instruction)?,
                }
            }
        }

        Ok(())
    }
}
