use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{config::EffectiveSource, source::ResolvedSource};

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
        render_builtin_wrappers(project_root, source, effective, &rules, options.dry_run)?;
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
        let block = managed_block(source, effective, &rules);
        let rendered = apply_template(&template, source, effective, &rules, &block);
        write_section_managed_file(
            &project_root.join(target.path),
            &rendered,
            &block,
            options.dry_run,
        )?;
    }

    Ok(())
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
    let mut paths = vec![PathBuf::from("AGENTS.md")];

    if source.single_file {
        paths.extend(builtin_wrapper_paths().into_iter().map(PathBuf::from));
        return Ok(paths);
    }

    let manifest = load_manifest(source, &effective.pack)?;
    for target in manifest.targets {
        if target.id != "agents" && target.managed {
            paths.push(PathBuf::from(target.path));
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

fn upsert_managed_block(existing: &str, initial_body: &str, block: &str) -> Result<String> {
    if existing.contains(START_MARKER) || existing.contains(END_MARKER) {
        return replace_managed_block(existing, block);
    }

    insert_managed_block(existing, initial_body, block)
}

fn insert_managed_block(existing: &str, initial_body: &str, block: &str) -> Result<String> {
    if existing.trim().is_empty() {
        return Ok(initial_body.to_string());
    }

    let (prefix, suffix) = initial_body
        .split_once(block)
        .ok_or_else(|| anyhow!("template does not contain an Overmind managed block"))?;

    let insert_at = exact_prefix_insert_position(existing, prefix)
        .or_else(|| frontmatter_insert_position(existing))
        .or_else(|| {
            last_non_empty_line(prefix).and_then(|line| insert_after_matching_line(existing, line))
        })
        .or_else(|| {
            first_non_empty_line(suffix)
                .and_then(|line| insert_before_matching_line(existing, line))
        })
        .or_else(|| first_heading_insert_position(existing))
        .unwrap_or(0);

    Ok(stitch_sections(
        &existing[..insert_at],
        block,
        &existing[insert_at..],
    ))
}

fn exact_prefix_insert_position(existing: &str, prefix: &str) -> Option<usize> {
    if prefix.is_empty() || !existing.starts_with(prefix) {
        return None;
    }

    Some(prefix.len())
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

fn last_non_empty_line(text: &str) -> Option<&str> {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
}

fn first_non_empty_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

fn insert_after_matching_line(existing: &str, needle: &str) -> Option<usize> {
    let mut offset = 0;
    for line in existing.split_inclusive('\n') {
        if line.strip_suffix('\n').unwrap_or(line).trim() == needle {
            return Some(offset + line.len());
        }
        offset += line.len();
    }

    None
}

fn insert_before_matching_line(existing: &str, needle: &str) -> Option<usize> {
    let mut offset = 0;
    for line in existing.split_inclusive('\n') {
        if line.strip_suffix('\n').unwrap_or(line).trim() == needle {
            return Some(offset);
        }
        offset += line.len();
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

fn render_agents(
    project_root: &Path,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
    dry_run: bool,
) -> Result<()> {
    let target = project_root.join("AGENTS.md");
    let block = managed_block(source, effective, rules);
    let initial_body = default_agent_rule_body(&block);
    write_section_managed_file(&target, &initial_body, &block, dry_run)
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
    if is_generated_scaffold_only(&next) {
        if dry_run {
            println!("Would delete {}", path.display());
        } else {
            fs::remove_file(path)
                .with_context(|| format!("failed to delete {}", path.display()))?;
            println!("Deleted {}", path.display());
        }
        return Ok(());
    }

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

fn is_generated_scaffold_only(body: &str) -> bool {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .all(is_generated_scaffold_line)
}

fn is_generated_scaffold_line(line: &str) -> bool {
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

fn write_section_managed_file(
    path: &Path,
    initial_body: &str,
    block: &str,
    dry_run: bool,
) -> Result<()> {
    let body = if path.exists() {
        let existing = fs::read_to_string(path)?;
        upsert_managed_block(&existing, initial_body, block)?
    } else {
        if !initial_body.contains(START_MARKER) || !initial_body.contains(END_MARKER) {
            bail!(
                "template for {} does not contain an Overmind managed block",
                path.display()
            );
        }
        initial_body.to_string()
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

fn default_agent_rule_body(block: &str) -> String {
    format!(
        "# Agent Instructions\n\n{}\n\n## Project Instructions\n\nAdd project-specific instructions here.\n",
        block
    )
}

fn apply_template(
    template: &str,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
    overmind_block: &str,
) -> String {
    template
        .replace("{{source}}", &source.label)
        .replace("{{pack}}", &effective.pack)
        .replace("{{overmind_block}}", overmind_block)
        .replace("{{rules}}", rules.trim())
}

fn render_builtin_wrappers(
    project_root: &Path,
    source: &ResolvedSource,
    effective: &EffectiveSource,
    rules: &str,
    dry_run: bool,
) -> Result<()> {
    let block = managed_block(source, effective, rules);
    let wrappers = builtin_wrapper_paths().map(|path| {
        let body = match path {
            "CLAUDE.md" => format!(
                "{}\n\n## Project Instructions\n\nAdd project-specific Claude instructions here.\n",
                block
            ),
            "GEMINI.md" => format!(
                "{}\n\n## Project Instructions\n\nAdd project-specific Gemini instructions here.\n",
                block
            ),
            ".cursor/rules/AGENTS.mdc" => format!(
                "---\ndescription: Universal baseline agent rules\nglobs:\nalwaysApply: true\n---\n\n{}\n\n## Project Instructions\n\nAdd project-specific Cursor instructions here.\n",
                block
            ),
            ".cursorrules" => format!(
                "{}\n\n## Project Instructions\n\nAdd project-specific legacy Cursor instructions here.\n",
                block
            ),
            ".clinerules/AGENTS.md" => format!(
                "# Universal Agent Rules\n\n{}\n\n## Project Instructions\n\nAdd project-specific Cline instructions here.\n",
                block
            ),
            ".roo/rules/AGENTS.md" => format!(
                "# Roo Code Rules\n\n{}\n\n## Project Instructions\n\nAdd project-specific Roo Code instructions here.\n",
                block
            ),
            ".agent/rules/AGENTS.md" => format!(
                "# Universal Agent Rules\n\n{}\n\n## Project Instructions\n\nAdd project-specific Antigravity instructions here.\n",
                block
            ),
            _ => unreachable!("unknown built-in wrapper path"),
        };
        (path, body)
    });

    for (path, body) in wrappers {
        write_section_managed_file(&project_root.join(path), &body, &block, dry_run)?;
    }
    Ok(())
}

fn builtin_wrapper_paths() -> [&'static str; 7] {
    [
        "CLAUDE.md",
        "GEMINI.md",
        ".cursor/rules/AGENTS.mdc",
        ".cursorrules",
        ".clinerules/AGENTS.md",
        ".roo/rules/AGENTS.md",
        ".agent/rules/AGENTS.md",
    ]
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
    fn creates_missing_section_managed_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("AGENTS.md");
        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        let initial = default_agent_rule_body(block);

        write_section_managed_file(&path, &initial, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.contains(block));
        assert!(written.contains("## Project Instructions"));
    }

    #[test]
    fn section_managed_file_replaces_block_and_preserves_local_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(
            &path,
            "intro\n<!-- OVERMIND:START source=a pack=x -->\nold\n<!-- OVERMIND:END -->\nlocal\n",
        )
        .unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        write_section_managed_file(&path, block, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.contains("intro"));
        assert!(written.contains("new"));
        assert!(written.contains("local"));
        assert!(!written.contains("old"));
    }

    #[test]
    fn section_managed_file_inserts_block_into_existing_unmanaged_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "local only").unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        let initial = format!(
            "{}\n\n## Project Instructions\n\nAdd project-specific Claude instructions here.\n",
            block
        );

        write_section_managed_file(&path, &initial, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.starts_with(block));
        assert!(written.contains("local only"));
    }

    #[test]
    fn section_managed_file_inserts_block_after_heading() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("AGENTS.md");
        fs::write(&path, "# Agent Instructions\n\n- keep me\n").unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        let initial = default_agent_rule_body(block);

        write_section_managed_file(&path, &initial, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.starts_with("# Agent Instructions\n\n<!-- OVERMIND:START"));
        assert!(written.contains("- keep me"));
    }

    #[test]
    fn section_managed_file_inserts_block_after_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".cursor/rules/AGENTS.mdc");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "---\ndescription: Existing project rules\nglobs:\nalwaysApply: true\n---\n\nKeep this note.\n",
        )
        .unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        let initial = format!(
            "---\ndescription: Universal baseline agent rules\nglobs:\nalwaysApply: true\n---\n\n{}\n\n## Project Instructions\n\nAdd project-specific Cursor instructions here.\n",
            block
        );

        write_section_managed_file(&path, &initial, block, false).unwrap();

        let written = fs::read_to_string(path).unwrap();
        assert!(written.starts_with(
            "---\ndescription: Existing project rules\nglobs:\nalwaysApply: true\n---\n\n<!-- OVERMIND:START"
        ));
        assert!(written.contains("Keep this note."));
    }

    #[test]
    fn section_managed_file_rejects_broken_existing_block() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "<!-- OVERMIND:START source=a pack=x -->\nold\n").unwrap();

        let block = "<!-- OVERMIND:START source=b pack=x -->\nnew\n<!-- OVERMIND:END -->";
        let err = write_section_managed_file(&path, block, block, false).unwrap_err();
        assert!(err.to_string().contains("without matching"));
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
}
