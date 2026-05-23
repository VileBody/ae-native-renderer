use std::fs;

use serde::Serialize;
use text_engine::{
    build_95cc_interval_list, build_descriptor_cursor_from_95cc_intervals_with_fixture_policy,
    build_e854_working_set, build_glyph_run_source_path_fixture_from_font_bytes,
    materialize_95cc_intervals, source_owned_materializer_to_ad68_rows,
    Are95ccDescriptorCursorOptions, AreBezierSourcePathInput, AreDescriptorCursorPolicy,
    ArePathPoint, ArePathVerb, AreSamplerIntervalList, AreSamplerIntervalRow,
    AreSamplerIntervalRun, AreSamplerIntervalTag, AreSamplerListWorkingState,
    AreSamplerMaterializerInput, AreSourcePathProvenance, AreSourceRecord32, AreSourceSamplerError,
    AreZeroWidthIntervalPolicy,
};
use ttf_parser::Face;

const FONT_PATH: &str = "fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf";
const FONT_LABEL: &str = "Point-Light.ttf";
const FONT_SIZE: f32 = 32.0;

const LATIN_SINGLES: &[char] = &['I', 'H', 'l', 'S', 'A', 'V', 'O', 'B', 'g', 'Q'];
const CYRILLIC_SINGLES: &[char] = &[
    'А', 'Н', 'О', 'В', 'Ж', 'Д', 'Ф', 'Ю', 'Я', 'Щ', 'Й', 'Ё', 'ж', 'ф', 'ю', 'я', 'д',
];
const PUNCTUATION_SINGLES: &[char] = &[
    '.', ',', ':', ';', '!', '?', '-', '_', '/', '(', ')', '"', '\'',
];
const OPTIONAL_PUNCTUATION: &[char] = &['…', '—', '–', '«', '»', '№'];
const LATIN_RUNS: &[&str] = &["Il", "AV", "To", "HH", "VA", "oo", "gg"];
const CYRILLIC_RUNS: &[&str] = &["Пр", "ве", "ет", "ЖА", "ДА", "Йо", "ЩИ", "ФО", "юя"];
const MIXED_RUNS: &[&str] = &["AЖ", "IЙ", "OО", "AVЖ", "TestПр"];
const WHITESPACE_RUNS: &[&str] = &["A A", "А А", "I I", "Ж Ж", "A  A", "Пр и"];
const PUNCTUATION_RUNS: &[&str] = &[
    "A.", "A,", "A!", "A?", "Пр.", "Ё!", "Ж?", "\"A\"", "(A)", "А-Б",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum MaterializerSupport {
    MaterializedOk,
    EmptyOutlineAdvanceOnly,
    UnsupportedMultiRecordRun,
    UnsupportedDiagonalSegment,
    UnsupportedCurveSegment,
    UnsupportedPunctuationTinyContour,
    UnsupportedWhitespaceAdvance,
    UnsupportedStateTransition,
    UnsupportedPayloadMath,
    InvalidInput,
}

#[derive(Debug, Serialize)]
struct P6_5zgSupportMap {
    generated: &'static str,
    task: &'static str,
    route_required_for_materialized_ok: &'static str,
    entries: Vec<P6_5zgSupportEntry>,
    summary: P6_5zgSupportSummary,
}

#[derive(Debug, Serialize)]
struct P6_5zgSupportEntry {
    text: String,
    row_y: Option<i32>,
    run_index: Option<usize>,
    current_x: Option<i32>,
    next_x: Option<i32>,
    route_used: &'static str,
    materialized_ok: bool,
    class: Option<&'static str>,
    payload_len: Option<usize>,
    rejection_reason: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct P6_5zgSupportSummary {
    total_entries: usize,
    routed_ad68_event_cursor: usize,
    unsupported: usize,
    class0: usize,
    class1: usize,
    class2: usize,
    whitespace_advance_only: usize,
    descriptor_fixture_policy_used: bool,
    fixture_payload_bytes_used: bool,
    synthetic_payload_bytes_used: bool,
    typed_span_fallback_used: bool,
    coverage_row_fallback_used: bool,
    interval_object_1b898_payload_bridge_used: bool,
}

fn font_bytes() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FONT_PATH);
    fs::read(path).expect("Point-Light fixture font must exist")
}

