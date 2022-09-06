//!
//! The LLVM build script.
//!

pub(crate) mod aarch64_macos;
pub(crate) mod arguments;
pub(crate) mod utils;
pub(crate) mod x86_64_linux_gnu;
pub(crate) mod x86_64_linux_musl;
pub(crate) mod x86_64_macos;

use std::path::PathBuf;
use std::process::Command;

use self::arguments::Arguments;

///
/// The entry.
///
fn main() {
    main_wrapper().expect("LLVM builder error");
}

///
/// The entry result wrapper.
///
fn main_wrapper() -> anyhow::Result<()> {
    utils::check_presence("git")?;
    let arguments = Arguments::new();

    let llvm_tag = match arguments.tag {
        Some(tag) => tag,
        None => format!("v{}", env!("CARGO_PKG_VERSION")),
    };

    let llvm_path = PathBuf::from("./compiler-llvm");
    if !llvm_path.exists() {
        utils::command(
            Command::new("git").args(&[
                "clone",
                "--branch",
                llvm_tag.as_str(),
                "https://github.com/matter-labs/compiler-llvm",
                llvm_path.to_string_lossy().as_ref(),
            ]),
            "LLVM cloning",
        )?;
    } else {
        utils::command(
            Command::new("git")
                .current_dir(llvm_path.as_path())
                .args(&["fetch", "--all", "--tags"]),
            "LLVM checking out",
        )?;
        utils::command(
            Command::new("git")
                .current_dir(llvm_path)
                .args(&["checkout", llvm_tag.as_str()]),
            "LLVM checking out",
        )?;
    }

    if cfg!(target_arch = "x86_64") {
        if cfg!(target_os = "linux") {
            if cfg!(target_env = "gnu") {
                x86_64_linux_gnu::build()?;
            } else if cfg!(target_env = "musl") {
                x86_64_linux_musl::build()?;
            }
        } else if cfg!(target_os = "macos") {
            x86_64_macos::build()?;
        }
    } else if cfg!(target_arch = "aarch64") {
        if cfg!(target_os = "macos") {
            aarch64_macos::build()?;
        }
    } else {
        anyhow::bail!("Unsupported on your machine");
    }

    Ok(())
}
