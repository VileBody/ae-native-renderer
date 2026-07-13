use crate::{param_f32_at_any, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

const COMPLETION_NAMES: &[&str] = &[
    "completion",
    "Completion",
    "completion_percent",
    "completionPercent",
    "Completion Percent",
    "Completion (%)",
    "0001",
    "CC Image Wipe-0001",
];
const BORDER_SOFTNESS_NAMES: &[&str] = &[
    "border_softness",
    "borderSoftness",
    "Border Softness",
    "0002",
    "CC Image Wipe-0002",
];

#[derive(Debug, Default)]
pub struct ImageWipe;

impl Effect for ImageWipe {
    fn match_name(&self) -> &'static str {
        "CC Image Wipe"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = ImageWipeParams::from_json(params, ctx.time);
        if params.completion <= 0.0 {
            return Ok(input.clone());
        }

        let mut output = input.clone();
        for (source, destination) in input
            .data
            .chunks_exact(4)
            .zip(output.data.chunks_exact_mut(4))
        {
            let alpha = source[3] as f32 / 255.0;
            let luminance =
                (0.2126 * source[0] as f32 + 0.7152 * source[1] as f32 + 0.0722 * source[2] as f32)
                    / 255.0;
            let gradient = luminance * alpha;
            let mask = image_wipe_mask(gradient, params.completion, params.border_softness);
            destination[3] = (source[3] as f32 * mask).round().clamp(0.0, 255.0) as u8;
        }
        Ok(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageWipeParams {
    pub completion: f32,
    pub border_softness: f32,
}

impl ImageWipeParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            completion: normalized_fraction(param_f32_at_any(params, COMPLETION_NAMES, time, 0.0)),
            border_softness: normalized_fraction(param_f32_at_any(
                params,
                BORDER_SOFTNESS_NAMES,
                time,
                0.0,
            )),
        }
    }
}

fn normalized_fraction(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    let normalized = if value.abs() > 1.0 {
        value / 100.0
    } else {
        value
    };
    normalized.clamp(0.0, 1.0)
}

fn image_wipe_mask(gradient: f32, completion: f32, softness: f32) -> f32 {
    if completion <= 0.0 {
        return 1.0;
    }
    if completion >= 1.0 {
        return 0.0;
    }
    if softness <= f32::EPSILON {
        return if gradient >= completion { 1.0 } else { 0.0 };
    }

    let lo = completion * (1.0 + softness) - softness;
    let hi = completion * (1.0 + softness);
    if gradient <= lo {
        0.0
    } else if gradient >= hi {
        1.0
    } else {
        ((gradient - lo) / softness).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn context(time: f64) -> EffectContext {
        EffectContext {
            time,
            fps: 24_000.0 / 1_001.0,
        }
    }

    #[test]
    fn params_accept_normalized_and_ae_percent_aliases_with_keyframes() {
        let normalized =
            ImageWipeParams::from_json(&json!({"completion": 0.4, "border_softness": 0.08}), 0.0);
        assert_eq!(normalized.completion, 0.4);
        assert_eq!(normalized.border_softness, 0.08);

        let ae_aliases = json!({
            "CC Image Wipe-0001": {
                "keyframes": [
                    {"time": 0.0, "value": 0.0},
                    {"time": 1.0, "value": 40.0}
                ]
            },
            "CC Image Wipe-0002": 8.0
        });
        let sampled = ImageWipeParams::from_json(&ae_aliases, 0.5);
        assert!((sampled.completion - 0.2).abs() < 1.0e-6);
        assert!((sampled.border_softness - 0.08).abs() < 1.0e-6);
    }

    #[test]
    fn production_brat_completion_matches_frame_locked_samples() {
        let period = 30.0 / 166.71;
        let ease = json!({"x1": 1.0 / 3.0, "y1": 0.0, "x2": 2.0 / 3.0, "y2": 1.0});
        let params = json!({
            "completion": {
                "keyframes": [
                    {"time": 3.0 * period, "value": 0.0, "ease": ease},
                    {"time": 3.5 * period, "value": 0.4, "ease": ease},
                    {"time": 4.0 * period, "value": 0.0, "ease": ease},
                    {"time": 4.5 * period, "value": 0.4, "ease": ease}
                ]
            }
        });
        let expected = [
            (14, 0.193_790_8),
            (15, 0.3974533),
            (17, 0.01582441),
            (19, 0.3611951),
        ];

        for (frame, expected) in expected {
            let time = frame as f64 * 1_001.0 / 24_000.0;
            let sampled = ImageWipeParams::from_json(&params, time).completion;
            assert!(
                (sampled - expected).abs() < 1.0e-5,
                "frame {frame}: expected {expected}, got {sampled}"
            );
        }
    }

    #[test]
    fn spatial_mask_uses_straight_rgba_luminance_times_alpha() {
        let input = Canvas::from_rgba(
            4,
            1,
            vec![
                102, 102, 102, 255, 128, 128, 128, 255, 153, 153, 153, 255, 255, 255, 255, 128,
            ],
        )
        .unwrap();
        let output = ImageWipe
            .render(
                &input,
                &context(0.0),
                &json!({"completion": 0.5, "border_softness": 0.2}),
            )
            .unwrap();

        assert_eq!(output.pixel(0, 0)[3], 0);
        assert_eq!(output.pixel(1, 0)[3], 130);
        assert_eq!(output.pixel(2, 0)[3], 255);
        assert_eq!(output.pixel(3, 0)[3], 65);
        for x in 0..input.width {
            assert_eq!(output.pixel(x, 0)[0..3], input.pixel(x, 0)[0..3]);
        }
    }
}
