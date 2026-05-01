use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct JobAssetResolver {
    root: PathBuf,
    temp_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetResolution {
    pub requested_path: String,
    pub resolved_path: Option<String>,
    pub exists: bool,
    pub candidates: Vec<String>,
}

impl JobAssetResolver {
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        Self {
            root: normalize_root(root.as_ref()),
            temp_root: None,
        }
    }

    pub fn from_archive(archive: impl AsRef<Path>) -> anyhow::Result<Self> {
        let archive = archive.as_ref();
        let temp_root = unique_temp_dir("ae-native-renderer-job")?;
        fs::create_dir_all(&temp_root)?;
        let output = Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&temp_root)
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "failed to extract job archive {}: {}",
                archive.display(),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let root = extracted_app_root(&temp_root);
        Ok(Self {
            root,
            temp_root: Some(temp_root),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve(&self, requested_path: &str, kind: Option<&str>) -> AssetResolution {
        let mut candidates = Vec::new();
        let requested = Path::new(requested_path);

        if requested.is_absolute() {
            push_candidate(&mut candidates, requested.to_path_buf());
            if requested.exists() {
                return found(requested_path, requested.to_path_buf(), candidates);
            }
        }

        push_candidate(&mut candidates, self.root.join(requested_path));

        let file_name = requested
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty());

        if let Some(file_name) = &file_name {
            match kind {
                Some("video") => push_candidate(
                    &mut candidates,
                    self.root.join("media/video").join(file_name),
                ),
                Some("audio") => push_candidate(
                    &mut candidates,
                    self.root.join("media/audio").join(file_name),
                ),
                Some("image") => push_candidate(
                    &mut candidates,
                    self.root.join("media/image").join(file_name),
                ),
                _ => {}
            }
            push_candidate(&mut candidates, self.root.join("media").join(file_name));
        }

        if let Some(candidate) = candidates
            .iter()
            .find(|candidate| Path::new(candidate.as_str()).exists())
            .cloned()
        {
            return found(requested_path, PathBuf::from(candidate), candidates);
        }

        if let Some(file_name) = &file_name {
            if let Some(path) = find_by_file_name(&self.root, file_name, 5) {
                push_candidate(&mut candidates, path.clone());
                return found(requested_path, path, candidates);
            }
        }

        AssetResolution {
            requested_path: requested_path.to_string(),
            resolved_path: None,
            exists: false,
            candidates,
        }
    }
}

impl Drop for JobAssetResolver {
    fn drop(&mut self) {
        if let Some(path) = &self.temp_root {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn normalize_root(root: &Path) -> PathBuf {
    if root.join("media").exists() {
        root.to_path_buf()
    } else if root.join("app/media").exists() {
        root.join("app")
    } else {
        root.to_path_buf()
    }
}

fn extracted_app_root(temp_root: &Path) -> PathBuf {
    if temp_root.join("media").exists() {
        return temp_root.to_path_buf();
    }
    if temp_root.join("app/media").exists() {
        return temp_root.join("app");
    }
    if let Ok(entries) = fs::read_dir(temp_root) {
        let dirs = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        if dirs.len() == 1 {
            return normalize_root(&dirs[0]);
        }
    }
    temp_root.to_path_buf()
}

fn unique_temp_dir(prefix: &str) -> anyhow::Result<PathBuf> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!("{prefix}-{}-{now}", std::process::id())))
}

fn push_candidate(candidates: &mut Vec<String>, path: PathBuf) {
    let rendered = path.display().to_string();
    if !candidates.iter().any(|candidate| candidate == &rendered) {
        candidates.push(rendered);
    }
}

fn found(requested_path: &str, resolved_path: PathBuf, candidates: Vec<String>) -> AssetResolution {
    AssetResolution {
        requested_path: requested_path.to_string(),
        resolved_path: Some(resolved_path.display().to_string()),
        exists: true,
        candidates,
    }
}

fn find_by_file_name(root: &Path, file_name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .map(|name| name.to_string_lossy() == file_name)
            .unwrap_or(false)
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_by_file_name(&path, file_name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}
