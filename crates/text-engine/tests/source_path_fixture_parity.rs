use text_engine::{
    build_95cc_interval_list, build_descriptor_cursor_from_95cc_intervals,
    build_descriptor_cursor_from_95cc_intervals_with_policy, build_e854_working_set,
    descriptor_builder_to_event_stream, emit_ad68_rows, AreBezierSourcePathInput,
    AreDescriptorCursorPolicy, AreDescriptorTriplet, AreIntervalDescriptorPolicy, ArePathPoint,
    ArePathVerb, AreSamplerIntervalTag, AreSourceRecord32,
};

fn source_fixture() -> AreBezierSourcePathInput {
    AreBezierSourcePathInput::source_owned(
        vec![
            ArePathPoint::new(2.0, 4.0),
            ArePathPoint::new(6.0, 4.0),
            ArePathPoint::new(8.0, 4.0),
            ArePathPoint::new(10.0, 4.0),
        ],
        vec![
            ArePathVerb::MoveTo,
            ArePathVerb::LineTo,
            ArePathVerb::MoveTo,
            ArePathVerb::LineTo,
        ],
    )
}

#[test]
fn source_path_fixture_e854_records_match_expected() {
    let working_set = build_e854_working_set(&source_fixture()).unwrap();

    assert_eq!(working_set.records.len(), 2);
    assert_eq!(working_set.records[0].source_point_index_0x00, 1);
    assert_eq!(working_set.records[0].min_x_0x08, 2);
    assert_eq!(working_set.records[0].min_y_0x0c, 4);
    assert_eq!(working_set.records[0].max_x_0x10, 6);
    assert_eq!(working_set.records[0].max_y_0x14, 4);
    assert_eq!(
        working_set.records[0].record_flag_0x18,
        AreSourceRecord32::NORMAL_FLAG
    );

    assert_eq!(working_set.records[1].source_point_index_0x00, 3);
    assert_eq!(working_set.records[1].min_x_0x08, 8);
    assert_eq!(working_set.records[1].min_y_0x0c, 4);
    assert_eq!(working_set.records[1].max_x_0x10, 10);
    assert_eq!(working_set.records[1].max_y_0x14, 4);
    assert_eq!(
        working_set.records[1].record_flag_0x18,
        AreSourceRecord32::NORMAL_FLAG
    );
}

#[test]
fn source_path_fixture_95cc_intervals_match_expected() {
    let working_set = build_e854_working_set(&source_fixture()).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();
    let row = interval_list.row(4).unwrap();

    assert_eq!(interval_list.y_min, 4);
    assert_eq!(interval_list.y_max, 4);
    assert_eq!(row.runs.len(), 2);
    assert_eq!(row.runs[0].current_x, 2);
    assert_eq!(row.runs[0].next_x, 6);
    assert_eq!(row.runs[0].tag, AreSamplerIntervalTag::SourceSpan);
    assert!(row.runs[0].materialize_candidate);
    assert_eq!(row.runs[1].current_x, 8);
    assert_eq!(row.runs[1].next_x, 10);
    assert_eq!(row.runs[1].tag, AreSamplerIntervalTag::SourceSpan);
    assert!(row.runs[1].materialize_candidate);
}

#[test]
fn source_path_fixture_descriptor_cursor_payload_policy_gap_is_explicit() {
    let fixture_expected_payloads = [vec![31, 32, 33, 34], vec![51, 52]];
    let working_set = build_e854_working_set(&source_fixture()).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();
    let bridge = build_descriptor_cursor_from_95cc_intervals(&interval_list).unwrap();
    let event_stream =
        descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
    let row_table = emit_ad68_rows(&event_stream).unwrap();
    let nodes = row_table.row(4).unwrap();

    assert_eq!(nodes[0].x, 2);
    assert_eq!(nodes[0].len, 4);
    assert_eq!(nodes[1].x, 8);
    assert_eq!(nodes[1].len, 2);
    assert_ne!(
        nodes[0].bytes(),
        Some(fixture_expected_payloads[0].as_slice())
    );
    assert_ne!(
        nodes[1].bytes(),
        Some(fixture_expected_payloads[1].as_slice())
    );
}

fn triplet(base: usize) -> AreDescriptorTriplet {
    AreDescriptorTriplet {
        base,
        second: base + 0x08,
        source: base + 0x10,
    }
}

fn fixture_policy() -> AreDescriptorCursorPolicy {
    AreDescriptorCursorPolicy::fixture_backed(vec![
        AreIntervalDescriptorPolicy::class2_payload(
            4,
            0,
            triplet(0x7100),
            0x8100,
            vec![31, 32, 33, 34],
        ),
        AreIntervalDescriptorPolicy::class2_payload(4, 1, triplet(0x7200), 0x8200, vec![51, 52]),
    ])
}

#[test]
fn source_path_fixture_descriptor_cursor_payload_policy_closes_with_fixture_policy() {
    let working_set = build_e854_working_set(&source_fixture()).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();
    let policy = fixture_policy();
    let bridge =
        build_descriptor_cursor_from_95cc_intervals_with_policy(&interval_list, &policy).unwrap();
    let event_stream =
        descriptor_builder_to_event_stream(&bridge.builder, &bridge.cursor_input).unwrap();
    let row_table = emit_ad68_rows(&event_stream).unwrap();
    let nodes = row_table.row(4).unwrap();

    assert_eq!(nodes[0].x, 2);
    assert_eq!(nodes[0].len, 4);
    assert_eq!(nodes[0].bytes(), Some(&[31, 32, 33, 34][..]));
    assert_eq!(nodes[1].x, 8);
    assert_eq!(nodes[1].len, 2);
    assert_eq!(nodes[1].bytes(), Some(&[51, 52][..]));
}
