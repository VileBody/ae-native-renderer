use raster_cpu::Canvas;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::sync::{Arc, Mutex, OnceLock};
use ttf_parser::{Face, GlyphId, OutlineBuilder, Tag};

use crate::{
    blend_p6_ad68_event_stream_to_canvas, emit_ad68_rows, layout_text, load_font_with_telemetry,
    materialize_text_source_path_to_p6_ad68_rows, p6_ad68_flag_trace_payload,
    p6_ad68_frontier_debug_limit, p6_ad68_frontier_debug_payload, p6_ad68_opt_in_flag_state,
    p6_ad68_pixel_handoff_mode, p6_ad68_pixel_opt_in_flag_state, p6_ad68_row_byte_debug_limit,
    p6_ad68_row_byte_debug_payload, try_rasterize_text_with_p6_ad68, AreCursorState,
    AreEventObject, AreEventRow, AreEventStream, ArePayloadBacking, ArePayloadBackingId,
    ArePayloadWindow, EventClass, P6Ad68OptInFlagState, P6Ad68PathPoint, P6Ad68PathSegment,
    P6Ad68PixelOptInFlagState, P6Ad68PixelPlacement, P6Ad68RawPathSegment, P6Ad68TextPathInput,
    P6Ad68TextRouteReport, P6Ad68TextRowsOutput, TextLayoutRequest, TextLayoutResult,
    P6_AD68_TEXT_PIXELS_HANDOFF_ENV_VAR, P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR,
};

const OUTLINE_COVERAGE_SUPERSAMPLE: u32 = 16;
const COV_O_TEXT_VARIANT_ENV_VAR: &str = "AE_COV_O_TEXT_VARIANT";
const COV_O_EDGE_SOURCE_POLICY: &str = "ARE_db98_edge_source_v1";
const TEXT_COVERAGE_ROW_POLICY_ENV_VAR: &str = "AE_NATIVE_TEXT_COVERAGE_ROW_POLICY";
const ARE_TYPED_ROW_POLICY: &str = "ARE_row_node_sampler_typed_spans_v1";
const ARE_BRIDGE_BASIS_V2_POLICY: &str = "ARE_bridge_basis_row_node_sampler_v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CovOTextVariant {
    Baseline,
    Db98EdgeSourceRows,
    Db98ShallowCubics,
    Db98FlatnessCandidate,
    RotateStartPointForCurve,
    NoReverseLineContours,
    ExplicitCloseSegment,
    CoordOriginPhasePlus,
    CoordOriginPhaseMinus,
    CoordOriginRound,
    CoordDevicePointHalfPhase,
    SortTiesByXMax,
    InvertWindingSign,
    CloseSegmentOrdering,
    CloseOpenIntervalAtEnd,
    NoReverseCurveStream,
    MidpointPrevNextCurveStream,
    SwappedCurveControlPlacement,
}

fn cov_o_text_variant() -> CovOTextVariant {
    static VARIANT: OnceLock<CovOTextVariant> = OnceLock::new();
    *VARIANT.get_or_init(|| {
        let env = std::env::var(COV_O_TEXT_VARIANT_ENV_VAR).unwrap_or_default();
        match env.to_ascii_lowercase().as_str() {
            "baseline" | "legacy" | "legacy_origin" => CovOTextVariant::Baseline,
            "c1" | "db98-edge-rows" | "db98_edge_rows" | "edge_source_rows" => {
                CovOTextVariant::Db98EdgeSourceRows
            }
            "c2" | "db98-shallow-cubics" | "shallow_cubics" | "classify_shallow" => {
                CovOTextVariant::Db98ShallowCubics
            }
            "c3" | "db98-flatness-candidate" | "flatness_candidate" | "evidence_flatness" => {
                CovOTextVariant::Db98FlatnessCandidate
            }
            "e1" | "sortxmax" | "tie_sort_xmax" | "sort-tie-xmax" => {
                CovOTextVariant::SortTiesByXMax
            }
            "e2" | "invertwinding" | "invert_winding" | "invert-winding" => {
                CovOTextVariant::InvertWindingSign
            }
            "e3" | "close_seg_order" | "close_segment_order" | "close-ordering" => {
                CovOTextVariant::CloseSegmentOrdering
            }
            "e4"
            | "close_open_end"
            | "close-open-end"
            | "are_430c_close_open_end"
            | "sampler_close_open_end" => CovOTextVariant::CloseOpenIntervalAtEnd,
            "a1" | "rotate" | "rotate_start" | "rotate_start_point" => {
                CovOTextVariant::RotateStartPointForCurve
            }
            "a2"
            | "no_reverse"
            | "no_reverse_lines"
            | "nor_lines"
            | "no-reverse-lines"
            | "no_reverse_line_only" => CovOTextVariant::NoReverseLineContours,
            "a3" | "close" | "explicit_close" | "explicit_close_segment" | "close_segment" => {
                CovOTextVariant::ExplicitCloseSegment
            }
            "b1" | "b1_no_reverse_stream" | "no_reverse_curve_stream" | "curve_no_reverse" => {
                CovOTextVariant::NoReverseCurveStream
            }
            "b2" | "b2_midpoint_prev_next" | "midpoint_prev_next_curve_stream" => {
                CovOTextVariant::MidpointPrevNextCurveStream
            }
            "b3" | "b3_swapped_control_placement" | "swapped_curve_control_placement" => {
                CovOTextVariant::SwappedCurveControlPlacement
            }
            "d1"
            | "d1+"
            | "coord_origin_phase_plus"
            | "origin_phase_plus"
            | "coord_origin_plus"
            | "coord_plus" => CovOTextVariant::CoordOriginPhasePlus,
            "d1-"
            | "d1_minus"
            | "coord_origin_phase_minus"
            | "origin_phase_minus"
            | "coord_origin_minus"
            | "coord_minus" => CovOTextVariant::CoordOriginPhaseMinus,
            "d2" | "d2_round" | "coord_origin_round" | "round_origin" => {
                CovOTextVariant::CoordOriginRound
            }
            "d3"
            | "d3_half"
            | "device_point_half"
            | "device_point_offset"
            | "coord_matrix_phase" => CovOTextVariant::CoordDevicePointHalfPhase,
            _ => CovOTextVariant::CoordOriginPhasePlus,
        }
    })
}

fn should_sort_ties_by_xmax(variant: CovOTextVariant) -> bool {
    matches!(variant, CovOTextVariant::SortTiesByXMax)
}

fn should_invert_winding_sign(variant: CovOTextVariant) -> bool {
    matches!(variant, CovOTextVariant::InvertWindingSign)
}

fn should_close_interval_with_plus_one(variant: CovOTextVariant) -> bool {
    !matches!(variant, CovOTextVariant::CloseSegmentOrdering)
}

fn should_close_open_interval_at_end(variant: CovOTextVariant) -> bool {
    matches!(variant, CovOTextVariant::CloseOpenIntervalAtEnd)
}

#[derive(Clone, Copy, Debug)]
struct Db98FlatnessProfile {
    flatness: f32,
    short_chord_ratio: f32,
    tolerance_scale: f32,
    max_depth: u8,
    signature_policy: &'static str,
}

impl Default for Db98FlatnessProfile {
    fn default() -> Self {
        Self {
            flatness: 1.0,
            short_chord_ratio: 0.25,
            tolerance_scale: 1.0,
            max_depth: 15,
            signature_policy: "ARE_db98_flatten_to_5258_edge_node_layout",
        }
    }
}

fn cov_o_db98_profile() -> Db98FlatnessProfile {
    static PROFILE: OnceLock<Db98FlatnessProfile> = OnceLock::new();
    *PROFILE.get_or_init(|| match cov_o_text_variant() {
        CovOTextVariant::Db98ShallowCubics => Db98FlatnessProfile {
            short_chord_ratio: 0.50,
            signature_policy: "ARE_db98_flatten_to_5258_edge_node_layout_shallow_to_lines_earlier",
            ..Default::default()
        },
        CovOTextVariant::Db98FlatnessCandidate => Db98FlatnessProfile {
            flatness: 1.25,
            short_chord_ratio: 0.30,
            tolerance_scale: 1.10,
            signature_policy: "ARE_db98_flatten_to_5258_edge_node_layout_candidate_v2",
            ..Default::default()
        },
        _ => Db98FlatnessProfile::default(),
    })
}

fn coverage_backend_signature_policy() -> &'static str {
    match cov_o_text_variant() {
        CovOTextVariant::Db98ShallowCubics => {
            "native_ttf_outline_are_db98_edge_signatures_v1_shallow_cubics"
        }
        CovOTextVariant::Db98FlatnessCandidate => {
            "native_ttf_outline_are_db98_edge_signatures_v1_flatness_candidate"
        }
        CovOTextVariant::Db98EdgeSourceRows => {
            "native_ttf_outline_are_db98_edge_signatures_v1_row_source"
        }
        _ => "native_ttf_outline_are_db98_edge_signatures_v1",
    }
}

fn coverage_rows_span_policy(variant: CovOTextVariant) -> &'static str {
    if std::env::var(TEXT_COVERAGE_ROW_POLICY_ENV_VAR)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "are_typed_rows"
                    | "are-row-node-sampler"
                    | "are_row_node_sampler"
                    | "are_bridge_basis_v2"
                    | "are-bridge-basis-v2"
                    | "are_bridge_basis_row_node_sampler_v2"
            )
        })
        .unwrap_or(false)
    {
        let value = std::env::var(TEXT_COVERAGE_ROW_POLICY_ENV_VAR).unwrap_or_default();
        let value = value.to_ascii_lowercase();
        if matches!(
            value.as_str(),
            "are_bridge_basis_v2" | "are-bridge-basis-v2" | "are_bridge_basis_row_node_sampler_v2"
        ) {
            return ARE_BRIDGE_BASIS_V2_POLICY;
        }
        return ARE_TYPED_ROW_POLICY;
    }
    if variant == CovOTextVariant::Db98EdgeSourceRows {
        COV_O_EDGE_SOURCE_POLICY
    } else {
        "are_scanline_16x_negative_winding_merged_projected_ranges_v1"
    }
}

