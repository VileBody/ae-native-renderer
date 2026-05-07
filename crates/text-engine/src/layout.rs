use fontdue::Font;
use rustybuzz::{BufferClusterLevel, Face as BuzzFace, UnicodeBuffer};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use ttf_parser::{Face, GlyphId, Rect as TtfRect};

use crate::{
    load_font_with_telemetry, FontResolutionSource, FontResolutionTelemetry, GlyphInstance,
};

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
    pub telemetry: TextLayoutTelemetry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLayoutTelemetry {
    pub font_resolution: FontResolutionTelemetry,
    pub text_box_rect: [f32; 4],
    pub line_height: f32,
    pub source_rect_union: Option<[f32; 4]>,
    pub line_boxes: Vec<LineLayoutTelemetry>,
    pub glyphs: Vec<GlyphLayoutTelemetry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineLayoutTelemetry {
    pub line_index: usize,
    pub char_start: usize,
    pub char_end: usize,
    pub baseline: f32,
    pub line_width: f32,
    pub line_height: f32,
    pub glyph_count: usize,
    pub line_box: [f32; 4],
    pub line_box_normalized: [f32; 4],
    pub glyph_bbox: Option<[f32; 4]>,
    pub glyph_bbox_normalized: Option<[f32; 4]>,
    pub text_box_rect: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphLayoutTelemetry {
    pub character: String,
    pub font_path: Option<PathBuf>,
    pub font_family: Option<String>,
    pub font_style: Option<String>,
    pub font_postscript_name: Option<String>,
    pub font_fallback: bool,
    pub font_resolution_source: FontResolutionSource,
    pub font_size: f32,
    pub font_units_per_em: Option<u16>,
    pub font_glyph_id: u32,
    pub glyph_run_index: usize,
    pub char_index: usize,
    pub word_index: usize,
    pub line_index: usize,
    pub advance: f32,
    pub advance_x: f32,
    pub advance_y: f32,
    pub advance_design_units: Option<u16>,
    pub advance_fixed16: Option<i64>,
    pub bbox_design_units: Option<[i16; 4]>,
    pub bbox_scaled: Option<[f32; 4]>,
    pub bbox: [f32; 4],
    pub cooltype_bbox_minmax: [f32; 4],
    pub bbox_center: [f32; 2],
    pub bbox_normalized: [f32; 4],
    pub bbox_center_normalized: [f32; 2],
    pub baseline: f32,
    pub baseline_delta: Option<[f32; 2]>,
    pub line_width: f32,
    pub text_box_rect: [f32; 4],
    pub metric_source: String,
    pub cooltype_reference_status: String,
}

pub fn layout_text(req: &TextLayoutRequest) -> anyhow::Result<TextLayoutResult> {
    let (font, font_resolution) = load_font_with_telemetry(&req.font_id)?;
    Ok(layout_with_font(req, &font, font_resolution))
}

pub fn layout_text_stub(req: &TextLayoutRequest) -> TextLayoutResult {
    let box_rect = req.box_rect.unwrap_or([0.0, 0.0, f32::MAX, f32::MAX]);
    let font_resolution = missing_font_resolution(&req.font_id);
    let mut glyphs = Vec::new();
    let mut telemetry_glyphs = Vec::new();
    let mut telemetry_line_boxes = Vec::new();
    let line_height = req.font_size;
    let origin_x = req.box_rect.map(|r| r[0]).unwrap_or(0.0);
    let origin_y = req.box_rect.map(|r| r[1]).unwrap_or(0.0);
    let mut word_index = 0usize;
    let mut char_offset = 0usize;
    let mut seen_word = false;

    for (line_index, line) in req.text.split('\n').enumerate() {
        let mut x = origin_x;
        let baseline = origin_y + req.font_size + line_index as f32 * line_height;
        let y = baseline - req.font_size;
        let line_width = line
            .chars()
            .map(|ch| stub_glyph_advance(ch, req.font_size))
            .sum();
        let mut line_glyph_bbox = None;
        let mut glyph_count = 0usize;
        let mut in_word = false;

        for (local_index, ch) in line.chars().enumerate() {
            let is_word_char = !ch.is_whitespace();
            if is_word_char && !in_word {
                if seen_word {
                    word_index += 1;
                }
                in_word = true;
                seen_word = true;
            } else if !is_word_char {
                in_word = false;
            }

            let char_index = char_offset + local_index;
            let advance = stub_glyph_advance(ch, req.font_size);
            let glyph_run_index = glyphs.len();
            let glyph = GlyphInstance {
                glyph_id: ch as u32,
                char_index,
                word_index,
                line_index,
                x,
                y,
                advance,
                bbox: [x, y, advance, req.font_size],
            };
            line_glyph_bbox = Some(union_optional_bbox(line_glyph_bbox, glyph.bbox));
            glyph_count += 1;
            let bbox_center = glyph_bbox_center(glyph.bbox);
            telemetry_glyphs.push(GlyphLayoutTelemetry {
                character: ch.to_string(),
                font_path: font_resolution.resolved_path.clone(),
                font_family: font_resolution.resolved_family.clone(),
                font_style: font_resolution.resolved_style.clone(),
                font_postscript_name: font_resolution.resolved_postscript_name.clone(),
                font_fallback: font_resolution.fallback,
                font_resolution_source: font_resolution.source.clone(),
                font_size: req.font_size,
                font_units_per_em: None,
                font_glyph_id: glyph.glyph_id,
                glyph_run_index,
                char_index,
                word_index,
                line_index,
                advance,
                advance_x: advance,
                advance_y: 0.0,
                advance_design_units: None,
                advance_fixed16: None,
                bbox_design_units: None,
                bbox_scaled: None,
                bbox: glyph.bbox,
                cooltype_bbox_minmax: bbox_to_minmax(glyph.bbox),
                bbox_center,
                bbox_normalized: normalize_bbox_to_text_box(glyph.bbox, box_rect),
                bbox_center_normalized: normalize_point_to_text_box(bbox_center, box_rect),
                baseline,
                baseline_delta: None,
                line_width,
                text_box_rect: box_rect,
                metric_source: "stub".to_string(),
                cooltype_reference_status: "not_cooltype_verified".to_string(),
            });
            glyphs.push(glyph);
            x += advance;
        }

        telemetry_line_boxes.push(line_layout_telemetry(
            line_index,
            char_offset,
            char_offset + line.chars().count(),
            baseline,
            origin_x,
            line_width,
            line_height,
            glyph_count,
            line_glyph_bbox,
            box_rect,
        ));
        char_offset += line.chars().count() + 1;
    }
    let source_rect_union = union_source_rect(&telemetry_glyphs);

    TextLayoutResult {
        glyphs,
        telemetry: TextLayoutTelemetry {
            font_resolution,
            text_box_rect: box_rect,
            line_height,
            source_rect_union,
            line_boxes: telemetry_line_boxes,
            glyphs: telemetry_glyphs,
        },
    }
}

fn layout_with_font(
    req: &TextLayoutRequest,
    font: &Font,
    font_resolution: FontResolutionTelemetry,
) -> TextLayoutResult {
    let metric_bytes = font_resolution
        .resolved_path
        .as_ref()
        .and_then(|path| fs::read(path).ok());
    let cooltype_face = metric_bytes
        .as_deref()
        .and_then(|bytes| Face::parse(bytes, 0).ok());
    let box_rect = req.box_rect.unwrap_or([0.0, 0.0, f32::MAX, f32::MAX]);
    let lines: Vec<&str> = req.text.split('\n').collect();
    let lines = if lines.is_empty() { vec![""] } else { lines };
    let line_height = font_line_height(font, req.font_size);
    let mut baseline = first_baseline(box_rect, line_height, lines.len());
    let mut glyphs = Vec::new();
    let mut telemetry_glyphs = Vec::new();
    let mut telemetry_line_boxes = Vec::new();
    let mut char_offset = 0usize;
    let mut word_index = 0usize;
    let mut in_word = false;
    let mut seen_word = false;

    for (line_index, line) in lines.iter().enumerate() {
        let render_line_width = measure_line_fontdue(font, line, req.font_size);
        let metric_line_width =
            measure_line_with_face(font, cooltype_face.as_ref(), line, req.font_size);
        let source_rect_metrics = metric_bytes.as_deref().and_then(|bytes| {
            source_rect_line_metrics(bytes, cooltype_face.as_ref(), line, req.font_size)
        });
        let telemetry_line_width = source_rect_metrics
            .as_ref()
            .map(|metrics| metrics.line.width)
            .unwrap_or(metric_line_width);
        let render_line_origin_x = line_start_x(box_rect, render_line_width);
        let metric_line_origin_x = line_start_x(box_rect, telemetry_line_width);
        let mut render_pen_x = render_line_origin_x;
        let mut metric_pen_x = line_start_x(box_rect, metric_line_width);
        let mut line_glyph_bbox = None;
        let mut glyph_count = 0usize;

        for (local_index, ch) in line.chars().enumerate() {
            let is_word_char = !ch.is_whitespace();
            if is_word_char && !in_word {
                if seen_word {
                    word_index += 1;
                }
                in_word = true;
                seen_word = true;
            } else if !is_word_char {
                in_word = false;
            }

            let metrics = glyph_metrics(
                font,
                cooltype_face.as_ref(),
                ch,
                req.font_size,
                metric_pen_x,
                baseline,
            );
            let render_advance = glyph_advance(font, ch, req.font_size);
            let render_bbox = glyph_bbox(
                font,
                ch,
                req.font_size,
                render_pen_x,
                baseline,
                render_advance,
            );
            let font_glyph_id = metrics.font_glyph_id;
            let bbox = metrics.bbox;
            let telemetry_bbox = source_rect_metrics
                .as_ref()
                .and_then(|source| {
                    let before = source.prefix_widths.get(local_index).copied()?;
                    let through = source.prefix_widths.get(local_index + 1).copied()?;
                    let advance = (through - before).max(0.0);
                    Some([
                        metric_line_origin_x + before,
                        baseline + source.line.top,
                        advance,
                        source.line.height.max(0.0),
                    ])
                })
                .unwrap_or(bbox);
            let telemetry_advance = telemetry_bbox[2].max(0.0);
            let bbox_center = glyph_bbox_center(telemetry_bbox);
            let glyph_run_index = glyphs.len();
            let glyph = GlyphInstance {
                glyph_id: font_glyph_id as u32,
                char_index: char_offset + local_index,
                word_index,
                line_index,
                x: render_pen_x,
                y: render_bbox[1],
                advance: render_advance,
                bbox: render_bbox,
            };
            line_glyph_bbox = union_visible_optional_bbox(line_glyph_bbox, telemetry_bbox);
            glyph_count += 1;
            telemetry_glyphs.push(GlyphLayoutTelemetry {
                character: ch.to_string(),
                font_path: font_resolution.resolved_path.clone(),
                font_family: font_resolution.resolved_family.clone(),
                font_style: font_resolution.resolved_style.clone(),
                font_postscript_name: font_resolution.resolved_postscript_name.clone(),
                font_fallback: font_resolution.fallback,
                font_resolution_source: font_resolution.source.clone(),
                font_size: req.font_size,
                font_units_per_em: metrics.font_units_per_em,
                font_glyph_id,
                glyph_run_index,
                char_index: glyph.char_index,
                word_index,
                line_index,
                advance: telemetry_advance,
                advance_x: telemetry_advance,
                advance_y: 0.0,
                advance_design_units: metrics.advance_design_units,
                advance_fixed16: metrics.advance_fixed16,
                bbox_design_units: metrics.bbox_design_units,
                bbox_scaled: metrics.bbox_scaled,
                bbox: telemetry_bbox,
                cooltype_bbox_minmax: bbox_to_minmax(telemetry_bbox),
                bbox_center,
                bbox_normalized: normalize_bbox_to_text_box(telemetry_bbox, box_rect),
                bbox_center_normalized: normalize_point_to_text_box(bbox_center, box_rect),
                baseline,
                baseline_delta: None,
                line_width: telemetry_line_width,
                text_box_rect: box_rect,
                metric_source: metrics.metric_source.to_string(),
                cooltype_reference_status: metrics.cooltype_reference_status.to_string(),
            });
            glyphs.push(glyph);
            render_pen_x += render_advance;
            metric_pen_x += metrics.advance;
        }

        telemetry_line_boxes.push(line_layout_telemetry(
            line_index,
            char_offset,
            char_offset + line.chars().count(),
            baseline,
            metric_line_origin_x,
            telemetry_line_width,
            line_height,
            glyph_count,
            line_glyph_bbox,
            box_rect,
        ));
        char_offset += line.chars().count() + 1;
        in_word = false;
        baseline += line_height;
    }
    let source_rect_union = union_source_rect(&telemetry_glyphs);

    TextLayoutResult {
        glyphs,
        telemetry: TextLayoutTelemetry {
            font_resolution,
            text_box_rect: box_rect,
            line_height,
            source_rect_union,
            line_boxes: telemetry_line_boxes,
            glyphs: telemetry_glyphs,
        },
    }
}

pub(crate) fn font_line_height(_font: &Font, font_size: f32) -> f32 {
    font_size * 1.2
}

fn measure_line_with_face(font: &Font, face: Option<&Face<'_>>, line: &str, font_size: f32) -> f32 {
    line.chars()
        .map(|ch| glyph_advance_with_face(font, face, ch, font_size))
        .sum()
}

fn measure_line_fontdue(font: &Font, line: &str, font_size: f32) -> f32 {
    line.chars()
        .map(|ch| glyph_advance(font, ch, font_size))
        .sum()
}

fn source_rect_line_metrics(
    font_bytes: &[u8],
    face: Option<&Face<'_>>,
    line: &str,
    font_size: f32,
) -> Option<SourceRectLineMetrics> {
    let face = face?;
    let line_measure = measure_source_rect(font_bytes, face, line, font_size)?;
    let char_count = line.chars().count();
    let mut prefix_widths = Vec::with_capacity(char_count + 1);
    prefix_widths.push(0.0);
    for end in 1..=char_count {
        let prefix = line.chars().take(end).collect::<String>();
        let width = measure_source_rect(font_bytes, face, &prefix, font_size)
            .map(|measure| measure.width)
            .unwrap_or(0.0);
        prefix_widths.push(width);
    }
    Some(SourceRectLineMetrics {
        line: line_measure,
        prefix_widths,
    })
}

fn measure_source_rect(
    font_bytes: &[u8],
    face: &Face<'_>,
    text: &str,
    font_size: f32,
) -> Option<SourceRectMeasure> {
    let buzz_face = BuzzFace::from_slice(font_bytes, 0)?;
    let units_per_em = face.units_per_em() as f32;
    let scale = font_size / units_per_em;
    let mut buffer = UnicodeBuffer::new();
    buffer.set_cluster_level(BufferClusterLevel::Characters);
    for (index, ch) in text.chars().enumerate() {
        buffer.add(ch, index as u32);
    }
    buffer.guess_segment_properties();
    let glyphs = rustybuzz::shape(&buzz_face, &[], buffer);
    let mut pen_x = 0.0f32;
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
        let glyph_id = GlyphId(info.glyph_id as u16);
        if let Some(rect) = face.glyph_bounding_box(glyph_id) {
            let bbox = ttf_rect_to_scaled_cooltype_bbox(rect, scale);
            let x_offset = position.x_offset as f32 * scale;
            let y_offset = -(position.y_offset as f32) * scale;
            let x0 = pen_x + x_offset + bbox[0];
            let y0 = y_offset + bbox[1];
            let x1 = pen_x + x_offset + bbox[2];
            let y1 = y_offset + bbox[3];
            min_x = min_x.min(x0);
            min_y = min_y.min(y0);
            max_x = max_x.max(x1);
            max_y = max_y.max(y1);
        }
        pen_x += position.x_advance as f32 * scale;
    }

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Some(SourceRectMeasure {
            width: (max_x - min_x).max(0.0),
            top: min_y,
            height: (max_y - min_y).max(0.0),
        })
    } else {
        Some(SourceRectMeasure {
            width: 0.0,
            top: 0.0,
            height: 0.0,
        })
    }
}

