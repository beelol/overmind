use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    config::{EffectiveSource, DEFAULT_TARGETS},
    source::ResolvedSource,
};

const START_MARKER: &str = "<!-- OVERMIND:START";
const END_MARKER: &str = "<!-- OVERMIND:END -->";

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
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default = "default_true")]
    pub managed: bool,
}

#[derive(Debug, Default)]
pub struct RenderOptions {
    pub dry_run: bool,
    pub only: Vec<String>,
    pub exclude: Vec<String>,
    pub targets: Vec<String>,
    pub explicit_targets: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinTargetKind {
    Agents,
    Claude,
    Gemini,
    Cursor,
    CursorLegacy,
    Cline,
    Roo,
    Antigravity,
}

#[derive(Debug, Clone, Copy)]
struct BuiltinTarget {
    id: &'static str,
    path: &'static str,
    legacy_paths: &'static [&'static str],
    kind: BuiltinTargetKind,
}

const BUILTIN_TARGETS: [BuiltinTarget; 8] = [
    BuiltinTarget {
        id: "agents",
        path: "AGENTS.md",
        legacy_paths: &[],
        kind: BuiltinTargetKind::Agents,
    },
    BuiltinTarget {
        id: "claude",
        path: "CLAUDE.md",
        legacy_paths: &[],
        kind: BuiltinTargetKind::Claude,
    },
    BuiltinTarget {
        id: "gemini",
        path: "GEMINI.md",
        legacy_paths: &[],
        kind: BuiltinTargetKind::Gemini,
    },
    BuiltinTarget {
        id: "cursor",
        path: ".cursor/rules/overmind.mdc",
        legacy_paths: &[".cursor/rules/AGENTS.mdc"],
        kind: BuiltinTargetKind::Cursor,
    },
    BuiltinTarget {
        id: "cursor-legacy",
        path: ".cursorrules",
        legacy_paths: &[],
        kind: BuiltinTargetKind::CursorLegacy,
    },
    BuiltinTarget {
        id: "cline",
        path: ".clinerules/overmind.md",
        legacy_paths: &[".clinerules/AGENTS.md"],
        kind: BuiltinTargetKind::Cline,
    },
    BuiltinTarget {
        id: "roo",
        path: ".roo/rules/overmind.md",
        legacy_paths: &[".roo/rules/AGENTS.md"],
        kind: BuiltinTargetKind::Roo,
    },
    BuiltinTarget {
        id: "antigravity",
        path: ".agent/rules/overmind.md",
        legacy_paths: &[".agent/rules/AGENTS.md"],
        kind: BuiltinTargetKind::Antigravity,
    },
];

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
    let selected_targets = selected_target_ids(options);

    render_builtin_targets(
        project_root,
        source,
        effective,
        &rules,
        &selected_targets,
        options.dry_run,
    )?;

    if source.single_file {
        return Ok(());
    }

    render_manifest_targets(
        project_root,
        source,
        effective,
        &rules,
        options,
        &selected_targets,
    )
}

