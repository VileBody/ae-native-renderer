//! Bridge between render-core property evaluation and expression-engine.

#[derive(Debug, Clone)]
pub struct ExpressionBinding {
    pub property_path: String,
    pub source: String,
}
