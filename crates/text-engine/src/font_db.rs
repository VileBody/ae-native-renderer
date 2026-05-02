use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default, Clone)]
pub struct FontDb {
    fonts: HashMap<String, PathBuf>,
}

impl FontDb {
    pub fn register(&mut self, id: impl Into<String>, path: impl Into<PathBuf>) {
        self.fonts.insert(id.into(), path.into());
    }

    pub fn get(&self, id: &str) -> Option<&PathBuf> {
        self.fonts.get(id)
    }
}

pub fn resolve_font_path(font_id: &str) -> Option<PathBuf> {
    let direct = Path::new(font_id);
    if direct.exists() {
        return Some(direct.to_path_buf());
    }

    if let Ok(output) = Command::new("fc-match")
        .arg("-f")
        .arg("%{file}")
        .arg(font_id)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && Path::new(&path).exists() {
                return Some(PathBuf::from(path));
            }
        }
    }

    common_font_paths()
        .into_iter()
        .find(|path| path.exists())
}

fn common_font_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
        PathBuf::from("/System/Library/Fonts/SFNS.ttf"),
        PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"),
        PathBuf::from("/Library/Fonts/Arial.ttf"),
    ]
}
