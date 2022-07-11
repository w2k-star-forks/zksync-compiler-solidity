//!
//! The LLVM build script.
//!

pub(crate) mod linux_gnu;
pub(crate) mod macos;
pub(crate) mod utils;

use std::path::PathBuf;
use std::process::Command;

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
    let llvm_tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let llvm_path = PathBuf::from("./compiler-llvm");
    if !llvm_path.exists() {
        utils::command(
            Command::new("git").args(&[
                "clone",
                "--branch",
                llvm_tag.as_str(),
                "ssh://git@github.com/matter-labs/compiler-llvm",
                llvm_path.to_string_lossy().as_ref(),
            ]),
            "LLVM cloning",
        )?;
    }

    if cfg!(target_arch = "x86_64") {
        if cfg!(target_os = "linux") {
            if cfg!(target_env = "gnu") {
                linux_gnu::build()?;
            }
        } else if cfg!(target_os = "macos") {
            macos::build()?;
        }
    }

    Ok(())
}
