use raster_cpu::Canvas;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex, OnceLock};
use ttf_parser::{Face, GlyphId, OutlineBuilder, Tag};

use crate::{
    layout_text, load_font_with_telemetry, p6_ad68_flag_trace_payload, p6_ad68_opt_in_flag_state,
    try_rasterize_text_with_p6_ad68, P6Ad68OptInFlagState, P6Ad68PathPoint, P6Ad68PathSegment,
    P6Ad68RawPathSegment, P6Ad68TextPathInput, P6Ad68TextRouteReport, TextLayoutRequest,
    TextLayoutResult,
};

const OUTLINE_COVERAGE_SUPERSAMPLE: u32 = 16;
const TEXT_RASTER_JOURNAL_ENV_VAR: &str = "AE_NATIVE_RENDERER_TEXT_RASTER_JOURNAL";

fn text_raster_journal_enabled() -> bool {
    std::env::var(TEXT_RASTER_JOURNAL_ENV_VAR)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn push_text_raster_event(
    events: &mut Vec<TextRasterEvent>,
    seq: &mut u64,
    stage: &'static str,
    glyph_run_index: Option<usize>,
    glyph_id: Option<u32>,
    payload: Value,
) {
    let input = json!({
        "stage": stage,
        "seq": *seq,
        "glyph_run_index": glyph_run_index,
        "glyph_id": glyph_id
    });
    let output_hash = sha256_json(&payload);
    events.push(TextRasterEvent {
        schema: "ae-native-renderer.text-raster-event.v1".to_string(),
        seq: *seq,
        case_id: None,
        frame: None,
        layer_id: None,
        glyph_run_index,
        glyph_id,
        stage: stage.to_string(),
        payload,
        input_hash: sha256_json(&input),
        output_hash,
    });
    *seq += 1;
}

fn push_p6_ad68_opt_in_flag_event(
    events: &mut Vec<TextRasterEvent>,
    seq: &mut u64,
    flag_state: P6Ad68OptInFlagState,
) {
    push_text_raster_event(
        events,
        seq,
        "p6_ad68_opt_in_flag",
        None,
        None,
        p6_ad68_flag_trace_payload(flag_state),
    );
}

fn push_p6_ad68_route_event(
    events: &mut Vec<TextRasterEvent>,
    seq: &mut u64,
    report: &P6Ad68TextRouteReport,
) {
    push_text_raster_event(
        events,
        seq,
        "p6_ad68_opt_in_route",
        Some(report.glyph_run_index),
        Some(report.glyph_id),
        report.trace_payload(),
    );
}

fn sha256_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_else(|_| b"null".to_vec());
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    bytes_hex(&hasher.finalize())
}

fn f32_trace(value: f32) -> Value {
    json!({
        "value": value,
        "f32_bits_hex": format!("{:08x}", value.to_bits())
    })
}

fn point_trace(point: Point) -> Value {
    json!({
        "x": f32_trace(point.x),
        "y": f32_trace(point.y)
    })
}

fn scaled_point_trace(point: Point, scale: f32) -> Value {
    point_trace(Point {
        x: point.x * scale,
        y: -point.y * scale,
    })
}

fn p6_ad68_path_segments_for_text(
    path_segments: &[RawPathSegment],
    scale: f32,
    x_min: f32,
    y_max: f32,
    supersample: u32,
) -> Vec<P6Ad68RawPathSegment> {
    let ss = supersample.max(1) as i32;
    path_segments
        .iter()
        .copied()
        .map(|raw| {
            let segment = match raw.segment {
                AePathSegment::Line { start, end } => P6Ad68PathSegment::Line {
                    start: p6_ad68_path_point(device_point(start, scale, x_min, y_max, ss)),
                    end: p6_ad68_path_point(device_point(end, scale, x_min, y_max, ss)),
                },
                AePathSegment::Cubic {
                    start,
                    control1,
                    control2,
                    end,
                } => P6Ad68PathSegment::Cubic {
                    start: p6_ad68_path_point(device_point(start, scale, x_min, y_max, ss)),
                    control1: p6_ad68_path_point(device_point(control1, scale, x_min, y_max, ss)),
                    control2: p6_ad68_path_point(device_point(control2, scale, x_min, y_max, ss)),
                    end: p6_ad68_path_point(device_point(end, scale, x_min, y_max, ss)),
                },
            };
            P6Ad68RawPathSegment {
                contour_index: raw.contour_index,
                segment_index: raw.segment_index,
                segment,
            }
        })
        .collect()
}

