use anyhow::{anyhow, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::channel,
    thread,
    time::Duration,
};

use crate::{
    cli::{
        Cli, Command as CliCommand, ModuleCommand, PackCommand, ProjectOptions, SourceCommand,
        SyncOptions,
    },
    config::{self, EffectiveSource, FlagOverrides},
    render::{self, RenderOptions},
    source::{self, SourceKind},
};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        CliCommand::Init(options) => init(options),
        CliCommand::Sync(options) => sync(options),
        CliCommand::Doctor(options) => doctor(options),
        CliCommand::Source { command } => source_command(command),
        CliCommand::Pack { command } => pack_command(command),
        CliCommand::Module { command } => module_command(command),
    }
}

fn init(options: ProjectOptions) -> Result<()> {
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

fn sync(options: SyncOptions) -> Result<()> {
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

fn source_command(command: SourceCommand) -> Result<()> {
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

fn pack_command(command: PackCommand) -> Result<()> {
    match command {
        PackCommand::Build(options) => {
            let project_root = options.project_root()?;
            let effective = effective_source(&project_root, &options)?;
            let resolved = source::resolve(&effective, true)?;
            render::build_pack_artifact(&resolved, &effective, options.dry_run)
        }
    }
}

fn module_command(command: ModuleCommand) -> Result<()> {
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

fn doctor(options: ProjectOptions) -> Result<()> {
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

fn publish(options: crate::cli::PublishOptions) -> Result<()> {
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

fn effective_source(project_root: &Path, options: &ProjectOptions) -> Result<EffectiveSource> {
    config::resolve_effective_source(project_root.to_path_buf(), FlagOverrides::from(options))
}

fn open_editor(path: PathBuf) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let status = Command::new(editor).arg(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("editor exited with status {}", status))
    }
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<()> {
    let status = Command::new("git").current_dir(cwd).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("git command failed with status {}", status))
    }
}

fn git_has_changes(cwd: &Path) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("git status failed with status {}", output.status));
    }
    Ok(!output.stdout.is_empty())
}
