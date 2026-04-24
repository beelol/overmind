use anyhow::{anyhow, Result};

use crate::{
    cli::{PublishOptions, SourceCommand},
    render, source,
};

use super::{effective_source, git, git_has_changes, open_editor};

pub fn run(command: SourceCommand) -> Result<()> {
    match command {
        SourceCommand::Update(options) => {
            let project_root = options.project_root()?;
            let effective = effective_source(&project_root, &options)?;
            let resolved = source::resolve(&effective, false)?;
            println!("{}", resolved.path.display());
            Ok(())
        }
        SourceCommand::Path(options) => {
            let project_root = options.project_root()?;
            let effective = effective_source(&project_root, &options)?;
            let resolved = source::resolve(&effective, true)?;
            println!("{}", resolved.path.display());
            Ok(())
        }
        SourceCommand::Edit(options) => {
            let project_root = options.project_root()?;
            let effective = effective_source(&project_root, &options)?;
            let resolved = source::resolve(&effective, false)?;
            open_editor(resolved.path)
        }
        SourceCommand::Publish(options) => publish(options),
    }
}

fn publish(options: PublishOptions) -> Result<()> {
    let project_root = options.project.project_root()?;
    let effective = effective_source(&project_root, &options.project)?;
    let resolved = source::resolve(&effective, false)?;
    if !resolved.git_backed {
        return Err(anyhow!("source is not git-backed"));
    }

    render::build_pack_artifact(&resolved, &effective, options.project.dry_run)?;
    if options.project.dry_run {
        println!("Would commit and push source changes");
        return Ok(());
    }

    git(&resolved.path, ["add", "."])?;
    if !git_has_changes(&resolved.path)? {
        println!("No source changes to publish.");
        return Ok(());
    }
    git(&resolved.path, ["commit", "-m", &options.message])?;
    git(&resolved.path, ["push", "origin", &effective.ref_name])?;
    Ok(())
}
