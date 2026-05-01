pub mod box_blur;
pub mod drop_shadow;
pub mod geometry;
pub mod glow;
pub mod minimax;
pub mod posterize_time;
pub mod registry;
pub mod turbulent_displace;

pub use registry::*;

use raster_cpu::Canvas;

#[derive(Debug, Clone)]
pub struct EffectContext {
    pub time: f64,
    pub fps: f64,
}

pub trait Effect: Send + Sync {
    fn match_name(&self) -> &'static str;

    fn render(&self, input: &Canvas, ctx: &EffectContext) -> anyhow::Result<Canvas>;
}

#[derive(Debug, Clone, Copy)]
pub enum EffectStatus {
    Stub,
    Approximate,
    Production,
}
