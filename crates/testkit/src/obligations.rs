use serde::{Deserialize, Serialize};

use crate::operator::{Dimension, Discontinuity, OperatorPassport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationKind {
    AlgebraicUnit,
    IdentityInvariant,
    BasisImage,
    CoordinateField,
    Boundary,
    DecompositionDebug,
    TemporalSampling,
    Interpolation,
    ProceduralDeterminism,
    GlyphLayout,
    GraphOrder,
    SequenceRegression,
    AeGoldenConformance,
    ReferenceConformance,
    ReleaseRegression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestObligation {
    pub kind: ObligationKind,
    pub reason: String,
    pub required_fixtures: Vec<String>,
    pub required_debug_outputs: Vec<String>,
    pub priority: u8,
}

impl TestObligation {
    pub fn new(kind: ObligationKind, reason: impl Into<String>, priority: u8) -> Self {
        Self {
            kind,
            reason: reason.into(),
            required_fixtures: Vec::new(),
            required_debug_outputs: Vec::new(),
            priority,
        }
    }

    pub fn fixtures(mut self, values: &[&str]) -> Self {
        self.required_fixtures = values.iter().map(|value| (*value).to_string()).collect();
        self
    }

    pub fn debug_outputs(mut self, values: &[String]) -> Self {
        self.required_debug_outputs = values.to_vec();
        self
    }
}

pub fn derive_obligations(passport: &OperatorPassport) -> Vec<TestObligation> {
    use Dimension::*;
    use ObligationKind::*;

    let mut obligations = vec![
        TestObligation::new(IdentityInvariant, "declared invariants must hold", 1).fixtures(&[
            "solid",
            "transparent",
            "impulse",
        ]),
        TestObligation::new(
            AeGoldenConformance,
            "operator must be compared to AE/reference golden outputs",
            2,
        ),
        TestObligation::new(
            ReleaseRegression,
            "operator must not drift versus last accepted release",
            2,
        ),
        TestObligation::new(
            ReferenceConformance,
            "optimized implementation must match deterministic reference",
            1,
        ),
    ];

    if passport.has_dimension(Pointwise)
        || passport.has_dimension(Alpha)
        || passport.has_dimension(Color)
    {
        obligations.push(
            TestObligation::new(
                BasisImage,
                "pointwise/color/alpha behavior must be tested on basis fields",
                2,
            )
            .fixtures(&["solid", "alpha_ramp", "checkerboard", "color_ramp"]),
        );
    }
    if passport.has_dimension(Spatial) {
        obligations.push(
            TestObligation::new(
                BasisImage,
                "spatial operators require impulse response and edge tests",
                1,
            )
            .fixtures(&[
                "impulse",
                "thin_line",
                "checkerboard",
                "near_edge_square",
                "alpha_ramp",
            ]),
        );
    }
    if passport.has_dimension(Coordinate) {
        obligations.push(
            TestObligation::new(
                CoordinateField,
                "coordinate operators require coordinate-field/UV tests",
                1,
            )
            .fixtures(&[
                "coordinate_field",
                "grid",
                "single_dot",
                "horizontal_ramp",
                "vertical_ramp",
            ]),
        );
    }
    if passport.has_dimension(Temporal) || passport.has_dimension(AnimatedParams) {
        obligations.push(
            TestObligation::new(
                TemporalSampling,
                "temporal operators require frame/subframe/keyframe timing tests",
                1,
            )
            .fixtures(&[
                "numbered_frames",
                "moving_square",
                "animated_param_boundaries",
            ]),
        );
        obligations.push(TestObligation::new(
            SequenceRegression,
            "video is tested as deterministic frame sequence",
            2,
        ));
    }
    if passport.has_dimension(AnimatedParams) {
        obligations.push(
            TestObligation::new(
                Interpolation,
                "animated parameters require hold/linear/bezier/keyframe-boundary probes",
                1,
            )
            .fixtures(&["animated_param_boundaries"]),
        );
    }
    if passport.has_dimension(Procedural) {
        obligations.push(
            TestObligation::new(
                ProceduralDeterminism,
                "procedural/noise operators must be deterministic and expose fields",
                1,
            )
            .fixtures(&["coordinate_field", "constant", "grid"])
            .debug_outputs(&passport.debug_outputs),
        );
    }
    if passport.has_dimension(Glyph) {
        obligations.push(
            TestObligation::new(
                GlyphLayout,
                "glyph operators must expose layout/selector/matrix telemetry",
                1,
            )
            .fixtures(&[
                "text_single_glyph",
                "text_words",
                "text_multiline_box",
                "text_boundary_spaces",
            ])
            .debug_outputs(&passport.debug_outputs),
        );
    }
    if passport.has_dimension(Graph) {
        obligations.push(
            TestObligation::new(
                GraphOrder,
                "graph operators require evaluation-order and non-commuting tests",
                1,
            )
            .fixtures(&[
                "nested_precomp",
                "adjustment_layer",
                "motion_blur_layer_on_off",
            ])
            .debug_outputs(&passport.debug_outputs),
        );
    }
    if !passport.debug_outputs.is_empty() {
        obligations.push(
            TestObligation::new(
                DecompositionDebug,
                "operator must expose intermediate checkpoints for first-divergence search",
                1,
            )
            .debug_outputs(&passport.debug_outputs),
        );
    }

    for discontinuity in &passport.discontinuities {
        let reason = match discontinuity {
            Discontinuity::Threshold => "threshold requires below/exact/above epsilon probes",
            Discontinuity::EdgeSampling => "edge sampling requires near-edge/out-of-bounds probes",
            Discontinuity::SamplerRounding => "sampler rounding requires subpixel probes",
            Discontinuity::IntegerRadius => "integer radius requires r-eps/r/r+eps probes",
            Discontinuity::KeyframeBoundary => "keyframe requires t-eps/t/t+eps probes",
            Discontinuity::PosterizeBoundary => "posterize requires frame-boundary probes",
            Discontinuity::SelectorBoundary => "selector requires glyph-boundary probes",
            Discontinuity::GlyphSegmentationBoundary => {
                "glyph segmentation requires spaces/word-boundary probes"
            }
            Discontinuity::LineWrapBoundary => "line wrapping requires box-width boundary probes",
            Discontinuity::ComplexityOctaveBoundary => {
                "octave/complexity requires n-eps/n/n+eps probes"
            }
            Discontinuity::SeedChange => "seed requires same-seed and different-seed probes",
            Discontinuity::GraphOrder => "graph order requires non-commuting operator-order probes",
            Discontinuity::PremultAlphaBoundary => "premult alpha requires alpha-ramp probes",
        };
        obligations.push(TestObligation::new(Boundary, reason, 1));
    }

    obligations.sort_by_key(|obligation| obligation.priority);
    obligations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator::builtin_passport;

    #[test]
    fn turbulent_displace_derives_coordinate_and_procedural_obligations() {
        let passport = builtin_passport("ADBE Turbulent Displace").unwrap();
        let obligations = derive_obligations(&passport);

        assert!(obligations
            .iter()
            .any(|obligation| obligation.kind == ObligationKind::CoordinateField));
        assert!(obligations
            .iter()
            .any(|obligation| obligation.kind == ObligationKind::ProceduralDeterminism));
    }
}
