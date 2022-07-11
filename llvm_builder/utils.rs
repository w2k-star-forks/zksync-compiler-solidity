//!
//! The LLVM build script utilities.
//!

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
