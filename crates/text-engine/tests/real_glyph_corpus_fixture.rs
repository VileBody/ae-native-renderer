use std::fs;

use text_engine::{
    build_95cc_interval_list, build_descriptor_cursor_from_95cc_intervals_with_fixture_policy,
    build_e854_working_set, build_glyph_source_path_fixture_from_font_bytes,
    descriptor_builder_to_event_stream, emit_ad68_rows, Are95ccDescriptorCursorError,
    Are95ccDescriptorCursorOptions, AreDescriptorCursorPolicy, AreDescriptorTriplet,
    AreIntervalDescriptorPolicy, ArePathPoint, ArePathVerb, AreSamplerIntervalList,
    AreSamplerIntervalRun, AreSamplerIntervalTag, AreSourcePathProvenance, AreSourceSamplerError,
};

const FONT_PATH: &str = "fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf";
const FONT_LABEL: &str = "Point-Light.ttf";
const FONT_SIZE: f32 = 32.0;

#[derive(Debug, Clone, Copy)]
struct CorpusGlyph {
    ch: char,
    role: &'static str,
    glyph_id: u16,
    requires_curve: bool,
    requires_multiple_contours: bool,
    requires_diagonal: bool,
}

fn corpus() -> Vec<CorpusGlyph> {
    vec![
        CorpusGlyph {
            ch: 'I',
            role: "baseline",
            glyph_id: 44,
            requires_curve: false,
            requires_multiple_contours: false,
            requires_diagonal: false,
        },
        CorpusGlyph {
            ch: 'H',
            role: "wide",
            glyph_id: 43,
            requires_curve: false,
            requires_multiple_contours: false,
            requires_diagonal: false,
        },
        CorpusGlyph {
            ch: 'l',
            role: "narrow",
            glyph_id: 79,
            requires_curve: false,
            requires_multiple_contours: false,
            requires_diagonal: false,
        },
        CorpusGlyph {
            ch: 'S',
            role: "curve",
            glyph_id: 54,
            requires_curve: true,
            requires_multiple_contours: false,
            requires_diagonal: true,
        },
        CorpusGlyph {
            ch: 'A',
            role: "multiple_contours_diagonal",
            glyph_id: 36,
            requires_curve: false,
            requires_multiple_contours: true,
            requires_diagonal: true,
        },
        CorpusGlyph {
            ch: 'V',
            role: "diagonal_complex",
            glyph_id: 57,
            requires_curve: false,
            requires_multiple_contours: false,
            requires_diagonal: true,
        },
    ]
}

fn font_bytes() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FONT_PATH);
    fs::read(path).expect("Point-Light fixture font must exist")
}

fn glyph_fixture(glyph: CorpusGlyph) -> text_engine::AreGlyphSourcePathFixture {
    build_glyph_source_path_fixture_from_font_bytes(FONT_LABEL, &font_bytes(), glyph.ch, FONT_SIZE)
        .expect("glyph source path fixture must build")
}

fn triplet(glyph_index: usize, row_index: usize, run_index: usize) -> AreDescriptorTriplet {
    let base = 0xB000 + glyph_index * 0x1000 + row_index * 0x40 + run_index * 0x08;
    AreDescriptorTriplet {
        base,
        second: base + 0x08,
        source: base + 0x10,
    }
}

fn source_probe(glyph_index: usize, row_index: usize, run_index: usize) -> usize {
    0xC000 + glyph_index * 0x1000 + row_index * 0x40 + run_index
}

fn fixture_payload(
    glyph_index: usize,
    row_y: i32,
    run_index: usize,
    run: &AreSamplerIntervalRun,
) -> Vec<u8> {
    (0..run.width() as usize)
        .map(|offset| {
            let value = glyph_index as i64 * 29
                + row_y as i64 * 7
                + run.current_x as i64
                + run_index as i64 * 13
                + offset as i64;
            value.rem_euclid(256) as u8
        })
        .collect()
}

fn fixture_policy(
    glyph_index: usize,
    interval_list: &AreSamplerIntervalList,
) -> AreDescriptorCursorPolicy {
    let mut policies = Vec::new();
    for (row_index, row) in interval_list.rows.iter().enumerate() {
        for (run_index, run) in row.runs.iter().enumerate() {
            if run.materialize_candidate && run.tag == AreSamplerIntervalTag::SourceSpan {
                policies.push(AreIntervalDescriptorPolicy::class2_payload(
                    row.row_y,
                    run_index,
                    triplet(glyph_index, row_index, run_index),
                    source_probe(glyph_index, row_index, run_index),
                    fixture_payload(glyph_index, row.row_y, run_index, run),
                ));
            } else {
                policies.push(AreIntervalDescriptorPolicy::state(
                    row.row_y,
                    run_index,
                    triplet(glyph_index, row_index, run_index),
                    source_probe(glyph_index, row_index, run_index),
                    text_engine::Are95ccStateClass::Class0,
                    0x44,
                ));
            }
        }
    }
    AreDescriptorCursorPolicy::fixture_backed(policies)
}

fn has_diagonal_record(records: &[text_engine::AreSourceRecord32]) -> bool {
    records.iter().any(|record| {
        record.max_x_0x10 > record.min_x_0x08 && record.max_y_0x14 > record.min_y_0x0c
    })
}

