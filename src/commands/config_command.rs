use anyhow::Result;
use std::{fs, path::PathBuf};

use crate::{
    cli::{ConfigCommand, ConfigInitOptions, ConfigOptions},
    config::{self, EffectiveSource},
};

use super::open_editor;

pub fn run(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Init(options) => init(options),
        ConfigCommand::Path(options) => {
            let path = config_path(&options)?;
            println!("{}", path.display());
            Ok(())
        }
        ConfigCommand::Edit(options) => {
            let path = config_path(&options)?;
            ensure_file_exists(&path)?;
            open_editor(path)
        }
    }
}

fn init(options: ConfigInitOptions) -> Result<()> {
    let path = config_path(&options.config)?;
    if path.exists() && !options.force {
        println!("Config already exists: {}", path.display());
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, default_config_body())?;
    println!("Wrote {}", path.display());
    Ok(())
}

fn config_path(options: &ConfigOptions) -> Result<PathBuf> {
    if options.global {
        return config::global_config_path();
    }

    Ok(std::env::current_dir()?.join(config::PROJECT_CONFIG))
}

fn ensure_file_exists(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(path, "")?;
    }
    Ok(())
}

fn default_config_body() -> String {
    let source = EffectiveSource::default();
    format!(
        r#"[source]
uri = "{}"
ref = "{}"
pack = "{}"
"#,
        source.uri, source.ref_name, source.pack
    )
}
