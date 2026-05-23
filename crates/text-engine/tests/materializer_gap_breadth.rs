use std::{collections::BTreeMap, fs, path::Path};

use serde::Serialize;
use text_engine::{
    build_95cc_interval_list, build_e854_working_set,
    build_glyph_run_source_path_fixture_from_font_bytes, materialize_95cc_intervals,
    AreMaterializedInterval, AreSamplerIntervalList, AreSamplerIntervalRow,
    AreSamplerListWorkingState, AreSamplerMaterializationError, AreSamplerMaterializerInput,
    AreSourcePathProvenance, AreZeroWidthIntervalPolicy,
};

const FONT_PATH: &str = "fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf";
const FONT_LABEL: &str = "Point-Light.ttf";
const FONT_SIZE: f32 = 32.0;

const LATIN_RUNS: &[&str] = &["Il", "AV", "To", "HH", "VA", "oo", "gg"];
const CYRILLIC_RUNS: &[&str] = &["Пр", "ве", "ет", "ЖА", "ДА", "Йо", "ЩИ", "ФО", "юя"];
const MIXED_RUNS: &[&str] = &["AЖ", "IЙ", "OО", "AVЖ", "TestПр"];
const WHITESPACE_RUNS: &[&str] = &["A A", "А А", "I I", "Ж Ж", "A  A", "Пр и"];
const PUNCTUATION_RUNS: &[&str] = &[
    "A.", "A,", "A!", "A?", "Пр.", "Ё!", "Ж?", "\"A\"", "(A)", "А-Б",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum GapClass {
    MaterializedOk,
    EmptyOutlineAdvanceOnly,
    MultiRecordMergedIntervalPayloadComposition,
    DiagonalSegmentSampling,
    CurveSampling,
    TinyContourPayloadMath,
    PunctuationTinyContour,
    CyrillicComplexContour,
    MultiGlyphOverlapOrGap,
    Class0StateTransition,
    Class1StateTransition,
    StateBoundaryTransition,
    PayloadMathUnknown,
    InvalidInput,
}

#[derive(Debug, Clone, Serialize)]
struct GapRecord {
    text: String,
    category: &'static str,
    row_y: i32,
    run_index: usize,
    current_x: i32,
    next_x: i32,
    source_record_indices: Vec<usize>,
    interval_width: i32,
    expected_payload_len: usize,
    gap_class: GapClass,
}

#[derive(Debug, Clone, Serialize)]
struct SupportMap {
    generated: &'static str,
    decision: &'static str,
    selected_expansion: &'static str,
    gap_counts: BTreeMap<GapClass, usize>,
    records: Vec<GapRecord>,
    notes: Vec<&'static str>,
}

fn font_bytes() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FONT_PATH);
    fs::read(path).expect("Point-Light fixture font must exist")
}

fn all_runs() -> Vec<(&'static str, &'static str)> {
    LATIN_RUNS
        .iter()
        .copied()
        .map(|text| (text, "latin"))
        .chain(CYRILLIC_RUNS.iter().copied().map(|text| (text, "cyrillic")))
        .chain(MIXED_RUNS.iter().copied().map(|text| (text, "mixed")))
        .chain(
            WHITESPACE_RUNS
                .iter()
                .copied()
                .map(|text| (text, "whitespace")),
        )
        .chain(
            PUNCTUATION_RUNS
                .iter()
                .copied()
                .map(|text| (text, "punctuation")),
        )
        .collect()
}

fn single_run_materializer_input(
    base: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
) -> AreSamplerMaterializerInput {
    let row = base.interval_list.row(row_y).unwrap();
    let run = row.runs[run_index].clone();
    AreSamplerMaterializerInput::source_owned(
        base.source_input.clone(),
        base.working_set.clone(),
        AreSamplerIntervalList {
            provenance: AreSourcePathProvenance::SourceOwned,
            y_min: base.interval_list.y_min,
            y_max: base.interval_list.y_max,
            rows: vec![AreSamplerIntervalRow {
                row_y,
                boundaries: Vec::new(),
                runs: vec![run],
                is_empty_sentinel: false,
            }],
            source_record_count: base.interval_list.source_record_count,
            working_state: AreSamplerListWorkingState {
                current_record_index: base.interval_list.working_state.current_record_index,
                emitted_boundary_count: 0,
                emitted_run_count: 1,
                skipped_zero_width_record_count: base
                    .interval_list
                    .working_state
                    .skipped_zero_width_record_count,
            },
            zero_width_policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
            zero_width_records: base.interval_list.zero_width_records.clone(),
        },
    )
}

fn classify_materialized(interval: &AreMaterializedInterval, category: &'static str) -> GapClass {
    if interval.payload.is_some() {
        return GapClass::MaterializedOk;
    }
    if category == "whitespace" {
        GapClass::EmptyOutlineAdvanceOnly
    } else {
        GapClass::StateBoundaryTransition
    }
}

