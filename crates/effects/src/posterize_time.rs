use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

#[derive(Debug, Default)]
pub struct PosterizeTime;

impl Effect for PosterizeTime {
    fn match_name(&self) -> &'static str {
        "ADBE Posterize Time"
    }

    fn render(&self, input: &Canvas, _ctx: &EffectContext) -> anyhow::Result<Canvas> {
        // TODO: implement ADBE Posterize Time math.
        // Current behavior: pass-through stub.
        Ok(input.clone())
    }
}
