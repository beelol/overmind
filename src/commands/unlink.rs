use anyhow::Result;

use crate::{cli::ProjectOptions, render, source};

use super::effective_config;

pub fn run(options: ProjectOptions) -> Result<()> {
    let project_root = options.project_root()?;
    let effective = effective_config(&project_root, &options)?;
    let resolved = source::resolve(&effective.source, true)?;
    render::unlink_project(&project_root, &resolved, &effective.source, options.dry_run)
}
