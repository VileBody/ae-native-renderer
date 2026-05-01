use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

#[derive(Debug, Default)]
pub struct BoxBlur2;

impl Effect for BoxBlur2 {
    fn match_name(&self) -> &'static str {
        "ADBE Box Blur2"
    }

    fn render(&self, input: &Canvas, _ctx: &EffectContext) -> anyhow::Result<Canvas> {
        // TODO: implement ADBE Box Blur2 math.
        // Current behavior: pass-through stub.
        Ok(input.clone())
    }
}