pub fn unlink_project(
    project_root: &Path,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    dry_run: bool,
) -> Result<()> {
    for target in managed_target_paths(source, effective)? {
        unlink_managed_file(&project_root.join(target), dry_run)?;
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

fn managed_target_paths(
    source: &ResolvedSource,
    effective: &EffectiveSource,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for target in BUILTIN_TARGETS {
        push_unique_path(&mut paths, PathBuf::from(target.path));
        for legacy in target.legacy_paths {
            push_unique_path(&mut paths, PathBuf::from(legacy));
        }
    }

    if source.single_file {
        return Ok(paths);
    }

    let manifest = load_manifest(source, &effective.pack)?;
    for target in manifest.targets {
        if target.id != "agents" && target.managed && builtin_target(target.id.as_str()).is_none() {
            push_unique_path(&mut paths, PathBuf::from(target.path));
        }
    }

    Ok(paths)
}

pub fn replace_managed_block(existing: &str, block: &str) -> Result<String> {
    if let Some(start) = existing.find(START_MARKER) {
        if let Some(end_relative) = existing[start..].find(END_MARKER) {
            let end = start + end_relative + END_MARKER.len();
            let mut next = String::new();
            next.push_str(&existing[..start]);
            next.push_str(block);
            next.push_str(&existing[end..]);
            return Ok(next);
        }

        bail!(
            "\x1b[31mfound {} without matching {}. Manually clean the Overmind block before syncing.\x1b[0m",
            START_MARKER,
            END_MARKER
        );
    }

    if existing.contains(END_MARKER) {
        bail!(
            "\x1b[31mfound {} without matching {}. Manually clean the Overmind block before syncing.\x1b[0m",
            END_MARKER,
            START_MARKER
        );
    }

    bail!(
        "\x1b[31mexisting file does not contain an Overmind managed block. \
         Manually delete or move the existing Overmind content, then run `ovmd sync` again.\x1b[0m"
    );
}

fn upsert_managed_block(path: &Path, existing: &str, block: &str) -> Result<String> {
    if existing.contains(START_MARKER) || existing.contains(END_MARKER) {
        let without_block = remove_managed_block(existing)?;
        if is_overmind_virtualized_only(&without_block) {
            return Ok(existing_virtualized_only_body_for_path(
                path,
                &without_block,
                block,
            ));
        }
        return replace_managed_block(existing, block);
    }

    insert_managed_block(existing, block)
}

fn insert_managed_block(existing: &str, block: &str) -> Result<String> {
    if existing.trim().is_empty() {
        return Ok(managed_only_body(block));
    }

    let insert_at = frontmatter_insert_position(existing)
        .or_else(|| first_heading_insert_position(existing))
        .unwrap_or(0);

    Ok(stitch_sections(
        &existing[..insert_at],
        block,
        &existing[insert_at..],
    ))
}

fn frontmatter_insert_position(existing: &str) -> Option<usize> {
    if !existing.starts_with("---\n") {
        return None;
    }

    let mut offset = 4;
    for line in existing[4..].split_inclusive('\n') {
        if line.strip_suffix('\n').unwrap_or(line).trim() == "---" {
            return Some(offset + line.len());
        }
        offset += line.len();
    }

    None
}

fn first_heading_insert_position(existing: &str) -> Option<usize> {
    let mut offset = 0;
    for line in existing.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line).trim();
        if trimmed.is_empty() {
            offset += line.len();
            continue;
        }
        if trimmed.starts_with('#') {
            return Some(offset + line.len());
        }
        return None;
    }

    None
}

fn stitch_sections(prefix: &str, block: &str, suffix: &str) -> String {
    let mut sections = Vec::new();

    if !prefix.trim().is_empty() {
        sections.push(prefix.trim_end().to_string());
    }
    sections.push(block.trim().to_string());
    if !suffix.trim().is_empty() {
        sections.push(suffix.trim_start().to_string());
    }

    format!("{}\n", sections.join("\n\n"))
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

    fs::write(path, body).with_context(|| format!("failed to write {}", path.display()))?;
    println!("Updated {}", path.display());
    Ok(())
}

fn unlink_managed_file(path: &Path, dry_run: bool) -> Result<()> {
    if !path.exists() {
        println!("Missing {}", path.display());
        return Ok(());
    }

    let existing = fs::read_to_string(path)?;
    if !existing.contains(START_MARKER) && !existing.contains(END_MARKER) {
        println!("Unmanaged {}", path.display());
        return Ok(());
    }

    let next = remove_managed_block(&existing)?;
    if should_delete_unlinked_file(path, &next) {
        if dry_run {
            println!("Would delete {}", path.display());
        } else {
            fs::remove_file(path)
                .with_context(|| format!("failed to delete {}", path.display()))?;
            println!("Deleted {}", path.display());
        }
        return Ok(());
    }

    let next = unlinked_body_for_path(path, &next);
    if dry_run {
        println!("Would update {}", path.display());
        return Ok(());
    }

    fs::write(path, trim_unlinked_content(&next))
        .with_context(|| format!("failed to write {}", path.display()))?;
    println!("Updated {}", path.display());
    Ok(())
}

fn remove_managed_block(existing: &str) -> Result<String> {
    if let Some(start) = existing.find(START_MARKER) {
        if let Some(end_relative) = existing[start..].find(END_MARKER) {
            let end = start + end_relative + END_MARKER.len();
            let mut next = String::new();
            next.push_str(&existing[..start]);
            next.push_str(&existing[end..]);
            return Ok(next);
        }

        bail!(
            "\x1b[31mfound {} without matching {}. Manually clean the Overmind block before unlinking.\x1b[0m",
            START_MARKER,
            END_MARKER
        );
    }

    if existing.contains(END_MARKER) {
        bail!(
            "\x1b[31mfound {} without matching {}. Manually clean the Overmind block before unlinking.\x1b[0m",
            END_MARKER,
            START_MARKER
        );
    }

    Ok(existing.to_string())
}

fn is_overmind_virtualized_only(body: &str) -> bool {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .all(is_legacy_generated_scaffold_line)
}

