use raster_cpu::Canvas;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex, OnceLock};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::{layout_text, load_font_with_telemetry, TextLayoutRequest, TextLayoutResult};

const OUTLINE_COVERAGE_SUPERSAMPLE: u32 = 4;

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
    pub coverage_backend: String,
    pub coverage_supersample: u32,
    pub coverage_origin_source: String,
    pub coverage_nonzero_pixels: u32,
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
    let (font, font_resolution) = load_font_with_telemetry(&req.font_id)?;
    let font_bytes = font_resolution
        .resolved_path
        .as_ref()
        .and_then(|path| fs::read(path).ok());
    let outline_face = font_bytes
        .as_deref()
        .and_then(|bytes| Face::parse(bytes, 0).ok());
    let mut canvas = Canvas::transparent(width, height);
    let mut draw_chars = Vec::with_capacity(layout.glyphs.len());
    let chars = req.text.chars().collect::<Vec<_>>();
    let fill_rgba = rgba_u8_to_f32(color);
    let stroke_rgba = [0.0, 0.0, 0.0, 0.0];
    let mut used_outline_backend = false;
    let outline_font_key = font_resolution
        .resolved_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| req.font_id.clone());

    for (run_index, glyph) in layout.glyphs.iter().enumerate() {
        let ch = chars.get(glyph.char_index).copied();
        let glyph_id = glyph.glyph_id.min(u16::MAX as u32) as u16;
        let telemetry = layout.telemetry.glyphs.get(run_index);
        let baseline = telemetry
            .map(|telemetry| telemetry.baseline)
            .unwrap_or_else(|| glyph.bbox[1] + glyph.bbox[3]);
        let coverage = outline_face
            .as_ref()
            .and_then(|face| {
                outline_coverage(
                    face,
                    &outline_font_key,
                    glyph_id,
                    glyph.x,
                    baseline,
                    req.font_size,
                )
            })
            .unwrap_or_else(|| fontdue_coverage(&font, glyph_id, glyph.x, baseline, req.font_size));
        used_outline_backend |= coverage.backend == "ttf_outline_nonzero_supersample";
        let clipped_bounds = clipped_bitmap_bounds(
            coverage.raster_x,
            coverage.raster_y,
            coverage.width,
            coverage.height,
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
            raster_origin: [coverage.raster_x, coverage.raster_y],
            bitmap_size: [coverage.width as u32, coverage.height as u32],
            glyph_bounds_minmax: [
                coverage.raster_x,
                coverage.raster_y,
                coverage.raster_x + coverage.width as f32,
                coverage.raster_y + coverage.height as f32,
            ],
            clipped_bounds_i32: clipped_bounds,
            coverage_backend: coverage.backend.to_string(),
            coverage_supersample: coverage.supersample,
            coverage_origin_source: coverage.origin_source.to_string(),
            coverage_nonzero_pixels: coverage.nonzero_pixels,
            output_semantics: "native_canvas_straight_rgba8_source_over_pending_pf_world_premult"
                .to_string(),
        };
        if will_draw {
            blend_bitmap(
                &mut canvas,
                coverage.raster_x,
                coverage.raster_y,
                coverage.width,
                coverage.height,
                coverage.bitmap.as_ref(),
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
            coverage_backend: if used_outline_backend {
                "ttf_outline_nonzero_supersample_v1"
            } else {
                "fontdue_rasterize_indexed_fallback"
            }
            .to_string(),
            pf_world_semantics:
                "AE TXT_DrawChar boundary recovered; native Canvas is still straight RGBA8"
                    .to_string(),
            draw_chars,
        },
    ))
}

struct CoverageBitmap {
    width: usize,
    height: usize,
    raster_x: f32,
    raster_y: f32,
    bitmap: Arc<Vec<u8>>,
    backend: &'static str,
    supersample: u32,
    origin_source: &'static str,
    nonzero_pixels: u32,
}

fn fontdue_coverage(
    font: &fontdue::Font,
    glyph_id: u16,
    glyph_x: f32,
    baseline: f32,
    font_size: f32,
) -> CoverageBitmap {
    let (metrics, bitmap) = font.rasterize_indexed(glyph_id, font_size);
    let raster_x = glyph_x + metrics.xmin as f32;
    let raster_y = baseline - metrics.ymin as f32 - metrics.height as f32;
    let nonzero_pixels = bitmap.iter().filter(|alpha| **alpha > 0).count() as u32;
    CoverageBitmap {
        width: metrics.width,
        height: metrics.height,
        raster_x,
        raster_y,
        bitmap: Arc::new(bitmap),
        backend: "fontdue_rasterize_indexed_fallback",
        supersample: 1,
        origin_source: "fontdue_metrics",
        nonzero_pixels,
    }
}

