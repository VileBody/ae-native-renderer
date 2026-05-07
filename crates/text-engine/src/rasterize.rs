use raster_cpu::Canvas;

use crate::{layout_text, load_font, TextLayoutRequest, TextLayoutResult};

pub fn rasterize_text(
    req: &TextLayoutRequest,
    width: u32,
    height: u32,
    color: [u8; 4],
) -> anyhow::Result<Canvas> {
    let font = load_font(&req.font_id)?;
    let mut canvas = Canvas::transparent(width, height);
    let layout = layout_text(req)?;

    for (run_index, glyph) in layout.glyphs.iter().enumerate() {
        let Some(ch) = req.text.chars().nth(glyph.char_index) else {
            continue;
        };
        if ch.is_whitespace() {
            continue;
        }
        let glyph_id = glyph.glyph_id.min(u16::MAX as u32) as u16;
        let (metrics, bitmap) = font.rasterize_indexed(glyph_id, req.font_size);
        let raster_x = glyph.x + metrics.xmin as f32;
        let raster_y = layout
            .telemetry
            .glyphs
            .get(run_index)
            .map(|telemetry| {
                telemetry.baseline - metrics.ymin as f32 - metrics.height as f32
            })
            .unwrap_or(glyph.bbox[1]);
        blend_bitmap(
            &mut canvas,
            raster_x,
            raster_y,
            metrics.width,
            metrics.height,
            &bitmap,
            color,
        );
    }

    Ok(canvas)
}

pub fn rasterize_text_debug(
    layout: &TextLayoutResult,
    width: u32,
    height: u32,
    color: [u8; 4],
) -> Canvas {
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

fn blend_bitmap(
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    width: usize,
    height: usize,
    bitmap: &[u8],
    color: [u8; 4],
) {
    for by in 0..height {
        for bx in 0..width {
            let alpha = bitmap[by * width + bx] as f32 / 255.0 * (color[3] as f32 / 255.0);
            if alpha <= 0.0 {
                continue;
            }
            let px = (x + bx as f32).round() as i32;
            let py = (y + by as f32).round() as i32;
            if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                continue;
            }

            let dst = canvas.pixel(px as u32, py as u32);
            let dst_a = dst[3] as f32 / 255.0;
            let out_a = alpha + dst_a * (1.0 - alpha);
            let mut out = [0u8; 4];
            for channel in 0..3 {
                let src = color[channel] as f32 / 255.0;
                let dst = dst[channel] as f32 / 255.0;
                let value = if out_a <= 0.0 {
                    0.0
                } else {
                    (src * alpha + dst * dst_a * (1.0 - alpha)) / out_a
                };
                out[channel] = (value * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            out[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            canvas.set_pixel(px as u32, py as u32, out);
        }
    }
}