fn is_legacy_generated_scaffold_line(line: &str) -> bool {
    matches!(
        line,
        "---"
            | "# Agent Instructions"
            | "# Universal Agent Rules"
            | "# Roo Code Rules"
            | "## Project Instructions"
            | "description: Universal baseline agent rules"
            | "globs:"
            | "alwaysApply: true"
            | "Add project-specific instructions here."
            | "Add project-specific Claude instructions here."
            | "Add project-specific Gemini instructions here."
            | "Add project-specific Cursor instructions here."
            | "Add project-specific legacy Cursor instructions here."
            | "Add project-specific Cline instructions here."
            | "Add project-specific Roo Code instructions here."
            | "Add project-specific Antigravity instructions here."
    )
}

fn trim_unlinked_content(body: &str) -> String {
    format!("{}\n", body.trim())
}

fn write_virtualized_file(path: &Path, block: &str, dry_run: bool) -> Result<()> {
    let body = if path.exists() {
        let existing = fs::read_to_string(path)?;
        upsert_managed_block(path, &existing, block)?
    } else {
        initial_body_for_path(path, block)
    };

    write_file_preserving_user_content(path, &body, dry_run)
}

fn managed_block(source: &ResolvedSource, effective: &EffectiveSource, rules: &str) -> String {
    format!(
        "<!-- OVERMIND:START source={} pack={} -->\n<!-- This section is generated by Overmind. Do not edit inside this block. -->\n{}\n<!-- OVERMIND:END -->",
        source.label,
        effective.pack,
        rules.trim()
    )
}

fn managed_only_body(block: &str) -> String {
    format!("{}\n", block.trim())
}

fn initial_body_for_path(path: &Path, block: &str) -> String {
    if let Some(policy) = product_file_policy(path) {
        return format!("{}{}", policy.created_file_prefix, managed_only_body(block));
    }

    managed_only_body(block)
}

fn existing_virtualized_only_body_for_path(path: &Path, remaining: &str, block: &str) -> String {
    if let Some(policy) = product_file_policy(path) {
        if policy.preserve_existing_frontmatter {
            if let Some(frontmatter) = leading_frontmatter(remaining) {
                return stitch_sections(frontmatter, block, "");
            }
        }
    }

    managed_only_body(block)
}

fn should_delete_unlinked_file(path: &Path, remaining: &str) -> bool {
    if let Some(policy) = product_file_policy(path) {
        if policy.preserve_existing_frontmatter
            && is_overmind_virtualized_only(remaining)
            && leading_frontmatter(remaining).is_some()
        {
            return false;
        }
    }

    is_overmind_virtualized_only(remaining)
}

fn unlinked_body_for_path(path: &Path, remaining: &str) -> String {
    if let Some(policy) = product_file_policy(path) {
        if policy.preserve_existing_frontmatter && is_overmind_virtualized_only(remaining) {
            if let Some(frontmatter) = leading_frontmatter(remaining) {
                return frontmatter.to_string();
            }
        }
    }

    remaining.to_string()
}

fn leading_frontmatter(body: &str) -> Option<&str> {
    frontmatter_insert_position(body).map(|end| &body[..end])
}

#[derive(Clone, Copy)]
struct ProductFilePolicy {
    created_file_prefix: &'static str,
    preserve_existing_frontmatter: bool,
}

fn product_file_policy(path: &Path) -> Option<ProductFilePolicy> {
    if path.ends_with(Path::new(".cursor/rules/AGENTS.mdc")) {
        return Some(ProductFilePolicy {
            created_file_prefix: "---\nalwaysApply: true\n---\n\n",
            preserve_existing_frontmatter: true,
        });
    }

    None
}

fn render_builtin_targets(
    project_root: &Path,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
    selected_targets: &HashSet<String>,
    dry_run: bool,
) -> Result<()> {
    for target in BUILTIN_TARGETS {
        if !selected_targets.contains(target.id) {
            continue;
        }
        let (initial_body, block) = builtin_target_body(target, source, effective, rules);
        sync_builtin_target(project_root, target, &initial_body, &block, dry_run)?;
    }

    Ok(())
}

