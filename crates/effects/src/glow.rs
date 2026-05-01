use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

#[derive(Debug, Default)]
pub struct Glow;

impl Effect for Glow {
    fn match_name(&self) -> &'static str {
        "ADBE Glo2"
    }

    fn render(&self, input: &Canvas, _ctx: &EffectContext) -> anyhow::Result<Canvas> {
        // TODO: implement ADBE Glo2 math.
        // Current behavior: pass-through stub.
        Ok(input.clone())
    }
}