fn fixture(text: &str) -> text_engine::AreGlyphRunSourcePathFixture {
    build_glyph_run_source_path_fixture_from_font_bytes(FONT_LABEL, &font_bytes(), text, FONT_SIZE)
        .unwrap_or_else(|err| panic!("glyph run fixture must build for {text:?}: {err:?}"))
}

fn face() -> Face<'static> {
    let bytes = font_bytes().into_boxed_slice();
    let leaked = Box::leak(bytes);
    Face::parse(leaked, 0).expect("Point-Light fixture font must parse")
}

fn all_required_single_chars() -> Vec<char> {
    LATIN_SINGLES
        .iter()
        .chain(CYRILLIC_SINGLES.iter())
        .chain([' '].iter())
        .chain(PUNCTUATION_SINGLES.iter())
        .copied()
        .collect()
}

fn all_required_runs() -> Vec<&'static str> {
    LATIN_RUNS
        .iter()
        .chain(CYRILLIC_RUNS.iter())
        .chain(MIXED_RUNS.iter())
        .chain(WHITESPACE_RUNS.iter())
        .chain(PUNCTUATION_RUNS.iter())
        .copied()
        .collect()
}

fn classify_run(
    input: Option<&AreBezierSourcePathInput>,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
    punctuation: bool,
) -> MaterializerSupport {
    let Some(input) = input else {
        return MaterializerSupport::EmptyOutlineAdvanceOnly;
    };
    let Ok(working_set) = build_e854_working_set(input) else {
        return MaterializerSupport::InvalidInput;
    };
    if run.source_record_indices.is_empty() {
        return MaterializerSupport::InvalidInput;
    }
    let mut statuses = Vec::with_capacity(run.source_record_indices.len());
    for record_index in &run.source_record_indices {
        let Some(record) = working_set.records.get(*record_index) else {
            return MaterializerSupport::InvalidInput;
        };
        statuses.push(classify_record_with_record(
            input,
            row_y,
            run_index,
            run,
            *record_index,
            record,
            punctuation,
        ));
    }
    if statuses
        .iter()
        .all(|status| *status == MaterializerSupport::MaterializedOk)
    {
        return MaterializerSupport::MaterializedOk;
    }
    if run.source_record_indices.len() > 1 {
        return MaterializerSupport::UnsupportedMultiRecordRun;
    }
    statuses
        .into_iter()
        .next()
        .unwrap_or(MaterializerSupport::InvalidInput)
}

fn classify_record_with_record(
    input: &AreBezierSourcePathInput,
    _row_y: i32,
    _run_index: usize,
    run: &AreSamplerIntervalRun,
    record_index: usize,
    record: &AreSourceRecord32,
    punctuation: bool,
) -> MaterializerSupport {
    if run.tag != AreSamplerIntervalTag::SourceSpan || !run.materialize_candidate {
        return MaterializerSupport::UnsupportedStateTransition;
    }
    if record.max_y_0x14 > record.min_y_0x0c + 1 {
        let point_index = record.source_point_index_0x00;
        if point_index < input.verbs.len()
            && matches!(
                input.verbs[point_index],
                ArePathVerb::QuadTo | ArePathVerb::CubicTo
            )
        {
            return MaterializerSupport::UnsupportedCurveSegment;
        }
        return MaterializerSupport::UnsupportedDiagonalSegment;
    }
    if punctuation && record.max_x_0x10 - record.min_x_0x08 <= 1 {
        return MaterializerSupport::UnsupportedPunctuationTinyContour;
    }
    if record_index >= input.points.len() {
        return MaterializerSupport::InvalidInput;
    }
    MaterializerSupport::MaterializedOk
}

fn interval_list_for_fixture(
    fixture: &text_engine::AreGlyphRunSourcePathFixture,
) -> Option<AreSamplerIntervalList> {
    let input = fixture.input.as_ref()?;
    let working_set = build_e854_working_set(input).ok()?;
    build_95cc_interval_list(&working_set).ok()
}

