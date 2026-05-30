use std::fs;

use text_engine::{
    build_95cc_interval_list, build_descriptor_cursor_from_95cc_intervals_with_fixture_policy,
    build_descriptor_cursor_from_95cc_intervals_with_policy, build_e854_working_set,
    build_glyph_source_path_fixture_from_font_bytes, descriptor_builder_to_event_stream,
    emit_ad68_rows, Are95ccDescriptorCursorError, Are95ccDescriptorCursorOptions,
    AreBezierSourcePathInput, AreDescriptorCursorPolicy, AreDescriptorTriplet,
    AreIntervalDescriptorPolicy, ArePathPoint, ArePathVerb, AreSamplerIntervalList,
    AreSamplerIntervalRun, AreSamplerIntervalTag, AreSourcePathProvenance, AreSourceRecord32,
    AreSourceSamplerError,
};

const FONT_PATH: &str = "fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf";
const FONT_LABEL: &str = "Point-Light.ttf";
const GLYPH_CHAR: char = 'I';
const GLYPH_FONT_SIZE: f32 = 32.0;

fn real_glyph_source_path_fixture() -> text_engine::AreGlyphSourcePathFixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FONT_PATH);
    let bytes = fs::read(path).expect("Point-Light fixture font must exist");
    build_glyph_source_path_fixture_from_font_bytes(FONT_LABEL, &bytes, GLYPH_CHAR, GLYPH_FONT_SIZE)
        .expect("Point-Light I glyph outline fixture must build")
}

fn triplet(row_y: i32, run_index: usize) -> AreDescriptorTriplet {
    let base = 0x9000 + (row_y as usize * 0x40) + (run_index * 0x08);
    AreDescriptorTriplet {
        base,
        second: base + 0x08,
        source: base + 0x10,
    }
}

fn source_probe(row_y: i32, run_index: usize) -> usize {
    0xA000 + (row_y as usize * 0x20) + run_index
}

fn fixture_payload(row_y: i32, run_index: usize, run: &AreSamplerIntervalRun) -> Vec<u8> {
    (0..run.width() as usize)
        .map(|offset| {
            let value =
                row_y as i64 * 7 + run.current_x as i64 + run_index as i64 * 13 + offset as i64;
            value.rem_euclid(256) as u8
        })
        .collect()
}

fn fixture_policy(interval_list: &AreSamplerIntervalList) -> AreDescriptorCursorPolicy {
    let mut policies = Vec::new();
    for row in &interval_list.rows {
        for (run_index, run) in row.runs.iter().enumerate() {
            if run.materialize_candidate && run.tag == AreSamplerIntervalTag::SourceSpan {
                policies.push(AreIntervalDescriptorPolicy::class2_payload(
                    row.row_y,
                    run_index,
                    triplet(row.row_y, run_index),
                    source_probe(row.row_y, run_index),
                    fixture_payload(row.row_y, run_index, run),
                ));
            } else {
                policies.push(AreIntervalDescriptorPolicy::state(
                    row.row_y,
                    run_index,
                    triplet(row.row_y, run_index),
                    source_probe(row.row_y, run_index),
                    text_engine::Are95ccStateClass::Class0,
                    0x44,
                ));
            }
        }
    }
    AreDescriptorCursorPolicy::fixture_backed(policies)
}

#[test]
fn real_glyph_source_path_to_e854_records() {
    let fixture = real_glyph_source_path_fixture();
    let working_set = build_e854_working_set(&fixture.input).unwrap();

    assert_eq!(fixture.font_label, FONT_LABEL);
    assert_eq!(fixture.character, GLYPH_CHAR);
    assert_eq!(fixture.glyph_id, 44);
    assert_eq!(fixture.units_per_em, 2048);
    assert_eq!(
        fixture.input.provenance,
        AreSourcePathProvenance::SourceOwnedGlyphPath
    );
    assert_eq!(working_set.records.len(), 5);
    assert_eq!(working_set.bounds.x_min, 3);
    assert_eq!(working_set.bounds.y_min, 0);
    assert_eq!(working_set.bounds.x_max, 6);
    assert_eq!(working_set.bounds.y_max, 22);

    assert_eq!(working_set.records[0].source_point_index_0x00, 1);
    assert_eq!(working_set.records[0].min_x_0x08, 3);
    assert_eq!(working_set.records[0].min_y_0x0c, 0);
    assert_eq!(working_set.records[0].max_x_0x10, 4);
    assert_eq!(working_set.records[0].max_y_0x14, 22);
    assert_eq!(
        working_set.records[0].record_flag_0x18,
        AreSourceRecord32::NORMAL_FLAG
    );

    assert_eq!(
        working_set.records.last().unwrap().record_flag_0x18,
        AreSourceRecord32::SENTINEL_FLAG
    );
}

