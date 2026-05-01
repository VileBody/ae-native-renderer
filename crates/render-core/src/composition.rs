use render_ir::Scene;

#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub strict_effects: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self { strict_effects: true }
    }
}

#[derive(Debug)]
pub struct RenderContext<'a> {
    pub scene: &'a Scene,
    pub settings: RenderSettings,
}
