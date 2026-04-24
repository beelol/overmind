use anyhow::Result;

use crate::{cli::ProjectOptions, source};

use super::effective_source;

pub fn run(options: ProjectOptions) -> Result<()> {
    let project_root = options.project_root()?;
    let effective = effective_source(&project_root, &options)?;
    let resolved = source::resolve(&effective, true)?;

    println!("project: {}", project_root.display());
    println!("source: {}", effective.uri);
    println!("ref: {}", effective.ref_name);
    println!("pack: {}", effective.pack);
    println!("source_path: {}", resolved.path.display());
    println!("source_kind: {:?}", resolved.kind);
    println!("single_file: {}", resolved.single_file);
    Ok(())
}