pub(crate) fn first_baseline(box_rect: [f32; 4], line_height: f32, line_count: usize) -> f32 {
    let block_offset = line_height * line_count.saturating_sub(1) as f32 * 0.5;
    box_rect[1] + box_rect[3] * 0.5 - block_offset
}

pub(crate) fn line_start_x(box_rect: [f32; 4], line_width: f32) -> f32 {
    box_rect[0] + (box_rect[2] - line_width) * 0.5
}

fn stub_glyph_advance(ch: char, font_size: f32) -> f32 {
    if ch == '\t' {
        font_size * 2.0
    } else {
        font_size * 0.6
    }
}

pub(crate) fn glyph_advance(font: &Font, ch: char, font_size: f32) -> f32 {
    if ch == '\t' {
        return font_size * 2.0;
    }
    if ch.is_whitespace() {
        return font
            .metrics_indexed(font.lookup_glyph_index(' '), font_size)
            .advance_width
            .max(0.0);
    }
    font.metrics_indexed(font.lookup_glyph_index(ch), font_size)
        .advance_width
}

fn glyph_advance_with_face(font: &Font, face: Option<&Face<'_>>, ch: char, font_size: f32) -> f32 {
    if ch == '\t' {
        return font_size * 2.0;
    }
    let Some(face) = face else {
        return glyph_advance(font, ch, font_size);
    };
    let Some(glyph_id) = face.glyph_index(ch) else {
        return glyph_advance(font, ch, font_size);
    };
    let units_per_em = face.units_per_em();
    let Some(advance_design_units) = face.glyph_hor_advance(glyph_id) else {
        return glyph_advance(font, ch, font_size);
    };
    advance_design_units as f32 * font_size / units_per_em as f32
}

