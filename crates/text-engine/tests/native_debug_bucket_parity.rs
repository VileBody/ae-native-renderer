use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use text_engine::{
    build_glyph_run_source_path_fixture_from_font_bytes,
    compare_native_debug_bucket_parity_for_run, AreNativeBucketParitySpan, ArePathVerb,
    AreSamplerIntervalTag,
};

const FONT_PATH: &str = "fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf";
const FONT_LABEL: &str = "Point-Light.ttf";
const FONT_SIZE: f32 = 32.0;

const PARITY_CASES: &[(&str, &str)] = &[
    ("COV_O", "O"),
    ("TXT_030", "GLYPH MOTION"),
    ("LATIN_S", "S"),
    ("CYRILLIC_O", "О"),
    ("CYRILLIC_F", "Ф"),
    ("CYRILLIC_ZH", "Ж"),
    ("MIXED_OO", "OО"),
    ("MIXED_AZH", "AЖ"),
    ("PUNCT_DOT", "."),
];

#[derive(Debug, Clone, Serialize)]
struct P6_10iBucketParityMap {
    generated: &'static str,
    task: &'static str,
    decision_target: &'static str,
    cases: Vec<P6_10iBucketParityEntry>,
    summary: P6_10iBucketParitySummary,
}

#[derive(Debug, Clone, Serialize)]
struct P6_10iBucketParityEntry {
    case_id: String,
    text: String,
    row_y: Option<i32>,
    run_index: Option<usize>,
    current_x: Option<i32>,
    next_x: Option<i32>,
    source_record_indices: Vec<usize>,
    source_verbs: Vec<ArePathVerb>,
    route: &'static str,
    flat_class: Option<&'static str>,
    native_debug_class: Option<&'static str>,
    flat_payload_len: Option<usize>,
    native_debug_payload_len: Option<usize>,
    native_debug_edge_count: Option<usize>,
    ordering_delta_subrows: Vec<usize>,
    crossing_delta_subrows: Vec<usize>,
    span_equal: Option<bool>,
    rejection_reason: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
struct P6_10iBucketParitySummary {
    total_entries: usize,
    compared_runs: usize,
    span_equal_runs: usize,
    span_delta_runs: usize,
    crossing_delta_runs: usize,
    ordering_delta_runs: usize,
    unresolved_726c_callback_runs: usize,
    non_bucket_state_runs: usize,
    unsupported_runs: usize,
    whitespace_advance_only_cases: usize,
    compared_by_case: BTreeMap<String, usize>,
    span_delta_by_case: BTreeMap<String, usize>,
}

fn font_bytes() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FONT_PATH);
    fs::read(path).expect("Point-Light fixture font must exist")
}

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/ae_agents/p6_10ah_430c_84e8_row_event_close_merge_parity_20260525")
}

fn span_class(span: &AreNativeBucketParitySpan) -> &'static str {
    span.class_name()
}

