use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventStream {
    pub y_min: i32,
    pub y_max: i32,
    pub rows: Vec<AreEventRow>,
    pub objects: Vec<AreEventObject>,
    pub payload_backings: Vec<ArePayloadBacking>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventRow {
    pub row_y: i32,
    pub cursors: Vec<AreCursorState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreCursorState {
    pub current_x: i32,
    pub next_x: i32,
    pub event_index: usize,
    pub materialize_flag: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventObject {
    pub event_class: EventClass,
    pub payload_window: Option<ArePayloadWindow>,
    pub state: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    Class0,
    Class1,
    Class2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArePayloadBacking {
    pub id: ArePayloadBackingId,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArePayloadBackingId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArePayloadWindow {
    pub backing_id: ArePayloadBackingId,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ad68RowTable {
    pub y_min: i32,
    pub y_max: i32,
    pub rows: Vec<Vec<Ad68RowNode>>,
}

impl Ad68RowTable {
    pub fn row(&self, row_y: i32) -> Option<&[Ad68RowNode]> {
        if row_y < self.y_min || row_y > self.y_max {
            return None;
        }
        self.rows
            .get((row_y - self.y_min) as usize)
            .map(Vec::as_slice)
    }

    pub fn iter_nodes(&self, row_y: i32) -> impl Iterator<Item = &Ad68RowNode> {
        self.row(row_y).into_iter().flatten()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ad68RowNode {
    pub x: i32,
    pub len: i32,
    pub bytes: Option<Vec<u8>>,
    pub state: u8,
}

impl Ad68RowNode {
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}
