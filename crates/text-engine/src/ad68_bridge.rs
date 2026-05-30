use crate::{
    Ad68RowNode, Ad68RowTable, AreCursorState, AreEventStream, ArePayloadBacking, ArePayloadWindow,
    EventClass,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ad68EmitError {
    InvalidRowBounds {
        y_min: i32,
        y_max: i32,
    },
    RowOutOfBounds {
        row_y: i32,
        y_min: i32,
        y_max: i32,
    },
    InvalidCursorWidth {
        row_y: i32,
        current_x: i32,
        next_x: i32,
    },
    InvalidEventIndex {
        row_y: i32,
        event_index: usize,
        objects_len: usize,
    },
    MissingPayloadWindow {
        row_y: i32,
        event_index: usize,
    },
    MissingPayloadBacking {
        row_y: i32,
        event_index: usize,
        backing_id: usize,
    },
    InvalidPayloadWindow {
        row_y: i32,
        event_index: usize,
        backing_id: usize,
        offset: usize,
        len: usize,
        backing_len: usize,
    },
    PayloadWindowShorterThanWidth {
        row_y: i32,
        event_index: usize,
        window_len: usize,
        width: usize,
    },
}

pub fn emit_ad68_rows(stream: &AreEventStream) -> Result<Ad68RowTable, Ad68EmitError> {
    if stream.y_max < stream.y_min {
        return Err(Ad68EmitError::InvalidRowBounds {
            y_min: stream.y_min,
            y_max: stream.y_max,
        });
    }

    let row_count = (stream.y_max - stream.y_min + 1) as usize;
    let mut rows = vec![Vec::new(); row_count];

    for row in &stream.rows {
        let row_index =
            row.row_y
                .checked_sub(stream.y_min)
                .ok_or(Ad68EmitError::RowOutOfBounds {
                    row_y: row.row_y,
                    y_min: stream.y_min,
                    y_max: stream.y_max,
                })?;
        if row.row_y > stream.y_max || row_index < 0 {
            return Err(Ad68EmitError::RowOutOfBounds {
                row_y: row.row_y,
                y_min: stream.y_min,
                y_max: stream.y_max,
            });
        }

        let output_row = &mut rows[row_index as usize];
        for cursor in &row.cursors {
            output_row.push(emit_node(stream, row.row_y, cursor)?);
        }
    }

    Ok(Ad68RowTable {
        y_min: stream.y_min,
        y_max: stream.y_max,
        rows,
    })
}

fn emit_node(
    stream: &AreEventStream,
    row_y: i32,
    cursor: &AreCursorState,
) -> Result<Ad68RowNode, Ad68EmitError> {
    if cursor.event_index >= stream.objects.len() {
        return Err(Ad68EmitError::InvalidEventIndex {
            row_y,
            event_index: cursor.event_index,
            objects_len: stream.objects.len(),
        });
    }
    let event = &stream.objects[cursor.event_index];

    let width =
        cursor
            .next_x
            .checked_sub(cursor.current_x)
            .ok_or(Ad68EmitError::InvalidCursorWidth {
                row_y,
                current_x: cursor.current_x,
                next_x: cursor.next_x,
            })?;
    if width < 0 {
        return Err(Ad68EmitError::InvalidCursorWidth {
            row_y,
            current_x: cursor.current_x,
            next_x: cursor.next_x,
        });
    }

    match event.event_class {
        EventClass::Class0 | EventClass::Class1 => Ok(Ad68RowNode {
            x: cursor.current_x,
            len: 0,
            bytes: None,
            state: event.state,
        }),
        EventClass::Class2 => {
            let width = width as usize;
            let payload_window =
                event
                    .payload_window
                    .ok_or(Ad68EmitError::MissingPayloadWindow {
                        row_y,
                        event_index: cursor.event_index,
                    })?;
            let backing = payload_backing(stream, row_y, cursor.event_index, payload_window)?;
            let payload_end = payload_window
                .offset
                .checked_add(payload_window.len)
                .ok_or(Ad68EmitError::InvalidPayloadWindow {
                    row_y,
                    event_index: cursor.event_index,
                    backing_id: payload_window.backing_id.0,
                    offset: payload_window.offset,
                    len: payload_window.len,
                    backing_len: backing.bytes.len(),
                })?;
            if payload_end > backing.bytes.len() {
                return Err(Ad68EmitError::InvalidPayloadWindow {
                    row_y,
                    event_index: cursor.event_index,
                    backing_id: payload_window.backing_id.0,
                    offset: payload_window.offset,
                    len: payload_window.len,
                    backing_len: backing.bytes.len(),
                });
            }
            if payload_window.len < width {
                return Err(Ad68EmitError::PayloadWindowShorterThanWidth {
                    row_y,
                    event_index: cursor.event_index,
                    window_len: payload_window.len,
                    width,
                });
            }

            let payload_start = payload_window.offset;
            let bytes = backing.bytes[payload_start..payload_start + width].to_vec();
            Ok(Ad68RowNode {
                x: cursor.current_x,
                len: width as i32,
                bytes: Some(bytes),
                state: 0,
            })
        }
    }
}

fn payload_backing<'a>(
    stream: &'a AreEventStream,
    row_y: i32,
    event_index: usize,
    payload_window: ArePayloadWindow,
) -> Result<&'a ArePayloadBacking, Ad68EmitError> {
    stream
        .payload_backings
        .iter()
        .find(|backing| backing.id == payload_window.backing_id)
        .ok_or(Ad68EmitError::MissingPayloadBacking {
            row_y,
            event_index,
            backing_id: payload_window.backing_id.0,
        })
}

