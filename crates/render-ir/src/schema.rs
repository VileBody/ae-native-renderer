#![allow(non_snake_case)]

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
    #[serde(default)]
    pub motion_blur: MotionBlurSettings,
}

fn default_background() -> [u8; 4] {
    [0, 0, 0, 0]
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MotionBlurSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_motion_blur_samples")]
    pub samples: u32,
    #[serde(default = "default_shutter_angle")]
    pub shutter_angle: f64,
    #[serde(default = "default_shutter_phase")]
    pub shutter_phase: f64,
}

impl Default for MotionBlurSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            samples: default_motion_blur_samples(),
            shutter_angle: default_shutter_angle(),
            shutter_phase: default_shutter_phase(),
        }
    }
}

pub const DEFAULT_MOTION_BLUR_SAMPLES: u32 = 17;

fn default_motion_blur_samples() -> u32 {
    DEFAULT_MOTION_BLUR_SAMPLES
}

fn default_shutter_angle() -> f64 {
    180.0
}

fn default_shutter_phase() -> f64 {
    -90.0
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
    pub motion_blur: bool,
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
            motion_blur: false,
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
    #[serde(default)]
    pub ease: Option<KeyframeEase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalarKeyframe {
    pub time: f64,
    pub value: f32,
    #[serde(default)]
    pub hold: bool,
    #[serde(default)]
    pub approximate: bool,
    #[serde(default)]
    pub ease: Option<KeyframeEase>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KeyframeEase {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectSpec {
    pub match_name: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

pub const TEXT_PAINT_MATCH_NAME: &str = "ANR Text Paint";

/// Optional text paint extension carried in `Layer::Text.effects`, either as
/// a standalone marker or in an effect's `params.text_paint` field.
///
/// Keeping paint as a typed marker effect preserves the v1 `fill: [u8; 4]`
/// wire shape while allowing one text layer to render fill and stroke together.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextPaintSpec {
    #[serde(default)]
    pub fill: Option<[u8; 4]>,
    /// `None` historically meant "use Layer::Text.fill". Keep that wire
    /// behavior by default, but let imported AE TextDocument data explicitly
    /// disable the fill for outline-only layers.
    #[serde(default = "default_text_paint_fill_enabled")]
    pub fill_enabled: bool,
    #[serde(default)]
    pub stroke_color: Option<[u8; 4]>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default)]
    pub stroke_over_fill: bool,
}

fn default_text_paint_fill_enabled() -> bool {
    true
}

impl TextPaintSpec {
    pub fn from_effect(effect: &EffectSpec) -> Option<Self> {
        let value = if effect.match_name == TEXT_PAINT_MATCH_NAME {
            &effect.params
        } else {
            effect.params.get("text_paint")?
        };
        serde_json::from_value(value.clone()).ok()
    }
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
    #[serde(default)]
    pub shape: TextSelectorShape,
    #[serde(default)]
    pub randomize_order: bool,
    #[serde(default)]
    pub wiggly: Option<TextWigglySelector>,
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
            shape: TextSelectorShape::default(),
            randomize_order: false,
            wiggly: None,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSelectorShape {
    #[default]
    Square,
    RampUp,
    RampDown,
    Triangle,
    Round,
    Smooth,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextWigglySelector {
    pub amount: f32,
    pub frequency: f32,
    #[serde(default)]
    pub seed: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextJustification {
    Left,
    #[default]
    Center,
    Full,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Screen,
    Difference,
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
    /// A text animator Tracking Amount property. This property is evaluated
    /// before layout/raster; the enclosing range selector remains available
    /// for future per-range tracking support.
    #[serde(rename = "tracking_amount")]
    TrackingAmount {
        #[serde(default)]
        value: f32,
        #[serde(default)]
        keyframes: Vec<ScalarKeyframe>,
    },
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Layer {
    #[serde(rename = "solid")]
    Solid {
        id: String,
        start: f64,
        duration: f64,
        #[serde(default)]
        blend_mode: BlendMode,
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
        blend_mode: BlendMode,
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
        #[allow(non_snake_case)]
        fontSize: f32,
        /// Per-character TextDocument overrides recovered from After Effects.
        /// Indices are Unicode scalar indices in the layer text, including explicit newlines.
        #[serde(default)]
        char_styles: Vec<TextCharStyle>,
        #[serde(default)]
        blend_mode: BlendMode,
        /// After Effects tracking units (1/1000 em).
        #[serde(default)]
        tracking: f32,
        /// Explicit After Effects line spacing in layer pixels.
        #[serde(default)]
        leading: Option<f32>,
        /// Center the text vertically from its AE-style source rect instead of line boxes.
        #[serde(default)]
        center_source_rect_y: bool,
        #[serde(default)]
        justification: TextJustification,
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
        blend_mode: BlendMode,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextCharStyle {
    pub index: usize,
    /// Sparse TextDocument font override keyed by Unicode scalar index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
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
            Layer::Solid {
                start, duration, ..
            }
            | Layer::Footage {
                start, duration, ..
            }
            | Layer::Text {
                start, duration, ..
            }
            | Layer::Precomp {
                start, duration, ..
            } => (*start, *duration),
            Layer::Adjustment {
                start, duration, ..
            } => (*start, *duration),
        }
    }

    pub fn blend_mode(&self) -> BlendMode {
        match self {
            Layer::Solid { blend_mode, .. }
            | Layer::Footage { blend_mode, .. }
            | Layer::Text { blend_mode, .. }
            | Layer::Precomp { blend_mode, .. } => *blend_mode,
            Layer::Adjustment { .. } => BlendMode::Normal,
        }
    }

    pub fn is_active(&self, time: f64) -> bool {
        let (start, duration) = self.time_range();
        time >= start && time < start + duration
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BlendMode, Layer, ScalarKeyframe, TextAnimatorSpec, TextExpressionSelector, TextPaintSpec,
        TEXT_PAINT_MATCH_NAME,
    };

    fn text_layer_json() -> serde_json::Value {
        serde_json::json!({
            "type": "text",
            "id": "text",
            "start": 0.0,
            "duration": 1.0,
            "text": "TRACK",
            "font": "default",
            "fontSize": 100.0,
            "fill": [255, 255, 255, 255],
            "box": null
        })
    }

    #[test]
    fn text_tracking_defaults_to_zero_and_round_trips_ae_units() {
        let layer: Layer = serde_json::from_value(text_layer_json()).unwrap();
        let Layer::Text {
            tracking,
            blend_mode,
            ..
        } = layer
        else {
            panic!("expected text layer");
        };
        assert_eq!(tracking, 0.0);
        assert_eq!(blend_mode, BlendMode::Normal);

        let mut json = text_layer_json();
        json["tracking"] = serde_json::json!(-55.0);
        let layer: Layer = serde_json::from_value(json).unwrap();
        let Layer::Text {
            tracking,
            leading,
            center_source_rect_y,
            justification,
            ..
        } = &layer
        else {
            panic!("expected text layer");
        };
        assert_eq!(*tracking, -55.0);
        assert_eq!(*leading, None);
        assert!(!center_source_rect_y);
        assert_eq!(*justification, super::TextJustification::Center);
        assert_eq!(serde_json::to_value(layer).unwrap()["tracking"], -55.0);
    }

    #[test]
    fn legacy_fill_json_and_typed_text_paint_coexist() {
        let mut json = text_layer_json();
        json["effects"] = serde_json::json!([{
            "match_name": TEXT_PAINT_MATCH_NAME,
            "params": {
                "stroke_color": [0, 0, 0, 255],
                "stroke_width": 5.0,
                "stroke_over_fill": false
            }
        }]);
        let layer: Layer = serde_json::from_value(json).unwrap();
        let Layer::Text { fill, effects, .. } = &layer else {
            panic!("expected text layer");
        };
        assert_eq!(*fill, [255, 255, 255, 255]);
        let paint = effects
            .iter()
            .find_map(TextPaintSpec::from_effect)
            .expect("text paint marker");
        assert_eq!(paint.fill, None);
        assert_eq!(paint.stroke_color, Some([0, 0, 0, 255]));
        assert_eq!(paint.stroke_width, 5.0);
        assert!(!paint.stroke_over_fill);
    }

    #[test]
    fn tracking_amount_animator_round_trips_keyframes() {
        let animator = TextAnimatorSpec {
            name: "tracking".to_string(),
            expression_selector: Some(TextExpressionSelector::TrackingAmount {
                value: 7.0,
                keyframes: vec![ScalarKeyframe {
                    time: 1.0,
                    value: -1.0,
                    hold: false,
                    approximate: false,
                    ease: None,
                }],
            }),
            ..TextAnimatorSpec::default()
        };
        let json = serde_json::to_value(&animator).unwrap();
        assert_eq!(json["expression_selector"]["type"], "tracking_amount");
        let decoded: TextAnimatorSpec = serde_json::from_value(json).unwrap();
        let Some(TextExpressionSelector::TrackingAmount { value, keyframes }) =
            decoded.expression_selector
        else {
            panic!("expected tracking amount");
        };
        assert_eq!(value, 7.0);
        assert_eq!(keyframes[0].value, -1.0);
    }

    #[test]
    fn blend_mode_defaults_and_round_trips_for_non_adjustment_layers() {
        let layers = [
            text_layer_json(),
            serde_json::json!({
                "type": "solid",
                "id": "solid",
                "start": 0.0,
                "duration": 1.0,
                "color": [255, 255, 255, 255],
                "rect": {"x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0}
            }),
            serde_json::json!({
                "type": "footage",
                "id": "footage",
                "start": 0.0,
                "duration": 1.0,
                "source": "asset"
            }),
            serde_json::json!({
                "type": "precomp",
                "id": "precomp",
                "start": 0.0,
                "duration": 1.0,
                "composition": "child"
            }),
        ];

        for value in layers {
            let layer: Layer = serde_json::from_value(value).unwrap();
            assert_eq!(layer.blend_mode(), BlendMode::Normal);
        }

        let mut value = text_layer_json();
        value["blend_mode"] = serde_json::json!("difference");
        let layer: Layer = serde_json::from_value(value).unwrap();
        assert_eq!(layer.blend_mode(), BlendMode::Difference);
        assert_eq!(
            serde_json::to_value(layer).unwrap()["blend_mode"],
            "difference"
        );
        assert_eq!(serde_json::to_value(BlendMode::Add).unwrap(), "add");
        assert_eq!(serde_json::to_value(BlendMode::Screen).unwrap(), "screen");
    }
}
