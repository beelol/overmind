use anyhow::Result;

use crate::{
    cli::ProjectOptions,
    config,
    render::{self, RenderOptions},
    source,
};

use super::effective_config;

pub fn run(options: ProjectOptions) -> Result<()> {
    let project_root = options.project_root()?;
    let effective = effective_config(&project_root, &options)?;
    config::write_project_config(project_root.clone(), &effective.source, options.dry_run)?;
    let resolved = source::resolve(&effective.source, false)?;
    render::render_project(
        &project_root,
        &resolved,
        &effective.source,
        &RenderOptions {
            dry_run: options.dry_run,
            targets: effective.sync.targets,
            explicit_targets: effective.sync.explicit_targets,
            ..Default::default()
        },
    )
}
