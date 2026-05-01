use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaProbe {
    pub path: String,
    pub kind: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub duration: Option<f64>,
}

pub fn probe(path: impl AsRef<Path>) -> anyhow::Result<MediaProbe> {
    // TODO: use discoverer or decodebin to extract accurate metadata.
    let path = path.as_ref();
    Ok(MediaProbe {
        path: path.display().to_string(),
        kind: "unknown-stub".to_string(),
        width: None,
        height: None,
        fps: None,
        duration: None,
    })
}