fn p6_ad68_path_point(point: Point) -> P6Ad68PathPoint {
    P6Ad68PathPoint::new(point.x, point.y)
}

fn path_segment_trace(
    segment: RawPathSegment,
    scale: f32,
    x_min: f32,
    y_max: f32,
    ss: i32,
) -> Value {
    match segment.segment {
        AePathSegment::Line { start, end } => json!({
            "segment_index": segment.segment_index,
            "contour_index": segment.contour_index,
            "kind": "line",
            "design": {
                "start": point_trace(start),
                "end": point_trace(end)
            },
            "are_local": {
                "start": scaled_point_trace(start, scale),
                "end": scaled_point_trace(end, scale)
            },
            "device": {
                "start": point_trace(device_point(start, scale, x_min, y_max, ss)),
                "end": point_trace(device_point(end, scale, x_min, y_max, ss))
            }
        }),
        AePathSegment::Cubic {
            start,
            control1,
            control2,
            end,
        } => json!({
            "segment_index": segment.segment_index,
            "contour_index": segment.contour_index,
            "kind": "cubic",
            "design": {
                "p0_start": point_trace(start),
                "p1_control": point_trace(control1),
                "p2_control": point_trace(control2),
                "p3_end": point_trace(end)
            },
            "are_local": {
                "p0_start": scaled_point_trace(start, scale),
                "p1_control": scaled_point_trace(control1, scale),
                "p2_control": scaled_point_trace(control2, scale),
                "p3_end": scaled_point_trace(end, scale)
            },
            "device": {
                "p0_start": point_trace(device_point(start, scale, x_min, y_max, ss)),
                "p1_control": point_trace(device_point(control1, scale, x_min, y_max, ss)),
                "p2_control": point_trace(device_point(control2, scale, x_min, y_max, ss)),
                "p3_end": point_trace(device_point(end, scale, x_min, y_max, ss))
            }
        }),
    }
}

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<TextRasterEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRasterEvent {
    pub schema: String,
    pub seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyph_run_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyph_id: Option<u32>,
    pub stage: String,
    pub payload: Value,
    pub input_hash: String,
    pub output_hash: String,
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
    let journal_enabled = text_raster_journal_enabled();
    let mut events = Vec::new();
    let mut event_seq = 0_u64;
    let outline_font_key = font_resolution
        .resolved_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| req.font_id.clone());
    if journal_enabled {
        push_text_raster_event(
            &mut events,
            &mut event_seq,
            "layout_input",
            None,
            None,
            json!({
                "text": &req.text,
                "font_id": &req.font_id,
                "font_size": f32_trace(req.font_size),
                "box_rect": req.box_rect,
                "canvas_size": [width, height],
                "glyph_count": layout.glyphs.len()
            }),
        );
        push_text_raster_event(
            &mut events,
            &mut event_seq,
            "layout_output",
            None,
            None,
            json!({
                "font_resolution": &layout.telemetry.font_resolution,
                "source_rect_union": layout.telemetry.source_rect_union,
                "line_count": layout.telemetry.line_boxes.len(),
                "glyphs": layout.glyphs.iter().map(|glyph| json!({
                    "glyph_id": glyph.glyph_id,
                    "char_index": glyph.char_index,
                    "word_index": glyph.word_index,
                    "line_index": glyph.line_index,
                    "x": f32_trace(glyph.x),
                    "y": f32_trace(glyph.y),
                    "advance": f32_trace(glyph.advance),
                    "bbox": glyph.bbox.iter().map(|value| f32_trace(*value)).collect::<Vec<_>>()
                })).collect::<Vec<_>>()
            }),
        );
    }
    let p6_ad68_flag_state = p6_ad68_opt_in_flag_state();
    if journal_enabled && p6_ad68_flag_state != P6Ad68OptInFlagState::DisabledDefault {
        push_p6_ad68_opt_in_flag_event(&mut events, &mut event_seq, p6_ad68_flag_state);
    }

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
        if p6_ad68_flag_state.enabled() {
            let p6_path_segments = p6_ad68_path_segments_for_text(
                coverage.path_segments.as_ref(),
                coverage.scale,
                coverage.x_min,
                coverage.y_max,
                coverage.supersample,
            );
            let report = try_rasterize_text_with_p6_ad68(&P6Ad68TextPathInput {
                glyph_run_index: run_index,
                glyph_id: glyph.glyph_id,
                path_segments: &p6_path_segments,
                expected_advance_only_space: ch.is_some_and(char::is_whitespace)
                    && glyph.advance.is_finite()
                    && glyph.advance > 0.0,
            });
            if journal_enabled {
                push_p6_ad68_route_event(&mut events, &mut event_seq, &report);
            }
        }
        if journal_enabled {
            let traced_segments = coverage
                .path_segments
                .iter()
                .copied()
                .map(|segment| {
                    path_segment_trace(
                        segment,
                        coverage.scale,
                        coverage.x_min,
                        coverage.y_max,
                        coverage.supersample.max(1) as i32,
                    )
                })
                .collect::<Vec<_>>();
            let cubic_calls = traced_segments
                .iter()
                .filter(|segment| segment.get("kind").and_then(Value::as_str) == Some("cubic"))
                .cloned()
                .collect::<Vec<_>>();
            let line_call_count = traced_segments
                .iter()
                .filter(|segment| segment.get("kind").and_then(Value::as_str) == Some("line"))
                .count();
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "raw_ttf_contour",
                Some(run_index),
                Some(glyph.glyph_id),
                json!({
                    "backend": coverage.backend,
                    "origin_source": coverage.origin_source,
                    "raster_origin": [f32_trace(coverage.raster_x), f32_trace(coverage.raster_y)],
                    "bitmap_size": [coverage.width, coverage.height],
                    "nonzero_pixels": coverage.nonzero_pixels,
                    "glyph_origin": [f32_trace(glyph.x), f32_trace(glyph.y)],
                    "baseline": f32_trace(baseline)
                }),
            );
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "quadratic_to_cubic_stream",
                Some(run_index),
                Some(glyph.glyph_id),
                json!({
                    "policy": "ttf_outline_source_path_capture_v1",
                    "segment_count": traced_segments.len(),
                    "cubic_call_count": cubic_calls.len(),
                    "line_call_count": line_call_count,
                    "scale": f32_trace(coverage.scale),
                    "x_min": f32_trace(coverage.x_min),
                    "y_max": f32_trace(coverage.y_max),
                    "supersample": coverage.supersample,
                    "cubic_calls": cubic_calls
                }),
            );
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "path_segment_emit",
                Some(run_index),
                Some(glyph.glyph_id),
                json!({
                    "segment_count": traced_segments.len(),
                    "segments": traced_segments,
                    "segment_policy": "ttf_outline_source_path_capture_v1",
                    "coverage_backend": coverage.backend
                }),
            );
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "scanline_event",
                Some(run_index),
                Some(glyph.glyph_id),
                json!({
                    "row_count": coverage_rows.len(),
                    "span_policy": "are_scanline_16x_negative_winding_merged_projected_ranges_v1",
                    "supersample": coverage.supersample
                }),
            );
            for row in &coverage_rows {
                push_text_raster_event(
                    &mut events,
                    &mut event_seq,
                    "row_span_emit",
                    Some(run_index),
                    Some(glyph.glyph_id),
                    json!({
                        "y": row.y,
                        "start_x": row.start_x,
                        "end_x": row.end_x,
                        "coverage_len": row.coverage_len,
                        "coverage_hash_fnv1a64": &row.coverage_hash_fnv1a64,
                        "coverage_sample_hex": &row.coverage_sample_hex,
                        "has_full_coverage_hex": row.coverage_hex.is_some()
                    }),
                );
            }
        }
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
        if journal_enabled {
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "draw_char_boundary",
                Some(run_index),
                Some(glyph.glyph_id),
                json!({
                    "char_index": plan.char_index,
                    "character": &plan.character,
                    "renderable": plan.renderable,
                    "will_draw": plan.will_draw,
                    "draw_fill": plan.draw_fill,
                    "draw_stroke": plan.draw_stroke,
                    "skip_reason": &plan.skip_reason,
                    "glyph_matrix": plan.glyph_matrix.iter().map(|value| f32_trace(*value)).collect::<Vec<_>>(),
                    "text_matrix": plan.text_matrix.iter().map(|value| f32_trace(*value)).collect::<Vec<_>>(),
                    "clipped_bounds_i32": plan.clipped_bounds_i32,
                    "coverage_backend": &plan.coverage_backend,
                    "coverage_rows": plan.coverage_rows.len()
                }),
            );
        }
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
            events,
        },
    ))
}

