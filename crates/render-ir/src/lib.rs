pub mod schema;

pub use schema::*;

use std::fs;
use std::path::Path;

pub fn load_scene(path: impl AsRef<Path>) -> anyhow::Result<Scene> {
    let raw = fs::read_to_string(path.as_ref())?;
    let scene: Scene = serde_json::from_str(&raw)?;
    Ok(scene)
}
