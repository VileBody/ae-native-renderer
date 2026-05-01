use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphInstance {
    pub glyph_id: u32,
    pub char_index: usize,
    pub word_index: usize,
    pub line_index: usize,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub bbox: [f32; 4],
}