struct CoverageBitmap {
    width: usize,
    height: usize,
    raster_x: f32,
    raster_y: f32,
    bitmap: Arc<Vec<u8>>,
    path_segments: Arc<Vec<RawPathSegment>>,
    scale: f32,
    x_min: f32,
    y_max: f32,
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
        path_segments: Arc::new(Vec::new()),
        scale: 0.0,
        x_min: 0.0,
        y_max: 0.0,
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
    path_segments: Arc<Vec<RawPathSegment>>,
    nonzero_pixels: u32,
    supersample: u32,
    scale: f32,
}

impl CoverageMask {
    fn to_bitmap(&self, glyph_x: f32, baseline: f32) -> CoverageBitmap {
        CoverageBitmap {
            width: self.width,
            height: self.height,
            raster_x: glyph_x + self.x_min,
            raster_y: baseline - self.y_max,
            bitmap: self.bitmap.clone(),
            path_segments: self.path_segments.clone(),
            scale: self.scale,
            x_min: self.x_min,
            y_max: self.y_max,
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
    let bbox = face.glyph_bounding_box(glyph_id)?;
    let outline = ae_outline_from_simple_glyf(face, glyph_id).unwrap_or_else(|| {
        let mut outline = FlattenedOutline::default();
        let _ = face.outline_glyph(glyph_id, &mut outline);
        outline.finish_contour();
        outline
    });
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
    let path_segments = collect_path_segments(&outline);
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
        path_segments: Arc::new(path_segments),
        nonzero_pixels,
        supersample: ss,
        scale,
    })
}

