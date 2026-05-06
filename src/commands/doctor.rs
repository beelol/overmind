use anyhow::Result;

use crate::cli::ProjectOptions;

use super::effective_config;

pub fn run(options: ProjectOptions) -> Result<()> {
    let project_root = options.project_root()?;
    let effective = effective_config(&project_root, &options)?;
    let resolved = crate::source::resolve(&effective.source, true)?;

    println!("project: {}", project_root.display());
    println!("source: {}", effective.source.uri);
    println!("ref: {}", effective.source.ref_name);
    println!("pack: {}", effective.source.pack);
    println!("targets: {}", effective.sync.targets.join(","));
    println!("source_path: {}", resolved.path.display());
    println!("source_kind: {:?}", resolved.kind);
    println!("single_file: {}", resolved.single_file);
    Ok(())
}
