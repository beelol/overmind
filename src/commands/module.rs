use anyhow::Result;

use crate::{cli::ModuleCommand, render, source};

use super::effective_source;

pub fn run(command: ModuleCommand) -> Result<()> {
    match command {
        ModuleCommand::List(options) => {
            let project_root = options.project_root()?;
            let effective = effective_source(&project_root, &options)?;
            let resolved = source::resolve(&effective, true)?;
            for module in render::list_modules(&resolved, &effective)? {
                println!("{}\t{}\t{}", module.id, module.enabled, module.path);
            }
            Ok(())
        }
    }
}
