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
        // Posterize Time changes which source frame is sampled. At this stateless
        // per-canvas stage the frame has already been rendered, so the stable
        // approximation is to preserve pixels unchanged.
        Ok(input.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PosterizeTimeParams {
    pub frame_rate: f32,
}

impl PosterizeTimeParams {
    pub(crate) fn from_json(params: &Value) -> Self {
        Self {
            frame_rate: param_f32_any(params, &["frameRate", "Frame Rate", "0001"], 0.0),
        }
    }
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
            PosterizeTimeParams::from_json(&json!({ "0001": { "value": 8 } })).frame_rate,
            8.0
        );
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
