use crate::{
    accumulate_75d0_for_sample_x, build_crossing_lists_for_row, AreActiveEdge,
    AreCrossingListError, AreFillRule, AreSourcePathProvenance, AreSubrowCrossingArray16,
    AreSubrowCrossingEntry, AreSubrowCrossingList, ARE_CROSSING_SENTINEL, ARE_FIXED_SUBPIXEL_SCALE,
    ARE_FULL_COVERAGE_0X264, ARE_SUBROW_COUNT,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventInput {
    pub provenance: AreSourcePathProvenance,
    pub row_y: i32,
    pub fill_rule: AreFillRule,
    pub options: AreNativeRowEventOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_order_4afc: Option<Vec<usize>>,
    pub edges: Vec<AreNativeRowEventEdgeInput>,
}

impl AreNativeRowEventInput {
    pub fn source_owned(
        row_y: i32,
        fill_rule: AreFillRule,
        edges: Vec<AreNativeRowEventEdgeInput>,
    ) -> Self {
        Self {
            provenance: AreSourcePathProvenance::SourceOwned,
            row_y,
            fill_rule,
            options: AreNativeRowEventOptions::for_row(row_y),
            seed_order_4afc: None,
            edges,
        }
    }

    pub fn source_owned_from_active_edges(
        row_y: i32,
        fill_rule: AreFillRule,
        edges: Vec<AreActiveEdge>,
    ) -> Self {
        Self::source_owned(
            row_y,
            fill_rule,
            edges
                .into_iter()
                .map(|edge| AreNativeRowEventEdgeInput::new(0, edge))
                .collect(),
        )
    }

    pub fn with_options(mut self, options: AreNativeRowEventOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_seed_order_4afc(mut self, seed_order_4afc: Vec<usize>) -> Self {
        self.seed_order_4afc = Some(seed_order_4afc);
        self
    }

    pub fn with_optional_seed_order_4afc(mut self, seed_order_4afc: Option<Vec<usize>>) -> Self {
        self.seed_order_4afc = seed_order_4afc;
        self
    }

    fn active_edges(&self) -> Vec<AreActiveEdge> {
        self.edges.iter().map(|edge| edge.edge.clone()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventEdgeInput {
    pub contour_id: usize,
    pub edge: AreActiveEdge,
}

impl AreNativeRowEventEdgeInput {
    pub fn new(contour_id: usize, edge: AreActiveEdge) -> Self {
        Self { contour_id, edge }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventOptions {
    pub row_base_y: i32,
    pub row_bucket_count: usize,
    pub x_origin_fixed: i32,
    pub bucket_policy: AreNativeRowBucketPolicy,
    pub emit_sentinel: bool,
}

impl AreNativeRowEventOptions {
    pub fn for_row(row_y: i32) -> Self {
        let row_base_y = row_y * ARE_FIXED_SUBPIXEL_SCALE;
        Self {
            row_base_y,
            row_bucket_count: ARE_SUBROW_COUNT,
            x_origin_fixed: 0,
            bucket_policy: AreNativeRowBucketPolicy::ResolvedP6OverscanCallbacks,
            emit_sentinel: true,
        }
    }

    pub fn with_x_origin_fixed(mut self, x_origin_fixed: i32) -> Self {
        self.x_origin_fixed = x_origin_fixed;
        self
    }

    pub fn with_bucket_table(mut self, row_base_y: i32, row_bucket_count: usize) -> Self {
        self.row_base_y = row_base_y;
        self.row_bucket_count = row_bucket_count;
        self
    }

    pub fn with_bucket_policy(mut self, bucket_policy: AreNativeRowBucketPolicy) -> Self {
        self.bucket_policy = bucket_policy;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreNativeRowBucketPolicy {
    ResolvedP6OverscanCallbacks,
    FallbackHeadPrependOnly,
}

impl AreNativeRowBucketPolicy {
    fn uses_resolved_726c_callbacks(self) -> bool {
        matches!(self, Self::ResolvedP6OverscanCallbacks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventCrossingOutput {
    pub crossing_lists: AreSubrowCrossingArray16,
    pub subrow_traces: Vec<AreNativeRowEventSubrowTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventComparison {
    pub flat_crossing_lists: AreSubrowCrossingArray16,
    pub native_row_event_crossing_lists: AreSubrowCrossingArray16,
    pub subrow_traces: Vec<AreNativeRowEventSubrowTrace>,
    pub differing_subrows: Vec<usize>,
    pub ordering_delta_subrows: Vec<usize>,
    pub event_rewind_subrows: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventSubrowTrace {
    pub subrow_index: usize,
    pub fixed_subrow_y: i32,
    pub contour_start_edge_indices: Vec<usize>,
    pub row_add_edge_indices: Vec<usize>,
    pub bucket_insert_edge_indices: Vec<usize>,
    pub bucket_drain_edge_indices: Vec<usize>,
    pub active_edge_indices: Vec<usize>,
    pub projected_edge_events: Vec<AreNativeProjectedEdgeTrace>,
    pub flat_sorted_edge_indices: Vec<usize>,
    pub event_vector_entries: Vec<i32>,
    pub event_vector_rewinds: Vec<AreNativeEventVectorRewind>,
}

#[derive(Debug, Clone)]
struct AreNativeRowSeedTrace {
    fixed_row_y: i32,
    row_add_edge_indices_by_subrow: Vec<Vec<usize>>,
    bucket_insert_edge_indices_by_subrow: Vec<Vec<usize>>,
}

impl AreNativeRowSeedTrace {
    fn new(fixed_row_y: i32) -> Self {
        Self {
            fixed_row_y,
            row_add_edge_indices_by_subrow: vec![Vec::new(); ARE_SUBROW_COUNT],
            bucket_insert_edge_indices_by_subrow: vec![Vec::new(); ARE_SUBROW_COUNT],
        }
    }

    fn record_row_add(&mut self, bucket_y: i32, input_index: usize) {
        if let Some(subrow_index) = self.subrow_index_for_bucket_y(bucket_y) {
            self.row_add_edge_indices_by_subrow[subrow_index].push(input_index);
        }
    }

    fn record_bucket_insert(&mut self, bucket_y: i32, input_index: usize) {
        if let Some(subrow_index) = self.subrow_index_for_bucket_y(bucket_y) {
            self.bucket_insert_edge_indices_by_subrow[subrow_index].push(input_index);
        }
    }

    fn row_add_edge_indices(&self, subrow_index: usize) -> Vec<usize> {
        self.row_add_edge_indices_by_subrow
            .get(subrow_index)
            .cloned()
            .unwrap_or_default()
    }

    fn bucket_insert_edge_indices(&self, subrow_index: usize) -> Vec<usize> {
        self.bucket_insert_edge_indices_by_subrow
            .get(subrow_index)
            .cloned()
            .unwrap_or_default()
    }

    fn subrow_index_for_bucket_y(&self, bucket_y: i32) -> Option<usize> {
        let index = bucket_y - self.fixed_row_y;
        (0..ARE_SUBROW_COUNT as i32)
            .contains(&index)
            .then_some(index as usize)
    }
}

impl AreNativeRowEventSubrowTrace {
    pub fn has_order_delta(&self) -> bool {
        self.active_edge_indices != self.flat_sorted_edge_indices
    }

    pub fn has_event_rewind(&self) -> bool {
        !self.event_vector_rewinds.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeEventVectorRewind {
    pub requested_start_fixed: i32,
    pub previous_cursor: usize,
    pub selected_cursor: usize,
    pub cursor_distance: usize,
    pub removed_entries: Vec<i32>,
    pub merged_close_fixed: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeProjectedEdgeTrace {
    pub input_index: usize,
    pub contour_id: usize,
    pub x_min_fixed: i32,
    pub x_max_fixed: i32,
    pub winding_delta: i8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventSpanInput {
    pub row_y: i32,
    pub current_x: i32,
    pub next_x: i32,
    pub fill_rule: AreFillRule,
    pub options: AreNativeRowEventOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_order_4afc: Option<Vec<usize>>,
    pub edges: Vec<AreNativeRowEventEdgeInput>,
}

impl AreNativeRowEventSpanInput {
    pub fn source_owned(
        row_y: i32,
        current_x: i32,
        next_x: i32,
        fill_rule: AreFillRule,
        edges: Vec<AreNativeRowEventEdgeInput>,
    ) -> Self {
        Self {
            row_y,
            current_x,
            next_x,
            fill_rule,
            options: AreNativeRowEventOptions::for_row(row_y),
            seed_order_4afc: None,
            edges,
        }
    }

    pub fn with_seed_order_4afc(mut self, seed_order_4afc: Vec<usize>) -> Self {
        self.seed_order_4afc = Some(seed_order_4afc);
        self
    }

    fn row_input(&self) -> AreNativeRowEventInput {
        AreNativeRowEventInput::source_owned(self.row_y, self.fill_rule, self.edges.clone())
            .with_options(self.options)
            .with_optional_seed_order_4afc(self.seed_order_4afc.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventMaterialization {
    pub span: AreNativeRowEventMaterializedSpan,
    pub coverages_0x264: Vec<u16>,
    pub crossing_output: AreNativeRowEventCrossingOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreNativeRowEventMaterializedSpan {
    State {
        class: AreNativeRowEventStateClass,
        state: u8,
    },
    Payload(Vec<u8>),
}

impl AreNativeRowEventMaterializedSpan {
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::State {
                class: AreNativeRowEventStateClass::Class0,
                ..
            } => "class0",
            Self::State {
                class: AreNativeRowEventStateClass::Class1,
                ..
            } => "class1",
            Self::Payload(_) => "class2",
        }
    }

    pub fn payload_len(&self) -> Option<usize> {
        match self {
            Self::State { .. } => None,
            Self::Payload(bytes) => Some(bytes.len()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreNativeRowEventStateClass {
    Class0,
    Class1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreNativeRowEventError {
    TypedSpanFallbackRejected,
    CoverageRowFallbackRejected,
    RequiresSourceOwnedProvenance {
        provenance: AreSourcePathProvenance,
    },
    EmptyEdges,
    ZeroWindingEdge,
    InvalidBucketTable {
        row_bucket_count: usize,
    },
    RowOutsideBucketTable {
        row_y: i32,
        row_base_y: i32,
        row_bucket_count: usize,
    },
    InvalidSeedOrderIndex {
        seed_index: usize,
        edge_count: usize,
    },
    MissingProjection {
        edge_index: usize,
        fixed_subrow_y: i32,
    },
    GuardLimitExceeded {
        phase: AreNativeRowEventPhase,
        edge_count: usize,
    },
    InvalidSpanWidth {
        row_y: i32,
        current_x: i32,
        next_x: i32,
    },
    CrossingList(AreCrossingListError),
}

impl From<AreCrossingListError> for AreNativeRowEventError {
    fn from(error: AreCrossingListError) -> Self {
        Self::CrossingList(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreNativeRowEventPhase {
    ContourStaging7348,
    RowAdd71f4,
    BucketInsert726c,
    BucketDrainB944,
    EventEmit430c,
}

pub fn build_native_row_event_crossing_lists_for_row(
    input: &AreNativeRowEventInput,
) -> Result<AreNativeRowEventCrossingOutput, AreNativeRowEventError> {
    validate_native_row_event_input(input)?;

    let staged = stage_contour_sibling_links_7348(&input.edges)?;
    let mut nodes = staged.nodes.clone();
    let mut context = AreNativeRowEventContext::new(input.row_y, input.options);
    let fixed_row_y = input.row_y * ARE_FIXED_SUBPIXEL_SCALE;
    let mut seed_trace = AreNativeRowSeedTrace::new(fixed_row_y);

    if let Some(seed_order_4afc) = &input.seed_order_4afc {
        add_seed_order_to_row_71f4(
            seed_order_4afc,
            &mut nodes,
            &mut context,
            input.options.bucket_policy,
            &mut seed_trace,
        )?;
    } else {
        for contour in &staged.contours {
            add_contour_chain_to_row_71f4(
                contour,
                &mut nodes,
                &mut context,
                input.options.bucket_policy,
                &mut seed_trace,
            )?;
        }
    }

    let mut lists = Vec::with_capacity(ARE_SUBROW_COUNT);
    let mut subrow_traces = Vec::with_capacity(ARE_SUBROW_COUNT);

    for subrow_index in 0..ARE_SUBROW_COUNT {
        let fixed_subrow_y = fixed_row_y + subrow_index as i32;
        let (event_vector, trace) = produce_subrow_events(
            fixed_subrow_y,
            subrow_index,
            &staged,
            &mut nodes,
            &mut context,
            input.fill_rule,
            &seed_trace,
        )?;
        let mut entries = event_vector.entries;
        if input.options.emit_sentinel {
            entries.push(ARE_CROSSING_SENTINEL);
        }
        lists.push(AreSubrowCrossingList {
            subrow_index,
            fixed_subrow_y,
            entries: entries
                .into_iter()
                .map(AreSubrowCrossingEntry::new)
                .collect(),
        });
        subrow_traces.push(trace);
    }

    Ok(AreNativeRowEventCrossingOutput {
        crossing_lists: AreSubrowCrossingArray16 {
            row_y: input.row_y,
            fill_rule: input.fill_rule,
            lists,
        },
        subrow_traces,
    })
}

pub fn build_native_row_event_crossing_lists_for_row_from_edges(
    row_y: i32,
    edges: &[AreActiveEdge],
    fill_rule: AreFillRule,
) -> Result<AreSubrowCrossingArray16, AreNativeRowEventError> {
    Ok(build_native_row_event_crossing_lists_for_row(
        &AreNativeRowEventInput::source_owned_from_active_edges(row_y, fill_rule, edges.to_vec()),
    )?
    .crossing_lists)
}

pub fn compare_native_row_event_and_flat_crossing_lists_for_row(
    input: &AreNativeRowEventInput,
) -> Result<AreNativeRowEventComparison, AreNativeRowEventError> {
    let native_row_event = build_native_row_event_crossing_lists_for_row(input)?;
    let flat_crossing_lists =
        build_crossing_lists_for_row(input.row_y, &input.active_edges(), input.fill_rule)
            .map_err(AreNativeRowEventError::CrossingList)?;
    let differing_subrows = native_row_event
        .crossing_lists
        .lists
        .iter()
        .zip(&flat_crossing_lists.lists)
        .filter_map(|(native, flat)| {
            (native.crossing_values() != flat.crossing_values()).then_some(native.subrow_index)
        })
        .collect();
    let ordering_delta_subrows = native_row_event
        .subrow_traces
        .iter()
        .filter_map(|trace| trace.has_order_delta().then_some(trace.subrow_index))
        .collect();
    let event_rewind_subrows = native_row_event
        .subrow_traces
        .iter()
        .filter_map(|trace| trace.has_event_rewind().then_some(trace.subrow_index))
        .collect();

    Ok(AreNativeRowEventComparison {
        flat_crossing_lists,
        native_row_event_crossing_lists: native_row_event.crossing_lists,
        subrow_traces: native_row_event.subrow_traces,
        differing_subrows,
        ordering_delta_subrows,
        event_rewind_subrows,
    })
}

pub fn materialize_source_span_with_native_row_events(
    input: &AreNativeRowEventSpanInput,
) -> Result<AreNativeRowEventMaterializedSpan, AreNativeRowEventError> {
    Ok(materialize_source_span_with_native_row_events_with_trace(input)?.span)
}

pub fn materialize_source_span_with_native_row_events_with_trace(
    input: &AreNativeRowEventSpanInput,
) -> Result<AreNativeRowEventMaterialization, AreNativeRowEventError> {
    if input.next_x <= input.current_x {
        return Err(AreNativeRowEventError::InvalidSpanWidth {
            row_y: input.row_y,
            current_x: input.current_x,
            next_x: input.next_x,
        });
    }

    let crossing_output = build_native_row_event_crossing_lists_for_row(&input.row_input())?;
    let mut coverages_0x264 = Vec::with_capacity((input.next_x - input.current_x) as usize);
    for sample_x in input.current_x..input.next_x {
        let coverage = accumulate_75d0_for_sample_x(&crossing_output.crossing_lists, sample_x)
            .map_err(AreNativeRowEventError::CrossingList)?
            .coverage
            .value_0x264;
        coverages_0x264.push(coverage);
    }

    let span = classify_native_row_event_coverages(&coverages_0x264);
    Ok(AreNativeRowEventMaterialization {
        span,
        coverages_0x264,
        crossing_output,
    })
}

#[derive(Debug, Clone)]
pub struct AreNativeContourStaging {
    pub nodes: Vec<AreNativeEdgeNode>,
    pub contours: Vec<AreNativeContourChain>,
}

#[derive(Debug, Clone)]
pub struct AreNativeContourChain {
    pub contour_id: usize,
    pub first_edge_index: usize,
    pub last_edge_index: usize,
    pub edge_indices: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct AreNativeEdgeNode {
    pub contour_id: usize,
    pub input_index: usize,
    pub row_next: Option<usize>,
    pub active_next: Option<usize>,
    pub active_prev: Option<usize>,
    pub sibling_next: Option<usize>,
    pub start_x_fixed: i32,
    pub start_y_fixed: i32,
    pub end_x_fixed: i32,
    pub end_y_fixed: i32,
    pub projected_x_min_fixed: Option<i32>,
    pub projected_x_max_fixed: Option<i32>,
    pub projection_fixed_subrow_y: Option<i32>,
    pub projection_dirty: bool,
    pub projection_override_active: bool,
    pub row_active_temp: bool,
    pub winding_delta: i8,
    pub slope_sentinel_0x34: i32,
}

impl AreNativeEdgeNode {
    fn from_input(input_index: usize, input: &AreNativeRowEventEdgeInput) -> Self {
        Self {
            contour_id: input.contour_id,
            input_index,
            row_next: None,
            active_next: None,
            active_prev: None,
            sibling_next: None,
            start_x_fixed: input.edge.start_x_fixed,
            start_y_fixed: input.edge.start_y_fixed,
            end_x_fixed: input.edge.end_x_fixed,
            end_y_fixed: input.edge.end_y_fixed,
            projected_x_min_fixed: None,
            projected_x_max_fixed: None,
            projection_fixed_subrow_y: None,
            projection_dirty: true,
            projection_override_active: false,
            row_active_temp: false,
            winding_delta: input.edge.winding_delta,
            slope_sentinel_0x34: 0,
        }
    }

    fn active_edge(&self) -> AreActiveEdge {
        AreActiveEdge::line_fixed(
            self.start_x_fixed,
            self.start_y_fixed,
            self.end_x_fixed,
            self.end_y_fixed,
            self.winding_delta,
        )
    }
}

#[derive(Debug, Clone)]
pub struct AreNativeRowEventContext {
    pub row_buckets: AreNativeRowBuckets,
    pub active_list: AreNativeActiveList,
    pub prepared_row_y: i32,
    pub guard_counter: usize,
    pub x_origin_fixed: i32,
    pub event_vector: AreNativeRowEventVector,
    pub coverage_cache_valid_0x260: bool,
    pub accumulated_coverage_0x264: u16,
    pub next_crossing_hint_0x268: i32,
}

impl AreNativeRowEventContext {
    fn new(_row_y: i32, options: AreNativeRowEventOptions) -> Self {
        Self {
            row_buckets: AreNativeRowBuckets::new(options.row_base_y, options.row_bucket_count),
            active_list: AreNativeActiveList::new(),
            prepared_row_y: options.row_base_y,
            guard_counter: 0,
            x_origin_fixed: options.x_origin_fixed,
            event_vector: AreNativeRowEventVector::new(),
            coverage_cache_valid_0x260: false,
            accumulated_coverage_0x264: 0,
            next_crossing_hint_0x268: ARE_CROSSING_SENTINEL,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AreNativeRowBuckets {
    pub row_base_y: i32,
    pub heads: Vec<Option<usize>>,
}

impl AreNativeRowBuckets {
    fn new(row_base_y: i32, row_bucket_count: usize) -> Self {
        Self {
            row_base_y,
            heads: vec![None; row_bucket_count],
        }
    }

    fn bucket_index(&self, row_y: i32) -> Result<usize, AreNativeRowEventError> {
        let index = row_y - self.row_base_y;
        if index < 0 || index as usize >= self.heads.len() {
            return Err(AreNativeRowEventError::RowOutsideBucketTable {
                row_y,
                row_base_y: self.row_base_y,
                row_bucket_count: self.heads.len(),
            });
        }
        Ok(index as usize)
    }

    fn head(&self, row_y: i32) -> Result<Option<usize>, AreNativeRowEventError> {
        Ok(self.heads[self.bucket_index(row_y)?])
    }

    fn set_head(&mut self, row_y: i32, head: Option<usize>) -> Result<(), AreNativeRowEventError> {
        let bucket_index = self.bucket_index(row_y)?;
        self.heads[bucket_index] = head;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AreNativeActiveList {
    pub head: Option<usize>,
}

impl AreNativeActiveList {
    fn new() -> Self {
        Self { head: None }
    }

    fn refresh_for_fixed_subrow_b944(
        &mut self,
        nodes: &mut [AreNativeEdgeNode],
        fixed_subrow_y: i32,
    ) -> Result<(), AreNativeRowEventError> {
        let existing_indices = self.take_indices(nodes)?;
        let mut inserted_indices = Vec::new();
        for edge_index in existing_indices {
            if inserted_indices.contains(&edge_index) {
                continue;
            }
            if let Some(active_edge_index) =
                active_sibling_extension_b944(edge_index, fixed_subrow_y, nodes)
            {
                if inserted_indices.contains(&active_edge_index) {
                    continue;
                }
                self.insert_ordered_by_projected_x_b944(active_edge_index, nodes, fixed_subrow_y)?;
                inserted_indices.push(active_edge_index);
            }
        }
        Ok(())
    }

    fn take_indices(
        &mut self,
        nodes: &mut [AreNativeEdgeNode],
    ) -> Result<Vec<usize>, AreNativeRowEventError> {
        let mut indices = Vec::new();
        let mut current = self.head;
        self.head = None;
        let mut guard = 0usize;
        while let Some(edge_index) = current {
            guard += 1;
            if guard > nodes.len() {
                return Err(AreNativeRowEventError::GuardLimitExceeded {
                    phase: AreNativeRowEventPhase::BucketDrainB944,
                    edge_count: nodes.len(),
                });
            }
            current = nodes[edge_index].active_next;
            nodes[edge_index].active_next = None;
            nodes[edge_index].active_prev = None;
            indices.push(edge_index);
        }
        Ok(indices)
    }

    fn insert_ordered_by_projected_x_b944(
        &mut self,
        edge_index: usize,
        nodes: &mut [AreNativeEdgeNode],
        fixed_subrow_y: i32,
    ) -> Result<(), AreNativeRowEventError> {
        let edge_x = projected_x_min(edge_index, nodes, fixed_subrow_y)?;
        nodes[edge_index].active_next = None;
        nodes[edge_index].active_prev = None;

        let Some(head_index) = self.head else {
            self.head = Some(edge_index);
            return Ok(());
        };

        let head_x = projected_x_min(head_index, nodes, fixed_subrow_y)?;
        if edge_x < head_x {
            nodes[edge_index].active_next = Some(head_index);
            nodes[head_index].active_prev = Some(edge_index);
            self.head = Some(edge_index);
            return Ok(());
        }

        let mut previous = head_index;
        while let Some(next) = nodes[previous].active_next {
            let next_x = projected_x_min(next, nodes, fixed_subrow_y)?;
            if next_x > edge_x {
                break;
            }
            previous = next;
        }

        let next = nodes[previous].active_next;
        nodes[edge_index].active_prev = Some(previous);
        nodes[edge_index].active_next = next;
        nodes[previous].active_next = Some(edge_index);
        if let Some(next) = next {
            nodes[next].active_prev = Some(edge_index);
        }
        Ok(())
    }

    fn indices(&self, nodes: &[AreNativeEdgeNode]) -> Result<Vec<usize>, AreNativeRowEventError> {
        let mut indices = Vec::new();
        let mut current = self.head;
        let mut guard = 0usize;
        while let Some(edge_index) = current {
            guard += 1;
            if guard > nodes.len() {
                return Err(AreNativeRowEventError::GuardLimitExceeded {
                    phase: AreNativeRowEventPhase::BucketDrainB944,
                    edge_count: nodes.len(),
                });
            }
            indices.push(edge_index);
            current = nodes[edge_index].active_next;
        }
        Ok(indices)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeRowEventVector {
    pub entries: Vec<i32>,
    pub cursor: usize,
    pub rewinds: Vec<AreNativeEventVectorRewind>,
}

impl AreNativeRowEventVector {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            rewinds: Vec::new(),
        }
    }

    fn push_start_4a04(&mut self, start_fixed: i32) {
        self.entries.push(start_fixed);
        self.cursor = self.entries.len();
    }

    fn rewind_overlapping_tail_84e8(&mut self, start_fixed: i32) -> Option<i32> {
        let previous_cursor = self.cursor;
        let mut selected_cursor = None;
        let mut merged_close_fixed = None;

        for pair_start in (0..self.entries.len()).step_by(2) {
            let Some(close) = self.entries.get(pair_start + 1).copied() else {
                break;
            };
            if close.saturating_add(1) >= start_fixed {
                selected_cursor = Some(pair_start);
                merged_close_fixed = Some(max_close_from_pairs(&self.entries[pair_start..]));
                break;
            }
        }

        let Some(selected_cursor) = selected_cursor else {
            return None;
        };

        let merged_close_fixed = merged_close_fixed.expect("selected cursor has a close entry");
        let truncate_to = selected_cursor + 1;
        let removed_entries = self.entries[truncate_to..].to_vec();
        self.entries.truncate(truncate_to);
        self.cursor = truncate_to;
        self.rewinds.push(AreNativeEventVectorRewind {
            requested_start_fixed: start_fixed,
            previous_cursor,
            selected_cursor,
            cursor_distance: previous_cursor.saturating_sub(selected_cursor),
            removed_entries,
            merged_close_fixed,
        });
        Some(merged_close_fixed)
    }

    fn push_close_430c(&mut self, close_fixed: i32) {
        if self.entries.len() % 2 == 1 {
            let start_fixed = *self
                .entries
                .last()
                .expect("odd event vector length has a start entry");
            self.entries.push(close_fixed.max(start_fixed));
        } else if let Some(last) = self.entries.last_mut() {
            *last = (*last).max(close_fixed);
        } else {
            self.entries.push(close_fixed);
        }
        self.cursor = self.entries.len();
    }

    fn subtract_x_origin_430c(&mut self, x_origin_fixed: i32) {
        if x_origin_fixed == 0 {
            return;
        }
        for entry in &mut self.entries {
            *entry = entry.saturating_sub(x_origin_fixed);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AreNativeProjectedEdgeEvent {
    x_min_fixed: i32,
    x_max_fixed: i32,
    winding_delta: i8,
}

fn validate_native_row_event_input(
    input: &AreNativeRowEventInput,
) -> Result<(), AreNativeRowEventError> {
    match input.provenance {
        AreSourcePathProvenance::SourceOwned
        | AreSourcePathProvenance::SourceOwnedGlyphPath
        | AreSourcePathProvenance::SourceOwnedGlyphRun => {}
        AreSourcePathProvenance::TypedSpanFallback => {
            return Err(AreNativeRowEventError::TypedSpanFallbackRejected);
        }
        AreSourcePathProvenance::CoverageRowFallback => {
            return Err(AreNativeRowEventError::CoverageRowFallbackRejected);
        }
        provenance => {
            return Err(AreNativeRowEventError::RequiresSourceOwnedProvenance { provenance });
        }
    }
    if input.edges.is_empty() {
        return Err(AreNativeRowEventError::EmptyEdges);
    }
    if input.edges.iter().any(|edge| edge.edge.winding_delta == 0) {
        return Err(AreNativeRowEventError::ZeroWindingEdge);
    }
    if input.options.row_bucket_count == 0 {
        return Err(AreNativeRowEventError::InvalidBucketTable {
            row_bucket_count: input.options.row_bucket_count,
        });
    }
    if let Some(seed_order_4afc) = &input.seed_order_4afc {
        for seed_index in seed_order_4afc {
            if *seed_index >= input.edges.len() {
                return Err(AreNativeRowEventError::InvalidSeedOrderIndex {
                    seed_index: *seed_index,
                    edge_count: input.edges.len(),
                });
            }
        }
    }
    let buckets =
        AreNativeRowBuckets::new(input.options.row_base_y, input.options.row_bucket_count);
    let fixed_row_y = input.row_y * ARE_FIXED_SUBPIXEL_SCALE;
    buckets.bucket_index(fixed_row_y)?;
    buckets.bucket_index(fixed_row_y + ARE_SUBROW_COUNT as i32 - 1)?;
    Ok(())
}

fn stage_contour_sibling_links_7348(
    edges: &[AreNativeRowEventEdgeInput],
) -> Result<AreNativeContourStaging, AreNativeRowEventError> {
    let mut nodes = Vec::with_capacity(edges.len());
    let mut contours: Vec<AreNativeContourChain> = Vec::new();

    for (input_index, edge_input) in edges.iter().enumerate() {
        let node_index = nodes.len();
        nodes.push(AreNativeEdgeNode::from_input(input_index, edge_input));

        if let Some(contour) = contours
            .iter_mut()
            .find(|contour| contour.contour_id == edge_input.contour_id)
        {
            link_contour_sibling_7348(&mut nodes, contour.last_edge_index, node_index);
            contour.last_edge_index = node_index;
            contour.edge_indices.push(node_index);
        } else {
            contours.push(AreNativeContourChain {
                contour_id: edge_input.contour_id,
                first_edge_index: node_index,
                last_edge_index: node_index,
                edge_indices: vec![node_index],
            });
        }
    }

    Ok(AreNativeContourStaging { nodes, contours })
}

fn link_contour_sibling_7348(
    nodes: &mut [AreNativeEdgeNode],
    previous_index: usize,
    current_index: usize,
) {
    let previous_winding = nodes[previous_index].winding_delta;
    let current_winding = nodes[current_index].winding_delta;
    if previous_winding != current_winding {
        return;
    }

    if current_winding > 0 {
        nodes[current_index].sibling_next = Some(previous_index);
    } else {
        nodes[previous_index].sibling_next = Some(current_index);
    }
}

fn produce_subrow_events(
    fixed_subrow_y: i32,
    subrow_index: usize,
    staged: &AreNativeContourStaging,
    nodes: &mut [AreNativeEdgeNode],
    context: &mut AreNativeRowEventContext,
    fill_rule: AreFillRule,
    seed_trace: &AreNativeRowSeedTrace,
) -> Result<(AreNativeRowEventVector, AreNativeRowEventSubrowTrace), AreNativeRowEventError> {
    let bucket_drain_edge_indices =
        drain_bucket_to_active_list_b944(fixed_subrow_y, nodes, context)?;
    let active_indices = context.active_list.indices(&nodes)?;
    let event_vector = emit_crossing_event_vector_430c(
        &active_indices,
        nodes,
        fixed_subrow_y,
        fill_rule,
        context.x_origin_fixed,
    )?;
    let projected_edge_events = active_indices
        .iter()
        .filter_map(|edge_index| {
            projected_event_for_node(*edge_index, fixed_subrow_y, nodes).map(|event| {
                AreNativeProjectedEdgeTrace {
                    input_index: nodes[*edge_index].input_index,
                    contour_id: nodes[*edge_index].contour_id,
                    x_min_fixed: event.x_min_fixed,
                    x_max_fixed: event.x_max_fixed,
                    winding_delta: event.winding_delta,
                }
            })
        })
        .collect();
    let trace = AreNativeRowEventSubrowTrace {
        subrow_index,
        fixed_subrow_y,
        contour_start_edge_indices: staged
            .contours
            .iter()
            .map(|contour| staged.nodes[contour.first_edge_index].input_index)
            .collect(),
        row_add_edge_indices: seed_trace.row_add_edge_indices(subrow_index),
        bucket_insert_edge_indices: seed_trace.bucket_insert_edge_indices(subrow_index),
        bucket_drain_edge_indices,
        active_edge_indices: active_indices
            .iter()
            .map(|edge_index| nodes[*edge_index].input_index)
            .collect(),
        projected_edge_events,
        flat_sorted_edge_indices: flat_sorted_projected_event_order(fixed_subrow_y, &staged.nodes),
        event_vector_entries: event_vector.entries.clone(),
        event_vector_rewinds: event_vector.rewinds.clone(),
    };

    Ok((event_vector, trace))
}

fn add_contour_chain_to_row_71f4(
    contour: &AreNativeContourChain,
    nodes: &mut [AreNativeEdgeNode],
    context: &mut AreNativeRowEventContext,
    bucket_policy: AreNativeRowBucketPolicy,
    seed_trace: &mut AreNativeRowSeedTrace,
) -> Result<(), AreNativeRowEventError> {
    let mut guard = 0usize;
    for edge_index in contour.edge_indices.iter().copied() {
        guard += 1;
        if guard > nodes.len() {
            return Err(AreNativeRowEventError::GuardLimitExceeded {
                phase: AreNativeRowEventPhase::RowAdd71f4,
                edge_count: nodes.len(),
            });
        }
        add_seed_edge_to_row_71f4(edge_index, nodes, context, bucket_policy, seed_trace)?;
    }

    Ok(())
}

fn add_seed_order_to_row_71f4(
    seed_order_4afc: &[usize],
    nodes: &mut [AreNativeEdgeNode],
    context: &mut AreNativeRowEventContext,
    bucket_policy: AreNativeRowBucketPolicy,
    seed_trace: &mut AreNativeRowSeedTrace,
) -> Result<(), AreNativeRowEventError> {
    let mut guard = 0usize;
    for seed_input_index in seed_order_4afc {
        guard += 1;
        if guard > nodes.len() {
            return Err(AreNativeRowEventError::GuardLimitExceeded {
                phase: AreNativeRowEventPhase::RowAdd71f4,
                edge_count: nodes.len(),
            });
        }
        let Some(edge_index) = nodes
            .iter()
            .position(|node| node.input_index == *seed_input_index)
        else {
            return Err(AreNativeRowEventError::InvalidSeedOrderIndex {
                seed_index: *seed_input_index,
                edge_count: nodes.len(),
            });
        };
        add_seed_edge_to_row_71f4(edge_index, nodes, context, bucket_policy, seed_trace)?;
    }

    Ok(())
}

fn add_seed_edge_to_row_71f4(
    edge_index: usize,
    nodes: &mut [AreNativeEdgeNode],
    context: &mut AreNativeRowEventContext,
    bucket_policy: AreNativeRowBucketPolicy,
    seed_trace: &mut AreNativeRowSeedTrace,
) -> Result<(), AreNativeRowEventError> {
    if nodes[edge_index].row_active_temp {
        return Ok(());
    }

    let Some(bucket_y) = bucket_y_for_edge_71f4(&nodes[edge_index], &context.row_buckets) else {
        return Ok(());
    };

    seed_trace.record_row_add(bucket_y, nodes[edge_index].input_index);
    if let Some(inserted_edge_index) = bucket_insert_726c(
        edge_index,
        bucket_y,
        nodes,
        &mut context.row_buckets,
        bucket_policy,
    )? {
        seed_trace.record_bucket_insert(bucket_y, nodes[inserted_edge_index].input_index);
    }

    Ok(())
}

fn bucket_y_for_edge_71f4(
    edge: &AreNativeEdgeNode,
    row_buckets: &AreNativeRowBuckets,
) -> Option<i32> {
    let table_start = row_buckets.row_base_y;
    let table_end = table_start + row_buckets.heads.len() as i32;
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

fn bucket_insert_726c(
    edge_index: usize,
    bucket_y: i32,
    nodes: &mut [AreNativeEdgeNode],
    row_buckets: &mut AreNativeRowBuckets,
    bucket_policy: AreNativeRowBucketPolicy,
) -> Result<Option<usize>, AreNativeRowEventError> {
    let Some(mut insert_projection) = project_edge_78e4(edge_index, bucket_y, nodes) else {
        return Ok(None);
    };
    let mut insert_index = edge_index;
    nodes[insert_index].row_next = None;

    if bucket_policy.uses_resolved_726c_callbacks() {
        while can_propagate_to_sibling_726c(&nodes[insert_index], bucket_y) {
            let Some(sibling_index) = nodes[insert_index].sibling_next else {
                break;
            };
            let Some(sibling_projection) = project_edge_78e4(sibling_index, bucket_y, nodes) else {
                break;
            };

            nodes[insert_index].row_active_temp = true;
            let propagated = sibling_range_propagation_b6c0(sibling_projection, insert_projection);
            apply_projection_override_b6c0(sibling_index, bucket_y, propagated, nodes);
            insert_index = sibling_index;
            insert_projection = propagated;
        }
    }

    nodes[insert_index].row_next = None;
    nodes[insert_index].row_active_temp = true;
    apply_projection_override_b6c0(insert_index, bucket_y, insert_projection, nodes);
    insert_into_row_bucket_726c(bucket_y, insert_index, nodes, row_buckets, bucket_policy)?;
    Ok(Some(insert_index))
}

fn insert_into_row_bucket_726c(
    row_y: i32,
    edge_index: usize,
    nodes: &mut [AreNativeEdgeNode],
    row_buckets: &mut AreNativeRowBuckets,
    bucket_policy: AreNativeRowBucketPolicy,
) -> Result<(), AreNativeRowEventError> {
    let Some(head_index) = row_buckets.head(row_y)? else {
        row_buckets.set_head(row_y, Some(edge_index))?;
        return Ok(());
    };

    if bucket_policy.uses_resolved_726c_callbacks() {
        let edge_x_min = nodes[edge_index].projected_x_min_fixed.ok_or(
            AreNativeRowEventError::MissingProjection {
                edge_index,
                fixed_subrow_y: nodes[edge_index]
                    .projection_fixed_subrow_y
                    .unwrap_or_default(),
            },
        )?;
        let head_x_min = nodes[head_index].projected_x_min_fixed.ok_or(
            AreNativeRowEventError::MissingProjection {
                edge_index: head_index,
                fixed_subrow_y: nodes[head_index]
                    .projection_fixed_subrow_y
                    .unwrap_or_default(),
            },
        )?;
        if edge_x_min >= head_x_min {
            let mut previous = head_index;
            while let Some(next) = nodes[previous].row_next {
                let next_x_min = nodes[next].projected_x_min_fixed.ok_or(
                    AreNativeRowEventError::MissingProjection {
                        edge_index: next,
                        fixed_subrow_y: nodes[next].projection_fixed_subrow_y.unwrap_or_default(),
                    },
                )?;
                if next_x_min > edge_x_min {
                    break;
                }
                previous = next;
            }
            nodes[edge_index].row_next = nodes[previous].row_next;
            nodes[previous].row_next = Some(edge_index);
            return Ok(());
        }
    }

    nodes[edge_index].row_next = Some(head_index);
    row_buckets.set_head(row_y, Some(edge_index))
}

fn drain_bucket_to_active_list_b944(
    fixed_subrow_y: i32,
    nodes: &mut [AreNativeEdgeNode],
    context: &mut AreNativeRowEventContext,
) -> Result<Vec<usize>, AreNativeRowEventError> {
    let mut bucket_drain_edge_indices = Vec::new();
    context
        .active_list
        .refresh_for_fixed_subrow_b944(nodes, fixed_subrow_y)?;
    let mut current = context.row_buckets.head(fixed_subrow_y)?;
    context.row_buckets.set_head(fixed_subrow_y, None)?;
    let mut guard = 0usize;
    let mut inserted_indices = Vec::new();

    while let Some(edge_index) = current {
        guard += 1;
        if guard > nodes.len() {
            return Err(AreNativeRowEventError::GuardLimitExceeded {
                phase: AreNativeRowEventPhase::BucketDrainB944,
                edge_count: nodes.len(),
            });
        }
        current = nodes[edge_index].row_next;
        nodes[edge_index].row_next = None;
        nodes[edge_index].row_active_temp = false;
        if inserted_indices.contains(&edge_index) {
            continue;
        }
        if let Some(active_edge_index) =
            active_sibling_extension_b944(edge_index, fixed_subrow_y, nodes)
        {
            if inserted_indices.contains(&active_edge_index) {
                continue;
            }
            context.active_list.insert_ordered_by_projected_x_b944(
                active_edge_index,
                nodes,
                fixed_subrow_y,
            )?;
            bucket_drain_edge_indices.push(nodes[active_edge_index].input_index);
            inserted_indices.push(active_edge_index);
        }
    }

    Ok(bucket_drain_edge_indices)
}

fn active_sibling_extension_b944(
    edge_index: usize,
    fixed_subrow_y: i32,
    nodes: &mut [AreNativeEdgeNode],
) -> Option<usize> {
    let mut active_index = edge_index;
    let mut active_projection = project_edge_78e4(active_index, fixed_subrow_y, nodes)?;
    let strip_end_y = fixed_subrow_y + 1;

    while nodes[active_index].end_y_fixed <= strip_end_y {
        let Some(sibling_index) = nodes[active_index].sibling_next else {
            break;
        };
        let Some(sibling_projection) = project_edge_78e4(sibling_index, fixed_subrow_y, nodes)
        else {
            break;
        };
        let propagated = sibling_range_propagation_b6c0(sibling_projection, active_projection);
        apply_projection_override_b6c0(sibling_index, fixed_subrow_y, propagated, nodes);
        active_index = sibling_index;
        active_projection = propagated;
    }

    Some(active_index)
}

fn emit_crossing_event_vector_430c(
    active_indices: &[usize],
    nodes: &[AreNativeEdgeNode],
    fixed_subrow_y: i32,
    fill_rule: AreFillRule,
    x_origin_fixed: i32,
) -> Result<AreNativeRowEventVector, AreNativeRowEventError> {
    let mut event_vector = AreNativeRowEventVector::new();
    let mut winding = 0i16;
    let mut parity = false;
    let mut open = false;
    let mut max_projected_close_floor = i32::MIN;

    for edge_index in active_indices {
        let event = projected_event_for_node(*edge_index, fixed_subrow_y, nodes).ok_or(
            AreNativeRowEventError::MissingProjection {
                edge_index: *edge_index,
                fixed_subrow_y,
            },
        )?;
        match fill_rule {
            AreFillRule::NonZeroWinding => winding += event.winding_delta as i16,
            AreFillRule::EvenOdd => parity ^= true,
        }
        let is_filled = fill_is_active(fill_rule, winding, parity);
        let start_fixed = event.x_min_fixed;
        let projected_close_floor = event.x_max_fixed;

        if !open {
            if event_vector.entries.is_empty()
                || max_projected_close_floor.saturating_add(1) < start_fixed
            {
                event_vector.push_start_4a04(start_fixed);
            } else if let Some(merged_close_fixed) =
                event_vector.rewind_overlapping_tail_84e8(start_fixed)
            {
                max_projected_close_floor = max_projected_close_floor.max(merged_close_fixed - 1);
            }
        }
        max_projected_close_floor = max_projected_close_floor.max(projected_close_floor);

        if is_filled {
            open = true;
        } else if open || event_vector.entries.len() % 2 != 0 {
            event_vector.push_close_430c(max_projected_close_floor.saturating_add(1));
            open = false;
        }
    }

    if event_vector.entries.len() % 2 != 0 {
        event_vector.push_close_430c(max_projected_close_floor.saturating_add(1));
    }

    event_vector.subtract_x_origin_430c(x_origin_fixed);
    Ok(event_vector)
}

fn project_edge_78e4(
    edge_index: usize,
    fixed_subrow_y: i32,
    nodes: &mut [AreNativeEdgeNode],
) -> Option<AreNativeProjectedEdgeEvent> {
    if nodes[edge_index].projection_override_active
        && nodes[edge_index].projection_fixed_subrow_y == Some(fixed_subrow_y)
    {
        return projected_event_for_node(edge_index, fixed_subrow_y, nodes);
    }

    let event =
        project_active_edge_for_fixed_strip(&nodes[edge_index].active_edge(), fixed_subrow_y)?;
    nodes[edge_index].projected_x_min_fixed = Some(event.x_min_fixed);
    nodes[edge_index].projected_x_max_fixed = Some(event.x_max_fixed);
    nodes[edge_index].projection_fixed_subrow_y = Some(fixed_subrow_y);
    nodes[edge_index].projection_dirty = false;
    nodes[edge_index].projection_override_active = false;
    Some(event)
}

fn projected_event_for_node(
    edge_index: usize,
    fixed_subrow_y: i32,
    nodes: &[AreNativeEdgeNode],
) -> Option<AreNativeProjectedEdgeEvent> {
    if nodes[edge_index].projection_fixed_subrow_y != Some(fixed_subrow_y) {
        return None;
    }
    Some(AreNativeProjectedEdgeEvent {
        x_min_fixed: nodes[edge_index].projected_x_min_fixed?,
        x_max_fixed: nodes[edge_index].projected_x_max_fixed?,
        winding_delta: nodes[edge_index].winding_delta,
    })
}

fn projected_x_min(
    edge_index: usize,
    nodes: &[AreNativeEdgeNode],
    fixed_subrow_y: i32,
) -> Result<i32, AreNativeRowEventError> {
    projected_event_for_node(edge_index, fixed_subrow_y, nodes)
        .map(|event| event.x_min_fixed)
        .ok_or(AreNativeRowEventError::MissingProjection {
            edge_index,
            fixed_subrow_y,
        })
}

fn project_active_edge_for_fixed_strip(
    edge: &AreActiveEdge,
    fixed_subrow_y: i32,
) -> Option<AreNativeProjectedEdgeEvent> {
    if edge.start_y_fixed == edge.end_y_fixed {
        return None;
    }

    let strip_start = fixed_subrow_y as f32;
    let strip_end = (fixed_subrow_y + 1) as f32;
    let y_min = edge.start_y_fixed.min(edge.end_y_fixed) as f32;
    let y_max = edge.start_y_fixed.max(edge.end_y_fixed) as f32;
    if y_max <= strip_start || y_min >= strip_end {
        return None;
    }

    let ay = edge.start_y_fixed as f32;
    let by = edge.end_y_fixed as f32;
    let ax = edge.start_x_fixed as f32;
    let bx = edge.end_x_fixed as f32;
    let y0 = strip_start.clamp(y_min, y_max);
    let y1 = strip_end.clamp(y_min, y_max);
    let t0 = ((y0 - ay) / (by - ay)).clamp(0.0, 1.0);
    let t1 = ((y1 - ay) / (by - ay)).clamp(0.0, 1.0);
    let x0 = ax + (bx - ax) * t0;
    let x1 = ax + (bx - ax) * t1;
    Some(AreNativeProjectedEdgeEvent {
        x_min_fixed: x0.min(x1).floor() as i32,
        x_max_fixed: x0.max(x1).floor() as i32,
        winding_delta: edge.winding_delta,
    })
}

fn can_propagate_to_sibling_726c(edge: &AreNativeEdgeNode, row_y: i32) -> bool {
    let start_floor = edge.start_y_fixed;
    let end_floor = edge.end_y_fixed;
    start_floor == end_floor || end_floor == row_y
}

fn sibling_range_propagation_b6c0(
    mut sibling_event: AreNativeProjectedEdgeEvent,
    current_event: AreNativeProjectedEdgeEvent,
) -> AreNativeProjectedEdgeEvent {
    sibling_event.x_min_fixed = sibling_event.x_min_fixed.min(current_event.x_min_fixed);
    sibling_event.x_max_fixed = sibling_event.x_max_fixed.max(current_event.x_max_fixed);
    sibling_event
}

fn apply_projection_override_b6c0(
    edge_index: usize,
    fixed_subrow_y: i32,
    event: AreNativeProjectedEdgeEvent,
    nodes: &mut [AreNativeEdgeNode],
) {
    let changed = nodes[edge_index].projected_x_min_fixed != Some(event.x_min_fixed)
        || nodes[edge_index].projected_x_max_fixed != Some(event.x_max_fixed);
    nodes[edge_index].projected_x_min_fixed = Some(event.x_min_fixed);
    nodes[edge_index].projected_x_max_fixed = Some(event.x_max_fixed);
    nodes[edge_index].projection_fixed_subrow_y = Some(fixed_subrow_y);
    nodes[edge_index].projection_dirty = changed;
    nodes[edge_index].projection_override_active = true;
}

fn flat_sorted_projected_event_order(
    fixed_subrow_y: i32,
    staged_edges: &[AreNativeEdgeNode],
) -> Vec<usize> {
    let mut events: Vec<(usize, AreNativeProjectedEdgeEvent)> = staged_edges
        .iter()
        .filter_map(|edge| {
            project_active_edge_for_fixed_strip(&edge.active_edge(), fixed_subrow_y)
                .map(|event| (edge.input_index, event))
        })
        .collect();
    events.sort_by_key(|(_, event)| (event.x_min_fixed, event.winding_delta));
    events
        .into_iter()
        .map(|(input_index, _)| input_index)
        .collect()
}

fn fill_is_active(fill_rule: AreFillRule, winding: i16, parity: bool) -> bool {
    match fill_rule {
        AreFillRule::NonZeroWinding => winding != 0,
        AreFillRule::EvenOdd => parity,
    }
}

fn max_close_from_pairs(entries: &[i32]) -> i32 {
    let mut max_close = i32::MIN;
    for pair_start in (0..entries.len()).step_by(2) {
        if let Some(close) = entries.get(pair_start + 1) {
            max_close = max_close.max(*close);
        }
    }
    max_close
}

fn classify_native_row_event_coverages(
    coverages_0x264: &[u16],
) -> AreNativeRowEventMaterializedSpan {
    if coverages_0x264.iter().all(|coverage| *coverage == 0) {
        return AreNativeRowEventMaterializedSpan::State {
            class: AreNativeRowEventStateClass::Class0,
            state: 0,
        };
    }
    if coverages_0x264
        .iter()
        .all(|coverage| *coverage == ARE_FULL_COVERAGE_0X264)
    {
        return AreNativeRowEventMaterializedSpan::State {
            class: AreNativeRowEventStateClass::Class1,
            state: 1,
        };
    }

    AreNativeRowEventMaterializedSpan::Payload(
        coverages_0x264
            .iter()
            .map(|coverage| coverage_to_payload_byte(*coverage))
            .collect(),
    )
}

fn coverage_to_payload_byte(value_0x264: u16) -> u8 {
    value_0x264.min(ARE_FULL_COVERAGE_0X264 - 1) as u8
}

#[cfg(test)]
mod native_row_event_tests {
    use super::*;

    #[test]
    fn native_row_event_builder_emits_75d0_compatible_sentinel_lists() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_pixels(0, 0, 0, 1, 1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_pixels(2, 0, 2, 1, -1)),
            ],
        );

        let output = build_native_row_event_crossing_lists_for_row(&input).unwrap();

        assert_eq!(
            output.crossing_lists.list(0).unwrap().crossing_values(),
            vec![0, 33, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            output.subrow_traces[0]
                .projected_edge_events
                .iter()
                .map(|event| (event.input_index, event.x_min_fixed, event.x_max_fixed))
                .collect::<Vec<_>>(),
            vec![(0, 0, 0), (1, 32, 32)]
        );
        assert_eq!(
            accumulate_75d0_for_sample_x(&output.crossing_lists, 1)
                .unwrap()
                .coverage
                .value_0x264,
            ARE_FULL_COVERAGE_0X264
        );
    }

    #[test]
    fn native_row_event_builder_keeps_76dc_active_edges_across_subrows() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 0, 0, 4, 1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(32, 0, 32, 4, -1)),
            ],
        );

        let output = build_native_row_event_crossing_lists_for_row(&input).unwrap();

        assert_eq!(output.subrow_traces[0].row_add_edge_indices, vec![0, 1]);
        assert_eq!(
            output.subrow_traces[0].bucket_drain_edge_indices,
            vec![0, 1]
        );
        assert_eq!(
            output.subrow_traces[1].row_add_edge_indices,
            Vec::<usize>::new()
        );
        assert_eq!(
            output.subrow_traces[1].bucket_drain_edge_indices,
            Vec::<usize>::new()
        );
        assert_eq!(output.subrow_traces[1].active_edge_indices, vec![0, 1]);
        assert_eq!(output.subrow_traces[3].active_edge_indices, vec![0, 1]);
        assert!(output.subrow_traces[4].active_edge_indices.is_empty());
    }

    #[test]
    fn native_row_event_builder_keeps_default_contour_seed_order_without_4afc_input() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(32, 0, 32, 2, -1)),
                AreNativeRowEventEdgeInput::new(1, AreActiveEdge::line_fixed(0, 0, 0, 2, 1)),
            ],
        );

        let output = build_native_row_event_crossing_lists_for_row(&input).unwrap();

        assert_eq!(output.subrow_traces[0].row_add_edge_indices, vec![0, 1]);
        assert_eq!(
            output.subrow_traces[0].bucket_insert_edge_indices,
            vec![0, 1]
        );
    }

    #[test]
    fn native_row_event_builder_uses_explicit_4afc_seed_order_for_71f4() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(32, 0, 32, 2, -1)),
                AreNativeRowEventEdgeInput::new(1, AreActiveEdge::line_fixed(0, 0, 0, 2, 1)),
            ],
        )
        .with_seed_order_4afc(vec![1, 0]);

        let output = build_native_row_event_crossing_lists_for_row(&input).unwrap();

        assert_eq!(output.subrow_traces[0].row_add_edge_indices, vec![1, 0]);
        assert_eq!(
            output.subrow_traces[0].bucket_insert_edge_indices,
            vec![1, 0]
        );
    }

    #[test]
    fn native_row_event_builder_uses_4afc_first_last_and_71f4_sibling_walk() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 16, 0, 0, -1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(4, 16, 8, 0, -1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(16, 0, 16, 16, 1)),
            ],
        )
        .with_seed_order_4afc(vec![0, 2]);

        let output = build_native_row_event_crossing_lists_for_row(&input).unwrap();

        assert_eq!(output.subrow_traces[0].row_add_edge_indices, vec![0, 2]);
        assert_eq!(
            output.subrow_traces[0].bucket_insert_edge_indices,
            vec![1, 2]
        );
        assert_eq!(
            output.subrow_traces[0].bucket_drain_edge_indices,
            vec![1, 2]
        );
        assert_eq!(output.subrow_traces[0].active_edge_indices, vec![1, 2]);
    }

    #[test]
    fn native_row_event_builder_rejects_invalid_4afc_seed_index() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![AreNativeRowEventEdgeInput::new(
                0,
                AreActiveEdge::line_fixed(0, 0, 0, 2, 1),
            )],
        )
        .with_seed_order_4afc(vec![1]);

        let error = build_native_row_event_crossing_lists_for_row(&input).unwrap_err();

        assert_eq!(
            error,
            AreNativeRowEventError::InvalidSeedOrderIndex {
                seed_index: 1,
                edge_count: 1
            }
        );
    }

    #[test]
    fn native_row_event_span_input_passes_4afc_seed_order_to_row_materializer() {
        let input = AreNativeRowEventSpanInput::source_owned(
            0,
            0,
            2,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(32, 0, 32, 2, -1)),
                AreNativeRowEventEdgeInput::new(1, AreActiveEdge::line_fixed(0, 0, 0, 2, 1)),
            ],
        )
        .with_seed_order_4afc(vec![1, 0]);

        let materialization = materialize_source_span_with_native_row_events_with_trace(&input)
            .expect("span input with explicit 4afc seed order should materialize");

        assert_eq!(
            materialization.crossing_output.subrow_traces[0].row_add_edge_indices,
            vec![1, 0]
        );
    }

    #[test]
    fn native_row_event_builder_extends_active_edge_to_sibling_in_b944() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 0, 0, 1, -1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 0, 16, 4, -1)),
                AreNativeRowEventEdgeInput::new(1, AreActiveEdge::line_fixed(32, 0, 32, 4, 1)),
            ],
        );

        let output = build_native_row_event_crossing_lists_for_row(&input).unwrap();

        assert_eq!(output.subrow_traces[0].active_edge_indices, vec![1, 2]);
        assert_eq!(
            output.subrow_traces[0]
                .projected_edge_events
                .iter()
                .map(|event| (event.input_index, event.x_min_fixed, event.x_max_fixed))
                .collect::<Vec<_>>(),
            vec![(1, 0, 4), (2, 32, 32)]
        );
    }

    #[test]
    fn native_row_event_builder_records_multi_pair_84e8_rewind() {
        let mut vector = AreNativeRowEventVector::new();
        vector.push_start_4a04(0);
        vector.push_close_430c(10);
        vector.push_start_4a04(12);
        vector.push_close_430c(20);

        assert_eq!(vector.rewind_overlapping_tail_84e8(9), Some(20));

        assert_eq!(vector.entries, vec![0]);
        assert_eq!(vector.rewinds.len(), 1);
        assert_eq!(vector.rewinds[0].removed_entries, vec![10, 12, 20]);
    }

    #[test]
    fn native_row_event_builder_merges_touching_430c_pairs() {
        let input = AreNativeRowEventInput::source_owned(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(0, 0, 0, 1, 1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(10, 0, 10, 1, -1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(11, 0, 11, 1, 1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_fixed(20, 0, 20, 1, -1)),
            ],
        );

        let output = build_native_row_event_crossing_lists_for_row(&input).unwrap();
        let subrow = &output.subrow_traces[0];

        assert_eq!(subrow.event_vector_entries, vec![0, 21]);
        assert_eq!(subrow.event_vector_rewinds.len(), 1);
    }

    #[test]
    fn materialize_source_span_with_native_row_events_classifies_full_span() {
        let input = AreNativeRowEventSpanInput::source_owned(
            0,
            0,
            2,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_pixels(0, 0, 0, 1, 1)),
                AreNativeRowEventEdgeInput::new(0, AreActiveEdge::line_pixels(2, 0, 2, 1, -1)),
            ],
        );

        let materialized = materialize_source_span_with_native_row_events(&input).unwrap();

        assert_eq!(
            materialized,
            AreNativeRowEventMaterializedSpan::State {
                class: AreNativeRowEventStateClass::Class1,
                state: 1
            }
        );
    }
}
