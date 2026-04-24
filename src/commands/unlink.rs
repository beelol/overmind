use anyhow::Result;

use crate::{cli::ProjectOptions, render, source};

use super::effective_source;

pub fn run(options: ProjectOptions) -> Result<()> {
    let project_root = options.project_root()?;
    let effective = effective_source(&project_root, &options)?;
    let resolved = source::resolve(&effective, true)?;
    render::unlink_project(&project_root, &resolved, &effective, options.dry_run)
}
