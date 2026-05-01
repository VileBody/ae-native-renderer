use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

#[derive(Debug, Default)]
pub struct Geometry2;

impl Effect for Geometry2 {
    fn match_name(&self) -> &'static str {
        "ADBE Geometry2"
    }

    fn render(&self, input: &Canvas, _ctx: &EffectContext) -> anyhow::Result<Canvas> {
        // TODO: implement ADBE Geometry2 math.
        // Current behavior: pass-through stub.
        Ok(input.clone())
    }
}
