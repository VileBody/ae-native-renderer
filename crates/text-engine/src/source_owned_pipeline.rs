use crate::{
    build_95cc_interval_list, build_e854_working_set, descriptor_builder_to_event_stream,
    emit_ad68_rows, Ad68EmitError, Ad68RowTable, AreBezierSourcePathInput,
    AreDescriptorBuilderRoot, AreDescriptorCursorEvent, AreDescriptorCursorInput,
    AreDescriptorCursorRow, AreDescriptorRef, AreDescriptorTriplet, AreEventDescriptorSourceObject,
    AreEventStream, AreIntervalBuildError, AreSamplerIntervalList, AreSamplerIntervalRun,
    AreSamplerIntervalTag, AreSourcePathProvenance, AreSourceSamplerError,
    AreSourceSamplerWorkingSet, DescriptorId, DescriptorToEventStreamError,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Are95ccDescriptorCursorBridge {
    pub builder: AreDescriptorBuilderRoot,
    pub cursor_input: AreDescriptorCursorInput,
    pub descriptor_refs: Vec<AreDescriptorRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorCursorPolicyInput {
    pub interval_list: AreSamplerIntervalList,
    pub policy: AreDescriptorCursorPolicy,
}

impl AreDescriptorCursorPolicyInput {
    pub fn new(interval_list: AreSamplerIntervalList, policy: AreDescriptorCursorPolicy) -> Self {
        Self {
            interval_list,
            policy,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorCursorPolicy {
    pub provenance: AreDescriptorCursorPolicyProvenance,
    pub synthetic_options: Are95ccDescriptorCursorOptions,
    pub interval_policies: Vec<AreIntervalDescriptorPolicy>,
}

pub type AreFixtureBackedDescriptorPolicy = AreDescriptorCursorPolicy;

impl AreDescriptorCursorPolicy {
    pub fn synthetic_only(options: Are95ccDescriptorCursorOptions) -> Self {
        Self {
            provenance: AreDescriptorCursorPolicyProvenance::SyntheticOnly,
            synthetic_options: options,
            interval_policies: Vec::new(),
        }
    }

    pub fn fixture_backed(interval_policies: Vec<AreIntervalDescriptorPolicy>) -> Self {
        Self {
            provenance: AreDescriptorCursorPolicyProvenance::FixtureBacked,
            synthetic_options: Are95ccDescriptorCursorOptions::default(),
            interval_policies,
        }
    }

    pub fn source_owned_materialized(interval_policies: Vec<AreIntervalDescriptorPolicy>) -> Self {
        Self {
            provenance: AreDescriptorCursorPolicyProvenance::SourceOwnedMaterialized,
            synthetic_options: Are95ccDescriptorCursorOptions::default(),
            interval_policies,
        }
    }

    fn interval_policy(
        &self,
        row_y: i32,
        run_index: usize,
    ) -> Option<&AreIntervalDescriptorPolicy> {
        self.interval_policies
            .iter()
            .find(|policy| policy.row_y == row_y && policy.run_index == run_index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreDescriptorCursorPolicyProvenance {
    FixtureBacked,
    SourceOwnedMaterialized,
    SyntheticOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreIntervalDescriptorPolicy {
    pub row_y: i32,
    pub run_index: usize,
    pub descriptor_triplet: Option<AreDescriptorTriplet>,
    pub source_probe: Option<usize>,
    pub payload: Option<AreIntervalPayloadPolicy>,
    pub state: Option<AreStatePolicy>,
}

impl AreIntervalDescriptorPolicy {
    pub fn class2_payload(
        row_y: i32,
        run_index: usize,
        descriptor_triplet: AreDescriptorTriplet,
        source_probe: usize,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            row_y,
            run_index,
            descriptor_triplet: Some(descriptor_triplet),
            source_probe: Some(source_probe),
            payload: Some(AreIntervalPayloadPolicy::bytes(bytes)),
            state: None,
        }
    }

    pub fn state(
        row_y: i32,
        run_index: usize,
        descriptor_triplet: AreDescriptorTriplet,
        source_probe: usize,
        state_class: Are95ccStateClass,
        state: u8,
    ) -> Self {
        Self {
            row_y,
            run_index,
            descriptor_triplet: Some(descriptor_triplet),
            source_probe: Some(source_probe),
            payload: None,
            state: Some(AreStatePolicy::new(state_class, state)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreIntervalPayloadPolicy {
    pub bytes: Vec<u8>,
}

impl AreIntervalPayloadPolicy {
    pub fn bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreStatePolicy {
    pub state_class: Are95ccStateClass,
    pub state: u8,
}

impl AreStatePolicy {
    pub fn new(state_class: Are95ccStateClass, state: u8) -> Self {
        Self { state_class, state }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Are95ccDescriptorCursorOptions {
    pub sentinel_state_class: Are95ccStateClass,
    pub class0_state: u8,
    pub class1_state: u8,
}

impl Default for Are95ccDescriptorCursorOptions {
    fn default() -> Self {
        Self {
            sentinel_state_class: Are95ccStateClass::Class0,
            class0_state: 0x10,
            class1_state: 0x20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Are95ccStateClass {
    Class0,
    Class1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Are95ccDescriptorCursorError {
    TypedSpanFallbackRejected,
    EmptyIntervalList,
    EmptyIntervalRun {
        row_y: i32,
        current_x: i32,
        next_x: i32,
    },
    UnsupportedEmptyIntervalAsEvent {
        row_y: i32,
    },
    MissingFixturePolicy {
        row_y: i32,
        run_index: usize,
    },
    MissingFixtureDescriptorTriplet {
        row_y: i32,
        run_index: usize,
    },
    MissingFixtureSourceProbe {
        row_y: i32,
        run_index: usize,
    },
    MissingFixturePayload {
        row_y: i32,
        run_index: usize,
    },
    MissingFixtureStatePolicy {
        row_y: i32,
        run_index: usize,
    },
    FixturePayloadShorterThanRun {
        row_y: i32,
        run_index: usize,
        payload_len: usize,
        run_width: usize,
    },
    SyntheticPolicyRejectedForFixture,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreSourceOwnedPipelineError {
    Source(AreSourceSamplerError),
    Interval(AreIntervalBuildError),
    DescriptorCursor(Are95ccDescriptorCursorError),
    EventStream(DescriptorToEventStreamError),
    Ad68(Ad68EmitError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AreSourceOwnedSyntheticPipeline {
    pub working_set: AreSourceSamplerWorkingSet,
    pub interval_list: AreSamplerIntervalList,
    pub descriptor_cursor: Are95ccDescriptorCursorBridge,
    pub event_stream: AreEventStream,
    pub row_table: Ad68RowTable,
}

pub fn build_descriptor_cursor_from_95cc_intervals(
    interval_list: &AreSamplerIntervalList,
) -> Result<Are95ccDescriptorCursorBridge, Are95ccDescriptorCursorError> {
    build_descriptor_cursor_from_95cc_intervals_with_options(
        interval_list,
        Are95ccDescriptorCursorOptions::default(),
    )
}

pub fn build_descriptor_cursor_from_95cc_intervals_with_options(
    interval_list: &AreSamplerIntervalList,
    options: Are95ccDescriptorCursorOptions,
) -> Result<Are95ccDescriptorCursorBridge, Are95ccDescriptorCursorError> {
    build_descriptor_cursor_from_95cc_intervals_with_policy(
        interval_list,
        &AreDescriptorCursorPolicy::synthetic_only(options),
    )
}

pub fn build_descriptor_cursor_from_95cc_intervals_with_policy(
    interval_list: &AreSamplerIntervalList,
    policy: &AreDescriptorCursorPolicy,
) -> Result<Are95ccDescriptorCursorBridge, Are95ccDescriptorCursorError> {
    if interval_list.provenance == AreSourcePathProvenance::TypedSpanFallback {
        return Err(Are95ccDescriptorCursorError::TypedSpanFallbackRejected);
    }
    if interval_list.rows.is_empty() {
        return Err(Are95ccDescriptorCursorError::EmptyIntervalList);
    }

    let mut builder = AreDescriptorBuilderRoot::new();
    let mut rows = Vec::new();

    for row in &interval_list.rows {
        let mut events = Vec::new();
        for (run_index, run) in row.runs.iter().enumerate() {
            if run.width() <= 0 {
                return Err(Are95ccDescriptorCursorError::EmptyIntervalRun {
                    row_y: row.row_y,
                    current_x: run.current_x,
                    next_x: run.next_x,
                });
            }
            let (descriptor_id, materialize_flag) =
                define_descriptor_for_run(&mut builder, row.row_y, run_index, run, policy)?;
            events.push(AreDescriptorCursorEvent::new(
                run.current_x,
                run.next_x,
                descriptor_id,
                materialize_flag,
            ));
        }
        if row.is_empty_sentinel && events.is_empty() {
            rows.push(AreDescriptorCursorRow::new(row.row_y, Vec::new()));
        } else {
            rows.push(AreDescriptorCursorRow::new(row.row_y, events));
        }
    }

    let y_min = rows
        .first()
        .ok_or(Are95ccDescriptorCursorError::EmptyIntervalList)?
        .row_y;
    let y_max = rows
        .last()
        .ok_or(Are95ccDescriptorCursorError::EmptyIntervalList)?
        .row_y;
    let descriptor_refs = builder.descriptor_vector.elements.clone();

    Ok(Are95ccDescriptorCursorBridge {
        builder,
        cursor_input: AreDescriptorCursorInput::new(y_min, y_max, rows),
        descriptor_refs,
    })
}

pub fn build_descriptor_cursor_from_95cc_intervals_with_fixture_policy(
    interval_list: &AreSamplerIntervalList,
    policy: &AreFixtureBackedDescriptorPolicy,
) -> Result<Are95ccDescriptorCursorBridge, Are95ccDescriptorCursorError> {
    if policy.provenance != AreDescriptorCursorPolicyProvenance::FixtureBacked {
        return Err(Are95ccDescriptorCursorError::SyntheticPolicyRejectedForFixture);
    }
    build_descriptor_cursor_from_95cc_intervals_with_policy(interval_list, policy)
}

pub fn build_source_owned_synthetic_pipeline(
    input: &AreBezierSourcePathInput,
) -> Result<AreSourceOwnedSyntheticPipeline, AreSourceOwnedPipelineError> {
    let working_set = build_e854_working_set(input).map_err(AreSourceOwnedPipelineError::Source)?;
    let interval_list =
        build_95cc_interval_list(&working_set).map_err(AreSourceOwnedPipelineError::Interval)?;
    let descriptor_cursor = build_descriptor_cursor_from_95cc_intervals(&interval_list)
        .map_err(AreSourceOwnedPipelineError::DescriptorCursor)?;
    let event_stream = descriptor_builder_to_event_stream(
        &descriptor_cursor.builder,
        &descriptor_cursor.cursor_input,
    )
    .map_err(AreSourceOwnedPipelineError::EventStream)?;
    let row_table = emit_ad68_rows(&event_stream).map_err(AreSourceOwnedPipelineError::Ad68)?;

    Ok(AreSourceOwnedSyntheticPipeline {
        working_set,
        interval_list,
        descriptor_cursor,
        event_stream,
        row_table,
    })
}

fn define_descriptor_for_run(
    builder: &mut AreDescriptorBuilderRoot,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    policy: &AreDescriptorCursorPolicy,
) -> Result<(DescriptorId, bool), Are95ccDescriptorCursorError> {
    match policy.provenance {
        AreDescriptorCursorPolicyProvenance::SyntheticOnly => Ok(
            define_synthetic_descriptor_for_run(builder, row_y, run, policy.synthetic_options),
        ),
        AreDescriptorCursorPolicyProvenance::FixtureBacked
        | AreDescriptorCursorPolicyProvenance::SourceOwnedMaterialized => {
            define_fixture_backed_descriptor_for_run(builder, row_y, run_index, run, policy)
        }
    }
}

fn define_synthetic_descriptor_for_run(
    builder: &mut AreDescriptorBuilderRoot,
    row_y: i32,
    run: &AreSamplerIntervalRun,
    options: Are95ccDescriptorCursorOptions,
) -> (DescriptorId, bool) {
    if run.materialize_candidate && run.tag == AreSamplerIntervalTag::SourceSpan {
        let id = builder.define_descriptor(
            descriptor_triplet_for_run(run),
            AreEventDescriptorSourceObject::with_0x20(source_probe_for_run(run)),
        );
        builder.register_descriptor(id);
        let payload = payload_bytes_for_run(row_y, run);
        let window = builder.payload_allocator.allocate(payload);
        builder.set_descriptor_payload_window(id, window);
        return (id, true);
    }

    let (source_object, state) = match options.sentinel_state_class {
        Are95ccStateClass::Class0 => (
            AreEventDescriptorSourceObject::with_0x18(source_probe_for_run(run)),
            options.class0_state,
        ),
        Are95ccStateClass::Class1 => (
            AreEventDescriptorSourceObject::with_0x20(source_probe_for_run(run)),
            options.class1_state,
        ),
    };
    let id = builder.define_descriptor(descriptor_triplet_for_run(run), source_object);
    builder.register_descriptor(id);
    builder.set_descriptor_state(id, state);
    (id, false)
}

fn define_fixture_backed_descriptor_for_run(
    builder: &mut AreDescriptorBuilderRoot,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    policy: &AreDescriptorCursorPolicy,
) -> Result<(DescriptorId, bool), Are95ccDescriptorCursorError> {
    let interval_policy = policy
        .interval_policy(row_y, run_index)
        .ok_or(Are95ccDescriptorCursorError::MissingFixturePolicy { row_y, run_index })?;
    let triplet = interval_policy.descriptor_triplet.ok_or(
        Are95ccDescriptorCursorError::MissingFixtureDescriptorTriplet { row_y, run_index },
    )?;
    let source_probe = interval_policy
        .source_probe
        .ok_or(Are95ccDescriptorCursorError::MissingFixtureSourceProbe { row_y, run_index })?;

    if run.materialize_candidate && run.tag == AreSamplerIntervalTag::SourceSpan {
        let payload = interval_policy
            .payload
            .as_ref()
            .ok_or(Are95ccDescriptorCursorError::MissingFixturePayload { row_y, run_index })?;
        let run_width = run.width() as usize;
        if payload.bytes.len() < run_width {
            return Err(Are95ccDescriptorCursorError::FixturePayloadShorterThanRun {
                row_y,
                run_index,
                payload_len: payload.bytes.len(),
                run_width,
            });
        }

        let id = builder.define_descriptor(
            triplet,
            AreEventDescriptorSourceObject::with_0x20(source_probe),
        );
        builder.register_descriptor(id);
        let window = builder.payload_allocator.allocate(payload.bytes.clone());
        builder.set_descriptor_payload_window(id, window);
        return Ok((id, true));
    }

    let state_policy = interval_policy
        .state
        .ok_or(Are95ccDescriptorCursorError::MissingFixtureStatePolicy { row_y, run_index })?;
    let source_object = match state_policy.state_class {
        Are95ccStateClass::Class0 => AreEventDescriptorSourceObject::with_0x18(source_probe),
        Are95ccStateClass::Class1 => AreEventDescriptorSourceObject::with_0x20(source_probe),
    };
    let id = builder.define_descriptor(triplet, source_object);
    builder.register_descriptor(id);
    builder.set_descriptor_state(id, state_policy.state);
    Ok((id, false))
}

fn descriptor_triplet_for_run(run: &AreSamplerIntervalRun) -> AreDescriptorTriplet {
    let first_record = run.source_record_indices.first().copied().unwrap_or(0);
    let base = 0x9500 + first_record * 0x30;
    AreDescriptorTriplet {
        base,
        second: base + 0x08,
        source: base + 0x10,
    }
}

fn source_probe_for_run(run: &AreSamplerIntervalRun) -> usize {
    0x5000 + run.source_record_indices.first().copied().unwrap_or(0)
}

fn payload_bytes_for_run(row_y: i32, run: &AreSamplerIntervalRun) -> Vec<u8> {
    let width = run.width().max(0) as usize;
    let record_seed = run.source_record_indices.first().copied().unwrap_or(0) as i64;
    (0..width)
        .map(|offset| {
            let value = row_y as i64 + run.current_x as i64 + offset as i64 + record_seed;
            value.rem_euclid(256) as u8
        })
        .collect()
}

#[cfg(test)]
mod source_owned_pipeline_tests {
    use super::*;
    use crate::{
        build_95cc_interval_list_from_input, ArePathPoint, ArePathVerb,
        AreSamplerIntervalListInput, AreSourceBounds, AreSourceRecord32,
    };

    fn source(points: Vec<ArePathPoint>, verbs: Vec<ArePathVerb>) -> AreBezierSourcePathInput {
        AreBezierSourcePathInput::source_owned(points, verbs)
    }

    fn record(
        min_x: i32,
        min_y: i32,
        max_x: i32,
        max_y: i32,
        flag: u8,
        source_point_index: usize,
    ) -> AreSourceRecord32 {
        AreSourceRecord32 {
            source_point_index_0x00: source_point_index,
            min_x_0x08: min_x,
            min_y_0x0c: min_y,
            max_x_0x10: max_x,
            max_y_0x14: max_y,
            record_flag_0x18: flag,
            merge_state_0x19: 0,
        }
    }

    fn working_set(
        records: Vec<AreSourceRecord32>,
        bounds: AreSourceBounds,
    ) -> AreSourceSamplerWorkingSet {
        AreSourceSamplerWorkingSet {
            inline_base_scratch_0x17e: 0,
            state_flag_0x180: false,
            current_point_cursor_0x182: 0,
            current_record_cursor_0x184: records.len(),
            saved_point_cursor_0x186: 0,
            saved_record_cursor_0x188: 0,
            counter_state_0x18a: 0,
            event_type_class_0x18b: 0,
            vector_pool_root_0x18c: 0,
            record_vector_base_0x18e: 0,
            record_vector_capacity_0x196: records.len(),
            mode_flag_0x601: false,
            mode_subflag_0x602: false,
            bounds,
            records,
            source_segments: Vec::new(),
        }
    }

    fn bounds(x_min: i32, y_min: i32, x_max: i32, y_max: i32) -> AreSourceBounds {
        AreSourceBounds {
            x_min,
            y_min,
            x_max,
            y_max,
        }
    }

    fn triplet(base: usize) -> AreDescriptorTriplet {
        AreDescriptorTriplet {
            base,
            second: base + 0x08,
            source: base + 0x10,
        }
    }

    fn one_span_interval_list() -> AreSamplerIntervalList {
        build_95cc_interval_list(&working_set(
            vec![record(2, 4, 6, 5, AreSourceRecord32::NORMAL_FLAG, 0)],
            bounds(0, 4, 10, 5),
        ))
        .unwrap()
    }

    fn two_span_interval_list() -> AreSamplerIntervalList {
        build_95cc_interval_list(&working_set(
            vec![
                record(2, 4, 6, 5, AreSourceRecord32::NORMAL_FLAG, 0),
                record(8, 4, 10, 5, AreSourceRecord32::NORMAL_FLAG, 1),
            ],
            bounds(0, 4, 10, 5),
        ))
        .unwrap()
    }

    fn fixture_policy_two_spans() -> AreDescriptorCursorPolicy {
        AreDescriptorCursorPolicy::fixture_backed(vec![
            AreIntervalDescriptorPolicy::class2_payload(
                4,
                0,
                triplet(0x7100),
                0x8100,
                vec![31, 32, 33, 34],
            ),
            AreIntervalDescriptorPolicy::class2_payload(
                4,
                1,
                triplet(0x7200),
                0x8200,
                vec![51, 52],
            ),
        ])
    }

    #[test]
    fn source_owned_pipeline_single_source_path_span() {
        let pipeline = build_source_owned_synthetic_pipeline(&source(
            vec![ArePathPoint::new(1.0, 2.0), ArePathPoint::new(5.0, 2.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        ))
        .unwrap();

        let nodes = pipeline.row_table.row(2).unwrap();

        assert_eq!(pipeline.row_table.y_min, 2);
        assert_eq!(nodes[0].x, 1);
        assert_eq!(nodes[0].len, 4);
        assert_eq!(nodes[0].bytes(), Some(&[3, 4, 5, 6][..]));
    }

    #[test]
    fn source_owned_pipeline_two_intervals_in_one_row() {
        let pipeline = build_source_owned_synthetic_pipeline(&source(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(2.0, 0.0),
                ArePathPoint::new(5.0, 0.0),
                ArePathPoint::new(7.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        ))
        .unwrap();

        let nodes = pipeline.row_table.row(0).unwrap();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].x, 0);
        assert_eq!(nodes[0].len, 2);
        assert_eq!(nodes[1].x, 5);
        assert_eq!(nodes[1].len, 2);
    }

    #[test]
    fn source_owned_pipeline_multi_row_path() {
        let pipeline = build_source_owned_synthetic_pipeline(&source(
            vec![ArePathPoint::new(1.0, 0.0), ArePathPoint::new(5.0, 3.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        ))
        .unwrap();

        assert_eq!(pipeline.row_table.y_min, 0);
        assert_eq!(pipeline.row_table.y_max, 2);
        assert_eq!(pipeline.row_table.row(0).unwrap()[0].x, 1);
        assert_eq!(pipeline.row_table.row(1).unwrap()[0].len, 4);
        assert_eq!(
            pipeline.row_table.row(2).unwrap()[0].bytes().unwrap().len(),
            4
        );
    }

    #[test]
    fn source_owned_pipeline_gap_empty_row() {
        let interval_list = build_95cc_interval_list(&working_set(
            vec![
                record(0, 0, 2, 1, AreSourceRecord32::NORMAL_FLAG, 0),
                record(4, 3, 7, 4, AreSourceRecord32::NORMAL_FLAG, 1),
            ],
            bounds(0, 0, 7, 4),
        ))
        .unwrap();
        let bridge = build_descriptor_cursor_from_95cc_intervals(&interval_list).unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert_eq!(table.row(0).unwrap()[0].x, 0);
        assert!(table.row(1).unwrap().is_empty());
        assert!(table.row(2).unwrap().is_empty());
        assert_eq!(table.row(3).unwrap()[0].x, 4);
    }

    #[test]
    fn source_owned_pipeline_class0_state_transition() {
        let interval_list = build_95cc_interval_list(&working_set(
            vec![record(1, 0, 4, 1, AreSourceRecord32::SENTINEL_FLAG, 0)],
            bounds(0, 0, 5, 1),
        ))
        .unwrap();
        let bridge = build_descriptor_cursor_from_95cc_intervals(&interval_list).unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert_eq!(bridge.builder.ctx_08.descriptors.len(), 1);
        assert!(bridge.builder.ctx_58.descriptors.is_empty());
        assert_eq!(table.row(0).unwrap()[0].state, 0x10);
        assert_eq!(table.row(0).unwrap()[0].bytes, None);
    }

    #[test]
    fn source_owned_pipeline_class1_state_transition_if_requested() {
        let interval_list = build_95cc_interval_list(&working_set(
            vec![record(1, 0, 4, 1, AreSourceRecord32::SENTINEL_FLAG, 0)],
            bounds(0, 0, 5, 1),
        ))
        .unwrap();
        let bridge = build_descriptor_cursor_from_95cc_intervals_with_options(
            &interval_list,
            Are95ccDescriptorCursorOptions {
                sentinel_state_class: Are95ccStateClass::Class1,
                class0_state: 0x10,
                class1_state: 0x33,
            },
        )
        .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert!(bridge.builder.ctx_08.descriptors.is_empty());
        assert_eq!(bridge.builder.ctx_58.descriptors.len(), 1);
        assert_eq!(table.row(0).unwrap()[0].state, 0x33);
    }

    #[test]
    fn source_owned_pipeline_class2_payload_window_copies_exact_bytes() {
        let interval_list = build_95cc_interval_list(&working_set(
            vec![record(3, 4, 8, 5, AreSourceRecord32::NORMAL_FLAG, 2)],
            bounds(0, 4, 10, 5),
        ))
        .unwrap();
        let bridge = build_descriptor_cursor_from_95cc_intervals(&interval_list).unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert_eq!(
            table.row(4).unwrap()[0].bytes(),
            Some(&[7, 8, 9, 10, 11][..])
        );
    }

    #[test]
    fn source_owned_pipeline_edge_x_zero_and_width_minus_one() {
        let pipeline = build_source_owned_synthetic_pipeline(&source(
            vec![
                ArePathPoint::new(0.0, 1.0),
                ArePathPoint::new(1.0, 1.0),
                ArePathPoint::new(9.0, 1.0),
                ArePathPoint::new(10.0, 1.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        ))
        .unwrap();

        let nodes = pipeline.row_table.row(1).unwrap();

        assert_eq!(nodes[0].x, 0);
        assert_eq!(nodes[0].len, 1);
        assert_eq!(nodes[1].x, 9);
        assert_eq!(nodes[1].len, 1);
    }

    #[test]
    fn source_owned_pipeline_descriptor_refs_are_not_inline_records() {
        let pipeline = build_source_owned_synthetic_pipeline(&source(
            vec![ArePathPoint::new(2.0, 0.0), ArePathPoint::new(6.0, 0.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        ))
        .unwrap();

        let builder = &pipeline.descriptor_cursor.builder;

        assert_eq!(builder.descriptor_vector.element_size_bytes(), 8);
        assert_eq!(
            builder.descriptor_vector.elements,
            pipeline.descriptor_cursor.descriptor_refs
        );
        assert_eq!(
            builder.ctx_58.descriptors,
            pipeline.descriptor_cursor.descriptor_refs
        );
    }

    #[test]
    fn source_owned_pipeline_state_nodes_do_not_corrupt_class2_streams() {
        let interval_list = build_95cc_interval_list(&working_set(
            vec![
                record(1, 0, 4, 1, AreSourceRecord32::NORMAL_FLAG, 0),
                record(4, 0, 7, 1, AreSourceRecord32::SENTINEL_FLAG, 1),
                record(7, 0, 9, 1, AreSourceRecord32::NORMAL_FLAG, 2),
            ],
            bounds(0, 0, 10, 1),
        ))
        .unwrap();
        let bridge = build_descriptor_cursor_from_95cc_intervals(&interval_list).unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();
        let nodes = table.row(0).unwrap();

        assert_eq!(nodes[0].bytes(), Some(&[1, 2, 3][..]));
        assert_eq!(nodes[1].state, 0x10);
        assert_eq!(nodes[1].bytes, None);
        assert_eq!(nodes[2].bytes(), Some(&[9, 10][..]));
    }

    #[test]
    fn source_owned_pipeline_rejects_typed_span_fallback() {
        let input = AreBezierSourcePathInput::typed_span_fallback(
            vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(4.0, 0.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        );

        assert_eq!(
            build_source_owned_synthetic_pipeline(&input),
            Err(AreSourceOwnedPipelineError::Source(
                AreSourceSamplerError::TypedSpanFallbackRejected
            ))
        );
    }

    #[test]
    fn source_owned_pipeline_interval_bridge_rejects_typed_span_provenance() {
        let working_set = working_set(
            vec![record(0, 0, 2, 1, AreSourceRecord32::NORMAL_FLAG, 0)],
            bounds(0, 0, 4, 1),
        );
        let err = build_95cc_interval_list_from_input(
            &AreSamplerIntervalListInput::typed_span_fallback(working_set),
        )
        .unwrap_err();

        assert_eq!(err, AreIntervalBuildError::TypedSpanFallbackRejected);
    }

    #[test]
    fn fixture_backed_class2_payload_bytes_are_used() {
        let interval_list = one_span_interval_list();
        let policy = AreDescriptorCursorPolicy::fixture_backed(vec![
            AreIntervalDescriptorPolicy::class2_payload(
                4,
                0,
                triplet(0x7000),
                0x8000,
                vec![31, 32, 33, 34],
            ),
        ]);
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy)
                .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert_eq!(
            table.row(4).unwrap()[0].bytes(),
            Some(&[31, 32, 33, 34][..])
        );
    }

    #[test]
    fn fixture_backed_two_span_payloads_match() {
        let interval_list = two_span_interval_list();
        let policy = fixture_policy_two_spans();
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy)
                .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();
        let nodes = table.row(4).unwrap();

        assert_eq!(nodes[0].bytes(), Some(&[31, 32, 33, 34][..]));
        assert_eq!(nodes[1].bytes(), Some(&[51, 52][..]));
    }

    #[test]
    fn fixture_backed_descriptor_cursor_to_ad68_rows() {
        let interval_list = two_span_interval_list();
        let policy = fixture_policy_two_spans();
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy)
                .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();
        let nodes = table.row(4).unwrap();

        assert_eq!(nodes[0].x, 2);
        assert_eq!(nodes[0].len, 4);
        assert_eq!(nodes[0].bytes(), Some(&[31, 32, 33, 34][..]));
        assert_eq!(nodes[1].x, 8);
        assert_eq!(nodes[1].len, 2);
        assert_eq!(nodes[1].bytes(), Some(&[51, 52][..]));
    }

    #[test]
    fn fixture_backed_policy_rejects_missing_class2_payload() {
        let interval_list = one_span_interval_list();
        let policy = AreDescriptorCursorPolicy::fixture_backed(vec![AreIntervalDescriptorPolicy {
            row_y: 4,
            run_index: 0,
            descriptor_triplet: Some(triplet(0x7000)),
            source_probe: Some(0x8000),
            payload: None,
            state: None,
        }]);

        let err = build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy)
            .unwrap_err();

        assert_eq!(
            err,
            Are95ccDescriptorCursorError::MissingFixturePayload {
                row_y: 4,
                run_index: 0,
            }
        );
    }

    #[test]
    fn fixture_backed_policy_rejects_synthetic_fallback() {
        let interval_list = one_span_interval_list();
        let policy = AreDescriptorCursorPolicy::fixture_backed(vec![AreIntervalDescriptorPolicy {
            row_y: 4,
            run_index: 0,
            descriptor_triplet: Some(triplet(0x7000)),
            source_probe: Some(0x8000),
            payload: None,
            state: None,
        }]);

        assert!(matches!(
            build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy),
            Err(Are95ccDescriptorCursorError::MissingFixturePayload { .. })
        ));
    }

    #[test]
    fn synthetic_policy_still_available_for_synthetic_tests() {
        let interval_list = one_span_interval_list();
        let bridge = build_descriptor_cursor_from_95cc_intervals_with_policy(
            &interval_list,
            &AreDescriptorCursorPolicy::synthetic_only(Are95ccDescriptorCursorOptions::default()),
        )
        .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert_ne!(
            table.row(4).unwrap()[0].bytes(),
            Some(&[31, 32, 33, 34][..])
        );
    }

    #[test]
    fn class0_state_from_fixture_policy() {
        let interval_list = build_95cc_interval_list(&working_set(
            vec![record(1, 4, 3, 5, AreSourceRecord32::SENTINEL_FLAG, 0)],
            bounds(0, 4, 4, 5),
        ))
        .unwrap();
        let policy =
            AreDescriptorCursorPolicy::fixture_backed(vec![AreIntervalDescriptorPolicy::state(
                4,
                0,
                triplet(0x7300),
                0x8300,
                Are95ccStateClass::Class0,
                0x44,
            )]);
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy)
                .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert_eq!(bridge.builder.ctx_08.descriptors.len(), 1);
        assert!(bridge.builder.ctx_58.descriptors.is_empty());
        assert_eq!(table.row(4).unwrap()[0].state, 0x44);
    }

    #[test]
    fn class1_state_from_fixture_policy() {
        let interval_list = build_95cc_interval_list(&working_set(
            vec![record(1, 4, 3, 5, AreSourceRecord32::SENTINEL_FLAG, 0)],
            bounds(0, 4, 4, 5),
        ))
        .unwrap();
        let policy =
            AreDescriptorCursorPolicy::fixture_backed(vec![AreIntervalDescriptorPolicy::state(
                4,
                0,
                triplet(0x7400),
                0x8400,
                Are95ccStateClass::Class1,
                0x55,
            )]);
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy)
                .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let table = emit_ad68_rows(&event_stream).unwrap();

        assert!(bridge.builder.ctx_08.descriptors.is_empty());
        assert_eq!(bridge.builder.ctx_58.descriptors.len(), 1);
        assert_eq!(table.row(4).unwrap()[0].state, 0x55);
    }

    #[test]
    fn descriptor_triplets_from_fixture_policy_if_present() {
        let interval_list = one_span_interval_list();
        let expected = triplet(0x7700);
        let policy = AreDescriptorCursorPolicy::fixture_backed(vec![
            AreIntervalDescriptorPolicy::class2_payload(
                4,
                0,
                expected,
                0x8700,
                vec![31, 32, 33, 34],
            ),
        ]);
        let bridge =
            build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy)
                .unwrap();

        assert_eq!(bridge.builder.descriptors[0].triplet, expected);
    }
}
