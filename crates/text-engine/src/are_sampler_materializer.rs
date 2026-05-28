use crate::{
    accumulate_75d0_for_sample_x, build_95cc_interval_list, build_e854_working_set,
    build_native_debug_crossing_lists_for_row, build_native_row_event_crossing_lists_for_row,
    classify_edge_pair_candidate, Are95ccStateClass, AreActiveEdge, AreBezierSourcePathInput,
    AreContourClose, AreCrossingListError, AreDescriptorCursorPolicy, AreDescriptorTriplet,
    AreFillRule, AreIntervalDescriptorPolicy, AreIntervalPayloadPolicy,
    AreNativeDebugCrossingInput, AreNativeDebugEdgeInput, AreNativeRowEventEdgeInput,
    AreNativeRowEventError, AreNativeRowEventInput, AreNativeRowEventMaterializedSpan,
    AreNativeRowEventSpanInput, AreNativeRowEventStateClass, AreNativeRowEventSubrowTrace,
    ArePathPoint, ArePathVerb, AreSamplerIntervalList, AreSamplerIntervalRun,
    AreSamplerIntervalTag, AreSourceContourId, AreSourcePathProvenance, AreSourceRecord32,
    AreSourceSamplerError, AreSourceSamplerWorkingSet, AreSourceSegment, AreSourceSegmentId,
    AreSourceSegmentKind, AreStatePolicy, AreSubrowCrossingArray16, ARE_FIXED_SUBPIXEL_SCALE,
    ARE_FULL_COVERAGE_0X264,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreNativeBucketParitySpan {
    State(AreMaterializedStateHint),
    Payload(Vec<u8>),
}

impl AreNativeBucketParitySpan {
    pub fn class_name(&self) -> &'static str {
        match self {
            AreNativeBucketParitySpan::State(hint) => match hint.state_class {
                Are95ccStateClass::Class0 => "class0",
                Are95ccStateClass::Class1 => "class1",
            },
            AreNativeBucketParitySpan::Payload(_) => "class2",
        }
    }

    pub fn payload_len(&self) -> Option<usize> {
        match self {
            AreNativeBucketParitySpan::State(_) => None,
            AreNativeBucketParitySpan::Payload(bytes) => Some(bytes.len()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeBucketParityRunComparison {
    pub row_y: i32,
    pub run_index: usize,
    pub current_x: i32,
    pub next_x: i32,
    pub source_record_indices: Vec<usize>,
    pub native_debug_edge_count: usize,
    pub flat_span: AreNativeBucketParitySpan,
    pub native_debug_span: AreNativeBucketParitySpan,
    pub differing_subrows: Vec<usize>,
    pub ordering_delta_subrows: Vec<usize>,
    pub unresolved_726c_callback_slots: bool,
    pub span_equal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreSamplerRowEventProjectionDebugStatus {
    Projected,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventEdgeBatch {
    pub edges: Vec<AreNativeRowEventEdgeInput>,
    pub edge_provenance: Vec<AreNativeRowEventEdgeProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_order_4afc: Option<Vec<usize>>,
}

impl AreNativeRowEventEdgeBatch {
    fn empty() -> Self {
        Self {
            edges: Vec::new(),
            edge_provenance: Vec::new(),
            seed_order_4afc: None,
        }
    }

    fn push(
        &mut self,
        edge: AreNativeRowEventEdgeInput,
        provenance: AreNativeRowEventEdgeProvenance,
    ) {
        self.edges.push(edge);
        self.edge_provenance.push(provenance);
    }

    fn extend(&mut self, other: AreNativeRowEventEdgeBatch) {
        self.edges.extend(other.edges);
        self.edge_provenance.extend(other.edge_provenance);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventEdgeProvenance {
    pub source_record_index: usize,
    pub source_point_index: usize,
    pub contour_id: AreSourceContourId,
    pub segment_id: AreSourceSegmentId,
    pub edge_ordinal_within_record: usize,
    pub origin: AreNativeRowEventEdgeOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_4afc_ordinal: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreNativeRowEventEdgeOrigin {
    DirectRecord,
    EdgePairRecovery { requested_record_index: usize },
    CurveFlattening,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSamplerRowEventProjectionDebug {
    pub row_y: i32,
    pub run_index: usize,
    pub current_x: i32,
    pub next_x: i32,
    pub interval_tag: AreSamplerIntervalTag,
    pub materialize_candidate: bool,
    pub source_record_indices: Vec<usize>,
    pub status: AreSamplerRowEventProjectionDebugStatus,
    pub error: Option<String>,
    pub edge_count: usize,
    pub edges: Vec<AreNativeRowEventEdgeInput>,
    pub edge_provenance: Vec<AreNativeRowEventEdgeProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_order_4afc: Option<Vec<usize>>,
    pub coverages_0x264: Vec<u16>,
    pub span_class: Option<String>,
    pub state_class: Option<AreNativeRowEventStateClass>,
    pub state: Option<u8>,
    pub payload_bytes: Option<Vec<u8>>,
    pub crossing_values_by_subrow: Vec<Vec<i32>>,
    pub subrow_traces: Vec<AreNativeRowEventSubrowTrace>,
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
    NativeRowEvent(AreNativeRowEventError),
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
    MissingIntervalRow {
        row_y: i32,
    },
    MissingIntervalRun {
        row_y: i32,
        run_index: usize,
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
            intervals.extend(materialize_run(input, row.row_y, run_index, run)?);
        }
    }

    Ok(AreSamplerMaterialization {
        provenance: input.provenance,
        y_min: input.interval_list.y_min,
        y_max: input.interval_list.y_max,
        intervals,
    })
}

pub fn compare_native_debug_bucket_parity_for_run(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
) -> Result<AreNativeBucketParityRunComparison, AreSamplerMaterializationError> {
    validate_materializer_input(input)?;
    let row = input
        .interval_list
        .row(row_y)
        .ok_or(AreSamplerMaterializationError::MissingIntervalRow { row_y })?;
    let run = row
        .runs
        .get(run_index)
        .ok_or(AreSamplerMaterializationError::MissingIntervalRun { row_y, run_index })?;
    if run.width() <= 0 {
        return Err(AreSamplerMaterializationError::InvalidIntervalWidth {
            row_y,
            run_index,
            current_x: run.current_x,
            next_x: run.next_x,
        });
    }

    let records = materializable_source_records(input, row_y, run_index, run)?;
    let native_debug_edges = native_debug_edges_for_run(input, row_y, run_index, run, &records)?;
    let native_row_event_edges = native_debug_edges
        .iter()
        .map(|edge| AreNativeRowEventEdgeInput::new(edge.contour_id, edge.edge.clone()))
        .collect::<Vec<_>>();
    let native_row_event_output = build_native_row_event_crossing_lists_for_row(
        &AreNativeRowEventInput::source_owned(row_y, input.fill_rule, native_row_event_edges),
    )
    .map_err(AreSamplerMaterializationError::NativeRowEvent)?;
    let native_debug_output = build_native_debug_crossing_lists_for_row(
        &AreNativeDebugCrossingInput::source_owned_debug(
            row_y,
            input.fill_rule,
            native_debug_edges.clone(),
        ),
    )
    .map_err(AreSamplerMaterializationError::CrossingList)?;
    let flat_span = native_bucket_parity_span_from_crossing_lists(
        &native_row_event_output.crossing_lists,
        run,
    )?;
    let native_debug_span =
        native_bucket_parity_span_from_crossing_lists(&native_debug_output.crossing_lists, run)?;
    let span_equal = flat_span == native_debug_span;
    let differing_subrows = native_row_event_output
        .crossing_lists
        .lists
        .iter()
        .zip(&native_debug_output.crossing_lists.lists)
        .filter_map(|(native_row_event, native_debug)| {
            (native_row_event.crossing_values() != native_debug.crossing_values())
                .then_some(native_row_event.subrow_index)
        })
        .collect();
    let ordering_delta_subrows = native_row_event_output
        .subrow_traces
        .iter()
        .zip(&native_debug_output.subrow_traces)
        .filter_map(|(native_row_event, native_debug)| {
            (native_row_event.active_edge_indices != native_debug.native_active_edge_indices)
                .then_some(native_row_event.subrow_index)
        })
        .collect();

    Ok(AreNativeBucketParityRunComparison {
        row_y,
        run_index,
        current_x: run.current_x,
        next_x: run.next_x,
        source_record_indices: run.source_record_indices.clone(),
        native_debug_edge_count: native_debug_edges.len(),
        flat_span,
        native_debug_span,
        differing_subrows,
        ordering_delta_subrows,
        unresolved_726c_callback_slots: false,
        span_equal,
    })
}

pub fn materializer_row_event_projection_debug(
    input: &AreSamplerMaterializerInput,
    max_runs: usize,
) -> Result<Vec<AreSamplerRowEventProjectionDebug>, AreSamplerMaterializationError> {
    validate_materializer_input(input)?;

    let mut debug_runs = Vec::new();
    'rows: for row in &input.interval_list.rows {
        for (run_index, run) in row.runs.iter().enumerate() {
            if debug_runs.len() >= max_runs {
                break 'rows;
            }
            if !run.materialize_candidate || run.tag != AreSamplerIntervalTag::SourceSpan {
                continue;
            }
            debug_runs.push(row_event_projection_debug_for_run(
                input, row.row_y, run_index, run,
            ));
        }
    }
    Ok(debug_runs)
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
) -> Result<Vec<AreMaterializedInterval>, AreSamplerMaterializationError> {
    if run.width() <= 0 {
        return Err(AreSamplerMaterializationError::InvalidIntervalWidth {
            row_y,
            run_index,
            current_x: run.current_x,
            next_x: run.next_x,
        });
    }

    if run.materialize_candidate && run.tag == AreSamplerIntervalTag::SourceSpan {
        return materialize_source_span(input, row_y, run_index, run);
    } else {
        let state_hint = materialize_state_hint(input, row_y, run_index, run)?;
        return Ok(vec![state_interval(
            row_y,
            run_index,
            run.current_x,
            run.next_x,
            &run.source_record_indices,
            state_hint,
        )]);
    }
}

fn materialize_source_span(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> Result<Vec<AreMaterializedInterval>, AreSamplerMaterializationError> {
    let records = materializable_source_records(input, row_y, run_index, run)?;
    if records
        .iter()
        .all(|(record_index, record)| is_horizontal_unit_record(input, *record_index, record))
    {
        let bytes =
            legacy_materialize_horizontal_unit_payload(input, row_y, run_index, run, &records)?;
        return Ok(vec![payload_interval(
            row_y,
            run_index,
            run.current_x,
            run.next_x,
            &run.source_record_indices,
            bytes,
        )]);
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
) -> Result<Vec<AreMaterializedInterval>, AreSamplerMaterializationError> {
    let batch = native_row_event_edge_batch_for_run(input, row_y, run_index, run, records)?;
    let span_input = native_row_event_span_input_from_batch(row_y, run, input.fill_rule, &batch);
    let materialization =
        crate::materialize_source_span_with_native_row_events_with_trace(&span_input)
            .map_err(AreSamplerMaterializationError::NativeRowEvent)?;
    Ok(materialized_intervals_from_native_row_event_trace(
        row_y,
        run_index,
        run,
        &materialization.coverages_0x264,
    ))
}

fn payload_interval(
    row_y: i32,
    run_index: usize,
    current_x: i32,
    next_x: i32,
    source_record_indices: &[usize],
    bytes: Vec<u8>,
) -> AreMaterializedInterval {
    AreMaterializedInterval {
        row_y,
        run_index,
        current_x,
        next_x,
        source_record_indices: source_record_indices.to_vec(),
        payload: Some(AreMaterializedPayload {
            payload_len: bytes.len(),
            bytes,
        }),
        state_hint: None,
    }
}

fn state_interval(
    row_y: i32,
    run_index: usize,
    current_x: i32,
    next_x: i32,
    source_record_indices: &[usize],
    state_hint: AreMaterializedStateHint,
) -> AreMaterializedInterval {
    AreMaterializedInterval {
        row_y,
        run_index,
        current_x,
        next_x,
        source_record_indices: source_record_indices.to_vec(),
        payload: None,
        state_hint: Some(state_hint),
    }
}

fn materialized_intervals_from_native_row_event_trace(
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    coverages_0x264: &[u16],
) -> Vec<AreMaterializedInterval> {
    if coverages_0x264.is_empty() {
        return vec![state_interval(
            row_y,
            run_index,
            run.current_x,
            run.next_x,
            &run.source_record_indices,
            AreMaterializedStateHint {
                state_class: Are95ccStateClass::Class0,
                state: 0,
            },
        )];
    }

    let mut intervals = Vec::new();
    let mut start = 0usize;
    while start < coverages_0x264.len() {
        let class = coverage_state_class(coverages_0x264[start]);
        let mut end = start + 1;
        while end < coverages_0x264.len() && coverage_state_class(coverages_0x264[end]) == class {
            end += 1;
        }

        let current_x = run.current_x + start as i32;
        let next_x = run.current_x + end as i32;
        match class {
            Some(state_class) => intervals.push(state_interval(
                row_y,
                run_index,
                current_x,
                next_x,
                &run.source_record_indices,
                AreMaterializedStateHint {
                    state_class,
                    state: match state_class {
                        Are95ccStateClass::Class0 => 0,
                        Are95ccStateClass::Class1 => 1,
                    },
                },
            )),
            None => intervals.push(payload_interval(
                row_y,
                run_index,
                current_x,
                next_x,
                &run.source_record_indices,
                coverages_0x264[start..end]
                    .iter()
                    .map(|coverage| coverage_to_payload_byte(*coverage))
                    .collect(),
            )),
        }
        start = end;
    }
    intervals
}

fn coverage_state_class(value_0x264: u16) -> Option<Are95ccStateClass> {
    match value_0x264 {
        0 => Some(Are95ccStateClass::Class0),
        ARE_FULL_COVERAGE_0X264 => Some(Are95ccStateClass::Class1),
        _ => None,
    }
}

fn native_row_event_span_input_from_batch(
    row_y: i32,
    run: &AreSamplerIntervalRun,
    fill_rule: AreFillRule,
    batch: &AreNativeRowEventEdgeBatch,
) -> AreNativeRowEventSpanInput {
    let span_input = AreNativeRowEventSpanInput::source_owned(
        row_y,
        run.current_x,
        run.next_x,
        fill_rule,
        batch.edges.clone(),
    );
    if let Some(seed_order_4afc) = &batch.seed_order_4afc {
        span_input.with_seed_order_4afc(seed_order_4afc.clone())
    } else {
        span_input
    }
}

fn row_event_projection_debug_for_run(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> AreSamplerRowEventProjectionDebug {
    let mut debug = AreSamplerRowEventProjectionDebug {
        row_y,
        run_index,
        current_x: run.current_x,
        next_x: run.next_x,
        interval_tag: run.tag,
        materialize_candidate: run.materialize_candidate,
        source_record_indices: run.source_record_indices.clone(),
        status: AreSamplerRowEventProjectionDebugStatus::Error,
        error: None,
        edge_count: 0,
        edges: Vec::new(),
        edge_provenance: Vec::new(),
        seed_order_4afc: None,
        coverages_0x264: Vec::new(),
        span_class: None,
        state_class: None,
        state: None,
        payload_bytes: None,
        crossing_values_by_subrow: Vec::new(),
        subrow_traces: Vec::new(),
    };

    if run.width() <= 0 {
        debug.error = Some(format!(
            "invalid_interval_width row_y={} run_index={} current_x={} next_x={}",
            row_y, run_index, run.current_x, run.next_x
        ));
        return debug;
    }

    let records = match materializable_source_records(input, row_y, run_index, run) {
        Ok(records) => records,
        Err(err) => {
            debug.error = Some(format!("{:?}", err));
            return debug;
        }
    };
    let batch = match native_row_event_edge_batch_for_run(input, row_y, run_index, run, &records) {
        Ok(batch) => batch,
        Err(err) => {
            debug.error = Some(format!("{:?}", err));
            return debug;
        }
    };
    debug.edge_count = batch.edges.len();
    debug.edges = batch.edges.clone();
    debug.edge_provenance = batch.edge_provenance.clone();
    debug.seed_order_4afc = batch.seed_order_4afc.clone();

    let materialization = match crate::materialize_source_span_with_native_row_events_with_trace(
        &native_row_event_span_input_from_batch(row_y, run, input.fill_rule, &batch),
    )
    .map_err(AreSamplerMaterializationError::NativeRowEvent)
    {
        Ok(materialization) => materialization,
        Err(err) => {
            debug.error = Some(format!("{:?}", err));
            return debug;
        }
    };

    debug.status = AreSamplerRowEventProjectionDebugStatus::Projected;
    debug.span_class = Some(materialization.span.class_name().to_string());
    match &materialization.span {
        AreNativeRowEventMaterializedSpan::State { class, state } => {
            debug.state_class = Some(*class);
            debug.state = Some(*state);
        }
        AreNativeRowEventMaterializedSpan::Payload(bytes) => {
            debug.payload_bytes = Some(bytes.clone());
        }
    }
    debug.coverages_0x264 = materialization.coverages_0x264;
    debug.crossing_values_by_subrow = materialization
        .crossing_output
        .crossing_lists
        .lists
        .iter()
        .map(|list| list.crossing_values())
        .collect();
    debug.subrow_traces = materialization.crossing_output.subrow_traces;
    debug
}

fn native_bucket_parity_span_from_crossing_lists(
    crossing_lists: &AreSubrowCrossingArray16,
    run: &AreSamplerIntervalRun,
) -> Result<AreNativeBucketParitySpan, AreSamplerMaterializationError> {
    let mut coverages = Vec::with_capacity(run.width() as usize);
    for sample_x in run.current_x..run.next_x {
        let coverage = accumulate_75d0_for_sample_x(crossing_lists, sample_x)
            .map_err(AreSamplerMaterializationError::CrossingList)?
            .coverage
            .value_0x264;
        coverages.push(coverage);
    }
    if coverages.iter().all(|coverage| *coverage == 0) {
        return Ok(AreNativeBucketParitySpan::State(AreMaterializedStateHint {
            state_class: Are95ccStateClass::Class0,
            state: 0,
        }));
    }
    if coverages
        .iter()
        .all(|coverage| *coverage == ARE_FULL_COVERAGE_0X264)
    {
        return Ok(AreNativeBucketParitySpan::State(AreMaterializedStateHint {
            state_class: Are95ccStateClass::Class1,
            state: 1,
        }));
    }
    Ok(AreNativeBucketParitySpan::Payload(
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
    let mut records = run.source_record_indices.iter();
    let Some(first_record_index) = records.next().copied() else {
        return Err(AreSamplerMaterializationError::UnsupportedMultiRecordRun {
            row_y,
            run_index,
            source_record_indices: run.source_record_indices.clone(),
        });
    };
    let first_record = source_record(input, row_y, run_index, first_record_index)?;
    let state = first_record.merge_state_0x19;
    for record_index in records {
        let record = source_record(input, row_y, run_index, *record_index)?;
        if record.merge_state_0x19 != state {
            return Err(AreSamplerMaterializationError::UnsupportedMultiRecordRun {
                row_y,
                run_index,
                source_record_indices: run.source_record_indices.clone(),
            });
        }
    }

    Ok(AreMaterializedStateHint {
        state_class: Are95ccStateClass::Class0,
        state,
    })
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

#[cfg(test)]
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

fn native_debug_edges_for_run(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    records: &[(usize, &AreSourceRecord32)],
) -> Result<Vec<AreNativeDebugEdgeInput>, AreSamplerMaterializationError> {
    let mut edges = Vec::new();
    for (record_index, record) in records {
        edges.extend(native_debug_edges_from_record(
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
                let contour_id = input
                    .working_set
                    .records
                    .get(record_index)
                    .and_then(|record| input.working_set.source_segment_for_record(record))
                    .map(|segment| segment.contour_id.0)
                    .unwrap_or(0);
                edges.push(AreNativeDebugEdgeInput::new(contour_id, edge));
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

fn native_row_event_edge_batch_for_run(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    records: &[(usize, &AreSourceRecord32)],
) -> Result<AreNativeRowEventEdgeBatch, AreSamplerMaterializationError> {
    let mut batch = AreNativeRowEventEdgeBatch::empty();
    for (record_index, record) in records {
        batch.extend(native_row_event_edge_batch_from_record(
            input,
            row_y,
            run_index,
            *record_index,
            record,
            None,
        )?);
    }

    if batch.edges.len() < 2 {
        let requested_record_index = records.first().map(|(record_index, _)| *record_index);
        for (record_index, edge) in edge_pair_policy_edges(input, row_y, run_index, run, records)? {
            if records
                .iter()
                .any(|(existing_record_index, _)| *existing_record_index == record_index)
            {
                continue;
            }
            let Some(record) = input.working_set.records.get(record_index) else {
                continue;
            };
            let segment = source_segment_detail(input, record_index, record)?;
            batch.push(
                AreNativeRowEventEdgeInput::new(segment.contour_id.0, edge),
                AreNativeRowEventEdgeProvenance {
                    source_record_index: record_index,
                    source_point_index: segment.source_point_index,
                    contour_id: segment.contour_id,
                    segment_id: segment.segment_id,
                    edge_ordinal_within_record: 0,
                    origin: AreNativeRowEventEdgeOrigin::EdgePairRecovery {
                        requested_record_index: requested_record_index.unwrap_or(record_index),
                    },
                    seed_4afc_ordinal: None,
                },
            );
        }
    }

    if batch.edges.len() < 2 {
        return Err(
            AreSamplerMaterializationError::UnsupportedAmbiguousLineGeometry {
                row_y,
                run_index,
                source_record_indices: run.source_record_indices.clone(),
            },
        );
    }

    apply_source_stage_seed_order_4afc(input, row_y, run_index, &mut batch)?;
    if batch.seed_order_4afc.is_none() {
        apply_row_stage_seed_order_4afc(row_y, input.fill_rule, &mut batch)?;
    }
    if batch.seed_order_4afc.is_none() {
        apply_default_71f4_seed_order(row_y, input.fill_rule, &mut batch)?;
    }
    Ok(batch)
}

fn apply_source_stage_seed_order_4afc(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    batch: &mut AreNativeRowEventEdgeBatch,
) -> Result<(), AreSamplerMaterializationError> {
    if batch.edges.is_empty() {
        return Ok(());
    }
    let full_batch = native_row_event_edge_batch_for_source(input, row_y, run_index)?;
    let full_seed_order = command_stage_raw_queue_4afc(&full_batch.edges);
    if full_seed_order.is_empty() {
        return Ok(());
    }

    let mut seen_row_edges = vec![false; batch.edges.len()];
    let mut row_seed_order = Vec::new();
    for full_edge_index in full_seed_order {
        let Some(full_provenance) = full_batch.edge_provenance.get(full_edge_index) else {
            return Ok(());
        };
        let Some(row_edge_index) = batch
            .edge_provenance
            .iter()
            .position(|row_provenance| same_command_stage_edge(row_provenance, full_provenance))
        else {
            continue;
        };
        if seen_row_edges[row_edge_index] {
            return Ok(());
        }
        seen_row_edges[row_edge_index] = true;
        row_seed_order.push(row_edge_index);
    }

    if !row_seed_order_reaches_seedable_edges_71f4(row_y, input.fill_rule, batch, &row_seed_order)?
    {
        return Ok(());
    }

    for provenance in &mut batch.edge_provenance {
        provenance.seed_4afc_ordinal = None;
    }
    for (ordinal, edge_index) in row_seed_order.iter().copied().enumerate() {
        if let Some(provenance) = batch.edge_provenance.get_mut(edge_index) {
            provenance.seed_4afc_ordinal = Some(ordinal);
        }
    }
    batch.seed_order_4afc = Some(row_seed_order);
    Ok(())
}

fn apply_row_stage_seed_order_4afc(
    row_y: i32,
    fill_rule: AreFillRule,
    batch: &mut AreNativeRowEventEdgeBatch,
) -> Result<(), AreSamplerMaterializationError> {
    let row_seed_order = command_stage_raw_queue_4afc(&batch.edges);
    if !row_seed_order_reaches_seedable_edges_71f4(row_y, fill_rule, batch, &row_seed_order)? {
        return Ok(());
    }

    for provenance in &mut batch.edge_provenance {
        provenance.seed_4afc_ordinal = None;
    }
    for (ordinal, edge_index) in row_seed_order.iter().copied().enumerate() {
        if let Some(provenance) = batch.edge_provenance.get_mut(edge_index) {
            provenance.seed_4afc_ordinal = Some(ordinal);
        }
    }
    batch.seed_order_4afc = Some(row_seed_order);
    Ok(())
}

fn apply_default_71f4_seed_order(
    row_y: i32,
    fill_rule: AreFillRule,
    batch: &mut AreNativeRowEventEdgeBatch,
) -> Result<(), AreSamplerMaterializationError> {
    let output = build_native_row_event_crossing_lists_for_row(
        &AreNativeRowEventInput::source_owned(row_y, fill_rule, batch.edges.clone()),
    )
    .map_err(AreSamplerMaterializationError::NativeRowEvent)?;

    let mut seed_order = Vec::new();
    for trace in &output.subrow_traces {
        for edge_index in &trace.row_add_edge_indices {
            if !seed_order.contains(edge_index) {
                seed_order.push(*edge_index);
            }
        }
    }

    if seed_order.is_empty()
        || !row_seed_order_covers_seedable_edges_71f4(row_y, batch, &seed_order)
    {
        return Ok(());
    }

    for provenance in &mut batch.edge_provenance {
        provenance.seed_4afc_ordinal = None;
    }
    for (ordinal, edge_index) in seed_order.iter().copied().enumerate() {
        if let Some(provenance) = batch.edge_provenance.get_mut(edge_index) {
            provenance.seed_4afc_ordinal = Some(ordinal);
        }
    }
    batch.seed_order_4afc = Some(seed_order);
    Ok(())
}

fn native_row_event_edge_batch_for_source(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
) -> Result<AreNativeRowEventEdgeBatch, AreSamplerMaterializationError> {
    let mut batch = AreNativeRowEventEdgeBatch::empty();
    for (record_index, record) in input.working_set.records.iter().enumerate() {
        let Ok(segment) = source_segment_detail(input, record_index, record) else {
            continue;
        };
        if !is_materializable_boundary_segment(&segment) {
            continue;
        }
        batch.extend(native_row_event_edge_batch_from_record(
            input,
            row_y,
            run_index,
            record_index,
            record,
            None,
        )?);
    }
    Ok(batch)
}

fn same_command_stage_edge(
    row_provenance: &AreNativeRowEventEdgeProvenance,
    full_provenance: &AreNativeRowEventEdgeProvenance,
) -> bool {
    row_provenance.source_record_index == full_provenance.source_record_index
        && row_provenance.source_point_index == full_provenance.source_point_index
        && row_provenance.contour_id == full_provenance.contour_id
        && row_provenance.segment_id == full_provenance.segment_id
        && row_provenance.edge_ordinal_within_record == full_provenance.edge_ordinal_within_record
}

#[cfg(test)]
fn command_stage_seed_order_4afc(edges: &[AreNativeRowEventEdgeInput]) -> Option<Vec<usize>> {
    complete_unique_seed_order_4afc(command_stage_raw_queue_4afc(edges), edges.len())
}

fn command_stage_raw_queue_4afc(edges: &[AreNativeRowEventEdgeInput]) -> Vec<usize> {
    if edges.is_empty() {
        return Vec::new();
    }

    let mut signs = edges
        .iter()
        .map(|edge| edge.edge.winding_delta)
        .collect::<Vec<_>>();
    let mut sibling_next = vec![None::<usize>; edges.len()];
    let mut raw_queue = Vec::new();
    let mut contour_ids = Vec::<usize>::new();
    for edge in edges {
        if !contour_ids.contains(&edge.contour_id) {
            contour_ids.push(edge.contour_id);
        }
    }

    for contour_id in contour_ids {
        let contour_edge_indices = edges
            .iter()
            .enumerate()
            .filter_map(|(edge_index, edge)| (edge.contour_id == contour_id).then_some(edge_index))
            .collect::<Vec<_>>();
        append_contour_command_stage_seed_4afc(
            &contour_edge_indices,
            edges,
            &mut signs,
            &mut sibling_next,
            &mut raw_queue,
        );
    }

    raw_queue
}

fn row_seed_order_covers_seedable_edges_71f4(
    row_y: i32,
    batch: &AreNativeRowEventEdgeBatch,
    row_seed_order: &[usize],
) -> bool {
    if row_seed_order.is_empty() {
        return false;
    }
    let mut seen = vec![false; batch.edges.len()];
    for edge_index in row_seed_order {
        if *edge_index >= batch.edges.len() || seen[*edge_index] {
            return false;
        }
        seen[*edge_index] = true;
    }
    batch.edges.iter().enumerate().all(|(edge_index, edge)| {
        !edge_seed_bucket_y_for_row_71f4(row_y, &edge.edge).is_some_and(|_| !seen[edge_index])
    })
}

fn row_seed_order_reaches_seedable_edges_71f4(
    row_y: i32,
    fill_rule: AreFillRule,
    batch: &AreNativeRowEventEdgeBatch,
    row_seed_order: &[usize],
) -> Result<bool, AreSamplerMaterializationError> {
    if row_seed_order.is_empty() {
        return Ok(false);
    }

    let mut reached = vec![false; batch.edges.len()];
    for edge_index in row_seed_order {
        if *edge_index >= batch.edges.len() || reached[*edge_index] {
            return Ok(false);
        }
        reached[*edge_index] = true;
    }

    let output = build_native_row_event_crossing_lists_for_row(
        &AreNativeRowEventInput::source_owned(row_y, fill_rule, batch.edges.clone())
            .with_seed_order_4afc(row_seed_order.to_vec()),
    )
    .map_err(AreSamplerMaterializationError::NativeRowEvent)?;

    for trace in &output.subrow_traces {
        for edge_index in trace
            .row_add_edge_indices
            .iter()
            .chain(&trace.bucket_insert_edge_indices)
            .chain(&trace.bucket_drain_edge_indices)
        {
            if let Some(reached_edge) = reached.get_mut(*edge_index) {
                *reached_edge = true;
            }
        }
    }

    Ok(batch.edges.iter().enumerate().all(|(edge_index, edge)| {
        !edge_seed_bucket_y_for_row_71f4(row_y, &edge.edge).is_some_and(|_| !reached[edge_index])
    }))
}

fn edge_seed_bucket_y_for_row_71f4(row_y: i32, edge: &AreActiveEdge) -> Option<i32> {
    let table_start = row_y * ARE_FIXED_SUBPIXEL_SCALE;
    let table_end = table_start + ARE_FIXED_SUBPIXEL_SCALE;
    let start_floor = edge.start_y_fixed.min(edge.end_y_fixed);
    let end_floor = edge.start_y_fixed.max(edge.end_y_fixed);

    if start_floor >= table_start {
        (start_floor < table_end).then_some(start_floor)
    } else if end_floor >= table_start {
        Some(table_start)
    } else {
        None
    }
}

fn append_contour_command_stage_seed_4afc(
    contour_edge_indices: &[usize],
    edges: &[AreNativeRowEventEdgeInput],
    signs: &mut [i8],
    sibling_next: &mut [Option<usize>],
    raw_queue: &mut Vec<usize>,
) {
    let mut first_edge_index = None;
    let mut last_edge_index = None;

    for current_index in contour_edge_indices.iter().copied() {
        if first_edge_index.is_none() {
            first_edge_index = Some(current_index);
            last_edge_index = Some(current_index);
            continue;
        }

        if let Some(previous_index) = last_edge_index {
            append_7348_edge_insert_seed(
                previous_index,
                current_index,
                signs,
                sibling_next,
                raw_queue,
            );
        }
        last_edge_index = Some(current_index);
    }

    if let (Some(first_index), Some(last_index)) = (first_edge_index, last_edge_index) {
        append_7d20_close_seed(
            first_index,
            last_index,
            edges,
            signs,
            sibling_next,
            raw_queue,
        );
    }
}

fn append_7348_edge_insert_seed(
    previous_index: usize,
    current_index: usize,
    signs: &mut [i8],
    sibling_next: &mut [Option<usize>],
    raw_queue: &mut Vec<usize>,
) {
    let previous_sign = signs[previous_index];
    let current_sign = signs[current_index];

    let link_sign = if previous_sign == 0 || previous_sign == current_sign {
        signs[previous_index] = current_sign;
        Some(current_sign)
    } else if current_sign != 0 {
        if previous_sign == 1 {
            raw_queue.push(previous_index);
            raw_queue.push(current_index);
        }
        None
    } else {
        signs[current_index] = previous_sign;
        Some(previous_sign)
    };

    if let Some(sign) = link_sign {
        if sign == 1 {
            sibling_next[current_index] = Some(previous_index);
        } else {
            sibling_next[previous_index] = Some(current_index);
        }
    }
}

fn append_7d20_close_seed(
    first_index: usize,
    last_index: usize,
    edges: &[AreNativeRowEventEdgeInput],
    signs: &mut [i8],
    sibling_next: &mut [Option<usize>],
    raw_queue: &mut Vec<usize>,
) {
    let first_sign = signs[first_index];
    let last_sign = signs[last_index];
    let mut first_rewrite_sign = 1;

    if last_sign != 1 || first_sign == 1 {
        if first_index == last_index {
            raw_queue.push(first_index);
            return;
        }
        if last_sign != first_sign {
            return;
        }
        if last_sign == 1 {
            sibling_next[first_index] = Some(last_index);
            return;
        }
        if sibling_next[first_index] != Some(last_index) {
            sibling_next[last_index] = Some(first_index);
            return;
        }
        if edge_endpoints_share_floor_y_7d20(&edges[last_index].edge) {
            signs[last_index] = 1;
            first_rewrite_sign = -1;
        }
        signs[first_index] = first_rewrite_sign;
        sibling_next[first_index] = None;
    }

    raw_queue.push(first_index);
    if first_index != last_index {
        raw_queue.push(last_index);
    }
}

fn edge_endpoints_share_floor_y_7d20(edge: &AreActiveEdge) -> bool {
    fixed_floor_pixel_7d20(edge.start_y_fixed) == fixed_floor_pixel_7d20(edge.end_y_fixed)
}

fn fixed_floor_pixel_7d20(value_fixed: i32) -> i32 {
    value_fixed.div_euclid(ARE_FIXED_SUBPIXEL_SCALE)
}

#[cfg(test)]
fn complete_unique_seed_order_4afc(raw_queue: Vec<usize>, edge_count: usize) -> Option<Vec<usize>> {
    if raw_queue.len() != edge_count {
        return None;
    }
    let mut seen = vec![false; edge_count];
    for edge_index in &raw_queue {
        if *edge_index >= edge_count || seen[*edge_index] {
            return None;
        }
        seen[*edge_index] = true;
    }
    seen.into_iter().all(|seen| seen).then_some(raw_queue)
}

fn native_row_event_edge_batch_from_record(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    record_index: usize,
    record: &AreSourceRecord32,
    origin_override: Option<AreNativeRowEventEdgeOrigin>,
) -> Result<AreNativeRowEventEdgeBatch, AreSamplerMaterializationError> {
    let segment = source_segment_detail(input, record_index, record)?;
    let origin = origin_override.unwrap_or_else(|| {
        if segment.kind.is_curve() {
            AreNativeRowEventEdgeOrigin::CurveFlattening
        } else {
            AreNativeRowEventEdgeOrigin::DirectRecord
        }
    });
    let mut batch = AreNativeRowEventEdgeBatch::empty();
    for (edge_ordinal_within_record, edge) in
        active_edges_from_record(input, row_y, run_index, record_index, record)?
            .into_iter()
            .enumerate()
    {
        batch.push(
            AreNativeRowEventEdgeInput::new(segment.contour_id.0, edge),
            AreNativeRowEventEdgeProvenance {
                source_record_index: record_index,
                source_point_index: segment.source_point_index,
                contour_id: segment.contour_id,
                segment_id: segment.segment_id,
                edge_ordinal_within_record,
                origin: origin.clone(),
                seed_4afc_ordinal: None,
            },
        );
    }
    Ok(batch)
}

fn native_debug_edges_from_record(
    input: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
    record_index: usize,
    record: &AreSourceRecord32,
) -> Result<Vec<AreNativeDebugEdgeInput>, AreSamplerMaterializationError> {
    let contour_id = source_segment_detail(input, record_index, record)?
        .contour_id
        .0;
    Ok(
        active_edges_from_record(input, row_y, run_index, record_index, record)?
            .into_iter()
            .map(|edge| AreNativeDebugEdgeInput::new(contour_id, edge))
            .collect(),
    )
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
            || !record_pair_recovery_active_on_row(
                input,
                record_segment,
                candidate_record,
                candidate_segment,
                row_y,
            )
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

fn record_pair_recovery_active_on_row(
    input: &AreSamplerMaterializerInput,
    record_segment: &AreSourceSegment,
    candidate_record: &AreSourceRecord32,
    candidate_segment: &AreSourceSegment,
    row_y: i32,
) -> bool {
    if record_active_on_row(candidate_record, row_y) {
        return true;
    }
    if !is_source_owned_glyph_path(input) {
        return false;
    }
    if !record_segment.kind.is_curve() || !candidate_segment.kind.is_curve() {
        return false;
    }
    if !segments_are_adjacent(record_segment, candidate_segment) {
        return false;
    }
    row_y == candidate_record.min_y_0x0c || row_y == candidate_record.max_y_0x14
}

fn is_source_owned_glyph_path(input: &AreSamplerMaterializerInput) -> bool {
    matches!(
        input.source_input.provenance,
        AreSourcePathProvenance::SourceOwnedGlyphPath
            | AreSourcePathProvenance::SourceOwnedGlyphRun
    )
}

fn segments_are_adjacent(left: &AreSourceSegment, right: &AreSourceSegment) -> bool {
    let left_id = left.segment_id.0;
    let right_id = right.segment_id.0;
    left_id.max(right_id) - left_id.min(right_id) == 1
}

fn to_fixed(value: f32) -> i32 {
    (value * crate::ARE_FIXED_SUBPIXEL_SCALE as f32).floor() as i32
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
        build_crossing_lists_for_row, build_descriptor_cursor_from_95cc_intervals_with_policy,
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
        input_with_manual_run_kind(
            source_input,
            row_y,
            current_x,
            next_x,
            source_record_indices,
            AreSamplerIntervalTag::SourceSpan,
            true,
        )
    }

    fn input_with_manual_run_kind(
        source_input: AreBezierSourcePathInput,
        row_y: i32,
        current_x: i32,
        next_x: i32,
        source_record_indices: Vec<usize>,
        tag: AreSamplerIntervalTag,
        materialize_candidate: bool,
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
                    tag,
                    source_record_indices,
                    materialize_candidate,
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

    fn intervals_for(
        materialized: &AreSamplerMaterialization,
        row_y: i32,
        run_index: usize,
    ) -> Vec<&AreMaterializedInterval> {
        materialized
            .intervals
            .iter()
            .filter(|interval| interval.row_y == row_y && interval.run_index == run_index)
            .collect()
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

    fn glyph_curve_boundary_pair_source() -> AreBezierSourcePathInput {
        let p0 = ArePathPoint::new(0.0, 23.0);
        let p1 = ArePathPoint::new(1.0, 19.0);
        let p2 = ArePathPoint::new(2.0, 10.0);
        let control_01 = ArePathPoint::new(0.25, 22.0);
        let control_02 = ArePathPoint::new(0.75, 20.0);
        let control_11 = ArePathPoint::new(1.25, 17.0);
        let control_12 = ArePathPoint::new(1.75, 12.0);

        AreBezierSourcePathInput::source_owned_glyph_path(
            vec![p0, p1, p2],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::CubicTo,
                ArePathVerb::CubicTo,
            ],
            crate::ArePathTransform::identity(),
        )
        .with_segments(vec![
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(0),
                source_point_index: 0,
                verb: ArePathVerb::MoveTo,
                previous_point: None,
                endpoint: p0,
                kind: AreSourceSegmentKind::Move,
                glyph_run_index: Some(0),
                glyph_id: Some(254),
                is_close_boundary: false,
            },
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(1),
                source_point_index: 1,
                verb: ArePathVerb::CubicTo,
                previous_point: Some(p0),
                endpoint: p1,
                kind: AreSourceSegmentKind::Cubic {
                    controls: Some(crate::AreCubicControl {
                        control_1: control_01,
                        control_2: control_02,
                    }),
                },
                glyph_run_index: Some(0),
                glyph_id: Some(254),
                is_close_boundary: false,
            },
            crate::AreSourceSegment {
                contour_id: crate::AreSourceContourId(0),
                segment_id: crate::AreSourceSegmentId(2),
                source_point_index: 2,
                verb: ArePathVerb::CubicTo,
                previous_point: Some(p1),
                endpoint: p2,
                kind: AreSourceSegmentKind::Cubic {
                    controls: Some(crate::AreCubicControl {
                        control_1: control_11,
                        control_2: control_12,
                    }),
                },
                glyph_run_index: Some(0),
                glyph_id: Some(254),
                is_close_boundary: false,
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
    fn multi_record_state_run_uses_shared_merge_state() {
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
        let mut input = input_with_manual_run_kind(
            source_input,
            3,
            0,
            5,
            vec![0, 1],
            AreSamplerIntervalTag::Sentinel,
            false,
        );
        for record in &mut input.working_set.records {
            record.record_flag_0x18 = AreSourceRecord32::SENTINEL_FLAG;
            record.merge_state_0x19 = 7;
        }

        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(3, 0).unwrap();

        assert_eq!(interval.source_record_indices, vec![0, 1]);
        assert_eq!(interval.payload, None);
        assert_eq!(
            interval.state_hint,
            Some(AreMaterializedStateHint {
                state_class: Are95ccStateClass::Class0,
                state: 7,
            })
        );
    }

    #[test]
    fn multi_record_state_run_rejects_conflicting_merge_state() {
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
        let mut input = input_with_manual_run_kind(
            source_input,
            3,
            0,
            5,
            vec![0, 1],
            AreSamplerIntervalTag::Sentinel,
            false,
        );
        input.working_set.records[0].record_flag_0x18 = AreSourceRecord32::SENTINEL_FLAG;
        input.working_set.records[0].merge_state_0x19 = 2;
        input.working_set.records[1].record_flag_0x18 = AreSourceRecord32::SENTINEL_FLAG;
        input.working_set.records[1].merge_state_0x19 = 3;

        assert_eq!(
            materialize_95cc_intervals(&input),
            Err(AreSamplerMaterializationError::UnsupportedMultiRecordRun {
                row_y: 3,
                run_index: 0,
                source_record_indices: vec![0, 1],
            })
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
        assert_eq!(interval.payload.as_ref().unwrap().bytes, vec![144]);
    }

    #[test]
    fn materializer_uses_crossing_list_for_diagonal_pair() {
        let input = input_from_source(&diagonal_pair_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(interval.current_x, 0);
        assert_eq!(interval.next_x, 2);
        assert_eq!(interval.source_record_indices, vec![0, 1]);
        assert_eq!(interval.payload.as_ref().unwrap().bytes, vec![136, 151]);
    }

    #[test]
    fn materializer_uses_nonzero_winding() {
        let input = input_from_source(&overlap_source());
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let intervals = intervals_for(&materialized, 0, 0);

        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals[0].payload.as_ref().unwrap().bytes, vec![192]);
        assert_eq!(intervals[0].current_x, 0);
        assert_eq!(intervals[0].next_x, 1);
        assert_eq!(
            intervals[1].state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
        assert_eq!(intervals[1].current_x, 1);
        assert_eq!(intervals[1].next_x, 3);
        assert_eq!(intervals[2].payload.as_ref().unwrap().bytes, vec![80]);
        assert_eq!(intervals[2].current_x, 3);
        assert_eq!(intervals[2].next_x, 4);
    }

    #[test]
    fn materializer_uses_evenodd_parity() {
        let input = input_from_source(&overlap_source()).with_fill_rule(AreFillRule::EvenOdd);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(0, 0).unwrap();

        assert_eq!(
            interval.payload.as_ref().unwrap().bytes,
            vec![192, 80, 192, 80]
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

        assert_eq!(accumulated.coverage.value_0x264, 144);
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
    fn native_row_event_edge_batch_preserves_direct_record_provenance() {
        let input = input_from_source(&vertical_pair_source());
        let run = &input.interval_list.row(4).unwrap().runs[0];
        let records = materializable_source_records(&input, 4, 0, run).unwrap();
        let batch = native_row_event_edge_batch_for_run(&input, 4, 0, run, &records).unwrap();

        assert_eq!(batch.edges.len(), 2);
        assert_eq!(batch.edge_provenance.len(), batch.edges.len());
        assert_eq!(batch.seed_order_4afc, Some(vec![0, 1]));
        assert_eq!(
            batch
                .edge_provenance
                .iter()
                .map(|provenance| provenance.source_record_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        for provenance in &batch.edge_provenance {
            assert_eq!(
                &provenance.origin,
                &AreNativeRowEventEdgeOrigin::DirectRecord
            );
            assert_eq!(provenance.edge_ordinal_within_record, 0);
            assert_eq!(
                provenance.seed_4afc_ordinal,
                Some(provenance.source_record_index)
            );
        }
    }

    #[test]
    fn native_row_event_edge_batch_marks_pair_recovery_provenance() {
        let input = input_with_manual_run(closed_rectangle_source(), 0, 0, 2, vec![0]);
        let run = &input.interval_list.row(0).unwrap().runs[0];
        let records = materializable_source_records(&input, 0, 0, run).unwrap();
        let batch = native_row_event_edge_batch_for_run(&input, 0, 0, run, &records).unwrap();

        assert_eq!(batch.edges.len(), 2);
        assert_eq!(batch.edge_provenance.len(), batch.edges.len());
        assert_eq!(batch.seed_order_4afc, Some(vec![0, 1]));
        assert!(batch.edge_provenance.iter().any(|provenance| {
            provenance.source_record_index == 0
                && provenance.origin == AreNativeRowEventEdgeOrigin::DirectRecord
                && provenance.seed_4afc_ordinal == Some(0)
        }));
        assert!(batch.edge_provenance.iter().any(|provenance| {
            matches!(
                &provenance.origin,
                AreNativeRowEventEdgeOrigin::EdgePairRecovery {
                    requested_record_index: 0
                }
            ) && provenance.source_record_index != 0
                && provenance.seed_4afc_ordinal == Some(1)
        }));
    }

    #[test]
    fn native_row_event_edge_batch_marks_curve_flattening_provenance() {
        let input = input_with_manual_run(quadratic_cap_source(), 0, 0, 2, vec![0]);
        let run = &input.interval_list.row(0).unwrap().runs[0];
        let records = materializable_source_records(&input, 0, 0, run).unwrap();
        let batch = native_row_event_edge_batch_for_run(&input, 0, 0, run, &records).unwrap();

        assert_eq!(batch.edges.len(), 2);
        assert_eq!(batch.edge_provenance.len(), batch.edges.len());
        assert_eq!(batch.seed_order_4afc, Some(vec![0, 1]));
        assert_eq!(
            batch
                .edge_provenance
                .iter()
                .map(|provenance| provenance.edge_ordinal_within_record)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(batch.edge_provenance.iter().all(|provenance| {
            provenance.source_record_index == 0
                && provenance.origin == AreNativeRowEventEdgeOrigin::CurveFlattening
                && provenance.seed_4afc_ordinal.is_some()
        }));
    }

    #[test]
    fn command_stage_seed_order_ports_7348_conflict_append() {
        let edges = vec![
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(16, 16, 16, 0, 1)),
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 0, 0, 16, -1)),
        ];

        assert_eq!(command_stage_seed_order_4afc(&edges), Some(vec![0, 1]));
    }

    #[test]
    fn command_stage_seed_order_ports_7d20_close_append() {
        let edges = vec![
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 0, 0, 16, -1)),
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(16, 16, 16, 0, 1)),
        ];

        assert_eq!(command_stage_seed_order_4afc(&edges), Some(vec![0, 1]));
    }

    #[test]
    fn command_stage_seed_order_stays_gated_for_incomplete_4afc_queue() {
        let edges = vec![
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 16, 0, 0, 1)),
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(16, 16, 16, 0, 1)),
        ];

        assert_eq!(command_stage_seed_order_4afc(&edges), None);
    }

    #[test]
    fn command_stage_raw_queue_can_reach_middle_edge_through_71f4_sibling_walk() {
        let edges = vec![
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 16, 0, 0, -1)),
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(4, 16, 8, 0, -1)),
            AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(16, 0, 16, 16, 1)),
        ];
        let mut batch = AreNativeRowEventEdgeBatch::empty();
        for (edge_index, edge) in edges.iter().cloned().enumerate() {
            batch.push(
                edge,
                AreNativeRowEventEdgeProvenance {
                    source_record_index: edge_index,
                    source_point_index: edge_index,
                    contour_id: AreSourceContourId(0),
                    segment_id: AreSourceSegmentId(edge_index),
                    edge_ordinal_within_record: 0,
                    origin: AreNativeRowEventEdgeOrigin::DirectRecord,
                    seed_4afc_ordinal: None,
                },
            );
        }

        let raw_queue = command_stage_raw_queue_4afc(&batch.edges);

        assert_eq!(raw_queue, vec![0, 2]);
        assert!(!row_seed_order_covers_seedable_edges_71f4(
            0, &batch, &raw_queue
        ));
        assert!(row_seed_order_reaches_seedable_edges_71f4(
            0,
            AreFillRule::NonZeroWinding,
            &batch,
            &raw_queue
        )
        .unwrap());
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
        let intervals = intervals_for(&materialized, 0, 0);

        assert_eq!(intervals.len(), 3);
        assert!(intervals
            .iter()
            .all(|interval| interval.source_record_indices == vec![0]));
        assert_eq!(
            intervals[0].state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
        assert_eq!(intervals[1].payload.as_ref().unwrap().bytes, vec![16]);
        assert_eq!(
            intervals[2].state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
    }

    #[test]
    fn glyph_boundary_pair_recovery_includes_adjacent_curve_on_terminal_row() {
        let input = input_with_manual_run(glyph_curve_boundary_pair_source(), 19, 0, 2, vec![0]);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let interval = materialized.interval(19, 0).unwrap();

        assert_eq!(interval.source_record_indices, vec![0]);
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
    fn mixed_full_and_partial_coverage_splits_class1_spans() {
        let input = input_with_manual_run(alternating_vertical_edges_source(), 0, 0, 3, vec![0]);
        let materialized = materialize_95cc_intervals(&input).unwrap();
        let intervals = intervals_for(&materialized, 0, 0);

        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals[0].current_x, 0);
        assert_eq!(intervals[0].next_x, 1);
        assert_eq!(
            intervals[0].state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
        assert_eq!(intervals[1].current_x, 1);
        assert_eq!(intervals[1].next_x, 2);
        assert_eq!(intervals[1].payload.as_ref().unwrap().bytes, vec![16]);
        assert_eq!(intervals[2].current_x, 2);
        assert_eq!(intervals[2].next_x, 3);
        assert_eq!(
            intervals[2].state_hint.unwrap().state_class,
            Are95ccStateClass::Class1
        );
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
            vec![144]
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
