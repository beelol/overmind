use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::cli::{SkillsCommand, SkillsSyncOptions};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Installation {
    pub skill: String,
    pub agent: String,
    pub canonical_path: PathBuf,
    pub installed_path: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ledger {
    #[serde(default)]
    pub installations: Vec<Installation>,
}

pub fn run(command: SkillsCommand) -> Result<()> {
    match command {
        SkillsCommand::Sync(options) => sync_global(options),
    }
}

fn sync_global(options: SkillsSyncOptions) -> Result<()> {
    let ledger_path = ledger_path()?;
    let source = match options.source {
        Some(source) => source,
        None => canonical_source_from_ledger(&ledger_path)?
            .unwrap_or(std::env::current_dir()?.join("skills")),
    };
    let source = source
        .canonicalize()
        .with_context(|| format!("failed to resolve skills source {}", source.display()))?;
    let home = dirs::home_dir().context("could not resolve home directory")?;
    reconcile(
        &source,
        &[
            ("codex", home.join(".codex/skills")),
            ("claude", home.join(".claude/skills")),
        ],
        &ledger_path,
        options.dry_run,
    )
}

fn canonical_source_from_ledger(path: &Path) -> Result<Option<PathBuf>> {
    let ledger = load_ledger(path)?;
    let Some(first) = ledger.installations.first() else {
        return Ok(None);
    };
    let parent = first
        .canonical_path
        .parent()
        .context("canonical skill path has no parent directory")?
        .to_path_buf();
    if ledger.installations.iter().any(|item| {
        item.canonical_path
            .parent()
            .map(|candidate| candidate != parent)
            .unwrap_or(true)
    }) {
        bail!("skill ledger contains multiple canonical source directories; pass --source")
    }
    Ok(Some(parent))
}

pub fn ledger_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().context("could not resolve local data directory")?;
    Ok(base.join("overmind").join("skills-ledger.toml"))
}

pub fn print_status() -> Result<()> {
    let path = ledger_path()?;
    let ledger = load_ledger(&path)?;
    println!("skills_ledger: {}", path.display());
    if ledger.installations.is_empty() {
        println!("skills: not synced");
        return Ok(());
    }
    for item in ledger.installations {
        println!(
            "skill: {} agent={} status={} path={}",
            item.skill,
            item.agent,
            installation_status(&item),
            item.installed_path.display()
        );
    }
    Ok(())
}

fn installation_status(item: &Installation) -> &'static str {
    if !item.canonical_path.is_dir() {
        return "stale";
    }
    let Ok(metadata) = fs::symlink_metadata(&item.installed_path) else {
        return "missing";
    };
    if !metadata.file_type().is_symlink() {
        return "conflicting";
    }
    if link_points_to(&item.installed_path, &item.canonical_path) {
        "installed"
    } else {
        "broken"
    }
}

pub fn load_ledger(path: &Path) -> Result<Ledger> {
    if !path.exists() {
        return Ok(Ledger::default());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read skill ledger {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse skill ledger {}", path.display()))
}

fn discover(source: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut skills = BTreeMap::new();
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read skills source {}", source.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() || !path.join("SKILL.md").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        skills.insert(name, path.canonicalize()?);
    }
    Ok(skills)
}

pub fn reconcile(
    source: &Path,
    targets: &[(&str, PathBuf)],
    ledger_path: &Path,
    dry_run: bool,
) -> Result<()> {
    let skills = discover(source)?;
    let old = load_ledger(ledger_path)?;
    let managed: BTreeSet<PathBuf> = old
        .installations
        .iter()
        .map(|item| item.installed_path.clone())
        .collect();
    let mut desired = Vec::new();

    for (skill, canonical_path) in &skills {
        for (agent, root) in targets {
            desired.push(Installation {
                skill: skill.clone(),
                agent: (*agent).to_string(),
                canonical_path: canonical_path.clone(),
                installed_path: root.join(skill),
            });
        }
    }

    let desired_paths: BTreeSet<_> = desired
        .iter()
        .map(|item| item.installed_path.clone())
        .collect();

    // Validate every destination before making any changes.
    for item in &desired {
        validate_destination(item, managed.contains(&item.installed_path))?;
    }
    for stale in old
        .installations
        .iter()
        .filter(|item| !desired_paths.contains(&item.installed_path))
    {
        validate_stale(stale)?;
    }

    for stale in old
        .installations
        .iter()
        .filter(|item| !desired_paths.contains(&item.installed_path))
    {
        println!("Remove {}", stale.installed_path.display());
        if !dry_run && fs::symlink_metadata(&stale.installed_path).is_ok() {
            fs::remove_file(&stale.installed_path).with_context(|| {
                format!(
                    "failed to remove stale skill {}",
                    stale.installed_path.display()
                )
            })?;
        }
    }

    for item in &desired {
        if link_points_to(&item.installed_path, &item.canonical_path) {
            println!("Installed {}: {}", item.agent, item.skill);
            continue;
        }
        println!(
            "Link {} -> {}",
            item.installed_path.display(),
            item.canonical_path.display()
        );
        if !dry_run {
            fs::create_dir_all(item.installed_path.parent().unwrap())?;
            if fs::symlink_metadata(&item.installed_path).is_ok() {
                fs::remove_file(&item.installed_path)?;
            }
            create_symlink(&item.canonical_path, &item.installed_path)?;
        }
    }

    if !dry_run {
        write_ledger_atomic(
            ledger_path,
            &Ledger {
                installations: desired,
            },
        )?;
    }
    Ok(())
}

fn validate_destination(item: &Installation, was_managed: bool) -> Result<()> {
    let Ok(metadata) = fs::symlink_metadata(&item.installed_path) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink()
        && (link_points_to(&item.installed_path, &item.canonical_path) || was_managed)
    {
        return Ok(());
    }
    bail!(
        "refusing to replace unmanaged skill destination {}",
        item.installed_path.display()
    )
}

fn validate_stale(item: &Installation) -> Result<()> {
    let Ok(metadata) = fs::symlink_metadata(&item.installed_path) else {
        return Ok(());
    };
    if !metadata.file_type().is_symlink() {
        bail!(
            "refusing to remove stale path that is no longer a symlink: {}",
            item.installed_path.display()
        );
    }
    if fs::read_link(&item.installed_path)? != item.canonical_path {
        bail!(
            "refusing to remove stale skill link with an unmanaged target: {}",
            item.installed_path.display()
        );
    }
    Ok(())
}

fn link_points_to(path: &Path, canonical: &Path) -> bool {
    fs::read_link(path)
        .map(|target| target == canonical)
        .unwrap_or(false)
}

fn write_ledger_atomic(path: &Path, ledger: &Ledger) -> Result<()> {
    let parent = path
        .parent()
        .context("skill ledger has no parent directory")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("toml.tmp");
    let body = toml::to_string_pretty(ledger)?;
    fs::write(&temporary, body)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(unix)]
fn create_symlink(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, destination)?;
    Ok(())
}