fn outline_coverage(
    face: &Face<'_>,
    font_key: &str,
    glyph_id: u16,
    glyph_x: f32,
    baseline: f32,
    font_size: f32,
) -> Option<CoverageBitmap> {
    let key = OutlineCoverageKey {
        font_key: font_key.to_string(),
        glyph_id,
        font_size_bits: font_size.to_bits(),
        supersample: OUTLINE_COVERAGE_SUPERSAMPLE,
    };
    if let Some(mask) = outline_coverage_cache()
        .lock()
        .expect("outline coverage cache poisoned")
        .get(&key)
        .cloned()
    {
        return Some(mask.to_bitmap(glyph_x, baseline));
    }
    let mask = Arc::new(build_outline_coverage_mask(
        face,
        glyph_id,
        font_size,
        OUTLINE_COVERAGE_SUPERSAMPLE,
    )?);
    outline_coverage_cache()
        .lock()
        .expect("outline coverage cache poisoned")
        .insert(key, mask.clone());
    Some(mask.to_bitmap(glyph_x, baseline))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OutlineCoverageKey {
    font_key: String,
    glyph_id: u16,
    font_size_bits: u32,
    supersample: u32,
}

#[derive(Debug)]
struct CoverageMask {
    width: usize,
    height: usize,
    x_min: f32,
    y_max: f32,
    bitmap: Arc<Vec<u8>>,
    nonzero_pixels: u32,
    supersample: u32,
}

impl CoverageMask {
    fn to_bitmap(&self, glyph_x: f32, baseline: f32) -> CoverageBitmap {
        CoverageBitmap {
            width: self.width,
            height: self.height,
            raster_x: glyph_x + self.x_min,
            raster_y: baseline - self.y_max,
            bitmap: self.bitmap.clone(),
            backend: "ttf_outline_nonzero_supersample",
            supersample: self.supersample,
            origin_source: "ttf_outline_bbox_baseline",
            nonzero_pixels: self.nonzero_pixels,
        }
    }
}

fn outline_coverage_cache() -> &'static Mutex<HashMap<OutlineCoverageKey, Arc<CoverageMask>>> {
    static CACHE: OnceLock<Mutex<HashMap<OutlineCoverageKey, Arc<CoverageMask>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_outline_coverage_mask(
    face: &Face<'_>,
    glyph_id: u16,
    font_size: f32,
    supersample: u32,
) -> Option<CoverageMask> {
    let glyph_id = GlyphId(glyph_id);
    let units_per_em = face.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return None;
    }
    let mut outline = FlattenedOutline::default();
    let bbox = face.outline_glyph(glyph_id, &mut outline)?;
    outline.finish_contour();
    if outline.contours.is_empty() {
        return None;
    }

    let scale = font_size / units_per_em;
    let x_min = (bbox.x_min as f32 * scale).floor();
    let x_max = (bbox.x_max as f32 * scale).ceil();
    let y_min = (bbox.y_min as f32 * scale).floor();
    let y_max = (bbox.y_max as f32 * scale).ceil();
    let width = (x_max - x_min).max(0.0) as usize;
    let height = (y_max - y_min).max(0.0) as usize;
    if width == 0 || height == 0 {
        return None;
    }

    let ss = supersample.max(1);
    let sample_count = ss * ss;
    let sample_step = 1.0 / ss as f32;
    let mut bitmap = vec![0u8; width * height];
    let mut nonzero_pixels = 0u32;
    for by in 0..height {
        for bx in 0..width {
            let mut covered = 0u32;
            for sy in 0..ss {
                for sx in 0..ss {
                    let px = x_min + bx as f32 + (sx as f32 + 0.5) * sample_step;
                    let py = y_max - by as f32 - (sy as f32 + 0.5) * sample_step;
                    let design_point = Point {
                        x: px / scale,
                        y: py / scale,
                    };
                    if outline.contains_nonzero(design_point) {
                        covered += 1;
                    }
                }
            }
            if covered > 0 {
                nonzero_pixels += 1;
            }
            bitmap[by * width + bx] =
                ((covered as f32 / sample_count as f32) * 255.0).round() as u8;
        }
    }

    Some(CoverageMask {
        width,
        height,
        x_min,
        y_max,
        bitmap: Arc::new(bitmap),
        nonzero_pixels,
        supersample: ss,
    })
}

