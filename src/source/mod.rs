use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::config::{cache_root, EffectiveSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    LocalFile,
    LocalDir,
    Git,
    Http,
}

#[derive(Debug, Clone)]
pub struct ResolvedSource {
    pub kind: SourceKind,
    pub path: PathBuf,
    pub label: String,
    pub single_file: bool,
    pub git_backed: bool,
}

pub fn classify(uri: &str) -> SourceKind {
    let expanded = expand_tilde(uri);
    let path = Path::new(&expanded);
    if path.is_file() {
        return SourceKind::LocalFile;
    }
    if path.is_dir() {
        return SourceKind::LocalDir;
    }
    if uri.starts_with("file://") {
        let local = uri.trim_start_matches("file://");
        return if Path::new(local).is_file() {
            SourceKind::LocalFile
        } else {
            SourceKind::LocalDir
        };
    }
    if uri.starts_with("ssh://") || uri.starts_with("git://") || uri.ends_with(".git") {
        return SourceKind::Git;
    }
    if uri.contains('@') && uri.contains(':') {
        return SourceKind::Git;
    }
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return SourceKind::Http;
    }
    SourceKind::LocalDir
}

pub fn resolve(source: &EffectiveSource, offline: bool) -> Result<ResolvedSource> {
    match classify(&source.uri) {
        SourceKind::LocalFile => {
            let path = local_path(&source.uri)?;
            Ok(ResolvedSource {
                kind: SourceKind::LocalFile,
                path,
                label: source.uri.clone(),
                single_file: true,
                git_backed: false,
            })
        }
        SourceKind::LocalDir => {
            let path = local_path(&source.uri)?;
            Ok(ResolvedSource {
                kind: SourceKind::LocalDir,
                path,
                label: source.uri.clone(),
                single_file: false,
                git_backed: is_git_repo(&source.uri),
            })
        }
        SourceKind::Git => {
            let path = git_cache_path(source)?;
            if !offline {
                update_git_source(&source.uri, &source.ref_name, &path)?;
            }
            Ok(ResolvedSource {
                kind: SourceKind::Git,
                path,
                label: format!("{}#{}", source.uri, source.ref_name),
                single_file: false,
                git_backed: true,
            })
        }
        SourceKind::Http => {
            let path = http_cache_path(source)?;
            if !offline {
                update_http_source(&source.uri, &path)?;
            }
            Ok(ResolvedSource {
                kind: SourceKind::Http,
                path: path.join("AGENTS.md"),
                label: source.uri.clone(),
                single_file: true,
                git_backed: false,
            })
        }
    }
}

pub fn cache_path_for(source: &EffectiveSource) -> Result<PathBuf> {
    Ok(cache_root()?.join(cache_key(source)))
}

pub fn cache_key(source: &EffectiveSource) -> String {
    let slug = slugify_source(&source.uri);
    let mut hasher = Sha256::new();
    hasher.update(source.uri.as_bytes());
    hasher.update(b"|");
    hasher.update(source.ref_name.as_bytes());
    hasher.update(b"|");
    hasher.update(source.pack.as_bytes());
    let hash = hex::encode(hasher.finalize());
    format!("{}-{}", slug, &hash[..8])
}

pub fn slugify_source(uri: &str) -> String {
    let mut s = uri
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("ssh://")
        .trim_start_matches("git@")
        .trim_end_matches(".git")
        .replace([':', '/', '\\'], "-");
    s.retain(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    s.trim_matches('-').to_string()
}

fn git_cache_path(source: &EffectiveSource) -> Result<PathBuf> {
    cache_path_for(source)
}

fn http_cache_path(source: &EffectiveSource) -> Result<PathBuf> {
    cache_path_for(source)
}

fn update_git_source(uri: &str, ref_name: &str, path: &Path) -> Result<()> {
    if path.exists() {
        run_git(["-C", path_str(path)?, "fetch", "origin", ref_name])?;
        run_git(["-C", path_str(path)?, "checkout", ref_name])?;
        run_git([
            "-C",
            path_str(path)?,
            "pull",
            "--ff-only",
            "origin",
            ref_name,
        ])?;
    } else {
        let parent = path.parent().context("cache path has no parent")?;
        fs::create_dir_all(parent)?;
        run_git(["clone", "--branch", ref_name, uri, path_str(path)?])?;
    }
    Ok(())
}

fn update_http_source(uri: &str, path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let response = ureq::get(uri)
        .call()
        .map_err(|err| anyhow!("failed to download {}: {}", uri, err))?;
    let body = response
        .into_string()
        .map_err(|err| anyhow!("failed to read response body for {}: {}", uri, err))?;
    if body.trim().is_empty() {
        return Err(anyhow!("downloaded source is empty: {}", uri));
    }
    fs::write(path.join("AGENTS.md"), body)?;
    Ok(())
}

fn run_git<const N: usize>(args: [&str; N]) -> Result<()> {
    let status = Command::new("git").args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("git command failed with status {}", status))
    }
}

fn local_path(uri: &str) -> Result<PathBuf> {
    if let Some(path) = uri.strip_prefix("file://") {
        return Ok(PathBuf::from(path));
    }
    Ok(PathBuf::from(expand_tilde(uri)))
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

fn path_str(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))
}

fn is_git_repo(uri: &str) -> bool {
    let path = PathBuf::from(expand_tilde(uri));
    path.join(".git").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_git_ssh_source() {
        assert_eq!(classify("git@github.com:beelol/rules.git"), SourceKind::Git);
    }

    #[test]
    fn detects_http_raw_source() {
        assert_eq!(
            classify("https://raw.githubusercontent.com/beelol/rules/master/AGENTS.md"),
            SourceKind::Http
        );
    }

    #[test]
    fn cache_key_is_readable_and_hashed() {
        let source = EffectiveSource::default();
        let key = cache_key(&source);
        assert!(key.starts_with("github.com-beelol-rules-"));
        assert!(key.len() > "github.com-beelol-rules-".len());
    }
}