#[derive(Debug, Clone)]
struct GlyphMetrics {
    font_glyph_id: u32,
    font_units_per_em: Option<u16>,
    advance: f32,
    advance_design_units: Option<u16>,
    advance_fixed16: Option<i64>,
    bbox: [f32; 4],
    bbox_design_units: Option<[i16; 4]>,
    bbox_scaled: Option<[f32; 4]>,
    metric_source: &'static str,
    cooltype_reference_status: &'static str,
}

#[derive(Debug, Clone)]
struct SourceRectMeasure {
    width: f32,
    top: f32,
    height: f32,
}

#[derive(Debug, Clone)]
struct SourceRectLineMetrics {
    line: SourceRectMeasure,
    prefix_widths: Vec<f32>,
}

fn glyph_metrics(
    font: &Font,
    face: Option<&Face<'_>>,
    ch: char,
    font_size: f32,
    pen_x: f32,
    baseline: f32,
) -> GlyphMetrics {
    if let Some(metrics) = cooltype_glyph_metrics(face, ch, font_size, pen_x, baseline) {
        return metrics;
    }
    let advance = glyph_advance(font, ch, font_size);
    let font_glyph_id = font.lookup_glyph_index(ch) as u32;
    GlyphMetrics {
        font_glyph_id,
        font_units_per_em: None,
        advance,
        advance_design_units: None,
        advance_fixed16: None,
        bbox: glyph_bbox(font, ch, font_size, pen_x, baseline, advance),
        bbox_design_units: None,
        bbox_scaled: None,
        metric_source: "fontdue",
        cooltype_reference_status: "not_cooltype_verified",
    }
}