#[cfg(windows)]
fn create_symlink(source: &Path, destination: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn skill(root: &Path, name: &str) {
        let path = root.join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), format!("---\nname: {name}\n---\n")).unwrap();
    }

    #[test]
    fn installs_idempotently_and_removes_deleted_skills() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("source");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        let ledger = temp.path().join("ledger.toml");
        fs::create_dir_all(&source).unwrap();
        skill(&source, "one");

        let targets = [("codex", codex.clone()), ("claude", claude.clone())];
        reconcile(&source, &targets, &ledger, false).unwrap();
        reconcile(&source, &targets, &ledger, false).unwrap();
        let canonical_source = source.canonicalize().unwrap();
        assert!(link_points_to(
            &codex.join("one"),
            &canonical_source.join("one")
        ));
        assert!(link_points_to(
            &claude.join("one"),
            &canonical_source.join("one")
        ));

        skill(&source, "two");
        fs::remove_dir_all(source.join("one")).unwrap();
        reconcile(&source, &targets, &ledger, false).unwrap();
        assert!(!codex.join("one").exists());
        assert!(link_points_to(
            &codex.join("two"),
            &canonical_source.join("two")
        ));

        fs::remove_dir_all(source.join("two")).unwrap();
        reconcile(&source, &targets, &ledger, false).unwrap();
        assert!(!codex.join("two").exists());
        assert!(load_ledger(&ledger).unwrap().installations.is_empty());
    }

    #[test]
    fn repairs_managed_link_and_refuses_unmanaged_collision() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let ledger = temp.path().join("ledger.toml");
        fs::create_dir_all(&source).unwrap();
        skill(&source, "one");
        reconcile(&source, &[("codex", target.clone())], &ledger, false).unwrap();

        fs::remove_file(target.join("one")).unwrap();
        create_symlink(temp.path(), &target.join("one")).unwrap();
        reconcile(&source, &[("codex", target.clone())], &ledger, false).unwrap();

        skill(&source, "two");
        fs::create_dir_all(target.join("two")).unwrap();
        let error = reconcile(&source, &[("codex", target)], &ledger, false).unwrap_err();
        assert!(error.to_string().contains("unmanaged"));
    }

    #[test]
    fn dry_run_does_not_write_links_or_ledger() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let ledger = temp.path().join("ledger.toml");
        fs::create_dir_all(&source).unwrap();
        skill(&source, "one");

        reconcile(&source, &[("codex", target.clone())], &ledger, true).unwrap();
        assert!(!target.exists());
        assert!(!ledger.exists());
    }

    #[test]
    fn collision_validation_prevents_partial_installation() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let ledger = temp.path().join("ledger.toml");
        fs::create_dir_all(&source).unwrap();
        skill(&source, "one");
        skill(&source, "two");
        fs::create_dir_all(target.join("two")).unwrap();

        assert!(reconcile(&source, &[("codex", target.clone())], &ledger, false).is_err());
        assert!(!target.join("one").exists());
        assert!(!ledger.exists());
    }

    #[test]
    fn classifies_installation_states() {
        let temp = tempdir().unwrap();
        let canonical = temp.path().join("canonical/one");
        let installed = temp.path().join("installed/one");
        fs::create_dir_all(&canonical).unwrap();
        fs::create_dir_all(installed.parent().unwrap()).unwrap();
        let item = Installation {
            skill: "one".into(),
            agent: "codex".into(),
            canonical_path: canonical.clone(),
            installed_path: installed.clone(),
        };

        assert_eq!(installation_status(&item), "missing");
        create_symlink(temp.path(), &installed).unwrap();
        assert_eq!(installation_status(&item), "broken");
        fs::remove_file(&installed).unwrap();
        fs::write(&installed, "collision").unwrap();
        assert_eq!(installation_status(&item), "conflicting");
        fs::remove_file(&installed).unwrap();
        create_symlink(&canonical, &installed).unwrap();
        assert_eq!(installation_status(&item), "installed");
        fs::remove_dir_all(&canonical).unwrap();
        assert_eq!(installation_status(&item), "stale");
    }
}