fn render_manifest_targets(
    project_root: &Path,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
    options: &RenderOptions,
    selected_targets: &HashSet<String>,
) -> Result<()> {
    let manifest = load_manifest(source, &effective.pack)?;
    let root = pack_root(source, &effective.pack);
    for target in manifest.targets {
        if target.id == "agents" || !target.managed || builtin_target(target.id.as_str()).is_some()
        {
            continue;
        }
        if options.explicit_targets && !selected_targets.contains(&target.id) {
            continue;
        }
        let block = managed_block(source, effective, rules);
        if let Some(template) = target.template {
            let template_path = root.join(template);
            let template = fs::read_to_string(&template_path)
                .with_context(|| format!("failed to read template {}", template_path.display()))?;
            let rendered = apply_template(&template, source, effective, rules, &block);
            write_section_managed_file(
                &project_root.join(target.path),
                &rendered,
                &block,
                options.dry_run,
            )?;
        } else {
            write_virtualized_file(&project_root.join(target.path), &block, options.dry_run)?;
        }
    }

    Ok(())
}

fn selected_target_ids(options: &RenderOptions) -> HashSet<String> {
    let configured = if options.targets.is_empty() {
        DEFAULT_TARGETS
            .iter()
            .map(|target| target.to_string())
            .collect()
    } else {
        options.targets.clone()
    };

    let mut selected: HashSet<String> = configured.into_iter().collect();
    if selected.contains("claude")
        || selected.contains("gemini")
        || selected.contains("cursor")
        || selected.contains("cursor-legacy")
    {
        selected.insert("agents".into());
    }

    selected
}

fn builtin_target(id: &str) -> Option<BuiltinTarget> {
    BUILTIN_TARGETS
        .iter()
        .copied()
        .find(|target| target.id == id)
}

fn builtin_target_body(
    target: BuiltinTarget,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
) -> (String, String) {
    match target.kind {
        BuiltinTargetKind::Agents => {
            let block = managed_block(source, effective, rules);
            (default_agent_rule_body(&block), block)
        }
        BuiltinTargetKind::Claude => {
            let block = managed_block(source, effective, "@AGENTS.md");
            (
                format!(
                    "{}\n\n## Project Instructions\n\nAdd project-specific Claude instructions here.\n",
                    block
                ),
                block,
            )
        }
        BuiltinTargetKind::Gemini => {
            let block = managed_block(source, effective, "@AGENTS.md");
            (
                format!(
                    "{}\n\n## Project Instructions\n\nAdd project-specific Gemini instructions here.\n",
                    block
                ),
                block,
            )
        }
        BuiltinTargetKind::Cursor => {
            let block = managed_block(source, effective, "@../../AGENTS.md");
            (
                format!(
                    "---\ndescription: Shared Overmind rules\nglobs:\nalwaysApply: true\n---\n\n{}\n",
                    block
                ),
                block,
            )
        }
        BuiltinTargetKind::CursorLegacy => {
            let block = managed_block(source, effective, "@AGENTS.md");
            (
                format!(
                    "{}\n\n## Project Instructions\n\nAdd project-specific legacy Cursor instructions here.\n",
                    block
                ),
                block,
            )
        }
        BuiltinTargetKind::Cline | BuiltinTargetKind::Roo | BuiltinTargetKind::Antigravity => {
            let block = managed_block(source, effective, rules);
            (format!("{}\n", block), block)
        }
    }
}

fn sync_builtin_target(
    project_root: &Path,
    target: BuiltinTarget,
    initial_body: &str,
    block: &str,
    dry_run: bool,
) -> Result<()> {
    let current_path = project_root.join(target.path);
    let migrated_from = if current_path.exists() {
        write_section_managed_file(&current_path, initial_body, block, dry_run)?;
        None
    } else if let Some((legacy_path, migrated_body)) =
        migrate_from_legacy_path(project_root, target, initial_body, block)?
    {
        write_file_preserving_user_content(&current_path, &migrated_body, dry_run)?;
        Some(legacy_path)
    } else {
        write_section_managed_file(&current_path, initial_body, block, dry_run)?;
        None
    };

    for legacy in target.legacy_paths {
        let legacy_path = project_root.join(legacy);
        if migrated_from.as_ref() == Some(&legacy_path) {
            delete_file(&legacy_path, dry_run)?;
            continue;
        }
        cleanup_stale_legacy_target(&current_path, &legacy_path, dry_run)?;
    }

    Ok(())
}

fn default_agent_rule_body(block: &str) -> String {
    managed_only_body(block)
}

fn apply_template(
    template: &str,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
    block: &str,
) -> String {
    template
        .replace("{{overmind_block}}", block)
        .replace("{{rules}}", rules.trim())
        .replace("{{source}}", &source.label)
        .replace("{{pack}}", &effective.pack)
}

