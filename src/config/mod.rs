use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub const PROJECT_CONFIG: &str = ".overmind.toml";
pub const DEFAULT_URI: &str = "git@github.com:beelol/rules.git";
pub const DEFAULT_REF: &str = "master";
pub const DEFAULT_PACK: &str = "universal";
pub const DEFAULT_TARGETS: &[&str] = &[
    "agents",
    "claude",
    "gemini",
    "cursor",
    "cline",
    "roo",
    "antigravity",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileConfig {
    pub source: Option<SourceConfig>,
    pub sync: Option<SyncConfig>,
    pub skills: Option<SkillsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceConfig {
    pub uri: Option<String>,
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub pack: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    pub targets: Option<Vec<String>>,
    pub modules: Option<Vec<String>>,
}

/// Global-only selection for skills. Skills install machine-wide, so this is
/// read from the global config only (never a project `.overmind.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillsConfig {
    /// Master on/off toggle for the whole skills set (the "global pack").
    pub enabled: Option<bool>,
    /// If non-empty, install only these skill ids.
    pub only: Option<Vec<String>>,
    /// Install everything except these skill ids.
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSource {
    pub uri: String,
    pub ref_name: String,
    pub pack: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSync {
    pub targets: Vec<String>,
    pub explicit_targets: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub source: EffectiveSource,
    pub sync: EffectiveSync,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSkills {
    pub enabled: bool,
    pub only: Vec<String>,
    pub exclude: Vec<String>,
}

impl Default for EffectiveSkills {
    fn default() -> Self {
        Self {
            enabled: true,
            only: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

impl EffectiveSkills {
    /// Whether a skill with this id should be installed under the current selection.
    pub fn includes(&self, id: &str) -> bool {
        if !self.enabled {
            return false;
        }
        if !self.only.is_empty() && !self.only.iter().any(|item| item == id) {
            return false;
        }
        !self.exclude.iter().any(|item| item == id)
    }
}

impl Default for EffectiveSource {
    fn default() -> Self {
        Self {
            uri: DEFAULT_URI.to_string(),
            ref_name: DEFAULT_REF.to_string(),
            pack: DEFAULT_PACK.to_string(),
        }
    }
}

impl Default for EffectiveSync {
    fn default() -> Self {
        Self {
            targets: DEFAULT_TARGETS
                .iter()
                .map(|target| target.to_string())
                .collect(),
            explicit_targets: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FlagOverrides {
    pub source: Option<String>,
    pub ref_name: Option<String>,
    pub pack: Option<String>,
}

pub fn global_config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not resolve user config directory")?;
    Ok(base.join("overmind").join("config.toml"))
}

pub fn cache_root() -> Result<PathBuf> {
    let base = dirs::cache_dir().context("could not resolve user cache directory")?;
    Ok(base.join("overmind").join("sources"))
}

pub fn load_config(path: PathBuf) -> Result<FileConfig> {
    if !path.exists() {
        return Ok(FileConfig::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse config {}", path.display()))
}

pub fn resolve_effective_source(
    project_root: PathBuf,
    flags: FlagOverrides,
) -> Result<EffectiveSource> {
    Ok(resolve_effective_config(project_root, flags)?.source)
}

/// Resolve skill selection from the global config only. Skills are machine-wide,
/// so a per-project `.overmind.toml` must not govern them.
pub fn resolve_effective_skills() -> Result<EffectiveSkills> {
    let mut skills = EffectiveSkills::default();
    if let Some(config) = load_config(global_config_path()?)?.skills {
        if let Some(enabled) = config.enabled {
            skills.enabled = enabled;
        }
        if let Some(only) = config.only {
            skills.only = only;
        }
        if let Some(exclude) = config.exclude {
            skills.exclude = exclude;
        }
    }
    Ok(skills)
}

pub fn resolve_effective_config(
    project_root: PathBuf,
    flags: FlagOverrides,
) -> Result<EffectiveConfig> {
    let mut source = EffectiveSource::default();
    let mut sync = EffectiveSync::default();

    merge_file_config(&mut source, &mut sync, load_config(global_config_path()?)?);
    merge_file_config(
        &mut source,
        &mut sync,
        load_config(project_root.join(PROJECT_CONFIG))?,
    );

    if let Some(uri) = flags.source {
        source.uri = uri;
    }
    if let Some(ref_name) = flags.ref_name {
        source.ref_name = ref_name;
    }
    if let Some(pack) = flags.pack {
        source.pack = pack;
    }

    Ok(EffectiveConfig { source, sync })
}

pub fn merge_file_config(
    source_effective: &mut EffectiveSource,
    sync_effective: &mut EffectiveSync,
    config: FileConfig,
) {
    if let Some(source) = config.source {
        if let Some(uri) = source.uri {
            source_effective.uri = uri;
        }
        if let Some(ref_name) = source.ref_name {
            source_effective.ref_name = ref_name;
        }
        if let Some(pack) = source.pack {
            source_effective.pack = pack;
        }
    }
    if let Some(sync) = config.sync {
        if let Some(targets) = sync.targets {
            sync_effective.targets = targets;
            sync_effective.explicit_targets = true;
        }
    }
}

pub fn write_project_config(
    project_root: PathBuf,
    source: &EffectiveSource,
    dry_run: bool,
) -> Result<()> {
    let path = project_root.join(PROJECT_CONFIG);
    let body = format!(
        r#"[source]
uri = "{}"
ref = "{}"
pack = "{}"

[sync]
targets = ["agents", "claude", "gemini", "cursor", "cline", "roo", "antigravity"]
"#,
        source.uri, source.ref_name, source.pack
    );

    if dry_run {
        println!("Would write {}", path.display());
        return Ok(());
    }

    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))
}

impl From<&crate::cli::ProjectOptions> for FlagOverrides {
    fn from(options: &crate::cli::ProjectOptions) -> Self {
        Self {
            source: options.source.clone(),
            ref_name: options.ref_name.clone(),
            pack: options.pack.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_override_defaults() {
        let mut source = EffectiveSource::default();
        let mut sync = EffectiveSync::default();
        merge_file_config(
            &mut source,
            &mut sync,
            FileConfig {
                source: Some(SourceConfig {
                    uri: Some("../rules".into()),
                    ref_name: Some("dev".into()),
                    pack: Some("custom".into()),
                }),
                sync: None,
                skills: None,
            },
        );

        assert_eq!(source.uri, "../rules");
        assert_eq!(source.ref_name, "dev");
        assert_eq!(source.pack, "custom");
    }

    #[test]
    fn sync_targets_override_defaults() {
        let mut source = EffectiveSource::default();
        let mut sync = EffectiveSync::default();
        merge_file_config(
            &mut source,
            &mut sync,
            FileConfig {
                source: None,
                sync: Some(SyncConfig {
                    targets: Some(vec!["agents".into(), "cursor".into()]),
                    modules: None,
                }),
                skills: None,
            },
        );

        assert_eq!(
            sync.targets,
            vec!["agents".to_string(), "cursor".to_string()]
        );
        assert!(sync.explicit_targets);
    }

    #[test]
    fn parses_skills_section() {
        let config: FileConfig = toml::from_str(
            "[skills]\nenabled = false\nonly = [\"open-pr\"]\nexclude = [\"review-pr\"]\n",
        )
        .unwrap();
        let skills = config.skills.unwrap();
        assert_eq!(skills.enabled, Some(false));
        assert_eq!(skills.only, Some(vec!["open-pr".to_string()]));
        assert_eq!(skills.exclude, Some(vec!["review-pr".to_string()]));
    }

    #[test]
    fn effective_skills_includes_logic() {
        // Default: everything on.
        assert!(EffectiveSkills::default().includes("open-pr"));

        let disabled = EffectiveSkills {
            enabled: false,
            ..Default::default()
        };
        assert!(!disabled.includes("open-pr"));

        let only = EffectiveSkills {
            only: vec!["open-pr".into()],
            ..Default::default()
        };
        assert!(only.includes("open-pr"));
        assert!(!only.includes("review-pr"));

        let exclude = EffectiveSkills {
            exclude: vec!["review-pr".into()],
            ..Default::default()
        };
        assert!(exclude.includes("open-pr"));
        assert!(!exclude.includes("review-pr"));
    }
}