fn cooltype_glyph_metrics(
    face: Option<&Face<'_>>,
    ch: char,
    font_size: f32,
    pen_x: f32,
    baseline: f32,
) -> Option<GlyphMetrics> {
    let face = face?;
    let glyph_id = face.glyph_index(ch)?;
    let units_per_em = face.units_per_em();
    let scale = font_size / units_per_em as f32;
    let advance_design_units = face.glyph_hor_advance(glyph_id).unwrap_or(0);
    let advance = advance_design_units as f32 * scale;
    let advance_fixed16 = Some((advance_design_units as i64) << 16);
    let (bbox, bbox_design_units, bbox_scaled) =
        if let Some(rect) = face.glyph_bounding_box(glyph_id) {
            let bbox_scaled = ttf_rect_to_scaled_cooltype_bbox(rect, scale);
            (
                [
                    pen_x + bbox_scaled[0],
                    baseline + bbox_scaled[1],
                    (bbox_scaled[2] - bbox_scaled[0]).max(0.0),
                    (bbox_scaled[3] - bbox_scaled[1]).max(0.0),
                ],
                Some([rect.x_min, rect.y_min, rect.x_max, rect.y_max]),
                Some(bbox_scaled),
            )
        } else {
            ([pen_x, baseline, 0.0, 0.0], None, None)
        };
    Some(GlyphMetrics {
        font_glyph_id: glyph_id_to_u32(glyph_id),
        font_units_per_em: Some(units_per_em),
        advance,
        advance_design_units: Some(advance_design_units),
        advance_fixed16,
        bbox,
        bbox_design_units,
        bbox_scaled,
        metric_source: "cooltype_shaped",
        cooltype_reference_status: "cooltype_metric_verified",
    })
}

