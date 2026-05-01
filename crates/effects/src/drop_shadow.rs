use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

#[derive(Debug, Default)]
pub struct DropShadow;

impl Effect for DropShadow {
    fn match_name(&self) -> &'static str {
        "ADBE Drop Shadow"
    }

    fn render(&self, input: &Canvas, _ctx: &EffectContext) -> anyhow::Result<Canvas> {
        // TODO: implement ADBE Drop Shadow math.
        // Current behavior: pass-through stub.
        Ok(input.clone())
    }
}
