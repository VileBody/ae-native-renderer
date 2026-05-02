use fontdue::{Font, FontSettings};
use raster_cpu::Canvas;
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex, OnceLock};

use crate::{resolve_font_path, TextLayoutRequest, TextLayoutResult};

static FONT_CACHE: OnceLock<Mutex<HashMap<String, Arc<Font>>>> = OnceLock::new();

pub fn rasterize_text(req: &TextLayoutRequest, width: u32, height: u32, color: [u8; 4]) -> anyhow::Result<Canvas> {
    let font = load_font(&req.font_id)?;
    let mut canvas = Canvas::transparent(width, height);
    let box_rect = req.box_rect.unwrap_or([0.0, 0.0, width as f32, height as f32]);
    let lines = req.text.lines().collect::<Vec<_>>();
    let lines = if lines.is_empty() { vec![""] } else { lines };
    let line_height = font
        .horizontal_line_metrics(req.font_size)
        .map(|metrics| metrics.new_line_size.abs().max(req.font_size))
        .unwrap_or(req.font_size * 1.2);
    let total_height = line_height * lines.len() as f32;
    let mut baseline = box_rect[1] + ((box_rect[3] - total_height) * 0.5).max(0.0) + req.font_size;

    for line in lines {
        let line_width = measure_line(&font, line, req.font_size);
        let mut pen_x = box_rect[0] + ((box_rect[2] - line_width) * 0.5).max(0.0);

        for ch in line.chars() {
            if ch == '\t' {
                pen_x += req.font_size * 2.0;
                continue;
            }
            if ch.is_whitespace() {
                pen_x += font.metrics(' ', req.font_size).advance_width.max(req.font_size * 0.3);
                continue;
            }

            let (metrics, bitmap) = font.rasterize(ch, req.font_size);
            let glyph_x = pen_x + metrics.xmin as f32;
            let glyph_y = baseline - metrics.ymin as f32 - metrics.height as f32;
            blend_bitmap(&mut canvas, glyph_x, glyph_y, metrics.width, metrics.height, &bitmap, color);
            pen_x += metrics.advance_width;
        }

        baseline += line_height;
    }

    Ok(canvas)
}

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

fn load_font(font_id: &str) -> anyhow::Result<Arc<Font>> {
    let cache = FONT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(font) = cache.lock().expect("font cache poisoned").get(font_id).cloned() {
        return Ok(font);
    }

    let path = resolve_font_path(font_id).ok_or_else(|| anyhow::anyhow!("font '{font_id}' was not found"))?;
    let bytes = fs::read(&path)?;
    let font = Font::from_bytes(bytes, FontSettings::default())
        .map_err(|err| anyhow::anyhow!("failed to load font {}: {err}", path.display()))?;
    let font = Arc::new(font);
    cache
        .lock()
        .expect("font cache poisoned")
        .insert(font_id.to_string(), font.clone());
    Ok(font)
}

fn measure_line(font: &Font, line: &str, font_size: f32) -> f32 {
    line.chars()
        .map(|ch| {
            if ch == '\t' {
                font_size * 2.0
            } else {
                font.metrics(ch, font_size).advance_width
            }
        })
        .sum()
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
