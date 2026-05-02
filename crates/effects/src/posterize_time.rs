use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

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
        _params: &serde_json::Value,
    ) -> anyhow::Result<Canvas> {
        // Posterize Time changes which source frame is sampled. At this stateless
        // per-canvas stage the frame has already been rendered, so the stable
        // approximation is to preserve pixels unchanged.
        Ok(input.clone())
    }
}