#[test]
fn p6_10i_native_row_event_bucket_parity_corpus() {
    let font_bytes = font_bytes();
    let mut entries = Vec::new();
    let mut summary = P6_10iBucketParitySummary::default();

    for (case_id, text) in PARITY_CASES {
        let fixture = build_glyph_run_source_path_fixture_from_font_bytes(
            FONT_LABEL,
            &font_bytes,
            text,
            FONT_SIZE,
        )
        .unwrap_or_else(|err| panic!("fixture must build for {case_id}: {err:?}"));
        let Some(source_input) = fixture.input.as_ref() else {
            entries.push(P6_10iBucketParityEntry {
                case_id: (*case_id).to_string(),
                text: (*text).to_string(),
                row_y: None,
                run_index: None,
                current_x: None,
                next_x: None,
                source_record_indices: Vec::new(),
                source_verbs: Vec::new(),
                route: "whitespace_advance_only",
                flat_class: None,
                native_debug_class: None,
                flat_payload_len: None,
                native_debug_payload_len: None,
                native_debug_edge_count: None,
                ordering_delta_subrows: Vec::new(),
                crossing_delta_subrows: Vec::new(),
                span_equal: None,
                rejection_reason: None,
            });
            summary.whitespace_advance_only_cases += 1;
            continue;
        };
        let materializer_input =
            text_engine::build_materializer_input_from_source(source_input).unwrap();
        for row in &materializer_input.interval_list.rows {
            for (run_index, run) in row.runs.iter().enumerate() {
                let source_verbs = run
                    .source_record_indices
                    .iter()
                    .filter_map(|record_index| {
                        materializer_input
                            .working_set
                            .records
                            .get(*record_index)
                            .and_then(|record| {
                                materializer_input
                                    .working_set
                                    .source_segment_for_record(record)
                            })
                            .map(|segment| segment.verb)
                    })
                    .collect::<Vec<_>>();

                if !run.materialize_candidate || run.tag != AreSamplerIntervalTag::SourceSpan {
                    entries.push(P6_10iBucketParityEntry {
                        case_id: (*case_id).to_string(),
                        text: (*text).to_string(),
                        row_y: Some(row.row_y),
                        run_index: Some(run_index),
                        current_x: Some(run.current_x),
                        next_x: Some(run.next_x),
                        source_record_indices: run.source_record_indices.clone(),
                        source_verbs,
                        route: "non_bucket_state_run",
                        flat_class: None,
                        native_debug_class: None,
                        flat_payload_len: None,
                        native_debug_payload_len: None,
                        native_debug_edge_count: None,
                        ordering_delta_subrows: Vec::new(),
                        crossing_delta_subrows: Vec::new(),
                        span_equal: None,
                        rejection_reason: None,
                    });
                    summary.non_bucket_state_runs += 1;
                    continue;
                }

                match compare_native_debug_bucket_parity_for_run(
                    &materializer_input,
                    row.row_y,
                    run_index,
                ) {
                    Ok(comparison) => {
                        summary.compared_runs += 1;
                        *summary
                            .compared_by_case
                            .entry((*case_id).to_string())
                            .or_insert(0) += 1;
                        if comparison.span_equal {
                            summary.span_equal_runs += 1;
                        } else {
                            summary.span_delta_runs += 1;
                            *summary
                                .span_delta_by_case
                                .entry((*case_id).to_string())
                                .or_insert(0) += 1;
                        }
                        if !comparison.differing_subrows.is_empty() {
                            summary.crossing_delta_runs += 1;
                        }
                        if !comparison.ordering_delta_subrows.is_empty() {
                            summary.ordering_delta_runs += 1;
                        }
                        if comparison.unresolved_726c_callback_slots {
                            summary.unresolved_726c_callback_runs += 1;
                        }
                        entries.push(P6_10iBucketParityEntry {
                            case_id: (*case_id).to_string(),
                            text: (*text).to_string(),
                            row_y: Some(comparison.row_y),
                            run_index: Some(comparison.run_index),
                            current_x: Some(comparison.current_x),
                            next_x: Some(comparison.next_x),
                            source_record_indices: comparison.source_record_indices.clone(),
                            source_verbs,
                            route: "resolved_native_row_event_bucket_compare",
                            flat_class: Some(span_class(&comparison.flat_span)),
                            native_debug_class: Some(span_class(&comparison.native_debug_span)),
                            flat_payload_len: comparison.flat_span.payload_len(),
                            native_debug_payload_len: comparison.native_debug_span.payload_len(),
                            native_debug_edge_count: Some(comparison.native_debug_edge_count),
                            ordering_delta_subrows: comparison.ordering_delta_subrows,
                            crossing_delta_subrows: comparison.differing_subrows,
                            span_equal: Some(comparison.span_equal),
                            rejection_reason: None,
                        });
                    }
                    Err(err) => {
                        summary.unsupported_runs += 1;
                        entries.push(P6_10iBucketParityEntry {
                            case_id: (*case_id).to_string(),
                            text: (*text).to_string(),
                            row_y: Some(row.row_y),
                            run_index: Some(run_index),
                            current_x: Some(run.current_x),
                            next_x: Some(run.next_x),
                            source_record_indices: run.source_record_indices.clone(),
                            source_verbs,
                            route: "not_bucket_comparable",
                            flat_class: None,
                            native_debug_class: None,
                            flat_payload_len: None,
                            native_debug_payload_len: None,
                            native_debug_edge_count: None,
                            ordering_delta_subrows: Vec::new(),
                            crossing_delta_subrows: Vec::new(),
                            span_equal: None,
                            rejection_reason: Some(format!("{err:?}")),
                        });
                    }
                }
            }
        }
    }

    summary.total_entries = entries.len();
    let map = P6_10iBucketParityMap {
        generated: "2026-05-25",
        task: "P6_10AH_430C_84E8_ROW_EVENT_CLOSE_MERGE_PARITY_001",
        decision_target: "stateful_76dc_row_event_model_supersedes_per_subrow_debug_oracle",
        cases: entries,
        summary,
    };

    let output_dir = output_dir();
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(
        output_dir.join("P6_10I_BUCKET_PARITY_MAP.json"),
        serde_json::to_string_pretty(&map).unwrap(),
    )
    .unwrap();

    assert!(map.summary.compared_runs > 0);
    assert_eq!(map.summary.unresolved_726c_callback_runs, 0);
    assert_eq!(map.summary.span_delta_runs, 34);
    assert_eq!(map.summary.crossing_delta_runs, 36);
    assert_eq!(map.summary.ordering_delta_runs, 65);
}
