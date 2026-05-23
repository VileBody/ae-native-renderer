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

    fn x_at_fixed_subrow(&self, fixed_subrow_y: i32) -> Option<i32> {
        if self.start_y_fixed == self.end_y_fixed {
            return None;
        }
        let y_min = self.start_y_fixed.min(self.end_y_fixed);
        let y_max = self.start_y_fixed.max(self.end_y_fixed);
        if fixed_subrow_y < y_min || fixed_subrow_y >= y_max {
            return None;
        }
        let dy = self.end_y_fixed - self.start_y_fixed;
        let dx = self.end_x_fixed - self.start_x_fixed;
        let local_y = fixed_subrow_y - self.start_y_fixed;
        let x = self.start_x_fixed as f32 + (local_y as f32 * dx as f32) / dy as f32;
        Some(x.floor() as i32)
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
    let mut events: Vec<(i32, i8)> = edges
        .iter()
        .filter_map(|edge| {
            edge.x_at_fixed_subrow(fixed_subrow_y)
                .map(|x_fixed| (x_fixed, edge.winding_delta))
        })
        .collect();
    events.sort_by_key(|(x_fixed, delta)| (*x_fixed, *delta));

    let mut crossings = Vec::new();
    let mut winding = 0i16;
    let mut parity = false;
    let mut start_x: Option<i32> = None;

    for (x_fixed, delta) in events {
        let was_filled = fill_is_active(fill_rule, winding, parity);
        match fill_rule {
            AreFillRule::NonZeroWinding => winding += delta as i16,
            AreFillRule::EvenOdd => parity ^= true,
        }
        let is_filled = fill_is_active(fill_rule, winding, parity);

        match (was_filled, is_filled) {
            (false, true) => start_x = Some(x_fixed),
            (true, false) => {
                if let Some(start) = start_x.take() {
                    crossings.push(start);
                    crossings.push(x_fixed);
                }
            }
            _ => {}
        }
    }

    crossings
}

fn fill_is_active(fill_rule: AreFillRule, winding: i16, parity: bool) -> bool {
    match fill_rule {
        AreFillRule::NonZeroWinding => winding != 0,
        AreFillRule::EvenOdd => parity,
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
            vec![32, 96, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            lists.list(15).unwrap().crossing_values(),
            vec![32, 96, ARE_CROSSING_SENTINEL]
        );
    }

    #[test]
    fn diagonal_edge_pair_builds_subrow_crossings() {
        let lists =
            build_crossing_lists_for_row(0, &diagonal_band_edges(), AreFillRule::NonZeroWinding)
                .unwrap();

        assert_eq!(
            lists.list(0).unwrap().crossing_values(),
            vec![0, 16, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            lists.list(15).unwrap().crossing_values(),
            vec![15, 31, ARE_CROSSING_SENTINEL]
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
            vec![0, 32, 64, 96, ARE_CROSSING_SENTINEL]
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
            vec![0, 48, ARE_CROSSING_SENTINEL]
        );
        assert_eq!(
            evenodd.list(0).unwrap().crossing_values(),
            vec![0, 16, 32, 48, ARE_CROSSING_SENTINEL]
        );
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
