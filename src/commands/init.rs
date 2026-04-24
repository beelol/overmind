use anyhow::Result;

use crate::{
    cli::ProjectOptions,
    config,
    render::{self, RenderOptions},
    source,
};

use super::effective_source;

pub fn run(options: ProjectOptions) -> Result<()> {
    let project_root = options.project_root()?;
    let effective = effective_source(&project_root, &options)?;
    config::write_project_config(project_root.clone(), &effective, options.dry_run)?;
    let resolved = source::resolve(&effective, false)?;
    render::render_project(
        &project_root,
        &resolved,
        &effective,
        &RenderOptions {
            dry_run: options.dry_run,
            ..Default::default()
        },
    )
}