#[derive(Debug, Copy, Clone)]
struct ProjectedScanlineEvent {
    x_min_fixed: i32,
    x_max_fixed: i32,
    winding_delta: i32,
}

#[derive(Debug, Copy, Clone)]
struct RawOutlinePoint {
    point: Point,
    on_curve: bool,
}

#[derive(Debug, Copy, Clone)]
struct RawPathSegment {
    contour_index: usize,
    segment_index: usize,
    segment: AePathSegment,
}

#[derive(Debug, Copy, Clone)]
enum AePathSegment {
    Line {
        start: Point,
        end: Point,
    },
    Cubic {
        start: Point,
        control1: Point,
        control2: Point,
        end: Point,
    },
}

fn collect_path_segments(outline: &FlattenedOutline) -> Vec<RawPathSegment> {
    let mut out = Vec::new();
    for (contour_index, contour) in outline.path_contours.iter().enumerate() {
        for (segment_index, segment) in contour.iter().copied().enumerate() {
            out.push(RawPathSegment {
                contour_index,
                segment_index,
                segment,
            });
        }
    }
    out
}

fn ae_outline_from_simple_glyf(face: &Face<'_>, glyph_id: GlyphId) -> Option<FlattenedOutline> {
    let glyph = raw_simple_glyph_contours(face, glyph_id)?;
    let mut contours = Vec::new();
    let mut path_contours = Vec::new();
    for contour in glyph {
        if contour.len() < 2 {
            continue;
        }
        let has_curve = contour.iter().any(|point| !point.on_curve);
        let (flattened, path_segments) = if has_curve {
            let stream = ae_curve_stream_from_reversed_tt(&contour);
            (flatten_ae_curve_stream(&stream), ae_cubic_segments(&stream))
        } else {
            let mut points = contour.iter().map(|point| point.point).collect::<Vec<_>>();
            // TXT_ARE_PathBuilder traces for line-only glyphs emit scaled TTF
            // contour points in reverse order.
            points.reverse();
            let segments = ae_line_segments(&points);
            (points, segments)
        };
        if flattened.len() >= 2 {
            contours.push(flattened);
            path_contours.push(path_segments);
        }
    }
    Some(FlattenedOutline {
        contours,
        path_contours,
        current: Vec::new(),
        current_point: Point::default(),
        current_has_curve: false,
    })
}