fn write_section_managed_file(path: &Path, body: &str, block: &str, dry_run: bool) -> Result<()> {
    let next = if path.exists() {
        let existing = fs::read_to_string(path)?;
        if contains_managed_marker(&existing) {
            replace_managed_block(&existing, block)?
        } else {
            insert_managed_block(&existing, block)?
        }
    } else {
        body.to_string()
    };

    write_file_preserving_user_content(path, &next, dry_run)
}

fn is_generated_scaffold_only(body: &str) -> bool {
    is_overmind_virtualized_only(body)
}

fn migrate_from_legacy_path(
    project_root: &Path,
    target: BuiltinTarget,
    initial_body: &str,
    block: &str,
) -> Result<Option<(PathBuf, String)>> {
    for legacy in target.legacy_paths {
        let legacy_path = project_root.join(legacy);
        if !legacy_path.exists() {
            continue;
        }

        let existing = fs::read_to_string(&legacy_path)?;
        if !contains_managed_marker(&existing) {
            println!(
                "Warning: leaving unmanaged legacy target {} in place while writing {}.",
                legacy_path.display(),
                project_root.join(target.path).display()
            );
            continue;
        }

        let body = migrate_legacy_body(&existing, initial_body, block)?;
        return Ok(Some((legacy_path, body)));
    }

    Ok(None)
}

fn migrate_legacy_body(existing: &str, initial_body: &str, block: &str) -> Result<String> {
    let remaining = remove_managed_block(existing)?;
    if remaining.trim().is_empty() || is_generated_scaffold_only(&remaining) {
        return Ok(initial_body.to_string());
    }

    replace_managed_block(existing, block)
}

fn cleanup_stale_legacy_target(
    current_path: &Path,
    legacy_path: &Path,
    dry_run: bool,
) -> Result<()> {
    if !legacy_path.exists() {
        return Ok(());
    }

    let existing = fs::read_to_string(legacy_path)?;
    if !contains_managed_marker(&existing) {
        println!(
            "Warning: leaving unmanaged legacy target {} in place. Remove it manually if {} is now the canonical file.",
            legacy_path.display(),
            current_path.display()
        );
        return Ok(());
    }

    let remaining = match remove_managed_block(&existing) {
        Ok(remaining) => remaining,
        Err(_) => {
            println!(
                "Warning: leaving legacy target {} in place because its Overmind block is malformed.",
                legacy_path.display()
            );
            return Ok(());
        }
    };

    if remaining.trim().is_empty() || is_generated_scaffold_only(&remaining) {
        return delete_file(legacy_path, dry_run);
    }

    println!(
        "Warning: leaving legacy target {} in place because it still has content outside the Overmind block. Move any kept content into {} and delete the legacy file when ready.",
        legacy_path.display(),
        current_path.display()
    );
    Ok(())
}

fn contains_managed_marker(existing: &str) -> bool {
    existing.contains(START_MARKER) || existing.contains(END_MARKER)
}