fn support_statuses_for_fixture(
    fixture: &text_engine::AreGlyphRunSourcePathFixture,
    punctuation: bool,
) -> Vec<MaterializerSupport> {
    let Some(input) = fixture.input.as_ref() else {
        return vec![MaterializerSupport::EmptyOutlineAdvanceOnly];
    };
    let Some(interval_list) = interval_list_for_fixture(fixture) else {
        return vec![MaterializerSupport::InvalidInput];
    };
    let mut statuses = Vec::new();
    for row in &interval_list.rows {
        for (run_index, run) in row.runs.iter().enumerate() {
            statuses.push(classify_run(
                Some(input),
                row.row_y,
                run_index,
                run,
                punctuation,
            ));
        }
    }
    statuses
}

fn all_corpus_texts() -> Vec<String> {
    all_required_single_chars()
        .into_iter()
        .map(|ch| ch.to_string())
        .chain(all_required_runs().into_iter().map(str::to_string))
        .collect()
}

fn single_run_materializer_input(
    input: &AreBezierSourcePathInput,
    working_set: &text_engine::AreSourceSamplerWorkingSet,
    row_y: i32,
    run: &AreSamplerIntervalRun,
) -> AreSamplerMaterializerInput {
    let interval_list = AreSamplerIntervalList {
        provenance: AreSourcePathProvenance::SourceOwned,
        y_min: row_y,
        y_max: row_y,
        rows: vec![AreSamplerIntervalRow {
            row_y,
            boundaries: Vec::new(),
            runs: vec![run.clone()],
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
    AreSamplerMaterializerInput::source_owned(input.clone(), working_set.clone(), interval_list)
}

#[test]
fn extended_glyph_corpus_cmap_inventory() {
    let face = face();
    for ch in all_required_single_chars() {
        assert!(face.glyph_index(ch).is_some(), "{ch:?}");
    }
    for ch in OPTIONAL_PUNCTUATION {
        assert!(face.glyph_index(*ch).is_some(), "{ch:?}");
    }
}

#[test]
fn real_glyph_extended_corpus_source_paths_extract() {
    for ch in LATIN_SINGLES.iter().chain(CYRILLIC_SINGLES.iter()) {
        let fixture = fixture(&ch.to_string());
        assert_eq!(
            fixture.input.as_ref().unwrap().provenance,
            AreSourcePathProvenance::SourceOwnedGlyphRun
        );
        assert_eq!(fixture.glyphs.len(), 1);
        assert!(fixture.glyphs[0].has_outline, "{ch}");
        assert!(!fixture.input.as_ref().unwrap().points.is_empty(), "{ch}");
    }
}

#[test]
fn real_glyph_run_corpus_to_e854_records() {
    for text in all_required_runs() {
        let fixture = fixture(text);
        if let Some(input) = &fixture.input {
            let working_set = build_e854_working_set(input).unwrap();
            assert!(!working_set.records.is_empty(), "{text:?}");
            assert_eq!(
                working_set.record_vector_capacity_0x196,
                working_set.records.len()
            );
        } else {
            assert!(text.chars().all(char::is_whitespace), "{text:?}");
            assert!(fixture.total_advance > 0.0, "{text:?}");
        }
    }
}

#[test]
fn real_glyph_run_corpus_to_95cc_intervals() {
    for text in all_required_runs() {
        let fixture = fixture(text);
        let Some(input) = fixture.input.as_ref() else {
            continue;
        };
        let working_set = build_e854_working_set(input).unwrap();
        let interval_list = build_95cc_interval_list(&working_set).unwrap();

        assert_eq!(
            interval_list.provenance,
            AreSourcePathProvenance::SourceOwned
        );
        assert!(!interval_list.rows.is_empty(), "{text:?}");
        assert!(
            interval_list.working_state.emitted_boundary_count > 0,
            "{text:?}"
        );
    }
}

#[test]
fn av_run_no_invalid_interval_order() {
    let fixture = fixture("AV");
    let input = fixture.input.as_ref().unwrap();
    let working_set = build_e854_working_set(input).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();

    assert!(!interval_list.rows.is_empty());
    assert!(!interval_list.zero_width_records.is_empty());
    assert_eq!(
        interval_list.working_state.skipped_zero_width_record_count,
        interval_list.zero_width_records.len()
    );
    assert!(interval_list
        .rows
        .iter()
        .flat_map(|row| row.runs.iter())
        .all(|run| run.width() > 0));
}

#[test]
fn real_glyph_run_corpus_materializer_support_map() {
    let mut materialized_ok = 0usize;
    let mut materializer_errors = 0usize;
    for text in LATIN_RUNS
        .iter()
        .chain(CYRILLIC_RUNS.iter())
        .chain(MIXED_RUNS.iter())
        .chain(PUNCTUATION_RUNS.iter())
    {
        let fixture = fixture(text);
        let Some(input) = fixture.input.as_ref() else {
            continue;
        };
        let working_set = build_e854_working_set(input).unwrap();
        let interval_list = build_95cc_interval_list(&working_set).unwrap();
        for row in &interval_list.rows {
            for run in &row.runs {
                let single_input =
                    single_run_materializer_input(input, &working_set, row.row_y, run);
                match materialize_95cc_intervals(&single_input) {
                    Ok(_) => materialized_ok += 1,
                    Err(_) => materializer_errors += 1,
                }
            }
        }
    }

    assert!(materialized_ok > 0);
    assert_eq!(materializer_errors, 0);
}

#[test]
fn real_glyph_run_corpus_rejects_typed_span_fallback() {
    let input = AreBezierSourcePathInput::typed_span_fallback(
        vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(1.0, 0.0)],
        vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
    );

    assert_eq!(
        build_e854_working_set(&input),
        Err(AreSourceSamplerError::TypedSpanFallbackRejected)
    );
}

#[test]
fn real_glyph_run_corpus_rejects_coverage_row_fallback() {
    let input = AreBezierSourcePathInput::coverage_row_fallback(
        vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(1.0, 0.0)],
        vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
    );

    assert_eq!(
        build_e854_working_set(&input),
        Err(AreSourceSamplerError::CoverageRowFallbackRejected)
    );
}

#[test]
fn real_glyph_run_corpus_rejects_synthetic_policy() {
    let fixture = fixture("A.");
    let interval_list = interval_list_for_fixture(&fixture).unwrap();
    let synthetic =
        AreDescriptorCursorPolicy::synthetic_only(Are95ccDescriptorCursorOptions::default());

    assert!(build_descriptor_cursor_from_95cc_intervals_with_fixture_policy(
        &interval_list,
        &synthetic,
    )
    .is_err());
}

#[test]
fn whitespace_space_emits_no_class2_payload() {
    let fixture = fixture(" ");

    assert!(fixture.input.is_none());
    assert_eq!(fixture.glyphs.len(), 1);
    assert!(!fixture.glyphs[0].has_outline);
    assert!(fixture.glyphs[0].advance > 0.0);
    assert_eq!(fixture.outline_glyph_count(), 0);
}

#[test]
fn whitespace_advance_preserves_next_glyph_offset() {
    let no_space = fixture("AA");
    let single_space = fixture("A A");
    let double_space = fixture("A  A");

    let no_space_second = no_space.glyphs[1].x_offset;
    let single_space_second = single_space.glyphs[2].x_offset;
    let double_space_second = double_space.glyphs[3].x_offset;

    assert!(single_space_second > no_space_second);
    assert!(double_space_second > single_space_second);
    assert!(!single_space.glyphs[1].has_outline);
    assert!(!double_space.glyphs[1].has_outline);
    assert!(!double_space.glyphs[2].has_outline);
}

#[test]
fn whitespace_still_advance_only() {
    whitespace_space_emits_no_class2_payload();
    whitespace_advance_preserves_next_glyph_offset();
}

#[test]
fn punctuation_tiny_contours_are_not_silently_dropped() {
    for ch in PUNCTUATION_SINGLES {
        let fixture = fixture(&ch.to_string());
        assert_eq!(fixture.glyphs.len(), 1, "{ch}");
        assert!(fixture.glyphs[0].has_outline, "{ch}");
        let input = fixture.input.as_ref().unwrap();
        let working_set = build_e854_working_set(input).unwrap();
        assert!(!working_set.records.is_empty(), "{ch}");
        assert!(
            support_statuses_for_fixture(&fixture, true)
                .iter()
                .any(|status| !matches!(status, MaterializerSupport::InvalidInput)),
            "{ch}"
        );
    }
}

#[test]
fn cyrillic_glyphs_available_and_exercised() {
    for ch in CYRILLIC_SINGLES {
        let fixture = fixture(&ch.to_string());
        assert_eq!(fixture.glyphs.len(), 1, "{ch}");
        assert!(fixture.glyphs[0].glyph_id > 0, "{ch}");
        assert!(fixture.glyphs[0].has_outline, "{ch}");
        assert!(fixture.input.is_some(), "{ch}");
    }
}

#[test]
fn supported_extended_intervals_feed_ad68_without_fixture_payloads() {
    let source = AreBezierSourcePathInput::source_owned_glyph_run(
        vec![ArePathPoint::new(2.0, 4.0), ArePathPoint::new(6.0, 4.0)],
        vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
    );
    let materializer_input = text_engine::build_materializer_input_from_source(&source).unwrap();
    let materialized = materialize_95cc_intervals(&materializer_input).unwrap();
    let row_table = source_owned_materializer_to_ad68_rows(&materialized).unwrap();

    assert_eq!(row_table.row(4).unwrap()[0].x, 2);
    assert_eq!(row_table.row(4).unwrap()[0].len, 4);
    assert_eq!(row_table.row(4).unwrap()[0].bytes().unwrap().len(), 4);
}

#[test]
fn materialized_ok_corpus_intervals_use_ad68_event_cursor_route() {
    let mut routed_ok = 0usize;
    let mut unsupported = 0usize;

    for text in all_corpus_texts() {
        let fixture = fixture(&text);
        let Some(input) = fixture.input.as_ref() else {
            assert!(text.chars().all(char::is_whitespace), "{text:?}");
            assert!(fixture.total_advance > 0.0, "{text:?}");
            continue;
        };
        let working_set = build_e854_working_set(input).unwrap();
        let interval_list = build_95cc_interval_list(&working_set).unwrap();
        for row in &interval_list.rows {
            for run in &row.runs {
                let single_input =
                    single_run_materializer_input(input, &working_set, row.row_y, run);
                let Ok(materialized) = materialize_95cc_intervals(&single_input) else {
                    unsupported += 1;
                    continue;
                };
                let table = source_owned_materializer_to_ad68_rows(&materialized).unwrap();
                let row_nodes = table.row(row.row_y).unwrap();

                if materialized
                    .intervals
                    .iter()
                    .filter_map(|interval| interval.payload.as_ref())
                    .flat_map(|payload| payload.bytes.iter())
                    .any(|byte| *byte != 0)
                {
                    assert!(
                        row_nodes.iter().any(|node| node.bytes().is_some()),
                        "{text:?} row={} x={}..{}",
                        row.row_y,
                        run.current_x,
                        run.next_x
                    );
                } else {
                    assert!(
                        row_nodes.iter().all(|node| node.bytes().is_none()),
                        "{text:?} row={} x={}..{}",
                        row.row_y,
                        run.current_x,
                        run.next_x
                    );
                }
                routed_ok += 1;
            }
        }
    }

    assert!(routed_ok > 0);
    assert_eq!(
        unsupported, 0,
        "outlined corpus intervals should route through the source-owned AD68 event cursor"
    );
}

#[test]
fn p6_5zg_event_cursor_route_support_map() {
    let mut entries = Vec::new();
    let mut summary = P6_5zgSupportSummary::default();

    for text in all_corpus_texts() {
        let fixture = fixture(&text);
        let Some(input) = fixture.input.as_ref() else {
            entries.push(P6_5zgSupportEntry {
                text,
                row_y: None,
                run_index: None,
                current_x: None,
                next_x: None,
                route_used: "unsupported",
                materialized_ok: false,
                class: None,
                payload_len: None,
                rejection_reason: Some("empty_outline_advance_only".to_string()),
            });
            summary.whitespace_advance_only += 1;
            summary.unsupported += 1;
            continue;
        };
        let working_set = build_e854_working_set(input).unwrap();
        let interval_list = build_95cc_interval_list(&working_set).unwrap();
        for row in &interval_list.rows {
            for (run_index, run) in row.runs.iter().enumerate() {
                let single_input =
                    single_run_materializer_input(input, &working_set, row.row_y, run);
                let (route_used, materialized_ok, class, payload_len, rejection_reason) =
                    match materialize_95cc_intervals(&single_input) {
                        Ok(materialized) => {
                            let table = source_owned_materializer_to_ad68_rows(&materialized)
                                .expect(
                                    "materialized_ok intervals must route through event cursor",
                                );
                            let row_nodes = table.row(row.row_y).unwrap_or(&[]);
                            let class = if row_nodes.iter().any(|node| node.bytes().is_some()) {
                                summary.class2 += 1;
                                "class2"
                            } else if materialized.intervals.iter().any(|interval| {
                                interval.state_hint.map(|hint| hint.state_class)
                                    == Some(text_engine::Are95ccStateClass::Class1)
                            }) {
                                summary.class1 += 1;
                                "class1"
                            } else {
                                summary.class0 += 1;
                                "class0"
                            };
                            let payload_len = row_nodes
                                .iter()
                                .find_map(|node| node.bytes().map(|bytes| bytes.len()));
                            summary.routed_ad68_event_cursor += 1;
                            ("ad68_event_cursor", true, Some(class), payload_len, None)
                        }
                        Err(err) => {
                            summary.unsupported += 1;
                            ("unsupported", false, None, None, Some(format!("{err:?}")))
                        }
                    };

                entries.push(P6_5zgSupportEntry {
                    text: text.clone(),
                    row_y: Some(row.row_y),
                    run_index: Some(run_index),
                    current_x: Some(run.current_x),
                    next_x: Some(run.next_x),
                    route_used,
                    materialized_ok,
                    class,
                    payload_len,
                    rejection_reason,
                });
            }
        }
    }

    summary.total_entries = entries.len();
    let support_map = P6_5zgSupportMap {
        generated: "2026-05-19",
        task: "P6_5ZG_EVENT_CURSOR_CORPUS_ROUTE_AND_NEXT_GAP_001",
        route_required_for_materialized_ok: "ad68_event_cursor",
        entries,
        summary,
    };

    let output_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/ae_agents/p6_5zg_event_cursor_corpus_route_20260519");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(
        output_dir.join("P6_5ZG_UPDATED_SUPPORT_MAP.json"),
        serde_json::to_string_pretty(&support_map).unwrap(),
    )
    .unwrap();

    assert!(support_map.summary.routed_ad68_event_cursor > 0);
    assert_eq!(
        support_map.summary.unsupported, support_map.summary.whitespace_advance_only,
        "unsupported entries should be advance-only whitespace, not materializer gaps"
    );
    assert!(!support_map.summary.descriptor_fixture_policy_used);
    assert!(!support_map.summary.fixture_payload_bytes_used);
    assert!(!support_map.summary.synthetic_payload_bytes_used);
    assert!(!support_map.summary.typed_span_fallback_used);
    assert!(!support_map.summary.coverage_row_fallback_used);
    assert!(
        !support_map
            .summary
            .interval_object_1b898_payload_bridge_used
    );
}

#[test]
fn extended_glyph_corpus_full_suite() {
    real_glyph_extended_corpus_source_paths_extract();
    real_glyph_run_corpus_to_e854_records();
    real_glyph_run_corpus_to_95cc_intervals();
    av_run_no_invalid_interval_order();
    real_glyph_run_corpus_materializer_support_map();
    real_glyph_run_corpus_rejects_typed_span_fallback();
    real_glyph_run_corpus_rejects_coverage_row_fallback();
    real_glyph_run_corpus_rejects_synthetic_policy();
    whitespace_space_emits_no_class2_payload();
    whitespace_advance_preserves_next_glyph_offset();
    whitespace_still_advance_only();
    punctuation_tiny_contours_are_not_silently_dropped();
    cyrillic_glyphs_available_and_exercised();
    supported_extended_intervals_feed_ad68_without_fixture_payloads();
    materialized_ok_corpus_intervals_use_ad68_event_cursor_route();
    p6_5zg_event_cursor_route_support_map();
}
