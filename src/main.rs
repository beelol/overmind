use anyhow::Result;
use clap::Parser;
use ovmd::{cli::Cli, commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::run(cli)
}
