use anyhow::{anyhow, Context, Result};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::cli::UpdateOptions;

const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/beelol/overmind/master/scripts/install.sh";

pub fn run(options: UpdateOptions) -> Result<()> {
    let install_dir = install_dir(options.install_dir)?;
    println!("Updating ovmd in {}", install_dir.display());

    let command = format!(
        "curl -fsSL {} | INSTALL_DIR={} bash",
        shell_quote(INSTALL_SCRIPT_URL),
        shell_quote(&install_dir.to_string_lossy())
    );

    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .status()
        .context("failed to run update script")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("update script exited with status {}", status))
    }
}

fn install_dir(override_dir: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = override_dir {
        return Ok(path);
    }

    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    current_exe
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("current executable has no parent directory"))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
