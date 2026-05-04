use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorKind {
    Source,
    PropertyEvaluator,
    Geometry,
    TextLayout,
    TextAnimator,
    PointwisePixel,
    SpatialNeighborhood,
    CoordinateWarp,
    Temporal,
    Graph,
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Pointwise,
    Spatial,
    Coordinate,
    Temporal,
    Glyph,
    Graph,
    Procedural,
    AnimatedParams,
    Alpha,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Discontinuity {
    Threshold,
    EdgeSampling,
    SamplerRounding,
    IntegerRadius,
    KeyframeBoundary,
    PosterizeBoundary,
    SelectorBoundary,
    GlyphSegmentationBoundary,
    LineWrapBoundary,
    ComplexityOctaveBoundary,
    SeedChange,
    GraphOrder,
    PremultAlphaBoundary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBudget {
    pub max_abs_rgba8: u8,
    pub mean_abs_rgba8: f64,
    pub rmse_rgba8: f64,
    pub max_uv_error_px: f64,
    pub max_time_error_seconds: f64,
    pub max_scalar_error: f64,
}

impl ErrorBudget {
    pub fn exact() -> Self {
        Self {
            max_abs_rgba8: 0,
            mean_abs_rgba8: 0.0,
            rmse_rgba8: 0.0,
            max_uv_error_px: 0.0,
            max_time_error_seconds: 0.0,
            max_scalar_error: 0.0,
        }
    }

    pub fn strict_rounding() -> Self {
        Self {
            max_abs_rgba8: 1,
            mean_abs_rgba8: 0.05,
            rmse_rgba8: 0.10,
            max_uv_error_px: 1e-4,
            max_time_error_seconds: 1e-9,
            max_scalar_error: 1e-6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterSpec {
    pub name: String,
    pub unit: Option<String>,
    pub animated: bool,
    pub important_values: Vec<f64>,
    pub boundary_values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorPassport {
    pub match_name: String,
    pub display_name: String,
    pub kind: OperatorKind,
    pub dimensions: Vec<Dimension>,
    pub discontinuities: Vec<Discontinuity>,
    pub parameters: Vec<ParameterSpec>,
    pub invariants: Vec<String>,
    pub debug_outputs: Vec<String>,
    pub budget: ErrorBudget,
}

impl OperatorPassport {
    pub fn has_dimension(&self, dimension: Dimension) -> bool {
        self.dimensions.contains(&dimension)
    }

    pub fn has_discontinuity(&self, discontinuity: Discontinuity) -> bool {
        self.discontinuities.contains(&discontinuity)
    }
}

fn parameter(
    name: &str,
    unit: &str,
    animated: bool,
    important_values: &[f64],
    boundary_values: &[f64],
) -> ParameterSpec {
    ParameterSpec {
        name: name.to_string(),
        unit: (!unit.is_empty()).then(|| unit.to_string()),
        animated,
        important_values: important_values.to_vec(),
        boundary_values: boundary_values.to_vec(),
    }
}

pub fn builtin_passport(match_name: &str) -> Option<OperatorPassport> {
    use Dimension as Dim;
    use Discontinuity as Disc;
    use OperatorKind as Kind;

    match match_name {
        "ADBE Transform Group" | "ADBE Geometry2" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: if match_name == "ADBE Geometry2" {
                "Transform effect"
            } else {
                "Layer Transform"
            }
            .to_string(),
            kind: Kind::Geometry,
            dimensions: vec![Dim::Coordinate, Dim::AnimatedParams, Dim::Alpha],
            discontinuities: vec![
                Disc::SamplerRounding,
                Disc::EdgeSampling,
                Disc::KeyframeBoundary,
            ],
            parameters: vec![
                parameter("anchor", "px", true, &[0.0, 0.5, 100.0], &[]),
                parameter("position", "px", true, &[0.0, 0.5, 100.0], &[]),
                parameter(
                    "scale",
                    "%",
                    true,
                    &[0.0, 50.0, 100.0, -100.0],
                    &[0.0, 100.0],
                ),
                parameter("rotation", "deg", true, &[0.0, 45.0, 90.0, 180.0], &[0.0]),
                parameter("opacity", "%", true, &[0.0, 50.0, 100.0], &[0.0, 100.0]),
            ],
            invariants: vec![
                "identity matrix -> same coordinate field".to_string(),
                "opacity 0 -> transparent contribution".to_string(),
            ],
            debug_outputs: vec![
                "matrix".to_string(),
                "inverse_matrix".to_string(),
                "sample_uv".to_string(),
                "bbox".to_string(),
            ],
            budget: ErrorBudget::strict_rounding(),
        }),
        "ADBE Box Blur2" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: "Fast Box Blur".to_string(),
            kind: Kind::SpatialNeighborhood,
            dimensions: vec![Dim::Spatial, Dim::Alpha, Dim::Color, Dim::AnimatedParams],
            discontinuities: vec![
                Disc::IntegerRadius,
                Disc::EdgeSampling,
                Disc::KeyframeBoundary,
                Disc::PremultAlphaBoundary,
            ],
            parameters: vec![
                parameter("radius", "px", true, &[0.0, 1.0, 5.0, 20.0], &[0.0, 1.0]),
                parameter("iterations", "count", false, &[1.0, 2.0, 3.0], &[1.0]),
            ],
            invariants: vec![
                "radius 0 -> identity".to_string(),
                "constant image -> same constant image".to_string(),
                "transparent input -> transparent output".to_string(),
            ],
            debug_outputs: vec![
                "horizontal_pass".to_string(),
                "vertical_pass".to_string(),
                "kernel".to_string(),
                "edge_samples".to_string(),
            ],
            budget: ErrorBudget::strict_rounding(),
        }),
        "ADBE Drop Shadow" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: "Drop Shadow".to_string(),
            kind: Kind::SpatialNeighborhood,
            dimensions: vec![
                Dim::Spatial,
                Dim::Coordinate,
                Dim::Alpha,
                Dim::Color,
                Dim::AnimatedParams,
            ],
            discontinuities: vec![
                Disc::IntegerRadius,
                Disc::EdgeSampling,
                Disc::KeyframeBoundary,
                Disc::PremultAlphaBoundary,
            ],
            parameters: vec![
                parameter("opacity", "%", true, &[0.0, 50.0, 100.0], &[0.0, 100.0]),
                parameter("direction", "deg", true, &[0.0, 45.0, 90.0, 180.0], &[0.0]),
                parameter("distance", "px", true, &[0.0, 1.0, 10.0, 50.0], &[0.0]),
                parameter("softness", "px", true, &[0.0, 1.0, 10.0, 30.0], &[0.0, 1.0]),
            ],
            invariants: vec![
                "shadow opacity 0 -> source unchanged".to_string(),
                "transparent source -> transparent output".to_string(),
            ],
            debug_outputs: vec![
                "source_alpha".to_string(),
                "shadow_mask".to_string(),
                "blurred_shadow".to_string(),
                "offset_shadow".to_string(),
                "final_composite".to_string(),
            ],
            budget: ErrorBudget::strict_rounding(),
        }),
        "ADBE Glo2" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: "Glow".to_string(),
            kind: Kind::SpatialNeighborhood,
            dimensions: vec![
                Dim::Pointwise,
                Dim::Spatial,
                Dim::Alpha,
                Dim::Color,
                Dim::AnimatedParams,
            ],
            discontinuities: vec![
                Disc::Threshold,
                Disc::IntegerRadius,
                Disc::EdgeSampling,
                Disc::PremultAlphaBoundary,
                Disc::KeyframeBoundary,
            ],
            parameters: vec![
                parameter("threshold", "normalized", true, &[0.0, 0.5, 1.0], &[0.5]),
                parameter("radius", "px", true, &[0.0, 1.0, 10.0, 40.0], &[0.0, 1.0]),
                parameter(
                    "intensity",
                    "multiplier",
                    true,
                    &[0.0, 0.5, 1.0, 2.0],
                    &[0.0],
                ),
            ],
            invariants: vec![
                "intensity 0 -> source unchanged".to_string(),
                "constant below threshold -> no glow contribution".to_string(),
            ],
            debug_outputs: vec![
                "luma".to_string(),
                "threshold_mask".to_string(),
                "blurred_glow".to_string(),
                "glow_contribution".to_string(),
                "final_composite".to_string(),
            ],
            budget: ErrorBudget::strict_rounding(),
        }),
        "ADBE Minimax" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: "Minimax".to_string(),
            kind: Kind::SpatialNeighborhood,
            dimensions: vec![Dim::Spatial, Dim::Alpha, Dim::Color, Dim::AnimatedParams],
            discontinuities: vec![Disc::IntegerRadius, Disc::EdgeSampling],
            parameters: vec![
                parameter("radius", "px", true, &[0.0, 1.0, 5.0, 20.0], &[0.0, 1.0]),
                parameter("operation", "enum", false, &[0.0, 1.0, 2.0, 3.0], &[]),
            ],
            invariants: vec![
                "radius 0 -> identity".to_string(),
                "constant image -> same constant image".to_string(),
            ],
            debug_outputs: vec![
                "neighborhood".to_string(),
                "channel_minmax".to_string(),
                "output_mask".to_string(),
            ],
            budget: ErrorBudget::strict_rounding(),
        }),
        "ADBE Posterize Time" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: "Posterize Time".to_string(),
            kind: Kind::Temporal,
            dimensions: vec![Dim::Temporal, Dim::AnimatedParams],
            discontinuities: vec![Disc::PosterizeBoundary, Disc::KeyframeBoundary],
            parameters: vec![parameter(
                "frame_rate",
                "fps",
                true,
                &[1.0, 2.0, 6.0, 12.0, 24.0, 30.0],
                &[1.0],
            )],
            invariants: vec![
                "posterize fps equal comp fps -> same frame mapping".to_string(),
                "posterize is idempotent for same fps".to_string(),
            ],
            debug_outputs: vec![
                "input_time".to_string(),
                "quantized_time".to_string(),
                "source_frame_index".to_string(),
            ],
            budget: ErrorBudget::exact(),
        }),
        "ADBE Turbulent Displace" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: "Turbulent Displace".to_string(),
            kind: Kind::CoordinateWarp,
            dimensions: vec![
                Dim::Coordinate,
                Dim::Spatial,
                Dim::Temporal,
                Dim::Procedural,
                Dim::AnimatedParams,
                Dim::Alpha,
                Dim::Color,
            ],
            discontinuities: vec![
                Disc::EdgeSampling,
                Disc::SamplerRounding,
                Disc::ComplexityOctaveBoundary,
                Disc::SeedChange,
                Disc::KeyframeBoundary,
            ],
            parameters: vec![
                parameter("amount", "px", true, &[0.0, 1.0, 10.0, 50.0], &[0.0]),
                parameter("size", "px", true, &[1.0, 10.0, 50.0, 200.0], &[1.0]),
                parameter(
                    "complexity",
                    "octaves",
                    true,
                    &[1.0, 2.0, 3.0, 6.0],
                    &[1.0, 2.0],
                ),
                parameter(
                    "evolution",
                    "deg",
                    true,
                    &[0.0, 45.0, 180.0, 360.0],
                    &[0.0, 360.0],
                ),
            ],
            invariants: vec![
                "amount 0 -> identity".to_string(),
                "constant source image -> same constant image".to_string(),
                "same seed and params -> same displacement field".to_string(),
            ],
            debug_outputs: vec![
                "noise".to_string(),
                "dx".to_string(),
                "dy".to_string(),
                "uv".to_string(),
                "sampled_source".to_string(),
                "final".to_string(),
            ],
            budget: ErrorBudget {
                max_uv_error_px: 0.01,
                ..ErrorBudget::strict_rounding()
            },
        }),
        "ADBE Text Animator" | "ADBE Text Selector" | "ADBE Text Expressible Selector" => {
            Some(OperatorPassport {
                match_name: match_name.to_string(),
                display_name: "Text Animator / Selector".to_string(),
                kind: Kind::TextAnimator,
                dimensions: vec![
                    Dim::Glyph,
                    Dim::Temporal,
                    Dim::AnimatedParams,
                    Dim::Coordinate,
                    Dim::Alpha,
                ],
                discontinuities: vec![
                    Disc::SelectorBoundary,
                    Disc::GlyphSegmentationBoundary,
                    Disc::LineWrapBoundary,
                    Disc::KeyframeBoundary,
                ],
                parameters: vec![
                    parameter("start", "%", true, &[0.0, 50.0, 100.0], &[0.0, 100.0]),
                    parameter("end", "%", true, &[0.0, 50.0, 100.0], &[0.0, 100.0]),
                    parameter("offset", "%", true, &[-100.0, 0.0, 100.0], &[0.0]),
                    parameter("smoothness", "%", false, &[0.0, 100.0], &[0.0, 100.0]),
                ],
                invariants: vec![
                    "animator amount 0 -> original glyph layout".to_string(),
                    "selector outside glyph range -> no glyph contribution".to_string(),
                ],
                debug_outputs: vec![
                    "glyph_layout".to_string(),
                    "word_segments".to_string(),
                    "line_segments".to_string(),
                    "selector_weights".to_string(),
                    "glyph_matrices".to_string(),
                    "glyph_opacity".to_string(),
                ],
                budget: ErrorBudget::strict_rounding(),
            })
        }
        "ADBE Motion Blur" | "Layer Motion Blur" => Some(OperatorPassport {
            match_name: match_name.to_string(),
            display_name: "Layer Motion Blur".to_string(),
            kind: Kind::Temporal,
            dimensions: vec![
                Dim::Temporal,
                Dim::Graph,
                Dim::AnimatedParams,
                Dim::Coordinate,
                Dim::Glyph,
            ],
            discontinuities: vec![
                Disc::KeyframeBoundary,
                Disc::PosterizeBoundary,
                Disc::GraphOrder,
            ],
            parameters: vec![
                parameter(
                    "samples_per_frame",
                    "count",
                    false,
                    &[1.0, 2.0, 8.0, 16.0],
                    &[1.0],
                ),
                parameter(
                    "shutter_angle",
                    "deg",
                    false,
                    &[0.0, 90.0, 180.0, 360.0],
                    &[0.0],
                ),
                parameter("shutter_phase", "deg", false, &[-180.0, -90.0, 0.0], &[0.0]),
            ],
            invariants: vec![
                "static layer -> same as no motion blur".to_string(),
                "shutter angle 0 -> same as no motion blur".to_string(),
            ],
            debug_outputs: vec![
                "sample_times".to_string(),
                "sample_weights".to_string(),
                "per_sample_matrix".to_string(),
                "per_sample_bbox".to_string(),
                "accumulated_buffer".to_string(),
            ],
            budget: ErrorBudget::exact(),
        }),
        _ => None,
    }
}

pub fn builtin_passports() -> Vec<OperatorPassport> {
    [
        "ADBE Transform Group",
        "ADBE Geometry2",
        "ADBE Box Blur2",
        "ADBE Drop Shadow",
        "ADBE Glo2",
        "ADBE Minimax",
        "ADBE Posterize Time",
        "ADBE Turbulent Displace",
        "ADBE Text Animator",
        "ADBE Motion Blur",
    ]
    .into_iter()
    .filter_map(builtin_passport)
    .collect()
}