fn raw_simple_glyph_contours(
    face: &Face<'_>,
    glyph_id: GlyphId,
) -> Option<Vec<Vec<RawOutlinePoint>>> {
    let head = face.raw_face().table(Tag::from_bytes(b"head"))?;
    let maxp = face.raw_face().table(Tag::from_bytes(b"maxp"))?;
    let loca = face.raw_face().table(Tag::from_bytes(b"loca"))?;
    let glyf = face.raw_face().table(Tag::from_bytes(b"glyf"))?;
    let units = face.units_per_em();
    if units == 0 || maxp.len() < 6 || head.len() < 52 {
        return None;
    }
    let glyph_count = read_u16(maxp, 4)? as usize;
    let glyph_index = glyph_id.0 as usize;
    if glyph_index >= glyph_count {
        return None;
    }
    let long_loca = read_i16(head, 50)? != 0;
    let glyph_start = loca_offset(loca, glyph_index, long_loca)?;
    let glyph_end = loca_offset(loca, glyph_index + 1, long_loca)?;
    if glyph_start == glyph_end {
        return None;
    }
    let data = glyf.get(glyph_start..glyph_end)?;
    if data.len() < 10 {
        return None;
    }
    let contour_count = read_i16(data, 0)?;
    if contour_count <= 0 {
        return None;
    }
    parse_simple_glyf_contours(data.get(10..)?, contour_count as usize)
}

fn parse_simple_glyf_contours(
    data: &[u8],
    contour_count: usize,
) -> Option<Vec<Vec<RawOutlinePoint>>> {
    let mut offset = 0usize;
    let mut end_points = Vec::with_capacity(contour_count);
    for _ in 0..contour_count {
        end_points.push(read_u16(data, offset)? as usize);
        offset += 2;
    }
    let point_count = end_points.last().copied()? + 1;
    if point_count == 0 {
        return None;
    }
    let instruction_len = read_u16(data, offset)? as usize;
    offset = offset.checked_add(2 + instruction_len)?;
    if offset > data.len() {
        return None;
    }

    let mut flags = Vec::with_capacity(point_count);
    while flags.len() < point_count {
        let flag = *data.get(offset)?;
        offset += 1;
        let repeat = if flag & 0x08 != 0 {
            let count = *data.get(offset)? as usize + 1;
            offset += 1;
            count
        } else {
            1
        };
        for _ in 0..repeat {
            flags.push(flag);
            if flags.len() > point_count {
                return None;
            }
        }
    }

    let mut xs = Vec::with_capacity(point_count);
    let mut x = 0i16;
    for flag in &flags {
        let delta = if flag & 0x02 != 0 {
            let value = *data.get(offset)? as i16;
            offset += 1;
            if flag & 0x10 != 0 {
                value
            } else {
                -value
            }
        } else if flag & 0x10 != 0 {
            0
        } else {
            let value = read_i16(data, offset)?;
            offset += 2;
            value
        };
        x = x.checked_add(delta)?;
        xs.push(x);
    }

    let mut ys = Vec::with_capacity(point_count);
    let mut y = 0i16;
    for flag in &flags {
        let delta = if flag & 0x04 != 0 {
            let value = *data.get(offset)? as i16;
            offset += 1;
            if flag & 0x20 != 0 {
                value
            } else {
                -value
            }
        } else if flag & 0x20 != 0 {
            0
        } else {
            let value = read_i16(data, offset)?;
            offset += 2;
            value
        };
        y = y.checked_add(delta)?;
        ys.push(y);
    }

    let mut contours = Vec::with_capacity(contour_count);
    let mut start = 0usize;
    for end in end_points {
        if end < start || end >= point_count {
            return None;
        }
        let mut contour = Vec::with_capacity(end - start + 1);
        for index in start..=end {
            contour.push(RawOutlinePoint {
                point: Point {
                    x: xs[index] as f32,
                    y: ys[index] as f32,
                },
                on_curve: flags[index] & 0x01 != 0,
            });
        }
        contours.push(contour);
        start = end + 1;
    }
    Some(contours)
}

