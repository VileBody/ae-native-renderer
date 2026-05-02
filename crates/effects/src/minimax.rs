use crate::{Effect, EffectContext};
use raster_cpu::Canvas;

#[derive(Debug, Default)]
pub struct Minimax;

impl Effect for Minimax {
    fn match_name(&self) -> &'static str {
        "ADBE Minimax"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        _params: &serde_json::Value,
    ) -> anyhow::Result<Canvas> {
        // TODO: implement ADBE Minimax math.
        // Current behavior: pass-through stub.
        Ok(input.clone())
    }
}