#[test]
fn real_glyph_source_path_to_95cc_intervals() {
    let fixture = real_glyph_source_path_fixture();
    let working_set = build_e854_working_set(&fixture.input).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();
    let row0 = interval_list.row(0).unwrap();
    let row21 = interval_list.row(21).unwrap();

    assert_eq!(interval_list.y_min, 0);
    assert_eq!(interval_list.y_max, 22);
    assert_eq!(row0.runs.len(), 3);
    assert_eq!(row0.runs[0].current_x, 3);
    assert_eq!(row0.runs[0].next_x, 4);
    assert_eq!(row0.runs[0].tag, AreSamplerIntervalTag::SourceSpan);
    assert_eq!(row0.runs[1].current_x, 3);
    assert_eq!(row0.runs[1].next_x, 4);
    assert_eq!(row0.runs[1].tag, AreSamplerIntervalTag::Sentinel);
    assert_eq!(row0.runs[2].current_x, 3);
    assert_eq!(row0.runs[2].next_x, 6);
    assert_eq!(row0.runs[2].tag, AreSamplerIntervalTag::SourceSpan);
    assert_eq!(row0.runs[2].source_record_indices, vec![3, 2]);

    assert_eq!(row21.runs.len(), 1);
    assert_eq!(row21.runs[0].current_x, 3);
    assert_eq!(row21.runs[0].next_x, 6);
    assert_eq!(row21.runs[0].source_record_indices, vec![0, 1, 2]);
}

#[test]
fn real_glyph_source_path_fixture_backed_payload_to_ad68_rows() {
    let fixture = real_glyph_source_path_fixture();
    let working_set = build_e854_working_set(&fixture.input).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();
    let policy = fixture_policy(&interval_list);
    let bridge =
        build_descriptor_cursor_from_95cc_intervals_with_fixture_policy(&interval_list, &policy)
            .unwrap();
    let event_stream =
        descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
    let row_table = emit_ad68_rows(&event_stream).unwrap();
    let row0 = row_table.row(0).unwrap();
    let row21 = row_table.row(21).unwrap();

    assert_eq!(row_table.y_min, 0);
    assert_eq!(row_table.y_max, 21);
    assert_eq!(row0[0].x, 3);
    assert_eq!(row0[0].len, 1);
    let expected_row0_run0 = fixture_payload(0, 0, &interval_list.row(0).unwrap().runs[0]);
    assert_eq!(row0[0].bytes(), Some(expected_row0_run0.as_slice()));
    assert_eq!(row0[1].x, 3);
    assert_eq!(row0[1].len, 0);
    assert_eq!(row0[1].state, 0x44);
    assert_eq!(row0[1].bytes, None);
    assert_eq!(row0[2].x, 3);
    assert_eq!(row0[2].len, 3);
    let expected_row0_run2 = fixture_payload(0, 2, &interval_list.row(0).unwrap().runs[2]);
    assert_eq!(row0[2].bytes(), Some(expected_row0_run2.as_slice()));

    assert_eq!(row21[0].x, 3);
    assert_eq!(row21[0].len, 3);
    let expected_row21_run0 = fixture_payload(21, 0, &interval_list.row(21).unwrap().runs[0]);
    assert_eq!(row21[0].bytes(), Some(expected_row21_run0.as_slice()));
}

#[test]
fn real_glyph_source_path_rejects_coverage_row_as_source_path() {
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
fn real_glyph_source_path_rejects_typed_span_as_source_path() {
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
fn real_glyph_source_path_rejects_synthetic_policy_in_real_source_fixture_mode() {
    let fixture = real_glyph_source_path_fixture();
    let working_set = build_e854_working_set(&fixture.input).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();
    let synthetic_policy =
        AreDescriptorCursorPolicy::synthetic_only(Are95ccDescriptorCursorOptions::default());

    assert_eq!(
        build_descriptor_cursor_from_95cc_intervals_with_fixture_policy(
            &interval_list,
            &synthetic_policy,
        ),
        Err(Are95ccDescriptorCursorError::SyntheticPolicyRejectedForFixture)
    );
    assert!(build_descriptor_cursor_from_95cc_intervals_with_policy(
        &interval_list,
        &synthetic_policy,
    )
    .is_ok());
}
