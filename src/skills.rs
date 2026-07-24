use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::cli::{SkillsCommand, SkillsSyncOptions};
use crate::config::{self, EffectiveSkills, FlagOverrides};
use crate::source;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Installation {
    /// Stable id from the skills manifest. Backfilled from `skill` for pre-0.2.0 ledgers.
    #[serde(default)]
    pub id: String,
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

/// `skills/manifest.toml` in the rules repo. Mirrors the pack manifest shape.
#[derive(Debug, Deserialize)]
pub struct SkillsManifest {
    #[serde(default)]
    pub skills: Vec<SkillEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SkillEntry {
    pub id: String,
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

pub fn run(command: SkillsCommand) -> Result<()> {
    match command {
        SkillsCommand::Sync(options) => sync_global(options),
    }
}

fn sync_global(options: SkillsSyncOptions) -> Result<()> {
    let ledger_path = ledger_path()?;
    let home = dirs::home_dir().context("could not resolve home directory")?;
    let targets = [
        ("codex", home.join(".codex/skills")),
        ("claude", home.join(".claude/skills")),
    ];

    // Resolve the skills root: an explicit --source override (dev), otherwise the
    // configured rules repo's top-level skills/ dir (same resolution packs use).
    let skills_root = match &options.source {
        Some(source) => source.clone(),
        None => resolve_rules_skills_root(options.offline)?,
    };

    match load_skills_manifest(&skills_root)? {
        Some(manifest) => {
            let selection = config::resolve_effective_skills()?;
            let desired =
                build_desired_from_manifest(&skills_root, &manifest, &selection, &targets)?;
            apply(desired, &ledger_path, options.dry_run)
        }
        None => {
            eprintln!(
                "warning: resolving skills by directory name is deprecated and will be removed in \
                 a future release; add skills/manifest.toml to your rules repo."
            );
            reconcile(&skills_root, &targets, &ledger_path, options.dry_run)
        }
    }
}

/// Resolve `<rules repo>/skills` via the effective source (global config -> project -> defaults),
/// mirroring how packs locate `<source>/packs/<pack>`.
fn resolve_rules_skills_root(offline: bool) -> Result<PathBuf> {
    let project_root = std::env::current_dir()?;
    let effective = config::resolve_effective_source(project_root, FlagOverrides::default())?;
    let resolved = source::resolve(&effective, offline)?;
    Ok(resolved.path.join("skills"))
}

/// Read `<root>/skills/manifest.toml`. Returns None when absent (triggers the legacy path).
fn load_skills_manifest(root: &Path) -> Result<Option<SkillsManifest>> {
    let manifest_path = root.join("manifest.toml");
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = toml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    Ok(Some(manifest))
}

/// Build the desired installation set from the manifest, honoring per-entry `enabled`
/// and the global `[skills]` selection.
fn build_desired_from_manifest(
    root: &Path,
    manifest: &SkillsManifest,
    selection: &EffectiveSkills,
    targets: &[(&str, PathBuf)],
) -> Result<Vec<Installation>> {
    let mut desired = Vec::new();
    for entry in &manifest.skills {
        if !entry.enabled || !selection.includes(&entry.id) {
            continue;
        }
        let skill_dir = root.join(&entry.path);
        let canonical = skill_dir.canonicalize().with_context(|| {
            format!(
                "skill '{}' path not found: {}",
                entry.id,
                skill_dir.display()
            )
        })?;
        if !canonical.join("SKILL.md").is_file() {
            bail!(
                "skill '{}' has no SKILL.md at {}",
                entry.id,
                canonical.display()
            );
        }
        for (agent, root_dir) in targets {
            desired.push(Installation {
                id: entry.id.clone(),
                skill: entry.id.clone(),
                agent: (*agent).to_string(),
                canonical_path: canonical.clone(),
                installed_path: root_dir.join(&entry.id),
            });
        }
    }
    Ok(desired)
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
    let mut ledger: Ledger = toml::from_str(&raw)
        .with_context(|| format!("failed to parse skill ledger {}", path.display()))?;
    // Migrate pre-0.2.0 ledgers: backfill the stable id from the skill name.
    for item in &mut ledger.installations {
        if item.id.is_empty() {
            item.id = item.skill.clone();
        }
    }
    Ok(ledger)
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

/// Legacy name-match reconcile: discover `<source>/<name>/SKILL.md` dirs and install by name.
pub fn reconcile(
    source: &Path,
    targets: &[(&str, PathBuf)],
    ledger_path: &Path,
    dry_run: bool,
) -> Result<()> {
    let skills = discover(source)?;
    let mut desired = Vec::new();
    for (skill, canonical_path) in &skills {
        for (agent, root) in targets {
            desired.push(Installation {
                id: skill.clone(),
                skill: skill.clone(),
                agent: (*agent).to_string(),
                canonical_path: canonical_path.clone(),
                installed_path: root.join(skill),
            });
        }
    }
    apply(desired, ledger_path, dry_run)
}

/// Validate, (re)link, and atomically record the desired installation set.
fn apply(desired: Vec<Installation>, ledger_path: &Path, dry_run: bool) -> Result<()> {
    let old = load_ledger(ledger_path)?;

    let desired_paths: BTreeSet<_> = desired
        .iter()
        .map(|item| item.installed_path.clone())
        .collect();

    // Validate every destination before making any changes.
    for item in &desired {
        validate_destination(item)?;
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

fn validate_destination(item: &Installation) -> Result<()> {
    let Ok(metadata) = fs::symlink_metadata(&item.installed_path) else {
        return Ok(());
    };
    // ovmd is the source of truth: overwrite any symlink whose name matches a
    // skill so it points back at the canonical file (re-pointing stale links,
    // e.g. after the repo moves). Refuse only real files or directories we did
    // not create, to avoid clobbering unrelated content.
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    bail!(
        "refusing to replace non-symlink skill destination {}",
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
    fn repoints_symlinks_and_refuses_non_symlink_collision() {
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
        assert!(error.to_string().contains("non-symlink"));
    }

    #[test]
    fn overwrites_unmanaged_symlink_matching_skill() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let ledger = temp.path().join("ledger.toml");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        skill(&source, "one");
        // Pre-existing symlink we never recorded, pointing somewhere unrelated.
        create_symlink(temp.path(), &target.join("one")).unwrap();

        reconcile(&source, &[("codex", target.clone())], &ledger, false).unwrap();
        let canonical = source.canonicalize().unwrap();
        assert!(link_points_to(&target.join("one"), &canonical.join("one")));
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
            id: "one".into(),
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

    fn skills_root_with_manifest(root: &Path, entries: &[(&str, &str, bool)]) {
        fs::create_dir_all(root).unwrap();
        let mut manifest = String::new();
        for (id, path, enabled) in entries {
            skill(root, path);
            manifest.push_str(&format!(
                "[[skills]]\nid = \"{id}\"\npath = \"{path}\"\nenabled = {enabled}\n\n"
            ));
        }
        fs::write(root.join("manifest.toml"), manifest).unwrap();
    }

    #[test]
    fn manifest_drives_reconcile_by_id() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("skills");
        let target = temp.path().join("claude");
        let ledger = temp.path().join("ledger.toml");
        // dir name differs from id to prove resolution is by manifest id, not folder name.
        skills_root_with_manifest(&root, &[("open-pr", "open-pr-dir", true)]);

        let manifest = load_skills_manifest(&root).unwrap().unwrap();
        let targets = [("claude", target.clone())];
        let desired =
            build_desired_from_manifest(&root, &manifest, &EffectiveSkills::default(), &targets)
                .unwrap();
        apply(desired, &ledger, false).unwrap();

        let canonical = root.join("open-pr-dir").canonicalize().unwrap();
        assert!(link_points_to(&target.join("open-pr"), &canonical));
        assert!(!target.join("open-pr-dir").exists());
        assert_eq!(load_ledger(&ledger).unwrap().installations[0].id, "open-pr");
    }

    #[test]
    fn global_config_disables_and_filters_skills() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("skills");
        let target = temp.path().join("claude");
        skills_root_with_manifest(
            &root,
            &[
                ("open-pr", "open-pr", true),
                ("review-pr", "review-pr", true),
            ],
        );
        let manifest = load_skills_manifest(&root).unwrap().unwrap();
        let targets = [("claude", target)];

        // Master toggle off -> nothing desired.
        let off = EffectiveSkills {
            enabled: false,
            ..Default::default()
        };
        assert!(
            build_desired_from_manifest(&root, &manifest, &off, &targets)
                .unwrap()
                .is_empty()
        );

        // exclude drops one; only keeps one.
        let excluded = EffectiveSkills {
            exclude: vec!["review-pr".into()],
            ..Default::default()
        };
        let desired = build_desired_from_manifest(&root, &manifest, &excluded, &targets).unwrap();
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].id, "open-pr");

        let only = EffectiveSkills {
            only: vec!["review-pr".into()],
            ..Default::default()
        };
        let desired = build_desired_from_manifest(&root, &manifest, &only, &targets).unwrap();
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].id, "review-pr");
    }

    #[test]
    fn manifest_entry_disabled_is_skipped() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("skills");
        skills_root_with_manifest(&root, &[("open-pr", "open-pr", false)]);
        let manifest = load_skills_manifest(&root).unwrap().unwrap();
        let targets = [("claude", temp.path().join("claude"))];
        assert!(build_desired_from_manifest(
            &root,
            &manifest,
            &EffectiveSkills::default(),
            &targets
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn old_ledger_without_id_backfills_from_skill() {
        let temp = tempdir().unwrap();
        let ledger = temp.path().join("ledger.toml");
        // A pre-0.2.0 ledger row with no `id` field.
        fs::write(
            &ledger,
            "[[installations]]\nskill = \"open-pr\"\nagent = \"claude\"\n\
             canonical_path = \"/src/open-pr\"\ninstalled_path = \"/dst/open-pr\"\n",
        )
        .unwrap();
        let loaded = load_ledger(&ledger).unwrap();
        assert_eq!(loaded.installations[0].id, "open-pr");
    }

    #[test]
    fn missing_manifest_returns_none_for_fallback() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("skills");
        fs::create_dir_all(&root).unwrap();
        skill(&root, "open-pr"); // dirs present, but no manifest.toml
        assert!(load_skills_manifest(&root).unwrap().is_none());
    }
}