fn ttf_rect_to_scaled_cooltype_bbox(rect: TtfRect, scale: f32) -> [f32; 4] {
    [
        rect.x_min as f32 * scale,
        -(rect.y_max as f32) * scale,
        rect.x_max as f32 * scale,
        -(rect.y_min as f32) * scale,
    ]
}

fn glyph_id_to_u32(glyph_id: GlyphId) -> u32 {
    glyph_id.0 as u32
}

fn glyph_bbox(
    font: &Font,
    ch: char,
    font_size: f32,
    pen_x: f32,
    baseline: f32,
    advance: f32,
) -> [f32; 4] {
    if ch.is_whitespace() {
        return [pen_x, baseline - font_size, advance, font_size];
    }
    let metrics = font.metrics_indexed(font.lookup_glyph_index(ch), font_size);
    [
        pen_x + metrics.xmin as f32,
        baseline - metrics.ymin as f32 - metrics.height as f32,
        metrics.width as f32,
        metrics.height as f32,
    ]
}

fn union_source_rect(glyphs: &[GlyphLayoutTelemetry]) -> Option<[f32; 4]> {
    glyphs
        .iter()
        .filter(|glyph| glyph.bbox[2] > 0.0 && glyph.bbox[3] > 0.0)
        .map(|glyph| glyph.bbox)
        .reduce(union_bbox)
}

fn glyph_bbox_center(bbox: [f32; 4]) -> [f32; 2] {
    [bbox[0] + bbox[2] * 0.5, bbox[1] + bbox[3] * 0.5]
}

fn bbox_to_minmax(bbox: [f32; 4]) -> [f32; 4] {
    [bbox[0], bbox[1], bbox[0] + bbox[2], bbox[1] + bbox[3]]
}

#[allow(clippy::too_many_arguments)]
fn line_layout_telemetry(
    line_index: usize,
    char_start: usize,
    char_end: usize,
    baseline: f32,
    line_start_x: f32,
    line_width: f32,
    line_height: f32,
    glyph_count: usize,
    glyph_bbox: Option<[f32; 4]>,
    text_box_rect: [f32; 4],
) -> LineLayoutTelemetry {
    let line_box = [
        line_start_x,
        baseline - line_height,
        line_width,
        line_height,
    ];
    LineLayoutTelemetry {
        line_index,
        char_start,
        char_end,
        baseline,
        line_width,
        line_height,
        glyph_count,
        line_box,
        line_box_normalized: normalize_bbox_to_text_box(line_box, text_box_rect),
        glyph_bbox,
        glyph_bbox_normalized: glyph_bbox
            .map(|bbox| normalize_bbox_to_text_box(bbox, text_box_rect)),
        text_box_rect,
    }
}

