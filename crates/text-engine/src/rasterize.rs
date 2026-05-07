use raster_cpu::Canvas;
use serde::{Deserialize, Serialize};

use crate::{layout_text, load_font, TextLayoutRequest, TextLayoutResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRasterTrace {
    pub schema: String,
    pub source: String,
    pub canvas_size: [u32; 2],
    pub fill_rgba: [f32; 4],
    pub stroke_rgba: [f32; 4],
    pub coverage_backend: String,
    pub pf_world_semantics: String,
    pub draw_chars: Vec<DrawCharPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawCharPlan {
    pub glyph_run_index: usize,
    pub char_index: usize,
    pub character: String,
    pub glyph_id: u32,
    pub font_postscript_name: Option<String>,
    pub font_path: Option<String>,
    pub metric_source: String,
    pub renderable: bool,
    pub will_draw: bool,
    pub draw_fill: bool,
    pub draw_stroke: bool,
    pub skip_reason: Option<String>,
    pub glyph_matrix: [f32; 9],
    pub text_matrix: [f32; 9],
    pub fill_rgba: [f32; 4],
    pub stroke_rgba: [f32; 4],
    pub stroke_width: f32,
    pub line_join: i32,
    pub miter_limit: f32,
    pub orientation: i32,
    pub glyph_origin: [f32; 2],
    pub baseline: f32,
    pub raster_origin: [f32; 2],
    pub bitmap_size: [u32; 2],
    pub glyph_bounds_minmax: [f32; 4],
    pub clipped_bounds_i32: Option<[i32; 4]>,
    pub output_semantics: String,
}

pub fn rasterize_text(
    req: &TextLayoutRequest,
    width: u32,
    height: u32,
    color: [u8; 4],
) -> anyhow::Result<Canvas> {
    let layout = layout_text(req)?;
    Ok(rasterize_text_with_layout(req, &layout, width, height, color)?.0)
}

pub fn rasterize_text_with_layout(
    req: &TextLayoutRequest,
    layout: &TextLayoutResult,
    width: u32,
    height: u32,
    color: [u8; 4],
) -> anyhow::Result<(Canvas, TextRasterTrace)> {
    let font = load_font(&req.font_id)?;
    let mut canvas = Canvas::transparent(width, height);
    let mut draw_chars = Vec::with_capacity(layout.glyphs.len());
    let chars = req.text.chars().collect::<Vec<_>>();
    let fill_rgba = rgba_u8_to_f32(color);
    let stroke_rgba = [0.0, 0.0, 0.0, 0.0];

    for (run_index, glyph) in layout.glyphs.iter().enumerate() {
        let ch = chars.get(glyph.char_index).copied();
        let glyph_id = glyph.glyph_id.min(u16::MAX as u32) as u16;
        let (metrics, bitmap) = font.rasterize_indexed(glyph_id, req.font_size);
        let raster_x = glyph.x + metrics.xmin as f32;
        let raster_y = layout
            .telemetry
            .glyphs
            .get(run_index)
            .map(|telemetry| telemetry.baseline - metrics.ymin as f32 - metrics.height as f32)
            .unwrap_or(glyph.bbox[1]);
        let telemetry = layout.telemetry.glyphs.get(run_index);
        let baseline = telemetry
            .map(|telemetry| telemetry.baseline)
            .unwrap_or_else(|| raster_y + metrics.height as f32);
        let clipped_bounds = clipped_bitmap_bounds(
            raster_x,
            raster_y,
            metrics.width,
            metrics.height,
            width,
            height,
        );
        let draw_fill = color[3] > 0;
        let draw_stroke = false;
        let skip_reason =
            draw_char_skip_reason(ch, glyph.glyph_id, draw_fill, draw_stroke, clipped_bounds);
        let renderable = skip_reason.is_none();
        let will_draw = renderable && clipped_bounds.is_some();
        let plan = DrawCharPlan {
            glyph_run_index: run_index,
            char_index: glyph.char_index,
            character: ch.map(|ch| ch.to_string()).unwrap_or_default(),
            glyph_id: glyph.glyph_id,
            font_postscript_name: telemetry
                .and_then(|telemetry| telemetry.font_postscript_name.clone()),
            font_path: telemetry
                .and_then(|telemetry| telemetry.font_path.as_ref())
                .map(|path| path.display().to_string()),
            metric_source: telemetry
                .map(|telemetry| telemetry.metric_source.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            renderable,
            will_draw,
            draw_fill,
            draw_stroke,
            skip_reason,
            glyph_matrix: [
                req.font_size,
                0.0,
                0.0,
                0.0,
                req.font_size,
                0.0,
                0.0,
                0.0,
                1.0,
            ],
            text_matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, glyph.x, baseline, 1.0],
            fill_rgba,
            stroke_rgba,
            stroke_width: 0.0,
            line_join: 0,
            miter_limit: 2.5,
            orientation: 0,
            glyph_origin: [glyph.x, glyph.y],
            baseline,
            raster_origin: [raster_x, raster_y],
            bitmap_size: [metrics.width as u32, metrics.height as u32],
            glyph_bounds_minmax: [
                raster_x,
                raster_y,
                raster_x + metrics.width as f32,
                raster_y + metrics.height as f32,
            ],
            clipped_bounds_i32: clipped_bounds,
            output_semantics: "native_canvas_straight_rgba8_source_over_pending_pf_world_premult"
                .to_string(),
        };
        if will_draw {
            blend_bitmap(
                &mut canvas,
                raster_x,
                raster_y,
                metrics.width,
                metrics.height,
                &bitmap,
                color,
                clipped_bounds,
            );
        }
        draw_chars.push(plan);
    }

    Ok((
        canvas,
        TextRasterTrace {
            schema: "txt_drawchar_boundary/v1".to_string(),
            source: "native_recovered_from_BEE_TextRenderNode_TXT_DrawChar".to_string(),
            canvas_size: [width, height],
            fill_rgba,
            stroke_rgba,
            coverage_backend: "fontdue_rasterize_indexed_pending_cooltype_TXT_DrawChar".to_string(),
            pf_world_semantics:
                "AE TXT_DrawChar boundary recovered; native Canvas is still straight RGBA8"
                    .to_string(),
            draw_chars,
        },
    ))
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
    clip: Option<[i32; 4]>,
) {
    let clip = clip.unwrap_or([0, 0, canvas.width as i32, canvas.height as i32]);
    for by in 0..height {
        for bx in 0..width {
            let alpha = bitmap[by * width + bx] as f32 / 255.0 * (color[3] as f32 / 255.0);
            if alpha <= 0.0 {
                continue;
            }
            let px = (x + bx as f32).round() as i32;
            let py = (y + by as f32).round() as i32;
            if px < clip[0] || py < clip[1] || px >= clip[2] || py >= clip[3] {
                continue;
            }
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

fn rgba_u8_to_f32(color: [u8; 4]) -> [f32; 4] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        color[3] as f32 / 255.0,
    ]
}

fn clipped_bitmap_bounds(
    x: f32,
    y: f32,
    width: usize,
    height: usize,
    canvas_width: u32,
    canvas_height: u32,
) -> Option<[i32; 4]> {
    if width == 0 || height == 0 {
        return None;
    }
    let x0 = (x.floor() as i32).max(0);
    let y0 = (y.floor() as i32).max(0);
    let x1 = ((x + width as f32).ceil() as i32).min(canvas_width as i32);
    let y1 = ((y + height as f32).ceil() as i32).min(canvas_height as i32);
    (x0 < x1 && y0 < y1).then_some([x0, y0, x1, y1])
}

fn draw_char_skip_reason(
    ch: Option<char>,
    glyph_id: u32,
    draw_fill: bool,
    draw_stroke: bool,
    clipped_bounds: Option<[i32; 4]>,
) -> Option<String> {
    let Some(ch) = ch else {
        return Some("missing_text_character".to_string());
    };
    if ch.is_whitespace() {
        return Some("whitespace".to_string());
    }
    if glyph_id < 1 {
        return Some("glyph_id_lt_1".to_string());
    }
    if !draw_fill && !draw_stroke {
        return Some("fill_and_stroke_disabled".to_string());
    }
    if clipped_bounds.is_none() {
        return Some("empty_or_clipped_bitmap_bounds".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{layout_text, TextLayoutRequest};

    fn request(text: &str) -> TextLayoutRequest {
        TextLayoutRequest {
            text: text.to_string(),
            font_id: "DejaVu Sans".to_string(),
            font_size: 24.0,
            box_rect: Some([0.0, 0.0, 200.0, 80.0]),
        }
    }

    #[test]
    fn draw_char_trace_keeps_whitespace_sublayers_but_skips_them() {
        let req = request("A A");
        let layout = layout_text(&req).unwrap();
        let (_canvas, trace) =
            rasterize_text_with_layout(&req, &layout, 200, 80, [255, 255, 255, 255]).unwrap();

        assert_eq!(trace.schema, "txt_drawchar_boundary/v1");
        assert_eq!(trace.draw_chars.len(), 3);
        assert_eq!(
            trace
                .draw_chars
                .iter()
                .filter(|draw_char| draw_char.will_draw)
                .count(),
            2
        );
        let space = &trace.draw_chars[1];
        assert_eq!(space.character, " ");
        assert_eq!(space.skip_reason.as_deref(), Some("whitespace"));
        assert!(!space.will_draw);
        let first = &trace.draw_chars[0];
        assert_eq!(first.glyph_matrix[0], req.font_size);
        assert_eq!(first.glyph_matrix[4], req.font_size);
        assert_eq!(first.fill_rgba, [1.0, 1.0, 1.0, 1.0]);
        assert!(first.draw_fill);
        assert!(!first.draw_stroke);
        assert!(first.clipped_bounds_i32.is_some());
    }

    #[test]
    fn draw_char_trace_applies_fill_gate_before_raster_output() {
        let req = request("A");
        let layout = layout_text(&req).unwrap();
        let (canvas, trace) =
            rasterize_text_with_layout(&req, &layout, 200, 80, [255, 255, 255, 0]).unwrap();

        let first = &trace.draw_chars[0];
        assert_eq!(
            first.skip_reason.as_deref(),
            Some("fill_and_stroke_disabled")
        );
        assert!(!first.will_draw);
        assert!(canvas.data.chunks_exact(4).all(|pixel| pixel[3] == 0));
    }
}
