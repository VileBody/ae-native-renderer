use serde_json::{json, Value};

use crate::{
    build_materializer_input_from_source, emit_ad68_rows, materialize_95cc_intervals,
    source_owned_materializer_to_event_stream, AreBezierSourcePathInput, AreCubicControl,
    AreEventStream, ArePathPoint, ArePathTransform, ArePathVerb, AreSourceContourId,
    AreSourcePathProvenance, AreSourceSegment, AreSourceSegmentId, AreSourceSegmentKind,
};

pub const P6_AD68_TEXT_OPT_IN_ENV_VAR: &str = "AE_NATIVE_RENDERER_P6_AD68_TEXT_OPT_IN";

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
        row_table: &crate::Ad68RowTable,
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

pub fn try_rasterize_text_with_p6_ad68(input: &P6Ad68TextPathInput<'_>) -> P6Ad68TextRouteReport {
    let Some(source_input) = p6_source_path_to_ad68_rows_for_text(
        input.glyph_run_index,
        input.glyph_id,
        input.path_segments,
    ) else {
        if input.expected_advance_only_space {
            return P6Ad68TextRouteReport::expected_advance_only_space(
                input.glyph_run_index,
                input.glyph_id,
            );
        }
        return P6Ad68TextRouteReport::unsupported(
            input.glyph_run_index,
            input.glyph_id,
            P6Ad68TextRouteStatus::UnsupportedSourcePathMissing,
            "source_owned_outline_path_segments_unavailable",
        );
    };

    let source_provenance = source_input.provenance;
    let materializer_input = match build_materializer_input_from_source(&source_input) {
        Ok(input) => input,
        Err(err) => {
            return P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::UnsupportedSourcePathRejected,
                format!("source_path_rejected: {:?}", err),
            );
        }
    };
    let materialized = match materialize_95cc_intervals(&materializer_input) {
        Ok(materialized) => materialized,
        Err(err) => {
            return P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::MaterializerError,
                format!("materializer_error: {:?}", err),
            );
        }
    };
    let event_stream = match source_owned_materializer_to_event_stream(&materialized) {
        Ok(event_stream) => event_stream,
        Err(err) => {
            return P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::EventCursorError,
                format!("event_cursor_error: {:?}", err),
            );
        }
    };
    let row_table = match emit_ad68_rows(&event_stream) {
        Ok(row_table) => row_table,
        Err(err) => {
            return P6Ad68TextRouteReport::unsupported(
                input.glyph_run_index,
                input.glyph_id,
                P6Ad68TextRouteStatus::Ad68EmitError,
                format!("ad68_emit_error: {:?}", err),
            );
        }
    };

    P6Ad68TextRouteReport::routed(
        input.glyph_run_index,
        input.glyph_id,
        source_provenance,
        &event_stream,
        &row_table,
    )
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

    for raw in path_segments {
        if current_contour != Some(raw.contour_index) {
            let start = match raw.segment {
                P6Ad68PathSegment::Line { start, .. } | P6Ad68PathSegment::Cubic { start, .. } => {
                    p6_are_path_point(start)
                }
            };
            current_contour = Some(raw.contour_index);
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
}