fn delete_file(path: &Path, dry_run: bool) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if dry_run {
        println!("Would delete {}", path.display());
        return Ok(());
    }

    fs::remove_file(path).with_context(|| format!("failed to delete {}", path.display()))?;
    println!("Deleted {}", path.display());
    Ok(())
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceKind;

    fn single_file_source(path: PathBuf) -> ResolvedSource {
        ResolvedSource {
            kind: SourceKind::LocalFile,
            path,
            label: "test-source".into(),
            single_file: true,
            git_backed: false,
        }
    }

    fn pack_source(path: PathBuf) -> ResolvedSource {
        ResolvedSource {
            kind: SourceKind::LocalDir,
            path,
            label: "test-pack".into(),
            single_file: false,
            git_backed: false,
        }
    }

    #[test]
    fn manifest_targets_do_not_require_templates() {
        let manifest: Manifest = toml::from_str(
            r#"
[[modules]]
id = "mission"
path = "rules/00-mission.md"

[[targets]]
id = "agents"
path = "AGENTS.md"
managed = true

[[targets]]
id = "claude"
path = "CLAUDE.md"
template = "legacy-template-is-ignored.tmpl"
managed = true
"#,
        )
        .unwrap();

        assert_eq!(manifest.targets.len(), 2);
        assert_eq!(manifest.targets[1].path, "CLAUDE.md");
    }

    #[test]
    fn replaces_existing_managed_block() {
        let existing =
            "before\n<!-- OVERMIND:START source=a pack=x -->\nold\n<!-- OVERMIND:END -->\nafter\n";
        let next = replace_managed_block(
            existing,
            "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->",
        )
        .unwrap();
        assert!(next.contains("before"));
        assert!(next.contains("new"));
        assert!(next.contains("after"));
        assert!(!next.contains("old"));
    }

    #[test]
    fn rejects_existing_content_without_managed_block() {
        let err = replace_managed_block(
            "# Existing\n\n- Keep me",
            "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->",
        )
        .unwrap_err();
        assert!(err.to_string().contains("does not contain an Overmind"));
    }

    #[test]
    fn rejects_broken_managed_block() {
        let err = replace_managed_block(
            "before\n<!-- OVERMIND:START source=a pack=x -->\nold\n",
            "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->",
        )
        .unwrap_err();
        assert!(err.to_string().contains("without matching"));
    }

    #[test]
    fn creates_missing_virtualized_file_with_only_managed_block() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("AGENTS.md");
        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";

        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert_eq!(written, managed_only_body(block));
    }

    #[test]
    fn creates_missing_cursor_rule_file_with_required_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".cursor/rules/AGENTS.mdc");
        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";

        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert_eq!(
            written,
            format!(
                "---\nalwaysApply: true\n---\n\n{}",
                managed_only_body(block)
            )
        );
    }

    #[test]
    fn virtualized_file_replaces_block_and_preserves_local_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(
            &path,
            "intro\n<!-- OVERMIND:START source=a pack=x -->\nold\n<!-- OVERMIND:END -->\nlocal\n",
        )
        .unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.contains("intro"));
        assert!(written.contains("new"));
        assert!(written.contains("local"));
        assert!(!written.contains("old"));
    }

    #[test]
    fn virtualized_file_drops_legacy_scaffold_when_replacing_block() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(
            &path,
            "# Agent Instructions\n\n<!-- OVERMIND:START source=a pack=x -->\nold\n<!-- OVERMIND:END -->\n\n## Project Instructions\n\nAdd project-specific Claude instructions here.\n",
        )
        .unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert_eq!(written, managed_only_body(block));
    }

    #[test]
    fn cursor_rule_drops_legacy_scaffold_but_preserves_existing_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".cursor/rules/AGENTS.mdc");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let frontmatter =
            "---\ndescription: Universal baseline agent rules\nglobs:\nalwaysApply: true\n---\n";
        fs::write(
            &path,
            format!(
                "{}\n<!-- OVERMIND:START source=a pack=x -->\nold\n<!-- OVERMIND:END -->\n\n## Project Instructions\n\nAdd project-specific Cursor instructions here.\n",
                frontmatter
            ),
        )
        .unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert_eq!(
            written,
            format!("{}\n{}", frontmatter, managed_only_body(block))
        );
    }

    #[test]
    fn existing_cursor_rule_block_only_does_not_get_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".cursor/rules/AGENTS.mdc");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "<!-- OVERMIND:START source=a pack=x -->\nold\n<!-- OVERMIND:END -->\n",
        )
        .unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert_eq!(written, managed_only_body(block));
    }

    #[test]
    fn virtualized_file_inserts_block_into_existing_unmanaged_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "local only").unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";

        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.starts_with(block));
        assert!(written.contains("local only"));
    }

    #[test]
    fn virtualized_file_inserts_block_after_heading() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("AGENTS.md");
        fs::write(&path, "# Agent Instructions\n\n- keep me\n").unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";

        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.starts_with("# Agent Instructions\n\n<!-- OVERMIND:START"));
        assert!(written.contains("- keep me"));
    }

    #[test]
    fn virtualized_file_inserts_block_after_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".cursor/rules/AGENTS.mdc");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "---\ndescription: Existing project rules\nglobs:\nalwaysApply: true\n---\n\nKeep this note.\n",
        )
        .unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";

        write_virtualized_file(&path, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.starts_with(
            "---\ndescription: Existing project rules\nglobs:\nalwaysApply: true\n---\n\n<!-- OVERMIND:START"
        ));
        assert!(written.contains("Keep this note."));
    }

    #[test]
    fn virtualized_file_rejects_broken_existing_block() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "<!-- OVERMIND:START source=a pack=x -->\nold\n").unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        let err = write_virtualized_file(&path, block, false).unwrap_err();
        assert!(err.to_string().contains("without matching"));
    }

    #[test]
    fn unlink_deletes_managed_block_only_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(
            &path,
            "<!-- OVERMIND:START source=a pack=x -->\nrules\n<!-- OVERMIND:END -->\n",
        )
        .unwrap();

        unlink_managed_file(&path, false).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn render_project_uses_vendor_native_builtin_targets() {
        let tmp = tempfile::tempdir().unwrap();
        let source_file = tmp.path().join("source.md");
        fs::write(&source_file, "shared rules").unwrap();
        let project = tempfile::tempdir().unwrap();

        render_project(
            project.path(),
            &single_file_source(source_file),
            &EffectiveSource::default(),
            &RenderOptions {
                targets: DEFAULT_TARGETS
                    .iter()
                    .map(|target| target.to_string())
                    .collect(),
                explicit_targets: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(project.path().join("AGENTS.md").exists());
        assert!(project.path().join("CLAUDE.md").exists());
        assert!(project.path().join("GEMINI.md").exists());
        assert!(project.path().join(".cursor/rules/overmind.mdc").exists());
        assert!(project.path().join(".clinerules/overmind.md").exists());
        assert!(project.path().join(".roo/rules/overmind.md").exists());
        assert!(project.path().join(".agent/rules/overmind.md").exists());
        assert!(!project.path().join(".cursor/rules/AGENTS.mdc").exists());

        let claude = fs::read_to_string(project.path().join("CLAUDE.md")).unwrap();
        let gemini = fs::read_to_string(project.path().join("GEMINI.md")).unwrap();
        let cursor = fs::read_to_string(project.path().join(".cursor/rules/overmind.mdc")).unwrap();
        let cline = fs::read_to_string(project.path().join(".clinerules/overmind.md")).unwrap();

        assert!(claude.contains("@AGENTS.md"));
        assert!(gemini.contains("@AGENTS.md"));
        assert!(cursor.contains("@../../AGENTS.md"));
        assert!(!cursor.contains("shared rules"));
        assert!(cline.contains("shared rules"));
    }

    #[test]
    fn render_project_honors_target_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let source_file = tmp.path().join("source.md");
        fs::write(&source_file, "shared rules").unwrap();
        let project = tempfile::tempdir().unwrap();

        render_project(
            project.path(),
            &single_file_source(source_file),
            &EffectiveSource::default(),
            &RenderOptions {
                targets: vec!["cline".into()],
                explicit_targets: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(!project.path().join("CLAUDE.md").exists());
        assert!(!project.path().join(".cursor/rules/overmind.mdc").exists());
        assert!(project.path().join(".clinerules/overmind.md").exists());
        assert!(!project.path().join("AGENTS.md").exists());
    }

    #[test]
    fn render_project_migrates_legacy_directory_target() {
        let tmp = tempfile::tempdir().unwrap();
        let source_file = tmp.path().join("source.md");
        fs::write(&source_file, "shared rules").unwrap();
        let project = tempfile::tempdir().unwrap();
        let legacy = project.path().join(".clinerules/AGENTS.md");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(
            &legacy,
            "# Universal Agent Rules\n\n<!-- OVERMIND:START source=old pack=universal -->\nold rules\n<!-- OVERMIND:END -->\n\n- keep me\n",
        )
        .unwrap();

        render_project(
            project.path(),
            &single_file_source(source_file),
            &EffectiveSource::default(),
            &RenderOptions {
                targets: vec!["cline".into()],
                explicit_targets: true,
                ..Default::default()
            },
        )
        .unwrap();

        let current = project.path().join(".clinerules/overmind.md");
        let written = fs::read_to_string(current).unwrap();
        assert!(written.contains("shared rules"));
        assert!(written.contains("- keep me"));
        assert!(!legacy.exists());
    }

    #[test]
    fn render_project_keeps_legacy_directory_target_with_user_content_when_new_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let source_file = tmp.path().join("source.md");
        fs::write(&source_file, "shared rules").unwrap();
        let project = tempfile::tempdir().unwrap();
        let current = project.path().join(".clinerules/overmind.md");
        let legacy = project.path().join(".clinerules/AGENTS.md");
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        fs::write(
            &current,
            "<!-- OVERMIND:START source=old pack=universal -->\nold rules\n<!-- OVERMIND:END -->\n",
        )
        .unwrap();
        fs::write(
            &legacy,
            "# Universal Agent Rules\n\n<!-- OVERMIND:START source=old pack=universal -->\nold rules\n<!-- OVERMIND:END -->\n\n- keep legacy note\n",
        )
        .unwrap();

        render_project(
            project.path(),
            &single_file_source(source_file),
            &EffectiveSource::default(),
            &RenderOptions {
                targets: vec!["cline".into()],
                explicit_targets: true,
                ..Default::default()
            },
        )
        .unwrap();

        let current_written = fs::read_to_string(current).unwrap();
        let legacy_written = fs::read_to_string(legacy).unwrap();
        assert!(current_written.contains("shared rules"));
        assert!(legacy_written.contains("- keep legacy note"));
        assert!(legacy_written.contains("OVERMIND:START"));
    }

    #[test]
    fn render_project_renders_unknown_manifest_targets_without_explicit_target_config() {
        let tmp = tempfile::tempdir().unwrap();
        let pack_root = tmp.path().join("packs/universal");
        fs::create_dir_all(pack_root.join("rules")).unwrap();
        fs::create_dir_all(pack_root.join("templates")).unwrap();
        fs::write(
            pack_root.join("manifest.toml"),
            r#"
modules = [{ id = "base", path = "rules/base.md", enabled = true }]
targets = [{ id = "custom", path = "CUSTOM.md", template = "templates/custom.md", managed = true }]
"#,
        )
        .unwrap();
        fs::write(pack_root.join("rules/base.md"), "shared rules").unwrap();
        fs::write(
            pack_root.join("templates/custom.md"),
            "{{overmind_block}}\n\ncustom target\n",
        )
        .unwrap();
        let project = tempfile::tempdir().unwrap();

        render_project(
            project.path(),
            &pack_source(tmp.path().to_path_buf()),
            &EffectiveSource::default(),
            &RenderOptions::default(),
        )
        .unwrap();

        assert!(project.path().join("CUSTOM.md").exists());
    }

    #[test]
    fn render_project_filters_unknown_manifest_targets_when_targets_are_explicit() {
        let tmp = tempfile::tempdir().unwrap();
        let pack_root = tmp.path().join("packs/universal");
        fs::create_dir_all(pack_root.join("rules")).unwrap();
        fs::create_dir_all(pack_root.join("templates")).unwrap();
        fs::write(
            pack_root.join("manifest.toml"),
            r#"
modules = [{ id = "base", path = "rules/base.md", enabled = true }]
targets = [{ id = "custom", path = "CUSTOM.md", template = "templates/custom.md", managed = true }]
"#,
        )
        .unwrap();
        fs::write(pack_root.join("rules/base.md"), "shared rules").unwrap();
        fs::write(
            pack_root.join("templates/custom.md"),
            "{{overmind_block}}\n\ncustom target\n",
        )
        .unwrap();
        let project = tempfile::tempdir().unwrap();

        render_project(
            project.path(),
            &pack_source(tmp.path().to_path_buf()),
            &EffectiveSource::default(),
            &RenderOptions {
                targets: vec!["agents".into()],
                explicit_targets: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(!project.path().join("CUSTOM.md").exists());
    }

    #[test]
    fn unlink_deletes_generated_scaffold_only_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(
            &path,
            "<!-- OVERMIND:START source=a pack=x -->\nrules\n<!-- OVERMIND:END -->\n\n## Project Instructions\n\nAdd project-specific Claude instructions here.\n",
        )
        .unwrap();

        unlink_managed_file(&path, false).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn unlink_preserves_local_content_outside_block() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(
            &path,
            "<!-- OVERMIND:START source=a pack=x -->\nrules\n<!-- OVERMIND:END -->\n\n## Project Instructions\n\n- keep me\n",
        )
        .unwrap();

        unlink_managed_file(&path, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(!written.contains("OVERMIND"));
        assert!(written.contains("- keep me"));
    }

    #[test]
    fn unlink_preserves_cursor_frontmatter_when_only_metadata_remains() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".cursor/rules/AGENTS.mdc");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let frontmatter = "---\nalwaysApply: true\n---\n";
        fs::write(
            &path,
            format!(
                "{}\n<!-- OVERMIND:START source=a pack=x -->\nrules\n<!-- OVERMIND:END -->\n",
                frontmatter
            ),
        )
        .unwrap();

        unlink_managed_file(&path, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert_eq!(written, frontmatter);
    }

    #[test]
    fn unlink_removes_current_and_legacy_builtin_targets() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let current = project.path().join(".clinerules/overmind.md");
        let legacy = project.path().join(".clinerules/AGENTS.md");
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        fs::write(
            &current,
            "<!-- OVERMIND:START source=a pack=universal -->\nrules\n<!-- OVERMIND:END -->\n",
        )
        .unwrap();
        fs::write(
            &legacy,
            "<!-- OVERMIND:START source=a pack=universal -->\nrules\n<!-- OVERMIND:END -->\n",
        )
        .unwrap();

        unlink_project(
            project.path(),
            &single_file_source(tmp.path().join("source.md")),
            &EffectiveSource::default(),
            false,
        )
        .unwrap();

        assert!(!current.exists());
        assert!(!legacy.exists());
    }
}
