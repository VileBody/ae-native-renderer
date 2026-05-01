#[derive(Debug, Clone)]
pub struct TextAnimatorSpec {
    pub name: String,
    pub selector: TextSelector,
    pub properties: TextAnimatorProperties,
}

#[derive(Debug, Clone)]
pub enum TextSelector {
    Range(RangeSelector),
    Expression(ExpressionSelector),
}

#[derive(Debug, Clone)]
pub struct RangeSelector {
    pub start_percent: f32,
    pub end_percent: f32,
    pub based_on: BasedOn,
    pub smoothness: f32,
}

#[derive(Debug, Clone)]
pub struct ExpressionSelector {
    pub expression: String,
}

#[derive(Debug, Clone, Copy)]
pub enum BasedOn {
    Characters,
    Words,
    Lines,
}

#[derive(Debug, Clone, Default)]
pub struct TextAnimatorProperties {
    pub opacity: Option<f32>,
    pub position_3d: Option<[f32; 3]>,
    pub scale_3d: Option<[f32; 3]>,
    pub rotation_z: Option<f32>,
    pub blur: Option<[f32; 2]>,
}

pub fn range_selector_weight_stub(index: usize, total: usize, start_percent: f32, end_percent: f32) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let pos = ((index + 1) as f32 / total as f32) * 100.0;
    if pos >= start_percent && pos <= end_percent { 1.0 } else { 0.0 }
}