#[cfg(test)]
mod are_event_stream_tests {
    use super::*;
    use crate::{
        Ad68RowNode, AreCursorState, AreEventObject, AreEventRow, AreEventStream,
        ArePayloadBacking, ArePayloadBackingId, ArePayloadWindow, EventClass,
    };

    fn stream(
        y_min: i32,
        y_max: i32,
        rows: Vec<AreEventRow>,
        objects: Vec<AreEventObject>,
        payload_backings: Vec<ArePayloadBacking>,
    ) -> AreEventStream {
        AreEventStream {
            y_min,
            y_max,
            rows,
            objects,
            payload_backings,
        }
    }

    fn row(row_y: i32, cursors: Vec<AreCursorState>) -> AreEventRow {
        AreEventRow { row_y, cursors }
    }

    fn cursor(current_x: i32, next_x: i32, event_index: usize) -> AreCursorState {
        AreCursorState {
            current_x,
            next_x,
            event_index,
            materialize_flag: false,
        }
    }

    fn object(event_class: EventClass, state: u8) -> AreEventObject {
        AreEventObject {
            event_class,
            payload_window: None,
            state,
        }
    }

    fn class2(window: ArePayloadWindow) -> AreEventObject {
        AreEventObject {
            event_class: EventClass::Class2,
            payload_window: Some(window),
            state: 0,
        }
    }

    fn backing(bytes: &[u8]) -> ArePayloadBacking {
        ArePayloadBacking {
            id: ArePayloadBackingId(0),
            bytes: bytes.to_vec(),
        }
    }

    fn window(offset: usize, len: usize) -> ArePayloadWindow {
        ArePayloadWindow {
            backing_id: ArePayloadBackingId(0),
            offset,
            len,
        }
    }

    #[test]
    fn empty_rows_preserve_bounds() {
        let table = emit_ad68_rows(&stream(10, 12, vec![], vec![], vec![])).unwrap();
        assert_eq!(table.y_min, 10);
        assert_eq!(table.y_max, 12);
        assert_eq!(table.rows, vec![vec![], vec![], vec![]]);
    }

