use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{config::EffectiveSource, source::ResolvedSource};

const START_MARKER: &str = "<!-- OVERMIND:START";
const END_MARKER: &str = "<!-- OVERMIND:END -->";
const MANAGED_TEXT: &str = "Managed by Overmind";

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub modules: Vec<Module>,
    pub targets: Vec<Target>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Module {
    pub id: String,
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Target {
    pub id: String,
    pub path: String,
    pub template: String,
    #[serde(default = "default_true")]
    pub managed: bool,
}

#[derive(Debug, Default)]
pub struct RenderOptions {
    pub dry_run: bool,
    pub only: Vec<String>,
    pub exclude: Vec<String>,
}

fn default_true() -> bool {
    true
}

pub fn load_manifest(source: &ResolvedSource, pack: &str) -> Result<Manifest> {
    let manifest_path = pack_root(source, pack).join("manifest.toml");
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse {}", manifest_path.display()))
}

pub fn pack_root(source: &ResolvedSource, pack: &str) -> PathBuf {
    source.path.join("packs").join(pack)
}

pub fn build_rules(
    source: &ResolvedSource,
    effective: &EffectiveSource,
    options: &RenderOptions,
) -> Result<String> {
    if source.single_file {
        return fs::read_to_string(&source.path)
            .with_context(|| format!("failed to read {}", source.path.display()));
    }

    let manifest = load_manifest(source, &effective.pack)?;
    let root = pack_root(source, &effective.pack);
    let only: HashSet<_> = options.only.iter().cloned().collect();
    let exclude: HashSet<_> = options.exclude.iter().cloned().collect();
    let mut parts = Vec::new();

    for module in manifest.modules {
        if !module.enabled {
            continue;
        }
        if !only.is_empty() && !only.contains(&module.id) {
            continue;
        }
        if exclude.contains(&module.id) {
            continue;
        }
        let path = root.join(&module.path);
        let body = fs::read_to_string(&path)
            .with_context(|| format!("failed to read module {}", path.display()))?;
        parts.push(body.trim().to_string());
    }

    if parts.is_empty() {
        return Err(anyhow!("no rule modules selected"));
    }

    Ok(parts.join("\n\n"))
}

pub fn render_project(
    project_root: &Path,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    options: &RenderOptions,
) -> Result<()> {
    let rules = build_rules(source, effective, options)?;
    render_agents(project_root, source, effective, &rules, options.dry_run)?;

    if source.single_file {
        render_builtin_wrappers(project_root, options.dry_run)?;
        return Ok(());
    }

    let manifest = load_manifest(source, &effective.pack)?;
    let root = pack_root(source, &effective.pack);
    for target in manifest.targets {
        if target.id == "agents" || !target.managed {
            continue;
        }
        let template_path = root.join(target.template);
        let template = fs::read_to_string(&template_path)
            .with_context(|| format!("failed to read template {}", template_path.display()))?;
        let rendered = apply_template(&template, source, effective, &rules);
        write_managed_file(&project_root.join(target.path), &rendered, options.dry_run)?;
    }

    Ok(())
}

pub fn build_pack_artifact(
    source: &ResolvedSource,
    effective: &EffectiveSource,
    dry_run: bool,
) -> Result<()> {
    if source.single_file {
        return Err(anyhow!("single-file sources do not have pack artifacts"));
    }
    let rules = build_rules(source, effective, &RenderOptions::default())?;
    let path = source.path.join("AGENTS.md");
    if dry_run {
        println!("Would write {}", path.display());
        return Ok(());
    }
    fs::write(&path, format!("{}\n", rules.trim()))?;
    println!("Built {}", path.display());
    Ok(())
}

pub fn list_modules(source: &ResolvedSource, effective: &EffectiveSource) -> Result<Vec<Module>> {
    if source.single_file {
        return Ok(vec![Module {
            id: "single-file".into(),
            path: source.path.display().to_string(),
            enabled: true,
        }]);
    }
    Ok(load_manifest(source, &effective.pack)?.modules)
}

pub fn replace_managed_block(existing: &str, block: &str) -> String {
    if let Some(start) = existing.find(START_MARKER) {
        if let Some(end_relative) = existing[start..].find(END_MARKER) {
            let end = start + end_relative + END_MARKER.len();
            let mut next = String::new();
            next.push_str(&existing[..start]);
            next.push_str(block);
            next.push_str(&existing[end..]);
            return next;
        }
    }

    if existing.trim().is_empty() {
        return format!("# Agent Instructions\n\n{}\n\n## Project Instructions\n\nAdd project-specific instructions here.\n", block);
    }

    format!(
        "# Agent Instructions\n\n{}\n\n## Project Instructions\n\n{}",
        block,
        existing.trim_start()
    )
}

fn render_agents(
    project_root: &Path,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
    dry_run: bool,
) -> Result<()> {
    let target = project_root.join("AGENTS.md");
    let block = format!(
        "<!-- OVERMIND:START source={} pack={} -->\n{}\n<!-- OVERMIND:END -->",
        source.label,
        effective.pack,
        rules.trim()
    );
    let existing = fs::read_to_string(&target).unwrap_or_default();
    let rendered = replace_managed_block(&existing, &block);
    write_file_preserving_user_content(&target, &rendered, dry_run)
}

fn write_managed_file(path: &Path, body: &str, dry_run: bool) -> Result<()> {
    write_file_preserving_user_content(path, body, dry_run)
}

fn write_file_preserving_user_content(path: &Path, body: &str, dry_run: bool) -> Result<()> {
    if path.exists() && fs::read_to_string(path)? == body {
        println!("Unchanged {}", path.display());
        return Ok(());
    }

    if dry_run {
        if path.exists() {
            println!("Would update {}", path.display());
        } else {
            println!("Would create {}", path.display());
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if path.exists() {
        let existing = fs::read_to_string(path).unwrap_or_default();
        if !existing.contains(MANAGED_TEXT) && !existing.contains(START_MARKER) {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let backup = PathBuf::from(format!("{}.bak.{}", path.display(), stamp));
            fs::copy(path, &backup).with_context(|| {
                format!(
                    "failed to back up {} to {}",
                    path.display(),
                    backup.display()
                )
            })?;
            println!("Backed up {} to {}", path.display(), backup.display());
        }
    }

    fs::write(path, body).with_context(|| format!("failed to write {}", path.display()))?;
    println!("Updated {}", path.display());
    Ok(())
}

fn apply_template(
    template: &str,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
) -> String {
    template
        .replace("{{source}}", &source.label)
        .replace("{{pack}}", &effective.pack)
        .replace("{{rules}}", rules.trim())
}

fn render_builtin_wrappers(project_root: &Path, dry_run: bool) -> Result<()> {
    let wrappers = [
        (
            "CLAUDE.md",
            "<!-- Managed by Overmind. Edit the Project Instructions section in AGENTS.md for local rules. -->\n@AGENTS.md\n",
        ),
        (
            "GEMINI.md",
            "<!-- Managed by Overmind. Edit the Project Instructions section in AGENTS.md for local rules. -->\n@AGENTS.md\n",
        ),
        (
            ".cursor/rules/universal-agent-rules.mdc",
            "---\ndescription: Universal baseline agent rules\nglobs:\nalwaysApply: true\n---\n\nManaged by Overmind. Follow the project root `AGENTS.md` for universal baseline and project-specific instructions. Do not edit the managed universal block in `AGENTS.md`; add local instructions below it.\n",
        ),
        (
            ".cursorrules",
            "Managed by Overmind. Follow the project root AGENTS.md for universal baseline and project-specific instructions. Do not edit the managed universal block in AGENTS.md; add local instructions below it.\n",
        ),
        (
            ".clinerules/universal-agent-rules.md",
            "# Universal Agent Rules\n\nManaged by Overmind. Follow the project root `AGENTS.md` for universal baseline and project-specific instructions. Do not edit the managed universal block in `AGENTS.md`; add local instructions below it.\n",
        ),
        (
            ".agent/rules/universal-agent-rules.md",
            "# Universal Agent Rules\n\nManaged by Overmind. Follow the project root `AGENTS.md` for universal baseline and project-specific instructions. Do not edit the managed universal block in `AGENTS.md`; add local instructions below it.\n",
        ),
    ];

    for (path, body) in wrappers {
        write_managed_file(&project_root.join(path), body, dry_run)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_managed_block() {
        let existing =
            "before\n<!-- OVERMIND:START source=a pack=x -->\nold\n<!-- OVERMIND:END -->\nafter\n";
        let next = replace_managed_block(
            existing,
            "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->",
        );
        assert!(next.contains("before"));
        assert!(next.contains("new"));
        assert!(next.contains("after"));
        assert!(!next.contains("old"));
    }

    #[test]
    fn preserves_existing_content_on_first_conversion() {
        let next = replace_managed_block(
            "# Existing\n\n- Keep me\n",
            "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->",
        );
        assert!(next.contains("- Keep me"));
        assert!(next.contains("## Project Instructions"));
    }
}