fn flatten_ae_curve_stream(stream: &[Point]) -> Vec<Point> {
    if stream.is_empty() {
        return Vec::new();
    }
    let mut flattened = vec![stream[0]];
    let mut index = 1usize;
    let mut current = stream[0];
    while index + 2 < stream.len() {
        let control1 = stream[index];
        let control2 = stream[index + 1];
        let end = stream[index + 2];
        flatten_cubic_points(current, control1, control2, end, &mut flattened);
        current = end;
        index += 3;
    }
    if flattened.len() >= 2 && distance(flattened[0], *flattened.last().unwrap()) < 0.0001 {
        flattened.pop();
    }
    flattened
}

fn ae_cubic_segments(stream: &[Point]) -> Vec<AePathSegment> {
    if stream.is_empty() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    let mut index = 1usize;
    let mut current = stream[0];
    while index + 2 < stream.len() {
        let control1 = stream[index];
        let control2 = stream[index + 1];
        let end = stream[index + 2];
        segments.push(AePathSegment::Cubic {
            start: current,
            control1,
            control2,
            end,
        });
        current = end;
        index += 3;
    }
    segments
}

fn ae_line_segments(points: &[Point]) -> Vec<AePathSegment> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut segments = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        segments.push(AePathSegment::Line {
            start: points[index],
            end: points[(index + 1) % points.len()],
        });
    }
    segments
}

fn ae_curve_stream_from_reversed_tt(contour: &[RawOutlinePoint]) -> Vec<Point> {
    if contour.is_empty() {
        return Vec::new();
    }
    let points = contour.iter().rev().copied().collect::<Vec<_>>();
    let (mut current, start_index) = if points[0].on_curve {
        (points[0].point, 1usize)
    } else if points.len() > 1 && !points[1].on_curve {
        (midpoint(points[0].point, points[1].point), 1usize)
    } else if points.len() > 1 {
        (points[1].point, 0usize)
    } else {
        (points[0].point, 0usize)
    };

    let mut segments = Vec::new();
    for offset in 0..points.len() {
        let raw = points[(start_index + offset) % points.len()];
        if raw.on_curve {
            current = raw.point;
            continue;
        }
        let next = points[(start_index + offset + 1) % points.len()];
        let end = if next.on_curve {
            next.point
        } else {
            midpoint(raw.point, next.point)
        };
        segments.push((current, raw.point, end));
        current = end;
    }
    if segments.is_empty() {
        return vec![current];
    }

    let mut stream = vec![segments[0].0];
    for (start, control, end) in segments {
        stream.push(Point {
            x: start.x + (2.0 / 3.0) * (control.x - start.x),
            y: start.y + (2.0 / 3.0) * (control.y - start.y),
        });
        stream.push(Point {
            x: end.x + (2.0 / 3.0) * (control.x - end.x),
            y: end.y + (2.0 / 3.0) * (control.y - end.y),
        });
        stream.push(end);
    }
    stream.push(stream[0]);
    stream
}