fn classify_error(
    error: AreSamplerMaterializationError,
    category: &'static str,
    source_record_indices: &[usize],
) -> GapClass {
    match error {
        AreSamplerMaterializationError::UnsupportedMultiRecordRun { .. } => {
            GapClass::MultiRecordMergedIntervalPayloadComposition
        }
        AreSamplerMaterializationError::UnsupportedNonHorizontalRecord { .. } => {
            if category == "punctuation" {
                GapClass::PunctuationTinyContour
            } else if category == "cyrillic" || category == "mixed" {
                GapClass::CyrillicComplexContour
            } else if source_record_indices.len() > 1 {
                GapClass::MultiGlyphOverlapOrGap
            } else {
                GapClass::DiagonalSegmentSampling
            }
        }
        AreSamplerMaterializationError::UnsupportedCurveRequiresControlPoints { .. } => {
            if category == "cyrillic" || category == "mixed" {
                GapClass::CyrillicComplexContour
            } else if category == "punctuation" {
                GapClass::PunctuationTinyContour
            } else {
                GapClass::CurveSampling
            }
        }
        AreSamplerMaterializationError::UnsupportedAmbiguousLineGeometry { .. } => {
            if category == "cyrillic" || category == "mixed" {
                GapClass::CyrillicComplexContour
            } else if category == "punctuation" {
                GapClass::PunctuationTinyContour
            } else if source_record_indices.len() > 1 {
                GapClass::MultiGlyphOverlapOrGap
            } else {
                GapClass::DiagonalSegmentSampling
            }
        }
        AreSamplerMaterializationError::CrossingList(_) => GapClass::PayloadMathUnknown,
        AreSamplerMaterializationError::MissingSourceSegment { .. } => GapClass::CurveSampling,
        AreSamplerMaterializationError::SourceRecordDoesNotCoverRun { .. }
        | AreSamplerMaterializationError::SourceRecordsDoNotCoverPixel { .. } => {
            GapClass::PayloadMathUnknown
        }
        _ => GapClass::InvalidInput,
    }
}

fn write_support_map(map: &SupportMap) {
    let output_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/ae_agents/p6_5p_materializer_gap_breadth_20260519");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(
        output_dir.join("P6_5P_UPDATED_SUPPORT_MAP.json"),
        serde_json::to_string_pretty(map).unwrap(),
    )
    .unwrap();
}

#[test]
fn p6_5p_materializer_gap_breadth_support_map() {
    let font_bytes = font_bytes();
    let mut records = Vec::new();

    for (text, category) in all_runs() {
        let fixture = build_glyph_run_source_path_fixture_from_font_bytes(
            FONT_LABEL,
            &font_bytes,
            text,
            FONT_SIZE,
        )
        .unwrap();
        let Some(source_input) = fixture.input else {
            records.push(GapRecord {
                text: text.to_string(),
                category,
                row_y: 0,
                run_index: 0,
                current_x: 0,
                next_x: 0,
                source_record_indices: Vec::new(),
                interval_width: 0,
                expected_payload_len: 0,
                gap_class: GapClass::EmptyOutlineAdvanceOnly,
            });
            continue;
        };
        let working_set = build_e854_working_set(&source_input).unwrap();
        let interval_list = build_95cc_interval_list(&working_set).unwrap();
        let base_input = AreSamplerMaterializerInput::source_owned(
            source_input,
            working_set,
            interval_list.clone(),
        );
        for row in &interval_list.rows {
            for (run_index, run) in row.runs.iter().enumerate() {
                let single_input = single_run_materializer_input(&base_input, row.row_y, run_index);
                let gap_class = match materialize_95cc_intervals(&single_input) {
                    Ok(materialized) => classify_materialized(&materialized.intervals[0], category),
                    Err(error) => classify_error(error, category, &run.source_record_indices),
                };
                records.push(GapRecord {
                    text: text.to_string(),
                    category,
                    row_y: row.row_y,
                    run_index,
                    current_x: run.current_x,
                    next_x: run.next_x,
                    source_record_indices: run.source_record_indices.clone(),
                    interval_width: run.width(),
                    expected_payload_len: run.width().max(0) as usize,
                    gap_class,
                });
            }
        }
    }

    let mut gap_counts = BTreeMap::new();
    for record in &records {
        *gap_counts.entry(record.gap_class.clone()).or_insert(0) += 1;
    }

    let support_map = SupportMap {
        generated: "2026-05-19",
        decision: "multi_record_horizontal_payload_composition_implemented",
        selected_expansion: "multi_record_merged_interval_payload_composition",
        gap_counts,
        records,
        notes: vec![
            "Horizontal merged source-record runs are now materialized from ordered source records.",
            "Diagonal, curve, tiny punctuation, and native state transitions remain explicit gaps.",
            "No typed-span, coverage-row, fixture-payload, or synthetic fallback is accepted.",
        ],
    };

    assert!(!support_map.records.is_empty());
    write_support_map(&support_map);
}
