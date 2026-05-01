use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub version: String,
    pub composition: Composition,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default)]
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Composition {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    #[serde(default = "default_background")]
    pub background: [u8; 4],
}

fn default_background() -> [u8; 4] {
    [0, 0, 0, 0]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: AssetKind,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Video,
    Audio,
    Image,
    Font,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform2D {
    pub anchor: [f32; 2],
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub opacity: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            anchor: [0.0, 0.0],
            position: [0.0, 0.0],
            scale: [100.0, 100.0],
            rotation: 0.0,
            opacity: 100.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectSpec {
    pub match_name: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Layer {
    #[serde(rename = "solid")]
    Solid {
        id: String,
        start: f64,
        duration: f64,
        color: [u8; 4],
        rect: Rect,
        #[serde(default)]
        transform: Transform2D,
        #[serde(default)]
        effects: Vec<EffectSpec>,
    },
    #[serde(rename = "footage")]
    Footage {
        id: String,
        start: f64,
        duration: f64,
        source: String,
        #[serde(default)]
        source_start: f64,
        #[serde(default)]
        transform: Transform2D,
        #[serde(default)]
        effects: Vec<EffectSpec>,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        start: f64,
        duration: f64,
        text: String,
        font: String,
        fontSize: f32,
        fill: [u8; 4],
        #[serde(rename = "box")]
        box_: Option<Rect>,
        #[serde(default)]
        transform: Transform2D,
        #[serde(default)]
        effects: Vec<EffectSpec>,
    },
    #[serde(rename = "precomp")]
    Precomp {
        id: String,
        start: f64,
        duration: f64,
        composition: String,
        #[serde(default)]
        collapse_transformations: bool,
        #[serde(default)]
        transform: Transform2D,
        #[serde(default)]
        effects: Vec<EffectSpec>,
    },
    #[serde(rename = "adjustment")]
    Adjustment {
        id: String,
        start: f64,
        duration: f64,
        #[serde(default)]
        effects: Vec<EffectSpec>,
    },
}

impl Layer {
    pub fn id(&self) -> &str {
        match self {
            Layer::Solid { id, .. }
            | Layer::Footage { id, .. }
            | Layer::Text { id, .. }
            | Layer::Precomp { id, .. }
            | Layer::Adjustment { id, .. } => id,
        }
    }

    pub fn time_range(&self) -> (f64, f64) {
        match self {
            Layer::Solid { start, duration, .. }
            | Layer::Footage { start, duration, .. }
            | Layer::Text { start, duration, .. }
            | Layer::Precomp { start, duration, .. } => (*start, *duration),
            Layer::Adjustment { start, duration, .. } => (*start, *duration),
        }
    }

    pub fn is_active(&self, time: f64) -> bool {
        let (start, duration) = self.time_range();
        time >= start && time < start + duration
    }
}
