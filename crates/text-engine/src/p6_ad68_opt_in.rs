use serde_json::{json, Value};

use crate::{
    build_materializer_input_from_source, emit_ad68_rows, materialize_95cc_intervals,
    materializer_row_event_projection_debug, source_owned_materializer_to_event_stream,
    Ad68RowTable, Are95ccStateClass, AreBezierSourcePathInput, AreContourClose, AreCubicControl,
    AreEventStream, ArePathPoint, ArePathTransform, ArePathVerb, AreSourceContourId,
    AreSourcePathProvenance, AreSourceSegment, AreSourceSegmentId, AreSourceSegmentKind,
};

pub const P6_AD68_TEXT_OPT_IN_ENV_VAR: &str = "AE_NATIVE_RENDERER_P6_AD68_TEXT_OPT_IN";
pub const P6_AD68_TEXT_ROW_BYTE_DEBUG_ENV_VAR: &str =
    "AE_NATIVE_RENDERER_P6_AD68_TEXT_ROW_BYTE_DEBUG";
pub const P6_AD68_TEXT_FRONTIER_DEBUG_ENV_VAR: &str =
    "AE_NATIVE_RENDERER_P6_AD68_TEXT_FRONTIER_DEBUG";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum P6Ad68OptInFlagState {
    DisabledDefault,
    DisabledExplicit,
    DisabledInvalid,
    Enabled,
}

impl P6Ad68OptInFlagState {
    pub fn enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DisabledDefault => "disabled_default",
            Self::DisabledExplicit => "disabled_explicit",
            Self::DisabledInvalid => "disabled_invalid",
            Self::Enabled => "enabled",
        }
    }
}

pub fn p6_ad68_opt_in_enabled() -> bool {
    p6_ad68_opt_in_flag_state().enabled()
}

pub fn p6_ad68_opt_in_flag_state() -> P6Ad68OptInFlagState {
    p6_ad68_opt_in_flag_state_from_value(std::env::var(P6_AD68_TEXT_OPT_IN_ENV_VAR).ok())
}

fn p6_ad68_opt_in_flag_state_from_value(value: Option<String>) -> P6Ad68OptInFlagState {
    let Some(value) = value else {
        return P6Ad68OptInFlagState::DisabledDefault;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "false" | "off" | "no" | "disabled" => P6Ad68OptInFlagState::DisabledExplicit,
        "1" | "true" | "on" | "yes" | "enabled" | "p6_ad68" | "p6-ad68" => {
            P6Ad68OptInFlagState::Enabled
        }
        _ => P6Ad68OptInFlagState::DisabledInvalid,
    }
}

pub fn p6_ad68_flag_trace_payload(flag_state: P6Ad68OptInFlagState) -> Value {
    json!({
        "env_var": P6_AD68_TEXT_OPT_IN_ENV_VAR,
        "state": flag_state.as_str(),
        "enabled": flag_state.enabled(),
        "default_behavior_unchanged": !flag_state.enabled(),
        "production_default": false
    })
}

pub fn p6_ad68_row_byte_debug_limit() -> Option<usize> {
    p6_ad68_debug_limit_from_value(std::env::var(P6_AD68_TEXT_ROW_BYTE_DEBUG_ENV_VAR).ok())
}

pub fn p6_ad68_frontier_debug_limit() -> Option<usize> {
    p6_ad68_debug_limit_from_value(std::env::var(P6_AD68_TEXT_FRONTIER_DEBUG_ENV_VAR).ok())
}

fn p6_ad68_debug_limit_from_value(value: Option<String>) -> Option<usize> {
    let value = value?;
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "false" | "off" | "no" | "disabled" => None,
        "1" | "true" | "on" | "yes" | "enabled" => Some(256),
        "all" | "full" => Some(usize::MAX),
        other => other.parse::<usize>().ok().filter(|limit| *limit > 0),
    }
}

pub fn p6_ad68_row_byte_debug_payload(row_table: &Ad68RowTable, max_nodes: usize) -> Value {
    let node_count = row_table.rows.iter().map(Vec::len).sum::<usize>();
    let mut class0_count = 0usize;
    let mut class1_count = 0usize;
    let mut class2_count = 0usize;
    let mut class2_byte_count = 0usize;
    let mut sampled_nodes = Vec::new();

    for (row_index, nodes) in row_table.rows.iter().enumerate() {
        let row_y = row_table.y_min + row_index as i32;
        for node in nodes {
            let bytes = node.bytes();
            let node_class = if let Some(bytes) = bytes {
                class2_count += 1;
                class2_byte_count += bytes.len();
                2u8
            } else if node.state == 0 {
                class0_count += 1;
                0u8
            } else {
                class1_count += 1;
                1u8
            };
            if sampled_nodes.len() < max_nodes {
                sampled_nodes.push(json!({
                    "row_y": row_y,
                    "row_basis": row_y - row_table.y_min,
                    "current_x": node.x,
                    "next_x": node.x + node.len,
                    "len": node.len,
                    "node_class": node_class,
                    "state": if bytes.is_some() { Value::Null } else { json!(node.state) },
                    "coverage_hex": bytes.map(bytes_hex),
                    "coverage_len": bytes.map_or(0, |bytes| bytes.len())
                }));
            }
        }
    }

    json!({
        "schema": "ae-native-renderer.p6-ad68-row-byte-debug.v1",
        "env_var": P6_AD68_TEXT_ROW_BYTE_DEBUG_ENV_VAR,
        "y_min": row_table.y_min,
        "y_max": row_table.y_max,
        "row_count": row_table.rows.len(),
        "node_count": node_count,
        "class0_count": class0_count,
        "class1_count": class1_count,
        "class2_count": class2_count,
        "class2_byte_count": class2_byte_count,
        "sampled_node_limit": max_nodes,
        "sampled_node_count": sampled_nodes.len(),
        "truncated": sampled_nodes.len() < node_count,
        "nodes": sampled_nodes
    })
}