#[test]
fn real_glyph_corpus_source_paths_extract() {
    for glyph in corpus() {
        let fixture = glyph_fixture(glyph);
        let move_count = fixture
            .input
            .verbs
            .iter()
            .filter(|verb| **verb == ArePathVerb::MoveTo)
            .count();
        let curve_count = fixture
            .input
            .verbs
            .iter()
            .filter(|verb| matches!(verb, ArePathVerb::QuadTo | ArePathVerb::CubicTo))
            .count();

        assert_eq!(fixture.font_label, FONT_LABEL, "{}", glyph.role);
        assert_eq!(fixture.glyph_id, glyph.glyph_id, "{}", glyph.role);
        assert_eq!(fixture.units_per_em, 2048, "{}", glyph.role);
        assert_eq!(
            fixture.input.provenance,
            AreSourcePathProvenance::SourceOwnedGlyphPath,
            "{}",
            glyph.role
        );
        assert!(!fixture.input.points.is_empty(), "{}", glyph.role);
        assert_eq!(
            fixture.input.points.len(),
            fixture.input.verbs.len(),
            "{}",
            glyph.role
        );
        if glyph.requires_curve {
            assert!(curve_count > 0, "{}", glyph.role);
        }
        if glyph.requires_multiple_contours {
            assert!(move_count > 1, "{}", glyph.role);
        }
    }
}

#[test]
fn real_glyph_corpus_to_e854_records() {
    for glyph in corpus() {
        let fixture = glyph_fixture(glyph);
        let working_set = build_e854_working_set(&fixture.input).unwrap();

        assert!(!working_set.records.is_empty(), "{}", glyph.role);
        assert!(working_set.bounds.width() > 0, "{}", glyph.role);
        assert!(working_set.bounds.height() > 0, "{}", glyph.role);
        assert_eq!(
            working_set.record_vector_capacity_0x196,
            working_set.records.len(),
            "{}",
            glyph.role
        );
        if glyph.requires_diagonal {
            assert!(has_diagonal_record(&working_set.records), "{}", glyph.role);
        }
    }
}

#[test]
fn real_glyph_corpus_to_95cc_intervals() {
    for glyph in corpus() {
        let fixture = glyph_fixture(glyph);
        let working_set = build_e854_working_set(&fixture.input).unwrap();
        let interval_list = build_95cc_interval_list(&working_set).unwrap();

        assert_eq!(
            interval_list.provenance,
            AreSourcePathProvenance::SourceOwned
        );
        assert!(!interval_list.rows.is_empty(), "{}", glyph.role);
        assert!(
            interval_list.rows.iter().any(|row| !row.runs.is_empty()),
            "{}",
            glyph.role
        );
        assert!(
            interval_list.working_state.emitted_run_count > 0,
            "{}",
            glyph.role
        );
        assert!(
            interval_list.working_state.emitted_boundary_count > 0,
            "{}",
            glyph.role
        );
    }
}

#[test]
fn real_glyph_corpus_fixture_backed_payload_to_ad68_rows() {
    for (glyph_index, glyph) in corpus().into_iter().enumerate() {
        let fixture = glyph_fixture(glyph);
        let working_set = build_e854_working_set(&fixture.input).unwrap();
        let interval_list = build_95cc_interval_list(&working_set).unwrap();
        let policy = fixture_policy(glyph_index, &interval_list);
        let bridge = build_descriptor_cursor_from_95cc_intervals_with_fixture_policy(
            &interval_list,
            &policy,
        )
        .unwrap();
        let event_stream =
            descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
        let row_table = emit_ad68_rows(&event_stream).unwrap();

        assert_eq!(row_table.y_min, interval_list.y_min, "{}", glyph.role);
        assert!(row_table.y_max <= interval_list.y_max, "{}", glyph.role);
        let mut class2_count = 0usize;
        for row in &interval_list.rows {
            let Some(nodes) = row_table.row(row.row_y) else {
                continue;
            };
            assert_eq!(nodes.len(), row.runs.len(), "{}", glyph.role);
            for (run_index, run) in row.runs.iter().enumerate() {
                let node = &nodes[run_index];
                assert_eq!(node.x, run.current_x, "{}", glyph.role);
                if run.materialize_candidate && run.tag == AreSamplerIntervalTag::SourceSpan {
                    class2_count += 1;
                    assert_eq!(node.len, run.width(), "{}", glyph.role);
                    let expected = fixture_payload(glyph_index, row.row_y, run_index, run);
                    assert_eq!(node.bytes(), Some(expected.as_slice()), "{}", glyph.role);
                } else {
                    assert_eq!(node.len, 0, "{}", glyph.role);
                    assert_eq!(node.state, 0x44, "{}", glyph.role);
                    assert_eq!(node.bytes, None, "{}", glyph.role);
                }
            }
        }
        assert!(class2_count > 0, "{}", glyph.role);
    }
}

#[test]
fn real_glyph_corpus_rejects_typed_span_fallback() {
    let input = text_engine::AreBezierSourcePathInput::typed_span_fallback(
        vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(1.0, 0.0)],
        vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
    );

    assert_eq!(
        build_e854_working_set(&input),
        Err(AreSourceSamplerError::TypedSpanFallbackRejected)
    );
}

#[test]
fn real_glyph_corpus_rejects_coverage_row_fallback() {
    let input = text_engine::AreBezierSourcePathInput::coverage_row_fallback(
        vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(1.0, 0.0)],
        vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
    );

    assert_eq!(
        build_e854_working_set(&input),
        Err(AreSourceSamplerError::CoverageRowFallbackRejected)
    );
}

#[test]
fn real_glyph_corpus_rejects_synthetic_policy() {
    let glyph = corpus()[0];
    let fixture = glyph_fixture(glyph);
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
}
