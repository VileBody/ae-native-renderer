use crate::{AreSourcePathProvenance, AreSourceRecord32, AreSourceSamplerWorkingSet};
use serde::{Deserialize, Serialize};

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

        for (record_index, record) in working_set.records.iter().enumerate() {
            let Some(classified) = classify_record_for_row(record_index, record, row_y)? else {
                continue;
            };
            let mut run = match classified {
                RowRecordClassification::Run(run) => run,
                RowRecordClassification::ZeroWidth(record) => {
                    zero_width_records.push(record);
                    continue;
                }
            };
            if let Some(last) = row.runs.last_mut() {
                if can_merge(last, &run) {
                    last.next_x = last.next_x.max(run.next_x);
                    last.source_record_indices
                        .append(&mut run.source_record_indices);
                    continue;
                }
            }
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
            row.runs.push(run);
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

#[cfg(test)]
mod interval_95cc_tests {
    use super::*;
    use crate::{
        build_e854_working_set, AreBezierSourcePathInput, ArePathPoint, ArePathVerb,
        AreSourceBounds,
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