pub fn p6_ad68_frontier_debug_payload(input: &P6Ad68TextPathInput<'_>, max_items: usize) -> Value {
    let Some(source_input) = p6_source_path_to_ad68_rows_for_text(
        input.glyph_run_index,
        input.glyph_id,
        input.path_segments,
    ) else {
        return json!({
            "schema": "ae-native-renderer.p6-ad68-frontier-debug.v1",
            "env_var": P6_AD68_TEXT_FRONTIER_DEBUG_ENV_VAR,
            "status": "source_path_missing",
            "glyph_run_index": input.glyph_run_index,
            "glyph_id": input.glyph_id
        });
    };

    let materializer_input = match build_materializer_input_from_source(&source_input) {
        Ok(materializer_input) => materializer_input,
        Err(err) => {
            return json!({
                "schema": "ae-native-renderer.p6-ad68-frontier-debug.v1",
                "env_var": P6_AD68_TEXT_FRONTIER_DEBUG_ENV_VAR,
                "status": "source_path_rejected",
                "glyph_run_index": input.glyph_run_index,
                "glyph_id": input.glyph_id,
                "error": format!("{:?}", err)
            });
        }
    };
    let materialized = match materialize_95cc_intervals(&materializer_input) {
        Ok(materialized) => materialized,
        Err(err) => {
            return json!({
                "schema": "ae-native-renderer.p6-ad68-frontier-debug.v1",
                "env_var": P6_AD68_TEXT_FRONTIER_DEBUG_ENV_VAR,
                "status": "materializer_error",
                "glyph_run_index": input.glyph_run_index,
                "glyph_id": input.glyph_id,
                "source_record_count": materializer_input.working_set.records.len(),
                "interval_row_count": materializer_input.interval_list.rows.len(),
                "error": format!("{:?}", err)
            });
        }
    };

    let working_set = &materializer_input.working_set;
    let source_records = working_set
        .records
        .iter()
        .enumerate()
        .take(max_items)
        .map(|(record_index, record)| {
            let segment = working_set.source_segment_for_record(record);
            json!({
                "record_index": record_index,
                "source_point_index": record.source_point_index_0x00,
                "min_x": record.min_x_0x08,
                "min_y": record.min_y_0x0c,
                "max_x": record.max_x_0x10,
                "max_y": record.max_y_0x14,
                "record_flag": record.record_flag_0x18,
                "merge_state": record.merge_state_0x19,
                "contour_id": segment.map(|segment| segment.contour_id.0),
                "segment_id": segment.map(|segment| segment.segment_id.0),
                "verb": segment.map(|segment| format!("{:?}", segment.verb)),
                "previous_point": segment.and_then(|segment| segment.previous_point).map(point_debug_json),
                "endpoint": segment.map(|segment| point_debug_json(segment.endpoint)),
                "segment_kind": segment.map(segment_kind_debug_json),
                "glyph_run_index": segment.and_then(|segment| segment.glyph_run_index),
                "glyph_id": segment.and_then(|segment| segment.glyph_id)
            })
        })
        .collect::<Vec<_>>();
    let interval_rows = materializer_input
        .interval_list
        .rows
        .iter()
        .filter(|row| !row.runs.is_empty())
        .take(max_items)
        .map(|row| {
            json!({
                "row_y": row.row_y,
                "run_count": row.runs.len(),
                "runs": row.runs.iter().take(max_items).enumerate().map(|(run_index, run)| json!({
                    "run_index": run_index,
                    "current_x": run.current_x,
                    "next_x": run.next_x,
                    "len": run.width(),
                    "tag": format!("{:?}", run.tag),
                    "materialize_candidate": run.materialize_candidate,
                    "source_record_indices": run.source_record_indices
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let materialized_intervals = materialized
        .intervals
        .iter()
        .take(max_items)
        .map(|interval| {
            json!({
                "row_y": interval.row_y,
                "run_index": interval.run_index,
                "current_x": interval.current_x,
                "next_x": interval.next_x,
                "len": interval.width(),
                "source_record_indices": interval.source_record_indices,
                "class": if interval.payload.is_some() {
                    "class2"
                } else {
                    match interval.state_hint.map(|hint| hint.state_class) {
                        Some(Are95ccStateClass::Class1) => "class1",
                        _ => "class0"
                    }
                },
                "coverage_hex": interval.payload.as_ref().map(|payload| bytes_hex(&payload.bytes)),
                "coverage_len": interval.payload.as_ref().map_or(0, |payload| payload.bytes.len())
            })
        })
        .collect::<Vec<_>>();
    let row_event_projection_debug =
        match materializer_row_event_projection_debug(&materializer_input, max_items) {
            Ok(runs) => json!({
                "status": "ok",
                "sampled_run_count": runs.len(),
                "runs": runs
            }),
            Err(err) => json!({
                "status": "error",
                "error": format!("{:?}", err)
            }),
        };

    json!({
        "schema": "ae-native-renderer.p6-ad68-frontier-debug.v1",
        "env_var": P6_AD68_TEXT_FRONTIER_DEBUG_ENV_VAR,
        "status": "ok",
        "glyph_run_index": input.glyph_run_index,
        "glyph_id": input.glyph_id,
        "source_provenance": format!("{:?}", source_input.provenance),
        "source_point_count": source_input.points.len(),
        "source_segment_count": source_input.segments.len(),
        "source_record_count": working_set.records.len(),
        "source_bounds": {
            "x_min": working_set.bounds.x_min,
            "y_min": working_set.bounds.y_min,
            "x_max": working_set.bounds.x_max,
            "y_max": working_set.bounds.y_max
        },
        "interval_y_min": materializer_input.interval_list.y_min,
        "interval_y_max": materializer_input.interval_list.y_max,
        "interval_row_count": materializer_input.interval_list.rows.len(),
        "materialized_y_min": materialized.y_min,
        "materialized_y_max": materialized.y_max,
        "materialized_interval_count": materialized.intervals.len(),
        "sampled_item_limit": max_items,
        "source_records": source_records,
        "interval_rows": interval_rows,
        "materialized_intervals": materialized_intervals,
        "row_event_projection": row_event_projection_debug
    })
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

fn point_debug_json(point: ArePathPoint) -> Value {
    json!({
        "x": point.x,
        "y": point.y
    })
}

fn segment_kind_debug_json(segment: &AreSourceSegment) -> Value {
    match &segment.kind {
        AreSourceSegmentKind::Line => json!({
            "type": "line"
        }),
        AreSourceSegmentKind::Close(close) => json!({
            "type": "close",
            "from": point_debug_json(close.from),
            "to": point_debug_json(close.to)
        }),
        AreSourceSegmentKind::Quadratic {
            control: Some(control),
        } => json!({
            "type": "quadratic",
            "control": point_debug_json(control.control)
        }),
        AreSourceSegmentKind::Quadratic { control: None } => json!({
            "type": "quadratic",
            "control": Value::Null
        }),
        AreSourceSegmentKind::Cubic {
            controls: Some(controls),
        } => json!({
            "type": "cubic",
            "control_1": point_debug_json(controls.control_1),
            "control_2": point_debug_json(controls.control_2)
        }),
        AreSourceSegmentKind::Cubic { controls: None } => json!({
            "type": "cubic",
            "control_1": Value::Null,
            "control_2": Value::Null
        }),
        AreSourceSegmentKind::Move => json!({
            "type": "move"
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P6Ad68PathPoint {
    pub x: f32,
    pub y: f32,
}

impl P6Ad68PathPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum P6Ad68PathSegment {
    Line {
        start: P6Ad68PathPoint,
        end: P6Ad68PathPoint,
    },
    Cubic {
        start: P6Ad68PathPoint,
        control1: P6Ad68PathPoint,
        control2: P6Ad68PathPoint,
        end: P6Ad68PathPoint,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P6Ad68RawPathSegment {
    pub contour_index: usize,
    pub segment_index: usize,
    pub segment: P6Ad68PathSegment,
}

#[derive(Clone, Copy, Debug)]
pub struct P6Ad68TextPathInput<'a> {
    pub glyph_run_index: usize,
    pub glyph_id: u32,
    pub path_segments: &'a [P6Ad68RawPathSegment],
    pub expected_advance_only_space: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum P6Ad68TextRouteStatus {
    Disabled,
    Routed,
    ExpectedAdvanceOnlySpace,
    UnsupportedSourcePathMissing,
    UnsupportedSourcePathRejected,
    MaterializerError,
    EventCursorError,
    Ad68EmitError,
}

impl P6Ad68TextRouteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Routed => "routed",
            Self::ExpectedAdvanceOnlySpace => "expected_advance_only_space",
            Self::UnsupportedSourcePathMissing => "unsupported_source_path_missing",
            Self::UnsupportedSourcePathRejected => "unsupported_source_path_rejected",
            Self::MaterializerError => "materializer_error",
            Self::EventCursorError => "event_cursor_error",
            Self::Ad68EmitError => "ad68_emit_error",
        }
    }

    pub fn fallback_to_default_path(&self) -> bool {
        !matches!(self, Self::Routed | Self::ExpectedAdvanceOnlySpace)
    }

    pub fn materializer_failure(&self) -> bool {
        matches!(
            self,
            Self::MaterializerError | Self::EventCursorError | Self::Ad68EmitError
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P6Ad68TextRouteReport {
    pub status: P6Ad68TextRouteStatus,
    pub glyph_run_index: usize,
    pub glyph_id: u32,
    pub source_provenance: Option<AreSourcePathProvenance>,
    pub row_count: usize,
    pub node_count: usize,
    pub class0_count: usize,
    pub class1_count: usize,
    pub class2_count: usize,
    pub class2_byte_count: usize,
    pub event_stream_row_count: usize,
    pub reason: Option<String>,
    pub used_typed_span_proof: bool,
    pub used_coverage_row_proof: bool,
    pub used_fixture_payload: bool,
    pub used_synthetic_payload: bool,
}

impl P6Ad68TextRouteReport {
    #[cfg(test)]
    fn disabled(glyph_run_index: usize, glyph_id: u32) -> Self {
        Self {
            status: P6Ad68TextRouteStatus::Disabled,
            glyph_run_index,
            glyph_id,
            source_provenance: None,
            row_count: 0,
            node_count: 0,
            class0_count: 0,
            class1_count: 0,
            class2_count: 0,
            class2_byte_count: 0,
            event_stream_row_count: 0,
            reason: Some("p6_ad68_opt_in_disabled".to_string()),
            used_typed_span_proof: false,
            used_coverage_row_proof: false,
            used_fixture_payload: false,
            used_synthetic_payload: false,
        }
    }

    fn unsupported(
        glyph_run_index: usize,
        glyph_id: u32,
        status: P6Ad68TextRouteStatus,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            status,
            glyph_run_index,
            glyph_id,
            source_provenance: None,
            row_count: 0,
            node_count: 0,
            class0_count: 0,
            class1_count: 0,
            class2_count: 0,
            class2_byte_count: 0,
            event_stream_row_count: 0,
            reason: Some(reason.into()),
            used_typed_span_proof: false,
            used_coverage_row_proof: false,
            used_fixture_payload: false,
            used_synthetic_payload: false,
        }
    }

    fn expected_advance_only_space(glyph_run_index: usize, glyph_id: u32) -> Self {
        Self {
            status: P6Ad68TextRouteStatus::ExpectedAdvanceOnlySpace,
            glyph_run_index,
            glyph_id,
            source_provenance: None,
            row_count: 0,
            node_count: 0,
            class0_count: 0,
            class1_count: 0,
            class2_count: 0,
            class2_byte_count: 0,
            event_stream_row_count: 0,
            reason: Some("advance_only_space_has_no_source_outline".to_string()),
            used_typed_span_proof: false,
            used_coverage_row_proof: false,
            used_fixture_payload: false,
            used_synthetic_payload: false,
        }
    }

    fn routed(
        glyph_run_index: usize,
        glyph_id: u32,
        source_provenance: AreSourcePathProvenance,
        event_stream: &AreEventStream,
        row_table: &Ad68RowTable,
    ) -> Self {
        let mut class0_count = 0usize;
        let mut class1_count = 0usize;
        let mut class2_count = 0usize;
        let mut class2_byte_count = 0usize;
        for node in row_table.rows.iter().flatten() {
            if let Some(bytes) = node.bytes() {
                class2_count += 1;
                class2_byte_count += bytes.len();
            } else if node.state == 0 {
                class0_count += 1;
            } else {
                class1_count += 1;
            }
        }
        Self {
            status: P6Ad68TextRouteStatus::Routed,
            glyph_run_index,
            glyph_id,
            source_provenance: Some(source_provenance),
            row_count: row_table.rows.len(),
            node_count: row_table.rows.iter().map(Vec::len).sum(),
            class0_count,
            class1_count,
            class2_count,
            class2_byte_count,
            event_stream_row_count: event_stream.rows.len(),
            reason: None,
            used_typed_span_proof: false,
            used_coverage_row_proof: false,
            used_fixture_payload: false,
            used_synthetic_payload: false,
        }
    }

    pub fn trace_payload(&self) -> Value {
        json!({
            "env_var": P6_AD68_TEXT_OPT_IN_ENV_VAR,
            "status": self.status.as_str(),
            "success": self.status == P6Ad68TextRouteStatus::Routed,
            "reason": self.reason,
            "source_provenance": self.source_provenance.map(|provenance| format!("{:?}", provenance)),
            "row_count": self.row_count,
            "node_count": self.node_count,
            "class0_count": self.class0_count,
            "class1_count": self.class1_count,
            "class2_count": self.class2_count,
            "class2_byte_count": self.class2_byte_count,
            "event_stream_row_count": self.event_stream_row_count,
            "expected_advance_only": self.status == P6Ad68TextRouteStatus::ExpectedAdvanceOnlySpace,
            "materializer_failure": self.status.materializer_failure(),
            "fallback_to_default_path": self.status.fallback_to_default_path(),
            "fallback_counted_as_p6_success": false,
            "used_typed_span_proof": self.used_typed_span_proof,
            "used_coverage_row_proof": self.used_coverage_row_proof,
            "used_fixture_payload": self.used_fixture_payload,
            "used_synthetic_payload": self.used_synthetic_payload,
            "producer": "p6_source_owned_ad68_materializer_opt_in_v1"
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum P6Ad68TextRowsOutput {
    Routed {
        report: P6Ad68TextRouteReport,
        event_stream: AreEventStream,
        row_table: Ad68RowTable,
    },
    ExpectedAdvanceOnlySpace {
        report: P6Ad68TextRouteReport,
    },
    Unsupported {
        report: P6Ad68TextRouteReport,
    },
}

pub fn try_rasterize_text_with_p6_ad68(input: &P6Ad68TextPathInput<'_>) -> P6Ad68TextRouteReport {
    match materialize_text_source_path_to_p6_ad68_rows(input) {
        P6Ad68TextRowsOutput::Routed { report, .. }
        | P6Ad68TextRowsOutput::ExpectedAdvanceOnlySpace { report }
        | P6Ad68TextRowsOutput::Unsupported { report } => report,
    }
}

pub fn materialize_text_source_path_to_p6_ad68_rows(
    input: &P6Ad68TextPathInput<'_>,
) -> P6Ad68TextRowsOutput {
    let Some(source_input) = p6_source_path_to_ad68_rows_for_text(
        input.glyph_run_index,
        input.glyph_id,
        input.path_segments,
    ) else {
        if input.expected_advance_only_space {
            let report = P6Ad68TextRouteReport::expected_advance_only_space(
                input.glyph_run_index,
                input.glyph_id,
            );
            return P6Ad68TextRowsOutput::ExpectedAdvanceOnlySpace { report };
        }
        let report = P6Ad68TextRouteReport::unsupported(
            input.glyph_run_index,
            input.glyph_id,
            P6Ad68TextRouteStatus::UnsupportedSourcePathMissing,
            "source_owned_outline_path_segments_unavailable",
        );
        return P6Ad68TextRowsOutput::Unsupported { report };
    };

    let source_provenance = source_input.provenance;
    let materializer_input = match build_materializer_input_from_source(&source_input) {
        Ok(input) => input,
        Err(err) => {
            let report = P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::UnsupportedSourcePathRejected,
                format!("source_path_rejected: {:?}", err),
            );
            return P6Ad68TextRowsOutput::Unsupported { report };
        }
    };
    let materialized = match materialize_95cc_intervals(&materializer_input) {
        Ok(materialized) => materialized,
        Err(err) => {
            let report = P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::MaterializerError,
                format!("materializer_error: {:?}", err),
            );
            return P6Ad68TextRowsOutput::Unsupported { report };
        }
    };
    let event_stream = match source_owned_materializer_to_event_stream(&materialized) {
        Ok(event_stream) => event_stream,
        Err(err) => {
            let report = P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::EventCursorError,
                format!("event_cursor_error: {:?}", err),
            );
            return P6Ad68TextRowsOutput::Unsupported { report };
        }
    };
    let row_table = match emit_ad68_rows(&event_stream) {
        Ok(row_table) => row_table,
        Err(err) => {
            let report = P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::Ad68EmitError,
                format!("ad68_emit_error: {:?}", err),
            );
            return P6Ad68TextRowsOutput::Unsupported { report };
        }
    };

    let report = P6Ad68TextRouteReport::routed(
        input.glyph_run_index,
        input.glyph_id,
        source_provenance,
        &event_stream,
        &row_table,
    );

    P6Ad68TextRowsOutput::Routed {
        report,
        event_stream,
        row_table,
    }
}

fn p6_source_path_to_ad68_rows_for_text(
    glyph_run_index: usize,
    glyph_id: u32,
    path_segments: &[P6Ad68RawPathSegment],
) -> Option<AreBezierSourcePathInput> {
    if path_segments.is_empty() {
        return None;
    }

    let glyph_id = u16::try_from(glyph_id).ok();
    let mut points = Vec::new();
    let mut verbs = Vec::new();
    let mut segments = Vec::new();
    let mut current_contour: Option<usize> = None;
    let mut next_segment_id = 0usize;
    let mut previous_point: Option<ArePathPoint> = None;
    let mut contour_start: Option<ArePathPoint> = None;

    for raw in path_segments {
        if current_contour != Some(raw.contour_index) {
            push_inferred_contour_close(
                &mut points,
                &mut verbs,
                &mut segments,
                &mut next_segment_id,
                current_contour,
                contour_start,
                previous_point,
                glyph_run_index,
                glyph_id,
            );
            let start = match raw.segment {
                P6Ad68PathSegment::Line { start, .. } | P6Ad68PathSegment::Cubic { start, .. } => {
                    p6_are_path_point(start)
                }
            };
            current_contour = Some(raw.contour_index);
            contour_start = Some(start);
            points.push(start);
            verbs.push(ArePathVerb::MoveTo);
            segments.push(AreSourceSegment {
                contour_id: AreSourceContourId(raw.contour_index),
                segment_id: AreSourceSegmentId(next_segment_id),
                source_point_index: points.len() - 1,
                verb: ArePathVerb::MoveTo,
                previous_point: None,
                endpoint: start,
                kind: AreSourceSegmentKind::Move,
                glyph_run_index: Some(glyph_run_index),
                glyph_id,
                is_close_boundary: false,
            });
            next_segment_id += 1;
            previous_point = Some(start);
        }

        let Some(start) = previous_point else {
            continue;
        };
        let (verb, endpoint, kind) = match raw.segment {
            P6Ad68PathSegment::Line { end, .. } => {
                let endpoint = p6_are_path_point(end);
                (ArePathVerb::LineTo, endpoint, AreSourceSegmentKind::Line)
            }
            P6Ad68PathSegment::Cubic {
                control1,
                control2,
                end,
                ..
            } => {
                let control_1 = p6_are_path_point(control1);
                let control_2 = p6_are_path_point(control2);
                let endpoint = p6_are_path_point(end);
                (
                    ArePathVerb::CubicTo,
                    endpoint,
                    AreSourceSegmentKind::Cubic {
                        controls: Some(AreCubicControl {
                            control_1,
                            control_2,
                        }),
                    },
                )
            }
        };
        points.push(endpoint);
        verbs.push(verb);
        segments.push(AreSourceSegment {
            contour_id: AreSourceContourId(raw.contour_index),
            segment_id: AreSourceSegmentId(next_segment_id),
            source_point_index: points.len() - 1,
            verb,
            previous_point: Some(start),
            endpoint,
            kind,
            glyph_run_index: Some(glyph_run_index),
            glyph_id,
            is_close_boundary: false,
        });
        next_segment_id += 1;
        previous_point = Some(endpoint);
    }

    push_inferred_contour_close(
        &mut points,
        &mut verbs,
        &mut segments,
        &mut next_segment_id,
        current_contour,
        contour_start,
        previous_point,
        glyph_run_index,
        glyph_id,
    );

    if points.is_empty() || verbs.is_empty() {
        return None;
    }

    Some(
        AreBezierSourcePathInput::source_owned_glyph_path(
            points,
            verbs,
            ArePathTransform::identity(),
        )
        .with_segments(segments),
    )
}

fn push_inferred_contour_close(
    points: &mut Vec<ArePathPoint>,
    verbs: &mut Vec<ArePathVerb>,
    segments: &mut Vec<AreSourceSegment>,
    next_segment_id: &mut usize,
    contour_index: Option<usize>,
    contour_start: Option<ArePathPoint>,
    previous_point: Option<ArePathPoint>,
    glyph_run_index: usize,
    glyph_id: Option<u16>,
) {
    let (Some(contour_index), Some(contour_start), Some(close_from)) =
        (contour_index, contour_start, previous_point)
    else {
        return;
    };
    if p6_points_close(close_from, contour_start) {
        return;
    }

    points.push(contour_start);
    verbs.push(ArePathVerb::Close);
    segments.push(AreSourceSegment {
        contour_id: AreSourceContourId(contour_index),
        segment_id: AreSourceSegmentId(*next_segment_id),
        source_point_index: points.len() - 1,
        verb: ArePathVerb::Close,
        previous_point: Some(close_from),
        endpoint: contour_start,
        kind: AreSourceSegmentKind::Close(AreContourClose {
            from: close_from,
            to: contour_start,
        }),
        glyph_run_index: Some(glyph_run_index),
        glyph_id,
        is_close_boundary: true,
    });
    *next_segment_id += 1;
}

fn p6_points_close(left: ArePathPoint, right: ArePathPoint) -> bool {
    (left.x - right.x).abs() <= 0.001 && (left.y - right.y).abs() <= 0.001
}

fn p6_are_path_point(point: P6Ad68PathPoint) -> ArePathPoint {
    ArePathPoint::new(point.x, point.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_unchanged_without_p6_env() {
        let flag = p6_ad68_opt_in_flag_state_from_value(None);
        assert_eq!(flag, P6Ad68OptInFlagState::DisabledDefault);
        assert!(!flag.enabled());

        let disabled = P6Ad68TextRouteReport::disabled(0, 42);
        assert_eq!(disabled.status, P6Ad68TextRouteStatus::Disabled);
        assert_eq!(disabled.reason.as_deref(), Some("p6_ad68_opt_in_disabled"));
    }

    #[test]
    fn p6_ad68_opt_in_flag_disabled_by_default() {
        assert_eq!(
            p6_ad68_opt_in_flag_state_from_value(None),
            P6Ad68OptInFlagState::DisabledDefault
        );
        assert!(!p6_ad68_opt_in_flag_state_from_value(None).enabled());
    }

    #[test]
    fn p6_ad68_opt_in_flag_enabled_routes_to_p6_branch() {
        assert_eq!(
            p6_ad68_opt_in_flag_state_from_value(Some("1".to_string())),
            P6Ad68OptInFlagState::Enabled
        );

        let report = try_rasterize_text_with_p6_ad68(&P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 42,
            path_segments: &p6_rectangle_path_segments(),
            expected_advance_only_space: false,
        });

        assert_eq!(report.status, P6Ad68TextRouteStatus::Routed);
        assert_eq!(
            report.source_provenance,
            Some(AreSourcePathProvenance::SourceOwnedGlyphPath)
        );
        assert!(report.row_count > 0);
        assert!(report.node_count > 0);
    }

    #[test]
    fn p6_ad68_opt_in_reports_unsupported_when_source_path_missing() {
        let report = try_rasterize_text_with_p6_ad68(&P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 42,
            path_segments: &[],
            expected_advance_only_space: false,
        });

        assert_eq!(
            report.status,
            P6Ad68TextRouteStatus::UnsupportedSourcePathMissing
        );
        assert_eq!(
            report.reason.as_deref(),
            Some("source_owned_outline_path_segments_unavailable")
        );
        assert_eq!(report.node_count, 0);
    }

    #[test]
    fn p6_ad68_opt_in_reports_expected_advance_only_space() {
        let report = try_rasterize_text_with_p6_ad68(&P6Ad68TextPathInput {
            glyph_run_index: 5,
            glyph_id: 3,
            path_segments: &[],
            expected_advance_only_space: true,
        });

        assert_eq!(
            report.status,
            P6Ad68TextRouteStatus::ExpectedAdvanceOnlySpace
        );
        assert_eq!(
            report.reason.as_deref(),
            Some("advance_only_space_has_no_source_outline")
        );
        assert_eq!(report.node_count, 0);
        assert_eq!(report.class2_count, 0);
        assert!(!report.status.materializer_failure());
        assert!(!report.status.fallback_to_default_path());
    }

    #[test]
    fn p6_ad68_opt_in_does_not_use_fixture_payload() {
        let report = routed_p6_rectangle_report();
        assert_eq!(report.status, P6Ad68TextRouteStatus::Routed);
        assert!(!report.used_fixture_payload);
    }

    #[test]
    fn p6_ad68_opt_in_does_not_use_synthetic_payload() {
        let report = routed_p6_rectangle_report();
        assert_eq!(report.status, P6Ad68TextRouteStatus::Routed);
        assert!(!report.used_synthetic_payload);
    }

    #[test]
    fn p6_ad68_opt_in_does_not_use_typed_span_as_proof() {
        let report = routed_p6_rectangle_report();
        assert_eq!(report.status, P6Ad68TextRouteStatus::Routed);
        assert!(!report.used_typed_span_proof);
        assert_eq!(
            report.source_provenance,
            Some(AreSourcePathProvenance::SourceOwnedGlyphPath)
        );
    }

    #[test]
    fn p6_ad68_opt_in_does_not_use_coverage_row_as_proof() {
        let report = routed_p6_rectangle_report();
        assert_eq!(report.status, P6Ad68TextRouteStatus::Routed);
        assert!(!report.used_coverage_row_proof);
    }

    #[test]
    fn p6_ad68_opt_in_does_not_change_production_default() {
        assert_eq!(
            p6_ad68_opt_in_flag_state_from_value(None),
            P6Ad68OptInFlagState::DisabledDefault
        );
        assert!(!p6_ad68_opt_in_enabled_for_value(None));
        assert!(!p6_ad68_opt_in_enabled_for_value(Some(
            "not-a-real-flag".to_string()
        )));
    }

    #[test]
    fn p6_ad68_row_byte_debug_disabled_by_default() {
        assert_eq!(p6_ad68_debug_limit_from_value(None), None);
        assert_eq!(p6_ad68_debug_limit_from_value(Some("0".to_string())), None);
        assert_eq!(
            p6_ad68_debug_limit_from_value(Some("1".to_string())),
            Some(256)
        );
        assert_eq!(
            p6_ad68_debug_limit_from_value(Some("3".to_string())),
            Some(3)
        );
    }

    #[test]
    fn p6_ad68_opt_in_materializes_owned_event_stream_and_row_table() {
        let output = materialize_text_source_path_to_p6_ad68_rows(&P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 42,
            path_segments: &p6_rectangle_path_segments(),
            expected_advance_only_space: false,
        });

        match output {
            P6Ad68TextRowsOutput::Routed {
                report,
                event_stream,
                row_table,
            } => {
                assert_eq!(report.status, P6Ad68TextRouteStatus::Routed);
                assert_eq!(report.row_count, row_table.rows.len());
                assert_eq!(report.event_stream_row_count, event_stream.rows.len());
                assert_eq!(
                    report.node_count,
                    row_table.rows.iter().map(Vec::len).sum::<usize>()
                );
                assert!(report.row_count > 0);
                assert!(report.node_count > 0);
                assert!(!report.used_typed_span_proof);
                assert!(!report.used_coverage_row_proof);
                assert!(!report.used_fixture_payload);
                assert!(!report.used_synthetic_payload);
            }
            other => panic!("expected routed rows output, got {:?}", other),
        }
    }

    #[test]
    fn p6_ad68_row_byte_debug_payload_reports_class2_bytes() {
        let output = materialize_text_source_path_to_p6_ad68_rows(&P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 42,
            path_segments: &p6_rectangle_path_segments(),
            expected_advance_only_space: false,
        });

        let P6Ad68TextRowsOutput::Routed { row_table, .. } = output else {
            panic!("expected routed rows output");
        };
        let payload = p6_ad68_row_byte_debug_payload(&row_table, 2);

        assert_eq!(
            payload.get("schema").and_then(Value::as_str),
            Some("ae-native-renderer.p6-ad68-row-byte-debug.v1")
        );
        assert!(payload
            .get("node_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0));
        assert!(payload
            .get("sampled_node_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count <= 2));
        assert_eq!(
            payload.get("env_var").and_then(Value::as_str),
            Some(P6_AD68_TEXT_ROW_BYTE_DEBUG_ENV_VAR)
        );
    }

    #[test]
    fn p6_ad68_frontier_debug_payload_reports_source_and_interval_frontier() {
        let payload = p6_ad68_frontier_debug_payload(
            &P6Ad68TextPathInput {
                glyph_run_index: 0,
                glyph_id: 42,
                path_segments: &p6_rectangle_path_segments(),
                expected_advance_only_space: false,
            },
            4,
        );

        assert_eq!(
            payload.get("schema").and_then(Value::as_str),
            Some("ae-native-renderer.p6-ad68-frontier-debug.v1")
        );
        assert_eq!(payload.get("status").and_then(Value::as_str), Some("ok"));
        assert!(payload
            .get("source_record_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0));
        assert!(payload
            .get("interval_row_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0));
        assert!(payload
            .get("materialized_interval_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0));
    }

    #[test]
    fn p6_ad68_frontier_debug_payload_reports_row_event_projection_trace() {
        let payload = p6_ad68_frontier_debug_payload(
            &P6Ad68TextPathInput {
                glyph_run_index: 0,
                glyph_id: 42,
                path_segments: &p6_rectangle_path_segments(),
                expected_advance_only_space: false,
            },
            4,
        );
        let row_event_projection = payload
            .get("row_event_projection")
            .expect("frontier payload includes row event projection debug");

        assert_eq!(
            row_event_projection.get("status").and_then(Value::as_str),
            Some("ok")
        );
        assert!(row_event_projection
            .get("sampled_run_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0));

        let runs = row_event_projection
            .get("runs")
            .and_then(Value::as_array)
            .expect("row event projection runs are serialized");
        assert!(runs
            .iter()
            .any(|run| run.get("status").and_then(Value::as_str) == Some("projected")));
        assert!(runs.iter().any(|run| run
            .get("subrow_traces")
            .and_then(Value::as_array)
            .is_some_and(|traces| traces.iter().any(|trace| trace
                .get("projected_edge_events")
                .and_then(Value::as_array)
                .is_some_and(|events| !events.is_empty())))));
    }

    #[test]
    fn p6_ad68_opt_in_materializer_space_is_non_drawing_output() {
        let output = materialize_text_source_path_to_p6_ad68_rows(&P6Ad68TextPathInput {
            glyph_run_index: 5,
            glyph_id: 3,
            path_segments: &[],
            expected_advance_only_space: true,
        });

        match output {
            P6Ad68TextRowsOutput::ExpectedAdvanceOnlySpace { report } => {
                assert_eq!(
                    report.status,
                    P6Ad68TextRouteStatus::ExpectedAdvanceOnlySpace
                );
                assert_eq!(report.row_count, 0);
                assert_eq!(report.node_count, 0);
                assert!(!report.status.materializer_failure());
                assert!(!report.status.fallback_to_default_path());
            }
            other => panic!("expected advance-only-space output, got {:?}", other),
        }
    }

    #[test]
    fn p6_ad68_opt_in_materializer_reports_unsupported_output() {
        let output = materialize_text_source_path_to_p6_ad68_rows(&P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 42,
            path_segments: &[],
            expected_advance_only_space: false,
        });

        match output {
            P6Ad68TextRowsOutput::Unsupported { report } => {
                assert_eq!(
                    report.status,
                    P6Ad68TextRouteStatus::UnsupportedSourcePathMissing
                );
                assert_eq!(
                    report.reason.as_deref(),
                    Some("source_owned_outline_path_segments_unavailable")
                );
                assert!(report.status.fallback_to_default_path());
            }
            other => panic!("expected unsupported output, got {:?}", other),
        }
    }

    #[test]
    fn p6_ad68_opt_in_rasterize_wrapper_remains_report_only() {
        let path_segments = p6_rectangle_path_segments();
        let input = P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 42,
            path_segments: &path_segments,
            expected_advance_only_space: false,
        };
        let wrapper_report = try_rasterize_text_with_p6_ad68(&input);

        match materialize_text_source_path_to_p6_ad68_rows(&input) {
            P6Ad68TextRowsOutput::Routed { report, .. } => {
                assert_eq!(wrapper_report, report);
            }
            other => panic!("expected routed rows output, got {:?}", other),
        }
    }

    #[test]
    fn p6_source_path_conversion_infers_missing_contour_close() {
        let source_input =
            p6_source_path_to_ad68_rows_for_text(7, 42, &p6_open_rectangle_path_segments())
                .expect("open raw contour should still build a source-owned glyph path");

        assert!(source_input
            .segments
            .iter()
            .any(|segment| segment.verb == ArePathVerb::Close
                && segment.is_close_boundary
                && segment.glyph_run_index == Some(7)
                && segment.glyph_id == Some(42)));
    }

    #[test]
    fn p6_ad68_opt_in_routes_open_raw_contour_with_inferred_close() {
        let output = materialize_text_source_path_to_p6_ad68_rows(&P6Ad68TextPathInput {
            glyph_run_index: 7,
            glyph_id: 42,
            path_segments: &p6_open_rectangle_path_segments(),
            expected_advance_only_space: false,
        });

        match output {
            P6Ad68TextRowsOutput::Routed { report, .. } => {
                assert_eq!(report.status, P6Ad68TextRouteStatus::Routed);
                assert!(!report.used_fixture_payload);
                assert!(!report.used_synthetic_payload);
            }
            other => panic!("expected routed output from inferred-close contour, got {other:?}"),
        }
    }

    fn p6_ad68_opt_in_enabled_for_value(value: Option<String>) -> bool {
        p6_ad68_opt_in_flag_state_from_value(value).enabled()
    }

    fn routed_p6_rectangle_report() -> P6Ad68TextRouteReport {
        try_rasterize_text_with_p6_ad68(&P6Ad68TextPathInput {
            glyph_run_index: 0,
            glyph_id: 42,
            path_segments: &p6_rectangle_path_segments(),
            expected_advance_only_space: false,
        })
    }

    fn p6_rectangle_path_segments() -> Vec<P6Ad68RawPathSegment> {
        let top_left = P6Ad68PathPoint::new(0.0, 0.0);
        let top_right = P6Ad68PathPoint::new(10.0, 0.0);
        let bottom_right = P6Ad68PathPoint::new(10.0, 10.0);
        let bottom_left = P6Ad68PathPoint::new(0.0, 10.0);
        vec![
            P6Ad68RawPathSegment {
                contour_index: 0,
                segment_index: 0,
                segment: P6Ad68PathSegment::Line {
                    start: top_left,
                    end: top_right,
                },
            },
            P6Ad68RawPathSegment {
                contour_index: 0,
                segment_index: 1,
                segment: P6Ad68PathSegment::Line {
                    start: top_right,
                    end: bottom_right,
                },
            },
            P6Ad68RawPathSegment {
                contour_index: 0,
                segment_index: 2,
                segment: P6Ad68PathSegment::Line {
                    start: bottom_right,
                    end: bottom_left,
                },
            },
            P6Ad68RawPathSegment {
                contour_index: 0,
                segment_index: 3,
                segment: P6Ad68PathSegment::Line {
                    start: bottom_left,
                    end: top_left,
                },
            },
        ]
    }

    fn p6_open_rectangle_path_segments() -> Vec<P6Ad68RawPathSegment> {
        let top_left = P6Ad68PathPoint::new(0.0, 0.0);
        let top_right = P6Ad68PathPoint::new(10.0, 0.0);
        let bottom_right = P6Ad68PathPoint::new(10.0, 10.0);
        let bottom_left = P6Ad68PathPoint::new(0.0, 10.0);
        vec![
            P6Ad68RawPathSegment {
                contour_index: 0,
                segment_index: 0,
                segment: P6Ad68PathSegment::Line {
                    start: top_left,
                    end: top_right,
                },
            },
            P6Ad68RawPathSegment {
                contour_index: 0,
                segment_index: 1,
                segment: P6Ad68PathSegment::Line {
                    start: top_right,
                    end: bottom_right,
                },
            },
            P6Ad68RawPathSegment {
                contour_index: 0,
                segment_index: 2,
                segment: P6Ad68PathSegment::Line {
                    start: bottom_right,
                    end: bottom_left,
                },
            },
        ]
    }
}
