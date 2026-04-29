use anyhow::{anyhow, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    cli::{Cli, Command as CliCommand, ProjectOptions},
    config::{self, EffectiveSource, FlagOverrides},
};

pub mod config_command;
pub mod desync;
pub mod doctor;
pub mod init;
pub mod module;
pub mod pack;
pub mod source;
pub mod sync;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        CliCommand::Init(options) => init::run(options),
        CliCommand::Desync(options) => desync::run(options),
        CliCommand::Sync(options) => sync::run(options),
        CliCommand::Doctor(options) => doctor::run(options),
        CliCommand::Source { command } => source::run(command),
        CliCommand::Pack { command } => pack::run(command),
        CliCommand::Module { command } => module::run(command),
        CliCommand::Config { command } => config_command::run(command),
    }
}

pub(crate) fn effective_source(
    project_root: &Path,
    options: &ProjectOptions,
) -> Result<EffectiveSource> {
    config::resolve_effective_source(project_root.to_path_buf(), FlagOverrides::from(options))
}

pub(crate) fn open_editor(path: PathBuf) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let status = Command::new(editor).arg(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("editor exited with status {}", status))
    }
}

pub(crate) fn git<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<()> {
    let status = Command::new("git").current_dir(cwd).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("git command failed with status {}", status))
    }
}

pub(crate) fn git_has_changes(cwd: &Path) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("git status failed with status {}", output.status));
    }
    Ok(!output.stdout.is_empty())
}
