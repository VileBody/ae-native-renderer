use raster_cpu::Canvas;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex, OnceLock};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::{layout_text, load_font_with_telemetry, TextLayoutRequest, TextLayoutResult};

const OUTLINE_COVERAGE_SUPERSAMPLE: u32 = 16;

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
    pub coverage_rows: Vec<CoverageRowSpan>,
    pub output_semantics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageRowSpan {
    pub schema: String,
    pub policy: String,
    pub glyph_run_index: usize,
    pub glyph_id: u32,
    pub y: i32,
    pub start_x: i32,
    pub end_x: i32,
    pub coverage_len: u32,
    pub coverage_hash_fnv1a64: String,
    pub coverage_sample_hex: String,
    pub coverage_hex: Option<String>,
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
        used_outline_backend |= coverage.backend == "ttf_outline_are_scanline_16x";
        let clipped_bounds = clipped_bitmap_bounds(
            coverage.raster_x,
            coverage.raster_y,
            coverage.width,
            coverage.height,
            width,
            height,
        );
        let coverage_rows = coverage_row_spans(
            run_index,
            glyph.glyph_id,
            coverage.raster_x,
            coverage.raster_y,
            coverage.width,
            coverage.height,
            coverage.bitmap.as_ref(),
            clipped_bounds,
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
            coverage_rows,
            output_semantics: "recovered_txt_are_pf_pixel8_integer_source_over_v1".to_string(),
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
                "ttf_outline_are_scanline_16x_v1"
            } else {
                "fontdue_rasterize_indexed_fallback"
            }
            .to_string(),
            pf_world_semantics:
                "TXT_DrawChar ARE PF_Pixel8 direct source-over; fill opacity is source pixel alpha"
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
        glyph_x_bits: glyph_x.to_bits(),
        baseline_bits: baseline.to_bits(),
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
        glyph_x,
        baseline,
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
    glyph_x_bits: u32,
    baseline_bits: u32,
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
            backend: "ttf_outline_are_scanline_16x",
            supersample: self.supersample,
            origin_source: "txt_are_integer_world_bbox_origin",
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
    glyph_x: f32,
    baseline: f32,
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
    let bbox_x_min = bbox.x_min as f32 * scale;
    let bbox_x_max = bbox.x_max as f32 * scale;
    let bbox_y_min = bbox.y_min as f32 * scale;
    let bbox_y_max = bbox.y_max as f32 * scale;
    let abs_left = (glyph_x + bbox_x_min).floor();
    let abs_right = (glyph_x + bbox_x_max).ceil();
    let abs_top = (baseline - bbox_y_max).floor();
    let abs_bottom = (baseline - bbox_y_min).ceil();
    let x_min = abs_left - glyph_x;
    let y_max = baseline - abs_top;
    let width = (abs_right - abs_left).max(0.0) as usize;
    let height = (abs_bottom - abs_top).max(0.0) as usize;
    if width == 0 || height == 0 {
        return None;
    }

    let ss = supersample.max(1);
    let mut bitmap = vec![0u8; width * height];
    let mut nonzero_pixels = 0u32;
    if ss == 16 {
        build_are_scanline_coverage(&outline, scale, x_min, y_max, width, height, &mut bitmap);
        nonzero_pixels = bitmap.iter().filter(|alpha| **alpha > 0).count() as u32;
    } else {
        let sample_count = ss * ss;
        let sample_step = 1.0 / ss as f32;
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

#[derive(Debug, Copy, Clone)]
struct ProjectedScanlineEvent {
    x_min_fixed: i32,
    x_max_fixed: i32,
    winding_delta: i32,
}

fn build_are_scanline_coverage(
    outline: &FlattenedOutline,
    scale: f32,
    x_min: f32,
    y_max: f32,
    width: usize,
    height: usize,
    bitmap: &mut [u8],
) {
    let ss = 16i32;
    for by in 0..height {
        let mut row_fixed = vec![0u16; width];
        for sy in 0..ss {
            // ARE.dll+0x76dc seeds 16 row buckets at y*16+subrow. The active
            // edge projector at +0x78e4 computes min/max x over each fixed
            // subrow strip, and +0x430c emits floor(min)..floor(max)+1 events.
            let strip_y_fixed = by as i32 * ss + sy;
            for (start_fixed, end_fixed) in
                projected_are_scanline_intervals(outline, scale, x_min, y_max, strip_y_fixed, ss)
            {
                if end_fixed <= start_fixed {
                    continue;
                }
                let bx0 = start_fixed.div_euclid(ss).clamp(0, width as i32);
                let bx1 = ((end_fixed + ss - 1).div_euclid(ss)).clamp(0, width as i32);
                for bx in bx0..bx1 {
                    let pixel_start = bx * ss;
                    let pixel_end = pixel_start + ss;
                    let covered =
                        (end_fixed.min(pixel_end) - start_fixed.max(pixel_start)).clamp(0, ss);
                    if covered > 0 {
                        let slot = &mut row_fixed[bx as usize];
                        *slot = (*slot + covered as u16).min(256);
                    }
                }
            }
        }

        for (bx, coverage) in row_fixed.into_iter().enumerate() {
            bitmap[by * width + bx] = if coverage >= 256 {
                u8::MAX
            } else {
                coverage as u8
            };
        }
    }
}

fn projected_are_scanline_intervals(
    outline: &FlattenedOutline,
    scale: f32,
    x_min: f32,
    y_max: f32,
    strip_y_fixed: i32,
    ss: i32,
) -> Vec<(i32, i32)> {
    let strip_start = strip_y_fixed as f32;
    let strip_end = (strip_y_fixed + 1) as f32;
    let mut events = Vec::new();
    for contour in &outline.contours {
        if contour.len() < 2 {
            continue;
        }
        for index in 0..contour.len() {
            let a = contour[index];
            let b = contour[(index + 1) % contour.len()];
            let ax = (a.x * scale - x_min) * ss as f32;
            let bx = (b.x * scale - x_min) * ss as f32;
            let ay = (y_max - a.y * scale) * ss as f32;
            let by = (y_max - b.y * scale) * ss as f32;
            let min_y = ay.min(by);
            let max_y = ay.max(by);
            if max_y <= strip_start || min_y >= strip_end || ay == by {
                continue;
            }
            let y0 = strip_start.clamp(min_y, max_y);
            let y1 = strip_end.clamp(min_y, max_y);
            let t0 = ((y0 - ay) / (by - ay)).clamp(0.0, 1.0);
            let t1 = ((y1 - ay) / (by - ay)).clamp(0.0, 1.0);
            let x0 = ax + (bx - ax) * t0;
            let x1 = ax + (bx - ax) * t1;
            events.push(ProjectedScanlineEvent {
                x_min_fixed: x0.min(x1).floor() as i32,
                x_max_fixed: x0.max(x1).floor() as i32,
                winding_delta: if b.y > a.y { 1 } else { -1 },
            });
        }
    }
    // ARE's event list preserves insertion order for equal projected starts;
    // adding a secondary x_max sort changes vertex-tie coverage by a byte.
    events.sort_by_key(|event| event.x_min_fixed);

    let mut intervals = Vec::new();
    let mut winding = 0i32;
    let mut start: Option<i32> = None;
    let mut max_end = i32::MIN;
    for event in events {
        if start.is_none() && winding == 0 {
            start = Some(event.x_min_fixed);
            max_end = event.x_max_fixed;
        } else if start.is_some() {
            max_end = max_end.max(event.x_max_fixed);
        }
        winding += event.winding_delta;
        if winding == 0 {
            if let Some(start_fixed) = start.take() {
                intervals.push((start_fixed, max_end + 1));
            }
        }
    }
    intervals
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
    current_has_curve: bool,
}

impl FlattenedOutline {
    fn finish_contour(&mut self) {
        if self.current.len() >= 2 {
            let mut contour = std::mem::take(&mut self.current);
            if !self.current_has_curve {
                // TXT_ARE_PathBuilder traces for line-only glyphs emit scaled
                // TTF contour points in reverse order.
                contour.reverse();
            }
            self.contours.push(contour);
        } else {
            self.current.clear();
        }
        self.current_has_curve = false;
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
        self.current_has_curve = true;
        self.flatten_quad(Point { x: x1, y: y1 }, Point { x, y });
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.current_has_curve = true;
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
    let origin_x = x.round() as i32;
    let origin_y = y.round() as i32;
    for by in 0..height {
        for bx in 0..width {
            let coverage = bitmap[by * width + bx];
            if coverage == 0 || color[3] == 0 {
                continue;
            }
            let px = origin_x + bx as i32;
            let py = origin_y + by as i32;
            if px < clip[0] || py < clip[1] || px >= clip[2] || py >= clip[3] {
                continue;
            }
            if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                continue;
            }

            let dst = canvas.pixel(px as u32, py as u32);
            let out = blend_text_pixel_ae_u8(dst, color, coverage);
            canvas.set_pixel(px as u32, py as u32, out);
        }
    }
}

fn coverage_row_spans(
    glyph_run_index: usize,
    glyph_id: u32,
    x: f32,
    y: f32,
    width: usize,
    height: usize,
    bitmap: &[u8],
    clip: Option<[i32; 4]>,
) -> Vec<CoverageRowSpan> {
    let Some(clip) = clip else {
        return Vec::new();
    };
    let origin_x = x.round() as i32;
    let origin_y = y.round() as i32;
    let mut spans = Vec::new();

    for by in 0..height {
        let py = origin_y + by as i32;
        if py < clip[1] || py >= clip[3] {
            continue;
        }

        let mut bx = 0usize;
        while bx < width {
            let px = origin_x + bx as i32;
            let coverage = bitmap[by * width + bx];
            if coverage == 0 || px < clip[0] || px >= clip[2] {
                bx += 1;
                continue;
            }

            let start_bx = bx;
            let start_x = px;
            bx += 1;
            while bx < width {
                let px = origin_x + bx as i32;
                if px < clip[0] || px >= clip[2] || bitmap[by * width + bx] == 0 {
                    break;
                }
                bx += 1;
            }

            let coverage_bytes = &bitmap[by * width + start_bx..by * width + bx];
            spans.push(CoverageRowSpan {
                schema: "ae-native-renderer.text-coverage-row.v1".to_string(),
                policy: "are_scanline_16x_contiguous_runs_with_integer_origin".to_string(),
                glyph_run_index,
                glyph_id,
                y: py,
                start_x,
                end_x: origin_x + bx as i32,
                coverage_len: coverage_bytes.len() as u32,
                coverage_hash_fnv1a64: format!("{:016x}", fnv1a64(coverage_bytes)),
                coverage_sample_hex: hex_prefix(coverage_bytes, 64),
                coverage_hex: (coverage_bytes.len() <= 256).then(|| bytes_hex(coverage_bytes)),
            });
        }
    }

    spans
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn hex_prefix(bytes: &[u8], max_len: usize) -> String {
    bytes_hex(&bytes[..bytes.len().min(max_len)])
}

fn bytes_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn blend_text_pixel_ae_u8(dst: [u8; 4], src: [u8; 4], coverage: u8) -> [u8; 4] {
    let src_alpha = if coverage == u8::MAX {
        src[3]
    } else {
        mul_u8_ae(src[3], coverage)
    };
    if src_alpha == 0 {
        return dst;
    }
    if src_alpha == u8::MAX {
        return [src[0], src[1], src[2], src_alpha];
    }

    let dst_alpha = dst[3];
    if dst_alpha == 0 {
        return [src[0], src[1], src[2], src_alpha];
    }

    let inv_mul = mul_u8_ae(u8::MAX - src_alpha, u8::MAX - dst_alpha);
    let out_alpha = u8::MAX - inv_mul;
    let mut out = [0u8; 4];
    for channel in 0..3 {
        let dst_premul = mul_u8_ae(dst[channel], dst_alpha) as i32;
        let src_delta = src[channel] as i32 - dst_premul;
        let out_premul = dst_premul + div255_signed_ae(src_delta * src_alpha as i32);
        out[channel] = unpremultiply_u8_ae(out_premul, out_alpha);
    }
    out[3] = out_alpha;
    out
}

fn mul_u8_ae(a: u8, b: u8) -> u8 {
    let x = a as u32 * b as u32 + 0x80;
    (((x >> 8) + x) >> 8).min(255) as u8
}

fn div255_signed_ae(value: i32) -> i32 {
    let x = value + 0x80;
    (x + (x >> 8)) >> 8
}

fn unpremultiply_u8_ae(premul: i32, alpha: u8) -> u8 {
    if alpha == 0 {
        return 0;
    }
    ((premul * 255 + alpha as i32 / 2) / alpha as i32).clamp(0, 255) as u8
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

        assert_eq!(trace.coverage_backend, "ttf_outline_are_scanline_16x_v1");
        assert!(trace.draw_chars.iter().all(|draw_char| {
            draw_char.coverage_backend == "ttf_outline_are_scanline_16x"
                && draw_char.coverage_supersample == OUTLINE_COVERAGE_SUPERSAMPLE
                && draw_char.coverage_nonzero_pixels > 0
                && !draw_char.coverage_rows.is_empty()
        }));
    }

    #[test]
    fn recovered_txt_are_origin_and_subrow_phase_match_cov_w_trace() {
        let Some(path) = montserrat_bolditalic_fixture() else {
            return;
        };
        let bytes = fs::read(path).unwrap();
        let face = Face::parse(&bytes, 0).unwrap();
        let mask = build_outline_coverage_mask(
            &face,
            331,
            72.94400024414062,
            128.0,
            96.0,
            OUTLINE_COVERAGE_SUPERSAMPLE,
        )
        .unwrap();

        assert_eq!(mask.width, 109);
        assert_eq!(mask.height, 68);
        assert!((mask.x_min - 9.055999755859375).abs() < 0.00001);
        assert_eq!(mask.y_max, 68.0);

        let first_row = &mask.bitmap[..mask.width];
        let spans = row_nonzero_spans(first_row);
        assert_eq!(
            spans[0],
            (0, 16, "2440404040404040404040404040403c".to_string())
        );
        assert_eq!(
            spans[1],
            (46, 62, "013e4040404040404040404040404024".to_string())
        );
        assert_eq!(
            spans[2],
            (92, 109, "1e40404040404040404040404040404004".to_string())
        );

        let row48_start = 48 * mask.width;
        let row48 = &mask.bitmap[row48_start..row48_start + mask.width];
        let row48_spans = row_nonzero_spans(row48);
        assert!(row48_spans.contains(&(
            53,
            84,
            "b4fffffffffffffffffffffffffff7ffffffffffffffffffffffffffffe006".to_string()
        )));
    }

    #[test]
    fn recovered_txt_are_pixel8_blend_matches_reverse_formula() {
        assert_eq!(
            blend_text_pixel_ae_u8([10, 20, 30, 128], [200, 40, 80, 128], 64),
            [51, 25, 41, 144]
        );
        assert_eq!(
            blend_text_pixel_ae_u8([5, 5, 6, 0], [250, 0, 0, 128], 128),
            [250, 0, 0, 64]
        );
        assert_eq!(
            blend_text_pixel_ae_u8([20, 40, 80, 128], [220, 80, 20, 128], 90),
            [80, 53, 61, 150]
        );
    }

    #[test]
    fn semitransparent_fill_uses_direct_source_pixel_alpha() {
        let mut canvas = Canvas::transparent(1, 1);
        blend_bitmap(
            &mut canvas,
            0.0,
            0.0,
            1,
            1,
            &[255],
            [200, 100, 50, 128],
            None,
        );
        assert_eq!(canvas.pixel(0, 0), [200, 100, 50, 128]);

        blend_bitmap(
            &mut canvas,
            0.0,
            0.0,
            1,
            1,
            &[128],
            [200, 100, 50, 128],
            None,
        );
        assert_eq!(canvas.pixel(0, 0), [199, 100, 49, 160]);
    }

    #[test]
    fn coverage_row_spans_record_integer_runs_and_hashes() {
        let bitmap = [0, 7, 9, 0, 0, 255, 1, 0];
        let spans = coverage_row_spans(3, 331, 10.2, 20.7, 4, 2, &bitmap, Some([0, 0, 100, 100]));

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].glyph_run_index, 3);
        assert_eq!(spans[0].glyph_id, 331);
        assert_eq!(spans[0].y, 21);
        assert_eq!(spans[0].start_x, 11);
        assert_eq!(spans[0].end_x, 13);
        assert_eq!(spans[0].coverage_len, 2);
        assert_eq!(spans[0].coverage_hex.as_deref(), Some("0709"));
        assert_eq!(spans[1].y, 22);
        assert_eq!(spans[1].start_x, 11);
        assert_eq!(spans[1].end_x, 13);
        assert_eq!(spans[1].coverage_hex.as_deref(), Some("ff01"));
    }

    fn row_nonzero_spans(row: &[u8]) -> Vec<(usize, usize, String)> {
        let mut spans = Vec::new();
        let mut index = 0usize;
        while index < row.len() {
            if row[index] == 0 {
                index += 1;
                continue;
            }
            let start = index;
            index += 1;
            while index < row.len() && row[index] != 0 {
                index += 1;
            }
            spans.push((start, index, bytes_hex(&row[start..index])));
        }
        spans
    }
}