#[derive(Debug, Copy, Clone, Default)]
struct Point {
    x: f32,
    y: f32,
}

#[derive(Debug, Default)]
struct FlattenedOutline {
    contours: Vec<Vec<Point>>,
    current: Vec<Point>,
    current_point: Point,
}

impl FlattenedOutline {
    fn finish_contour(&mut self) {
        if self.current.len() >= 2 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }

    fn push_line(&mut self, point: Point) {
        self.current.push(point);
        self.current_point = point;
    }

    fn flatten_quad(&mut self, control: Point, end: Point) {
        let start = self.current_point;
        let steps = curve_steps(start, control, end);
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let mt = 1.0 - t;
            self.push_line(Point {
                x: mt * mt * start.x + 2.0 * mt * t * control.x + t * t * end.x,
                y: mt * mt * start.y + 2.0 * mt * t * control.y + t * t * end.y,
            });
        }
    }

    fn flatten_cubic(&mut self, control1: Point, control2: Point, end: Point) {
        let start = self.current_point;
        let steps = curve_steps(start, control1, end).max(curve_steps(start, control2, end));
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let mt = 1.0 - t;
            self.push_line(Point {
                x: mt * mt * mt * start.x
                    + 3.0 * mt * mt * t * control1.x
                    + 3.0 * mt * t * t * control2.x
                    + t * t * t * end.x,
                y: mt * mt * mt * start.y
                    + 3.0 * mt * mt * t * control1.y
                    + 3.0 * mt * t * t * control2.y
                    + t * t * t * end.y,
            });
        }
    }

    fn contains_nonzero(&self, point: Point) -> bool {
        let mut winding = 0i32;
        for contour in &self.contours {
            winding += contour_winding(contour, point);
        }
        winding != 0
    }
}

impl OutlineBuilder for FlattenedOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish_contour();
        let point = Point { x, y };
        self.current.push(point);
        self.current_point = point;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.push_line(Point { x, y });
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.flatten_quad(Point { x: x1, y: y1 }, Point { x, y });
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.flatten_cubic(
            Point { x: x1, y: y1 },
            Point { x: x2, y: y2 },
            Point { x, y },
        );
    }

    fn close(&mut self) {
        self.finish_contour();
    }
}

fn curve_steps(a: Point, b: Point, c: Point) -> u32 {
    let length = distance(a, b) + distance(b, c);
    ((length / 16.0).ceil() as u32).clamp(8, 48)
}

fn distance(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn contour_winding(contour: &[Point], point: Point) -> i32 {
    if contour.len() < 2 {
        return 0;
    }
    let mut winding = 0i32;
    for index in 0..contour.len() {
        let a = contour[index];
        let b = contour[(index + 1) % contour.len()];
        if a.y <= point.y {
            if b.y > point.y && is_left(a, b, point) > 0.0 {
                winding += 1;
            }
        } else if b.y <= point.y && is_left(a, b, point) < 0.0 {
            winding -= 1;
        }
    }
    winding
}

fn is_left(a: Point, b: Point, p: Point) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y)
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
    use std::path::PathBuf;

    fn request(text: &str) -> TextLayoutRequest {
        TextLayoutRequest {
            text: text.to_string(),
            font_id: "DejaVu Sans".to_string(),
            font_size: 24.0,
            box_rect: Some([0.0, 0.0, 200.0, 80.0]),
        }
    }

    fn montserrat_bolditalic_fixture() -> Option<PathBuf> {
        let path =
            PathBuf::from("fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf");
        path.exists().then_some(path)
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

    #[test]
    fn draw_char_trace_uses_outline_coverage_for_ttf_fonts() {
        let Some(path) = montserrat_bolditalic_fixture() else {
            return;
        };
        let req = TextLayoutRequest {
            text: "WA".to_string(),
            font_id: path.display().to_string(),
            font_size: 58.0,
            box_rect: Some([0.0, 0.0, 256.0, 128.0]),
        };
        let layout = layout_text(&req).unwrap();
        let (_canvas, trace) =
            rasterize_text_with_layout(&req, &layout, 256, 128, [255, 255, 255, 255]).unwrap();

        assert_eq!(trace.coverage_backend, "ttf_outline_nonzero_supersample_v1");
        assert!(trace.draw_chars.iter().all(|draw_char| {
            draw_char.coverage_backend == "ttf_outline_nonzero_supersample"
                && draw_char.coverage_supersample == OUTLINE_COVERAGE_SUPERSAMPLE
                && draw_char.coverage_nonzero_pixels > 0
        }));
    }
}
