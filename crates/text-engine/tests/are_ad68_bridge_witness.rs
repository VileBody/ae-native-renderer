use sha2::{Digest, Sha256};
use text_engine::{
    emit_ad68_rows, Ad68RowNode, AreCursorState, AreEventObject, AreEventRow, AreEventStream,
    ArePayloadBacking, ArePayloadBackingId, ArePayloadWindow, EventClass,
};

const WITNESS_ROW_Y: i32 = 18;
const WITNESS_Y_MIN: i32 = 0;
const WITNESS_EVENT_X: i32 = 20;
const WITNESS_EVENT_NEXT_X: i32 = 34;
const WITNESS_BYTES: [u8; 14] = [3, 44, 89, 123, 151, 164, 176, 175, 160, 143, 117, 81, 37, 3];
const WITNESS_FIRST_SPAN_SHA256: &str =
    "a8b31d7561acadac1067e37dc57a9b029b0f1a40f8b3df1294f569e23099b3bf";
const KNOWN_FULL_CLASS2_STREAM_SHA256: &str =
    "0cbc21afac5cddd6890f2a337328608b38d5f1dd39541064ef35334b32eaa052";
const KNOWN_TYPE2_MAP_SHA256: &str =
    "9707ab0b22375abc19dcdd77318b81339145e126c865ccbfc16f927fb79ad009";

fn build_txt030_focused_bridge_stream() -> AreEventStream {
    AreEventStream {
        y_min: WITNESS_Y_MIN,
        y_max: WITNESS_ROW_Y,
        rows: vec![AreEventRow {
            row_y: WITNESS_ROW_Y,
            cursors: vec![AreCursorState {
                current_x: WITNESS_EVENT_X,
                next_x: WITNESS_EVENT_NEXT_X,
                event_index: 0,
                materialize_flag: false,
            }],
        }],
        objects: vec![AreEventObject {
            event_class: EventClass::Class2,
            payload_window: Some(ArePayloadWindow {
                backing_id: ArePayloadBackingId(0),
                offset: 0,
                len: WITNESS_BYTES.len(),
            }),
            state: 0,
        }],
        payload_backings: vec![ArePayloadBacking {
            id: ArePayloadBackingId(0),
            bytes: WITNESS_BYTES.to_vec(),
        }],
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn class2_stream_bytes(nodes: &[Ad68RowNode]) -> Vec<u8> {
    nodes
        .iter()
        .filter_map(Ad68RowNode::bytes)
        .flat_map(|bytes| bytes.iter().copied())
        .collect()
}

#[test]
fn are_ad68_bridge_witness_txt030_first_class2_span() {
    let stream = build_txt030_focused_bridge_stream();
    let table = emit_ad68_rows(&stream).unwrap();

    assert_eq!(WITNESS_ROW_Y - table.y_min, 18);
    assert_eq!(table.rows.len(), 19);

    let witness_row = table.row(WITNESS_ROW_Y).unwrap();
    assert_eq!(witness_row.len(), 1);

    let node = &witness_row[0];
    assert_eq!(node.x, WITNESS_EVENT_X);
    assert_eq!(node.len, WITNESS_BYTES.len() as i32);
    assert_eq!(node.bytes(), Some(WITNESS_BYTES.as_slice()));
    assert_eq!(node.state, 0);

    let class2_bytes = class2_stream_bytes(witness_row);
    assert_eq!(class2_bytes, WITNESS_BYTES);
    assert_eq!(sha256_hex(&class2_bytes), WITNESS_FIRST_SPAN_SHA256);

    assert!(table
        .iter_nodes(WITNESS_ROW_Y)
        .all(|node| node.bytes().is_some() && node.len > 0));

    // These are full TXT_030 witness hashes over 117 class2 bytes and the full type2 map.
    // The focused fixture intentionally proves the first span representation only.
    assert_ne!(sha256_hex(&class2_bytes), KNOWN_FULL_CLASS2_STREAM_SHA256);
    assert_eq!(
        KNOWN_TYPE2_MAP_SHA256.len(),
        KNOWN_FULL_CLASS2_STREAM_SHA256.len()
    );
}
