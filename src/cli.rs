use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "ovmd",
    version,
    about = "Sync modular AI agent rules into projects."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create project config and render rule targets.
    Init(ProjectOptions),
    /// Update the configured source and render rule targets.
    Sync(SyncOptions),
    /// Manage rule sources.
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    /// Manage rule packs.
    Pack {
        #[command(subcommand)]
        command: PackCommand,
    },
    /// List available modules in the effective pack.
    Module {
        #[command(subcommand)]
        command: ModuleCommand,
    },
    /// Inspect effective config and source state.
    Doctor(ProjectOptions),
    /// Manage ovmd config files.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Args, Clone)]
pub struct ProjectOptions {
    /// Override source URI or local path.
    #[arg(long)]
    pub source: Option<String>,

    /// Override source ref or branch.
    #[arg(long = "ref")]
    pub ref_name: Option<String>,

    /// Override pack id.
    #[arg(long)]
    pub pack: Option<String>,

    /// Do not write files.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args, Clone)]
pub struct SyncOptions {
    #[command(flatten)]
    pub project: ProjectOptions,

    /// Re-render when source changes.
    #[arg(short, long)]
    pub watch: bool,

    /// Poll interval in seconds for remote sources.
    #[arg(long, default_value_t = 60)]
    pub poll_interval: u64,

    /// Render from current cache without updating remote sources.
    #[arg(long)]
    pub offline: bool,

    /// Render only these module ids.
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,

    /// Exclude these module ids.
    #[arg(long, value_delimiter = ',')]
    pub exclude: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    /// Update the effective source only.
    Update(ProjectOptions),
    /// Print the path ovmd reads from.
    Path(ProjectOptions),
    /// Open the effective editable source path.
    Edit(ProjectOptions),
    /// Commit and push changes from a git-backed source.
    Publish(PublishOptions),
}

#[derive(Debug, Args)]
pub struct PublishOptions {
    #[command(flatten)]
    pub project: ProjectOptions,

    /// Commit message.
    #[arg(short, long)]
    pub message: String,
}

#[derive(Debug, Subcommand)]
pub enum PackCommand {
    /// Build generated AGENTS.md in the source repository.
    Build(ProjectOptions),
}

#[derive(Debug, Subcommand)]
pub enum ModuleCommand {
    /// List modules in the effective pack.
    List(ProjectOptions),
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Create a default config file.
    Init(ConfigInitOptions),
    /// Print config file path.
    Path(ConfigOptions),
    /// Open config file in $EDITOR.
    Edit(ConfigOptions),
}

#[derive(Debug, Args, Clone, Default)]
pub struct ConfigOptions {
    /// Use global config (~/.config/overmind/config.toml).
    #[arg(long)]
    pub global: bool,
}

#[derive(Debug, Args, Clone, Default)]
pub struct ConfigInitOptions {
    #[command(flatten)]
    pub config: ConfigOptions,

    /// Overwrite an existing config file.
    #[arg(long)]
    pub force: bool,
}

impl ProjectOptions {
    pub fn project_root(&self) -> anyhow::Result<PathBuf> {
        Ok(std::env::current_dir()?)
    }
}
