use crate::GlyphInstance;

#[derive(Debug, Clone)]
pub struct TextLayoutRequest {
    pub text: String,
    pub font_id: String,
    pub font_size: f32,
    pub box_rect: Option<[f32; 4]>,
}

#[derive(Debug, Clone)]
pub struct TextLayoutResult {
    pub glyphs: Vec<GlyphInstance>,
}

pub fn layout_text_stub(req: &TextLayoutRequest) -> TextLayoutResult {
    // TODO: replace with HarfBuzz/FreeType shaping.
    // This stub keeps per-character structure for future Text Animator work.
    let mut glyphs = Vec::new();
    let mut x = req.box_rect.map(|r| r[0]).unwrap_or(0.0);
    let y = req.box_rect.map(|r| r[1]).unwrap_or(0.0);
    let mut word_index = 0usize;
    for (char_index, ch) in req.text.chars().enumerate() {
        if ch.is_whitespace() {
            word_index += 1;
        }
        let advance = req.font_size * 0.6;
        glyphs.push(GlyphInstance {
            glyph_id: ch as u32,
            char_index,
            word_index,
            line_index: 0,
            x,
            y,
            advance,
            bbox: [x, y, advance, req.font_size],
        });
        x += advance;
    }
    TextLayoutResult { glyphs }
}
