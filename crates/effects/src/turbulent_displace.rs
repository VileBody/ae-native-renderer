use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

#[derive(Debug, Default)]
pub struct TurbulentDisplace;

impl Effect for TurbulentDisplace {
    fn match_name(&self) -> &'static str {
        "ADBE Turbulent Displace"
    }

    fn render(&self, input: &Canvas, _ctx: &EffectContext) -> anyhow::Result<Canvas> {
        // TODO: implement ADBE Turbulent Displace math.
        // Current behavior: pass-through stub.
        Ok(input.clone())
    }
}