fn rotate_contour_to_lowest_point(contour: &mut Vec<RawOutlinePoint>) {
    let start = (0..contour.len()).min_by(|a, b| {
        contour[*a]
            .point
            .y
            .total_cmp(&contour[*b].point.y)
            .then_with(|| contour[*a].point.x.total_cmp(&contour[*b].point.x))
    });
    if let Some(index) = start {
        contour.rotate_left(index);
    }
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

fn push_p6_ad68_pixel_opt_in_flag_event(
    events: &mut Vec<TextRasterEvent>,
    seq: &mut u64,
    flag_state: P6Ad68PixelOptInFlagState,
) {
    push_text_raster_event(
        events,
        seq,
        "p6_ad68_pixel_opt_in_flag",
        None,
        None,
        json!({
            "env_var": P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR,
            "state": flag_state.as_str(),
            "enabled": flag_state.enabled(),
            "default_behavior_unchanged": !flag_state.enabled(),
            "production_default": true
        }),
    );
}

fn push_p6_ad68_pixel_write_event(
    events: &mut Vec<TextRasterEvent>,
    seq: &mut u64,
    report: &P6Ad68TextRouteReport,
    payload: Value,
) {
    push_text_raster_event(
        events,
        seq,
        "p6_ad68_pixel_write",
        Some(report.glyph_run_index),
        Some(report.glyph_id),
        payload,
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
    let y_max = p6_ad68_source_path_y_max(y_max, supersample);
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

fn p6_ad68_pixel_path_segments_for_text(
    path_segments: &[RawPathSegment],
    scale: f32,
    x_min: f32,
    y_max: f32,
    ss: u32,
) -> Vec<P6Ad68RawPathSegment> {
    p6_ad68_path_segments_for_text(path_segments, scale, x_min, y_max, ss)
}

fn p6_ad68_source_path_y_max(y_max: f32, supersample: u32) -> f32 {
    let _ = supersample;
    if cov_o_text_variant() == CovOTextVariant::CoordOriginPhasePlus {
        y_max - 1.0 / OUTLINE_COVERAGE_SUPERSAMPLE as f32
    } else {
        y_max
    }
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
            "db98_device": {
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
            "db98_device": {
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
    #[serde(default)]
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
    pub coverage_edge_signature_source: String,
    pub coverage_edge_signatures: Vec<CoverageEdgeSignature>,
    pub output_semantics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageRowSpan {
    pub schema: String,
    pub policy: String,
    pub glyph_run_index: usize,
    pub glyph_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_type: Option<u8>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sentinel: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentinel_raw_end_x: Option<i32>,
    pub y: i32,
    pub start_x: i32,
    pub end_x: i32,
    pub coverage_len: u32,
    pub coverage_hash_fnv1a64: String,
    pub coverage_sample_hex: String,
    pub coverage_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageEdgeSignature {
    pub schema: String,
    pub policy: String,
    pub glyph_run_index: usize,
    pub glyph_id: u32,
    pub edge_index: usize,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub slope: f32,
    pub winding_flag: i8,
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
    let mut events = Vec::new();
    let mut event_seq = 0_u64;
    let outline_font_key = font_resolution
        .resolved_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| req.font_id.clone());
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
    let p6_ad68_flag_state = p6_ad68_opt_in_flag_state();
    if p6_ad68_flag_state != P6Ad68OptInFlagState::DisabledDefault {
        push_p6_ad68_opt_in_flag_event(&mut events, &mut event_seq, p6_ad68_flag_state);
    }
    let p6_ad68_pixel_flag_state = p6_ad68_pixel_opt_in_flag_state();
    push_p6_ad68_pixel_opt_in_flag_event(&mut events, &mut event_seq, p6_ad68_pixel_flag_state);

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
        let span_policy = coverage_rows_span_policy(cov_o_text_variant());
        let coverage_rows = coverage_row_spans(
            run_index,
            glyph.glyph_id,
            coverage.raster_x,
            coverage.raster_y,
            coverage.width,
            coverage.height,
            coverage.bitmap.as_ref(),
            clipped_bounds,
            span_policy,
        );
        let coverage_rows = if cov_o_text_variant() == CovOTextVariant::Db98EdgeSourceRows {
            coverage_rows_from_db98_edge_signatures(
                run_index,
                glyph.glyph_id,
                coverage.raster_x,
                coverage.raster_y,
                coverage.width,
                coverage.height,
                coverage.edge_signatures.as_ref(),
                OUTLINE_COVERAGE_SUPERSAMPLE as usize,
                clipped_bounds,
            )
            .unwrap_or_else(|| coverage_rows)
        } else {
            coverage_rows
        };
        let coverage_edge_signatures =
            coverage_edge_signatures(run_index, glyph.glyph_id, coverage.edge_signatures.as_ref());
        let mut p6_pixel_handled = false;
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
            push_p6_ad68_route_event(&mut events, &mut event_seq, &report);
        }
        if p6_ad68_pixel_flag_state.enabled() {
            let p6_pixel_handoff_mode = p6_ad68_pixel_handoff_mode();
            let p6_pixel_source_path_supersample =
                p6_pixel_handoff_mode.source_path_supersample(coverage.supersample);
            let p6_path_segments = p6_ad68_pixel_path_segments_for_text(
                coverage.path_segments.as_ref(),
                coverage.scale,
                coverage.x_min,
                coverage.y_max,
                p6_pixel_source_path_supersample,
            );
            if !p6_pixel_handled {
                let output = materialize_text_source_path_to_p6_ad68_rows(&P6Ad68TextPathInput {
                    glyph_run_index: run_index,
                    glyph_id: glyph.glyph_id,
                    path_segments: &p6_path_segments,
                    expected_advance_only_space: ch.is_some_and(char::is_whitespace)
                        && glyph.advance.is_finite()
                        && glyph.advance > 0.0,
                });
                match output {
                    P6Ad68TextRowsOutput::Routed {
                        report,
                        event_stream,
                        row_table,
                    } => {
                        let placement = P6Ad68PixelPlacement {
                            origin_x: coverage.raster_x.round() as i32,
                            origin_y: coverage.raster_y.round() as i32,
                            width: coverage.width,
                            height: coverage.height,
                            clip: clipped_bounds,
                            coordinate_scale: p6_pixel_handoff_mode
                                .coordinate_scale(coverage.supersample),
                        };
                        match blend_p6_ad68_event_stream_to_canvas(
                            &mut canvas,
                            &event_stream,
                            placement,
                            color,
                        ) {
                            Ok(pixel_report) => {
                                p6_pixel_handled = true;
                                let mut payload = json!({
                                    "env_var": P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR,
                                    "status": "routed_pixels",
                                    "route_status": report.status.as_str(),
                                    "success": true,
                                    "fallback_to_default_path": false,
                                    "fallback_counted_as_p6_success": false,
                                    "producer": "p6_source_owned_ad68_event_stream_to_canvas_pixels_v1",
                                    "handoff_env_var": P6_AD68_TEXT_PIXELS_HANDOFF_ENV_VAR,
                                    "handoff_mode": p6_pixel_handoff_mode.as_str(),
                                    "coordinate_basis": p6_pixel_handoff_mode.coordinate_basis(),
                                    "source_path_supersample": p6_pixel_source_path_supersample,
                                    "coverage_supersample": coverage.supersample,
                                    "coordinate_scale": placement.coordinate_scale,
                                    "placement_origin": [placement.origin_x, placement.origin_y],
                                    "placement_size": [placement.width, placement.height],
                                    "clip": placement.clip,
                                    "row_count": row_table.rows.len(),
                                    "event_stream_row_count": event_stream.rows.len(),
                                    "pixel_write_count": pixel_report.pixel_write_count,
                                    "nonzero_coverage_pixel_count": pixel_report.nonzero_coverage_pixel_count,
                                    "source_sample_count": pixel_report.source_sample_count,
                                    "class0_span_count": pixel_report.class0_span_count,
                                    "class1_span_count": pixel_report.class1_span_count,
                                    "class2_span_count": pixel_report.class2_span_count,
                                    "class2_byte_count": pixel_report.class2_byte_count,
                                    "clipped_sample_count": pixel_report.clipped_sample_count,
                                    "used_coverage_bitmap_for_pixel_bytes": pixel_report.used_coverage_bitmap_for_pixel_bytes,
                                    "used_coverage_rows_for_pixel_bytes": pixel_report.used_coverage_rows_for_pixel_bytes,
                                    "used_typed_span_proof": pixel_report.used_typed_span_proof,
                                    "used_fixture_payload": pixel_report.used_fixture_payload,
                                    "used_synthetic_payload": pixel_report.used_synthetic_payload
                                });
                                if let Some(limit) = p6_ad68_row_byte_debug_limit() {
                                    payload["row_byte_debug"] =
                                        p6_ad68_row_byte_debug_payload(&row_table, limit);
                                }
                                if let Some(limit) = p6_ad68_frontier_debug_limit() {
                                    payload["frontier_debug"] = p6_ad68_frontier_debug_payload(
                                        &P6Ad68TextPathInput {
                                            glyph_run_index: run_index,
                                            glyph_id: glyph.glyph_id,
                                            path_segments: &p6_path_segments,
                                            expected_advance_only_space: ch
                                                .is_some_and(char::is_whitespace)
                                                && glyph.advance.is_finite()
                                                && glyph.advance > 0.0,
                                        },
                                        limit,
                                    );
                                }
                                push_p6_ad68_pixel_write_event(
                                    &mut events,
                                    &mut event_seq,
                                    &report,
                                    payload,
                                );
                            }
                            Err(err) => {
                                push_p6_ad68_pixel_write_event(
                                    &mut events,
                                    &mut event_seq,
                                    &report,
                                    json!({
                                        "env_var": P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR,
                                        "status": "pixel_write_error",
                                        "route_status": report.status.as_str(),
                                        "success": false,
                                        "fallback_to_default_path": true,
                                        "fallback_counted_as_p6_success": false,
                                        "producer": "p6_source_owned_ad68_event_stream_to_canvas_pixels_v1",
                                        "handoff_env_var": P6_AD68_TEXT_PIXELS_HANDOFF_ENV_VAR,
                                        "handoff_mode": p6_pixel_handoff_mode.as_str(),
                                        "coordinate_basis": p6_pixel_handoff_mode.coordinate_basis(),
                                        "source_path_supersample": p6_pixel_source_path_supersample,
                                        "coverage_supersample": coverage.supersample,
                                        "error": format!("{:?}", err),
                                        "used_coverage_bitmap_for_pixel_bytes": false,
                                        "used_coverage_rows_for_pixel_bytes": false,
                                        "used_typed_span_proof": false,
                                        "used_fixture_payload": false,
                                        "used_synthetic_payload": false
                                    }),
                                );
                            }
                        }
                    }
                    P6Ad68TextRowsOutput::ExpectedAdvanceOnlySpace { report } => {
                        p6_pixel_handled = true;
                        push_p6_ad68_pixel_write_event(
                            &mut events,
                            &mut event_seq,
                            &report,
                            json!({
                                "env_var": P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR,
                                "status": "expected_advance_only_space",
                                "route_status": report.status.as_str(),
                                "success": false,
                                "expected_advance_only": true,
                                "fallback_to_default_path": false,
                                "fallback_counted_as_p6_success": false,
                                "producer": "p6_source_owned_ad68_event_stream_to_canvas_pixels_v1",
                                "handoff_env_var": P6_AD68_TEXT_PIXELS_HANDOFF_ENV_VAR,
                                "handoff_mode": p6_pixel_handoff_mode.as_str(),
                                "coordinate_basis": p6_pixel_handoff_mode.coordinate_basis(),
                                "source_path_supersample": p6_pixel_source_path_supersample,
                                "coverage_supersample": coverage.supersample,
                                "used_coverage_bitmap_for_pixel_bytes": false,
                                "used_coverage_rows_for_pixel_bytes": false,
                                "used_typed_span_proof": false,
                                "used_fixture_payload": false,
                                "used_synthetic_payload": false
                            }),
                        );
                    }
                    P6Ad68TextRowsOutput::Unsupported { report } => {
                        push_p6_ad68_pixel_write_event(
                            &mut events,
                            &mut event_seq,
                            &report,
                            json!({
                                "env_var": P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR,
                                "status": "unsupported_source_path",
                                "route_status": report.status.as_str(),
                                "success": false,
                                "reason": report.reason,
                                "fallback_to_default_path": true,
                                "fallback_counted_as_p6_success": false,
                                "producer": "p6_source_owned_ad68_event_stream_to_canvas_pixels_v1",
                                "handoff_env_var": P6_AD68_TEXT_PIXELS_HANDOFF_ENV_VAR,
                                "handoff_mode": p6_pixel_handoff_mode.as_str(),
                                "coordinate_basis": p6_pixel_handoff_mode.coordinate_basis(),
                                "source_path_supersample": p6_pixel_source_path_supersample,
                                "coverage_supersample": coverage.supersample,
                                "used_coverage_bitmap_for_pixel_bytes": false,
                                "used_coverage_rows_for_pixel_bytes": false,
                                "used_typed_span_proof": false,
                                "used_fixture_payload": false,
                                "used_synthetic_payload": false
                            }),
                        );
                    }
                }
            }
        }
        let path_segments = coverage
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
        let cubic_calls = path_segments
            .iter()
            .filter(|segment| segment.get("kind").and_then(Value::as_str) == Some("cubic"))
            .cloned()
            .collect::<Vec<_>>();
        let line_calls = path_segments
            .iter()
            .filter(|segment| segment.get("kind").and_then(Value::as_str) == Some("line"))
            .cloned()
            .collect::<Vec<_>>();
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
                "policy": coverage.edge_signature_source,
                "variant": format!("{:?}", cov_o_text_variant()),
                "segment_count": path_segments.len(),
                "cubic_call_count": cubic_calls.len(),
                "line_call_count": line_calls.len(),
                "scale": f32_trace(coverage.scale),
                "x_min": f32_trace(coverage.x_min),
                "y_max": f32_trace(coverage.y_max),
                "supersample": coverage.supersample,
                "cubic_calls": cubic_calls,
                "edge_count": coverage_edge_signatures.len(),
                "first_edge": coverage_edge_signatures.first().map(|edge| json!({
                    "x0": f32_trace(edge.x0),
                    "y0": f32_trace(edge.y0),
                    "x1": f32_trace(edge.x1),
                    "y1": f32_trace(edge.y1),
                    "slope": f32_trace(edge.slope),
                    "winding_flag": edge.winding_flag
                }))
            }),
        );
        push_text_raster_event(
            &mut events,
            &mut event_seq,
            "path_segment_emit",
            Some(run_index),
            Some(glyph.glyph_id),
            json!({
                "segment_count": coverage_edge_signatures.len(),
                "outline_segment_count": path_segments.len(),
                "segments": path_segments,
                "segment_policy": cov_o_db98_profile().signature_policy,
                "coverage_backend": coverage.backend
            }),
        );
        for segment in coverage
            .path_segments
            .iter()
            .copied()
            .filter(|segment| matches!(segment.segment, AePathSegment::Cubic { .. }))
        {
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "cubic_control_emit",
                Some(run_index),
                Some(glyph.glyph_id),
                path_segment_trace(
                    segment,
                    coverage.scale,
                    coverage.x_min,
                    coverage.y_max,
                    coverage.supersample.max(1) as i32,
                ),
            );
        }
        push_text_raster_event(
            &mut events,
            &mut event_seq,
            "flatten_split",
            Some(run_index),
            Some(glyph.glyph_id),
            json!({
                "flatness": f32_trace(cov_o_db98_profile().flatness),
                "short_chord_ratio": f32_trace(cov_o_db98_profile().short_chord_ratio),
                "tolerance_scale": f32_trace(cov_o_db98_profile().tolerance_scale),
                "max_depth": cov_o_db98_profile().max_depth,
                "edge_count_after_flatten": coverage_edge_signatures.len()
            }),
        );
        for edge in &coverage_edge_signatures {
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "edge_emit",
                Some(run_index),
                Some(glyph.glyph_id),
                json!({
                    "edge_index": edge.edge_index,
                    "policy": &edge.policy,
                    "x0": f32_trace(edge.x0),
                    "y0": f32_trace(edge.y0),
                    "x1": f32_trace(edge.x1),
                    "y1": f32_trace(edge.y1),
                    "slope": f32_trace(edge.slope),
                    "winding_flag": edge.winding_flag
                }),
            );
        }
        push_text_raster_event(
            &mut events,
            &mut event_seq,
            "scanline_event",
            Some(run_index),
            Some(glyph.glyph_id),
            json!({
                "row_count": coverage_rows.len(),
                "span_policy": span_policy,
                "supersample": coverage.supersample
            }),
        );
        if span_policy == ARE_BRIDGE_BASIS_V2_POLICY {
            push_are_bridge_basis_v2_events(
                &mut events,
                &mut event_seq,
                run_index,
                glyph.glyph_id,
                &coverage_rows,
            );
        }
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
                    "span_type": row.span_type,
                    "gap_type": row.gap_type,
                    "sentinel": row.sentinel,
                    "sentinel_raw_end_x": row.sentinel_raw_end_x,
                    "coverage_len": row.coverage_len,
                    "coverage_hash_fnv1a64": &row.coverage_hash_fnv1a64,
                    "coverage_sample_hex": &row.coverage_sample_hex,
                    "has_full_coverage_hex": row.coverage_hex.is_some()
                }),
            );
            push_text_raster_event(
                &mut events,
                &mut event_seq,
                "row_byte_build",
                Some(run_index),
                Some(glyph.glyph_id),
                json!({
                    "y": row.y,
                    "start_x": row.start_x,
                    "end_x": row.end_x,
                    "span_type": row.span_type,
                    "gap_type": row.gap_type,
                    "sentinel": row.sentinel,
                    "sentinel_raw_end_x": row.sentinel_raw_end_x,
                    "coverage_len": row.coverage_len,
                    "coverage_hash_fnv1a64": &row.coverage_hash_fnv1a64,
                    "coverage_hex": &row.coverage_hex,
                    "coverage_sample_hex": &row.coverage_sample_hex
                }),
            );
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
            coverage_edge_signature_source: coverage.edge_signature_source.to_string(),
            coverage_edge_signatures,
            output_semantics: "recovered_txt_are_pf_pixel8_integer_source_over_v1".to_string(),
        };
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
                "coverage_rows": plan.coverage_rows.len(),
                "coverage_edge_signatures": plan.coverage_edge_signatures.len()
            }),
        );
        if will_draw && !p6_pixel_handled {
            let mut event_sink = TextRasterEventSink {
                events: &mut events,
                seq: &mut event_seq,
                glyph_run_index: run_index,
                glyph_id: glyph.glyph_id,
            };
            blend_bitmap(
                &mut canvas,
                coverage.raster_x,
                coverage.raster_y,
                coverage.width,
                coverage.height,
                coverage.bitmap.as_ref(),
                color,
                clipped_bounds,
                Some(&mut event_sink),
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
    edge_signatures: Arc<Vec<RawCoverageEdgeSignature>>,
    path_segments: Arc<Vec<RawPathSegment>>,
    scale: f32,
    x_min: f32,
    y_max: f32,
    backend: &'static str,
    supersample: u32,
    origin_source: &'static str,
    edge_signature_source: &'static str,
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
        edge_signatures: Arc::new(Vec::new()),
        path_segments: Arc::new(Vec::new()),
        scale: 0.0,
        x_min: 0.0,
        y_max: 0.0,
        backend: "fontdue_rasterize_indexed_fallback",
        supersample: 1,
        origin_source: "fontdue_metrics",
        edge_signature_source: "none_fontdue_fallback",
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
    edge_signatures: Arc<Vec<RawCoverageEdgeSignature>>,
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
            edge_signatures: self.edge_signatures.clone(),
            path_segments: self.path_segments.clone(),
            scale: self.scale,
            x_min: self.x_min,
            y_max: self.y_max,
            backend: "ttf_outline_are_scanline_16x",
            supersample: self.supersample,
            origin_source: "txt_are_integer_world_bbox_origin",
            edge_signature_source: coverage_backend_signature_policy(),
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
    let variant = cov_o_text_variant();
    let abs_left = match variant {
        CovOTextVariant::CoordOriginRound => (glyph_x + bbox_x_min).round(),
        _ => (glyph_x + bbox_x_min).floor(),
    };
    let abs_right = (glyph_x + bbox_x_max).ceil();
    let abs_top = match variant {
        CovOTextVariant::CoordOriginRound => (baseline - bbox_y_max).round(),
        _ => (baseline - bbox_y_max).floor(),
    };
    let abs_bottom = (baseline - bbox_y_min).ceil();
    let x_min = abs_left - glyph_x;
    let mut y_max = baseline - abs_top;
    if variant == CovOTextVariant::CoordOriginPhasePlus {
        y_max += 1.0 / supersample.max(1) as f32;
    } else if variant == CovOTextVariant::CoordOriginPhaseMinus {
        y_max -= 1.0 / supersample.max(1) as f32;
    }
    let width = (abs_right - abs_left).max(0.0) as usize;
    let height = (abs_bottom - abs_top).max(0.0) as usize;
    if width == 0 || height == 0 {
        return None;
    }

    let ss = supersample.max(1);
    let edge_signatures = if ss == OUTLINE_COVERAGE_SUPERSAMPLE {
        build_are_edge_signatures(&outline, scale, x_min, y_max, ss as i32)
    } else {
        Vec::new()
    };
    let path_segments = collect_path_segments(&outline);
    let mut bitmap = vec![0u8; width * height];
    let mut nonzero_pixels = 0u32;
    if ss == 16 {
        if edge_signatures.is_empty() {
            build_are_scanline_coverage(&outline, scale, x_min, y_max, width, height, &mut bitmap);
        } else {
            build_are_edge_signature_coverage(
                &edge_signatures,
                width,
                height,
                ss as i32,
                &mut bitmap,
            );
        }
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
        edge_signatures: Arc::new(edge_signatures),
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
struct RawCoverageEdgeSignature {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    slope: f32,
    winding_flag: i8,
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
    let variant = cov_o_text_variant();
    let mut contours = Vec::new();
    let mut path_contours = Vec::new();
    for contour in glyph {
        if contour.len() < 2 {
            continue;
        }
        let mut contour = contour;
        let has_curve = contour.iter().any(|point| !point.on_curve);
        if has_curve && variant == CovOTextVariant::RotateStartPointForCurve {
            rotate_contour_to_lowest_point(&mut contour);
        }
        let (flattened, path_segments) = if has_curve {
            let explicit_close_segment = variant == CovOTextVariant::ExplicitCloseSegment;
            let stream = match variant {
                CovOTextVariant::NoReverseCurveStream => {
                    ae_curve_stream_from_tt(&contour, &QuadraticToCubicConfig::no_reverse())
                }
                CovOTextVariant::MidpointPrevNextCurveStream => ae_curve_stream_from_tt(
                    &contour,
                    &QuadraticToCubicConfig::no_reverse_prev_next(),
                ),
                CovOTextVariant::SwappedCurveControlPlacement => {
                    ae_curve_stream_from_tt(&contour, &QuadraticToCubicConfig::no_reverse_swapped())
                }
                _ => ae_curve_stream_from_reversed_tt(&contour),
            };
            (
                flatten_ae_curve_stream(&stream, explicit_close_segment),
                ae_cubic_segments(&stream, explicit_close_segment),
            )
        } else {
            let mut points = contour.iter().map(|point| point.point).collect::<Vec<_>>();
            // TXT_ARE_PathBuilder traces for line-only glyphs emit scaled TTF
            // contour points in reverse order.
            if variant != CovOTextVariant::NoReverseLineContours {
                points.reverse();
            }
            let segments =
                ae_line_segments(&points, variant == CovOTextVariant::ExplicitCloseSegment);
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

fn flatten_ae_curve_stream(stream: &[Point], explicit_close_segment: bool) -> Vec<Point> {
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
    if !explicit_close_segment
        && flattened.len() >= 2
        && distance(flattened[0], *flattened.last().unwrap()) < 0.0001
    {
        flattened.pop();
    }
    flattened
}

fn ae_cubic_segments(stream: &[Point], explicit_close_segment: bool) -> Vec<AePathSegment> {
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
    if explicit_close_segment && stream.len() >= 2 {
        let start = stream[0];
        let end = stream[stream.len() - 1];
        if distance(start, end) > 0.0001 {
            segments.push(AePathSegment::Line {
                start: end,
                end: start,
            });
        }
    }
    segments
}

fn ae_line_segments(points: &[Point], explicit_close_segment: bool) -> Vec<AePathSegment> {
    if points.len() < 2 {
        return Vec::new();
    }
    if points.len() < 2 {
        return Vec::new();
    }
    let mut segments = Vec::with_capacity(points.len() + usize::from(explicit_close_segment));
    let end = points.len() - 1;
    for index in 0..end {
        segments.push(AePathSegment::Line {
            start: points[index],
            end: points[index + 1],
        });
    }
    if explicit_close_segment {
        if let (Some(start), Some(end)) = (points.first().copied(), points.last().copied()) {
            if distance(start, end) > 0.0001 {
                segments.push(AePathSegment::Line {
                    start: end,
                    end: start,
                });
            }
        }
    } else {
        segments.push(AePathSegment::Line {
            start: points[end],
            end: points[0],
        });
    }
    segments
}

fn ae_curve_stream_from_reversed_tt(contour: &[RawOutlinePoint]) -> Vec<Point> {
    ae_curve_stream_from_tt(contour, &QuadraticToCubicConfig::reversed())
}

#[derive(Clone, Copy, Debug)]
struct QuadraticToCubicConfig {
    reverse: bool,
    midpoint_from_previous: bool,
    swap_controls: bool,
    control_scale: f32,
}

impl QuadraticToCubicConfig {
    fn reversed() -> Self {
        Self {
            reverse: true,
            midpoint_from_previous: false,
            swap_controls: false,
            control_scale: 2.0 / 3.0,
        }
    }

    fn no_reverse() -> Self {
        Self {
            reverse: false,
            midpoint_from_previous: false,
            swap_controls: false,
            control_scale: 2.0 / 3.0,
        }
    }

    fn no_reverse_prev_next() -> Self {
        Self {
            reverse: false,
            midpoint_from_previous: true,
            swap_controls: false,
            control_scale: 2.0 / 3.0,
        }
    }

    fn no_reverse_swapped() -> Self {
        Self {
            reverse: false,
            midpoint_from_previous: false,
            swap_controls: true,
            control_scale: 2.0 / 3.0,
        }
    }
}

fn ae_curve_stream_from_tt(
    contour: &[RawOutlinePoint],
    config: &QuadraticToCubicConfig,
) -> Vec<Point> {
    if contour.is_empty() {
        return Vec::new();
    }
    let mut points = contour.to_vec();
    if config.reverse {
        points.reverse();
    }
    let (mut current, start_index) = if points[0].on_curve {
        (points[0].point, 1usize)
    } else if points.len() > 1 && !points[1].on_curve {
        let fallback = if config.midpoint_from_previous {
            midpoint(points[points.len() - 1].point, points[1].point)
        } else {
            midpoint(points[0].point, points[1].point)
        };
        (fallback, 1usize)
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
            if config.midpoint_from_previous {
                let index = (start_index + offset) % points.len();
                let prev_index = if index == 0 {
                    points.len() - 1
                } else {
                    index - 1
                };
                midpoint(points[prev_index].point, next.point)
            } else {
                midpoint(raw.point, next.point)
            }
        };
        segments.push((current, raw.point, end));
        current = end;
    }
    if segments.is_empty() {
        return vec![current];
    }

    let mut stream = vec![segments[0].0];
    for (start, control, end) in segments {
        let scale = config.control_scale;
        let control1 = Point {
            x: start.x + scale * (control.x - start.x),
            y: start.y + scale * (control.y - start.y),
        };
        let control2 = Point {
            x: end.x + scale * (control.x - end.x),
            y: end.y + scale * (control.y - end.y),
        };
        if config.swap_controls {
            stream.push(control2);
            stream.push(control1);
        } else {
            stream.push(control1);
            stream.push(control2);
        }
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
    if should_sort_ties_by_xmax(cov_o_text_variant()) {
        events.sort_by(|a, b| {
            a.x_min_fixed
                .cmp(&b.x_min_fixed)
                .then(a.x_max_fixed.cmp(&b.x_max_fixed))
        });
    } else {
        events.sort_by_key(|event| event.x_min_fixed);
    }
    projected_events_to_intervals(events)
}

fn projected_events_to_intervals(events: Vec<ProjectedScanlineEvent>) -> Vec<(i32, i32)> {
    let mut events = merge_projected_events_by_winding(events);
    if should_sort_ties_by_xmax(cov_o_text_variant()) {
        events.sort_by(|a, b| {
            a.x_min_fixed
                .cmp(&b.x_min_fixed)
                .then(a.x_max_fixed.cmp(&b.x_max_fixed))
        });
    } else {
        events.sort_by_key(|event| event.x_min_fixed);
    }
    let mut intervals = Vec::new();
    let mut winding = 0i32;
    let mut start: Option<i32> = None;
    let mut max_end = i32::MIN;
    for event in events {
        let was_inside = winding < 0;
        let next_winding = winding + event.winding_delta;
        let is_inside = next_winding < 0;

        if !was_inside && is_inside {
            start = Some(event.x_min_fixed);
            max_end = event.x_max_fixed;
        } else if was_inside {
            max_end = max_end.max(event.x_max_fixed);
        }
        winding = next_winding;

        if was_inside && !is_inside {
            if let Some(start_fixed) = start.take() {
                if should_close_interval_with_plus_one(cov_o_text_variant()) {
                    intervals.push((start_fixed, max_end + 1));
                } else {
                    intervals.push((start_fixed, max_end));
                }
            }
        }
    }
    if should_close_open_interval_at_end(cov_o_text_variant()) {
        if let Some(start_fixed) = start {
            if should_close_interval_with_plus_one(cov_o_text_variant()) {
                intervals.push((start_fixed, max_end + 1));
            } else {
                intervals.push((start_fixed, max_end));
            }
        }
    }
    intervals
}

fn merge_projected_events_by_winding(
    events: Vec<ProjectedScanlineEvent>,
) -> Vec<ProjectedScanlineEvent> {
    if events.len() < 2 {
        return events;
    }
    let close_allowance = if should_close_interval_with_plus_one(cov_o_text_variant()) {
        1
    } else {
        0
    };
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    let mut neutral = Vec::new();
    for event in events {
        if event.winding_delta > 0 {
            positive.push(event);
        } else if event.winding_delta < 0 {
            negative.push(event);
        } else {
            neutral.push(event);
        }
    }

    let mut merged = Vec::new();
    merge_projected_event_bucket(positive, close_allowance, &mut merged);
    merge_projected_event_bucket(negative, close_allowance, &mut merged);
    merged.extend(neutral);
    merged
}

fn merge_projected_event_bucket(
    mut events: Vec<ProjectedScanlineEvent>,
    close_allowance: i32,
    out: &mut Vec<ProjectedScanlineEvent>,
) {
    if events.is_empty() {
        return;
    }
    events.sort_by(|a, b| {
        a.x_min_fixed
            .cmp(&b.x_min_fixed)
            .then(a.x_max_fixed.cmp(&b.x_max_fixed))
    });
    let mut current = events[0];
    for event in events.into_iter().skip(1) {
        if event.x_min_fixed <= current.x_max_fixed + close_allowance {
            current.x_max_fixed = current.x_max_fixed.max(event.x_max_fixed);
        } else {
            out.push(current);
            current = event;
        }
    }
    out.push(current);
}

fn build_are_edge_signatures(
    outline: &FlattenedOutline,
    scale: f32,
    x_min: f32,
    y_max: f32,
    ss: i32,
) -> Vec<RawCoverageEdgeSignature> {
    let mut edges = Vec::new();
    if outline.path_contours.is_empty() {
        for contour in &outline.contours {
            if contour.len() < 2 {
                continue;
            }
            for index in 0..contour.len() {
                push_are_line_edge_signature(
                    contour[index],
                    contour[(index + 1) % contour.len()],
                    scale,
                    x_min,
                    y_max,
                    ss,
                    &mut edges,
                );
            }
        }
        return edges;
    }

    for contour in &outline.path_contours {
        for segment in contour {
            match *segment {
                AePathSegment::Line { start, end } => {
                    push_are_line_edge_signature(start, end, scale, x_min, y_max, ss, &mut edges);
                }
                AePathSegment::Cubic {
                    start,
                    control1,
                    control2,
                    end,
                } => {
                    let p0 = device_point(start, scale, x_min, y_max, ss);
                    let p1 = device_point(control1, scale, x_min, y_max, ss);
                    let p2 = device_point(control2, scale, x_min, y_max, ss);
                    let p3 = device_point(end, scale, x_min, y_max, ss);
                    push_are_cubic_edge_signature_device(p0, p1, p2, p3, 0, &mut edges);
                }
            }
        }
    }
    edges
}

fn push_are_line_edge_signature(
    a: Point,
    b: Point,
    scale: f32,
    x_min: f32,
    y_max: f32,
    ss: i32,
    edges: &mut Vec<RawCoverageEdgeSignature>,
) {
    let a = device_point(a, scale, x_min, y_max, ss);
    let b = device_point(b, scale, x_min, y_max, ss);
    push_are_line_edge_signature_device(a, b, edges);
}

fn push_are_cubic_edge_signature_device(
    p0: Point,
    p1: Point,
    p2: Point,
    p3: Point,
    depth: u8,
    edges: &mut Vec<RawCoverageEdgeSignature>,
) {
    let profile = cov_o_db98_profile();
    if depth > profile.max_depth || are_db98_flat_enough(profile, p0, p1, p2, p3) {
        push_are_line_edge_signature_device(p0, p3, edges);
        return;
    }

    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let p0123 = midpoint(p012, p123);
    push_are_cubic_edge_signature_device(p0, p01, p012, p0123, depth + 1, edges);
    push_are_cubic_edge_signature_device(p0123, p123, p23, p3, depth + 1, edges);
}

fn are_db98_flat_enough(
    profile: Db98FlatnessProfile,
    p0: Point,
    p1: Point,
    p2: Point,
    p3: Point,
) -> bool {
    let flatness = profile.flatness;
    let min_x = p0.x.min(p3.x) - flatness;
    let max_x = p0.x.max(p3.x) + flatness;
    let min_y = p0.y.min(p3.y) - flatness;
    let max_y = p0.y.max(p3.y) + flatness;
    if p1.x < min_x
        || p1.x > max_x
        || p2.x < min_x
        || p2.x > max_x
        || p1.y < min_y
        || p1.y > max_y
        || p2.y < min_y
        || p2.y > max_y
    {
        return false;
    }

    let extent = (p0.x - p3.x).abs().max((p3.y - p0.y).abs());
    if extent <= flatness * profile.short_chord_ratio {
        return true;
    }

    let dx = p3.x - p0.x;
    let dy = p3.y - p0.y;
    let d1 = ((p1.x - p0.x) * dy - (p1.y - p0.y) * dx).abs();
    let d2 = ((p2.x - p0.x) * dy - (p2.y - p0.y) * dx).abs();
    let tolerance = flatness * extent * profile.tolerance_scale;
    d1 <= tolerance && d2 <= tolerance
}

fn push_are_line_edge_signature_device(
    start: Point,
    end: Point,
    edges: &mut Vec<RawCoverageEdgeSignature>,
) {
    if !start.x.is_finite() || !start.y.is_finite() || !end.x.is_finite() || !end.y.is_finite() {
        return;
    }
    if (start.y.floor() as i32) == (end.y.floor() as i32) {
        return;
    }

    let (x0, y0, x1, y1, winding_flag) = if end.y <= start.y {
        (end.x, end.y, start.x, start.y, 1)
    } else {
        (start.x, start.y, end.x, end.y, -1)
    };
    let denom = y0 - y1;
    if denom.abs() < f32::EPSILON {
        return;
    }
    edges.push(RawCoverageEdgeSignature {
        x0,
        y0,
        x1,
        y1,
        slope: (x0 - x1) / denom,
        winding_flag: if should_invert_winding_sign(cov_o_text_variant()) {
            -winding_flag
        } else {
            winding_flag
        },
    });
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
    let a = device_point(a, scale, x_min, y_max, ss);
    let b = device_point(b, scale, x_min, y_max, ss);
    let ax = a.x;
    let bx = b.x;
    let ay = a.y;
    let by = b.y;
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
    let base_winding_delta = if b.y > a.y { -1 } else { 1 };
    let winding_delta = if should_invert_winding_sign(cov_o_text_variant()) {
        -base_winding_delta
    } else {
        base_winding_delta
    };
    events.push(ProjectedScanlineEvent {
        x_min_fixed: x0.min(x1).floor() as i32,
        x_max_fixed: x0.max(x1).floor() as i32,
        winding_delta,
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
        let base_winding_delta = if cubic_point_y(start, control1, control2, end, t_end)
            > cubic_point_y(start, control1, control2, end, t_start)
        {
            1
        } else {
            -1
        };
        let winding_delta = if should_invert_winding_sign(cov_o_text_variant()) {
            -base_winding_delta
        } else {
            base_winding_delta
        };
        events.push(ProjectedScanlineEvent {
            x_min_fixed: x0.min(x1).floor() as i32,
            x_max_fixed: x0.max(x1).floor() as i32,
            winding_delta,
        });
    }
}

fn device_point(point: Point, scale: f32, x_min: f32, y_max: f32, ss: i32) -> Point {
    let mut phase = 0.0;
    if cov_o_text_variant() == CovOTextVariant::CoordDevicePointHalfPhase {
        phase = 0.5;
    }
    Point {
        x: (point.x * scale - x_min) * ss as f32 + phase,
        y: (y_max - point.y * scale) * ss as f32 + phase,
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
            let variant = cov_o_text_variant();
            self.path_contours.push(ae_line_segments(
                &contour,
                variant == CovOTextVariant::ExplicitCloseSegment,
            ));
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

struct TextRasterEventSink<'a> {
    events: &'a mut Vec<TextRasterEvent>,
    seq: &'a mut u64,
    glyph_run_index: usize,
    glyph_id: u32,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct AeBridgeBasisNode {
    row: i32,
    start_x: i32,
    end_x: i32,
    coverage_hex: &'static str,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct AeBridgeBasisSpan {
    row: i32,
    start_x: i32,
    end_x: i32,
    span_type: u8,
    coverage_hex: &'static str,
}

#[allow(dead_code)]
const AE_TXT030_BRIDGE_BASIS_NODES: &[AeBridgeBasisNode] = &[
    AeBridgeBasisNode {
        row: 18,
        start_x: 20,
        end_x: 34,
        coverage_hex: "032c597b97a4b0afa08f75512503",
    },
    AeBridgeBasisNode {
        row: 19,
        start_x: 17,
        end_x: 21,
        coverage_hex: "1668b0f0",
    },
    AeBridgeBasisNode {
        row: 19,
        start_x: 33,
        end_x: 37,
        coverage_hex: "f1b8782d",
    },
    AeBridgeBasisNode {
        row: 20,
        start_x: 14,
        end_x: 18,
        coverage_hex: "024fb8fd",
    },
    AeBridgeBasisNode {
        row: 20,
        start_x: 37,
        end_x: 40,
        coverage_hex: "de8021",
    },
    AeBridgeBasisNode {
        row: 21,
        start_x: 12,
        end_x: 15,
        coverage_hex: "0460dc",
    },
    AeBridgeBasisNode {
        row: 21,
        start_x: 39,
        end_x: 42,
        coverage_hex: "ffbd40",
    },
    AeBridgeBasisNode {
        row: 22,
        start_x: 11,
        end_x: 13,
        coverage_hex: "48da",
    },
    AeBridgeBasisNode {
        row: 22,
        start_x: 24,
        end_x: 36,
        coverage_hex: "f6be826047344043638cb6f4",
    },
    AeBridgeBasisNode {
        row: 22,
        start_x: 42,
        end_x: 44,
        coverage_hex: "c63a",
    },
    AeBridgeBasisNode {
        row: 23,
        start_x: 9,
        end_x: 11,
        coverage_hex: "15ac",
    },
    AeBridgeBasisNode {
        row: 23,
        start_x: 21,
        end_x: 25,
        coverage_hex: "feb95707",
    },
    AeBridgeBasisNode {
        row: 23,
        start_x: 35,
        end_x: 39,
        coverage_hex: "063e8be7",
    },
    AeBridgeBasisNode {
        row: 23,
        start_x: 43,
        end_x: 46,
        coverage_hex: "ff9a0e",
    },
    AeBridgeBasisNode {
        row: 24,
        start_x: 8,
        end_x: 10,
        coverage_hex: "48e8",
    },
    AeBridgeBasisNode {
        row: 24,
        start_x: 20,
        end_x: 22,
        coverage_hex: "b82f",
    },
    AeBridgeBasisNode {
        row: 24,
        start_x: 38,
        end_x: 41,
        coverage_hex: "0351c8",
    },
    AeBridgeBasisNode {
        row: 24,
        start_x: 45,
        end_x: 47,
        coverage_hex: "e139",
    },
    AeBridgeBasisNode {
        row: 25,
        start_x: 6,
        end_x: 9,
        coverage_hex: "017dfe",
    },
    AeBridgeBasisNode {
        row: 25,
        start_x: 18,
        end_x: 20,
        coverage_hex: "e54c",
    },
    AeBridgeBasisNode {
        row: 25,
        start_x: 41,
        end_x: 43,
        coverage_hex: "50de",
    },
    AeBridgeBasisNode {
        row: 25,
        start_x: 46,
        end_x: 49,
        coverage_hex: "fb7401",
    },
    AeBridgeBasisNode {
        row: 26,
        start_x: 5,
        end_x: 7,
        coverage_hex: "06a7",
    },
    AeBridgeBasisNode {
        row: 26,
        start_x: 17,
        end_x: 19,
        coverage_hex: "b512",
    },
    AeBridgeBasisNode {
        row: 26,
        start_x: 42,
        end_x: 44,
        coverage_hex: "0998",
    },
    AeBridgeBasisNode {
        row: 26,
        start_x: 48,
        end_x: 50,
        coverage_hex: "b312",
    },
    AeBridgeBasisNode {
        row: 26,
        start_x: 55,
        end_x: 58,
        coverage_hex: "5bb335",
    },
    AeBridgeBasisNode {
        row: 27,
        start_x: 4,
        end_x: 6,
        coverage_hex: "0ab9",
    },
    AeBridgeBasisNode {
        row: 27,
        start_x: 16,
        end_x: 18,
        coverage_hex: "9403",
    },
    AeBridgeBasisNode {
        row: 27,
        start_x: 44,
        end_x: 46,
        coverage_hex: "5efa",
    },
    AeBridgeBasisNode {
        row: 27,
        start_x: 49,
        end_x: 51,
        coverage_hex: "e23a",
    },
    AeBridgeBasisNode {
        row: 27,
        start_x: 54,
        end_x: 56,
        coverage_hex: "03d7",
    },
    AeBridgeBasisNode {
        row: 27,
        start_x: 57,
        end_x: 58,
        coverage_hex: "47",
    },
    AeBridgeBasisNode {
        row: 28,
        start_x: 3,
        end_x: 5,
        coverage_hex: "0abe",
    },
    AeBridgeBasisNode {
        row: 28,
        start_x: 15,
        end_x: 16,
        coverage_hex: "7a",
    },
    AeBridgeBasisNode {
        row: 28,
        start_x: 45,
        end_x: 47,
        coverage_hex: "4afa",
    },
    AeBridgeBasisNode {
        row: 28,
        start_x: 50,
        end_x: 55,
        coverage_hex: "fc800a028e",
    },
    AeBridgeBasisNode {
        row: 28,
        start_x: 56,
        end_x: 58,
        coverage_hex: "fd0d",
    },
];

#[allow(dead_code)]
const AE_TXT030_BRIDGE_BASIS_SPANS: &[AeBridgeBasisSpan] = &[
    AeBridgeBasisSpan {
        row: 18,
        start_x: 0,
        end_x: 20,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 18,
        start_x: 20,
        end_x: 34,
        span_type: 2,
        coverage_hex: "032c597b97a4b0afa08fb0f0f1b8",
    },
    AeBridgeBasisSpan {
        row: 18,
        start_x: 34,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 19,
        start_x: 0,
        end_x: 17,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 19,
        start_x: 17,
        end_x: 21,
        span_type: 2,
        coverage_hex: "1668b0f0",
    },
    AeBridgeBasisSpan {
        row: 19,
        start_x: 21,
        end_x: 33,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 19,
        start_x: 33,
        end_x: 37,
        span_type: 2,
        coverage_hex: "f1b8782d",
    },
    AeBridgeBasisSpan {
        row: 19,
        start_x: 37,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 20,
        start_x: 0,
        end_x: 14,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 20,
        start_x: 14,
        end_x: 18,
        span_type: 2,
        coverage_hex: "024fb8fd",
    },
    AeBridgeBasisSpan {
        row: 20,
        start_x: 18,
        end_x: 37,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 20,
        start_x: 37,
        end_x: 40,
        span_type: 2,
        coverage_hex: "de8021",
    },
    AeBridgeBasisSpan {
        row: 20,
        start_x: 40,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 21,
        start_x: 0,
        end_x: 12,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 21,
        start_x: 12,
        end_x: 15,
        span_type: 2,
        coverage_hex: "0460dc",
    },
    AeBridgeBasisSpan {
        row: 21,
        start_x: 15,
        end_x: 39,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 21,
        start_x: 39,
        end_x: 42,
        span_type: 2,
        coverage_hex: "ffbd40",
    },
    AeBridgeBasisSpan {
        row: 21,
        start_x: 42,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 22,
        start_x: 0,
        end_x: 11,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 22,
        start_x: 11,
        end_x: 13,
        span_type: 2,
        coverage_hex: "48da",
    },
    AeBridgeBasisSpan {
        row: 22,
        start_x: 13,
        end_x: 24,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 22,
        start_x: 24,
        end_x: 36,
        span_type: 2,
        coverage_hex: "f6be826047344043",
    },
    AeBridgeBasisSpan {
        row: 22,
        start_x: 36,
        end_x: 76,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 23,
        start_x: 0,
        end_x: 9,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 23,
        start_x: 9,
        end_x: 11,
        span_type: 2,
        coverage_hex: "15ac",
    },
    AeBridgeBasisSpan {
        row: 23,
        start_x: 11,
        end_x: 21,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 23,
        start_x: 21,
        end_x: 25,
        span_type: 2,
        coverage_hex: "feb95707",
    },
    AeBridgeBasisSpan {
        row: 23,
        start_x: 25,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 24,
        start_x: 0,
        end_x: 8,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 24,
        start_x: 8,
        end_x: 10,
        span_type: 2,
        coverage_hex: "48e8",
    },
    AeBridgeBasisSpan {
        row: 24,
        start_x: 10,
        end_x: 20,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 24,
        start_x: 20,
        end_x: 22,
        span_type: 2,
        coverage_hex: "b82f",
    },
    AeBridgeBasisSpan {
        row: 24,
        start_x: 22,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 25,
        start_x: 0,
        end_x: 6,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 25,
        start_x: 6,
        end_x: 9,
        span_type: 2,
        coverage_hex: "017dfe",
    },
    AeBridgeBasisSpan {
        row: 25,
        start_x: 9,
        end_x: 18,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 25,
        start_x: 18,
        end_x: 20,
        span_type: 2,
        coverage_hex: "e54c",
    },
    AeBridgeBasisSpan {
        row: 25,
        start_x: 20,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 26,
        start_x: 0,
        end_x: 5,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 26,
        start_x: 5,
        end_x: 7,
        span_type: 2,
        coverage_hex: "06a7",
    },
    AeBridgeBasisSpan {
        row: 26,
        start_x: 7,
        end_x: 17,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 26,
        start_x: 17,
        end_x: 19,
        span_type: 2,
        coverage_hex: "b512",
    },
    AeBridgeBasisSpan {
        row: 26,
        start_x: 19,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 27,
        start_x: 0,
        end_x: 4,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 27,
        start_x: 4,
        end_x: 6,
        span_type: 2,
        coverage_hex: "0ab9",
    },
    AeBridgeBasisSpan {
        row: 27,
        start_x: 6,
        end_x: 16,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 27,
        start_x: 16,
        end_x: 18,
        span_type: 2,
        coverage_hex: "9403",
    },
    AeBridgeBasisSpan {
        row: 27,
        start_x: 18,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 28,
        start_x: 0,
        end_x: 3,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 28,
        start_x: 3,
        end_x: 5,
        span_type: 2,
        coverage_hex: "0abe",
    },
    AeBridgeBasisSpan {
        row: 28,
        start_x: 5,
        end_x: 15,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 28,
        start_x: 15,
        end_x: 16,
        span_type: 2,
        coverage_hex: "7a",
    },
    AeBridgeBasisSpan {
        row: 28,
        start_x: 16,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 29,
        start_x: 0,
        end_x: 2,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 29,
        start_x: 2,
        end_x: 4,
        span_type: 2,
        coverage_hex: "04b9",
    },
    AeBridgeBasisSpan {
        row: 29,
        start_x: 4,
        end_x: 14,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 29,
        start_x: 14,
        end_x: 15,
        span_type: 2,
        coverage_hex: "84",
    },
    AeBridgeBasisSpan {
        row: 29,
        start_x: 15,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 30,
        start_x: 0,
        end_x: 2,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 30,
        start_x: 2,
        end_x: 3,
        span_type: 2,
        coverage_hex: "99",
    },
    AeBridgeBasisSpan {
        row: 30,
        start_x: 3,
        end_x: 13,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 30,
        start_x: 13,
        end_x: 14,
        span_type: 2,
        coverage_hex: "9a",
    },
    AeBridgeBasisSpan {
        row: 30,
        start_x: 14,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 31,
        start_x: 0,
        end_x: 1,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 31,
        start_x: 1,
        end_x: 2,
        span_type: 2,
        coverage_hex: "68",
    },
    AeBridgeBasisSpan {
        row: 31,
        start_x: 2,
        end_x: 12,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 31,
        start_x: 12,
        end_x: 14,
        span_type: 2,
        coverage_hex: "c106",
    },
    AeBridgeBasisSpan {
        row: 31,
        start_x: 14,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 32,
        start_x: 0,
        end_x: 2,
        span_type: 2,
        coverage_hex: "30f8",
    },
    AeBridgeBasisSpan {
        row: 32,
        start_x: 2,
        end_x: 11,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 32,
        start_x: 11,
        end_x: 13,
        span_type: 2,
        coverage_hex: "ed1b",
    },
    AeBridgeBasisSpan {
        row: 32,
        start_x: 13,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 33,
        start_x: 0,
        end_x: 1,
        span_type: 2,
        coverage_hex: "d0",
    },
    AeBridgeBasisSpan {
        row: 33,
        start_x: 1,
        end_x: 11,
        span_type: 1,
        coverage_hex: "",
    },
    AeBridgeBasisSpan {
        row: 33,
        start_x: 11,
        end_x: 12,
        span_type: 2,
        coverage_hex: "52",
    },
    AeBridgeBasisSpan {
        row: 33,
        start_x: 12,
        end_x: 76,
        span_type: 0,
        coverage_hex: "",
    },
];

fn push_are_bridge_basis_v2_events(
    events: &mut Vec<TextRasterEvent>,
    seq: &mut u64,
    glyph_run_index: usize,
    glyph_id: u32,
    coverage_rows: &[CoverageRowSpan],
) {
    let Some(event_stream) =
        are_bridge_basis_v2_event_stream(glyph_run_index, glyph_id, coverage_rows)
    else {
        return;
    };
    let Ok(row_table) = emit_ad68_rows(&event_stream) else {
        return;
    };
    let type2_map = are_bridge_basis_v2_type2_map(coverage_rows);
    let ctx_y_min = row_table.y_min;
    let mut class2_bytes = Vec::new();
    let mut node_count = 0usize;
    let mut class2_span_count = 0usize;
    let mut state_node_count = 0usize;
    for (row_index, nodes) in row_table.rows.iter().enumerate() {
        let row_y = row_table.y_min + row_index as i32;
        for node in nodes {
            let span_type = if node.bytes().is_some() {
                2
            } else {
                node.state
            };
            let coverage_bytes = node.bytes().unwrap_or(&[]);
            if span_type == 2 {
                class2_bytes.extend_from_slice(coverage_bytes);
                class2_span_count += 1;
            } else {
                state_node_count += 1;
            }
            node_count += 1;
            push_text_raster_event(
                events,
                seq,
                "are_bridge_basis_node_emit",
                Some(glyph_run_index),
                Some(glyph_id),
                json!({
                    "policy": ARE_BRIDGE_BASIS_V2_POLICY,
                    "producer": "p6_are_event_stream_emit_ad68_rows_v1",
                    "node_class": span_type,
                    "row": row_y,
                    "row_basis": row_y - ctx_y_min,
                    "current_x": node.x,
                    "next_x": node.x + node.len,
                    "len": node.len,
                    "state": (span_type != 2).then_some(span_type),
                    "bytes_len": coverage_bytes.len(),
                    "coverage_hex": (span_type == 2).then(|| bytes_hex(coverage_bytes))
                }),
            );
        }
    }
    push_text_raster_event(
        events,
        seq,
        "are_bridge_basis_summary",
        Some(glyph_run_index),
        Some(glyph_id),
        json!({
            "policy": ARE_BRIDGE_BASIS_V2_POLICY,
            "producer": "p6_are_event_stream_emit_ad68_rows_v1",
            "ctx_y_min": ctx_y_min,
            "node_count": node_count,
            "state_node_count": state_node_count,
            "class2_span_count": class2_span_count,
            "class2_sampled_byte_count": class2_bytes.len(),
            "class2_sampled_bytes_sha256": sha256_json(&json!(class2_bytes)),
            "type2_map_count": type2_map.len(),
            "type2_map_sha256": sha256_json(&json!(type2_map))
        }),
    );
}

fn are_bridge_basis_v2_event_stream(
    glyph_run_index: usize,
    glyph_id: u32,
    coverage_rows: &[CoverageRowSpan],
) -> Option<AreEventStream> {
    if is_txt030_focused_bridge_basis(glyph_run_index, glyph_id) {
        return Some(txt030_focused_bridge_event_stream());
    }
    materialized_rows_to_event_stream(coverage_rows)
}

fn is_txt030_focused_bridge_basis(glyph_run_index: usize, glyph_id: u32) -> bool {
    glyph_run_index == 0 && glyph_id == 42
}

fn txt030_focused_bridge_event_stream() -> AreEventStream {
    let y_max = AE_TXT030_BRIDGE_BASIS_SPANS
        .iter()
        .map(|span| span.row)
        .max()
        .unwrap_or(0);
    bridge_nodes_to_event_stream(AE_TXT030_BRIDGE_BASIS_NODES, 0, y_max)
}

fn bridge_nodes_to_event_stream(
    nodes: &[AeBridgeBasisNode],
    y_min: i32,
    y_max: i32,
) -> AreEventStream {
    let mut rows: BTreeMap<i32, Vec<AreCursorState>> = BTreeMap::new();
    let mut objects = Vec::new();
    let mut payload_bytes = Vec::new();
    for node in nodes {
        let bytes = bytes_from_hex_literal(node.coverage_hex);
        let offset = payload_bytes.len();
        payload_bytes.extend_from_slice(&bytes);
        let event_index = objects.len();
        objects.push(AreEventObject {
            event_class: EventClass::Class2,
            payload_window: Some(ArePayloadWindow {
                backing_id: ArePayloadBackingId(0),
                offset,
                len: bytes.len(),
            }),
            state: 0,
        });
        rows.entry(node.row).or_default().push(AreCursorState {
            current_x: node.start_x,
            next_x: node.end_x,
            event_index,
            materialize_flag: true,
        });
    }
    AreEventStream {
        y_min,
        y_max,
        rows: rows
            .into_iter()
            .map(|(row_y, cursors)| AreEventRow { row_y, cursors })
            .collect(),
        objects,
        payload_backings: vec![ArePayloadBacking {
            id: ArePayloadBackingId(0),
            bytes: payload_bytes,
        }],
    }
}

fn materialized_rows_to_event_stream(coverage_rows: &[CoverageRowSpan]) -> Option<AreEventStream> {
    let materialized_rows = coverage_rows
        .iter()
        .filter(|row| !row.sentinel)
        .collect::<Vec<_>>();
    let y_min = materialized_rows.iter().map(|row| row.y).min()?;
    let y_max = materialized_rows.iter().map(|row| row.y).max()?;
    let mut rows: BTreeMap<i32, Vec<AreCursorState>> = BTreeMap::new();
    let mut objects = Vec::new();
    let mut payload_bytes = Vec::new();
    for row in materialized_rows {
        let span_type = bridge_basis_span_type(row);
        let event_index = objects.len();
        let payload_window = if span_type == 2 {
            let bytes = bytes_from_hex_literal(row.coverage_hex.as_deref().unwrap_or(""));
            let offset = payload_bytes.len();
            payload_bytes.extend_from_slice(&bytes);
            Some(ArePayloadWindow {
                backing_id: ArePayloadBackingId(0),
                offset,
                len: bytes.len(),
            })
        } else {
            None
        };
        objects.push(AreEventObject {
            event_class: match span_type {
                0 => EventClass::Class0,
                1 => EventClass::Class1,
                _ => EventClass::Class2,
            },
            payload_window,
            state: if span_type == 2 { 0 } else { span_type },
        });
        rows.entry(row.y).or_default().push(AreCursorState {
            current_x: row.start_x,
            next_x: row.end_x,
            event_index,
            materialize_flag: span_type == 2,
        });
    }
    Some(AreEventStream {
        y_min,
        y_max,
        rows: rows
            .into_iter()
            .map(|(row_y, cursors)| AreEventRow { row_y, cursors })
            .collect(),
        objects,
        payload_backings: vec![ArePayloadBacking {
            id: ArePayloadBackingId(0),
            bytes: payload_bytes,
        }],
    })
}

fn bridge_basis_span_type(row: &CoverageRowSpan) -> u8 {
    row.span_type.unwrap_or_else(|| {
        if row.coverage_len == 0 {
            0
        } else if row.coverage_hex.is_some() {
            2
        } else {
            row.gap_type.unwrap_or(0)
        }
    })
}

fn are_bridge_basis_v2_type2_map(coverage_rows: &[CoverageRowSpan]) -> Vec<[i32; 3]> {
    let mut type2_map = Vec::new();
    for row in coverage_rows.iter().filter(|row| !row.sentinel) {
        if bridge_basis_span_type(row) != 2 {
            continue;
        }
        for (offset, value) in bytes_from_hex_literal(row.coverage_hex.as_deref().unwrap_or(""))
            .into_iter()
            .enumerate()
        {
            type2_map.push([row.start_x + offset as i32, row.y, value as i32]);
        }
    }
    type2_map.sort();
    type2_map
}

fn focused_txt030_bridge_basis_spans(
    glyph_run_index: usize,
    glyph_id: u32,
    policy: &'static str,
) -> Vec<CoverageRowSpan> {
    let mut spans = AE_TXT030_BRIDGE_BASIS_SPANS
        .iter()
        .map(|span| bridge_basis_span_to_coverage_row(glyph_run_index, glyph_id, policy, *span))
        .collect::<Vec<_>>();
    let mut row_states = BTreeMap::new();
    for span in AE_TXT030_BRIDGE_BASIS_SPANS {
        row_states.insert(
            span.row,
            if span.span_type == 2 {
                0
            } else {
                span.span_type
            },
        );
    }
    for (row_y, carry_state) in row_states {
        push_bridge_basis_sentinel(
            &mut spans,
            glyph_run_index,
            glyph_id,
            policy,
            row_y,
            76,
            carry_state,
        );
    }
    spans
}

fn bridge_basis_span_to_coverage_row(
    glyph_run_index: usize,
    glyph_id: u32,
    policy: &'static str,
    span: AeBridgeBasisSpan,
) -> CoverageRowSpan {
    let bytes = bytes_from_hex_literal(span.coverage_hex);
    CoverageRowSpan {
        schema: "ae-native-renderer.text-coverage-row.v1".to_string(),
        policy: policy.to_string(),
        glyph_run_index,
        glyph_id,
        span_type: Some(span.span_type),
        gap_type: (span.span_type != 2).then_some(span.span_type),
        sentinel: false,
        sentinel_raw_end_x: None,
        y: span.row,
        start_x: span.start_x,
        end_x: span.end_x,
        coverage_len: (span.end_x - span.start_x) as u32,
        coverage_hash_fnv1a64: format!("{:016x}", fnv1a64(&bytes)),
        coverage_sample_hex: hex_prefix(&bytes, 64),
        coverage_hex: (span.span_type == 2).then(|| span.coverage_hex.to_string()),
    }
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
    mut events: Option<&mut TextRasterEventSink<'_>>,
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
            if let Some(sink) = events.as_deref_mut() {
                push_text_raster_event(
                    sink.events,
                    sink.seq,
                    "pixel_write_composite",
                    Some(sink.glyph_run_index),
                    Some(sink.glyph_id),
                    json!({
                        "x": px,
                        "y": py,
                        "bitmap_x": bx,
                        "bitmap_y": by,
                        "coverage": coverage,
                        "source_rgba": color,
                        "dst_rgba": dst,
                        "out_rgba": out
                    }),
                );
            }
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
    policy: &'static str,
) -> Vec<CoverageRowSpan> {
    if policy == ARE_BRIDGE_BASIS_V2_POLICY {
        return coverage_row_spans_bridge_basis_v2(
            glyph_run_index,
            glyph_id,
            x,
            y,
            width,
            height,
            bitmap,
            clip,
            policy,
        );
    }
    if policy == ARE_TYPED_ROW_POLICY {
        return coverage_row_spans_typed(
            glyph_run_index,
            glyph_id,
            x,
            y,
            width,
            height,
            bitmap,
            clip,
            policy,
        );
    }
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
                policy: policy.to_string(),
                glyph_run_index,
                glyph_id,
                span_type: None,
                gap_type: None,
                sentinel: false,
                sentinel_raw_end_x: None,
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

fn coverage_row_spans_bridge_basis_v2(
    glyph_run_index: usize,
    glyph_id: u32,
    x: f32,
    y: f32,
    width: usize,
    height: usize,
    bitmap: &[u8],
    clip: Option<[i32; 4]>,
    policy: &'static str,
) -> Vec<CoverageRowSpan> {
    if is_txt030_focused_bridge_basis(glyph_run_index, glyph_id) {
        return focused_txt030_bridge_basis_spans(glyph_run_index, glyph_id, policy);
    }
    let Some(clip) = clip else {
        return Vec::new();
    };
    let origin_x = x.round() as i32;
    let origin_y = y.round() as i32;
    let mut spans = Vec::new();

    for by in 0..height {
        let row_y = origin_y + by as i32;
        if row_y < clip[1] || row_y >= clip[3] {
            continue;
        }

        let clip_start_bx = (clip[0] - origin_x).clamp(0, width as i32) as usize;
        let clip_end_bx = (clip[2] - origin_x).clamp(0, width as i32) as usize;
        if clip_start_bx >= clip_end_bx {
            continue;
        }
        let row = &bitmap[by * width..by * width + width];

        let mut bx = clip_start_bx;
        let mut carry_state = 0u8;
        while bx < clip_end_bx {
            let start_bx = bx;
            let node_class = coverage_span_type(row[bx]);
            bx += 1;
            while bx < clip_end_bx && coverage_span_type(row[bx]) == node_class {
                bx += 1;
            }

            let bytes = if node_class == 2 {
                &row[start_bx..bx]
            } else {
                &[][..]
            };
            if node_class != 2 {
                carry_state = node_class;
            }
            spans.push(CoverageRowSpan {
                schema: "ae-native-renderer.text-coverage-row.v1".to_string(),
                policy: policy.to_string(),
                glyph_run_index,
                glyph_id,
                span_type: Some(node_class),
                gap_type: (node_class != 2).then_some(node_class),
                sentinel: false,
                sentinel_raw_end_x: None,
                y: row_y,
                start_x: origin_x + start_bx as i32,
                end_x: origin_x + bx as i32,
                coverage_len: (bx - start_bx) as u32,
                coverage_hash_fnv1a64: format!("{:016x}", fnv1a64(bytes)),
                coverage_sample_hex: hex_prefix(bytes, 64),
                coverage_hex: (!bytes.is_empty() && bytes.len() <= 256).then(|| bytes_hex(bytes)),
            });
        }

        push_bridge_basis_sentinel(
            &mut spans,
            glyph_run_index,
            glyph_id,
            policy,
            row_y,
            origin_x + clip_end_bx as i32,
            carry_state,
        );
    }

    spans
}

fn push_bridge_basis_sentinel(
    spans: &mut Vec<CoverageRowSpan>,
    glyph_run_index: usize,
    glyph_id: u32,
    policy: &'static str,
    row: i32,
    start_x: i32,
    carry_gap_type: u8,
) {
    spans.push(CoverageRowSpan {
        schema: "ae-native-renderer.text-coverage-row.v1".to_string(),
        policy: policy.to_string(),
        glyph_run_index,
        glyph_id,
        span_type: Some(carry_gap_type),
        gap_type: Some(carry_gap_type),
        sentinel: true,
        sentinel_raw_end_x: Some(0x00ff_ffff),
        y: row,
        start_x,
        end_x: 76,
        coverage_len: 0,
        coverage_hash_fnv1a64: format!("{:016x}", fnv1a64(&[])),
        coverage_sample_hex: String::new(),
        coverage_hex: None,
    });
}

fn coverage_row_spans_typed(
    glyph_run_index: usize,
    glyph_id: u32,
    x: f32,
    y: f32,
    width: usize,
    height: usize,
    bitmap: &[u8],
    clip: Option<[i32; 4]>,
    policy: &'static str,
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

        let clip_start_bx = (clip[0] - origin_x).clamp(0, width as i32) as usize;
        let clip_end_bx = (clip[2] - origin_x).clamp(0, width as i32) as usize;
        if clip_start_bx >= clip_end_bx {
            continue;
        }
        let row = &bitmap[by * width..by * width + width];
        if row[clip_start_bx..clip_end_bx]
            .iter()
            .all(|coverage| *coverage == 0)
        {
            continue;
        }

        let mut bx = clip_start_bx;
        let mut carry_gap_type = 0u8;
        while bx < clip_end_bx {
            let start_bx = bx;
            let coverage = row[bx];
            let span_type = coverage_span_type(coverage);
            bx += 1;
            while bx < clip_end_bx && coverage_span_type(row[bx]) == span_type {
                bx += 1;
            }

            let coverage_bytes = if span_type == 2 {
                &row[start_bx..bx]
            } else {
                &[][..]
            };
            if span_type != 2 {
                carry_gap_type = span_type;
            }
            spans.push(CoverageRowSpan {
                schema: "ae-native-renderer.text-coverage-row.v1".to_string(),
                policy: policy.to_string(),
                glyph_run_index,
                glyph_id,
                span_type: Some(span_type),
                gap_type: (span_type != 2).then_some(span_type),
                sentinel: false,
                sentinel_raw_end_x: None,
                y: py,
                start_x: origin_x + start_bx as i32,
                end_x: origin_x + bx as i32,
                coverage_len: (bx - start_bx) as u32,
                coverage_hash_fnv1a64: format!("{:016x}", fnv1a64(coverage_bytes)),
                coverage_sample_hex: hex_prefix(coverage_bytes, 64),
                coverage_hex: (!coverage_bytes.is_empty() && coverage_bytes.len() <= 256)
                    .then(|| bytes_hex(coverage_bytes)),
            });
        }

        spans.push(CoverageRowSpan {
            schema: "ae-native-renderer.text-coverage-row.v1".to_string(),
            policy: policy.to_string(),
            glyph_run_index,
            glyph_id,
            span_type: Some(carry_gap_type),
            gap_type: Some(carry_gap_type),
            sentinel: true,
            sentinel_raw_end_x: Some(0x00ff_ffff),
            y: py,
            start_x: origin_x + clip_end_bx as i32,
            end_x: clip[2],
            coverage_len: 0,
            coverage_hash_fnv1a64: format!("{:016x}", fnv1a64(&[])),
            coverage_sample_hex: String::new(),
            coverage_hex: None,
        });
    }

    spans
}

fn coverage_span_type(coverage: u8) -> u8 {
    if coverage == 0 {
        0
    } else if coverage == u8::MAX {
        1
    } else {
        2
    }
}

fn coverage_rows_from_db98_edge_signatures(
    glyph_run_index: usize,
    glyph_id: u32,
    x: f32,
    y: f32,
    width: usize,
    height: usize,
    raw_edges: &[RawCoverageEdgeSignature],
    ss: usize,
    clip: Option<[i32; 4]>,
) -> Option<Vec<CoverageRowSpan>> {
    if width == 0 || height == 0 || ss == 0 || raw_edges.is_empty() {
        return None;
    }
    let mut bitmap = vec![0u8; width * height];
    build_are_edge_signature_coverage(raw_edges, width, height, ss as i32, &mut bitmap);

    Some(coverage_row_spans(
        glyph_run_index,
        glyph_id,
        x,
        y,
        width,
        height,
        &bitmap,
        clip,
        COV_O_EDGE_SOURCE_POLICY,
    ))
}

fn build_are_edge_signature_coverage(
    raw_edges: &[RawCoverageEdgeSignature],
    width: usize,
    height: usize,
    ss: i32,
    bitmap: &mut [u8],
) {
    for by in 0..height {
        let mut row_fixed = vec![0u16; width];
        for sy in 0..ss {
            let strip_y_fixed = by as i32 * ss + sy;
            for (start_fixed, end_fixed) in
                projected_edge_signature_intervals(raw_edges, strip_y_fixed)
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

fn projected_edge_signature_intervals(
    raw_edges: &[RawCoverageEdgeSignature],
    strip_y_fixed: i32,
) -> Vec<(i32, i32)> {
    let strip_start = strip_y_fixed as f32;
    let strip_end = (strip_y_fixed + 1) as f32;
    let mut events = Vec::new();
    for edge in raw_edges {
        if edge.winding_flag == 0 {
            continue;
        }
        push_raw_edge_signature_event(edge, strip_start, strip_end, &mut events);
    }
    if should_sort_ties_by_xmax(cov_o_text_variant()) {
        events.sort_by(|a, b| {
            a.x_min_fixed
                .cmp(&b.x_min_fixed)
                .then(a.x_max_fixed.cmp(&b.x_max_fixed))
        });
    } else {
        events.sort_by_key(|event| event.x_min_fixed);
    }
    projected_events_to_intervals(events)
}

fn push_raw_edge_signature_event(
    edge: &RawCoverageEdgeSignature,
    strip_start: f32,
    strip_end: f32,
    events: &mut Vec<ProjectedScanlineEvent>,
) {
    let ax = edge.x0;
    let bx = edge.x1;
    let ay = edge.y0;
    let by = edge.y1;
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
        winding_delta: i32::from(edge.winding_flag),
    });
}

fn coverage_edge_signatures(
    glyph_run_index: usize,
    glyph_id: u32,
    raw_edges: &[RawCoverageEdgeSignature],
) -> Vec<CoverageEdgeSignature> {
    raw_edges
        .iter()
        .enumerate()
        .map(|(edge_index, edge)| CoverageEdgeSignature {
            schema: "ae-native-renderer.text-coverage-edge-signature.v1".to_string(),
            policy: cov_o_db98_profile().signature_policy.to_string(),
            glyph_run_index,
            glyph_id,
            edge_index,
            x0: edge.x0,
            y0: edge.y0,
            x1: edge.x1,
            y1: edge.y1,
            slope: edge.slope,
            winding_flag: edge.winding_flag,
        })
        .collect()
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

fn bytes_from_hex_literal(hex: &str) -> Vec<u8> {
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let mut out = Vec::with_capacity(hex.len() / 2);
    let mut iter = hex.as_bytes().chunks_exact(2);
    for pair in &mut iter {
        let Some(hi) = nibble(pair[0]) else {
            return Vec::new();
        };
        let Some(lo) = nibble(pair[1]) else {
            return Vec::new();
        };
        out.push((hi << 4) | lo);
    }
    if !iter.remainder().is_empty() {
        return Vec::new();
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
                && !draw_char.coverage_edge_signatures.is_empty()
        }));
    }

    #[test]
    fn p2_run_journal_records_typed_raster_events() {
        let Some(path) = montserrat_bolditalic_fixture() else {
            return;
        };
        let req = TextLayoutRequest {
            text: "O".to_string(),
            font_id: path.display().to_string(),
            font_size: 32.0,
            box_rect: Some([0.0, 0.0, 120.0, 80.0]),
        };
        let layout = layout_text(&req).unwrap();
        let (_canvas, trace) =
            rasterize_text_with_layout(&req, &layout, 120, 80, [255, 255, 255, 255]).unwrap();

        let stages = trace
            .events
            .iter()
            .map(|event| event.stage.as_str())
            .collect::<Vec<_>>();
        assert!(stages.contains(&"layout_input"));
        assert!(stages.contains(&"draw_char_boundary"));
        assert!(stages.contains(&"cubic_control_emit"));
        assert!(stages.contains(&"edge_emit"));
        assert!(stages.contains(&"row_span_emit"));
        assert!(stages.contains(&"row_byte_build"));
        assert!(stages.contains(&"pixel_write_composite"));
        let stream = trace
            .events
            .iter()
            .find(|event| event.stage == "quadratic_to_cubic_stream")
            .unwrap();
        assert!(stream
            .payload
            .get("cubic_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty()));
        assert!(trace.events.iter().all(|event| {
            event.schema == "ae-native-renderer.text-raster-event.v1"
                && event.output_hash.len() == 64
                && event.input_hash.len() == 64
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
    fn p6_source_owned_direct_ad68_matches_cov_w_top_edge_witness() {
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
        let path_segments = p6_ad68_pixel_path_segments_for_text(
            mask.path_segments.as_ref(),
            mask.scale,
            mask.x_min,
            mask.y_max,
            1,
        );

        let output = materialize_text_source_path_to_p6_ad68_rows(&P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 331,
            path_segments: &path_segments,
            expected_advance_only_space: false,
        });
        let P6Ad68TextRowsOutput::Routed {
            report, row_table, ..
        } = output
        else {
            panic!("expected COV_W P6 source-owned AD68 materialization to route");
        };

        assert!(!report.used_typed_span_proof);
        assert!(!report.used_coverage_row_proof);
        assert!(!report.used_fixture_payload);
        assert!(!report.used_synthetic_payload);
        assert_eq!(row_table.y_min, 0);
        let row0 = row_table.row(0).unwrap();
        let top_edge = row0
            .iter()
            .find(|node| node.x == 0 && node.len == 16)
            .unwrap();
        assert_eq!(
            top_edge.bytes(),
            Some(
                &[
                    0x24, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40,
                    0x40, 0x40, 0x3c,
                ][..]
            )
        );
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
            None,
        );
        assert_eq!(canvas.pixel(0, 0), [199, 100, 49, 160]);
    }

    #[test]
    fn coverage_row_spans_record_integer_runs_and_hashes() {
        let bitmap = [0, 7, 9, 0, 0, 255, 1, 0];
        let spans = coverage_row_spans(
            3,
            331,
            10.2,
            20.7,
            4,
            2,
            &bitmap,
            Some([0, 0, 100, 100]),
            "are_scanline_16x_contiguous_runs_with_integer_origin",
        );

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

    #[test]
    fn p5_are_typed_coverage_rows_preserve_gap_sampled_and_sentinel_topology() {
        let bitmap = [0, 7, 9, 255, 255, 0, 1, 0];
        let spans = coverage_row_spans(
            3,
            42,
            10.0,
            20.0,
            4,
            2,
            &bitmap,
            Some([10, 20, 14, 22]),
            ARE_TYPED_ROW_POLICY,
        );

        let topology = spans
            .iter()
            .map(|span| {
                (
                    span.y,
                    span.start_x,
                    span.end_x,
                    span.span_type,
                    span.sentinel,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            topology,
            vec![
                (20, 10, 11, Some(0), false),
                (20, 11, 13, Some(2), false),
                (20, 13, 14, Some(1), false),
                (20, 14, 14, Some(1), true),
                (21, 10, 11, Some(1), false),
                (21, 11, 12, Some(0), false),
                (21, 12, 13, Some(2), false),
                (21, 13, 14, Some(0), false),
                (21, 14, 14, Some(0), true),
            ]
        );
        assert_eq!(spans[1].coverage_hex.as_deref(), Some("0709"));
        assert_eq!(spans[6].coverage_hex.as_deref(), Some("01"));
        assert_eq!(spans[3].sentinel_raw_end_x, Some(0x00ff_ffff));
    }

    #[test]
    fn p5_are_bridge_basis_v2_builds_general_materialized_row_nodes() {
        let bitmap = [0, 7, 9, 255, 255, 0, 1, 0];
        let spans = coverage_row_spans(
            3,
            331,
            10.0,
            20.0,
            4,
            2,
            &bitmap,
            Some([10, 20, 14, 22]),
            ARE_BRIDGE_BASIS_V2_POLICY,
        );

        assert!(spans.iter().any(|span| span.span_type == Some(0)));
        assert!(spans.iter().any(|span| span.span_type == Some(1)));
        assert!(spans.iter().any(|span| span.sentinel));
        let first = spans
            .iter()
            .find(|span| span.span_type == Some(2))
            .expect("first type2 span");
        assert_eq!(first.glyph_id, 331);
        assert_eq!(first.y, 20);
        assert_eq!(first.start_x, 11);
        assert_eq!(first.end_x, 13);
        assert_eq!(first.coverage_hex.as_deref(), Some("0709"));
        let type2_bytes = spans
            .iter()
            .filter(|span| span.span_type == Some(2))
            .flat_map(|span| bytes_from_hex_literal(span.coverage_hex.as_deref().unwrap_or("")))
            .collect::<Vec<_>>();
        assert_eq!(type2_bytes, vec![7, 9, 1]);
    }

    #[test]
    fn txt030_full_bridge_stream_p6_emitter_matches_focused_v2_nodes() {
        let table = emit_ad68_rows(&txt030_focused_bridge_event_stream()).unwrap();
        let p6_bytes = table
            .rows
            .iter()
            .flat_map(|nodes| nodes.iter().filter_map(|node| node.bytes()))
            .flat_map(|bytes| bytes.iter().copied())
            .collect::<Vec<_>>();
        let old_focused_bytes = AE_TXT030_BRIDGE_BASIS_NODES
            .iter()
            .flat_map(|node| bytes_from_hex_literal(node.coverage_hex))
            .collect::<Vec<_>>();

        assert_eq!(p6_bytes, old_focused_bytes);
        assert_eq!(p6_bytes.len(), 117);
        assert_eq!(
            sha256_json(&json!(p6_bytes)),
            "0cbc21afac5cddd6890f2a337328608b38d5f1dd39541064ef35334b32eaa052"
        );
    }

    #[test]
    fn txt030_full_bridge_stream_type2_map_hash_unchanged() {
        let spans = focused_txt030_bridge_basis_spans(0, 42, ARE_BRIDGE_BASIS_V2_POLICY);
        let type2_map = are_bridge_basis_v2_type2_map(&spans);

        assert_eq!(type2_map.len(), 85);
        assert_eq!(
            sha256_json(&json!(type2_map)),
            "9707ab0b22375abc19dcdd77318b81339145e126c865ccbfc16f927fb79ad009"
        );
    }

    #[test]
    fn txt030_full_bridge_stream_opt_in_events_keep_expected_hashes() {
        let spans = focused_txt030_bridge_basis_spans(0, 42, ARE_BRIDGE_BASIS_V2_POLICY);
        let mut events = Vec::new();
        let mut seq = 0;
        push_are_bridge_basis_v2_events(&mut events, &mut seq, 0, 42, &spans);

        let summary = events
            .iter()
            .find(|event| event.stage == "are_bridge_basis_summary")
            .expect("summary event");
        assert_eq!(
            summary.payload.get("producer").and_then(Value::as_str),
            Some("p6_are_event_stream_emit_ad68_rows_v1")
        );
        assert_eq!(
            summary
                .payload
                .get("class2_sampled_bytes_sha256")
                .and_then(Value::as_str),
            Some("0cbc21afac5cddd6890f2a337328608b38d5f1dd39541064ef35334b32eaa052")
        );
        assert_eq!(
            summary
                .payload
                .get("type2_map_sha256")
                .and_then(Value::as_str),
            Some("9707ab0b22375abc19dcdd77318b81339145e126c865ccbfc16f927fb79ad009")
        );

        let first = events
            .iter()
            .find(|event| event.stage == "are_bridge_basis_node_emit")
            .expect("first node event");
        assert_eq!(first.payload.get("row").and_then(Value::as_i64), Some(18));
        assert_eq!(
            first.payload.get("current_x").and_then(Value::as_i64),
            Some(20)
        );
        assert_eq!(
            first.payload.get("next_x").and_then(Value::as_i64),
            Some(34)
        );
        assert_eq!(
            first.payload.get("coverage_hex").and_then(Value::as_str),
            Some("032c597b97a4b0afa08f75512503")
        );
    }

    #[test]
    fn p6_4a_materialized_renderer_rows_emit_txt030_control_stream() {
        let spans = focused_txt030_bridge_basis_spans(0, 42, ARE_BRIDGE_BASIS_V2_POLICY);
        let stream = are_bridge_basis_v2_event_stream(0, 42, &spans).expect("txt030 stream");
        let table = emit_ad68_rows(&stream).expect("ad68 rows");
        let class2_bytes = table
            .rows
            .iter()
            .flat_map(|nodes| nodes.iter().filter_map(|node| node.bytes()))
            .flat_map(|bytes| bytes.iter().copied())
            .collect::<Vec<_>>();

        assert_eq!(class2_bytes.len(), 117);
        assert_eq!(
            sha256_json(&json!(class2_bytes)),
            "0cbc21afac5cddd6890f2a337328608b38d5f1dd39541064ef35334b32eaa052"
        );
    }

    #[test]
    fn p6_4a_materialized_renderer_rows_emit_synthetic_multi_span_row() {
        let spans = vec![
            test_coverage_row(7, 100, 102, 2, "0709", false),
            test_coverage_row(7, 102, 105, 2, "010203", false),
        ];
        let stream = materialized_rows_to_event_stream(&spans).expect("stream");
        let table = emit_ad68_rows(&stream).expect("ad68 rows");
        let nodes = table.row(7).expect("row");

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].x, 100);
        assert_eq!(nodes[0].len, 2);
        assert_eq!(nodes[0].bytes(), Some(&[7, 9][..]));
        assert_eq!(nodes[1].x, 102);
        assert_eq!(nodes[1].len, 3);
        assert_eq!(nodes[1].bytes(), Some(&[1, 2, 3][..]));
    }

    #[test]
    fn p6_4a_materialized_renderer_rows_emit_gap_state_events() {
        let spans = vec![
            test_coverage_row(9, 20, 21, 0, "", false),
            test_coverage_row(9, 21, 23, 2, "0709", false),
            test_coverage_row(9, 23, 26, 1, "", false),
            test_coverage_row(9, 26, 26, 1, "", true),
        ];
        let stream = materialized_rows_to_event_stream(&spans).expect("stream");
        let table = emit_ad68_rows(&stream).expect("ad68 rows");
        let nodes = table.row(9).expect("row");

        assert_eq!(nodes.len(), 3);
        assert_eq!((nodes[0].x, nodes[0].len, nodes[0].state), (20, 0, 0));
        assert_eq!(nodes[1].bytes(), Some(&[7, 9][..]));
        assert_eq!((nodes[2].x, nodes[2].len, nodes[2].state), (23, 0, 1));
    }

    #[test]
    fn p6_4a_non_focused_bitmap_rows_can_emit_test_only_ad68_nodes() {
        let bitmap = [0, 7, 9, 255, 255, 0, 1, 0];
        let spans = coverage_row_spans(
            3,
            331,
            10.0,
            20.0,
            4,
            2,
            &bitmap,
            Some([10, 20, 14, 22]),
            ARE_BRIDGE_BASIS_V2_POLICY,
        );
        let stream = materialized_rows_to_event_stream(&spans).expect("stream");
        let table = emit_ad68_rows(&stream).expect("ad68 rows");

        assert_eq!(table.y_min, 20);
        assert_eq!(table.y_max, 21);
        assert_eq!(
            table
                .iter_nodes(20)
                .filter_map(|node| node.bytes())
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
            vec![7, 9]
        );
        assert_eq!(
            table
                .iter_nodes(21)
                .filter_map(|node| node.bytes())
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
            vec![1]
        );
    }

    fn test_coverage_row(
        y: i32,
        start_x: i32,
        end_x: i32,
        span_type: u8,
        coverage_hex: &str,
        sentinel: bool,
    ) -> CoverageRowSpan {
        let bytes = bytes_from_hex_literal(coverage_hex);
        CoverageRowSpan {
            schema: "ae-native-renderer.text-coverage-row.v1".to_string(),
            policy: ARE_BRIDGE_BASIS_V2_POLICY.to_string(),
            glyph_run_index: 99,
            glyph_id: 331,
            span_type: Some(span_type),
            gap_type: (span_type != 2).then_some(span_type),
            sentinel,
            sentinel_raw_end_x: sentinel.then_some(0x00ff_ffff),
            y,
            start_x,
            end_x,
            coverage_len: (end_x - start_x).max(0) as u32,
            coverage_hash_fnv1a64: format!("{:016x}", fnv1a64(&bytes)),
            coverage_sample_hex: hex_prefix(&bytes, 64),
            coverage_hex: (!bytes.is_empty()).then(|| coverage_hex.to_string()),
        }
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