fn flatten_cubic_points(
    start: Point,
    control1: Point,
    control2: Point,
    end: Point,
    out: &mut Vec<Point>,
) {
    let steps = curve_steps(start, control1, end).max(curve_steps(start, control2, end));
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let mt = 1.0 - t;
        out.push(Point {
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

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    }
}

fn loca_offset(loca: &[u8], glyph_index: usize, long_loca: bool) -> Option<usize> {
    if long_loca {
        read_u32(loca, glyph_index.checked_mul(4)?)?.try_into().ok()
    } else {
        Some((read_u16(loca, glyph_index.checked_mul(2)?)? as usize) * 2)
    }
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
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
    if outline.path_contours.is_empty() {
        for contour in &outline.contours {
            if contour.len() < 2 {
                continue;
            }
            for index in 0..contour.len() {
                push_projected_line_event(
                    contour[index],
                    contour[(index + 1) % contour.len()],
                    scale,
                    x_min,
                    y_max,
                    strip_start,
                    strip_end,
                    ss,
                    &mut events,
                );
            }
        }
    } else {
        for contour in &outline.path_contours {
            for segment in contour {
                match *segment {
                    AePathSegment::Line { start, end } => push_projected_line_event(
                        start,
                        end,
                        scale,
                        x_min,
                        y_max,
                        strip_start,
                        strip_end,
                        ss,
                        &mut events,
                    ),
                    AePathSegment::Cubic {
                        start,
                        control1,
                        control2,
                        end,
                    } => push_projected_cubic_events(
                        start,
                        control1,
                        control2,
                        end,
                        scale,
                        x_min,
                        y_max,
                        strip_start,
                        strip_end,
                        ss,
                        &mut events,
                    ),
                }
            }
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

#[allow(clippy::too_many_arguments)]
fn push_projected_line_event(
    a: Point,
    b: Point,
    scale: f32,
    x_min: f32,
    y_max: f32,
    strip_start: f32,
    strip_end: f32,
    ss: i32,
    events: &mut Vec<ProjectedScanlineEvent>,
) {
    let ax = (a.x * scale - x_min) * ss as f32;
    let bx = (b.x * scale - x_min) * ss as f32;
    let ay = (y_max - a.y * scale) * ss as f32;
    let by = (y_max - b.y * scale) * ss as f32;
    let min_y = ay.min(by);
    let max_y = ay.max(by);
    if max_y <= strip_start || min_y >= strip_end || ay == by {
        return;
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

#[allow(clippy::too_many_arguments)]
fn push_projected_cubic_events(
    start: Point,
    control1: Point,
    control2: Point,
    end: Point,
    scale: f32,
    x_min: f32,
    y_max: f32,
    strip_start: f32,
    strip_end: f32,
    ss: i32,
    events: &mut Vec<ProjectedScanlineEvent>,
) {
    let p0 = device_point(start, scale, x_min, y_max, ss);
    let p1 = device_point(control1, scale, x_min, y_max, ss);
    let p2 = device_point(control2, scale, x_min, y_max, ss);
    let p3 = device_point(end, scale, x_min, y_max, ss);
    let mut splits = vec![0.0f32, 1.0];
    extend_cubic_derivative_roots(p0.x, p1.x, p2.x, p3.x, &mut splits);
    extend_cubic_derivative_roots(p0.y, p1.y, p2.y, p3.y, &mut splits);
    splits.sort_by(|a, b| a.total_cmp(b));
    splits.dedup_by(|a, b| (*a - *b).abs() < 0.000001);

    for window in splits.windows(2) {
        let t_start = window[0];
        let t_end = window[1];
        if t_end <= t_start {
            continue;
        }
        let a = cubic_point(p0, p1, p2, p3, t_start);
        let b = cubic_point(p0, p1, p2, p3, t_end);
        let min_y = a.y.min(b.y);
        let max_y = a.y.max(b.y);
        if max_y <= strip_start || min_y >= strip_end || a.y == b.y {
            continue;
        }

        let y0 = strip_start.clamp(min_y, max_y);
        let y1 = strip_end.clamp(min_y, max_y);
        let local0 = solve_monotonic_cubic_y(p0, p1, p2, p3, t_start, t_end, y0);
        let local1 = solve_monotonic_cubic_y(p0, p1, p2, p3, t_start, t_end, y1);
        let x0 = cubic_point(p0, p1, p2, p3, local0).x;
        let x1 = cubic_point(p0, p1, p2, p3, local1).x;
        events.push(ProjectedScanlineEvent {
            x_min_fixed: x0.min(x1).floor() as i32,
            x_max_fixed: x0.max(x1).floor() as i32,
            winding_delta: if cubic_point_y(start, control1, control2, end, t_end)
                > cubic_point_y(start, control1, control2, end, t_start)
            {
                1
            } else {
                -1
            },
        });
    }
}

fn device_point(point: Point, scale: f32, x_min: f32, y_max: f32, ss: i32) -> Point {
    Point {
        x: (point.x * scale - x_min) * ss as f32,
        y: (y_max - point.y * scale) * ss as f32,
    }
}

fn cubic_point(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
    let mt = 1.0 - t;
    Point {
        x: mt * mt * mt * p0.x
            + 3.0 * mt * mt * t * p1.x
            + 3.0 * mt * t * t * p2.x
            + t * t * t * p3.x,
        y: mt * mt * mt * p0.y
            + 3.0 * mt * mt * t * p1.y
            + 3.0 * mt * t * t * p2.y
            + t * t * t * p3.y,
    }
}

fn cubic_point_y(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> f32 {
    let mt = 1.0 - t;
    mt * mt * mt * p0.y + 3.0 * mt * mt * t * p1.y + 3.0 * mt * t * t * p2.y + t * t * t * p3.y
}

fn extend_cubic_derivative_roots(p0: f32, p1: f32, p2: f32, p3: f32, out: &mut Vec<f32>) {
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;
    if a.abs() < 1e-6 {
        if b.abs() >= 1e-6 {
            push_unit_root(-c / b, out);
        }
        return;
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return;
    }
    let root = discriminant.sqrt();
    push_unit_root((-b - root) / (2.0 * a), out);
    push_unit_root((-b + root) / (2.0 * a), out);
}

fn push_unit_root(value: f32, out: &mut Vec<f32>) {
    if value > 0.0 && value < 1.0 && value.is_finite() {
        out.push(value);
    }
}

fn solve_monotonic_cubic_y(
    p0: Point,
    p1: Point,
    p2: Point,
    p3: Point,
    t_start: f32,
    t_end: f32,
    target_y: f32,
) -> f32 {
    let y_start = cubic_point_y(p0, p1, p2, p3, t_start);
    let y_end = cubic_point_y(p0, p1, p2, p3, t_end);
    if (target_y - y_start).abs() < 0.000001 {
        return t_start;
    }
    if (target_y - y_end).abs() < 0.000001 {
        return t_end;
    }
    let increasing = y_end > y_start;
    let mut lo = t_start;
    let mut hi = t_end;
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        let y = cubic_point_y(p0, p1, p2, p3, mid);
        if (y < target_y) == increasing {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}

#[derive(Debug, Copy, Clone, Default)]
struct Point {
    x: f32,
    y: f32,
}

#[derive(Debug, Default)]
struct FlattenedOutline {
    contours: Vec<Vec<Point>>,
    path_contours: Vec<Vec<AePathSegment>>,
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
            self.path_contours.push(ae_line_segments(&contour));
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
    fn recovered_txt_curve_producer_matches_cov_o_prefix() {
        let Some(path) = montserrat_bolditalic_fixture() else {
            return;
        };
        let bytes = fs::read(path).unwrap();
        let face = Face::parse(&bytes, 0).unwrap();
        let contours = raw_simple_glyph_contours(&face, GlyphId(204)).unwrap();
        let stream = ae_curve_stream_from_reversed_tt(&contours[0]);
        let scale = 96.0 / face.units_per_em() as f32;
        let ae_space = |point: Point| [point.x * scale, -point.y * scale];

        let expected = [
            [54.576004, -1.9200001],
            [59.664002, -3.9675002],
            [64.047005, -6.7995],
            [67.727997, -10.416],
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            let actual = ae_space(stream[index]);
            assert!((actual[0] - expected[0]).abs() < 0.002);
            assert!((actual[1] - expected[1]).abs() < 0.002);
        }
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
