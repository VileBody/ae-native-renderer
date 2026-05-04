use fontdue::Font;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub font_glyph_id: u32,
    pub char_index: usize,
    pub word_index: usize,
    pub line_index: usize,
    pub advance: f32,
    pub bbox: [f32; 4],
    pub bbox_center: [f32; 2],
    pub bbox_normalized: [f32; 4],
    pub bbox_center_normalized: [f32; 2],
    pub baseline: f32,
    pub line_width: f32,
    pub text_box_rect: [f32; 4],
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
                font_glyph_id: glyph.glyph_id,
                char_index,
                word_index,
                line_index,
                advance,
                bbox: glyph.bbox,
                bbox_center,
                bbox_normalized: normalize_bbox_to_text_box(glyph.bbox, box_rect),
                bbox_center_normalized: normalize_point_to_text_box(bbox_center, box_rect),
                baseline,
                line_width,
                text_box_rect: box_rect,
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

    TextLayoutResult {
        glyphs,
        telemetry: TextLayoutTelemetry {
            font_resolution,
            text_box_rect: box_rect,
            line_height,
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
    let box_rect = req.box_rect.unwrap_or([0.0, 0.0, f32::MAX, f32::MAX]);
    let lines: Vec<&str> = req.text.split('\n').collect();
    let lines = if lines.is_empty() { vec![""] } else { lines };
    let line_height = font_line_height(font, req.font_size);
    let mut baseline = first_baseline(box_rect);
    let mut glyphs = Vec::new();
    let mut telemetry_glyphs = Vec::new();
    let mut telemetry_line_boxes = Vec::new();
    let mut char_offset = 0usize;
    let mut word_index = 0usize;
    let mut in_word = false;
    let mut seen_word = false;

    for (line_index, line) in lines.iter().enumerate() {
        let line_width = measure_line(font, line, req.font_size);
        let line_origin_x = line_start_x(box_rect, line_width);
        let mut pen_x = line_origin_x;
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

            let advance = glyph_advance(font, ch, req.font_size);
            let font_glyph_id = font.lookup_glyph_index(ch);
            let bbox = glyph_bbox(font, ch, req.font_size, pen_x, baseline, advance);
            let bbox_center = glyph_bbox_center(bbox);
            let glyph = GlyphInstance {
                glyph_id: font_glyph_id as u32,
                char_index: char_offset + local_index,
                word_index,
                line_index,
                x: pen_x,
                y: bbox[1],
                advance,
                bbox,
            };
            line_glyph_bbox = Some(union_optional_bbox(line_glyph_bbox, bbox));
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
                font_glyph_id: font_glyph_id as u32,
                char_index: glyph.char_index,
                word_index,
                line_index,
                advance,
                bbox,
                bbox_center,
                bbox_normalized: normalize_bbox_to_text_box(bbox, box_rect),
                bbox_center_normalized: normalize_point_to_text_box(bbox_center, box_rect),
                baseline,
                line_width,
                text_box_rect: box_rect,
            });
            glyphs.push(glyph);
            pen_x += advance;
        }

        telemetry_line_boxes.push(line_layout_telemetry(
            line_index,
            char_offset,
            char_offset + line.chars().count(),
            baseline,
            line_origin_x,
            line_width,
            line_height,
            glyph_count,
            line_glyph_bbox,
            box_rect,
        ));
        char_offset += line.chars().count() + 1;
        in_word = false;
        baseline += line_height;
    }

    TextLayoutResult {
        glyphs,
        telemetry: TextLayoutTelemetry {
            font_resolution,
            text_box_rect: box_rect,
            line_height,
            line_boxes: telemetry_line_boxes,
            glyphs: telemetry_glyphs,
        },
    }
}

pub(crate) fn font_line_height(font: &Font, font_size: f32) -> f32 {
    font.horizontal_line_metrics(font_size)
        .map(|metrics| metrics.new_line_size.abs().max(font_size))
        .unwrap_or(font_size * 1.2)
}

pub(crate) fn measure_line(font: &Font, line: &str, font_size: f32) -> f32 {
    line.chars()
        .map(|ch| glyph_advance(font, ch, font_size))
        .sum()
}

pub(crate) fn first_baseline(box_rect: [f32; 4]) -> f32 {
    box_rect[1] + box_rect[3] * 0.5
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
            .max(font_size * 0.3);
    }
    font.metrics_indexed(font.lookup_glyph_index(ch), font_size)
        .advance_width
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

fn glyph_bbox_center(bbox: [f32; 4]) -> [f32; 2] {
    [bbox[0] + bbox[2] * 0.5, bbox[1] + bbox[3] * 0.5]
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
    let line_box = [line_start_x, baseline - line_height, line_width, line_height];
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

    fn point_light_fixture() -> Option<PathBuf> {
        let path = PathBuf::from("fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf");
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
        assert_eq!(glyph.bbox_center, [16.0, 30.0]);
        assert_eq!(layout.telemetry.line_boxes[0].line_box, [10.0, 20.0, 12.0, 20.0]);
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
