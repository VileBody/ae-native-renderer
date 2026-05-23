use crate::{
    accumulate_75d0_for_sample_x, build_95cc_interval_list, build_crossing_lists_for_row,
    build_e854_working_set, classify_edge_pair_candidate, Are95ccStateClass, AreActiveEdge,
    AreBezierSourcePathInput, AreContourClose, AreCrossingListError, AreDescriptorCursorPolicy,
    AreDescriptorTriplet, AreFillRule, AreIntervalDescriptorPolicy, AreIntervalPayloadPolicy,
    ArePathPoint, ArePathVerb, AreSamplerIntervalList, AreSamplerIntervalRun,
    AreSamplerIntervalTag, AreSourcePathProvenance, AreSourceRecord32, AreSourceSamplerError,
    AreSourceSamplerWorkingSet, AreSourceSegment, AreSourceSegmentId, AreSourceSegmentKind,
    AreStatePolicy, ARE_FULL_COVERAGE_0X264,
};
use serde::{Deserialize, Serialize};

const ARE_DB98_CURVE_MAX_DEPTH: u8 = 15;
const ARE_DB98_FLATNESS: f32 = 1.0;
const ARE_DB98_SHORT_CHORD_RATIO: f32 = 0.25;
const ARE_DB98_TOLERANCE_SCALE: f32 = 1.0;
const QUADRATIC_TO_CUBIC_CONTROL_SCALE: f32 = 2.0 / 3.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreSamplerMaterializerInput {
    pub provenance: AreSamplerMaterializationProvenance,
    pub fill_rule: AreFillRule,
    pub source_input: AreBezierSourcePathInput,
    pub working_set: AreSourceSamplerWorkingSet,
    pub interval_list: AreSamplerIntervalList,
}

impl AreSamplerMaterializerInput {
    pub fn source_owned(
        source_input: AreBezierSourcePathInput,
        working_set: AreSourceSamplerWorkingSet,
        interval_list: AreSamplerIntervalList,
    ) -> Self {
        Self {
            provenance: AreSamplerMaterializationProvenance::SourceOwnedSamplerMaterialized,
            fill_rule: AreFillRule::NonZeroWinding,
            source_input,
            working_set,
            interval_list,
        }
    }

    pub fn typed_span_fallback(
        source_input: AreBezierSourcePathInput,
        working_set: AreSourceSamplerWorkingSet,
        interval_list: AreSamplerIntervalList,
    ) -> Self {
        Self {
            provenance: AreSamplerMaterializationProvenance::TypedSpanFallback,
            fill_rule: AreFillRule::NonZeroWinding,
            source_input,
            working_set,
            interval_list,
        }
    }

    pub fn coverage_row_fallback(
        source_input: AreBezierSourcePathInput,
        working_set: AreSourceSamplerWorkingSet,
        interval_list: AreSamplerIntervalList,
    ) -> Self {
        Self {
            provenance: AreSamplerMaterializationProvenance::CoverageRowFallback,
            fill_rule: AreFillRule::NonZeroWinding,
            source_input,
            working_set,
            interval_list,
        }
    }

