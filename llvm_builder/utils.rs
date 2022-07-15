//!
//! The LLVM build script utilities.
//!

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

///
/// The subprocess runner.
///
/// Checks the status and prints `stderr`.
///
pub fn command(command: &mut Command, description: &str) -> anyhow::Result<()> {
    let status = command
        .status()
        .map_err(|error| anyhow::anyhow!("{} process: {}", description, error))?;
    if !status.success() {
        anyhow::bail!("{} failed", description);
    }
    Ok(())
}

///
/// Create an absolute path, appending it to the current working directory.
///
pub fn absolute_path<P: AsRef<Path>>(path: P) -> anyhow::Result<PathBuf> {
    let mut full_path = std::env::current_dir()?;
    full_path.push(path);
    Ok(full_path)
}

///
/// Checks if the tool exists in the system.
///
pub fn check_presence(name: &str) -> anyhow::Result<()> {
    let status = Command::new("which")
        .arg(name)
        .status()
        .map_err(|error| anyhow::anyhow!("`which {}` process: {}", name, error))?;
    if !status.success() {
        anyhow::bail!("Tool `{}` is missing. Please install", name);
    }
    Ok(())
}
