use raster_cpu::Canvas;

use crate::TextLayoutResult;

pub fn rasterize_text_debug(layout: &TextLayoutResult, width: u32, height: u32, color: [u8; 4]) -> Canvas {
    // TODO: real glyph rasterization.
    // Current debug behavior: draw tiny rectangles at glyph positions.
    let mut canvas = Canvas::transparent(width, height);
    for glyph in &layout.glyphs {
        let x0 = glyph.x.max(0.0) as u32;
        let y0 = glyph.y.max(0.0) as u32;
        let w = glyph.advance.max(1.0) as u32;
        let h = (glyph.bbox[3]).max(1.0) as u32;
        for y in y0..(y0 + h).min(height) {
            for x in x0..(x0 + w).min(width) {
                canvas.set_pixel(x, y, color);
            }
        }
    }
    canvas
}
