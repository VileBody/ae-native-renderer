use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub version: String,
    pub composition: Composition,
    #[serde(default)]
    pub compositions: Vec<CompositionNode>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default)]
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionNode {
    pub composition: Composition,
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
    #[serde(default)]
    pub animation: Transform2DAnimation,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            anchor: [0.0, 0.0],
            position: [0.0, 0.0],
            scale: [100.0, 100.0],
            rotation: 0.0,
            opacity: 100.0,
            animation: Transform2DAnimation::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transform2DAnimation {
    #[serde(default)]
    pub position: Vec<Vec2Keyframe>,
    #[serde(default)]
    pub scale: Vec<Vec2Keyframe>,
    #[serde(default)]
    pub opacity: Vec<ScalarKeyframe>,
    #[serde(default)]
    pub reveal: Vec<ScalarKeyframe>,
    #[serde(default)]
    pub expression: Transform2DExpression,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transform2DExpression {
    #[serde(default)]
    pub position: Option<PositionExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PositionExpression {
    #[serde(rename = "edge_wobble")]
    EdgeWobble {
        intro: f32,
        outro: f32,
        amp: f32,
        freq: f32,
        #[serde(default)]
        source: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vec2Keyframe {
    pub time: f64,
    pub value: [f32; 2],
    #[serde(default)]
    pub hold: bool,
    #[serde(default)]
    pub approximate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalarKeyframe {
    pub time: f64,
    pub value: f32,
    #[serde(default)]
    pub hold: bool,
    #[serde(default)]
    pub approximate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectSpec {
    pub match_name: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TextAnimatorSpec {
    pub name: String,
    #[serde(default = "default_animator_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub position: Option<[f32; 2]>,
    #[serde(default)]
    pub scale: Option<[f32; 2]>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub blur: Option<[f32; 2]>,
    #[serde(default)]
    pub selector: TextRangeSelector,
    #[serde(default)]
    pub expression_selector: Option<TextExpressionSelector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRangeSelector {
    #[serde(default)]
    pub start: f32,
    #[serde(default = "default_selector_end")]
    pub end: f32,
    #[serde(default)]
    pub start_keyframes: Vec<ScalarKeyframe>,
    #[serde(default)]
    pub end_keyframes: Vec<ScalarKeyframe>,
    #[serde(default)]
    pub based_on: TextSelectorBasedOn,
    #[serde(default)]
    pub smoothness: f32,
}

impl Default for TextRangeSelector {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 100.0,
            start_keyframes: Vec::new(),
            end_keyframes: Vec::new(),
            based_on: TextSelectorBasedOn::default(),
            smoothness: 100.0,
        }
    }
}

fn default_selector_end() -> f32 {
    100.0
}

fn default_animator_opacity() -> f32 {
    100.0
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSelectorBasedOn {
    #[default]
    Characters,
    Words,
    Lines,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TextExpressionSelector {
    #[serde(rename = "per_character_bounce")]
    PerCharacterBounce {
        delay: f32,
        freq: f32,
        amplitude: f32,
        decay: f32,
        #[serde(default)]
        source: String,
    },
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
        text_animators: Vec<TextAnimatorSpec>,
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
