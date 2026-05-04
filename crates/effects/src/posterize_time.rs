use crate::{param_f32_any, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct PosterizeTime;

impl Effect for PosterizeTime {
    fn match_name(&self) -> &'static str {
        "ADBE Posterize Time"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let _params = PosterizeTimeParams::from_json(params);
        // Render-core handles Posterize Time before source/effect evaluation.
        // At this stateless per-canvas stage the pixels are already sampled.
        Ok(input.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PosterizeTimeParams {
    pub frame_rate: f32,
}

impl PosterizeTimeParams {
    pub fn from_json(params: &Value) -> Self {
        Self {
            frame_rate: param_f32_any(
                params,
                &["frameRate", "frame_rate", "Frame Rate", "0001"],
                0.0,
            ),
        }
    }

    pub fn quantized_time(self, time: f64) -> f64 {
        quantize_time(time, self.frame_rate)
    }
}

pub fn quantize_time(time: f64, frame_rate: f32) -> f64 {
    if !time.is_finite() || !frame_rate.is_finite() || frame_rate <= 0.0 {
        return time;
    }
    let fps = frame_rate as f64;
    ((time * fps) + 1.0e-9).floor() / fps
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn params_accept_named_and_ae_numbered_frame_rate() {
        assert_eq!(
            PosterizeTimeParams::from_json(&json!({ "frameRate": 12 })).frame_rate,
            12.0
        );
        assert_eq!(
            PosterizeTimeParams::from_json(&json!({ "frame_rate": 10 })).frame_rate,
            10.0
        );
        assert_eq!(
            PosterizeTimeParams::from_json(&json!({ "0001": { "value": 8 } })).frame_rate,
            8.0
        );
    }

    #[test]
    fn quantized_time_floors_to_posterize_frame() {
        assert_eq!(
            PosterizeTimeParams { frame_rate: 10.0 }.quantized_time(0.16),
            0.1
        );
        assert_eq!(quantize_time(0.5, 0.0), 0.5);
    }

    #[test]
    fn render_is_canvas_stage_noop() {
        let input = Canvas::new(2, 2, [10, 20, 30, 40]);
        let output = PosterizeTime::default()
            .render(
                &input,
                &EffectContext {
                    time: 1.0,
                    fps: 30.0,
                },
                &json!({ "0001": 12 }),
            )
            .unwrap();

        assert_eq!(output.data, input.data);
    }
}
