use serde::Deserialize;
use sha2::{Digest, Sha256};
use text_engine::{emit_ad68_rows, Ad68RowNode, AreEventStream};

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

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct BridgeSpan {
    row: i32,
    current_x: i32,
    next_x: i32,
    bytes: Vec<u8>,
}

fn fixture() -> Txt030FullBridgeFixture {
    serde_json::from_str(include_str!("fixtures/txt030_focused_bridge_stream.json"))
        .expect("TXT_030 focused bridge stream fixture must deserialize")
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

#[test]
fn txt030_full_bridge_stream_matches_known_hashes() {
    let fixture = fixture();
    let table = emit_ad68_rows(&fixture.stream).unwrap();
    let nodes = class2_nodes(&table);
    let class2_bytes = class2_stream_bytes(&nodes);

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
}

#[test]
fn txt030_full_bridge_stream_first_span_stays_p6_2_compatible() {
    let fixture = fixture();
    let table = emit_ad68_rows(&fixture.stream).unwrap();
    let first_row = table.row(fixture.expected_first_span.row).unwrap();
    let first_node = first_row
        .iter()
        .find(|node| node.x == fixture.expected_first_span.current_x)
        .expect("first witness span must be present");

    assert_eq!(fixture.expected_first_span.row, 18);
    assert_eq!(fixture.expected_first_span.current_x, 20);
    assert_eq!(fixture.expected_first_span.next_x, 34);
    assert_eq!(
        first_node.len,
        fixture.expected_first_span.next_x - fixture.expected_first_span.current_x
    );
    assert_eq!(
        first_node.bytes(),
        Some(fixture.expected_first_span.bytes.as_slice())
    );
}

#[test]
fn txt030_full_bridge_stream_topology_and_payload_windows_are_stable() {
    let fixture = fixture();
    let table = emit_ad68_rows(&fixture.stream).unwrap();
    let nodes = class2_nodes(&table);

    assert_eq!(nodes.len(), fixture.expected_bridge_spans.len());
    for ((row_y, node), expected) in nodes.iter().zip(&fixture.expected_bridge_spans) {
        assert_eq!(*row_y, expected.row);
        assert_eq!(node.x, expected.current_x);
        assert_eq!(node.len, expected.next_x - expected.current_x);
        assert_eq!(node.bytes(), Some(expected.bytes.as_slice()));
        assert_eq!(node.state, 0);
    }
}
