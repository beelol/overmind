use anyhow::Result;

use crate::{cli::PackCommand, render, source};

use super::effective_source;

pub fn run(command: PackCommand) -> Result<()> {
    match command {
        PackCommand::Build(options) => {
            let project_root = options.project_root()?;
            let effective = effective_source(&project_root, &options)?;
            let resolved = source::resolve(&effective, true)?;
            render::build_pack_artifact(&resolved, &effective, options.dry_run)
        }
    }
}
