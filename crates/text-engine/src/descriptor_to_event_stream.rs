use crate::{
    AreCursorState, AreDescriptorBuilderRoot, AreDescriptorRef, AreEventDescriptor, AreEventObject,
    AreEventRow, AreEventStream, ArePayloadBackingId, ArePayloadWindow, DescriptorId, EventClass,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorCursorInput {
    pub y_min: i32,
    pub y_max: i32,
    pub rows: Vec<AreDescriptorCursorRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorCursorRow {
    pub row_y: i32,
    pub events: Vec<AreDescriptorCursorEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorCursorEvent {
    pub current_x: i32,
    pub next_x: i32,
    pub descriptor_id: DescriptorId,
    pub materialize_flag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorCursorSpan {
    pub row_y: i32,
    pub current_x: i32,
    pub next_x: i32,
    pub descriptor_id: DescriptorId,
    pub materialize_flag: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorToEventStreamError {
    InvalidRowBounds {
        y_min: i32,
        y_max: i32,
    },
    RowOutOfBounds {
        row_y: i32,
        y_min: i32,
        y_max: i32,
    },
    NextXBeforeCurrentX {
        row_y: i32,
        current_x: i32,
        next_x: i32,
    },
    UnknownDescriptorId {
        descriptor_id: DescriptorId,
        descriptors_len: usize,
    },
    DescriptorOutsideVector {
        descriptor_id: DescriptorId,
    },
    ConflictingClassRegistration {
        descriptor_id: DescriptorId,
    },
    DescriptorWithoutClassRegistration {
        descriptor_id: DescriptorId,
    },
    Class2DescriptorWithoutPayload {
        descriptor_id: DescriptorId,
    },
    MissingPayloadBacking {
        descriptor_id: DescriptorId,
        backing_id: ArePayloadBackingId,
    },
    PayloadWindowShorterThanSpan {
        descriptor_id: DescriptorId,
        window_len: usize,
        span_width: usize,
    },
}

pub fn descriptor_builder_to_event_stream(
    builder: &AreDescriptorBuilderRoot,
    input: &AreDescriptorCursorInput,
) -> Result<AreEventStream, DescriptorToEventStreamError> {
    build_event_stream_from_descriptor_cursor(builder, input)
}

pub fn build_event_stream_from_descriptor_cursor(
    builder: &AreDescriptorBuilderRoot,
    input: &AreDescriptorCursorInput,
) -> Result<AreEventStream, DescriptorToEventStreamError> {
    if input.y_max < input.y_min {
        return Err(DescriptorToEventStreamError::InvalidRowBounds {
            y_min: input.y_min,
            y_max: input.y_max,
        });
    }

    let mut rows = Vec::with_capacity(input.rows.len());
    let mut objects = Vec::new();
    for row in &input.rows {
        if row.row_y < input.y_min || row.row_y > input.y_max {
            return Err(DescriptorToEventStreamError::RowOutOfBounds {
                row_y: row.row_y,
                y_min: input.y_min,
                y_max: input.y_max,
            });
        }

        let mut cursors = Vec::with_capacity(row.events.len());
        for event in &row.events {
            let descriptor = descriptor(builder, event.descriptor_id)?;
            let event_class = descriptor_event_class(builder, descriptor, event.materialize_flag)?;
            let width = span_width(row.row_y, event.current_x, event.next_x)?;
            let payload_window = payload_window(builder, descriptor, event_class, width)?;

            let event_index = objects.len();
            objects.push(AreEventObject {
                event_class,
                payload_window,
                state: descriptor.state,
            });
            cursors.push(AreCursorState {
                current_x: event.current_x,
                next_x: event.next_x,
                event_index,
                materialize_flag: event.materialize_flag,
            });
        }
        rows.push(AreEventRow {
            row_y: row.row_y,
            cursors,
        });
    }

    Ok(AreEventStream {
        y_min: input.y_min,
        y_max: input.y_max,
        rows,
        objects,
        payload_backings: builder.payload_allocator.backings.clone(),
    })
}

fn descriptor(
    builder: &AreDescriptorBuilderRoot,
    descriptor_id: DescriptorId,
) -> Result<&AreEventDescriptor, DescriptorToEventStreamError> {
    let descriptor = builder.descriptor(descriptor_id).ok_or(
        DescriptorToEventStreamError::UnknownDescriptorId {
            descriptor_id,
            descriptors_len: builder.descriptors.len(),
        },
    )?;
    if !builder
        .descriptor_vector
        .elements
        .contains(&AreDescriptorRef { id: descriptor_id })
    {
        return Err(DescriptorToEventStreamError::DescriptorOutsideVector { descriptor_id });
    }
    Ok(descriptor)
}

fn descriptor_event_class(
    builder: &AreDescriptorBuilderRoot,
    descriptor: &AreEventDescriptor,
    materialize_flag: bool,
) -> Result<EventClass, DescriptorToEventStreamError> {
    let descriptor_ref = AreDescriptorRef { id: descriptor.id };
    let in_ctx_08 = builder.ctx_08.descriptors.contains(&descriptor_ref);
    let in_ctx_58 = builder.ctx_58.descriptors.contains(&descriptor_ref);

    if in_ctx_08 && in_ctx_58 {
        return Err(DescriptorToEventStreamError::ConflictingClassRegistration {
            descriptor_id: descriptor.id,
        });
    }

    if !in_ctx_08 && !in_ctx_58 {
        return Err(
            DescriptorToEventStreamError::DescriptorWithoutClassRegistration {
                descriptor_id: descriptor.id,
            },
        );
    }

    if materialize_flag {
        return Ok(EventClass::Class2);
    }
    if in_ctx_08 {
        return Ok(EventClass::Class0);
    }
    Ok(EventClass::Class1)
}

fn span_width(
    row_y: i32,
    current_x: i32,
    next_x: i32,
) -> Result<usize, DescriptorToEventStreamError> {
    let width =
        next_x
            .checked_sub(current_x)
            .ok_or(DescriptorToEventStreamError::NextXBeforeCurrentX {
                row_y,
                current_x,
                next_x,
            })?;
    if width <= 0 {
        return Err(DescriptorToEventStreamError::NextXBeforeCurrentX {
            row_y,
            current_x,
            next_x,
        });
    }
    Ok(width as usize)
}

fn payload_window(
    builder: &AreDescriptorBuilderRoot,
    descriptor: &AreEventDescriptor,
    event_class: EventClass,
    span_width: usize,
) -> Result<Option<ArePayloadWindow>, DescriptorToEventStreamError> {
    if event_class != EventClass::Class2 {
        return Ok(None);
    }
    let window = descriptor.payload_window.ok_or(
        DescriptorToEventStreamError::Class2DescriptorWithoutPayload {
            descriptor_id: descriptor.id,
        },
    )?;
    if builder
        .payload_allocator
        .backing(window.backing_id)
        .is_none()
    {
        return Err(DescriptorToEventStreamError::MissingPayloadBacking {
            descriptor_id: descriptor.id,
            backing_id: window.backing_id,
        });
    }
    if window.len < span_width {
        return Err(DescriptorToEventStreamError::PayloadWindowShorterThanSpan {
            descriptor_id: descriptor.id,
            window_len: window.len,
            span_width,
        });
    }
    Ok(Some(window))
}

impl AreDescriptorCursorInput {
    pub fn new(y_min: i32, y_max: i32, rows: Vec<AreDescriptorCursorRow>) -> Self {
        Self { y_min, y_max, rows }
    }
}

impl AreDescriptorCursorRow {
    pub fn new(row_y: i32, events: Vec<AreDescriptorCursorEvent>) -> Self {
        Self { row_y, events }
    }
}

impl AreDescriptorCursorEvent {
    pub fn new(
        current_x: i32,
        next_x: i32,
        descriptor_id: DescriptorId,
        materialize_flag: bool,
    ) -> Self {
        Self {
            current_x,
            next_x,
            descriptor_id,
            materialize_flag,
        }
    }
}

impl From<AreDescriptorCursorSpan> for AreDescriptorCursorRow {
    fn from(span: AreDescriptorCursorSpan) -> Self {
        Self {
            row_y: span.row_y,
            events: vec![AreDescriptorCursorEvent {
                current_x: span.current_x,
                next_x: span.next_x,
                descriptor_id: span.descriptor_id,
                materialize_flag: span.materialize_flag,
            }],
        }
    }
}

#[cfg(test)]
mod descriptor_to_event_stream_tests {
    use super::*;
    use crate::{
        emit_ad68_rows, AreDescriptorBuilderRoot, AreDescriptorTriplet,
        AreEventDescriptorSourceObject, AreEventDescriptorVector, ArePayloadBackingId,
    };

    fn triplet() -> AreDescriptorTriplet {
        AreDescriptorTriplet {
            base: 0x1000,
            second: 0x2000,
            source: 0x3000,
        }
    }

    fn event(
        current_x: i32,
        next_x: i32,
        descriptor_id: DescriptorId,
        materialize_flag: bool,
    ) -> AreDescriptorCursorEvent {
        AreDescriptorCursorEvent::new(current_x, next_x, descriptor_id, materialize_flag)
    }

    fn input(row_y: i32, events: Vec<AreDescriptorCursorEvent>) -> AreDescriptorCursorInput {
        AreDescriptorCursorInput::new(
            row_y,
            row_y,
            vec![AreDescriptorCursorRow::new(row_y, events)],
        )
    }

    fn registered_descriptor(
        source_object: AreEventDescriptorSourceObject,
    ) -> (AreDescriptorBuilderRoot, DescriptorId) {
        let mut root = AreDescriptorBuilderRoot::new();
        let id = root.define_descriptor(triplet(), source_object);
        root.register_descriptor(id);
        (root, id)
    }

    fn class2_descriptor(bytes: &[u8]) -> (AreDescriptorBuilderRoot, DescriptorId) {
        let (mut root, id) =
            registered_descriptor(AreEventDescriptorSourceObject::with_0x20(0x2220));
        let window = root.payload_allocator.allocate(bytes.to_vec());
        assert!(root.set_descriptor_payload_window(id, window));
        (root, id)
    }

    fn class2_descriptor_with_window(
        root: &mut AreDescriptorBuilderRoot,
        bytes: &[u8],
        offset: usize,
        len: usize,
    ) -> DescriptorId {
        let id = root.define_descriptor(triplet(), AreEventDescriptorSourceObject::with_0x20(2));
        root.register_descriptor(id);
        let backing_window = root.payload_allocator.allocate(bytes.to_vec());
        assert!(root.set_descriptor_payload_window(
            id,
            ArePayloadWindow {
                backing_id: backing_window.backing_id,
                offset,
                len,
            },
        ));
        id
    }

    struct DescriptorCursorMultiRowFixture {
        root: AreDescriptorBuilderRoot,
        input: AreDescriptorCursorInput,
        descriptor_refs: Vec<AreDescriptorRef>,
        class0: DescriptorId,
        class1: DescriptorId,
        class2_first: DescriptorId,
        class2_offset: DescriptorId,
        class2_gap: DescriptorId,
    }

    fn build_descriptor_cursor_multi_row_fixture() -> DescriptorCursorMultiRowFixture {
        let mut root = AreDescriptorBuilderRoot::new();

        let class0 =
            root.define_descriptor(triplet(), AreEventDescriptorSourceObject::with_0x18(0x1118));
        root.register_descriptor(class0);
        root.set_descriptor_state(class0, 0x10);

        let class1 =
            root.define_descriptor(triplet(), AreEventDescriptorSourceObject::with_0x20(0x2220));
        root.register_descriptor(class1);
        root.set_descriptor_state(class1, 0x20);

        let class2_first = class2_descriptor_with_window(&mut root, &[1, 2, 3], 0, 3);
        let class2_offset = class2_descriptor_with_window(&mut root, &[90, 10, 11, 91], 1, 2);
        let class2_gap = class2_descriptor_with_window(&mut root, &[20, 21, 22, 23], 0, 3);

        let descriptor_refs = root.descriptor_vector.elements.clone();
        let input = AreDescriptorCursorInput::new(
            10,
            13,
            vec![
                AreDescriptorCursorRow::new(10, vec![event(2, 5, class2_first, true)]),
                AreDescriptorCursorRow::new(11, vec![event(1, 4, class0, false)]),
                AreDescriptorCursorRow::new(12, vec![event(3, 6, class1, false)]),
                AreDescriptorCursorRow::new(
                    13,
                    vec![
                        event(0, 2, class2_offset, true),
                        event(5, 7, class0, false),
                        event(9, 12, class2_gap, true),
                    ],
                ),
            ],
        );

        DescriptorCursorMultiRowFixture {
            root,
            input,
            descriptor_refs,
            class0,
            class1,
            class2_first,
            class2_offset,
            class2_gap,
        }
    }

    #[test]
    fn class2_descriptor_to_span_event() {
        let (root, id) = class2_descriptor(&[9, 8, 7, 6]);

        let stream = build_event_stream_from_descriptor_cursor(
            &root,
            &input(4, vec![event(10, 14, id, true)]),
        )
        .unwrap();

        assert_eq!(stream.objects[0].event_class, EventClass::Class2);
        assert_eq!(
            stream.objects[0].payload_window,
            root.descriptor(id).unwrap().payload_window
        );
        assert_eq!(stream.rows[0].cursors[0].event_index, 0);
    }

    #[test]
    fn class2_descriptor_to_ad68_row_node() {
        let (root, id) = class2_descriptor(&[1, 2, 3, 4, 5]);
        let stream =
            descriptor_builder_to_event_stream(&root, &input(0, vec![event(3, 7, id, true)]))
                .unwrap();

        let table = emit_ad68_rows(&stream).unwrap();

        assert_eq!(table.rows[0][0].x, 3);
        assert_eq!(table.rows[0][0].len, 4);
        assert_eq!(table.rows[0][0].bytes, Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn class0_descriptor_to_state_event() {
        let (mut root, id) =
            registered_descriptor(AreEventDescriptorSourceObject::with_0x18(0x1118));
        assert!(root.set_descriptor_state(id, 7));

        let stream =
            descriptor_builder_to_event_stream(&root, &input(0, vec![event(1, 5, id, false)]))
                .unwrap();

        assert_eq!(stream.objects[0].event_class, EventClass::Class0);
        assert_eq!(stream.objects[0].state, 7);
        assert_eq!(stream.objects[0].payload_window, None);
    }

    #[test]
    fn class1_descriptor_to_state_event() {
        let (mut root, id) =
            registered_descriptor(AreEventDescriptorSourceObject::with_0x20(0x2220));
        assert!(root.set_descriptor_state(id, 11));

        let stream =
            descriptor_builder_to_event_stream(&root, &input(0, vec![event(1, 5, id, false)]))
                .unwrap();

        assert_eq!(stream.objects[0].event_class, EventClass::Class1);
        assert_eq!(stream.objects[0].state, 11);
        assert_eq!(stream.objects[0].payload_window, None);
    }

    #[test]
    fn mixed_class_vectors_preserve_row_order() {
        let (mut root, class0) =
            registered_descriptor(AreEventDescriptorSourceObject::with_0x18(0x1118));
        let class1 =
            root.define_descriptor(triplet(), AreEventDescriptorSourceObject::with_0x20(2));
        root.register_descriptor(class1);
        let class2 =
            root.define_descriptor(triplet(), AreEventDescriptorSourceObject::with_0x20(3));
        root.register_descriptor(class2);
        let window = root.payload_allocator.allocate(vec![5, 6, 7]);
        root.set_descriptor_payload_window(class2, window);
        root.set_descriptor_state(class0, 4);
        root.set_descriptor_state(class1, 8);

        let stream = descriptor_builder_to_event_stream(
            &root,
            &input(
                9,
                vec![
                    event(0, 1, class0, false),
                    event(1, 4, class2, true),
                    event(4, 5, class1, false),
                ],
            ),
        )
        .unwrap();
        let table = emit_ad68_rows(&stream).unwrap();

        assert_eq!(stream.rows[0].cursors[0].event_index, 0);
        assert_eq!(stream.rows[0].cursors[1].event_index, 1);
        assert_eq!(stream.rows[0].cursors[2].event_index, 2);
        assert_eq!(table.rows[0][0].state, 4);
        assert_eq!(table.rows[0][1].bytes, Some(vec![5, 6, 7]));
        assert_eq!(table.rows[0][2].state, 8);
    }

    #[test]
    fn descriptor_cursor_multi_row_fixture_to_ad68_rows() {
        let fixture = build_descriptor_cursor_multi_row_fixture();

        let stream = descriptor_builder_to_event_stream(&fixture.root, &fixture.input).unwrap();
        let table = emit_ad68_rows(&stream).unwrap();

        assert_eq!(
            fixture.root.descriptor_vector.elements,
            fixture.descriptor_refs
        );
        assert_eq!(
            fixture.descriptor_refs,
            vec![
                AreDescriptorRef { id: fixture.class0 },
                AreDescriptorRef { id: fixture.class1 },
                AreDescriptorRef {
                    id: fixture.class2_first
                },
                AreDescriptorRef {
                    id: fixture.class2_offset
                },
                AreDescriptorRef {
                    id: fixture.class2_gap
                },
            ]
        );

        assert!(fixture
            .root
            .ctx_08
            .descriptors
            .contains(&AreDescriptorRef { id: fixture.class0 }));
        assert!(fixture
            .root
            .ctx_58
            .descriptors
            .contains(&AreDescriptorRef { id: fixture.class1 }));
        assert!(fixture.root.ctx_58.descriptors.contains(&AreDescriptorRef {
            id: fixture.class2_first,
        }));

        assert_eq!(
            stream
                .objects
                .iter()
                .map(|object| object.event_class)
                .collect::<Vec<_>>(),
            vec![
                EventClass::Class2,
                EventClass::Class0,
                EventClass::Class1,
                EventClass::Class2,
                EventClass::Class0,
                EventClass::Class2,
            ]
        );
        assert_eq!(table.y_min, 10);
        assert_eq!(table.rows.len(), 4);

        let row_10 = table.row(10).unwrap();
        assert_eq!(row_10.len(), 1);
        assert_eq!(row_10[0].x, 2);
        assert_eq!(row_10[0].len, 3);
        assert_eq!(row_10[0].bytes(), Some(&[1, 2, 3][..]));

        let row_11 = table.row(11).unwrap();
        assert_eq!(row_11.len(), 1);
        assert_eq!(row_11[0].x, 1);
        assert_eq!(row_11[0].len, 0);
        assert_eq!(row_11[0].state, 0x10);

        let row_12 = table.row(12).unwrap();
        assert_eq!(row_12.len(), 1);
        assert_eq!(row_12[0].x, 3);
        assert_eq!(row_12[0].len, 0);
        assert_eq!(row_12[0].state, 0x20);

        let mixed = table.row(13).unwrap();
        assert_eq!(mixed.len(), 3);
        assert_eq!(mixed[0].x, 0);
        assert_eq!(mixed[0].len, 2);
        assert_eq!(mixed[0].bytes(), Some(&[10, 11][..]));
        assert_eq!(mixed[1].x, 5);
        assert_eq!(mixed[1].len, 0);
        assert_eq!(mixed[1].state, 0x10);
        assert_eq!(mixed[2].x, 9);
        assert_eq!(mixed[2].len, 3);
        assert_eq!(mixed[2].bytes(), Some(&[20, 21, 22][..]));
    }

    #[test]
    fn unknown_descriptor_id_errors() {
        let root = AreDescriptorBuilderRoot::new();

        let err = descriptor_builder_to_event_stream(
            &root,
            &input(0, vec![event(0, 1, DescriptorId(4), false)]),
        )
        .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::UnknownDescriptorId {
                descriptor_id: DescriptorId(4),
                descriptors_len: 0,
            }
        );
    }

    #[test]
    fn descriptor_without_class_registration_errors() {
        let mut root = AreDescriptorBuilderRoot::new();
        let id = root.define_descriptor(triplet(), AreEventDescriptorSourceObject::empty());

        let err =
            descriptor_builder_to_event_stream(&root, &input(0, vec![event(0, 1, id, false)]))
                .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::DescriptorWithoutClassRegistration { descriptor_id: id }
        );
    }

    #[test]
    fn class2_descriptor_without_payload_errors() {
        let (root, id) = registered_descriptor(AreEventDescriptorSourceObject::with_0x20(0x2220));

        let err = descriptor_builder_to_event_stream(&root, &input(0, vec![event(0, 4, id, true)]))
            .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::Class2DescriptorWithoutPayload { descriptor_id: id }
        );
    }

    #[test]
    fn next_x_before_current_x_errors() {
        let (root, id) = registered_descriptor(AreEventDescriptorSourceObject::with_0x18(0x1118));

        let err =
            descriptor_builder_to_event_stream(&root, &input(3, vec![event(5, 4, id, false)]))
                .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::NextXBeforeCurrentX {
                row_y: 3,
                current_x: 5,
                next_x: 4,
            }
        );
    }

    #[test]
    fn next_x_equal_current_x_errors() {
        let (root, id) = registered_descriptor(AreEventDescriptorSourceObject::with_0x18(0x1118));

        let err =
            descriptor_builder_to_event_stream(&root, &input(3, vec![event(5, 5, id, false)]))
                .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::NextXBeforeCurrentX {
                row_y: 3,
                current_x: 5,
                next_x: 5,
            }
        );
    }

    #[test]
    fn conflicting_class_vectors_errors() {
        let (root, id) =
            registered_descriptor(AreEventDescriptorSourceObject::with_0x18_0x20(1, 2));

        let err =
            descriptor_builder_to_event_stream(&root, &input(0, vec![event(0, 1, id, false)]))
                .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::ConflictingClassRegistration { descriptor_id: id }
        );
    }

    #[test]
    fn cursor_references_descriptor_outside_vector_errors() {
        let (mut root, id) =
            registered_descriptor(AreEventDescriptorSourceObject::with_0x18(0x1118));
        root.descriptor_vector.elements.clear();

        let err =
            descriptor_builder_to_event_stream(&root, &input(0, vec![event(0, 1, id, false)]))
                .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::DescriptorOutsideVector { descriptor_id: id }
        );
    }

    #[test]
    fn payload_window_shorter_than_span_errors() {
        let (mut root, id) =
            registered_descriptor(AreEventDescriptorSourceObject::with_0x20(0x2220));
        let window = root.payload_allocator.allocate(vec![1, 2]);
        root.set_descriptor_payload_window(id, window);

        let err = descriptor_builder_to_event_stream(&root, &input(0, vec![event(0, 3, id, true)]))
            .unwrap_err();

        assert_eq!(
            err,
            DescriptorToEventStreamError::PayloadWindowShorterThanSpan {
                descriptor_id: id,
                window_len: 2,
                span_width: 3,
            }
        );
    }

    #[test]
    fn descriptor_vector_ref_not_inline_record() {
        let (root, id) = class2_descriptor(&[1, 2, 3, 4]);

        assert_eq!(AreEventDescriptorVector::ELEMENT_SIZE_BYTES, 8);
        assert_eq!(
            root.descriptor_vector.elements,
            vec![AreDescriptorRef { id }]
        );

        let stream =
            descriptor_builder_to_event_stream(&root, &input(0, vec![event(0, 4, id, true)]))
                .unwrap();
        assert_eq!(stream.objects.len(), 1);
        assert_eq!(stream.payload_backings[0].id, ArePayloadBackingId(0));
    }
}
