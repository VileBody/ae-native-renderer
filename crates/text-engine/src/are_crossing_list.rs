use crate::AreSourcePathProvenance;
use serde::{Deserialize, Serialize};

pub const ARE_CROSSING_SENTINEL: i32 = 0x7fff_ffff;
pub const ARE_SUBROW_COUNT: usize = 16;
pub const ARE_FIXED_SUBPIXEL_SCALE: i32 = 16;
pub const ARE_FULL_COVERAGE_0X264: u16 = 0x100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreFillRule {
    NonZeroWinding,
    EvenOdd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreActiveEdge {
    pub start_x_fixed: i32,
    pub start_y_fixed: i32,
    pub end_x_fixed: i32,
    pub end_y_fixed: i32,
    pub winding_delta: i8,
}

impl AreActiveEdge {
    pub fn line_fixed(
        start_x_fixed: i32,
        start_y_fixed: i32,
        end_x_fixed: i32,
        end_y_fixed: i32,
        winding_delta: i8,
    ) -> Self {
        Self {
            start_x_fixed,
            start_y_fixed,
            end_x_fixed,
            end_y_fixed,
            winding_delta,
        }
    }

    pub fn line_pixels(
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        winding_delta: i8,
    ) -> Self {
        Self::line_fixed(
            start_x * ARE_FIXED_SUBPIXEL_SCALE,
            start_y * ARE_FIXED_SUBPIXEL_SCALE,
            end_x * ARE_FIXED_SUBPIXEL_SCALE,
            end_y * ARE_FIXED_SUBPIXEL_SCALE,
            winding_delta,
        )
    }

    fn projected_x_span_for_fixed_strip(&self, fixed_subrow_y: i32) -> Option<(i32, i32)> {
        if self.start_y_fixed == self.end_y_fixed {
            return None;
        }
        let strip_start = fixed_subrow_y as f32;
        let strip_end = (fixed_subrow_y + 1) as f32;
        let y_min = self.start_y_fixed.min(self.end_y_fixed) as f32;
        let y_max = self.start_y_fixed.max(self.end_y_fixed) as f32;
        if y_max <= strip_start || y_min >= strip_end {
            return None;
        }

        let ay = self.start_y_fixed as f32;
        let by = self.end_y_fixed as f32;
        let ax = self.start_x_fixed as f32;
        let bx = self.end_x_fixed as f32;
        let y0 = strip_start.clamp(y_min, y_max);
        let y1 = strip_end.clamp(y_min, y_max);
        let t0 = ((y0 - ay) / (by - ay)).clamp(0.0, 1.0);
        let t1 = ((y1 - ay) / (by - ay)).clamp(0.0, 1.0);
        let x0 = ax + (bx - ax) * t0;
        let x1 = ax + (bx - ax) * t1;
        Some((x0.min(x1).floor() as i32, x0.max(x1).floor() as i32))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreCrossingListInput {
    pub provenance: AreSourcePathProvenance,
    pub row_y: i32,
    pub fill_rule: AreFillRule,
    pub edges: Vec<AreActiveEdge>,
}

impl AreCrossingListInput {
    pub fn source_owned(row_y: i32, fill_rule: AreFillRule, edges: Vec<AreActiveEdge>) -> Self {
        Self {
            provenance: AreSourcePathProvenance::SourceOwned,
            row_y,
            fill_rule,
            edges,
        }
    }

    pub fn typed_span_fallback(
        row_y: i32,
        fill_rule: AreFillRule,
        edges: Vec<AreActiveEdge>,
    ) -> Self {
        Self {
            provenance: AreSourcePathProvenance::TypedSpanFallback,
            row_y,
            fill_rule,
            edges,
        }
    }

    pub fn coverage_row_fallback(
        row_y: i32,
        fill_rule: AreFillRule,
        edges: Vec<AreActiveEdge>,
    ) -> Self {
        Self {
            provenance: AreSourcePathProvenance::CoverageRowFallback,
            row_y,
            fill_rule,
            edges,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeDebugEdgeInput {
    pub contour_id: usize,
    pub edge: AreActiveEdge,
}

impl AreNativeDebugEdgeInput {
    pub fn new(contour_id: usize, edge: AreActiveEdge) -> Self {
        Self { contour_id, edge }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreNativeBucketCallbackPolicy {
    FallbackHeadPrependOnly,
    OverscanSampler726cCallbacks,
    NativeVtableSlotsUnresolvedFallbackHeadPrepend,
}

impl AreNativeBucketCallbackPolicy {
    pub fn uses_unresolved_native_vtable_slots(self) -> bool {
        matches!(
            self,
            AreNativeBucketCallbackPolicy::NativeVtableSlotsUnresolvedFallbackHeadPrepend
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeDebugCrossingInput {
    pub row_y: i32,
    pub fill_rule: AreFillRule,
    pub bucket_callback_policy: AreNativeBucketCallbackPolicy,
    pub edges: Vec<AreNativeDebugEdgeInput>,
}

impl AreNativeDebugCrossingInput {
    pub fn source_owned_debug(
        row_y: i32,
        fill_rule: AreFillRule,
        edges: Vec<AreNativeDebugEdgeInput>,
    ) -> Self {
        Self {
            row_y,
            fill_rule,
            bucket_callback_policy: AreNativeBucketCallbackPolicy::OverscanSampler726cCallbacks,
            edges,
        }
    }

    pub fn with_bucket_callback_policy(
        mut self,
        bucket_callback_policy: AreNativeBucketCallbackPolicy,
    ) -> Self {
        self.bucket_callback_policy = bucket_callback_policy;
        self
    }

    fn active_edges(&self) -> Vec<AreActiveEdge> {
        self.edges.iter().map(|edge| edge.edge.clone()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeDebugSubrowTrace {
    pub subrow_index: usize,
    pub fixed_subrow_y: i32,
    pub bucket_insert_edge_indices: Vec<usize>,
    pub bucket_drain_edge_indices: Vec<usize>,
    pub native_active_edge_indices: Vec<usize>,
    pub flat_sorted_edge_indices: Vec<usize>,
    pub unresolved_726c_callback_slots: bool,
}

impl AreNativeDebugSubrowTrace {
    pub fn has_order_delta(&self) -> bool {
        self.native_active_edge_indices != self.flat_sorted_edge_indices
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeDebugCrossingOutput {
    pub crossing_lists: AreSubrowCrossingArray16,
    pub subrow_traces: Vec<AreNativeDebugSubrowTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreNativeDebugCrossingComparison {
    pub flat_crossing_lists: AreSubrowCrossingArray16,
    pub native_debug_crossing_lists: AreSubrowCrossingArray16,
    pub subrow_traces: Vec<AreNativeDebugSubrowTrace>,
    pub differing_subrows: Vec<usize>,
    pub ordering_delta_subrows: Vec<usize>,
    pub unresolved_726c_callback_slots: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSubrowCrossingEntry {
    pub x_fixed: i32,
}

impl AreSubrowCrossingEntry {
    pub fn new(x_fixed: i32) -> Self {
        Self { x_fixed }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSubrowCrossingList {
    pub subrow_index: usize,
    pub fixed_subrow_y: i32,
    pub entries: Vec<AreSubrowCrossingEntry>,
}

impl AreSubrowCrossingList {
    pub fn crossing_values(&self) -> Vec<i32> {
        self.entries.iter().map(|entry| entry.x_fixed).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSubrowCrossingArray16 {
    pub row_y: i32,
    pub fill_rule: AreFillRule,
    pub lists: Vec<AreSubrowCrossingList>,
}

impl AreSubrowCrossingArray16 {
    pub fn list(&self, subrow_index: usize) -> Option<&AreSubrowCrossingList> {
        self.lists.get(subrow_index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreCoverageByte {
    pub value_0x264: u16,
}

impl AreCoverageByte {
    pub fn new(value_0x264: u16) -> Result<Self, AreCrossingListError> {
        if value_0x264 > ARE_FULL_COVERAGE_0X264 {
            return Err(AreCrossingListError::CoverageOutOfRange { value_0x264 });
        }
        Ok(Self { value_0x264 })
    }

    pub fn class2_payload_byte(self) -> Option<u8> {
        match self.value_0x264 {
            0 | ARE_FULL_COVERAGE_0X264 => None,
            value => Some(value as u8),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSubrowAccumulator75d0 {
    pub sample_x: i32,
    pub sample_x_fixed_start: i32,
    pub sample_x_fixed_end: i32,
    pub coverage: AreCoverageByte,
    pub next_crossing_hint: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreCrossingListError {
    TypedSpanFallbackRejected,
    CoverageRowFallbackRejected,
    RequiresSourceOwnedProvenance {
        provenance: AreSourcePathProvenance,
    },
    EmptyEdges,
    ZeroWindingEdge,
    MissingSentinel {
        subrow_index: usize,
    },
    NonMonotonicCrossing {
        subrow_index: usize,
        previous_x_fixed: i32,
        current_x_fixed: i32,
    },
    CoverageOutOfRange {
        value_0x264: u16,
    },
    CurveRequiresControlPoints,
}

pub fn build_crossing_lists_for_row(
    row_y: i32,
    edges: &[AreActiveEdge],
    fill_rule: AreFillRule,
) -> Result<AreSubrowCrossingArray16, AreCrossingListError> {
    build_crossing_lists_for_row_from_input(&AreCrossingListInput::source_owned(
        row_y,
        fill_rule,
        edges.to_vec(),
    ))
}

pub fn build_crossing_lists_for_row_from_input(
    input: &AreCrossingListInput,
) -> Result<AreSubrowCrossingArray16, AreCrossingListError> {
    validate_input_provenance(input.provenance)?;
    if input.edges.is_empty() {
        return Err(AreCrossingListError::EmptyEdges);
    }
    if input.edges.iter().any(|edge| edge.winding_delta == 0) {
        return Err(AreCrossingListError::ZeroWindingEdge);
    }

    let mut lists = Vec::with_capacity(ARE_SUBROW_COUNT);
    let fixed_row_y = input.row_y * ARE_FIXED_SUBPIXEL_SCALE;
    for subrow_index in 0..ARE_SUBROW_COUNT {
        let fixed_subrow_y = fixed_row_y + subrow_index as i32;
        let mut crossings = crossings_for_subrow(fixed_subrow_y, &input.edges, input.fill_rule);
        crossings.push(ARE_CROSSING_SENTINEL);
        lists.push(AreSubrowCrossingList {
            subrow_index,
            fixed_subrow_y,
            entries: crossings
                .into_iter()
                .map(AreSubrowCrossingEntry::new)
                .collect(),
        });
    }

    Ok(AreSubrowCrossingArray16 {
        row_y: input.row_y,
        fill_rule: input.fill_rule,
        lists,
    })
}

pub fn build_native_debug_crossing_lists_for_row(
    input: &AreNativeDebugCrossingInput,
) -> Result<AreNativeDebugCrossingOutput, AreCrossingListError> {
    if input.edges.is_empty() {
        return Err(AreCrossingListError::EmptyEdges);
    }
    if input.edges.iter().any(|edge| edge.edge.winding_delta == 0) {
        return Err(AreCrossingListError::ZeroWindingEdge);
    }

    let staged_edges = native_debug_stage_edges(&input.edges);
    let contour_starts = native_debug_contour_starts(&staged_edges);
    let mut lists = Vec::with_capacity(ARE_SUBROW_COUNT);
    let mut subrow_traces = Vec::with_capacity(ARE_SUBROW_COUNT);
    let fixed_row_y = input.row_y * ARE_FIXED_SUBPIXEL_SCALE;
    for subrow_index in 0..ARE_SUBROW_COUNT {
        let fixed_subrow_y = fixed_row_y + subrow_index as i32;
        let (ordered_events, trace) = native_debug_projected_events_for_subrow(
            fixed_subrow_y,
            subrow_index,
            &staged_edges,
            &contour_starts,
            input.bucket_callback_policy,
        );
        let mut crossings = crossings_from_ordered_projected_events(
            ordered_events.into_iter().map(|(_, event)| event),
            input.fill_rule,
        );
        crossings.push(ARE_CROSSING_SENTINEL);
        lists.push(AreSubrowCrossingList {
            subrow_index,
            fixed_subrow_y,
            entries: crossings
                .into_iter()
                .map(AreSubrowCrossingEntry::new)
                .collect(),
        });
        subrow_traces.push(trace);
    }

    Ok(AreNativeDebugCrossingOutput {
        crossing_lists: AreSubrowCrossingArray16 {
            row_y: input.row_y,
            fill_rule: input.fill_rule,
            lists,
        },
        subrow_traces,
    })
}

pub fn compare_native_debug_and_flat_crossing_lists_for_row(
    input: &AreNativeDebugCrossingInput,
) -> Result<AreNativeDebugCrossingComparison, AreCrossingListError> {
    let native_debug = build_native_debug_crossing_lists_for_row(input)?;
    let flat_edges = input.active_edges();
    let flat_crossing_lists =
        build_crossing_lists_for_row(input.row_y, &flat_edges, input.fill_rule)?;
    let differing_subrows = native_debug
        .crossing_lists
        .lists
        .iter()
        .zip(&flat_crossing_lists.lists)
        .filter_map(|(native, flat)| {
            (native.crossing_values() != flat.crossing_values()).then_some(native.subrow_index)
        })
        .collect();
    let ordering_delta_subrows = native_debug
        .subrow_traces
        .iter()
        .filter_map(|trace| trace.has_order_delta().then_some(trace.subrow_index))
        .collect();

    Ok(AreNativeDebugCrossingComparison {
        flat_crossing_lists,
        native_debug_crossing_lists: native_debug.crossing_lists,
        subrow_traces: native_debug.subrow_traces,
        differing_subrows,
        ordering_delta_subrows,
        unresolved_726c_callback_slots: input
            .bucket_callback_policy
            .uses_unresolved_native_vtable_slots(),
    })
}

pub fn accumulate_75d0_for_sample_x(
    crossing_lists: &AreSubrowCrossingArray16,
    sample_x: i32,
) -> Result<AreSubrowAccumulator75d0, AreCrossingListError> {
    let sample_x_fixed_start = sample_x * ARE_FIXED_SUBPIXEL_SCALE;
    let sample_x_fixed_end = sample_x_fixed_start + ARE_FIXED_SUBPIXEL_SCALE;
    let mut coverage: u16 = 0;
    let mut next_crossing_hint = ARE_CROSSING_SENTINEL;

    for list in &crossing_lists.lists {
        validate_crossing_list(list)?;
        let entries = &list.entries;
        let mut cursor = 0usize;
        let mut crossing = entries[cursor].x_fixed;
        let mut filled = false;

        while crossing <= sample_x_fixed_start {
            filled = !filled;
            cursor += 1;
            crossing = entries[cursor].x_fixed;
        }

        let mut last = sample_x_fixed_start;
        loop {
            if filled {
                let end = crossing.min(sample_x_fixed_end);
                coverage += (end - last).max(0) as u16;
            }
            if sample_x_fixed_end <= crossing {
                break;
            }
            last = crossing;
            filled = !filled;
            cursor += 1;
            crossing = entries[cursor].x_fixed;
        }
        next_crossing_hint = next_crossing_hint.min(crossing);
    }

    Ok(AreSubrowAccumulator75d0 {
        sample_x,
        sample_x_fixed_start,
        sample_x_fixed_end,
        coverage: AreCoverageByte::new(coverage)?,
        next_crossing_hint,
    })
}

pub fn curve_requires_control_points() -> Result<(), AreCrossingListError> {
    Err(AreCrossingListError::CurveRequiresControlPoints)
}

fn validate_input_provenance(
    provenance: AreSourcePathProvenance,
) -> Result<(), AreCrossingListError> {
    match provenance {
        AreSourcePathProvenance::SourceOwned
        | AreSourcePathProvenance::SourceOwnedGlyphPath
        | AreSourcePathProvenance::SourceOwnedGlyphRun => Ok(()),
        AreSourcePathProvenance::TypedSpanFallback => {
            Err(AreCrossingListError::TypedSpanFallbackRejected)
        }
        AreSourcePathProvenance::CoverageRowFallback => {
            Err(AreCrossingListError::CoverageRowFallbackRejected)
        }
        provenance => Err(AreCrossingListError::RequiresSourceOwnedProvenance { provenance }),
    }
}

fn crossings_for_subrow(
    fixed_subrow_y: i32,
    edges: &[AreActiveEdge],
    fill_rule: AreFillRule,
) -> Vec<i32> {
    let mut events: Vec<ProjectedEdgeEvent> = edges
        .iter()
        .filter_map(|edge| {
            edge.projected_x_span_for_fixed_strip(fixed_subrow_y).map(
                |(x_min_fixed, x_max_fixed)| ProjectedEdgeEvent {
                    x_min_fixed,
                    x_max_fixed,
                    winding_delta: edge.winding_delta,
                },
            )
        })
        .collect();
    events.sort_by_key(|event| (event.x_min_fixed, event.winding_delta));

    crossings_from_ordered_projected_events(events, fill_rule)
}

fn crossings_from_ordered_projected_events<I>(events: I, fill_rule: AreFillRule) -> Vec<i32>
where
    I: IntoIterator<Item = ProjectedEdgeEvent>,
{
    let mut crossings = Vec::new();
    let mut winding = 0i16;
    let mut parity = false;
    let mut open = false;
    let mut max_end = i32::MIN;

    for event in events {
        match fill_rule {
            AreFillRule::NonZeroWinding => winding += event.winding_delta as i16,
            AreFillRule::EvenOdd => parity ^= true,
        }
        let is_filled = fill_is_active(fill_rule, winding, parity);

        if !open {
            push_430c_start_or_merge(&mut crossings, max_end, event.x_min_fixed);
        }
        max_end = max_end.max(event.x_max_fixed);

        if is_filled {
            open = true;
        } else if open || crossings.len() % 2 != 0 {
            crossings.push(max_end + 1);
            open = false;
        }
    }

    if crossings.len() % 2 != 0 {
        crossings.push(max_end + 1);
    }

    crossings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectedEdgeEvent {
    x_min_fixed: i32,
    x_max_fixed: i32,
    winding_delta: i8,
}

#[derive(Debug, Clone)]
struct NativeDebugEdgeNode {
    contour_id: usize,
    input_index: usize,
    edge: AreActiveEdge,
    sibling_next: Option<usize>,
    row_next: Option<usize>,
}

#[derive(Debug, Clone)]
struct NativeDebugContourState {
    contour_id: usize,
    last: usize,
}

fn native_debug_stage_edges(edges: &[AreNativeDebugEdgeInput]) -> Vec<NativeDebugEdgeNode> {
    let mut nodes = Vec::with_capacity(edges.len());
    let mut contours: Vec<NativeDebugContourState> = Vec::new();

    for (input_index, edge_input) in edges.iter().enumerate() {
        let node_index = nodes.len();
        nodes.push(NativeDebugEdgeNode {
            contour_id: edge_input.contour_id,
            input_index,
            edge: edge_input.edge.clone(),
            sibling_next: None,
            row_next: None,
        });

        if let Some(contour) = contours
            .iter_mut()
            .find(|contour| contour.contour_id == edge_input.contour_id)
        {
            nodes[contour.last].sibling_next = Some(node_index);
            contour.last = node_index;
        } else {
            contours.push(NativeDebugContourState {
                contour_id: edge_input.contour_id,
                last: node_index,
            });
        }
    }

    nodes
}

fn native_debug_contour_starts(edges: &[NativeDebugEdgeNode]) -> Vec<usize> {
    let mut starts: Vec<usize> = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        if edges
            .iter()
            .all(|candidate| candidate.sibling_next != Some(index))
            && !starts
                .iter()
                .any(|start| edges[*start].contour_id == edge.contour_id)
        {
            starts.push(index);
        }
    }
    starts
}

fn native_debug_projected_events_for_subrow(
    fixed_subrow_y: i32,
    subrow_index: usize,
    staged_edges: &[NativeDebugEdgeNode],
    contour_starts: &[usize],
    bucket_callback_policy: AreNativeBucketCallbackPolicy,
) -> (Vec<(usize, ProjectedEdgeEvent)>, AreNativeDebugSubrowTrace) {
    let mut row_edges = staged_edges.to_vec();
    let mut bucket_head = None;
    let mut row_event_overrides = vec![None; row_edges.len()];
    let mut sibling_consumed = vec![false; row_edges.len()];
    let mut bucket_insert_edge_indices = Vec::new();

    for start in contour_starts {
        let mut current = Some(*start);
        let mut guard = 0usize;
        while let Some(edge_index) = current {
            guard += 1;
            if guard > row_edges.len() {
                break;
            }
            current = row_edges[edge_index].sibling_next;
            if sibling_consumed[edge_index]
                || row_edges[edge_index]
                    .edge
                    .projected_x_span_for_fixed_strip(fixed_subrow_y)
                    .is_none()
            {
                continue;
            }

            let inserted = native_debug_726c_insert_edge(
                edge_index,
                fixed_subrow_y,
                &mut row_edges,
                &mut row_event_overrides,
                &mut sibling_consumed,
                &mut bucket_head,
                bucket_callback_policy,
            );
            bucket_insert_edge_indices.push(row_edges[inserted].input_index);
        }
    }

    let mut bucket_drain_edge_indices = Vec::new();
    let mut native_events: Vec<(usize, ProjectedEdgeEvent)> = Vec::new();
    let mut current = bucket_head;
    let mut guard = 0usize;
    while let Some(edge_index) = current {
        guard += 1;
        if guard > row_edges.len() {
            break;
        }
        current = row_edges[edge_index].row_next;
        bucket_drain_edge_indices.push(row_edges[edge_index].input_index);
        let Some(event) = native_debug_projected_event(
            edge_index,
            fixed_subrow_y,
            &row_edges,
            &row_event_overrides,
        ) else {
            continue;
        };
        let insert_at = native_events
            .iter()
            .position(|(_, existing)| existing.x_min_fixed >= event.x_min_fixed)
            .unwrap_or(native_events.len());
        native_events.insert(insert_at, (row_edges[edge_index].input_index, event));
    }

    let flat_sorted_edge_indices = flat_sorted_projected_event_order(fixed_subrow_y, staged_edges);
    let native_active_edge_indices = native_events
        .iter()
        .map(|(input_index, _)| *input_index)
        .collect();

    (
        native_events,
        AreNativeDebugSubrowTrace {
            subrow_index,
            fixed_subrow_y,
            bucket_insert_edge_indices,
            bucket_drain_edge_indices,
            native_active_edge_indices,
            flat_sorted_edge_indices,
            unresolved_726c_callback_slots: bucket_callback_policy
                .uses_unresolved_native_vtable_slots(),
        },
    )
}

fn native_debug_726c_insert_edge(
    edge_index: usize,
    fixed_subrow_y: i32,
    row_edges: &mut [NativeDebugEdgeNode],
    row_event_overrides: &mut [Option<ProjectedEdgeEvent>],
    sibling_consumed: &mut [bool],
    bucket_head: &mut Option<usize>,
    bucket_callback_policy: AreNativeBucketCallbackPolicy,
) -> usize {
    let mut insert_index = edge_index;
    let mut insert_event =
        native_debug_projected_event(insert_index, fixed_subrow_y, row_edges, row_event_overrides)
            .expect("caller filters non-projecting edges");

    if bucket_callback_policy == AreNativeBucketCallbackPolicy::OverscanSampler726cCallbacks {
        let row_y = fixed_subrow_y.div_euclid(ARE_FIXED_SUBPIXEL_SCALE);
        while native_debug_726c_can_propagate_to_sibling(&row_edges[insert_index].edge, row_y) {
            let Some(sibling_index) = row_edges[insert_index].sibling_next else {
                break;
            };
            let Some(sibling_event) = native_debug_projected_event(
                sibling_index,
                fixed_subrow_y,
                row_edges,
                row_event_overrides,
            ) else {
                break;
            };
            sibling_consumed[insert_index] = true;
            insert_event = native_debug_b6c0_propagate_range(sibling_event, insert_event);
            row_event_overrides[sibling_index] = Some(insert_event);
            insert_index = sibling_index;
        }
    }

    row_edges[insert_index].row_next = None;
    sibling_consumed[insert_index] = true;
    row_event_overrides[insert_index] = Some(insert_event);
    native_debug_726c_insert_into_bucket(
        insert_index,
        row_edges,
        row_event_overrides,
        bucket_head,
        bucket_callback_policy,
    );
    insert_index
}

fn native_debug_726c_insert_into_bucket(
    edge_index: usize,
    row_edges: &mut [NativeDebugEdgeNode],
    row_event_overrides: &[Option<ProjectedEdgeEvent>],
    bucket_head: &mut Option<usize>,
    bucket_callback_policy: AreNativeBucketCallbackPolicy,
) {
    let Some(head_index) = *bucket_head else {
        *bucket_head = Some(edge_index);
        return;
    };

    if bucket_callback_policy == AreNativeBucketCallbackPolicy::OverscanSampler726cCallbacks {
        let edge_x_min = row_event_overrides[edge_index]
            .expect("inserted native debug edge has a projected event")
            .x_min_fixed;
        let head_x_min = row_event_overrides[head_index]
            .expect("bucket head has a projected event")
            .x_min_fixed;
        if edge_x_min > head_x_min {
            let mut previous = head_index;
            while let Some(next) = row_edges[previous].row_next {
                let next_x_min = row_event_overrides[next]
                    .expect("bucket member has a projected event")
                    .x_min_fixed;
                if next_x_min >= edge_x_min {
                    break;
                }
                previous = next;
            }
            row_edges[edge_index].row_next = row_edges[previous].row_next;
            row_edges[previous].row_next = Some(edge_index);
            return;
        }
    }

    row_edges[edge_index].row_next = *bucket_head;
    *bucket_head = Some(edge_index);
}

fn native_debug_projected_event(
    edge_index: usize,
    fixed_subrow_y: i32,
    row_edges: &[NativeDebugEdgeNode],
    row_event_overrides: &[Option<ProjectedEdgeEvent>],
) -> Option<ProjectedEdgeEvent> {
    if let Some(event) = row_event_overrides[edge_index] {
        return Some(event);
    }
    row_edges[edge_index]
        .edge
        .projected_x_span_for_fixed_strip(fixed_subrow_y)
        .map(|(x_min_fixed, x_max_fixed)| ProjectedEdgeEvent {
            x_min_fixed,
            x_max_fixed,
            winding_delta: row_edges[edge_index].edge.winding_delta,
        })
}

fn native_debug_726c_can_propagate_to_sibling(edge: &AreActiveEdge, row_y: i32) -> bool {
    let start_floor = edge.start_y_fixed.div_euclid(ARE_FIXED_SUBPIXEL_SCALE);
    let end_floor = edge.end_y_fixed.div_euclid(ARE_FIXED_SUBPIXEL_SCALE);
    start_floor == end_floor || end_floor == row_y
}

fn native_debug_b6c0_propagate_range(
    mut sibling_event: ProjectedEdgeEvent,
    current_event: ProjectedEdgeEvent,
) -> ProjectedEdgeEvent {
    sibling_event.x_min_fixed = sibling_event.x_min_fixed.min(current_event.x_min_fixed);
    sibling_event.x_max_fixed = sibling_event.x_max_fixed.max(current_event.x_max_fixed);
    sibling_event
}

fn flat_sorted_projected_event_order(
    fixed_subrow_y: i32,
    staged_edges: &[NativeDebugEdgeNode],
) -> Vec<usize> {
    let mut events: Vec<(usize, ProjectedEdgeEvent)> = staged_edges
        .iter()
        .filter_map(|edge| {
            edge.edge
                .projected_x_span_for_fixed_strip(fixed_subrow_y)
                .map(|(x_min_fixed, x_max_fixed)| {
                    (
                        edge.input_index,
                        ProjectedEdgeEvent {
                            x_min_fixed,
                            x_max_fixed,
                            winding_delta: edge.edge.winding_delta,
                        },
                    )
                })
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

fn push_430c_start_or_merge(crossings: &mut Vec<i32>, previous_max_end: i32, start: i32) {
    if crossings.is_empty() || previous_max_end.saturating_add(1) < start {
        crossings.push(start);
    } else {
        crossings.pop();
    }
}

fn validate_crossing_list(list: &AreSubrowCrossingList) -> Result<(), AreCrossingListError> {
    if list
        .entries
        .last()
        .is_none_or(|entry| entry.x_fixed != ARE_CROSSING_SENTINEL)
    {
        return Err(AreCrossingListError::MissingSentinel {
            subrow_index: list.subrow_index,
        });
    }

    let mut previous = None;
    for entry in &list.entries {
        if entry.x_fixed == ARE_CROSSING_SENTINEL {
            break;
        }
        if let Some(previous_x_fixed) = previous {
            if entry.x_fixed < previous_x_fixed {
                return Err(AreCrossingListError::NonMonotonicCrossing {
                    subrow_index: list.subrow_index,
                    previous_x_fixed,
                    current_x_fixed: entry.x_fixed,
                });
            }
        }
        previous = Some(entry.x_fixed);
    }

    Ok(())
}

#[cfg(test)]
mod crossing_list_tests {
    use super::*;

    fn vertical_rectangle_edges() -> Vec<AreActiveEdge> {
        vec![
            AreActiveEdge::line_pixels(2, 4, 2, 5, 1),
            AreActiveEdge::line_pixels(6, 4, 6, 5, -1),
        ]
    }

    fn diagonal_band_edges() -> Vec<AreActiveEdge> {
        vec![
            AreActiveEdge::line_fixed(0, 0, 16, 16, 1),
            AreActiveEdge::line_fixed(16, 0, 32, 16, -1),
        ]
    }

    #[test]
    fn vertical_edge_pair_builds_crossing_entries() {
        let lists = build_crossing_lists_for_row(
            4,
            &vertical_rectangle_edges(),
            AreFillRule::NonZeroWinding,
        )
        .unwrap();

        assert_eq!(lists.lists.len(), ARE_SUBROW_COUNT);
        assert_eq!(lists.list(0).unwrap().fixed_subrow_y, 64);
        assert_eq!(
            lists.list(0).unwrap().crossing_values(),
            vec![32, 97, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            lists.list(15).unwrap().crossing_values(),
            vec![32, 97, ARE_CROSSING_SENTINEL]
        );
    }

    #[test]
    fn diagonal_edge_pair_builds_subrow_crossings() {
        let lists =
            build_crossing_lists_for_row(0, &diagonal_band_edges(), AreFillRule::NonZeroWinding)
                .unwrap();

        assert_eq!(
            lists.list(0).unwrap().crossing_values(),
            vec![0, 18, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            lists.list(15).unwrap().crossing_values(),
            vec![15, 33, ARE_CROSSING_SENTINEL]
        );
    }

    #[test]
    fn nonzero_winding_accumulates_filled_region() {
        let lists = build_crossing_lists_for_row(
            4,
            &vertical_rectangle_edges(),
            AreFillRule::NonZeroWinding,
        )
        .unwrap();

        let inside = accumulate_75d0_for_sample_x(&lists, 3).unwrap();
        let outside = accumulate_75d0_for_sample_x(&lists, 1).unwrap();

        assert_eq!(inside.coverage.value_0x264, ARE_FULL_COVERAGE_0X264);
        assert_eq!(outside.coverage.value_0x264, 0);
    }

    #[test]
    fn evenodd_parity_accumulates_filled_region() {
        let edges = vec![
            AreActiveEdge::line_pixels(0, 0, 0, 1, 1),
            AreActiveEdge::line_pixels(2, 0, 2, 1, 1),
            AreActiveEdge::line_pixels(4, 0, 4, 1, 1),
            AreActiveEdge::line_pixels(6, 0, 6, 1, 1),
        ];
        let lists = build_crossing_lists_for_row(0, &edges, AreFillRule::EvenOdd).unwrap();

        assert_eq!(
            lists.list(0).unwrap().crossing_values(),
            vec![0, 33, 64, 97, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            accumulate_75d0_for_sample_x(&lists, 1)
                .unwrap()
                .coverage
                .value_0x264,
            ARE_FULL_COVERAGE_0X264
        );
        assert_eq!(
            accumulate_75d0_for_sample_x(&lists, 3)
                .unwrap()
                .coverage
                .value_0x264,
            0
        );
        assert_eq!(
            accumulate_75d0_for_sample_x(&lists, 5)
                .unwrap()
                .coverage
                .value_0x264,
            ARE_FULL_COVERAGE_0X264
        );
    }

    #[test]
    fn line_based_multi_contour_overlap_preserves_fill_rule_difference() {
        let edges = vec![
            AreActiveEdge::line_pixels(0, 0, 0, 1, 1),
            AreActiveEdge::line_pixels(2, 0, 2, 1, -1),
            AreActiveEdge::line_pixels(1, 0, 1, 1, 1),
            AreActiveEdge::line_pixels(3, 0, 3, 1, -1),
        ];
        let nonzero = build_crossing_lists_for_row(0, &edges, AreFillRule::NonZeroWinding).unwrap();
        let evenodd = build_crossing_lists_for_row(0, &edges, AreFillRule::EvenOdd).unwrap();

        assert_eq!(
            nonzero.list(0).unwrap().crossing_values(),
            vec![0, 49, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            evenodd.list(0).unwrap().crossing_values(),
            vec![0, 17, 32, 49, ARE_CROSSING_SENTINEL]
        );
    }

    #[test]
    fn touching_spans_merge_before_75d0_toggle_accumulation() {
        let edges = vec![
            AreActiveEdge::line_pixels(0, 0, 0, 1, 1),
            AreActiveEdge::line_pixels(1, 0, 1, 1, -1),
            AreActiveEdge::line_pixels(1, 0, 1, 1, 1),
            AreActiveEdge::line_pixels(2, 0, 2, 1, -1),
        ];
        let lists = build_crossing_lists_for_row(0, &edges, AreFillRule::NonZeroWinding).unwrap();

        assert_eq!(
            lists.list(0).unwrap().crossing_values(),
            vec![0, 33, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            accumulate_75d0_for_sample_x(&lists, 1)
                .unwrap()
                .coverage
                .value_0x264,
            ARE_FULL_COVERAGE_0X264
        );
    }

    #[test]
    fn native_debug_mirror_tracks_bucket_order_separately_from_flat_sort() {
        let input = AreNativeDebugCrossingInput::source_owned_debug(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(0, 0, 0, 1, 1)),
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(0, 0, 0, 1, -1)),
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(2, 0, 2, 1, -1)),
            ],
        );

        let comparison = compare_native_debug_and_flat_crossing_lists_for_row(&input).unwrap();
        let trace = &comparison.subrow_traces[0];

        assert_eq!(trace.bucket_insert_edge_indices, vec![0, 1, 2]);
        assert_eq!(trace.bucket_drain_edge_indices, vec![1, 0, 2]);
        assert_eq!(trace.native_active_edge_indices, vec![0, 1, 2]);
        assert_eq!(trace.flat_sorted_edge_indices, vec![1, 0, 2]);
        assert_eq!(
            comparison.ordering_delta_subrows,
            (0..ARE_SUBROW_COUNT).collect::<Vec<_>>()
        );
        assert!(comparison.differing_subrows.is_empty());
    }

    #[test]
    fn native_debug_mirror_resolves_726c_callback_policy_by_default() {
        let input = AreNativeDebugCrossingInput::source_owned_debug(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(0, 0, 0, 1, 1)),
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(1, 0, 1, 1, -1)),
            ],
        );

        let comparison = compare_native_debug_and_flat_crossing_lists_for_row(&input).unwrap();

        assert!(!comparison.unresolved_726c_callback_slots);
        assert!(comparison
            .subrow_traces
            .iter()
            .all(|trace| !trace.unresolved_726c_callback_slots));
    }

    #[test]
    fn native_debug_mirror_can_still_record_unresolved_726c_fallback_mode() {
        let input = AreNativeDebugCrossingInput::source_owned_debug(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(0, 0, 0, 1, 1)),
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(1, 0, 1, 1, -1)),
            ],
        )
        .with_bucket_callback_policy(
            AreNativeBucketCallbackPolicy::NativeVtableSlotsUnresolvedFallbackHeadPrepend,
        );

        let comparison = compare_native_debug_and_flat_crossing_lists_for_row(&input).unwrap();

        assert!(comparison.unresolved_726c_callback_slots);
        assert!(comparison
            .subrow_traces
            .iter()
            .all(|trace| trace.unresolved_726c_callback_slots));
    }

    #[test]
    fn native_debug_mirror_applies_b6c0_sibling_range_propagation() {
        let input = AreNativeDebugCrossingInput::source_owned_debug(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_fixed(0, 0, 0, 8, 1)),
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_fixed(32, 0, 32, 8, -1)),
            ],
        );

        let output = build_native_debug_crossing_lists_for_row(&input).unwrap();
        let trace = &output.subrow_traces[0];

        assert_eq!(trace.bucket_insert_edge_indices, vec![1]);
        assert_eq!(trace.bucket_drain_edge_indices, vec![1]);
        assert_eq!(trace.native_active_edge_indices, vec![1]);
        assert_eq!(
            output.crossing_lists.list(0).unwrap().crossing_values(),
            vec![0, 33, ARE_CROSSING_SENTINEL]
        );
    }

    #[test]
    fn native_debug_mirror_crossing_lists_remain_75d0_compatible() {
        let input = AreNativeDebugCrossingInput::source_owned_debug(
            0,
            AreFillRule::NonZeroWinding,
            vec![
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(0, 0, 0, 1, 1)),
                AreNativeDebugEdgeInput::new(0, AreActiveEdge::line_pixels(2, 0, 2, 1, -1)),
            ],
        )
        .with_bucket_callback_policy(AreNativeBucketCallbackPolicy::FallbackHeadPrependOnly);

        let output = build_native_debug_crossing_lists_for_row(&input).unwrap();
        let accumulated = accumulate_75d0_for_sample_x(&output.crossing_lists, 1).unwrap();

        assert_eq!(accumulated.coverage.value_0x264, ARE_FULL_COVERAGE_0X264);
        assert!(output
            .subrow_traces
            .iter()
            .all(|trace| !trace.unresolved_726c_callback_slots));
    }

    #[test]
    fn sentinel_terminates_subrow_list() {
        let lists = build_crossing_lists_for_row(
            4,
            &vertical_rectangle_edges(),
            AreFillRule::NonZeroWinding,
        )
        .unwrap();

        assert!(lists
            .lists
            .iter()
            .all(|list| list.entries.last().unwrap().x_fixed == ARE_CROSSING_SENTINEL));
    }

    #[test]
    fn subrow_accumulator_matches_75d0_formula() {
        let lists =
            build_crossing_lists_for_row(0, &diagonal_band_edges(), AreFillRule::NonZeroWinding)
                .unwrap();
        let accumulated = accumulate_75d0_for_sample_x(&lists, 0).unwrap();

        assert_eq!(accumulated.sample_x_fixed_start, 0);
        assert_eq!(accumulated.sample_x_fixed_end, 16);
        assert_eq!(accumulated.coverage.value_0x264, 136);
        assert_eq!(accumulated.coverage.class2_payload_byte(), Some(136));
    }

    #[test]
    fn payload_byte_matches_ctx_264_domain() {
        let lists =
            build_crossing_lists_for_row(0, &diagonal_band_edges(), AreFillRule::NonZeroWinding)
                .unwrap();
        let partial = accumulate_75d0_for_sample_x(&lists, 0).unwrap();
        let full = accumulate_75d0_for_sample_x(
            &build_crossing_lists_for_row(
                4,
                &vertical_rectangle_edges(),
                AreFillRule::NonZeroWinding,
            )
            .unwrap(),
            3,
        )
        .unwrap();

        assert_eq!(partial.coverage.class2_payload_byte(), Some(136));
        assert_eq!(full.coverage.value_0x264, 0x100);
        assert_eq!(full.coverage.class2_payload_byte(), None);
    }

    #[test]
    fn sample_x_axis_not_row_y_axis() {
        let edges = vec![
            AreActiveEdge::line_pixels(10, 4, 10, 5, 1),
            AreActiveEdge::line_pixels(12, 4, 12, 5, -1),
        ];
        let lists = build_crossing_lists_for_row(4, &edges, AreFillRule::NonZeroWinding).unwrap();

        assert_eq!(lists.row_y, 4);
        assert_eq!(
            accumulate_75d0_for_sample_x(&lists, 4)
                .unwrap()
                .coverage
                .value_0x264,
            0
        );
        assert_eq!(
            accumulate_75d0_for_sample_x(&lists, 10)
                .unwrap()
                .coverage
                .value_0x264,
            ARE_FULL_COVERAGE_0X264
        );
    }

    #[test]
    fn curve_requires_control_points_guard() {
        assert_eq!(
            super::curve_requires_control_points(),
            Err(AreCrossingListError::CurveRequiresControlPoints)
        );
    }

    #[test]
    fn no_typed_span_or_coverage_row_fallback() {
        let typed = AreCrossingListInput::typed_span_fallback(
            4,
            AreFillRule::NonZeroWinding,
            vertical_rectangle_edges(),
        );
        let coverage = AreCrossingListInput::coverage_row_fallback(
            4,
            AreFillRule::NonZeroWinding,
            vertical_rectangle_edges(),
        );

        assert_eq!(
            build_crossing_lists_for_row_from_input(&typed),
            Err(AreCrossingListError::TypedSpanFallbackRejected)
        );
        assert_eq!(
            build_crossing_lists_for_row_from_input(&coverage),
            Err(AreCrossingListError::CoverageRowFallbackRejected)
        );
    }
}
