use crate::{
    accumulate_75d0_for_sample_x, build_crossing_lists_for_row, AreActiveEdge,
    AreBezierSourcePathInput, AreFillRule, ArePathPoint, ArePathVerb, AreSourcePathProvenance,
    AreSourceRecord32, AreSourceSamplerWorkingSet, AreSourceSegment, AreSourceSegmentKind,
    ARE_FIXED_SUBPIXEL_SCALE,
};
use serde::{Deserialize, Serialize};

const ARE_DB98_CURVE_MAX_DEPTH: u8 = 15;
const ARE_DB98_FLATNESS: f32 = 1.0;
const ARE_DB98_SHORT_CHORD_RATIO: f32 = 0.25;
const ARE_DB98_TOLERANCE_SCALE: f32 = 1.0;
const QUADRATIC_TO_CUBIC_CONTROL_SCALE: f32 = 2.0 / 3.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreSamplerIntervalListInput {
    pub provenance: AreSourcePathProvenance,
    pub working_set: AreSourceSamplerWorkingSet,
}

impl AreSamplerIntervalListInput {
    pub fn source_owned(working_set: AreSourceSamplerWorkingSet) -> Self {
        Self {
            provenance: AreSourcePathProvenance::SourceOwned,
            working_set,
        }
    }

    pub fn typed_span_fallback(working_set: AreSourceSamplerWorkingSet) -> Self {
        Self {
            provenance: AreSourcePathProvenance::TypedSpanFallback,
            working_set,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSamplerIntervalList {
    pub provenance: AreSourcePathProvenance,
    pub y_min: i32,
    pub y_max: i32,
    pub rows: Vec<AreSamplerIntervalRow>,
    pub source_record_count: usize,
    pub working_state: AreSamplerListWorkingState,
    pub zero_width_policy: AreZeroWidthIntervalPolicy,
    pub zero_width_records: Vec<AreZeroWidthIntervalRecord>,
}

impl AreSamplerIntervalList {
    pub fn row(&self, row_y: i32) -> Option<&AreSamplerIntervalRow> {
        self.rows.iter().find(|row| row.row_y == row_y)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSamplerIntervalRow {
    pub row_y: i32,
    pub boundaries: Vec<AreSamplerIntervalBoundary>,
    pub runs: Vec<AreSamplerIntervalRun>,
    pub is_empty_sentinel: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSamplerIntervalRun {
    pub current_x: i32,
    pub next_x: i32,
    pub tag: AreSamplerIntervalTag,
    pub source_record_indices: Vec<usize>,
    pub materialize_candidate: bool,
}

impl AreSamplerIntervalRun {
    pub fn width(&self) -> i32 {
        self.next_x - self.current_x
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSamplerIntervalBoundary {
    pub x: i32,
    pub kind: AreSamplerIntervalBoundaryKind,
    pub tag: AreSamplerIntervalTag,
    pub source_record_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreSamplerIntervalBoundaryKind {
    Start,
    End,
    Sentinel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreSamplerIntervalTag {
    SourceSpan,
    Sentinel,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreZeroWidthIntervalPolicy {
    SkipNonMaterialized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreZeroWidthIntervalRecord {
    pub record_index: usize,
    pub row_y: i32,
    pub x: i32,
    pub tag: AreSamplerIntervalTag,
    pub source_point_index: usize,
    pub policy: AreZeroWidthIntervalPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSamplerListWorkingState {
    pub current_record_index: usize,
    pub emitted_boundary_count: usize,
    pub emitted_run_count: usize,
    pub skipped_zero_width_record_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreIntervalBuildError {
    TypedSpanFallbackRejected,
    RequiresSourceOwnedProvenance {
        provenance: AreSourcePathProvenance,
    },
    EmptyWorkingSet,
    InvalidBounds {
        x_min: i32,
        x_max: i32,
        y_min: i32,
        y_max: i32,
    },
    InvalidIntervalOrder {
        record_index: usize,
        current_x: i32,
        next_x: i32,
    },
    MissingSourceSegment {
        record_index: usize,
        source_point_index: usize,
    },
    UnsupportedCurveRequiresControlPoints {
        record_index: usize,
        source_point_index: usize,
    },
    ScanlineCrossingList {
        row_y: i32,
    },
}

pub fn build_95cc_interval_list(
    working_set: &AreSourceSamplerWorkingSet,
) -> Result<AreSamplerIntervalList, AreIntervalBuildError> {
    build_interval_list(working_set, AreSourcePathProvenance::SourceOwned)
}

pub fn build_95cc_interval_list_from_input(
    input: &AreSamplerIntervalListInput,
) -> Result<AreSamplerIntervalList, AreIntervalBuildError> {
    build_interval_list(&input.working_set, input.provenance)
}

pub fn build_95cc_scanline_interval_list_from_source(
    source_input: &AreBezierSourcePathInput,
    working_set: &AreSourceSamplerWorkingSet,
) -> Result<AreSamplerIntervalList, AreIntervalBuildError> {
    match source_input.provenance {
        AreSourcePathProvenance::SourceOwned
        | AreSourcePathProvenance::SourceOwnedGlyphPath
        | AreSourcePathProvenance::SourceOwnedGlyphRun => {}
        AreSourcePathProvenance::TypedSpanFallback => {
            return Err(AreIntervalBuildError::TypedSpanFallbackRejected);
        }
        provenance => {
            return Err(AreIntervalBuildError::RequiresSourceOwnedProvenance { provenance })
        }
    }
    build_scanline_interval_list_from_source(source_input, working_set)
}

fn build_interval_list(
    working_set: &AreSourceSamplerWorkingSet,
    provenance: AreSourcePathProvenance,
) -> Result<AreSamplerIntervalList, AreIntervalBuildError> {
    if provenance == AreSourcePathProvenance::TypedSpanFallback {
        return Err(AreIntervalBuildError::TypedSpanFallbackRejected);
    }
    if working_set.records.is_empty() {
        return Err(AreIntervalBuildError::EmptyWorkingSet);
    }
    if working_set.bounds.x_max <= working_set.bounds.x_min
        || working_set.bounds.y_max < working_set.bounds.y_min
    {
        return Err(AreIntervalBuildError::InvalidBounds {
            x_min: working_set.bounds.x_min,
            x_max: working_set.bounds.x_max,
            y_min: working_set.bounds.y_min,
            y_max: working_set.bounds.y_max,
        });
    }

    let mut rows = Vec::new();
    let mut zero_width_records = Vec::new();
    let mut emitted_boundary_count = 0;
    let mut emitted_run_count = 0;
    let row_end = if working_set.bounds.y_max == working_set.bounds.y_min {
        working_set.bounds.y_max + 1
    } else {
        working_set.bounds.y_max
    };
    for row_y in working_set.bounds.y_min..row_end {
        let mut row = AreSamplerIntervalRow {
            row_y,
            boundaries: Vec::new(),
            runs: Vec::new(),
            is_empty_sentinel: false,
        };
        let mut row_runs = Vec::new();

        for (record_index, record) in working_set.records.iter().enumerate() {
            let Some(classified) = classify_record_for_row(record_index, record, row_y)? else {
                continue;
            };
            let run = match classified {
                RowRecordClassification::Run(run) => run,
                RowRecordClassification::ZeroWidth(record) => {
                    zero_width_records.push(record);
                    continue;
                }
            };
            row_runs.push(run);
        }

        row_runs.sort_by_key(|run| {
            (
                run.current_x,
                run.next_x,
                interval_tag_sort_key(run.tag),
                run.materialize_candidate,
            )
        });
        for mut run in row_runs {
            if let Some(last) = row.runs.last_mut() {
                if can_merge(last, &run) {
                    last.current_x = last.current_x.min(run.current_x);
                    last.next_x = last.next_x.max(run.next_x);
                    last.source_record_indices
                        .append(&mut run.source_record_indices);
                    continue;
                }
            }
            row.runs.push(run);
        }

        for run in &row.runs {
            row.boundaries.push(AreSamplerIntervalBoundary {
                x: run.current_x,
                kind: AreSamplerIntervalBoundaryKind::Start,
                tag: run.tag,
                source_record_index: run.source_record_indices.first().copied(),
            });
            row.boundaries.push(AreSamplerIntervalBoundary {
                x: run.next_x,
                kind: AreSamplerIntervalBoundaryKind::End,
                tag: run.tag,
                source_record_index: run.source_record_indices.first().copied(),
            });
        }

        if row.runs.is_empty() {
            row.is_empty_sentinel = true;
            row.boundaries.push(AreSamplerIntervalBoundary {
                x: working_set.bounds.x_min,
                kind: AreSamplerIntervalBoundaryKind::Sentinel,
                tag: AreSamplerIntervalTag::Empty,
                source_record_index: None,
            });
        }

        emitted_boundary_count += row.boundaries.len();
        emitted_run_count += row.runs.len();
        rows.push(row);
    }

    Ok(AreSamplerIntervalList {
        provenance,
        y_min: working_set.bounds.y_min,
        y_max: working_set.bounds.y_max,
        rows,
        source_record_count: working_set.records.len(),
        working_state: AreSamplerListWorkingState {
            current_record_index: working_set.records.len(),
            emitted_boundary_count,
            emitted_run_count,
            skipped_zero_width_record_count: zero_width_records.len(),
        },
        zero_width_policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
        zero_width_records,
    })
}

fn build_scanline_interval_list_from_source(
    source_input: &AreBezierSourcePathInput,
    working_set: &AreSourceSamplerWorkingSet,
) -> Result<AreSamplerIntervalList, AreIntervalBuildError> {
    validate_working_set_bounds_and_records(working_set)?;

    let scanline_edges = scanline_active_edges_from_source(source_input, working_set)?;
    let mut rows = Vec::new();
    let mut zero_width_records = Vec::new();
    let mut emitted_boundary_count = 0;
    let mut emitted_run_count = 0;
    let row_end = interval_row_end(working_set);

    for row_y in working_set.bounds.y_min..row_end {
        let mut row = AreSamplerIntervalRow {
            row_y,
            boundaries: Vec::new(),
            runs: Vec::new(),
            is_empty_sentinel: false,
        };

        for (record_index, record) in working_set.records.iter().enumerate() {
            if let Some(RowRecordClassification::ZeroWidth(record)) =
                classify_record_for_row(record_index, record, row_y)?
            {
                zero_width_records.push(record);
            }
        }

        let row_edges = scanline_edges
            .iter()
            .filter(|edge| active_edge_touches_row(&edge.edge, row_y))
            .collect::<Vec<_>>();
        if !row_edges.is_empty() {
            let edges = row_edges
                .iter()
                .map(|edge| edge.edge.clone())
                .collect::<Vec<_>>();
            let crossings =
                build_crossing_lists_for_row(row_y, &edges, AreFillRule::NonZeroWinding)
                    .map_err(|_| AreIntervalBuildError::ScanlineCrossingList { row_y })?;
            let source_record_indices = unique_record_indices(&row_edges);
            let mut row_runs = Vec::new();
            let mut current_start = None;

            for x in working_set.bounds.x_min..working_set.bounds.x_max {
                let coverage = accumulate_75d0_for_sample_x(&crossings, x)
                    .map_err(|_| AreIntervalBuildError::ScanlineCrossingList { row_y })?
                    .coverage
                    .value_0x264;
                if coverage > 0 {
                    current_start.get_or_insert(x);
                } else if let Some(start) = current_start.take() {
                    push_scanline_run(&mut row_runs, start, x, &source_record_indices);
                }
            }
            if let Some(start) = current_start.take() {
                push_scanline_run(
                    &mut row_runs,
                    start,
                    working_set.bounds.x_max,
                    &source_record_indices,
                );
            }

            for run in row_runs {
                push_or_merge_run(&mut row.runs, run);
            }
        }

        for run in &row.runs {
            row.boundaries.push(AreSamplerIntervalBoundary {
                x: run.current_x,
                kind: AreSamplerIntervalBoundaryKind::Start,
                tag: run.tag,
                source_record_index: run.source_record_indices.first().copied(),
            });
            row.boundaries.push(AreSamplerIntervalBoundary {
                x: run.next_x,
                kind: AreSamplerIntervalBoundaryKind::End,
                tag: run.tag,
                source_record_index: run.source_record_indices.first().copied(),
            });
        }

        if row.runs.is_empty() {
            row.is_empty_sentinel = true;
            row.boundaries.push(AreSamplerIntervalBoundary {
                x: working_set.bounds.x_min,
                kind: AreSamplerIntervalBoundaryKind::Sentinel,
                tag: AreSamplerIntervalTag::Empty,
                source_record_index: None,
            });
        }

        emitted_boundary_count += row.boundaries.len();
        emitted_run_count += row.runs.len();
        rows.push(row);
    }

    Ok(AreSamplerIntervalList {
        provenance: AreSourcePathProvenance::SourceOwned,
        y_min: working_set.bounds.y_min,
        y_max: working_set.bounds.y_max,
        rows,
        source_record_count: working_set.records.len(),
        working_state: AreSamplerListWorkingState {
            current_record_index: working_set.records.len(),
            emitted_boundary_count,
            emitted_run_count,
            skipped_zero_width_record_count: zero_width_records.len(),
        },
        zero_width_policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
        zero_width_records,
    })
}

fn validate_working_set_bounds_and_records(
    working_set: &AreSourceSamplerWorkingSet,
) -> Result<(), AreIntervalBuildError> {
    if working_set.records.is_empty() {
        return Err(AreIntervalBuildError::EmptyWorkingSet);
    }
    if working_set.bounds.x_max <= working_set.bounds.x_min
        || working_set.bounds.y_max < working_set.bounds.y_min
    {
        return Err(AreIntervalBuildError::InvalidBounds {
            x_min: working_set.bounds.x_min,
            x_max: working_set.bounds.x_max,
            y_min: working_set.bounds.y_min,
            y_max: working_set.bounds.y_max,
        });
    }
    for (record_index, record) in working_set.records.iter().enumerate() {
        if record.max_x_0x10 < record.min_x_0x08 {
            return Err(AreIntervalBuildError::InvalidIntervalOrder {
                record_index,
                current_x: record.min_x_0x08,
                next_x: record.max_x_0x10,
            });
        }
    }
    Ok(())
}

fn interval_row_end(working_set: &AreSourceSamplerWorkingSet) -> i32 {
    if working_set.bounds.y_max == working_set.bounds.y_min {
        working_set.bounds.y_max + 1
    } else {
        working_set.bounds.y_max
    }
}

struct ScanlineActiveEdge {
    record_index: usize,
    edge: AreActiveEdge,
}

fn scanline_active_edges_from_source(
    source_input: &AreBezierSourcePathInput,
    working_set: &AreSourceSamplerWorkingSet,
) -> Result<Vec<ScanlineActiveEdge>, AreIntervalBuildError> {
    let mut edges = Vec::new();
    for (record_index, record) in working_set.records.iter().enumerate() {
        let Some(segment) = working_set.source_segment_for_record(record) else {
            return Err(AreIntervalBuildError::MissingSourceSegment {
                record_index,
                source_point_index: record.source_point_index_0x00,
            });
        };
        let transformed = segment.transformed(source_input.transform);
        for edge in active_edges_from_segment(record_index, &transformed)? {
            edges.push(ScanlineActiveEdge { record_index, edge });
        }
    }
    Ok(edges)
}

fn active_edges_from_segment(
    record_index: usize,
    segment: &AreSourceSegment,
) -> Result<Vec<AreActiveEdge>, AreIntervalBuildError> {
    let Some(start) = segment.previous_point else {
        return Err(AreIntervalBuildError::MissingSourceSegment {
            record_index,
            source_point_index: segment.source_point_index,
        });
    };
    match segment.verb {
        ArePathVerb::LineTo | ArePathVerb::Close => {
            Ok(push_line_active_edge(start, segment.endpoint)
                .into_iter()
                .collect())
        }
        ArePathVerb::QuadTo | ArePathVerb::CubicTo => {
            active_edges_from_curve_segment(record_index, segment, start)
        }
        ArePathVerb::MoveTo | ArePathVerb::Unknown(_) => Ok(Vec::new()),
    }
}

fn active_edges_from_curve_segment(
    record_index: usize,
    segment: &AreSourceSegment,
    start: ArePathPoint,
) -> Result<Vec<AreActiveEdge>, AreIntervalBuildError> {
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
                AreIntervalBuildError::UnsupportedCurveRequiresControlPoints {
                    record_index,
                    source_point_index: segment.source_point_index,
                },
            );
        }
        _ => {}
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

fn to_fixed(value: f32) -> i32 {
    (value * ARE_FIXED_SUBPIXEL_SCALE as f32).floor() as i32
}

fn active_edge_touches_row(edge: &AreActiveEdge, row_y: i32) -> bool {
    let row_start = row_y * ARE_FIXED_SUBPIXEL_SCALE;
    let row_end = row_start + ARE_FIXED_SUBPIXEL_SCALE;
    let min_y = edge.start_y_fixed.min(edge.end_y_fixed);
    let max_y = edge.start_y_fixed.max(edge.end_y_fixed);
    max_y > row_start && min_y < row_end
}

fn unique_record_indices(row_edges: &[&ScanlineActiveEdge]) -> Vec<usize> {
    let mut indices = Vec::new();
    for edge in row_edges {
        if !indices.contains(&edge.record_index) {
            indices.push(edge.record_index);
        }
    }
    indices
}

fn push_scanline_run(
    row_runs: &mut Vec<AreSamplerIntervalRun>,
    current_x: i32,
    next_x: i32,
    source_record_indices: &[usize],
) {
    if next_x <= current_x || source_record_indices.is_empty() {
        return;
    }
    row_runs.push(AreSamplerIntervalRun {
        current_x,
        next_x,
        tag: AreSamplerIntervalTag::SourceSpan,
        source_record_indices: source_record_indices.to_vec(),
        materialize_candidate: true,
    });
}

fn push_or_merge_run(row_runs: &mut Vec<AreSamplerIntervalRun>, mut run: AreSamplerIntervalRun) {
    if let Some(last) = row_runs.last_mut() {
        if can_merge(last, &run) {
            last.current_x = last.current_x.min(run.current_x);
            last.next_x = last.next_x.max(run.next_x);
            for record_index in run.source_record_indices.drain(..) {
                if !last.source_record_indices.contains(&record_index) {
                    last.source_record_indices.push(record_index);
                }
            }
            return;
        }
    }
    row_runs.push(run);
}

enum RowRecordClassification {
    Run(AreSamplerIntervalRun),
    ZeroWidth(AreZeroWidthIntervalRecord),
}

fn classify_record_for_row(
    record_index: usize,
    record: &AreSourceRecord32,
    row_y: i32,
) -> Result<Option<RowRecordClassification>, AreIntervalBuildError> {
    if record.max_y_0x14 <= record.min_y_0x0c {
        if row_y != record.min_y_0x0c {
            return Ok(None);
        }
    } else if row_y < record.min_y_0x0c || row_y >= record.max_y_0x14 {
        return Ok(None);
    }

    let current_x = record.min_x_0x08;
    let next_x = record.max_x_0x10;
    if next_x < current_x {
        return Err(AreIntervalBuildError::InvalidIntervalOrder {
            record_index,
            current_x,
            next_x,
        });
    }

    let tag = if record.record_flag_0x18 == AreSourceRecord32::SENTINEL_FLAG {
        AreSamplerIntervalTag::Sentinel
    } else {
        AreSamplerIntervalTag::SourceSpan
    };

    if next_x == current_x {
        return Ok(Some(RowRecordClassification::ZeroWidth(
            AreZeroWidthIntervalRecord {
                record_index,
                row_y,
                x: current_x,
                tag,
                source_point_index: record.source_point_index_0x00,
                policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
            },
        )));
    }

    Ok(Some(RowRecordClassification::Run(AreSamplerIntervalRun {
        current_x,
        next_x,
        tag,
        source_record_indices: vec![record_index],
        materialize_candidate: tag == AreSamplerIntervalTag::SourceSpan,
    })))
}

fn can_merge(left: &AreSamplerIntervalRun, right: &AreSamplerIntervalRun) -> bool {
    left.tag == right.tag
        && left.materialize_candidate == right.materialize_candidate
        && left.next_x >= right.current_x
}

fn interval_tag_sort_key(tag: AreSamplerIntervalTag) -> u8 {
    match tag {
        AreSamplerIntervalTag::SourceSpan => 0,
        AreSamplerIntervalTag::Sentinel => 1,
        AreSamplerIntervalTag::Empty => 2,
    }
}

#[cfg(test)]
mod interval_95cc_tests {
    use super::*;
    use crate::{
        build_e854_working_set, AreBezierSourcePathInput, ArePathPoint, ArePathTransform,
        ArePathVerb, AreSourceBounds,
    };

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
        bounds: AreSourceBounds,
        records: Vec<AreSourceRecord32>,
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

    fn rectangle_source(x0: f32, y0: f32, x1: f32, y1: f32) -> AreBezierSourcePathInput {
        AreBezierSourcePathInput::source_owned_glyph_path(
            vec![
                ArePathPoint::new(x0, y0),
                ArePathPoint::new(x1, y0),
                ArePathPoint::new(x1, y1),
                ArePathPoint::new(x0, y1),
                ArePathPoint::new(x0, y0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
                ArePathVerb::Close,
            ],
            ArePathTransform::identity(),
        )
    }

    #[test]
    fn single_record_interval_list() {
        let working_set = working_set(
            bounds(0, 4, 20, 5),
            vec![record(3, 4, 9, 5, AreSourceRecord32::NORMAL_FLAG, 0)],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(4).unwrap();

        assert_eq!(list.source_record_count, 1);
        assert_eq!(row.runs.len(), 1);
        assert_eq!(row.runs[0].current_x, 3);
        assert_eq!(row.runs[0].next_x, 9);
        assert_eq!(row.runs[0].tag, AreSamplerIntervalTag::SourceSpan);
        assert!(row.runs[0].materialize_candidate);
    }

    #[test]
    fn multiple_e854_records_preserve_order() {
        let working_set = working_set(
            bounds(0, 0, 30, 1),
            vec![
                record(2, 0, 5, 1, AreSourceRecord32::NORMAL_FLAG, 1),
                record(8, 0, 12, 1, AreSourceRecord32::NORMAL_FLAG, 2),
            ],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(0).unwrap();

        assert_eq!(
            row.runs
                .iter()
                .map(|run| run.source_record_indices[0])
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(row.runs[0].current_x, 2);
        assert_eq!(row.runs[1].current_x, 8);
    }

    #[test]
    fn line_segment_interval_start_end() {
        let source = AreBezierSourcePathInput::source_owned(
            vec![ArePathPoint::new(1.2, 2.0), ArePathPoint::new(5.7, 2.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        );
        let working_set = build_e854_working_set(&source).unwrap();

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(2).unwrap();

        assert_eq!(row.runs[0].current_x, 1);
        assert_eq!(row.runs[0].next_x, 6);
        assert_eq!(row.runs[0].width(), 5);
    }

    #[test]
    fn scanline_95cc_fills_between_active_edges() {
        let source = rectangle_source(1.0, 0.0, 5.0, 3.0);
        let working_set = build_e854_working_set(&source).unwrap();
        let legacy = build_95cc_interval_list(&working_set).unwrap();
        let scanline =
            build_95cc_scanline_interval_list_from_source(&source, &working_set).unwrap();

        assert!(legacy.row(1).unwrap().runs.is_empty());
        let row = scanline.row(1).unwrap();
        assert_eq!(row.runs.len(), 1);
        assert_eq!(row.runs[0].current_x, 1);
        assert!(row.runs[0].next_x >= 5);
        assert_eq!(row.runs[0].tag, AreSamplerIntervalTag::SourceSpan);
        assert!(row.runs[0].materialize_candidate);
        assert!(row.runs[0].source_record_indices.len() >= 2);
    }

    #[test]
    fn scanline_95cc_splits_disjoint_filled_spans() {
        let mut left = rectangle_source(0.0, 0.0, 2.0, 2.0);
        let right = rectangle_source(5.0, 0.0, 7.0, 2.0);
        let point_offset = left.points.len();
        let segment_offset = left.segments.len();
        left.points.extend(right.points);
        left.verbs.extend(right.verbs);
        left.segments
            .extend(right.segments.into_iter().map(|mut segment| {
                segment.contour_id = crate::AreSourceContourId(segment.contour_id.0 + 1);
                segment.segment_id =
                    crate::AreSourceSegmentId(segment.segment_id.0 + segment_offset);
                segment.source_point_index += point_offset;
                segment
            }));
        left.segments = left
            .segments
            .into_iter()
            .enumerate()
            .map(|(index, mut segment)| {
                segment.source_point_index = index;
                segment
            })
            .collect();

        let working_set = build_e854_working_set(&left).unwrap();
        let scanline = build_95cc_scanline_interval_list_from_source(&left, &working_set).unwrap();
        let row = scanline.row(1).unwrap();

        assert_eq!(row.runs.len(), 2);
        assert_eq!(row.runs[0].current_x, 0);
        assert!(row.runs[0].next_x < row.runs[1].current_x);
        assert_eq!(row.runs[1].current_x, 5);
    }

    #[test]
    fn scanline_95cc_keeps_open_horizontal_source_empty() {
        let source = AreBezierSourcePathInput::source_owned_glyph_path(
            vec![ArePathPoint::new(1.0, 2.0), ArePathPoint::new(5.0, 2.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
            ArePathTransform::identity(),
        );
        let working_set = build_e854_working_set(&source).unwrap();
        let scanline =
            build_95cc_scanline_interval_list_from_source(&source, &working_set).unwrap();

        assert!(scanline.row(2).unwrap().runs.is_empty());
        assert!(scanline.row(2).unwrap().is_empty_sentinel);
    }

    #[test]
    fn interval_helper_merges_adjacent_runs() {
        let working_set = working_set(
            bounds(0, 0, 20, 1),
            vec![
                record(2, 0, 5, 1, AreSourceRecord32::NORMAL_FLAG, 0),
                record(5, 0, 8, 1, AreSourceRecord32::NORMAL_FLAG, 1),
            ],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(0).unwrap();

        assert_eq!(row.runs.len(), 1);
        assert_eq!(row.runs[0].current_x, 2);
        assert_eq!(row.runs[0].next_x, 8);
        assert_eq!(row.runs[0].source_record_indices, vec![0, 1]);
    }

    #[test]
    fn interval_helper_sorts_record_runs_before_merging() {
        let working_set = working_set(
            bounds(0, 0, 120, 1),
            vec![
                record(72, 0, 109, 1, AreSourceRecord32::NORMAL_FLAG, 0),
                record(0, 0, 16, 1, AreSourceRecord32::NORMAL_FLAG, 1),
            ],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(0).unwrap();

        assert_eq!(row.runs.len(), 2);
        assert_eq!((row.runs[0].current_x, row.runs[0].next_x), (0, 16));
        assert_eq!((row.runs[1].current_x, row.runs[1].next_x), (72, 109));
        assert_eq!(row.runs[0].source_record_indices, vec![1]);
        assert_eq!(row.runs[1].source_record_indices, vec![0]);
    }

    #[test]
    fn sentinel_empty_interval_row() {
        let working_set = working_set(
            bounds(0, 0, 20, 3),
            vec![record(2, 0, 5, 1, AreSourceRecord32::NORMAL_FLAG, 0)],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(2).unwrap();

        assert!(row.is_empty_sentinel);
        assert!(row.runs.is_empty());
        assert_eq!(
            row.boundaries[0].kind,
            AreSamplerIntervalBoundaryKind::Sentinel
        );
        assert_eq!(row.boundaries[0].tag, AreSamplerIntervalTag::Empty);
    }

    #[test]
    fn multi_row_interval_topology() {
        let working_set = working_set(
            bounds(0, 0, 20, 4),
            vec![
                record(1, 0, 6, 2, AreSourceRecord32::NORMAL_FLAG, 0),
                record(3, 2, 9, 4, AreSourceRecord32::SENTINEL_FLAG, 1),
            ],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();

        assert_eq!(list.rows.len(), 4);
        assert_eq!(
            list.row(0).unwrap().runs[0].tag,
            AreSamplerIntervalTag::SourceSpan
        );
        assert_eq!(
            list.row(1).unwrap().runs[0].tag,
            AreSamplerIntervalTag::SourceSpan
        );
        assert_eq!(
            list.row(2).unwrap().runs[0].tag,
            AreSamplerIntervalTag::Sentinel
        );
        assert!(!list.row(2).unwrap().runs[0].materialize_candidate);
    }

    #[test]
    fn rejects_working_set_without_records() {
        let working_set = working_set(bounds(0, 0, 10, 1), Vec::new());

        assert_eq!(
            build_95cc_interval_list(&working_set),
            Err(AreIntervalBuildError::EmptyWorkingSet)
        );
    }

    #[test]
    fn rejects_invalid_interval_order() {
        let working_set = working_set(
            bounds(0, 0, 10, 1),
            vec![record(8, 0, 7, 1, AreSourceRecord32::NORMAL_FLAG, 0)],
        );

        assert_eq!(
            build_95cc_interval_list(&working_set),
            Err(AreIntervalBuildError::InvalidIntervalOrder {
                record_index: 0,
                current_x: 8,
                next_x: 7,
            })
        );
    }

    #[test]
    fn zero_width_record_classified() {
        let working_set = working_set(
            bounds(0, 0, 10, 1),
            vec![record(7, 0, 7, 1, AreSourceRecord32::NORMAL_FLAG, 4)],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();

        assert_eq!(
            list.zero_width_policy,
            AreZeroWidthIntervalPolicy::SkipNonMaterialized
        );
        assert_eq!(
            list.zero_width_records,
            vec![AreZeroWidthIntervalRecord {
                record_index: 0,
                row_y: 0,
                x: 7,
                tag: AreSamplerIntervalTag::SourceSpan,
                source_point_index: 4,
                policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
            }]
        );
        assert_eq!(list.working_state.skipped_zero_width_record_count, 1);
    }

    #[test]
    fn zero_width_record_does_not_create_class2_payload() {
        let working_set = working_set(
            bounds(0, 0, 10, 1),
            vec![record(4, 0, 4, 1, AreSourceRecord32::NORMAL_FLAG, 0)],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(0).unwrap();

        assert!(row.runs.is_empty());
        assert!(row.is_empty_sentinel);
        assert_eq!(row.boundaries.len(), 1);
        assert_eq!(row.boundaries[0].tag, AreSamplerIntervalTag::Empty);
        assert_eq!(list.zero_width_records.len(), 1);
    }

    #[test]
    fn zero_width_record_preserves_neighbor_order() {
        let working_set = working_set(
            bounds(0, 0, 20, 1),
            vec![
                record(1, 0, 3, 1, AreSourceRecord32::NORMAL_FLAG, 0),
                record(5, 0, 5, 1, AreSourceRecord32::NORMAL_FLAG, 1),
                record(8, 0, 11, 1, AreSourceRecord32::NORMAL_FLAG, 2),
            ],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();
        let row = list.row(0).unwrap();

        assert_eq!(
            row.runs
                .iter()
                .map(|run| run.source_record_indices[0])
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(list.zero_width_records[0].record_index, 1);
    }

    #[test]
    fn punctuation_zero_width_not_silently_dropped() {
        let working_set = working_set(
            bounds(0, 0, 10, 1),
            vec![record(2, 0, 2, 1, AreSourceRecord32::SENTINEL_FLAG, 9)],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();

        assert!(list.row(0).unwrap().runs.is_empty());
        assert_eq!(list.zero_width_records.len(), 1);
        assert_eq!(
            list.zero_width_records[0].tag,
            AreSamplerIntervalTag::Sentinel
        );
        assert_eq!(list.zero_width_records[0].source_point_index, 9);
    }

    #[test]
    fn typed_span_fallback_rejected() {
        let working_set = working_set(
            bounds(0, 0, 10, 1),
            vec![record(1, 0, 4, 1, AreSourceRecord32::NORMAL_FLAG, 0)],
        );
        let input = AreSamplerIntervalListInput::typed_span_fallback(working_set);

        assert_eq!(
            build_95cc_interval_list_from_input(&input),
            Err(AreIntervalBuildError::TypedSpanFallbackRejected)
        );
    }

    #[test]
    fn no_descriptor_cursor_integration() {
        let working_set = working_set(
            bounds(0, 0, 10, 1),
            vec![record(1, 0, 4, 1, AreSourceRecord32::NORMAL_FLAG, 0)],
        );

        let list_json =
            serde_json::to_value(build_95cc_interval_list(&working_set).unwrap()).unwrap();

        assert!(list_json.get("descriptor_id").is_none());
        assert!(list_json.get("descriptor_ref").is_none());
    }

    #[test]
    fn no_renderer_integration() {
        let working_set = working_set(
            bounds(0, 0, 10, 1),
            vec![record(1, 0, 4, 1, AreSourceRecord32::NORMAL_FLAG, 0)],
        );

        let list = build_95cc_interval_list(&working_set).unwrap();

        assert_eq!(list.working_state.current_record_index, 1);
        assert_eq!(list.y_min, 0);
        assert_eq!(list.y_max, 1);
    }
}
