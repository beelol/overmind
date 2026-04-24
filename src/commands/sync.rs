use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::channel,
    thread,
    time::Duration,
};

use crate::{
    cli::SyncOptions,
    config::EffectiveSource,
    render::{self, RenderOptions},
    source::{self, SourceKind},
};

use super::effective_source;

pub fn run(options: SyncOptions) -> Result<()> {
    let project_root = options.project.project_root()?;
    let effective = effective_source(&project_root, &options.project)?;
    sync_once(&project_root, &effective, &options)?;

    if options.watch {
        watch(project_root, effective, options)?;
    }

    Ok(())
}

fn sync_once(
    project_root: &Path,
    effective: &EffectiveSource,
    options: &SyncOptions,
) -> Result<()> {
    let resolved = source::resolve(effective, options.offline)?;
    render::render_project(
        project_root,
        &resolved,
        effective,
        &RenderOptions {
            dry_run: options.project.dry_run,
            only: options.only.clone(),
            exclude: options.exclude.clone(),
        },
    )
}

fn watch(project_root: PathBuf, effective: EffectiveSource, options: SyncOptions) -> Result<()> {
    let resolved = source::resolve(&effective, options.offline)?;
    println!("Watching source: {}", resolved.path.display());

    match resolved.kind {
        SourceKind::LocalDir | SourceKind::LocalFile => {
            watch_filesystem(project_root, effective, options, resolved.path)
        }
        SourceKind::Git | SourceKind::Http => loop {
            thread::sleep(Duration::from_secs(options.poll_interval));
            sync_once(&project_root, &effective, &options)?;
        },
    }
}

fn watch_filesystem(
    project_root: PathBuf,
    effective: EffectiveSource,
    options: SyncOptions,
    path: PathBuf,
) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    let mode = if path.is_dir() {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    watcher.watch(&path, mode)?;

    for event in rx {
        event?;
        sync_once(&project_root, &effective, &options)?;
    }

    Ok(())
}