    pub fn with_fill_rule(mut self, fill_rule: AreFillRule) -> Self {
        self.fill_rule = fill_rule;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreSamplerMaterializationProvenance {
    SourceOwnedSamplerMaterialized,
    TypedSpanFallback,
    CoverageRowFallback,
    FixtureBacked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSamplerMaterialization {
    pub provenance: AreSamplerMaterializationProvenance,
    pub y_min: i32,
    pub y_max: i32,
    pub intervals: Vec<AreMaterializedInterval>,
}

impl AreSamplerMaterialization {
    pub fn interval(&self, row_y: i32, run_index: usize) -> Option<&AreMaterializedInterval> {
        self.intervals
            .iter()
            .find(|interval| interval.row_y == row_y && interval.run_index == run_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreMaterializedInterval {
    pub row_y: i32,
    pub run_index: usize,
    pub current_x: i32,
    pub next_x: i32,
    pub source_record_indices: Vec<usize>,
    pub payload: Option<AreMaterializedPayload>,
    pub state_hint: Option<AreMaterializedStateHint>,
}

impl AreMaterializedInterval {
    pub fn width(&self) -> i32 {
        self.next_x - self.current_x
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreMaterializedPayload {
    pub bytes: Vec<u8>,
    pub payload_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreMaterializedStateHint {
    pub state_class: Are95ccStateClass,
    pub state: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreSamplerMaterializationError {
    Source(AreSourceSamplerError),
    Interval(crate::AreIntervalBuildError),
    TypedSpanFallbackRejected,
    CoverageRowFallbackRejected,
    FixtureBackedPolicyRejected,
    RequiresSourceOwnedProvenance {
        provenance: AreSourcePathProvenance,
    },
    IntervalListProvenanceMismatch {
        provenance: AreSourcePathProvenance,
    },
    WorkingSetRecordCountMismatch {
        working_set_records: usize,
        interval_source_records: usize,
    },
    InvalidIntervalWidth {
        row_y: i32,
        run_index: usize,
        current_x: i32,
        next_x: i32,
    },
    MissingSourceRecord {
        row_y: i32,
        run_index: usize,
        record_index: usize,
    },
    UnsupportedMultiRecordRun {
        row_y: i32,
        run_index: usize,
        source_record_indices: Vec<usize>,
    },
    UnsupportedNonHorizontalRecord {
        row_y: i32,
        run_index: usize,
        record_index: usize,
        min_y: i32,
        max_y: i32,
    },
    UnsupportedCurveRequiresControlPoints {
        row_y: i32,
        run_index: usize,
        record_index: usize,
        verb: ArePathVerb,
    },
    UnsupportedAmbiguousLineGeometry {
        row_y: i32,
        run_index: usize,
        source_record_indices: Vec<usize>,
    },
    CrossingList(AreCrossingListError),
    SourceRecordDoesNotCoverRun {
        row_y: i32,
        run_index: usize,
        record_index: usize,
    },
    SourceRecordsDoNotCoverPixel {
        row_y: i32,
        run_index: usize,
        x: i32,
        source_record_indices: Vec<usize>,
    },
    MissingSourceSegment {
        record_index: usize,
        source_point_index: usize,
    },
}

pub fn build_materializer_input_from_source(
    source_input: &AreBezierSourcePathInput,
) -> Result<AreSamplerMaterializerInput, AreSamplerMaterializationError> {
    let working_set =
        build_e854_working_set(source_input).map_err(AreSamplerMaterializationError::Source)?;
    let interval_list =
        build_95cc_interval_list(&working_set).map_err(AreSamplerMaterializationError::Interval)?;
    Ok(AreSamplerMaterializerInput::source_owned(
        source_input.clone(),
        working_set,
        interval_list,
    ))
}

pub fn materialize_95cc_intervals(
    input: &AreSamplerMaterializerInput,
) -> Result<AreSamplerMaterialization, AreSamplerMaterializationError> {
    validate_materializer_input(input)?;

    let mut intervals = Vec::new();
    for row in &input.interval_list.rows {
        for (run_index, run) in row.runs.iter().enumerate() {
            intervals.push(materialize_run(input, row.row_y, run_index, run)?);
        }
    }

    Ok(AreSamplerMaterialization {
        provenance: input.provenance,
        y_min: input.interval_list.y_min,
        y_max: input.interval_list.y_max,
        intervals,
    })
}

pub fn descriptor_policy_from_materialized_intervals(
    materialization: &AreSamplerMaterialization,
) -> AreDescriptorCursorPolicy {
    let policies = materialization
        .intervals
        .iter()
        .map(|interval| {
            let triplet = descriptor_triplet_for_materialized_interval(interval);
            let source_probe = source_probe_for_materialized_interval(interval);
            if let Some(payload) = &interval.payload {
                AreIntervalDescriptorPolicy {
                    row_y: interval.row_y,
                    run_index: interval.run_index,
                    descriptor_triplet: Some(triplet),
                    source_probe: Some(source_probe),
                    payload: Some(AreIntervalPayloadPolicy::bytes(payload.bytes.clone())),
                    state: None,
                }
            } else {
                let state_hint = interval.state_hint.unwrap_or(AreMaterializedStateHint {
                    state_class: Are95ccStateClass::Class0,
                    state: 0,
                });
                AreIntervalDescriptorPolicy {
                    row_y: interval.row_y,
                    run_index: interval.run_index,
                    descriptor_triplet: Some(triplet),
                    source_probe: Some(source_probe),
                    payload: None,
                    state: Some(AreStatePolicy::new(
                        state_hint.state_class,
                        state_hint.state,
                    )),
                }
            }
        })
        .collect();

    AreDescriptorCursorPolicy::source_owned_materialized(policies)
}

fn validate_materializer_input(
    input: &AreSamplerMaterializerInput,
) -> Result<(), AreSamplerMaterializationError> {
    match input.provenance {
        AreSamplerMaterializationProvenance::SourceOwnedSamplerMaterialized => {}
        AreSamplerMaterializationProvenance::TypedSpanFallback => {
            return Err(AreSamplerMaterializationError::TypedSpanFallbackRejected);
        }
        AreSamplerMaterializationProvenance::CoverageRowFallback => {
            return Err(AreSamplerMaterializationError::CoverageRowFallbackRejected);
        }
        AreSamplerMaterializationProvenance::FixtureBacked => {
            return Err(AreSamplerMaterializationError::FixtureBackedPolicyRejected);
        }
    }

    match input.source_input.provenance {
        AreSourcePathProvenance::SourceOwned | AreSourcePathProvenance::SourceOwnedGlyphPath => {}
        AreSourcePathProvenance::SourceOwnedGlyphRun => {}
        provenance => {
            return Err(
                AreSamplerMaterializationError::RequiresSourceOwnedProvenance { provenance },
            );
        }
    }
    if input.interval_list.provenance != AreSourcePathProvenance::SourceOwned {
        return Err(
            AreSamplerMaterializationError::IntervalListProvenanceMismatch {
                provenance: input.interval_list.provenance,
            },
        );
    }
    if input.interval_list.source_record_count != input.working_set.records.len() {
        return Err(
            AreSamplerMaterializationError::WorkingSetRecordCountMismatch {
                working_set_records: input.working_set.records.len(),
                interval_source_records: input.interval_list.source_record_count,
            },
        );
    }

    Ok(())
}

fn materialize_run(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> Result<AreMaterializedInterval, AreSamplerMaterializationError> {
    if run.width() <= 0 {
        return Err(AreSamplerMaterializationError::InvalidIntervalWidth {
            row_y,
            run_index,
            current_x: run.current_x,
            next_x: run.next_x,
        });
    }

    let mut interval = AreMaterializedInterval {
        row_y,
        run_index,
        current_x: run.current_x,
        next_x: run.next_x,
        source_record_indices: run.source_record_indices.clone(),
        payload: None,
        state_hint: None,
    };

    if run.materialize_candidate && run.tag == AreSamplerIntervalTag::SourceSpan {
        match materialize_source_span(input, row_y, run_index, run)? {
            AreMaterializedSpan::Payload(bytes) => {
                interval.payload = Some(AreMaterializedPayload {
                    payload_len: bytes.len(),
                    bytes,
                });
            }
            AreMaterializedSpan::State(state_hint) => {
                interval.state_hint = Some(state_hint);
            }
        }
    } else {
        interval.state_hint = Some(materialize_state_hint(input, row_y, run_index, run)?);
    }

    Ok(interval)
}

enum AreMaterializedSpan {
    Payload(Vec<u8>),
    State(AreMaterializedStateHint),
}

fn materialize_source_span(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> Result<AreMaterializedSpan, AreSamplerMaterializationError> {
    let records = materializable_source_records(input, row_y, run_index, run)?;
    if records
        .iter()
        .all(|(record_index, record)| is_horizontal_unit_record(input, *record_index, record))
    {
        return legacy_materialize_horizontal_unit_payload(input, row_y, run_index, run, &records)
            .map(AreMaterializedSpan::Payload);
    }

    materialize_source_span_with_crossing_lists(input, row_y, run_index, run, &records)
}

fn legacy_materialize_horizontal_unit_payload(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    records: &[(usize, &AreSourceRecord32)],
) -> Result<Vec<u8>, AreSamplerMaterializationError> {
    let mut bytes = Vec::with_capacity(run.width() as usize);
    for x in run.current_x..run.next_x {
        let Some((_record_index, record)) = records
            .iter()
            .find(|(_record_index, record)| record_covers_x(row_y, record, x))
        else {
            return Err(
                AreSamplerMaterializationError::SourceRecordsDoNotCoverPixel {
                    row_y,
                    run_index,
                    x,
                    source_record_indices: run.source_record_indices.clone(),
                },
            );
        };
        bytes.push(materialized_coverage_byte(input, record, x));
    }

    Ok(bytes)
}

fn materialize_source_span_with_crossing_lists(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    records: &[(usize, &AreSourceRecord32)],
) -> Result<AreMaterializedSpan, AreSamplerMaterializationError> {
    let edges = active_edges_for_run(input, row_y, run_index, run, records)?;
    let crossing_lists = build_crossing_lists_for_row(row_y, &edges, input.fill_rule)
        .map_err(AreSamplerMaterializationError::CrossingList)?;
    let mut coverages = Vec::with_capacity(run.width() as usize);
    for sample_x in run.current_x..run.next_x {
        let coverage = accumulate_75d0_for_sample_x(&crossing_lists, sample_x)
            .map_err(AreSamplerMaterializationError::CrossingList)?
            .coverage
            .value_0x264;
        coverages.push(coverage);
    }
    if coverages.iter().all(|coverage| *coverage == 0) {
        return Ok(AreMaterializedSpan::State(AreMaterializedStateHint {
            state_class: Are95ccStateClass::Class0,
            state: 0,
        }));
    }
    if coverages
        .iter()
        .all(|coverage| *coverage == ARE_FULL_COVERAGE_0X264)
    {
        return Ok(AreMaterializedSpan::State(AreMaterializedStateHint {
            state_class: Are95ccStateClass::Class1,
            state: 1,
        }));
    }
    Ok(AreMaterializedSpan::Payload(
        coverages
            .into_iter()
            .map(coverage_to_payload_byte)
            .collect(),
    ))
}

fn materialize_state_hint(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> Result<AreMaterializedStateHint, AreSamplerMaterializationError> {
    let record_index = single_record_index(row_y, run_index, run)?;
    let record = source_record(input, row_y, run_index, record_index)?;
    Ok(AreMaterializedStateHint {
        state_class: Are95ccStateClass::Class0,
        state: record.merge_state_0x19,
    })
}

fn single_record_index(
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> Result<usize, AreSamplerMaterializationError> {
    match run.source_record_indices.as_slice() {
        [record_index] => Ok(*record_index),
        _ => Err(AreSamplerMaterializationError::UnsupportedMultiRecordRun {
            row_y,
            run_index,
            source_record_indices: run.source_record_indices.clone(),
        }),
    }
}

fn materializable_source_records<'a>(
    input: &'a AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> Result<Vec<(usize, &'a AreSourceRecord32)>, AreSamplerMaterializationError> {
    let mut records = Vec::with_capacity(run.source_record_indices.len());
    for record_index in &run.source_record_indices {
        let record = source_record(input, row_y, run_index, *record_index)?;
        validate_source_segment(input, *record_index, record)?;
        records.push((*record_index, record));
    }
    Ok(records)
}

fn source_record<'a>(
    input: &'a AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    record_index: usize,
) -> Result<&'a AreSourceRecord32, AreSamplerMaterializationError> {
    input.working_set.records.get(record_index).ok_or(
        AreSamplerMaterializationError::MissingSourceRecord {
            row_y,
            run_index,
            record_index,
        },
    )
}

fn is_horizontal_unit_record(
    input: &AreSamplerMaterializerInput,
    record_index: usize,
    record: &AreSourceRecord32,
) -> bool {
    if record.min_y_0x0c != record.max_y_0x14 && record.max_y_0x14 != record.min_y_0x0c + 1 {
        return false;
    }
    let Ok((start, end, verb)) = source_segment(input, record_index, record) else {
        return false;
    };
    verb == ArePathVerb::LineTo && (end.y - start.y).abs() <= 0.001
}

fn record_covers_x(row_y: i32, record: &AreSourceRecord32, x: i32) -> bool {
    record.min_x_0x08 <= x
        && record.max_x_0x10 > x
        && row_y >= record.min_y_0x0c
        && row_y <= record.max_y_0x14
}

fn validate_source_segment(
    input: &AreSamplerMaterializerInput,
    record_index: usize,
    record: &AreSourceRecord32,
) -> Result<(), AreSamplerMaterializationError> {
    let (_, _, verb) = source_segment(input, record_index, record)?;
    if matches!(verb, ArePathVerb::MoveTo | ArePathVerb::Unknown(_)) {
        return Err(AreSamplerMaterializationError::MissingSourceSegment {
            record_index,
            source_point_index: record.source_point_index_0x00,
        });
    }
    Ok(())
}

fn source_segment(
    input: &AreSamplerMaterializerInput,
    record_index: usize,
    record: &AreSourceRecord32,
) -> Result<(ArePathPoint, ArePathPoint, ArePathVerb), AreSamplerMaterializationError> {
    let point_index = record.source_point_index_0x00;
    if let Some(segment) = input.working_set.source_segment_for_record(record) {
        let transformed = segment.transformed(input.source_input.transform);
        let Some(start) = transformed.previous_point else {
            return Err(AreSamplerMaterializationError::MissingSourceSegment {
                record_index,
                source_point_index: point_index,
            });
        };
        return Ok((start, transformed.endpoint, transformed.verb));
    }
    if point_index == 0 || point_index >= input.source_input.points.len() {
        return Err(AreSamplerMaterializationError::MissingSourceSegment {
            record_index,
            source_point_index: point_index,
        });
    }
    let start = input
        .source_input
        .transform
        .apply(input.source_input.points[point_index - 1]);
    let end = input
        .source_input
        .transform
        .apply(input.source_input.points[point_index]);
    Ok((start, end, input.source_input.verbs[point_index]))
}

fn source_segment_detail(
    input: &AreSamplerMaterializerInput,
    record_index: usize,
    record: &AreSourceRecord32,
) -> Result<AreSourceSegment, AreSamplerMaterializationError> {
    let point_index = record.source_point_index_0x00;
    if let Some(segment) = input.working_set.source_segment_for_record(record) {
        let transformed = segment.transformed(input.source_input.transform);
        if transformed.previous_point.is_none() {
            return Err(AreSamplerMaterializationError::MissingSourceSegment {
                record_index,
                source_point_index: point_index,
            });
        }
        return Ok(transformed);
    }
    if point_index == 0 || point_index >= input.source_input.points.len() {
        return Err(AreSamplerMaterializationError::MissingSourceSegment {
            record_index,
            source_point_index: point_index,
        });
    }
    let start = input
        .source_input
        .transform
        .apply(input.source_input.points[point_index - 1]);
    let end = input
        .source_input
        .transform
        .apply(input.source_input.points[point_index]);
    let verb = input.source_input.verbs[point_index];
    Ok(AreSourceSegment {
        contour_id: crate::AreSourceContourId(0),
        segment_id: AreSourceSegmentId(point_index),
        source_point_index: point_index,
        verb,
        previous_point: Some(start),
        endpoint: end,
        kind: match verb {
            ArePathVerb::LineTo => AreSourceSegmentKind::Line,
            ArePathVerb::QuadTo => AreSourceSegmentKind::Quadratic { control: None },
            ArePathVerb::CubicTo => AreSourceSegmentKind::Cubic { controls: None },
            ArePathVerb::Close => AreSourceSegmentKind::Close(AreContourClose {
                from: start,
                to: end,
            }),
            ArePathVerb::MoveTo | ArePathVerb::Unknown(_) => AreSourceSegmentKind::Move,
        },
        glyph_run_index: None,
        glyph_id: None,
        is_close_boundary: verb == ArePathVerb::Close,
    })
}

fn active_edges_from_curve_segment(
    _input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    record_index: usize,
    segment: &AreSourceSegment,
) -> Result<Vec<AreActiveEdge>, AreSamplerMaterializationError> {
    let Some(start) = segment.previous_point else {
        return Err(AreSamplerMaterializationError::MissingSourceSegment {
            record_index,
            source_point_index: segment.source_point_index,
        });
    };

    let mut edges = Vec::new();
    match segment.kind {
        AreSourceSegmentKind::Quadratic {
            control: Some(control),
        } => {
            let (control_1, control_2) =
                quadratic_to_cubic_controls(start, control.control, segment.endpoint);
            push_db98_cubic_active_edges(
                start,
                control_1,
                control_2,
                segment.endpoint,
                0,
                &mut edges,
            );
        }
        AreSourceSegmentKind::Cubic {
            controls: Some(controls),
        } => {
            push_db98_cubic_active_edges(
                start,
                controls.control_1,
                controls.control_2,
                segment.endpoint,
                0,
                &mut edges,
            );
        }
        AreSourceSegmentKind::Quadratic { .. } | AreSourceSegmentKind::Cubic { .. } => {
            return Err(
                AreSamplerMaterializationError::UnsupportedCurveRequiresControlPoints {
                    row_y,
                    run_index,
                    record_index,
                    verb: segment.verb,
                },
            );
        }
        _ => return Ok(Vec::new()),
    }

    Ok(edges)
}

fn quadratic_to_cubic_controls(
    start: ArePathPoint,
    control: ArePathPoint,
    end: ArePathPoint,
) -> (ArePathPoint, ArePathPoint) {
    (
        ArePathPoint {
            x: start.x + QUADRATIC_TO_CUBIC_CONTROL_SCALE * (control.x - start.x),
            y: start.y + QUADRATIC_TO_CUBIC_CONTROL_SCALE * (control.y - start.y),
        },
        ArePathPoint {
            x: end.x + QUADRATIC_TO_CUBIC_CONTROL_SCALE * (control.x - end.x),
            y: end.y + QUADRATIC_TO_CUBIC_CONTROL_SCALE * (control.y - end.y),
        },
    )
}

fn push_db98_cubic_active_edges(
    p0: ArePathPoint,
    p1: ArePathPoint,
    p2: ArePathPoint,
    p3: ArePathPoint,
    depth: u8,
    edges: &mut Vec<AreActiveEdge>,
) {
    if depth > ARE_DB98_CURVE_MAX_DEPTH || db98_flat_enough(p0, p1, p2, p3) {
        push_5258_active_edge(p0, p3, edges);
        return;
    }

    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let p0123 = midpoint(p012, p123);
    let next_depth = depth + 1;

    push_db98_cubic_active_edges(p0, p01, p012, p0123, next_depth, edges);
    push_db98_cubic_active_edges(p0123, p123, p23, p3, next_depth, edges);
}

fn db98_flat_enough(
    p0: ArePathPoint,
    control_1: ArePathPoint,
    control_2: ArePathPoint,
    p3: ArePathPoint,
) -> bool {
    let min_x = p0.x.min(p3.x) - ARE_DB98_FLATNESS;
    let max_x = p0.x.max(p3.x) + ARE_DB98_FLATNESS;
    let min_y = p0.y.min(p3.y) - ARE_DB98_FLATNESS;
    let max_y = p0.y.max(p3.y) + ARE_DB98_FLATNESS;

    if control_1.x < min_x
        || control_1.x > max_x
        || control_2.x < min_x
        || control_2.x > max_x
        || control_1.y < min_y
        || control_1.y > max_y
        || control_2.y < min_y
        || control_2.y > max_y
    {
        return false;
    }

    let extent = (p0.x - p3.x).abs().max((p3.y - p0.y).abs());
    if extent <= ARE_DB98_FLATNESS * ARE_DB98_SHORT_CHORD_RATIO {
        return true;
    }

    let dx = p3.x - p0.x;
    let dy = p3.y - p0.y;
    let d1 = ((control_1.x - p0.x) * dy - (control_1.y - p0.y) * dx).abs();
    let d2 = ((control_2.x - p0.x) * dy - (control_2.y - p0.y) * dx).abs();
    let tolerance = ARE_DB98_FLATNESS * extent * ARE_DB98_TOLERANCE_SCALE;
    d1 <= tolerance && d2 <= tolerance
}

fn midpoint(a: ArePathPoint, b: ArePathPoint) -> ArePathPoint {
    ArePathPoint {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    }
}

fn push_5258_active_edge(start: ArePathPoint, end: ArePathPoint, edges: &mut Vec<AreActiveEdge>) {
    if !start.x.is_finite() || !start.y.is_finite() || !end.x.is_finite() || !end.y.is_finite() {
        return;
    }
    if (start.y.floor() as i32) == (end.y.floor() as i32) {
        return;
    }

    let (start, end, winding_delta) = if end.y <= start.y {
        (end, start, 1)
    } else {
        (start, end, -1)
    };
    edges.push(AreActiveEdge::line_fixed(
        to_fixed(start.x),
        to_fixed(start.y),
        to_fixed(end.x),
        to_fixed(end.y),
        winding_delta,
    ));
}

fn push_line_active_edge(start: ArePathPoint, end: ArePathPoint) -> Option<AreActiveEdge> {
    let mut edges = Vec::with_capacity(1);
    push_5258_active_edge(start, end, &mut edges);
    edges.pop()
}

fn active_edges_for_run(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    records: &[(usize, &AreSourceRecord32)],
) -> Result<Vec<AreActiveEdge>, AreSamplerMaterializationError> {
    let mut edges = Vec::new();
    for (record_index, record) in records {
        edges.extend(active_edges_from_record(
            input,
            row_y,
            run_index,
            *record_index,
            record,
        )?);
    }

    if edges.len() < 2 {
        for (record_index, edge) in edge_pair_policy_edges(input, row_y, run_index, run, records)? {
            if !records
                .iter()
                .any(|(existing_record_index, _)| *existing_record_index == record_index)
            {
                edges.push(edge);
            }
        }
    }

    if edges.len() < 2 {
        return Err(
            AreSamplerMaterializationError::UnsupportedAmbiguousLineGeometry {
                row_y,
                run_index,
                source_record_indices: run.source_record_indices.clone(),
            },
        );
    }

    Ok(edges)
}

fn active_edges_from_record(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    record_index: usize,
    record: &AreSourceRecord32,
) -> Result<Vec<AreActiveEdge>, AreSamplerMaterializationError> {
    let segment = source_segment_detail(input, record_index, record)?;
    let Some(start) = segment.previous_point else {
        return Err(AreSamplerMaterializationError::MissingSourceSegment {
            record_index,
            source_point_index: record.source_point_index_0x00,
        });
    };
    let end = segment.endpoint;
    match segment.verb {
        ArePathVerb::LineTo | ArePathVerb::Close => {}
        ArePathVerb::QuadTo | ArePathVerb::CubicTo => {
            return active_edges_from_curve_segment(
                input,
                row_y,
                run_index,
                record_index,
                &segment,
            );
        }
        ArePathVerb::MoveTo | ArePathVerb::Unknown(_) => {
            return Err(AreSamplerMaterializationError::MissingSourceSegment {
                record_index,
                source_point_index: record.source_point_index_0x00,
            });
        }
    }

    Ok(push_line_active_edge(start, end).into_iter().collect())
}

fn edge_pair_policy_edges(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    _run: &AreSamplerIntervalRun,
    records: &[(usize, &AreSourceRecord32)],
) -> Result<Vec<(usize, AreActiveEdge)>, AreSamplerMaterializationError> {
    let (record_index, record) = records[0];
    let Some(record_segment) = input.working_set.source_segment_for_record(record) else {
        return Ok(Vec::new());
    };
    if !is_materializable_boundary_segment(record_segment) {
        return Ok(Vec::new());
    }
    let Some(candidate) = classify_edge_pair_candidate(&input.working_set, record_index) else {
        return Ok(Vec::new());
    };
    if !matches!(
        candidate.kind,
        crate::AreEdgePairCandidateKind::NeedsOppositeEdge
            | crate::AreEdgePairCandidateKind::ContourPairCandidate
            | crate::AreEdgePairCandidateKind::CurveFlatteningCandidate
    ) {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
    for (candidate_index, candidate_record) in input.working_set.records.iter().enumerate() {
        if candidate_index == record_index {
            continue;
        }
        let Some(candidate_segment) = input
            .working_set
            .source_segment_for_record(candidate_record)
        else {
            continue;
        };
        if candidate_segment.contour_id != record_segment.contour_id
            || candidate_segment.glyph_id != record_segment.glyph_id
            || candidate_segment.glyph_run_index != record_segment.glyph_run_index
            || !is_materializable_boundary_segment(candidate_segment)
            || !record_active_on_row(candidate_record, row_y)
        {
            continue;
        }
        for edge in
            active_edges_from_record(input, row_y, run_index, candidate_index, candidate_record)?
        {
            candidates.push((candidate_index, edge));
        }
    }

    Ok(candidates)
}

fn is_materializable_boundary_segment(segment: &AreSourceSegment) -> bool {
    matches!(
        segment.kind,
        AreSourceSegmentKind::Line
            | AreSourceSegmentKind::Close(_)
            | AreSourceSegmentKind::Quadratic { control: Some(_) }
            | AreSourceSegmentKind::Cubic { controls: Some(_) }
    )
}

fn record_active_on_row(record: &AreSourceRecord32, row_y: i32) -> bool {
    if record.max_y_0x14 <= record.min_y_0x0c {
        row_y == record.min_y_0x0c
    } else {
        row_y >= record.min_y_0x0c && row_y < record.max_y_0x14
    }
}

fn to_fixed(value: f32) -> i32 {
    (value * crate::ARE_FIXED_SUBPIXEL_SCALE as f32).round() as i32
}

fn coverage_to_payload_byte(value_0x264: u16) -> u8 {
    value_0x264.min(ARE_FULL_COVERAGE_0X264 - 1) as u8
}

fn materialized_coverage_byte(
    input: &AreSamplerMaterializerInput,
    record: &AreSourceRecord32,
    x: i32,
) -> u8 {
    let point_index = record.source_point_index_0x00;
    let start = input
        .source_input
        .transform
        .apply(input.source_input.points[point_index - 1]);
    let end = input
        .source_input
        .transform
        .apply(input.source_input.points[point_index]);
    let segment_width = (end.x - start.x).abs().max(1.0);
    let local = ((x as f32 + 0.5) - start.x.min(end.x)) / segment_width;
    let clamped = local.clamp(0.0, 1.0);
    (clamped * 255.0).round() as u8
}

fn descriptor_triplet_for_materialized_interval(
    interval: &AreMaterializedInterval,
) -> AreDescriptorTriplet {
    let first_record = interval.source_record_indices.first().copied().unwrap_or(0);
    let base = 0xA500 + first_record * 0x40 + interval.run_index * 0x08;
    AreDescriptorTriplet {
        base,
        second: base + 0x08,
        source: base + 0x10,
    }
}

fn source_probe_for_materialized_interval(interval: &AreMaterializedInterval) -> usize {
    0xD500
        + interval.source_record_indices.first().copied().unwrap_or(0) * 0x40
        + interval.run_index
}

#[cfg(test)]
mod materializer_tests {
    use super::*;
    use crate::{
        build_descriptor_cursor_from_95cc_intervals_with_policy,
        descriptor_builder_to_event_stream, emit_ad68_rows, ArePathPoint,
        AreSamplerIntervalBoundary, AreSamplerIntervalList, AreSamplerIntervalRow,
        AreSamplerIntervalRun, AreSamplerListWorkingState, AreZeroWidthIntervalPolicy,
    };

    fn source(points: Vec<ArePathPoint>, verbs: Vec<ArePathVerb>) -> AreBezierSourcePathInput {
        AreBezierSourcePathInput::source_owned(points, verbs)
    }

    fn input_from_source(source_input: &AreBezierSourcePathInput) -> AreSamplerMaterializerInput {
        build_materializer_input_from_source(source_input).unwrap()
    }

    fn input_with_manual_run(
        source_input: AreBezierSourcePathInput,
        row_y: i32,
        current_x: i32,
        next_x: i32,
        source_record_indices: Vec<usize>,
    ) -> AreSamplerMaterializerInput {
        let working_set = build_e854_working_set(&source_input).unwrap();
        let interval_list = AreSamplerIntervalList {
            provenance: AreSourcePathProvenance::SourceOwned,
            y_min: row_y,
            y_max: row_y + 1,
            rows: vec![AreSamplerIntervalRow {
                row_y,
                boundaries: Vec::<AreSamplerIntervalBoundary>::new(),
                runs: vec![AreSamplerIntervalRun {
                    current_x,
                    next_x,
                    tag: AreSamplerIntervalTag::SourceSpan,
                    source_record_indices,
                    materialize_candidate: true,
                }],
                is_empty_sentinel: false,
            }],
            source_record_count: working_set.records.len(),
            working_state: AreSamplerListWorkingState {
                current_record_index: working_set.records.len(),
                emitted_boundary_count: 0,
                emitted_run_count: 1,
                skipped_zero_width_record_count: 0,
            },
            zero_width_policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
            zero_width_records: Vec::new(),
        };
        AreSamplerMaterializerInput::source_owned(source_input, working_set, interval_list)
    }

    fn horizontal_source() -> AreBezierSourcePathInput {
        source(
            vec![ArePathPoint::new(2.0, 4.0), ArePathPoint::new(6.0, 4.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        )
    }

    fn vertical_pair_source() -> AreBezierSourcePathInput {
        source(
            vec![
                ArePathPoint::new(2.25, 4.0),
                ArePathPoint::new(2.25, 5.0),
                ArePathPoint::new(2.75, 5.0),
                ArePathPoint::new(2.75, 4.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        )
    }

    fn diagonal_pair_source() -> AreBezierSourcePathInput {
        source(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(1.0, 1.0),
                ArePathPoint::new(2.0, 1.0),
                ArePathPoint::new(1.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        )
    }

    fn overlap_source() -> AreBezierSourcePathInput {
        source(
            vec![
                ArePathPoint::new(0.25, 0.0),
                ArePathPoint::new(0.25, 1.0),
                ArePathPoint::new(1.25, 0.0),
                ArePathPoint::new(1.25, 1.0),
                ArePathPoint::new(2.25, 1.0),
                ArePathPoint::new(2.25, 0.0),
                ArePathPoint::new(3.25, 1.0),
                ArePathPoint::new(3.25, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        )
    }

    fn closed_rectangle_source() -> AreBezierSourcePathInput {
        source(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(0.0, 2.0),
                ArePathPoint::new(2.0, 2.0),
                ArePathPoint::new(2.0, 0.0),
                ArePathPoint::new(0.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::Close,
            ],
        )
    }

    fn quadratic_cap_source() -> AreBezierSourcePathInput {
        let start = ArePathPoint::new(0.0, 0.0);
        let control = ArePathPoint::new(1.0, 2.0);
        let end = ArePathPoint::new(2.0, 0.0);
        source(
            vec![start, end, start],
            vec![ArePathVerb::MoveTo, ArePathVerb::QuadTo, ArePathVerb::Close],
        )
        .with_segments(vec![
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(0),
                source_point_index: 0,
                verb: ArePathVerb::MoveTo,
                previous_point: None,
                endpoint: start,
                kind: AreSourceSegmentKind::Move,
                glyph_run_index: None,
                glyph_id: None,
                is_close_boundary: false,
            },
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(1),
                source_point_index: 1,
                verb: ArePathVerb::QuadTo,
                previous_point: Some(start),
                endpoint: end,
                kind: AreSourceSegmentKind::Quadratic {
                    control: Some(crate::AreQuadraticControl { control }),
                },
                glyph_run_index: None,
                glyph_id: None,
                is_close_boundary: false,
            },
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(2),
                source_point_index: 2,
                verb: ArePathVerb::Close,
                previous_point: Some(end),
                endpoint: start,
                kind: AreSourceSegmentKind::Close(crate::AreContourClose {
                    from: end,
                    to: start,
                }),
                glyph_run_index: None,
                glyph_id: None,
                is_close_boundary: true,
            },
        ])
    }

    fn cubic_cap_source() -> AreBezierSourcePathInput {
        let start = ArePathPoint::new(0.0, 0.0);
        let control_1 = ArePathPoint::new(0.0, 2.0);
        let control_2 = ArePathPoint::new(2.0, 2.0);
        let end = ArePathPoint::new(2.0, 0.0);
        source(
            vec![start, end, start],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::CubicTo,
                ArePathVerb::Close,
            ],
        )
        .with_segments(vec![
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(0),
                source_point_index: 0,
                verb: ArePathVerb::MoveTo,
                previous_point: None,
                endpoint: start,
                kind: AreSourceSegmentKind::Move,
                glyph_run_index: None,
                glyph_id: None,
                is_close_boundary: false,
            },
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(1),
                source_point_index: 1,
                verb: ArePathVerb::CubicTo,
                previous_point: Some(start),
                endpoint: end,
                kind: AreSourceSegmentKind::Cubic {
                    controls: Some(crate::AreCubicControl {
                        control_1,
                        control_2,
                    }),
                },
                glyph_run_index: None,
                glyph_id: None,
                is_close_boundary: false,
            },
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(2),
                source_point_index: 2,
                verb: ArePathVerb::Close,
                previous_point: Some(end),
                endpoint: start,
                kind: AreSourceSegmentKind::Close(crate::AreContourClose {
                    from: end,
                    to: start,
                }),
                glyph_run_index: None,
                glyph_id: None,
                is_close_boundary: true,
            },
        ])
    }

    fn alternating_vertical_edges_source() -> AreBezierSourcePathInput {
        source(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(0.0, 1.0),
                ArePathPoint::new(1.0, 1.0),
                ArePathPoint::new(1.0, 0.0),
                ArePathPoint::new(2.0, 0.0),
                ArePathPoint::new(2.0, 1.0),
                ArePathPoint::new(3.0, 1.0),
                ArePathPoint::new(3.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
            ],
        )
    }

    #[test]
    fn materialize_single_horizontal_interval() {
        let input = input_from_source(&horizontal_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();

        let interval = materialized.interval(4, 0).unwrap();

        assert_eq!(interval.current_x, 2);
        assert_eq!(interval.next_x, 6);
        assert_eq!(
            interval.payload.as_ref().unwrap().bytes,
            vec![32, 96, 159, 223]
        );
        assert_eq!(interval.payload.as_ref().unwrap().payload_len, 4);
    }

    #[test]
    fn materialize_two_intervals_one_row() {
        let source_input = source(
            vec![
                ArePathPoint::new(0.0, 3.0),
                ArePathPoint::new(2.0, 3.0),
                ArePathPoint::new(5.0, 3.0),
                ArePathPoint::new(7.0, 3.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let input = input_from_source(&source_input);
        let materialized = materialize_95cc_intervals(&input).unwrap();

        assert_eq!(materialized.intervals.len(), 2);
        assert_eq!(
            materialized
                .interval(3, 0)
                .unwrap()
                .payload
                .as_ref()
                .unwrap()
                .bytes,
            vec![64, 191]
        );
        assert_eq!(
            materialized
                .interval(3, 1)
                .unwrap()
                .payload
                .as_ref()
                .unwrap()
                .bytes,
            vec![64, 191]
        );
    }

    #[test]
    fn multi_record_two_horizontal_segments() {
        let source_input = source(
            vec![
                ArePathPoint::new(0.0, 3.0),
                ArePathPoint::new(2.0, 3.0),
                ArePathPoint::new(2.0, 3.0),
                ArePathPoint::new(5.0, 3.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let input = input_from_source(&source_input);
        let run = &input.interval_list.row(3).unwrap().runs[0];

        assert_eq!(run.current_x, 0);
        assert_eq!(run.next_x, 5);
        assert_eq!(run.source_record_indices, vec![0, 1]);

        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(3, 0).unwrap();

        assert_eq!(interval.source_record_indices, vec![0, 1]);
        assert_eq!(
            interval.payload.as_ref().unwrap().bytes,
            vec![64, 191, 43, 128, 213]
        );
        assert_eq!(interval.payload.as_ref().unwrap().payload_len, 5);
    }

    #[test]
    fn multi_record_preserves_payload_order() {
        let source_input = source(
            vec![
                ArePathPoint::new(10.0, 2.0),
                ArePathPoint::new(12.0, 2.0),
                ArePathPoint::new(12.0, 2.0),
                ArePathPoint::new(15.0, 2.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let input = input_from_source(&source_input);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(2, 0).unwrap();

        assert_eq!(interval.current_x, 10);
        assert_eq!(interval.next_x, 15);
        assert_eq!(interval.source_record_indices, vec![0, 1]);
        assert_eq!(
            interval.payload.as_ref().unwrap().bytes,
            vec![64, 191, 43, 128, 213]
        );
    }

    #[test]
    fn multi_record_rejects_single_diagonal_without_pair() {
        let source_input = source(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(2.0, 2.0),
                ArePathPoint::new(2.0, 0.0),
                ArePathPoint::new(5.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let input = input_from_source(&source_input);

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(
                AreSamplerMaterializationError::UnsupportedAmbiguousLineGeometry {
                    row_y: 0,
                    run_index: 0,
                    source_record_indices: vec![0, 1],
                }
            )
        );
    }

    #[test]
    fn materializer_uses_crossing_list_for_vertical_pair() {
        let input = input_from_source(&vertical_pair_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(4, 0).unwrap();

        assert_eq!(interval.current_x, 2);
        assert_eq!(interval.next_x, 3);
        assert_eq!(interval.source_record_indices, vec![0, 1]);
        assert_eq!(interval.payload.as_ref().unwrap().bytes, vec![128]);
    }

    #[test]
    fn materializer_uses_crossing_list_for_diagonal_pair() {
        let input = input_from_source(&diagonal_pair_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(interval.current_x, 0);
        assert_eq!(interval.next_x, 2);
        assert_eq!(interval.source_record_indices, vec![0, 1]);
        assert_eq!(interval.payload.as_ref().unwrap().bytes, vec![136, 120]);
    }

    #[test]
    fn materializer_uses_nonzero_winding() {
        let input = input_from_source(&overlap_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(
            interval.payload.as_ref().unwrap().bytes,
            vec![192, 255, 255, 64]
        );
    }

    #[test]
    fn materializer_uses_evenodd_parity() {
        let input = input_from_source(&overlap_source()).with_fill_rule(AreFillRule::EvenOdd);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(
            interval.payload.as_ref().unwrap().bytes,
            vec![192, 64, 192, 64]
        );
    }

    #[test]
    fn materializer_payload_bytes_match_75d0_accumulator() {
        let input = input_from_source(&vertical_pair_source());
        let run = &input.interval_list.row(4).unwrap().runs[0];
        let records = materializable_source_records(&input, 4, 0, run).unwrap();
        let edges = active_edges_for_run(&input, 4, 0, run, &records).unwrap();
        let crossing_lists = build_crossing_lists_for_row(4, &edges, input.fill_rule).unwrap();
        let accumulated = accumulate_75d0_for_sample_x(&crossing_lists, 2).unwrap();
        let materialized = materialize_95cc_intervals(&input).unwrap();

        assert_eq!(accumulated.coverage.value_0x264, 128);
        assert_eq!(
            materialized
                .interval(4, 0)
                .unwrap()
                .payload
                .as_ref()
                .unwrap()
                .bytes,
            vec![coverage_to_payload_byte(accumulated.coverage.value_0x264)]
        );
    }

    #[test]
    fn one_sided_edge_never_materializes() {
        let source_input = source(
            vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(0.0, 2.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        );
        let input = input_with_manual_run(source_input, 0, 0, 1, vec![0]);

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(
                AreSamplerMaterializationError::UnsupportedAmbiguousLineGeometry {
                    row_y: 0,
                    run_index: 0,
                    source_record_indices: vec![0],
                }
            )
        );
    }

    #[test]
    fn opposite_edge_pair_materializes() {
        let input = input_with_manual_run(closed_rectangle_source(), 0, 0, 2, vec![0]);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(interval.source_record_indices, vec![0]);
        assert_eq!(
            interval.state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
        assert!(interval.payload.is_none());
    }

    #[test]
    fn close_boundary_preserves_contour_state() {
        let source_input = closed_rectangle_source();
        let working_set = build_e854_working_set(&source_input).unwrap();
        let close_record_index = working_set
            .records
            .iter()
            .position(|record| {
                working_set
                    .source_segment_for_record(record)
                    .is_some_and(|segment| segment.verb == ArePathVerb::Close)
            })
            .unwrap();
        let candidate = crate::classify_edge_pair_candidate(&working_set, close_record_index)
            .expect("close boundary should be classified");

        assert_eq!(
            candidate.kind,
            crate::AreEdgePairCandidateKind::CloseBoundaryCandidate
        );
    }

    #[test]
    fn full_edge_pair_uses_class1_state_policy() {
        let input = input_with_manual_run(closed_rectangle_source(), 0, 0, 2, vec![0]);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert!(interval.payload.is_none());
        assert_eq!(
            interval.state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
    }

    #[test]
    fn multi_contour_pairing_respects_contour_id() {
        let source_input = source(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(0.0, 2.0),
                ArePathPoint::new(3.0, 0.0),
                ArePathPoint::new(3.0, 2.0),
                ArePathPoint::new(5.0, 2.0),
                ArePathPoint::new(5.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
            ],
        );
        let unpaired = input_with_manual_run(source_input.clone(), 0, 0, 1, vec![0]);
        let paired = input_with_manual_run(source_input, 0, 3, 5, vec![1]);

        assert!(matches!(
            materialize_95cc_intervals(&unpaired),
            Err(AreSamplerMaterializationError::UnsupportedAmbiguousLineGeometry { .. })
        ));
        let materialized = materialize_95cc_intervals(&paired).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert!(interval.payload.is_none());
        assert_eq!(
            interval.state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
    }

    #[test]
    fn no_fixture_or_synthetic_payload() {
        let mut input = input_with_manual_run(closed_rectangle_source(), 0, 0, 2, vec![0]);
        input.provenance = AreSamplerMaterializationProvenance::FixtureBacked;

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(AreSamplerMaterializationError::FixtureBackedPolicyRejected)
        );
    }

    #[test]
    fn materializer_rejects_curve_without_control_points() {
        let source_input = source(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(1.0, 1.0),
                ArePathPoint::new(2.0, 1.0),
                ArePathPoint::new(1.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::QuadTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let input = input_from_source(&source_input);

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(
                AreSamplerMaterializationError::UnsupportedCurveRequiresControlPoints {
                    row_y: 0,
                    run_index: 0,
                    record_index: 0,
                    verb: ArePathVerb::QuadTo,
                }
            )
        );
    }

    #[test]
    fn quadratic_curve_uses_control_point_on_current_ad68_route() {
        let input = input_from_source(&quadratic_cap_source());
        let curve_record = input
            .working_set
            .records
            .iter()
            .position(|record| {
                input
                    .working_set
                    .source_segment_for_record(record)
                    .is_some_and(|segment| segment.verb == ArePathVerb::QuadTo)
            })
            .unwrap();

        assert_eq!(input.working_set.records[curve_record].max_y_0x14, 2);

        let materialized = materialize_95cc_intervals(&input).unwrap();
        assert!(materialized
            .intervals
            .iter()
            .any(|interval| interval.source_record_indices.contains(&curve_record)));

        let row_table = crate::source_owned_materializer_to_ad68_rows(&materialized).unwrap();
        assert!(row_table
            .rows
            .iter()
            .flatten()
            .any(|node| node.bytes().is_some() || node.state != 0));
    }

    #[test]
    fn cubic_curve_uses_two_control_points_if_supported() {
        let input = input_from_source(&cubic_cap_source());
        let curve_record = input
            .working_set
            .records
            .iter()
            .position(|record| {
                input
                    .working_set
                    .source_segment_for_record(record)
                    .is_some_and(|segment| segment.verb == ArePathVerb::CubicTo)
            })
            .unwrap();

        assert_eq!(input.working_set.records[curve_record].max_y_0x14, 2);

        let materialized = materialize_95cc_intervals(&input).unwrap();
        assert!(materialized
            .intervals
            .iter()
            .any(|interval| interval.source_record_indices.contains(&curve_record)));
    }

    #[test]
    fn quadratic_curve_uses_db98_adaptive_edges() {
        let input = input_from_source(&quadratic_cap_source());
        let curve_record = input
            .working_set
            .records
            .iter()
            .position(|record| {
                input
                    .working_set
                    .source_segment_for_record(record)
                    .is_some_and(|segment| segment.verb == ArePathVerb::QuadTo)
            })
            .unwrap();
        let segment = input
            .working_set
            .source_segment_for_record(&input.working_set.records[curve_record])
            .unwrap();

        let edges = active_edges_from_curve_segment(&input, 0, 0, curve_record, segment).unwrap();

        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].winding_delta, -1);
        assert_eq!(edges[1].winding_delta, 1);
    }

    #[test]
    fn cubic_curve_uses_db98_adaptive_edges() {
        let input = input_from_source(&cubic_cap_source());
        let curve_record = input
            .working_set
            .records
            .iter()
            .position(|record| {
                input
                    .working_set
                    .source_segment_for_record(record)
                    .is_some_and(|segment| segment.verb == ArePathVerb::CubicTo)
            })
            .unwrap();
        let segment = input
            .working_set
            .source_segment_for_record(&input.working_set.records[curve_record])
            .unwrap();

        let edges = active_edges_from_curve_segment(&input, 0, 0, curve_record, segment).unwrap();

        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].winding_delta, -1);
        assert_eq!(edges[1].winding_delta, 1);
    }

    #[test]
    fn ambiguous_edge_pair_uses_same_contour_active_edges() {
        let input = input_with_manual_run(alternating_vertical_edges_source(), 0, 0, 3, vec![0]);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(interval.source_record_indices, vec![0]);
        assert_eq!(interval.payload.as_ref().unwrap().bytes, vec![255, 0, 255]);
    }

    #[test]
    fn full_coverage_maps_class1_state_without_payload() {
        let input = input_with_manual_run(closed_rectangle_source(), 0, 0, 2, vec![0]);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();
        let row_table = crate::source_owned_materializer_to_ad68_rows(&materialized).unwrap();
        let row = row_table.row(0).unwrap();

        assert!(interval.payload.is_none());
        assert_eq!(
            interval.state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
        assert!(row.iter().all(|node| node.bytes().is_none()));
        assert!(row.iter().any(|node| node.state == 1));
    }

    #[test]
    fn mixed_full_and_empty_coverage_stays_class2_payload() {
        let input = input_with_manual_run(alternating_vertical_edges_source(), 0, 0, 3, vec![0]);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(interval.payload.as_ref().unwrap().bytes, vec![255, 0, 255]);
        assert!(interval.state_hint.is_none());
    }

    #[test]
    fn materializer_preserves_sample_x_axis() {
        let source_input = source(
            vec![
                ArePathPoint::new(10.25, 4.0),
                ArePathPoint::new(10.25, 5.0),
                ArePathPoint::new(10.75, 5.0),
                ArePathPoint::new(10.75, 4.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let input = input_from_source(&source_input);
        let materialized = materialize_95cc_intervals(&input).unwrap();

        assert!(materialized.interval(4, 0).is_some());
        assert_eq!(
            materialized
                .interval(4, 0)
                .unwrap()
                .payload
                .as_ref()
                .unwrap()
                .bytes,
            vec![128]
        );
        assert_eq!(materialized.interval(10, 0), None);
    }

    #[test]
    fn multi_record_no_fixture_payload_fallback() {
        let source_input = source(
            vec![
                ArePathPoint::new(0.0, 1.0),
                ArePathPoint::new(2.0, 1.0),
                ArePathPoint::new(2.0, 1.0),
                ArePathPoint::new(4.0, 1.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let mut input = input_from_source(&source_input);
        input.provenance = AreSamplerMaterializationProvenance::FixtureBacked;

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(AreSamplerMaterializationError::FixtureBackedPolicyRejected)
        );
    }

    #[test]
    fn materialized_payload_len_matches_interval_width() {
        let input = input_from_source(&horizontal_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(4, 0).unwrap();

        assert_eq!(
            interval.payload.as_ref().unwrap().payload_len,
            interval.width() as usize
        );
        assert_eq!(
            interval.payload.as_ref().unwrap().bytes.len(),
            interval.width() as usize
        );
    }

    #[test]
    fn materialized_payload_window_allocated() {
        let input = input_from_source(&horizontal_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let policy = descriptor_policy_from_materialized_intervals(&materialized);
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&input.interval_list, &policy)
                .unwrap();
        let descriptor = bridge
            .builder
            .descriptor(bridge.descriptor_refs[0].id)
            .unwrap();

        assert!(descriptor.payload_window.is_some());
        let window = descriptor.payload_window.unwrap();
        let backing = bridge
            .builder
            .payload_allocator
            .backing(window.backing_id)
            .unwrap();
        assert_eq!(
            &backing.bytes[window.offset..window.offset + window.len],
            &[32, 96, 159, 223]
        );
    }

    #[test]
    fn materializer_rejects_typed_span_input() {
        let source_input = horizontal_source();
        let mut input = input_from_source(&source_input);
        input.provenance = AreSamplerMaterializationProvenance::TypedSpanFallback;

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(AreSamplerMaterializationError::TypedSpanFallbackRejected)
        );
    }

    #[test]
    fn materializer_rejects_coverage_row_input() {
        let source_input = horizontal_source();
        let mut input = input_from_source(&source_input);
        input.provenance = AreSamplerMaterializationProvenance::CoverageRowFallback;

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(AreSamplerMaterializationError::CoverageRowFallbackRejected)
        );
    }

    #[test]
    fn materializer_requires_source_owned_provenance() {
        let source_input = horizontal_source();
        let mut input = input_from_source(&source_input);
        input.source_input.provenance = AreSourcePathProvenance::FocusedFixture;

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(
                AreSamplerMaterializationError::RequiresSourceOwnedProvenance {
                    provenance: AreSourcePathProvenance::FocusedFixture,
                }
            )
        );
    }

    #[test]
    fn materializer_rejects_unclassified_zero_width() {
        let source_input = horizontal_source();
        let working_set = build_e854_working_set(&source_input).unwrap();
        let interval_list = AreSamplerIntervalList {
            provenance: AreSourcePathProvenance::SourceOwned,
            y_min: 4,
            y_max: 4,
            rows: vec![AreSamplerIntervalRow {
                row_y: 4,
                boundaries: Vec::<AreSamplerIntervalBoundary>::new(),
                runs: vec![AreSamplerIntervalRun {
                    current_x: 2,
                    next_x: 2,
                    tag: AreSamplerIntervalTag::SourceSpan,
                    source_record_indices: vec![0],
                    materialize_candidate: true,
                }],
                is_empty_sentinel: false,
            }],
            source_record_count: working_set.records.len(),
            working_state: AreSamplerListWorkingState {
                current_record_index: working_set.records.len(),
                emitted_boundary_count: 0,
                emitted_run_count: 1,
                skipped_zero_width_record_count: 0,
            },
            zero_width_policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
            zero_width_records: Vec::new(),
        };
        let input =
            AreSamplerMaterializerInput::source_owned(source_input, working_set, interval_list);

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(AreSamplerMaterializationError::InvalidIntervalWidth {
                row_y: 4,
                run_index: 0,
                current_x: 2,
                next_x: 2,
            })
        );
    }

    #[test]
    fn materialized_intervals_feed_new_source_owned_policy() {
        let input = input_from_source(&horizontal_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let policy = descriptor_policy_from_materialized_intervals(&materialized);
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&input.interval_list, &policy)
                .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let row_table = emit_ad68_rows(&event_stream).unwrap();

        assert_eq!(
            policy.provenance,
            crate::AreDescriptorCursorPolicyProvenance::SourceOwnedMaterialized
        );
        assert_eq!(row_table.row(4).unwrap()[0].x, 2);
        assert_eq!(row_table.row(4).unwrap()[0].len, 4);
        assert_eq!(
            row_table.row(4).unwrap()[0].bytes(),
            Some(&[32, 96, 159, 223][..])
        );
    }

    #[test]
    fn no_renderer_integration() {
        let input = input_from_source(&horizontal_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();

        assert_eq!(
            materialized.provenance,
            AreSamplerMaterializationProvenance::SourceOwnedSamplerMaterialized
        );
        assert_eq!(materialized.intervals.len(), 1);
    }
}