    #[test]
    fn class0_state_node() {
        let table = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(4, 8, 0)])],
            vec![object(EventClass::Class0, 7)],
            vec![],
        ))
        .unwrap();
        assert_eq!(
            table.rows[0],
            vec![Ad68RowNode {
                x: 4,
                len: 0,
                bytes: None,
                state: 7,
            }]
        );
    }

    #[test]
    fn class1_state_node() {
        let table = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(2, 5, 0)])],
            vec![object(EventClass::Class1, 11)],
            vec![],
        ))
        .unwrap();
        assert_eq!(
            table.rows[0],
            vec![Ad68RowNode {
                x: 2,
                len: 0,
                bytes: None,
                state: 11,
            }]
        );
    }

    #[test]
    fn class2_payload_node() {
        let table = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(3, 7, 0)])],
            vec![class2(window(0, 4))],
            vec![backing(&[1, 2, 3, 4])],
        ))
        .unwrap();
        assert_eq!(
            table.rows[0],
            vec![Ad68RowNode {
                x: 3,
                len: 4,
                bytes: Some(vec![1, 2, 3, 4]),
                state: 0,
            }]
        );
    }

    #[test]
    fn mixed_row_topology() {
        let table = emit_ad68_rows(&stream(
            5,
            5,
            vec![row(
                5,
                vec![cursor(0, 2, 0), cursor(2, 6, 1), cursor(6, 8, 2)],
            )],
            vec![
                object(EventClass::Class0, 3),
                class2(window(1, 4)),
                object(EventClass::Class1, 9),
            ],
            vec![backing(&[0, 10, 11, 12, 13, 99])],
        ))
        .unwrap();
        assert_eq!(
            table.rows[0],
            vec![
                Ad68RowNode {
                    x: 0,
                    len: 0,
                    bytes: None,
                    state: 3,
                },
                Ad68RowNode {
                    x: 2,
                    len: 4,
                    bytes: Some(vec![10, 11, 12, 13]),
                    state: 0,
                },
                Ad68RowNode {
                    x: 6,
                    len: 0,
                    bytes: None,
                    state: 9,
                },
            ]
        );
    }

    #[test]
    fn row_y_basis() {
        let table = emit_ad68_rows(&stream(
            10,
            12,
            vec![
                row(10, vec![cursor(1, 2, 0)]),
                row(12, vec![cursor(3, 4, 0)]),
            ],
            vec![object(EventClass::Class0, 5)],
            vec![],
        ))
        .unwrap();
        assert_eq!(table.rows[0][0].x, 1);
        assert!(table.rows[1].is_empty());
        assert_eq!(table.rows[2][0].x, 3);
    }

    #[test]
    fn payload_window_offset() {
        let table = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(0, 4, 0)])],
            vec![class2(window(2, 4))],
            vec![backing(&[9, 8, 1, 2, 3, 4])],
        ))
        .unwrap();
        assert_eq!(table.rows[0][0].bytes, Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn invalid_event_index_errors() {
        let err = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(0, 1, 4)])],
            vec![object(EventClass::Class0, 0)],
            vec![],
        ))
        .unwrap_err();
        assert_eq!(
            err,
            Ad68EmitError::InvalidEventIndex {
                row_y: 0,
                event_index: 4,
                objects_len: 1,
            }
        );
    }

    #[test]
    fn invalid_window_bounds_errors() {
        let err = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(0, 2, 0)])],
            vec![class2(window(2, 4))],
            vec![backing(&[1, 2, 3])],
        ))
        .unwrap_err();
        assert_eq!(
            err,
            Ad68EmitError::InvalidPayloadWindow {
                row_y: 0,
                event_index: 0,
                backing_id: 0,
                offset: 2,
                len: 4,
                backing_len: 3,
            }
        );
    }

    #[test]
    fn negative_width_errors() {
        let err = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(5, 4, 0)])],
            vec![object(EventClass::Class0, 0)],
            vec![],
        ))
        .unwrap_err();
        assert_eq!(
            err,
            Ad68EmitError::InvalidCursorWidth {
                row_y: 0,
                current_x: 5,
                next_x: 4,
            }
        );
    }

    #[test]
    fn class2_missing_payload_errors() {
        let err = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(0, 1, 0)])],
            vec![object(EventClass::Class2, 0)],
            vec![],
        ))
        .unwrap_err();
        assert_eq!(
            err,
            Ad68EmitError::MissingPayloadWindow {
                row_y: 0,
                event_index: 0,
            }
        );
    }

    #[test]
    fn class2_payload_window_shorter_than_width_errors() {
        let err = emit_ad68_rows(&stream(
            0,
            0,
            vec![row(0, vec![cursor(0, 4, 0)])],
            vec![class2(window(0, 3))],
            vec![backing(&[1, 2, 3, 4])],
        ))
        .unwrap_err();
        assert_eq!(
            err,
            Ad68EmitError::PayloadWindowShorterThanWidth {
                row_y: 0,
                event_index: 0,
                window_len: 3,
                width: 4,
            }
        );
    }
}
