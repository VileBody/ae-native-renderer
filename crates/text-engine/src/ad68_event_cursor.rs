use crate::{
    emit_ad68_rows, Ad68EmitError, Ad68RowTable, Are95ccStateClass, AreCursorState, AreEventRow,
    AreEventStream, AreMaterializedInterval, ArePayloadBacking, ArePayloadBackingId,
    ArePayloadWindow, AreSamplerMaterialization, AreSamplerMaterializationProvenance, EventClass,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreAd68EventCursor {
    pub y_min: i32,
    pub y_max: i32,
    pub x_min: i32,
    pub event_objects: AreEventObjectTable,
    pub class_map: AreEventClassMap,
    pub payload_backing: AreEventPayloadBacking,
    pub rows: Vec<AreAd68EventCursorRow>,
    pub active_row_y: Option<i32>,
    pub current_event_index: usize,
    pub provenance: AreAd68EventCursorProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventObjectTable {
    pub objects: Vec<AreEventObject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventObject {
    pub kind: AreEventObjectKind,
    pub payload_base_offset: usize,
    pub row_local_payload_base: usize,
    pub current_payload_offset: usize,
    pub payload_stride: usize,
    pub row_stride: usize,
    pub state: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreEventObjectKind {
    Class0,
    Class1,
    Class2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventPayloadBacking {
    pub id: ArePayloadBackingId,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventPayloadWindow {
    pub backing_id: ArePayloadBackingId,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventClassMap {
    pub class0_event_index: usize,
    pub class1_event_index: usize,
    pub class2_event_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSampleAdvanceResult {
    pub row_y: i32,
    pub current_x: i32,
    pub next_x: i32,
    pub event_index: usize,
    pub event_class: AreEventObjectKind,
    pub materialize_flag: bool,
    pub payload_window: Option<AreEventPayloadWindow>,
    pub state: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreRowPrimerResult {
    pub row_y: i32,
    pub object_payload_offsets: Vec<ArePrimedEventObject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArePrimedEventObject {
    pub event_index: usize,
    pub row_local_payload_base: usize,
    pub current_payload_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreAd68EventCursorRow {
    pub row_y: i32,
    pub samples: Vec<AreSampleAdvanceResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreAd68EventCursorProvenance {
    SourceOwnedSamplerMaterialized,
    TypedSpanFallback,
    CoverageRowFallback,
    FixturePayloadFallback,
    SyntheticPayloadFallback,
    IntervalObject1b898,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreEventCursorError {
    InvalidRowBounds {
        y_min: i32,
        y_max: i32,
    },
    RowOutOfBounds {
        row_y: i32,
        y_min: i32,
        y_max: i32,
    },
    SampleXBeforeMin {
        sample_x: i32,
        x_min: i32,
    },
    NoPrimedRow,
    InvalidEventIndex {
        event_index: usize,
        objects_len: usize,
    },
    MissingClass2Object,
    PayloadOffsetOutOfBounds {
        offset: usize,
        backing_len: usize,
    },
    PayloadWindowOutOfBounds {
        offset: usize,
        len: usize,
        backing_len: usize,
    },
    InvalidCoverage {
        coverage: u16,
    },
    InvalidSampleWidth {
        current_x: i32,
        next_x: i32,
    },
    InvalidMaterializedPayloadLength {
        row_y: i32,
        run_index: usize,
        payload_len: usize,
        width: usize,
    },
    Ad68(Ad68EmitError),
    TypedSpanFallbackRejected,
    CoverageRowFallbackRejected,
    FixturePayloadFallbackRejected,
    SyntheticPayloadFallbackRejected,
    IntervalObject1b898PayloadBridgeRejected,
}

pub fn prime_event_cursor_row(
    cursor: &mut AreAd68EventCursor,
    row_y: i32,
) -> Result<AreRowPrimerResult, AreEventCursorError> {
    validate_source_owned(cursor)?;
    validate_row_bounds(cursor)?;
    if row_y < cursor.y_min || row_y > cursor.y_max {
        return Err(AreEventCursorError::RowOutOfBounds {
            row_y,
            y_min: cursor.y_min,
            y_max: cursor.y_max,
        });
    }

    let row_index = (row_y - cursor.y_min) as usize;
    let backing_len = cursor.payload_backing.bytes.len();
    let mut object_payload_offsets = Vec::with_capacity(cursor.event_objects.objects.len());

    for (event_index, object) in cursor.event_objects.objects.iter_mut().enumerate() {
        let row_offset = row_index
            .checked_mul(object.row_stride)
            .and_then(|offset| object.payload_base_offset.checked_add(offset))
            .ok_or(AreEventCursorError::PayloadOffsetOutOfBounds {
                offset: usize::MAX,
                backing_len,
            })?;
        if object.kind == AreEventObjectKind::Class2 && row_offset > backing_len {
            return Err(AreEventCursorError::PayloadOffsetOutOfBounds {
                offset: row_offset,
                backing_len,
            });
        }
        object.row_local_payload_base = row_offset;
        object.current_payload_offset = row_offset;
        object_payload_offsets.push(ArePrimedEventObject {
            event_index,
            row_local_payload_base: object.row_local_payload_base,
            current_payload_offset: object.current_payload_offset,
        });
    }

    cursor.active_row_y = Some(row_y);
    ensure_active_row(cursor, row_y);

    Ok(AreRowPrimerResult {
        row_y,
        object_payload_offsets,
    })
}

pub fn advance_event_cursor_sample_x(
    cursor: &mut AreAd68EventCursor,
    sample_x: i32,
) -> Result<AreSampleAdvanceResult, AreEventCursorError> {
    validate_source_owned(cursor)?;
    let row_y = cursor
        .active_row_y
        .ok_or(AreEventCursorError::NoPrimedRow)?;
    advance_payload_pointers(cursor, sample_x)?;

    let event_index = cursor.current_event_index;
    let object = cursor.event_objects.objects.get(event_index).ok_or(
        AreEventCursorError::InvalidEventIndex {
            event_index,
            objects_len: cursor.event_objects.objects.len(),
        },
    )?;

    Ok(AreSampleAdvanceResult {
        row_y,
        current_x: sample_x,
        next_x: sample_x + 1,
        event_index,
        event_class: object.kind,
        materialize_flag: false,
        payload_window: event_payload_window(object, 1),
        state: object.state,
    })
}

pub fn classify_sample_coverage(
    coverage_0_to_0x100: u16,
) -> Result<AreEventObjectKind, AreEventCursorError> {
    match coverage_0_to_0x100 {
        0 => Ok(AreEventObjectKind::Class0),
        0x100 => Ok(AreEventObjectKind::Class1),
        1..=0xff => Ok(AreEventObjectKind::Class2),
        coverage => Err(AreEventCursorError::InvalidCoverage { coverage }),
    }
}

pub fn write_class2_payload_byte(
    cursor: &mut AreAd68EventCursor,
    sample_x: i32,
    coverage_0_to_0x100: u16,
) -> Result<AreSampleAdvanceResult, AreEventCursorError> {
    validate_source_owned(cursor)?;
    let row_y = cursor
        .active_row_y
        .ok_or(AreEventCursorError::NoPrimedRow)?;
    advance_payload_pointers(cursor, sample_x)?;
    let event_class = classify_sample_coverage(coverage_0_to_0x100)?;
    let event_index = cursor.class_map.event_index_for(event_class);
    cursor.current_event_index = event_index;

    let objects_len = cursor.event_objects.objects.len();
    let object = cursor.event_objects.objects.get(event_index).ok_or(
        AreEventCursorError::InvalidEventIndex {
            event_index,
            objects_len,
        },
    )?;
    if object.kind != event_class {
        return Err(AreEventCursorError::InvalidEventIndex {
            event_index,
            objects_len,
        });
    }

    let payload_window = if event_class == AreEventObjectKind::Class2 {
        let offset = object.current_payload_offset;
        let backing_len = cursor.payload_backing.bytes.len();
        if offset >= backing_len {
            return Err(AreEventCursorError::PayloadOffsetOutOfBounds {
                offset,
                backing_len,
            });
        }
        cursor.payload_backing.bytes[offset] = coverage_0_to_0x100 as u8;
        Some(AreEventPayloadWindow {
            backing_id: cursor.payload_backing.id,
            offset,
            len: 1,
        })
    } else {
        None
    };

    let result = AreSampleAdvanceResult {
        row_y,
        current_x: sample_x,
        next_x: sample_x + 1,
        event_index,
        event_class,
        materialize_flag: false,
        payload_window,
        state: object.state,
    };
    ensure_active_row(cursor, row_y)
        .samples
        .push(result.clone());
    Ok(result)
}

pub fn event_cursor_to_event_stream(
    cursor: &AreAd68EventCursor,
) -> Result<AreEventStream, AreEventCursorError> {
    validate_source_owned(cursor)?;
    validate_row_bounds(cursor)?;

    let mut rows = Vec::with_capacity(cursor.rows.len());
    let mut objects = Vec::new();

    for row in &cursor.rows {
        if row.row_y < cursor.y_min || row.row_y > cursor.y_max {
            return Err(AreEventCursorError::RowOutOfBounds {
                row_y: row.row_y,
                y_min: cursor.y_min,
                y_max: cursor.y_max,
            });
        }
        let spans = coalesced_spans(&row.samples)?;
        let mut cursors = Vec::with_capacity(spans.len());
        for span in spans {
            let event_index = objects.len();
            let event_class = event_class_for(span.event_class);
            let payload_window = span.payload_window.map(payload_window_for_stream);
            objects.push(crate::AreEventObject {
                event_class,
                payload_window,
                state: span.state,
            });
            cursors.push(AreCursorState {
                current_x: span.current_x,
                next_x: span.next_x,
                event_index,
                materialize_flag: span.materialize_flag,
            });
        }
        rows.push(AreEventRow {
            row_y: row.row_y,
            cursors,
        });
    }

    Ok(AreEventStream {
        y_min: cursor.y_min,
        y_max: cursor.y_max,
        rows,
        objects,
        payload_backings: vec![ArePayloadBacking {
            id: cursor.payload_backing.id,
            bytes: cursor.payload_backing.bytes.clone(),
        }],
    })
}

pub fn materialized_intervals_to_ad68_event_cursor(
    materialization: &AreSamplerMaterialization,
) -> Result<AreAd68EventCursor, AreEventCursorError> {
    materialized_intervals_to_ad68_event_cursor_with_provenance(
        materialization,
        AreAd68EventCursorProvenance::SourceOwnedSamplerMaterialized,
    )
}

pub fn materialized_intervals_to_ad68_event_cursor_with_provenance(
    materialization: &AreSamplerMaterialization,
    provenance: AreAd68EventCursorProvenance,
) -> Result<AreAd68EventCursor, AreEventCursorError> {
    validate_materialization_provenance(materialization)?;
    let (x_min, width) = materialization_x_domain(materialization);
    let mut cursor = AreAd68EventCursor::source_owned(
        materialization.y_min,
        materialization.y_max,
        x_min,
        width,
        0,
        1,
    );
    cursor.provenance = provenance;

    let mut intervals = materialization.intervals.clone();
    intervals.sort_by_key(|interval| (interval.row_y, interval.current_x, interval.run_index));

    let mut primed_row = None;
    for interval in &intervals {
        if primed_row != Some(interval.row_y) {
            prime_event_cursor_row(&mut cursor, interval.row_y)?;
            primed_row = Some(interval.row_y);
        }
        write_materialized_interval_to_cursor(&mut cursor, interval)?;
    }

    Ok(cursor)
}

pub fn source_owned_materializer_to_event_stream(
    materialization: &AreSamplerMaterialization,
) -> Result<AreEventStream, AreEventCursorError> {
    let cursor = materialized_intervals_to_ad68_event_cursor(materialization)?;
    event_cursor_to_event_stream(&cursor)
}

pub fn source_owned_materializer_to_ad68_rows(
    materialization: &AreSamplerMaterialization,
) -> Result<Ad68RowTable, AreEventCursorError> {
    let stream = source_owned_materializer_to_event_stream(materialization)?;
    emit_ad68_rows(&stream).map_err(AreEventCursorError::Ad68)
}

impl AreAd68EventCursor {
    pub fn source_owned(
        y_min: i32,
        y_max: i32,
        x_min: i32,
        width: usize,
        class0_state: u8,
        class1_state: u8,
    ) -> Self {
        let row_count = if y_max >= y_min {
            (y_max - y_min + 1) as usize
        } else {
            0
        };
        let backing_len = row_count.saturating_mul(width);
        Self {
            y_min,
            y_max,
            x_min,
            event_objects: AreEventObjectTable {
                objects: vec![
                    AreEventObject::state(AreEventObjectKind::Class0, class0_state),
                    AreEventObject::state(AreEventObjectKind::Class1, class1_state),
                    AreEventObject::class2(0, width, 1),
                ],
            },
            class_map: AreEventClassMap {
                class0_event_index: 0,
                class1_event_index: 1,
                class2_event_index: 2,
            },
            payload_backing: AreEventPayloadBacking {
                id: ArePayloadBackingId(0),
                bytes: vec![0; backing_len],
            },
            rows: Vec::new(),
            active_row_y: None,
            current_event_index: 0,
            provenance: AreAd68EventCursorProvenance::SourceOwnedSamplerMaterialized,
        }
    }
}

fn validate_materialization_provenance(
    materialization: &AreSamplerMaterialization,
) -> Result<(), AreEventCursorError> {
    match materialization.provenance {
        AreSamplerMaterializationProvenance::SourceOwnedSamplerMaterialized => Ok(()),
        AreSamplerMaterializationProvenance::TypedSpanFallback => {
            Err(AreEventCursorError::TypedSpanFallbackRejected)
        }
        AreSamplerMaterializationProvenance::CoverageRowFallback => {
            Err(AreEventCursorError::CoverageRowFallbackRejected)
        }
        AreSamplerMaterializationProvenance::FixtureBacked => {
            Err(AreEventCursorError::FixturePayloadFallbackRejected)
        }
    }
}

fn materialization_x_domain(materialization: &AreSamplerMaterialization) -> (i32, usize) {
    let x_min = materialization
        .intervals
        .iter()
        .map(|interval| interval.current_x)
        .min()
        .unwrap_or(0);
    let x_max = materialization
        .intervals
        .iter()
        .map(|interval| interval.next_x)
        .max()
        .unwrap_or(x_min);
    (x_min, usize::try_from((x_max - x_min).max(0)).unwrap_or(0))
}

fn write_materialized_interval_to_cursor(
    cursor: &mut AreAd68EventCursor,
    interval: &AreMaterializedInterval,
) -> Result<(), AreEventCursorError> {
    let width = usize::try_from(interval.width().max(0)).unwrap_or(0);
    if let Some(payload) = &interval.payload {
        if payload.payload_len != width || payload.bytes.len() != width {
            return Err(AreEventCursorError::InvalidMaterializedPayloadLength {
                row_y: interval.row_y,
                run_index: interval.run_index,
                payload_len: payload.payload_len,
                width,
            });
        }
        for (sample_x, byte) in (interval.current_x..interval.next_x).zip(payload.bytes.iter()) {
            write_class2_payload_byte(cursor, sample_x, u16::from(*byte))?;
        }
        return Ok(());
    }

    let coverage = match interval
        .state_hint
        .map(|hint| hint.state_class)
        .unwrap_or(Are95ccStateClass::Class0)
    {
        Are95ccStateClass::Class0 => 0,
        Are95ccStateClass::Class1 => 0x100,
    };
    if let Some(state_hint) = interval.state_hint {
        set_state_for_class(cursor, state_hint.state_class, state_hint.state)?;
    }
    for sample_x in interval.current_x..interval.next_x {
        write_class2_payload_byte(cursor, sample_x, coverage)?;
    }
    Ok(())
}

fn set_state_for_class(
    cursor: &mut AreAd68EventCursor,
    state_class: Are95ccStateClass,
    state: u8,
) -> Result<(), AreEventCursorError> {
    let event_index = match state_class {
        Are95ccStateClass::Class0 => cursor.class_map.class0_event_index,
        Are95ccStateClass::Class1 => cursor.class_map.class1_event_index,
    };
    let objects_len = cursor.event_objects.objects.len();
    let object = cursor.event_objects.objects.get_mut(event_index).ok_or(
        AreEventCursorError::InvalidEventIndex {
            event_index,
            objects_len,
        },
    )?;
    object.state = state;
    Ok(())
}

impl AreEventObject {
    pub fn state(kind: AreEventObjectKind, state: u8) -> Self {
        Self {
            kind,
            payload_base_offset: 0,
            row_local_payload_base: 0,
            current_payload_offset: 0,
            payload_stride: 0,
            row_stride: 0,
            state,
        }
    }

    pub fn class2(payload_base_offset: usize, row_stride: usize, payload_stride: usize) -> Self {
        Self {
            kind: AreEventObjectKind::Class2,
            payload_base_offset,
            row_local_payload_base: payload_base_offset,
            current_payload_offset: payload_base_offset,
            payload_stride,
            row_stride,
            state: 0,
        }
    }
}

impl AreEventClassMap {
    pub fn event_index_for(self, event_class: AreEventObjectKind) -> usize {
        match event_class {
            AreEventObjectKind::Class0 => self.class0_event_index,
            AreEventObjectKind::Class1 => self.class1_event_index,
            AreEventObjectKind::Class2 => self.class2_event_index,
        }
    }
}

fn validate_source_owned(cursor: &AreAd68EventCursor) -> Result<(), AreEventCursorError> {
    match cursor.provenance {
        AreAd68EventCursorProvenance::SourceOwnedSamplerMaterialized => Ok(()),
        AreAd68EventCursorProvenance::TypedSpanFallback => {
            Err(AreEventCursorError::TypedSpanFallbackRejected)
        }
        AreAd68EventCursorProvenance::CoverageRowFallback => {
            Err(AreEventCursorError::CoverageRowFallbackRejected)
        }
        AreAd68EventCursorProvenance::FixturePayloadFallback => {
            Err(AreEventCursorError::FixturePayloadFallbackRejected)
        }
        AreAd68EventCursorProvenance::SyntheticPayloadFallback => {
            Err(AreEventCursorError::SyntheticPayloadFallbackRejected)
        }
        AreAd68EventCursorProvenance::IntervalObject1b898 => {
            Err(AreEventCursorError::IntervalObject1b898PayloadBridgeRejected)
        }
    }
}

fn validate_row_bounds(cursor: &AreAd68EventCursor) -> Result<(), AreEventCursorError> {
    if cursor.y_max < cursor.y_min {
        return Err(AreEventCursorError::InvalidRowBounds {
            y_min: cursor.y_min,
            y_max: cursor.y_max,
        });
    }
    Ok(())
}

fn advance_payload_pointers(
    cursor: &mut AreAd68EventCursor,
    sample_x: i32,
) -> Result<(), AreEventCursorError> {
    if sample_x < cursor.x_min {
        return Err(AreEventCursorError::SampleXBeforeMin {
            sample_x,
            x_min: cursor.x_min,
        });
    }
    let sample_offset = usize::try_from(sample_x - cursor.x_min).map_err(|_| {
        AreEventCursorError::SampleXBeforeMin {
            sample_x,
            x_min: cursor.x_min,
        }
    })?;
    let backing_len = cursor.payload_backing.bytes.len();
    for object in &mut cursor.event_objects.objects {
        if object.payload_stride == 0 {
            object.current_payload_offset = object.row_local_payload_base;
            continue;
        }
        let offset = sample_offset
            .checked_mul(object.payload_stride)
            .and_then(|offset| object.row_local_payload_base.checked_add(offset))
            .ok_or(AreEventCursorError::PayloadOffsetOutOfBounds {
                offset: usize::MAX,
                backing_len,
            })?;
        if object.kind == AreEventObjectKind::Class2 && offset > backing_len {
            return Err(AreEventCursorError::PayloadOffsetOutOfBounds {
                offset,
                backing_len,
            });
        }
        object.current_payload_offset = offset;
    }
    Ok(())
}

fn ensure_active_row(cursor: &mut AreAd68EventCursor, row_y: i32) -> &mut AreAd68EventCursorRow {
    if let Some(index) = cursor.rows.iter().position(|row| row.row_y == row_y) {
        return &mut cursor.rows[index];
    }
    cursor.rows.push(AreAd68EventCursorRow {
        row_y,
        samples: Vec::new(),
    });
    cursor.rows.last_mut().expect("row was just pushed")
}

fn event_payload_window(object: &AreEventObject, len: usize) -> Option<AreEventPayloadWindow> {
    (object.kind == AreEventObjectKind::Class2).then_some(AreEventPayloadWindow {
        backing_id: ArePayloadBackingId(0),
        offset: object.current_payload_offset,
        len,
    })
}

fn coalesced_spans(
    samples: &[AreSampleAdvanceResult],
) -> Result<Vec<AreSampleAdvanceResult>, AreEventCursorError> {
    let mut spans: Vec<AreSampleAdvanceResult> = Vec::new();
    for sample in samples {
        if sample.next_x <= sample.current_x {
            return Err(AreEventCursorError::InvalidSampleWidth {
                current_x: sample.current_x,
                next_x: sample.next_x,
            });
        }
        if let Some(last) = spans.last_mut() {
            let contiguous = last.next_x == sample.current_x;
            let same_event_class = last.event_class == sample.event_class;
            let same_event_index = last.event_index == sample.event_index;
            let same_state = last.state == sample.state;
            let same_materialize_flag = last.materialize_flag == sample.materialize_flag;
            let payload_contiguous = match (last.payload_window.as_mut(), sample.payload_window) {
                (Some(last_window), Some(sample_window))
                    if last_window.backing_id == sample_window.backing_id
                        && last_window.offset + last_window.len == sample_window.offset =>
                {
                    last_window.len += sample_window.len;
                    true
                }
                (None, None) => true,
                _ => false,
            };
            if contiguous
                && same_event_class
                && same_event_index
                && same_state
                && same_materialize_flag
                && payload_contiguous
            {
                last.next_x = sample.next_x;
                continue;
            }
        }
        spans.push(sample.clone());
    }
    Ok(spans)
}

fn event_class_for(kind: AreEventObjectKind) -> EventClass {
    match kind {
        AreEventObjectKind::Class0 => EventClass::Class0,
        AreEventObjectKind::Class1 => EventClass::Class1,
        AreEventObjectKind::Class2 => EventClass::Class2,
    }
}

fn payload_window_for_stream(window: AreEventPayloadWindow) -> ArePayloadWindow {
    ArePayloadWindow {
        backing_id: window.backing_id,
        offset: window.offset,
        len: window.len,
    }
}

#[cfg(test)]
mod ad68_event_cursor_tests {
    use super::*;
    use crate::{
        build_e854_working_set, build_materializer_input_from_source, emit_ad68_rows,
        materialize_95cc_intervals, source_owned_materializer_to_ad68_rows,
        AreBezierSourcePathInput, AreMaterializedInterval, AreMaterializedPayload,
        AreMaterializedStateHint, ArePathPoint, ArePathVerb, AreSamplerIntervalList,
        AreSamplerIntervalRow, AreSamplerIntervalRun, AreSamplerIntervalTag,
        AreSamplerListWorkingState, AreSourcePathProvenance, AreZeroWidthIntervalPolicy,
    };

    fn cursor() -> AreAd68EventCursor {
        AreAd68EventCursor::source_owned(10, 10, 2, 8, 0, 1)
    }

    fn materialization(intervals: Vec<AreMaterializedInterval>) -> AreSamplerMaterialization {
        AreSamplerMaterialization {
            provenance: AreSamplerMaterializationProvenance::SourceOwnedSamplerMaterialized,
            y_min: intervals
                .iter()
                .map(|interval| interval.row_y)
                .min()
                .unwrap_or(0),
            y_max: intervals
                .iter()
                .map(|interval| interval.row_y)
                .max()
                .unwrap_or(0),
            intervals,
        }
    }

    fn payload_interval(row_y: i32, current_x: i32, bytes: &[u8]) -> AreMaterializedInterval {
        AreMaterializedInterval {
            row_y,
            run_index: 0,
            current_x,
            next_x: current_x + bytes.len() as i32,
            source_record_indices: vec![0],
            payload: Some(AreMaterializedPayload {
                bytes: bytes.to_vec(),
                payload_len: bytes.len(),
            }),
            state_hint: None,
        }
    }

    fn state_interval(
        row_y: i32,
        current_x: i32,
        next_x: i32,
        state_class: Are95ccStateClass,
        state: u8,
    ) -> AreMaterializedInterval {
        AreMaterializedInterval {
            row_y,
            run_index: 0,
            current_x,
            next_x,
            source_record_indices: vec![0],
            payload: None,
            state_hint: Some(AreMaterializedStateHint { state_class, state }),
        }
    }

    fn source(points: Vec<ArePathPoint>, verbs: Vec<ArePathVerb>) -> AreBezierSourcePathInput {
        AreBezierSourcePathInput::source_owned(points, verbs)
    }

    fn horizontal_source() -> AreBezierSourcePathInput {
        source(
            vec![ArePathPoint::new(2.0, 4.0), ArePathPoint::new(6.0, 4.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        )
    }

    fn vertical_pair_source() -> AreBezierSourcePathInput {
        source(
            vec![
                ArePathPoint::new(2.25, 4.0),
                ArePathPoint::new(2.25, 5.0),
                ArePathPoint::new(2.75, 5.0),
                ArePathPoint::new(2.75, 4.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        )
    }

    fn input_with_manual_run(
        source_input: AreBezierSourcePathInput,
        row_y: i32,
        current_x: i32,
        next_x: i32,
        source_record_indices: Vec<usize>,
    ) -> crate::AreSamplerMaterializerInput {
        let working_set = build_e854_working_set(&source_input).unwrap();
        let interval_list = AreSamplerIntervalList {
            provenance: AreSourcePathProvenance::SourceOwned,
            y_min: row_y,
            y_max: row_y + 1,
            rows: vec![AreSamplerIntervalRow {
                row_y,
                boundaries: Vec::new(),
                runs: vec![AreSamplerIntervalRun {
                    current_x,
                    next_x,
                    tag: AreSamplerIntervalTag::SourceSpan,
                    source_record_indices,
                    materialize_candidate: true,
                }],
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
        crate::AreSamplerMaterializerInput::source_owned(source_input, working_set, interval_list)
    }

    #[test]
    fn row_primer_sets_event_object_payload_base_and_current() {
        let mut cursor = AreAd68EventCursor::source_owned(10, 12, 2, 8, 0, 1);

        let result = prime_event_cursor_row(&mut cursor, 11).unwrap();

        let class2 = &cursor.event_objects.objects[2];
        assert_eq!(result.row_y, 11);
        assert_eq!(class2.row_local_payload_base, 8);
        assert_eq!(class2.current_payload_offset, 8);
        assert_eq!(result.object_payload_offsets[2].row_local_payload_base, 8);
    }

    #[test]
    fn sample_advance_moves_event_object_payload_pointer() {
        let mut cursor = cursor();
        prime_event_cursor_row(&mut cursor, 10).unwrap();

        advance_event_cursor_sample_x(&mut cursor, 5).unwrap();

        let class2 = &cursor.event_objects.objects[2];
        assert_eq!(class2.current_payload_offset, 3);
    }

    #[test]
    fn coverage_zero_maps_class0() {
        assert_eq!(
            classify_sample_coverage(0).unwrap(),
            AreEventObjectKind::Class0
        );
    }

    #[test]
    fn coverage_full_maps_class1() {
        assert_eq!(
            classify_sample_coverage(0x100).unwrap(),
            AreEventObjectKind::Class1
        );
    }

    #[test]
    fn coverage_partial_maps_class2_and_writes_payload() {
        let mut cursor = cursor();
        prime_event_cursor_row(&mut cursor, 10).unwrap();

        let result = write_class2_payload_byte(&mut cursor, 4, 0x44).unwrap();

        assert_eq!(result.event_class, AreEventObjectKind::Class2);
        assert_eq!(result.payload_window.unwrap().offset, 2);
        assert_eq!(cursor.payload_backing.bytes[2], 0x44);
    }

    #[test]
    fn class2_event_window_points_to_payload_backing() {
        let mut cursor = cursor();
        prime_event_cursor_row(&mut cursor, 10).unwrap();

        let result = write_class2_payload_byte(&mut cursor, 7, 0x55).unwrap();
        let window = result.payload_window.unwrap();

        assert_eq!(window.backing_id, cursor.payload_backing.id);
        assert_eq!(window.offset, 5);
        assert_eq!(window.len, 1);
        assert_eq!(cursor.payload_backing.bytes[window.offset], 0x55);
    }

    #[test]
    fn event_cursor_to_event_stream_class2_span() {
        let mut cursor = cursor();
        prime_event_cursor_row(&mut cursor, 10).unwrap();
        write_class2_payload_byte(&mut cursor, 2, 0x11).unwrap();
        write_class2_payload_byte(&mut cursor, 3, 0x22).unwrap();
        write_class2_payload_byte(&mut cursor, 4, 0x33).unwrap();

        let stream = event_cursor_to_event_stream(&cursor).unwrap();

        assert_eq!(stream.rows[0].cursors.len(), 1);
        assert_eq!(stream.rows[0].cursors[0].current_x, 2);
        assert_eq!(stream.rows[0].cursors[0].next_x, 5);
        assert_eq!(stream.objects[0].event_class, EventClass::Class2);
        assert_eq!(stream.objects[0].payload_window.unwrap().len, 3);
        assert_eq!(&stream.payload_backings[0].bytes[0..3], &[0x11, 0x22, 0x33]);
    }

    #[test]
    fn class0_class1_emit_state_without_payload() {
        let mut cursor = cursor();
        prime_event_cursor_row(&mut cursor, 10).unwrap();
        write_class2_payload_byte(&mut cursor, 2, 0).unwrap();
        write_class2_payload_byte(&mut cursor, 3, 0x100).unwrap();

        let stream = event_cursor_to_event_stream(&cursor).unwrap();

        assert_eq!(stream.objects[0].event_class, EventClass::Class0);
        assert_eq!(stream.objects[0].state, 0);
        assert!(stream.objects[0].payload_window.is_none());
        assert_eq!(stream.objects[1].event_class, EventClass::Class1);
        assert_eq!(stream.objects[1].state, 1);
        assert!(stream.objects[1].payload_window.is_none());
    }

    #[test]
    fn ad68_rows_copy_event_object_payload_window() {
        let mut cursor = cursor();
        prime_event_cursor_row(&mut cursor, 10).unwrap();
        write_class2_payload_byte(&mut cursor, 4, 0x31).unwrap();
        write_class2_payload_byte(&mut cursor, 5, 0x32).unwrap();

        let stream = event_cursor_to_event_stream(&cursor).unwrap();
        let table = emit_ad68_rows(&stream).unwrap();
        let row = table.row(10).unwrap();

        assert_eq!(row.len(), 1);
        assert_eq!(row[0].x, 4);
        assert_eq!(row[0].len, 2);
        assert_eq!(row[0].bytes(), Some(&[0x31, 0x32][..]));
    }

    #[test]
    fn rejects_typed_span_fallback() {
        let mut cursor = cursor();
        cursor.provenance = AreAd68EventCursorProvenance::TypedSpanFallback;

        assert_eq!(
            prime_event_cursor_row(&mut cursor, 10),
            Err(AreEventCursorError::TypedSpanFallbackRejected)
        );
    }

    #[test]
    fn rejects_fixture_payload_fallback() {
        let mut cursor = cursor();
        cursor.provenance = AreAd68EventCursorProvenance::FixturePayloadFallback;

        assert_eq!(
            event_cursor_to_event_stream(&cursor),
            Err(AreEventCursorError::FixturePayloadFallbackRejected)
        );
    }

    #[test]
    fn rejects_synthetic_payload_fallback() {
        let mut cursor = cursor();
        cursor.provenance = AreAd68EventCursorProvenance::SyntheticPayloadFallback;

        assert_eq!(
            write_class2_payload_byte(&mut cursor, 2, 0x7f),
            Err(AreEventCursorError::SyntheticPayloadFallbackRejected)
        );
    }

    #[test]
    fn rejects_coverage_row_fallback() {
        let mut cursor = cursor();
        cursor.provenance = AreAd68EventCursorProvenance::CoverageRowFallback;

        assert_eq!(
            advance_event_cursor_sample_x(&mut cursor, 2),
            Err(AreEventCursorError::CoverageRowFallbackRejected)
        );
    }

    #[test]
    fn no_1b898_interval_object_payload_bridge() {
        let mut cursor = cursor();
        cursor.provenance = AreAd68EventCursorProvenance::IntervalObject1b898;

        assert_eq!(
            event_cursor_to_event_stream(&cursor),
            Err(AreEventCursorError::IntervalObject1b898PayloadBridgeRejected)
        );
    }

    #[test]
    fn materialized_partial_coverage_to_class2_ad68_row() {
        let materialization = materialization(vec![payload_interval(10, 2, &[0x44])]);

        let table = source_owned_materializer_to_ad68_rows(&materialization).unwrap();
        let row = table.row(10).unwrap();

        assert_eq!(row.len(), 1);
        assert_eq!(row[0].x, 2);
        assert_eq!(row[0].len, 1);
        assert_eq!(row[0].bytes(), Some(&[0x44][..]));
    }

    #[test]
    fn materialized_zero_coverage_to_class0_state_no_payload() {
        let materialization = materialization(vec![state_interval(
            10,
            2,
            3,
            Are95ccStateClass::Class0,
            0x10,
        )]);

        let table = source_owned_materializer_to_ad68_rows(&materialization).unwrap();
        let row = table.row(10).unwrap();

        assert_eq!(row.len(), 1);
        assert_eq!(row[0].len, 0);
        assert_eq!(row[0].state, 0x10);
        assert!(row[0].bytes().is_none());
    }

    #[test]
    fn materialized_full_coverage_to_class1_state_no_payload() {
        let materialization = materialization(vec![state_interval(
            10,
            2,
            3,
            Are95ccStateClass::Class1,
            0x20,
        )]);

        let table = source_owned_materializer_to_ad68_rows(&materialization).unwrap();
        let row = table.row(10).unwrap();

        assert_eq!(row.len(), 1);
        assert_eq!(row[0].len, 0);
        assert_eq!(row[0].state, 0x20);
        assert!(row[0].bytes().is_none());
    }

    #[test]
    fn materialized_interval_payload_window_matches_event_object_plus_0x20() {
        let materialization = materialization(vec![payload_interval(10, 4, &[0x51])]);

        let cursor = materialized_intervals_to_ad68_event_cursor(&materialization).unwrap();
        let sample = &cursor.rows[0].samples[0];
        let class2 = &cursor.event_objects.objects[2];

        assert_eq!(sample.payload_window.unwrap().offset, 0);
        assert_eq!(class2.current_payload_offset, 0);
        assert_eq!(cursor.payload_backing.bytes[0], 0x51);
    }

    #[test]
    fn materialized_multi_sample_interval_to_ad68_span() {
        let materialization = materialization(vec![payload_interval(10, 2, &[0x11, 0x22, 0x33])]);

        let table = source_owned_materializer_to_ad68_rows(&materialization).unwrap();
        let row = table.row(10).unwrap();

        assert_eq!(row.len(), 1);
        assert_eq!(row[0].x, 2);
        assert_eq!(row[0].len, 3);
        assert_eq!(row[0].bytes(), Some(&[0x11, 0x22, 0x33][..]));
    }

    #[test]
    fn materialized_two_rows_to_ad68_row_table() {
        let materialization = materialization(vec![
            payload_interval(10, 2, &[0x11, 0x12]),
            payload_interval(11, 3, &[0x21]),
        ]);

        let table = source_owned_materializer_to_ad68_rows(&materialization).unwrap();

        assert_eq!(table.row(10).unwrap()[0].bytes(), Some(&[0x11, 0x12][..]));
        assert_eq!(table.row(11).unwrap()[0].bytes(), Some(&[0x21][..]));
    }

    #[test]
    fn line_geometry_pipeline_to_ad68_without_descriptor_fixture() {
        let input = build_materializer_input_from_source(&horizontal_source()).unwrap();
        let materialized = materialize_95cc_intervals(&input).unwrap();

        let table = source_owned_materializer_to_ad68_rows(&materialized).unwrap();
        let row = table.row(4).unwrap();

        assert!(row.iter().any(|node| node.bytes().is_some()));
    }

    #[test]
    fn edge_pair_pipeline_to_ad68_without_descriptor_fixture() {
        let input = input_with_manual_run(vertical_pair_source(), 4, 2, 3, vec![0, 1]);
        let materialized = materialize_95cc_intervals(&input).unwrap();

        let table = source_owned_materializer_to_ad68_rows(&materialized).unwrap();
        let row = table.row(4).unwrap();

        assert_eq!(row.len(), 1);
        assert_eq!(row[0].x, 2);
        assert_eq!(row[0].len, 1);
        assert!(row[0].bytes().is_some());
    }

    #[test]
    fn materializer_bridge_rejects_fixture_payload_fallback() {
        let mut materialization = materialization(vec![payload_interval(10, 2, &[0x44])]);
        materialization.provenance = AreSamplerMaterializationProvenance::FixtureBacked;

        assert_eq!(
            materialized_intervals_to_ad68_event_cursor(&materialization),
            Err(AreEventCursorError::FixturePayloadFallbackRejected)
        );
    }

    #[test]
    fn materializer_bridge_rejects_synthetic_payload_fallback() {
        let materialization = materialization(vec![payload_interval(10, 2, &[0x44])]);

        assert_eq!(
            materialized_intervals_to_ad68_event_cursor_with_provenance(
                &materialization,
                AreAd68EventCursorProvenance::SyntheticPayloadFallback,
            ),
            Err(AreEventCursorError::SyntheticPayloadFallbackRejected)
        );
    }

    #[test]
    fn materializer_bridge_rejects_typed_span_fallback() {
        let mut materialization = materialization(vec![payload_interval(10, 2, &[0x44])]);
        materialization.provenance = AreSamplerMaterializationProvenance::TypedSpanFallback;

        assert_eq!(
            source_owned_materializer_to_event_stream(&materialization),
            Err(AreEventCursorError::TypedSpanFallbackRejected)
        );
    }

    #[test]
    fn materializer_bridge_rejects_1b898_interval_object_payload_bridge() {
        let materialization = materialization(vec![payload_interval(10, 2, &[0x44])]);

        assert_eq!(
            materialized_intervals_to_ad68_event_cursor_with_provenance(
                &materialization,
                AreAd68EventCursorProvenance::IntervalObject1b898,
            ),
            Err(AreEventCursorError::IntervalObject1b898PayloadBridgeRejected)
        );
    }
}
