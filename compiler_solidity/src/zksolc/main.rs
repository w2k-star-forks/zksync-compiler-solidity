//!
//! Solidity to zkEVM compiler binary.
//!

pub mod arguments;

use self::arguments::Arguments;

///
/// The application entry point.
///
fn main() {
    std::process::exit(match main_inner() {
        Ok(()) => compiler_common::EXIT_CODE_SUCCESS,
        Err(error) => {
            eprintln!("{}", error);
            compiler_common::EXIT_CODE_FAILURE
        }
    })
}

///
/// The auxiliary `main` function to facilitate the `?` error conversion operator.
///
fn main_inner() -> anyhow::Result<()> {
    let mut arguments = Arguments::new();
    arguments.validate()?;

    let dump_flags = compiler_solidity::DumpFlag::from_booleans(
        arguments.dump_yul,
        arguments.dump_ethir,
        arguments.dump_evm,
        arguments.dump_llvm,
        arguments.dump_assembly,
    );

    for path in arguments.input_files.iter_mut() {
        *path = path.canonicalize()?;
    }

    let solc =
        compiler_solidity::SolcCompiler::new(arguments.solc.unwrap_or_else(|| {
            compiler_solidity::SolcCompiler::DEFAULT_EXECUTABLE_NAME.to_owned()
        }));
    let solc_version = solc.version()?;
    if solc_version > compiler_solidity::SolcCompiler::LAST_SUPPORTED_VERSION {
        anyhow::bail!(
            "solc versions >{} are not supported yet, found {}",
            compiler_solidity::SolcCompiler::LAST_SUPPORTED_VERSION,
            solc_version
        );
    }

    let pipeline = if solc_version.minor < 8 || arguments.force_evmla {
        compiler_solidity::SolcPipeline::EVM
    } else {
        compiler_solidity::SolcPipeline::Yul
    };

    compiler_llvm_context::initialize_target();
    if let Some(llvm_options) = arguments.llvm_options {
        let llvm_options = shell_words::split(llvm_options.as_str())
            .map_err(|error| anyhow::anyhow!("LLVM options parsing error: {}", error))?;
        let llvm_options = Vec::from_iter(llvm_options.iter().map(String::as_str));
        inkwell::support::parse_command_line_options(
            llvm_options.len() as i32,
            llvm_options.as_slice(),
            "",
        );
    }

    let build = if arguments.yul {
        let path = match arguments.input_files.len() {
            1 => arguments.input_files.remove(0),
            0 => anyhow::bail!("The input file is missing"),
            length => anyhow::bail!(
                "Only one input file is allowed in the Yul mode, but found {}",
                length
            ),
        };

        let project = compiler_solidity::Project::try_from_default_yul(&path, &solc_version)?;
        let optimizer_settings = if arguments.optimize {
            compiler_llvm_context::OptimizerSettings::cycles()
        } else {
            compiler_llvm_context::OptimizerSettings::none()
        };
        project.compile_all(optimizer_settings, dump_flags)
    } else {
        let output_selection =
            compiler_solidity::SolcStandardJsonInputSettings::get_output_selection(
                arguments
                    .input_files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                pipeline,
            );
        let solc_input = if arguments.standard_json {
            let mut input: compiler_solidity::SolcStandardJsonInput =
                serde_json::from_reader(std::io::BufReader::new(std::io::stdin()))?;
            input.settings.output_selection = output_selection;
            input
        } else {
            compiler_solidity::SolcStandardJsonInput::try_from_paths(
                compiler_solidity::SolcStandardJsonInputLanguage::Solidity,
                arguments.input_files.as_slice(),
                arguments.libraries,
                output_selection,
                true,
            )?
        };

        let libraries = solc_input.settings.libraries.clone().unwrap_or_default();
        let optimize = if arguments.standard_json {
            solc_input.settings.optimizer.enabled
        } else {
            arguments.optimize
        };
        let mut solc_output = solc.standard_json(
            solc_input,
            arguments.base_path,
            arguments.include_paths,
            arguments.allow_paths,
        )?;

        if let Some(errors) = solc_output.errors.as_deref() {
            let mut cannot_compile = false;
            for error in errors.iter() {
                if error.severity.as_str() == "error" {
                    cannot_compile = true;
                    if arguments.standard_json {
                        serde_json::to_writer(std::io::stdout(), &solc_output)?;
                        return Ok(());
                    }
                }

                if !arguments.standard_json && arguments.combined_json.is_none() {
                    eprintln!("{}", error);
                }
            }

            if cannot_compile {
                anyhow::bail!("Error(s) found. Compilation aborted");
            }
        }

        let project =
            solc_output.try_to_project(libraries, pipeline, solc_version, dump_flags.as_slice())?;
        let optimizer_settings = if optimize {
            compiler_llvm_context::OptimizerSettings::cycles()
        } else {
            compiler_llvm_context::OptimizerSettings::none()
        };
        let build = project.compile_all(optimizer_settings, dump_flags)?;
        if arguments.standard_json {
            build.write_to_standard_json(&mut solc_output)?;
            serde_json::to_writer(std::io::stdout(), &solc_output)?;
            return Ok(());
        }
        Ok(build)
    }?;

    let combined_json = if let Some(combined_json) = arguments.combined_json {
        Some(solc.combined_json(arguments.input_files.as_slice(), combined_json.as_str())?)
    } else {
        None
    };

    if let Some(output_directory) = arguments.output_directory {
        std::fs::create_dir_all(&output_directory)?;

        if let Some(mut combined_json) = combined_json {
            build.write_to_combined_json(&mut combined_json)?;
            combined_json.write_to_directory(&output_directory, arguments.overwrite)?;
        } else {
            build.write_to_directory(
                &output_directory,
                arguments.output_assembly,
                arguments.output_binary,
                arguments.output_abi,
                arguments.overwrite,
            )?;
        }

        eprintln!(
            "Compiler run successful. Artifact(s) can be found in directory {:?}.",
            output_directory
        );
    } else if let Some(mut combined_json) = combined_json {
        build.write_to_combined_json(&mut combined_json)?;
        println!(
            "{}",
            serde_json::to_string(&combined_json).expect("Always valid")
        );
    } else if arguments.output_assembly
        || arguments.output_binary
        || arguments.output_hashes
        || arguments.output_abi
    {
        for (path, contract) in build.contracts.into_iter() {
            if arguments.output_assembly {
                println!(
                    "Contract `{}` assembly:\n\n{}",
                    path, contract.build.assembly_text
                );
            }
            if arguments.output_binary {
                println!(
                    "Contract `{}` bytecode: 0x{}",
                    path,
                    hex::encode(contract.build.bytecode)
                );
            }
        }

        if arguments.output_abi || arguments.output_hashes {
            let extra_output = solc.extra_output(
                arguments.input_files.as_slice(),
                arguments.output_abi,
                arguments.output_hashes,
            )?;
            print!("{}", extra_output);
        }
    } else {
        eprintln!("Compiler run successful. No output requested. Use --asm and --bin flags.");
    }

    Ok(())
}
