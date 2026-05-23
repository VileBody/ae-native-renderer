use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use text_engine::{
    descriptor_builder_to_event_stream, emit_ad68_rows, Ad68RowNode, AreDescriptorBuilderRoot,
    AreDescriptorCursorEvent, AreDescriptorCursorInput, AreDescriptorCursorRow, AreDescriptorRef,
    AreDescriptorTriplet, AreEventDescriptorSourceObject, AreEventStream, ArePayloadWindow,
    DescriptorId, DescriptorToEventStreamError, EventClass,
};

#[derive(Debug, Deserialize)]
struct Txt030FullBridgeFixture {
    expected_bridge_spans: Vec<BridgeSpan>,
    expected_class2_byte_count: usize,
    expected_class2_span_count: usize,
    expected_class2_stream_sha256: String,
    expected_first_span: BridgeSpan,
    expected_type2_map: Vec<[i32; 3]>,
    expected_type2_map_count: usize,
    expected_type2_map_sha256: String,
    stream: AreEventStream,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct BridgeSpan {
    row: i32,
    current_x: i32,
    next_x: i32,
    bytes: Vec<u8>,
}

struct Txt030DescriptorBackedFixture {
    root: AreDescriptorBuilderRoot,
    input: AreDescriptorCursorInput,
    descriptor_refs: Vec<AreDescriptorRef>,
}

fn raw_fixture() -> Txt030FullBridgeFixture {
    serde_json::from_str(include_str!("fixtures/txt030_focused_bridge_stream.json"))
        .expect("TXT_030 focused bridge stream fixture must deserialize")
}

fn triplet(index: usize) -> AreDescriptorTriplet {
    AreDescriptorTriplet {
        base: 0x1000 + index,
        second: 0x2000 + index,
        source: 0x3000 + index,
    }
}

fn sha256_json<T: serde::Serialize>(value: &T) -> String {
    let payload = serde_json::to_vec(value).expect("fixture hash payload must serialize");
    let digest = Sha256::digest(payload);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn class2_nodes(table: &text_engine::Ad68RowTable) -> Vec<(i32, &Ad68RowNode)> {
    table
        .rows
        .iter()
        .enumerate()
        .flat_map(|(row_index, nodes)| {
            let row_y = table.y_min + row_index as i32;
            nodes
                .iter()
                .filter(|node| node.bytes().is_some())
                .map(move |node| (row_y, node))
        })
        .collect()
}

fn class2_stream_bytes(nodes: &[(i32, &Ad68RowNode)]) -> Vec<u8> {
    nodes
        .iter()
        .filter_map(|(_, node)| node.bytes())
        .flat_map(|bytes| bytes.iter().copied())
        .collect()
}

fn build_txt030_descriptor_backed_focused_bridge_fixture(
) -> (Txt030FullBridgeFixture, Txt030DescriptorBackedFixture) {
    let fixture = raw_fixture();
    let mut root = AreDescriptorBuilderRoot::new();
    let mut rows: BTreeMap<i32, Vec<AreDescriptorCursorEvent>> = BTreeMap::new();

    for (index, span) in fixture.expected_bridge_spans.iter().enumerate() {
        let descriptor_id = root.define_descriptor(
            triplet(index),
            AreEventDescriptorSourceObject::with_0x20(0x5000 + index),
        );
        root.register_descriptor(descriptor_id);

        let backing_window = root.payload_allocator.allocate(span.bytes.clone());
        assert!(root.set_descriptor_payload_window(
            descriptor_id,
            ArePayloadWindow {
                backing_id: backing_window.backing_id,
                offset: 0,
                len: span.bytes.len(),
            },
        ));

        rows.entry(span.row)
            .or_default()
            .push(AreDescriptorCursorEvent::new(
                span.current_x,
                span.next_x,
                descriptor_id,
                true,
            ));
    }

    let descriptor_refs = root.descriptor_vector.elements.clone();
    let input_rows = rows
        .into_iter()
        .map(|(row_y, events)| AreDescriptorCursorRow::new(row_y, events))
        .collect();
    let input =
        AreDescriptorCursorInput::new(fixture.stream.y_min, fixture.stream.y_max, input_rows);

    (
        fixture,
        Txt030DescriptorBackedFixture {
            root,
            input,
            descriptor_refs,
        },
    )
}

fn single_registered_descriptor(
    source_object: AreEventDescriptorSourceObject,
) -> (AreDescriptorBuilderRoot, DescriptorId) {
    let mut root = AreDescriptorBuilderRoot::new();
    let descriptor_id = root.define_descriptor(triplet(0), source_object);
    root.register_descriptor(descriptor_id);
    (root, descriptor_id)
}

fn input_for(descriptor_id: DescriptorId, materialize_flag: bool) -> AreDescriptorCursorInput {
    AreDescriptorCursorInput::new(
        18,
        18,
        vec![AreDescriptorCursorRow::new(
            18,
            vec![AreDescriptorCursorEvent::new(
                20,
                34,
                descriptor_id,
                materialize_flag,
            )],
        )],
    )
}

#[test]
fn txt030_descriptor_backed_focused_bridge_hashes_and_topology_match() {
    let (fixture, descriptor_fixture) = build_txt030_descriptor_backed_focused_bridge_fixture();

    let descriptor_stream =
        descriptor_builder_to_event_stream(&descriptor_fixture.root, &descriptor_fixture.input)
            .unwrap();
    let descriptor_table = emit_ad68_rows(&descriptor_stream).unwrap();
    let direct_table = emit_ad68_rows(&fixture.stream).unwrap();
    let nodes = class2_nodes(&descriptor_table);
    let class2_bytes = class2_stream_bytes(&nodes);

    assert_eq!(descriptor_table, direct_table);
    assert_eq!(nodes.len(), fixture.expected_class2_span_count);
    assert_eq!(class2_bytes.len(), fixture.expected_class2_byte_count);
    assert_eq!(
        sha256_json(&class2_bytes),
        fixture.expected_class2_stream_sha256
    );
    assert_eq!(
        fixture.expected_type2_map.len(),
        fixture.expected_type2_map_count
    );
    assert_eq!(
        sha256_json(&fixture.expected_type2_map),
        fixture.expected_type2_map_sha256
    );

    let first_row = descriptor_table
        .row(fixture.expected_first_span.row)
        .unwrap();
    let first_node = first_row
        .iter()
        .find(|node| node.x == fixture.expected_first_span.current_x)
        .expect("first witness span must be present");
    assert_eq!(fixture.expected_first_span.row, 18);
    assert_eq!(fixture.expected_first_span.current_x, 20);
    assert_eq!(fixture.expected_first_span.next_x, 34);
    assert_eq!(
        first_node.bytes(),
        Some(fixture.expected_first_span.bytes.as_slice())
    );
}

#[test]
fn txt030_descriptor_backed_focused_bridge_descriptor_contract_is_preserved() {
    let (fixture, descriptor_fixture) = build_txt030_descriptor_backed_focused_bridge_fixture();
    let stream =
        descriptor_builder_to_event_stream(&descriptor_fixture.root, &descriptor_fixture.input)
            .unwrap();

    assert_eq!(
        descriptor_fixture.root.descriptor_vector.elements,
        descriptor_fixture.descriptor_refs
    );
    assert_eq!(
        descriptor_fixture.descriptor_refs.len(),
        fixture.expected_class2_span_count
    );
    assert!(descriptor_fixture.root.ctx_08.descriptors.is_empty());
    assert_eq!(
        descriptor_fixture.root.ctx_58.descriptors,
        descriptor_fixture.descriptor_refs
    );
    assert!(stream
        .objects
        .iter()
        .all(|object| object.event_class == EventClass::Class2));
    assert!(stream
        .rows
        .iter()
        .flat_map(|row| &row.cursors)
        .all(|cursor| cursor.materialize_flag));

    let descriptor_json =
        serde_json::to_value(descriptor_fixture.root.descriptor(DescriptorId(0)).unwrap()).unwrap();
    assert!(descriptor_json.get("class_tag").is_none());
}

#[test]
fn txt030_descriptor_backed_focused_bridge_missing_class_vector_errors() {
    let mut root = AreDescriptorBuilderRoot::new();
    let descriptor_id = root.define_descriptor(triplet(0), AreEventDescriptorSourceObject::empty());

    let err =
        descriptor_builder_to_event_stream(&root, &input_for(descriptor_id, false)).unwrap_err();

    assert_eq!(
        err,
        DescriptorToEventStreamError::DescriptorWithoutClassRegistration { descriptor_id }
    );
}

#[test]
fn txt030_descriptor_backed_focused_bridge_class2_without_payload_errors() {
    let (root, descriptor_id) =
        single_registered_descriptor(AreEventDescriptorSourceObject::with_0x20(0x2220));

    let err =
        descriptor_builder_to_event_stream(&root, &input_for(descriptor_id, true)).unwrap_err();

    assert_eq!(
        err,
        DescriptorToEventStreamError::Class2DescriptorWithoutPayload { descriptor_id }
    );
}

#[test]
fn txt030_descriptor_backed_focused_bridge_descriptor_outside_vector_errors() {
    let (mut root, descriptor_id) =
        single_registered_descriptor(AreEventDescriptorSourceObject::with_0x20(0x2220));
    root.descriptor_vector.elements.clear();

    let err =
        descriptor_builder_to_event_stream(&root, &input_for(descriptor_id, true)).unwrap_err();

    assert_eq!(
        err,
        DescriptorToEventStreamError::DescriptorOutsideVector { descriptor_id }
    );
}

#[test]
fn txt030_descriptor_backed_focused_bridge_conflicting_class_registration_is_explicit() {
    let (root, descriptor_id) =
        single_registered_descriptor(AreEventDescriptorSourceObject::with_0x18_0x20(1, 2));

    let err =
        descriptor_builder_to_event_stream(&root, &input_for(descriptor_id, false)).unwrap_err();

    assert_eq!(
        err,
        DescriptorToEventStreamError::ConflictingClassRegistration { descriptor_id }
    );
}

#[test]
fn txt030_descriptor_backed_focused_bridge_payload_window_shorter_than_span_errors() {
    let (mut root, descriptor_id) =
        single_registered_descriptor(AreEventDescriptorSourceObject::with_0x20(0x2220));
    let window = root.payload_allocator.allocate(vec![1, 2, 3]);
    root.set_descriptor_payload_window(descriptor_id, window);

    let err =
        descriptor_builder_to_event_stream(&root, &input_for(descriptor_id, true)).unwrap_err();

    assert_eq!(
        err,
        DescriptorToEventStreamError::PayloadWindowShorterThanSpan {
            descriptor_id,
            window_len: 3,
            span_width: 14,
        }
    );
}