fn union_optional_bbox(current: Option<[f32; 4]>, next: [f32; 4]) -> [f32; 4] {
    if let Some(current) = current {
        union_bbox(current, next)
    } else {
        next
    }
}

fn union_visible_optional_bbox(current: Option<[f32; 4]>, next: [f32; 4]) -> Option<[f32; 4]> {
    if next[2] <= 0.0 || next[3] <= 0.0 {
        current
    } else if let Some(current) = current {
        Some(union_bbox(current, next))
    } else {
        Some(next)
    }
}

fn union_bbox(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let ax1 = a[0] + a[2];
    let ay1 = a[1] + a[3];
    let bx1 = b[0] + b[2];
    let by1 = b[1] + b[3];
    let x0 = a[0].min(b[0]);
    let y0 = a[1].min(b[1]);
    let x1 = ax1.max(bx1);
    let y1 = ay1.max(by1);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

fn normalize_bbox_to_text_box(bbox: [f32; 4], text_box_rect: [f32; 4]) -> [f32; 4] {
    [
        normalize_axis_to_text_box(bbox[0], text_box_rect[0], text_box_rect[2]),
        normalize_axis_to_text_box(bbox[1], text_box_rect[1], text_box_rect[3]),
        normalize_size_to_text_box(bbox[2], text_box_rect[2]),
        normalize_size_to_text_box(bbox[3], text_box_rect[3]),
    ]
}

fn normalize_point_to_text_box(point: [f32; 2], text_box_rect: [f32; 4]) -> [f32; 2] {
    [
        normalize_axis_to_text_box(point[0], text_box_rect[0], text_box_rect[2]),
        normalize_axis_to_text_box(point[1], text_box_rect[1], text_box_rect[3]),
    ]
}

fn normalize_axis_to_text_box(value: f32, origin: f32, extent: f32) -> f32 {
    if value.is_finite() && origin.is_finite() && valid_text_box_extent(extent) {
        (value - origin) / extent
    } else {
        0.0
    }
}

fn normalize_size_to_text_box(size: f32, extent: f32) -> f32 {
    if size.is_finite() && valid_text_box_extent(extent) {
        size / extent
    } else {
        0.0
    }
}

fn valid_text_box_extent(extent: f32) -> bool {
    extent.is_finite() && extent.abs() > f32::EPSILON
}

fn missing_font_resolution(font_id: &str) -> FontResolutionTelemetry {
    FontResolutionTelemetry {
        requested_id: font_id.to_string(),
        resolved_path: None,
        resolved_family: None,
        resolved_style: None,
        resolved_fullname: None,
        resolved_postscript_name: None,
        fallback: true,
        source: FontResolutionSource::Missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assert_approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_approx_eps(actual: f32, expected: f32, epsilon: f32) {
        assert!(
            (actual - expected).abs() < epsilon,
            "expected {expected}, got {actual}, epsilon {epsilon}"
        );
    }

    fn point_light_fixture() -> Option<PathBuf> {
        let path = PathBuf::from("fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf");
        path.exists().then_some(path)
    }

    fn montserrat_bolditalic_fixture() -> Option<PathBuf> {
        let path =
            PathBuf::from("fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf");
        path.exists().then_some(path)
    }

    #[test]
    fn stub_keeps_source_indices_across_lines() {
        let layout = layout_text_stub(&TextLayoutRequest {
            text: "AB\nC".to_string(),
            font_id: "missing".to_string(),
            font_size: 10.0,
            box_rect: Some([0.0, 0.0, 100.0, 40.0]),
        });

        assert_eq!(layout.glyphs.len(), 3);
        assert_eq!(layout.glyphs[0].char_index, 0);
        assert_eq!(layout.glyphs[2].char_index, 3);
        assert_eq!(layout.glyphs[2].line_index, 1);
        assert_eq!(layout.telemetry.line_boxes.len(), 2);
        assert_eq!(layout.telemetry.line_boxes[0].char_start, 0);
        assert_eq!(layout.telemetry.line_boxes[0].char_end, 2);
        assert_eq!(layout.telemetry.line_boxes[1].char_start, 3);
        assert_eq!(layout.telemetry.line_boxes[1].char_end, 4);
    }

    #[test]
    fn stub_reports_bbox_center_and_normalized_text_box_coordinates() {
        let layout = layout_text_stub(&TextLayoutRequest {
            text: "A".to_string(),
            font_id: "missing".to_string(),
            font_size: 20.0,
            box_rect: Some([10.0, 20.0, 100.0, 50.0]),
        });
        let glyph = &layout.telemetry.glyphs[0];

        assert_eq!(glyph.bbox, [10.0, 20.0, 12.0, 20.0]);
        assert_eq!(glyph.glyph_run_index, 0);
        assert_eq!(glyph.advance_x, 12.0);
        assert_eq!(glyph.advance_y, 0.0);
        assert_eq!(glyph.cooltype_bbox_minmax, [10.0, 20.0, 22.0, 40.0]);
        assert_eq!(glyph.baseline_delta, None);
        assert_eq!(glyph.metric_source, "stub");
        assert_eq!(glyph.cooltype_reference_status, "not_cooltype_verified");
        assert_eq!(glyph.bbox_center, [16.0, 30.0]);
        assert_eq!(
            layout.telemetry.line_boxes[0].line_box,
            [10.0, 20.0, 12.0, 20.0]
        );
        assert_eq!(layout.telemetry.line_boxes[0].glyph_bbox, Some(glyph.bbox));
        assert_approx(glyph.bbox_normalized[0], 0.0);
        assert_approx(glyph.bbox_normalized[1], 0.0);
        assert_approx(glyph.bbox_normalized[2], 0.12);
        assert_approx(glyph.bbox_normalized[3], 0.4);
        assert_approx(glyph.bbox_center_normalized[0], 0.06);
        assert_approx(glyph.bbox_center_normalized[1], 0.2);
    }

    #[test]
    fn real_layout_reports_words_lines_and_bounds() {
        let layout = layout_text(&TextLayoutRequest {
            text: "Hi all\nYo".to_string(),
            font_id: "DejaVu Sans".to_string(),
            font_size: 18.0,
            box_rect: Some([0.0, 0.0, 240.0, 80.0]),
        })
        .unwrap();

        assert!(layout.glyphs.len() >= 7);
        assert_eq!(layout.glyphs.first().unwrap().line_index, 0);
        assert_eq!(layout.glyphs.last().unwrap().line_index, 1);
        assert!(layout.glyphs.iter().any(|glyph| glyph.word_index > 0));
        assert!(
            layout
                .glyphs
                .iter()
                .filter(|glyph| glyph.bbox[2] > 0.0 && glyph.bbox[3] > 0.0)
                .count()
                >= 6
        );
        assert_eq!(layout.telemetry.glyphs.len(), layout.glyphs.len());
        assert_eq!(layout.telemetry.line_boxes.len(), 2);
        assert_eq!(layout.telemetry.line_boxes[0].line_index, 0);
        assert_eq!(layout.telemetry.line_boxes[1].line_index, 1);
        assert_eq!(layout.telemetry.line_boxes[0].char_start, 0);
        assert_eq!(layout.telemetry.line_boxes[0].char_end, 6);
        assert_eq!(layout.telemetry.line_boxes[1].char_start, 7);
        assert_eq!(layout.telemetry.line_boxes[1].char_end, 9);
        assert!(layout.telemetry.line_boxes[0].glyph_bbox.is_some());
        assert!(layout
            .telemetry
            .glyphs
            .iter()
            .any(|glyph| glyph.baseline > glyph.bbox[1]));
    }

    #[test]
    fn real_layout_centers_multiline_block_with_ae_auto_leading() {
        let Some(path) = point_light_fixture() else {
            return;
        };
        let font_size = 20.0;
        let layout = layout_text(&TextLayoutRequest {
            text: "A\nB".to_string(),
            font_id: path.display().to_string(),
            font_size,
            box_rect: Some([0.0, 0.0, 100.0, 80.0]),
        })
        .unwrap();

        assert_approx(layout.telemetry.line_height, font_size * 1.2);
        assert_approx(layout.telemetry.line_boxes[0].baseline, 28.0);
        assert_approx(layout.telemetry.line_boxes[1].baseline, 52.0);
    }

    #[test]
    fn space_advance_uses_font_metric_without_synthetic_floor() {
        let Some(path) = point_light_fixture() else {
            return;
        };
        let (font, _) = load_font_with_telemetry(path.to_str().unwrap()).unwrap();

        assert_approx(glyph_advance(&font, ' ', 64.0), 0.0);
    }

    #[test]
    fn real_layout_uses_font_glyph_indices_and_reports_point_telemetry() {
        let Some(path) = point_light_fixture() else {
            return;
        };
        let font_id = path.display().to_string();
        let (font, _) = load_font_with_telemetry(&font_id).unwrap();
        let layout = layout_text(&TextLayoutRequest {
            text: "Point".to_string(),
            font_id,
            font_size: 28.0,
            box_rect: Some([0.0, 0.0, 180.0, 60.0]),
        })
        .unwrap();

        let expected_glyph_id = font.lookup_glyph_index('P') as u32;
        assert_eq!(layout.glyphs[0].glyph_id, expected_glyph_id);
        assert_eq!(layout.telemetry.glyphs[0].font_glyph_id, expected_glyph_id);
        assert_eq!(layout.telemetry.glyphs[0].glyph_run_index, 0);
        assert_eq!(layout.telemetry.glyphs[0].advance_y, 0.0);
        assert_eq!(layout.telemetry.glyphs[0].baseline_delta, None);
        assert_eq!(layout.telemetry.glyphs[0].metric_source, "fontdue");
        assert_eq!(
            layout.telemetry.glyphs[0].cooltype_reference_status,
            "not_cooltype_verified"
        );
        assert_eq!(
            layout.telemetry.glyphs[0].font_path.as_deref(),
            Some(path.as_path())
        );
        assert_eq!(
            layout.telemetry.glyphs[0].font_resolution_source,
            FontResolutionSource::DirectPath
        );
        assert!(!layout.telemetry.glyphs[0].font_fallback);
        assert_eq!(layout.telemetry.glyphs[0].font_size, 28.0);
        if let Some(postscript_name) = &layout.telemetry.glyphs[0].font_postscript_name {
            assert_eq!(postscript_name, "Point-Light");
        }
        assert!(!layout.telemetry.font_resolution.fallback);
        assert_eq!(
            layout.telemetry.font_resolution.source,
            FontResolutionSource::DirectPath
        );
    }

    #[test]
    fn montserrat_layout_reports_cooltype_metrics_without_changing_render_bbox() {
        let Some(path) = montserrat_bolditalic_fixture() else {
            return;
        };
        let layout = layout_text(&TextLayoutRequest {
            text: "W".to_string(),
            font_id: path.display().to_string(),
            font_size: 58.0,
            box_rect: Some([0.0, 0.0, 512.0, 512.0]),
        })
        .unwrap();

        let glyph = &layout.telemetry.glyphs[0];
        assert_eq!(glyph.metric_source, "cooltype_shaped");
        assert_eq!(glyph.cooltype_reference_status, "cooltype_metric_verified");
        assert_eq!(glyph.font_units_per_em, Some(1000));
        assert_eq!(glyph.advance_design_units, Some(1147));
        assert_eq!(glyph.advance_fixed16, Some(1147_i64 << 16));
        assert_eq!(glyph.bbox_design_units, Some([99, 0, 1220, 700]));
        assert_approx(glyph.bbox_scaled.unwrap()[0], 5.742);
        assert_approx(glyph.bbox[2], 65.018);

        // Raster placement remains on the previous fontdue bitmap bbox until
        // CoolType raster coverage/glyph ids are probed deeply enough.
        assert_ne!(layout.glyphs[0].bbox, glyph.bbox);
        assert_approx(layout.glyphs[0].advance, glyph.advance);
    }

    #[test]
    fn montserrat_source_rect_rows_match_ae_prefix_partition() {
        let Some(path) = montserrat_bolditalic_fixture() else {
            return;
        };
        let layout = layout_text(&TextLayoutRequest {
            text: "WORD REVEAL\nMONTSERRAT TEST".to_string(),
            font_id: path.display().to_string(),
            font_size: 58.0,
            box_rect: Some([0.0, 0.0, 512.0, 512.0]),
        })
        .unwrap();
        let glyphs = &layout.telemetry.glyphs;

        assert_approx_eps(glyphs[0].advance, 65.018001, 0.001);
        assert_approx_eps(glyphs[1].advance, 42.282002, 0.001);
        assert_approx_eps(glyphs[4].advance, 0.0, 0.001);
        assert_approx_eps(glyphs[5].advance, 59.739991, 0.001);
        assert_approx_eps(glyphs[0].bbox[0], 28.466008, 0.001);
        assert_approx_eps(glyphs[0].bbox[1], 179.903999, 0.001);
        assert_approx_eps(glyphs[0].bbox[3], 41.992001, 0.001);
    }

    #[test]
    fn real_layout_centers_overfull_point_text_around_text_box() {
        let Some(path) = point_light_fixture() else {
            return;
        };
        let layout = layout_text(&TextLayoutRequest {
            text: "GLYPH MOTION".to_string(),
            font_id: path.display().to_string(),
            font_size: 74.0,
            box_rect: Some([0.0, 0.0, 512.0, 512.0]),
        })
        .unwrap();

        let first = layout.glyphs.first().unwrap();
        let last = layout.glyphs.last().unwrap();
        assert!(layout.telemetry.glyphs[0].line_width > 512.0);
        assert_eq!(layout.telemetry.line_boxes.len(), 1);
        assert!(layout.telemetry.line_boxes[0].line_width > 512.0);
        assert!(layout.telemetry.line_boxes[0].line_box[0] < 0.0);
        assert!(first.bbox[0] < 0.0);
        assert!(last.bbox[0] + last.bbox[2] > 512.0);
        assert!((layout.telemetry.glyphs[0].baseline - 256.0).abs() < 0.001);
        assert!((layout.telemetry.line_boxes[0].baseline - 256.0).abs() < 0.001);
    }
}
