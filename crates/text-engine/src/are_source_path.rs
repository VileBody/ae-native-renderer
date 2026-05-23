use serde::{Deserialize, Serialize};
use ttf_parser::{Face, OutlineBuilder};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreBezierSourcePathInput {
    pub provenance: AreSourcePathProvenance,
    pub points: Vec<ArePathPoint>,
    pub verbs: Vec<ArePathVerb>,
    pub segments: Vec<AreSourceSegment>,
    pub transform: ArePathTransform,
    pub mode: AreSourcePathMode,
}

impl AreBezierSourcePathInput {
    pub fn source_owned(points: Vec<ArePathPoint>, verbs: Vec<ArePathVerb>) -> Self {
        let segments = derive_segments_from_points_verbs(&points, &verbs);
        Self {
            provenance: AreSourcePathProvenance::SourceOwned,
            points,
            verbs,
            segments,
            transform: ArePathTransform::identity(),
            mode: AreSourcePathMode::Fill,
        }
    }

    pub fn source_owned_glyph_path(
        points: Vec<ArePathPoint>,
        verbs: Vec<ArePathVerb>,
        transform: ArePathTransform,
    ) -> Self {
        let segments = derive_segments_from_points_verbs(&points, &verbs);
        Self {
            provenance: AreSourcePathProvenance::SourceOwnedGlyphPath,
            points,
            verbs,
            segments,
            transform,
            mode: AreSourcePathMode::Fill,
        }
    }

    pub fn source_owned_glyph_run(points: Vec<ArePathPoint>, verbs: Vec<ArePathVerb>) -> Self {
        let segments = derive_segments_from_points_verbs(&points, &verbs);
        Self {
            provenance: AreSourcePathProvenance::SourceOwnedGlyphRun,
            points,
            verbs,
            segments,
            transform: ArePathTransform::identity(),
            mode: AreSourcePathMode::Fill,
        }
    }

    pub fn with_transform(mut self, transform: ArePathTransform) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_mode(mut self, mode: AreSourcePathMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_segments(mut self, segments: Vec<AreSourceSegment>) -> Self {
        self.segments = segments;
        self
    }

    pub fn typed_span_fallback(points: Vec<ArePathPoint>, verbs: Vec<ArePathVerb>) -> Self {
        let segments = derive_segments_from_points_verbs(&points, &verbs);
        Self {
            provenance: AreSourcePathProvenance::TypedSpanFallback,
            points,
            verbs,
            segments,
            transform: ArePathTransform::identity(),
            mode: AreSourcePathMode::Fill,
        }
    }

    pub fn coverage_row_fallback(points: Vec<ArePathPoint>, verbs: Vec<ArePathVerb>) -> Self {
        let segments = derive_segments_from_points_verbs(&points, &verbs);
        Self {
            provenance: AreSourcePathProvenance::CoverageRowFallback,
            points,
            verbs,
            segments,
            transform: ArePathTransform::identity(),
            mode: AreSourcePathMode::Fill,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreSourcePathProvenance {
    SourceOwned,
    SourceOwnedGlyphPath,
    SourceOwnedGlyphRun,
    FocusedFixture,
    TypedSpanFallback,
    CoverageRowFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArePathPoint {
    pub x: f32,
    pub y: f32,
}

impl ArePathPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArePathVerb {
    MoveTo,
    LineTo,
    QuadTo,
    CubicTo,
    Close,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AreSourceContourId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AreSourceSegmentId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AreQuadraticControl {
    pub control: ArePathPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AreCubicControl {
    pub control_1: ArePathPoint,
    pub control_2: ArePathPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AreContourClose {
    pub from: ArePathPoint,
    pub to: ArePathPoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreSourceSegmentKind {
    Move,
    Line,
    Quadratic {
        control: Option<AreQuadraticControl>,
    },
    Cubic {
        controls: Option<AreCubicControl>,
    },
    Close(AreContourClose),
}

impl AreSourceSegmentKind {
    pub fn has_curve_controls(&self) -> bool {
        matches!(
            self,
            AreSourceSegmentKind::Quadratic { control: Some(_) }
                | AreSourceSegmentKind::Cubic { controls: Some(_) }
        )
    }

    pub fn is_curve(&self) -> bool {
        matches!(
            self,
            AreSourceSegmentKind::Quadratic { .. } | AreSourceSegmentKind::Cubic { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreSourceSegment {
    pub contour_id: AreSourceContourId,
    pub segment_id: AreSourceSegmentId,
    pub source_point_index: usize,
    pub verb: ArePathVerb,
    pub previous_point: Option<ArePathPoint>,
    pub endpoint: ArePathPoint,
    pub kind: AreSourceSegmentKind,
    pub glyph_run_index: Option<usize>,
    pub glyph_id: Option<u16>,
    pub is_close_boundary: bool,
}

impl AreSourceSegment {
    pub fn transformed(&self, transform: ArePathTransform) -> Self {
        let previous_point = self.previous_point.map(|point| transform.apply(point));
        let endpoint = transform.apply(self.endpoint);
        let kind = match self.kind {
            AreSourceSegmentKind::Move => AreSourceSegmentKind::Move,
            AreSourceSegmentKind::Line => AreSourceSegmentKind::Line,
            AreSourceSegmentKind::Quadratic { control } => AreSourceSegmentKind::Quadratic {
                control: control.map(|control| AreQuadraticControl {
                    control: transform.apply(control.control),
                }),
            },
            AreSourceSegmentKind::Cubic { controls } => AreSourceSegmentKind::Cubic {
                controls: controls.map(|controls| AreCubicControl {
                    control_1: transform.apply(controls.control_1),
                    control_2: transform.apply(controls.control_2),
                }),
            },
            AreSourceSegmentKind::Close(close) => AreSourceSegmentKind::Close(AreContourClose {
                from: transform.apply(close.from),
                to: transform.apply(close.to),
            }),
        };

        Self {
            contour_id: self.contour_id,
            segment_id: self.segment_id,
            source_point_index: self.source_point_index,
            verb: self.verb,
            previous_point,
            endpoint,
            kind,
            glyph_run_index: self.glyph_run_index,
            glyph_id: self.glyph_id,
            is_close_boundary: self.is_close_boundary,
        }
    }

    pub fn with_glyph_metadata(
        mut self,
        glyph_run_index: Option<usize>,
        glyph_id: Option<u16>,
    ) -> Self {
        self.glyph_run_index = glyph_run_index;
        self.glyph_id = glyph_id;
        self
    }

    pub fn is_curve_flattening_candidate(&self) -> bool {
        self.kind.is_curve() && self.kind.has_curve_controls()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreEdgePairCandidateKind {
    NeedsOppositeEdge,
    CloseBoundaryCandidate,
    StateTransitionCandidate,
    ContourPairCandidate,
    CurveFlatteningCandidate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreEdgePairCandidate {
    pub kind: AreEdgePairCandidateKind,
    pub record_index: usize,
    pub contour_id: AreSourceContourId,
    pub segment_id: AreSourceSegmentId,
    pub source_point_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArePathTransform {
    pub xx: f32,
    pub xy: f32,
    pub yx: f32,
    pub yy: f32,
    pub tx: f32,
    pub ty: f32,
}

impl ArePathTransform {
    pub fn identity() -> Self {
        Self {
            xx: 1.0,
            xy: 0.0,
            yx: 0.0,
            yy: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn apply(&self, point: ArePathPoint) -> ArePathPoint {
        ArePathPoint {
            x: point.x * self.xx + point.y * self.xy + self.tx,
            y: point.x * self.yx + point.y * self.yy + self.ty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreSourcePathMode {
    Fill,
    Stroke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSourceBounds {
    pub x_min: i32,
    pub y_min: i32,
    pub x_max: i32,
    pub y_max: i32,
}

impl AreSourceBounds {
    pub fn width(&self) -> i32 {
        self.x_max - self.x_min
    }

    pub fn height(&self) -> i32 {
        self.y_max - self.y_min
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreSourceSamplerWorkingSet {
    pub inline_base_scratch_0x17e: usize,
    pub state_flag_0x180: bool,
    pub current_point_cursor_0x182: usize,
    pub current_record_cursor_0x184: usize,
    pub saved_point_cursor_0x186: usize,
    pub saved_record_cursor_0x188: usize,
    pub counter_state_0x18a: i32,
    pub event_type_class_0x18b: u8,
    pub vector_pool_root_0x18c: usize,
    pub record_vector_base_0x18e: usize,
    pub record_vector_capacity_0x196: usize,
    pub mode_flag_0x601: bool,
    pub mode_subflag_0x602: bool,
    pub bounds: AreSourceBounds,
    pub records: Vec<AreSourceRecord32>,
    pub source_segments: Vec<AreSourceSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreSourceRecord32 {
    pub source_point_index_0x00: usize,
    pub min_x_0x08: i32,
    pub min_y_0x0c: i32,
    pub max_x_0x10: i32,
    pub max_y_0x14: i32,
    pub record_flag_0x18: u8,
    pub merge_state_0x19: u8,
}

impl AreSourceRecord32 {
    pub const NATIVE_SIZE_BYTES: usize = 0x20;
    pub const NORMAL_FLAG: u8 = 0x08;
    pub const SENTINEL_FLAG: u8 = 0xf8;
}

impl AreSourceSamplerWorkingSet {
    pub fn source_segment_for_record_index(
        &self,
        record_index: usize,
    ) -> Option<&AreSourceSegment> {
        let record = self.records.get(record_index)?;
        self.source_segment_for_record(record)
    }

    pub fn source_segment_for_record(
        &self,
        record: &AreSourceRecord32,
    ) -> Option<&AreSourceSegment> {
        self.source_segments
            .iter()
            .find(|segment| segment.source_point_index == record.source_point_index_0x00)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreSourceSamplerError {
    EmptySourcePath,
    PointVerbLenMismatch { points_len: usize, verbs_len: usize },
    TypedSpanFallbackRejected,
    CoverageRowFallbackRejected,
    InvalidPathVerb { index: usize, verb: ArePathVerb },
    FirstVerbMustMoveTo { verb: ArePathVerb },
    SegmentBeforeMoveTo { index: usize },
    NonFinitePoint { index: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreGlyphSourcePathFixture {
    pub font_label: String,
    pub character: char,
    pub glyph_id: u16,
    pub units_per_em: u16,
    pub font_size: f32,
    pub input: AreBezierSourcePathInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreGlyphRunSourcePathFixture {
    pub font_label: String,
    pub text: String,
    pub units_per_em: u16,
    pub font_size: f32,
    pub glyphs: Vec<AreGlyphRunGlyph>,
    pub total_advance: f32,
    pub input: Option<AreBezierSourcePathInput>,
}

impl AreGlyphRunSourcePathFixture {
    pub fn outline_glyph_count(&self) -> usize {
        self.glyphs.iter().filter(|glyph| glyph.has_outline).count()
    }

    pub fn advance_only_glyph_count(&self) -> usize {
        self.glyphs
            .iter()
            .filter(|glyph| !glyph.has_outline)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreGlyphRunGlyph {
    pub character: char,
    pub glyph_id: u16,
    pub x_offset: f32,
    pub advance: f32,
    pub has_outline: bool,
    pub outline_point_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreGlyphSourcePathError {
    FontParseFailed,
    MissingGlyph { character: char },
    MissingOutline { glyph_id: u16 },
    EmptyOutline,
    Source(AreSourceSamplerError),
}

pub fn build_glyph_source_path_fixture_from_font_bytes(
    font_label: impl Into<String>,
    font_bytes: &[u8],
    character: char,
    font_size: f32,
) -> Result<AreGlyphSourcePathFixture, AreGlyphSourcePathError> {
    let face = Face::parse(font_bytes, 0).map_err(|_| AreGlyphSourcePathError::FontParseFailed)?;
    let glyph_id = face
        .glyph_index(character)
        .ok_or(AreGlyphSourcePathError::MissingGlyph { character })?;
    let mut builder = GlyphSourcePathOutlineBuilder::default();
    face.outline_glyph(glyph_id, &mut builder)
        .ok_or(AreGlyphSourcePathError::MissingOutline {
            glyph_id: glyph_id.0,
        })?;
    builder.finish_open_contour();
    if builder.points.is_empty() || builder.verbs.is_empty() {
        return Err(AreGlyphSourcePathError::EmptyOutline);
    }

    let units_per_em = face.units_per_em();
    let scale = font_size / units_per_em as f32;
    let input = AreBezierSourcePathInput::source_owned_glyph_path(
        builder.points,
        builder.verbs,
        ArePathTransform {
            xx: scale,
            xy: 0.0,
            yx: 0.0,
            yy: scale,
            tx: 0.0,
            ty: 0.0,
        },
    )
    .with_segments(
        builder
            .segments
            .into_iter()
            .map(|segment| segment.with_glyph_metadata(Some(0), Some(glyph_id.0)))
            .collect(),
    );
    validate_source_input(&input).map_err(AreGlyphSourcePathError::Source)?;

    Ok(AreGlyphSourcePathFixture {
        font_label: font_label.into(),
        character,
        glyph_id: glyph_id.0,
        units_per_em,
        font_size,
        input,
    })
}

pub fn build_glyph_run_source_path_fixture_from_font_bytes(
    font_label: impl Into<String>,
    font_bytes: &[u8],
    text: &str,
    font_size: f32,
) -> Result<AreGlyphRunSourcePathFixture, AreGlyphSourcePathError> {
    let face = Face::parse(font_bytes, 0).map_err(|_| AreGlyphSourcePathError::FontParseFailed)?;
    let units_per_em = face.units_per_em();
    let scale = font_size / units_per_em as f32;
    let mut pen_x = 0.0f32;
    let mut glyphs = Vec::new();
    let mut points = Vec::new();
    let mut verbs = Vec::new();

    let mut segments = Vec::new();

    for (glyph_run_index, character) in text.chars().enumerate() {
        let glyph_id = face
            .glyph_index(character)
            .ok_or(AreGlyphSourcePathError::MissingGlyph { character })?;
        let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f32;
        let mut builder = GlyphSourcePathOutlineBuilder::default();
        let has_outline = face.outline_glyph(glyph_id, &mut builder).is_some();
        if has_outline {
            builder.finish_open_contour();
        }
        let outline_point_count = builder.points.len();
        for (point, verb) in builder.points.into_iter().zip(builder.verbs.into_iter()) {
            points.push(ArePathPoint::new(
                point.x * scale + pen_x * scale,
                point.y * scale,
            ));
            verbs.push(verb);
        }
        let glyph_transform = ArePathTransform {
            xx: scale,
            xy: 0.0,
            yx: 0.0,
            yy: scale,
            tx: pen_x * scale,
            ty: 0.0,
        };
        segments.extend(builder.segments.into_iter().map(|segment| {
            let mut transformed = segment.transformed(glyph_transform);
            transformed.source_point_index += points.len() - outline_point_count;
            transformed.with_glyph_metadata(Some(glyph_run_index), Some(glyph_id.0))
        }));
        glyphs.push(AreGlyphRunGlyph {
            character,
            glyph_id: glyph_id.0,
            x_offset: pen_x * scale,
            advance: advance * scale,
            has_outline: outline_point_count > 0,
            outline_point_count,
        });
        pen_x += advance;
    }

    let input = if points.is_empty() || verbs.is_empty() {
        None
    } else {
        let input =
            AreBezierSourcePathInput::source_owned_glyph_run(points, verbs).with_segments(segments);
        validate_source_input(&input).map_err(AreGlyphSourcePathError::Source)?;
        Some(input)
    };

    Ok(AreGlyphRunSourcePathFixture {
        font_label: font_label.into(),
        text: text.to_string(),
        units_per_em,
        font_size,
        glyphs,
        total_advance: pen_x * scale,
        input,
    })
}

#[derive(Default)]
struct GlyphSourcePathOutlineBuilder {
    points: Vec<ArePathPoint>,
    verbs: Vec<ArePathVerb>,
    segments: Vec<AreSourceSegment>,
    current: Option<ArePathPoint>,
    contour_start: Option<ArePathPoint>,
    contour_id: usize,
    next_segment_id: usize,
}

impl GlyphSourcePathOutlineBuilder {
    fn push(&mut self, verb: ArePathVerb, point: ArePathPoint) -> usize {
        self.verbs.push(verb);
        self.points.push(point);
        self.current = Some(point);
        self.points.len() - 1
    }

    fn push_segment(
        &mut self,
        source_point_index: usize,
        verb: ArePathVerb,
        previous_point: Option<ArePathPoint>,
        endpoint: ArePathPoint,
        kind: AreSourceSegmentKind,
    ) {
        let is_close_boundary = matches!(kind, AreSourceSegmentKind::Close(_));
        self.segments.push(AreSourceSegment {
            contour_id: AreSourceContourId(self.contour_id),
            segment_id: AreSourceSegmentId(self.next_segment_id),
            source_point_index,
            verb,
            previous_point,
            endpoint,
            kind,
            glyph_run_index: None,
            glyph_id: None,
            is_close_boundary,
        });
        self.next_segment_id += 1;
    }

    fn finish_open_contour(&mut self) {
        if self
            .verbs
            .last()
            .is_some_and(|verb| *verb != ArePathVerb::Close)
        {
            if let (Some(point), Some(contour_start)) = (self.current, self.contour_start) {
                let source_point_index = self.push(ArePathVerb::Close, point);
                self.push_segment(
                    source_point_index,
                    ArePathVerb::Close,
                    Some(point),
                    contour_start,
                    AreSourceSegmentKind::Close(AreContourClose {
                        from: point,
                        to: contour_start,
                    }),
                );
            }
        }
    }
}

impl OutlineBuilder for GlyphSourcePathOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish_open_contour();
        if !self.points.is_empty() {
            self.contour_id += 1;
        }
        let point = ArePathPoint::new(x, y);
        let source_point_index = self.push(ArePathVerb::MoveTo, point);
        self.contour_start = Some(point);
        self.push_segment(
            source_point_index,
            ArePathVerb::MoveTo,
            None,
            point,
            AreSourceSegmentKind::Move,
        );
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let point = ArePathPoint::new(x, y);
        let previous = self.current;
        let source_point_index = self.push(ArePathVerb::LineTo, point);
        self.push_segment(
            source_point_index,
            ArePathVerb::LineTo,
            previous,
            point,
            AreSourceSegmentKind::Line,
        );
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let point = ArePathPoint::new(x, y);
        let previous = self.current;
        let source_point_index = self.push(ArePathVerb::QuadTo, point);
        self.push_segment(
            source_point_index,
            ArePathVerb::QuadTo,
            previous,
            point,
            AreSourceSegmentKind::Quadratic {
                control: Some(AreQuadraticControl {
                    control: ArePathPoint::new(x1, y1),
                }),
            },
        );
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let point = ArePathPoint::new(x, y);
        let previous = self.current;
        let source_point_index = self.push(ArePathVerb::CubicTo, point);
        self.push_segment(
            source_point_index,
            ArePathVerb::CubicTo,
            previous,
            point,
            AreSourceSegmentKind::Cubic {
                controls: Some(AreCubicControl {
                    control_1: ArePathPoint::new(x1, y1),
                    control_2: ArePathPoint::new(x2, y2),
                }),
            },
        );
    }

    fn close(&mut self) {
        if let (Some(point), Some(contour_start)) = (self.current, self.contour_start) {
            let source_point_index = self.push(ArePathVerb::Close, point);
            self.push_segment(
                source_point_index,
                ArePathVerb::Close,
                Some(point),
                contour_start,
                AreSourceSegmentKind::Close(AreContourClose {
                    from: point,
                    to: contour_start,
                }),
            );
        }
    }
}

pub fn classify_are_source_bounds(
    input: &AreBezierSourcePathInput,
) -> Result<AreSourceBounds, AreSourceSamplerError> {
    validate_source_input(input)?;

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (index, point) in input.points.iter().copied().enumerate() {
        ensure_finite_point(index, point)?;
        let transformed = input.transform.apply(point);
        ensure_finite_point(index, transformed)?;
        min_x = min_x.min(transformed.x);
        min_y = min_y.min(transformed.y);
        max_x = max_x.max(transformed.x);
        max_y = max_y.max(transformed.y);
    }
    for segment in &input.segments {
        for point in source_segment_control_points(segment) {
            let transformed = input.transform.apply(point);
            min_x = min_x.min(transformed.x);
            min_y = min_y.min(transformed.y);
            max_x = max_x.max(transformed.x);
            max_y = max_y.max(transformed.y);
        }
    }

    Ok(AreSourceBounds {
        x_min: round_native_min(min_x),
        y_min: round_native_min(min_y),
        x_max: round_native_max(max_x),
        y_max: round_native_max(max_y),
    })
}

pub fn build_e854_working_set(
    input: &AreBezierSourcePathInput,
) -> Result<AreSourceSamplerWorkingSet, AreSourceSamplerError> {
    let bounds = classify_are_source_bounds(input)?;
    let mut records = Vec::new();
    let mut subpath_start: Option<ArePathPoint> = None;
    let mut previous: Option<ArePathPoint> = None;

    for (index, (verb, point)) in input
        .verbs
        .iter()
        .copied()
        .zip(input.points.iter().copied())
        .enumerate()
    {
        match verb {
            ArePathVerb::MoveTo => {
                let transformed = input.transform.apply(point);
                subpath_start = Some(transformed);
                previous = Some(transformed);
            }
            ArePathVerb::LineTo | ArePathVerb::QuadTo | ArePathVerb::CubicTo => {
                let Some(start) = previous else {
                    return Err(AreSourceSamplerError::SegmentBeforeMoveTo { index });
                };
                let end = input.transform.apply(point);
                records.push(record_from_input_segment(
                    input,
                    index,
                    start,
                    end,
                    AreSourceRecord32::NORMAL_FLAG,
                ));
                previous = Some(end);
            }
            ArePathVerb::Close => {
                let Some(start) = previous else {
                    return Err(AreSourceSamplerError::SegmentBeforeMoveTo { index });
                };
                let Some(end) = subpath_start else {
                    return Err(AreSourceSamplerError::SegmentBeforeMoveTo { index });
                };
                records.push(record_from_input_segment(
                    input,
                    index,
                    start,
                    end,
                    AreSourceRecord32::SENTINEL_FLAG,
                ));
                previous = Some(end);
            }
            ArePathVerb::Unknown(_) => {
                return Err(AreSourceSamplerError::InvalidPathVerb { index, verb });
            }
        }
    }

    let record_count = records.len();
    Ok(AreSourceSamplerWorkingSet {
        inline_base_scratch_0x17e: 0,
        state_flag_0x180: false,
        current_point_cursor_0x182: input.points.len(),
        current_record_cursor_0x184: record_count,
        saved_point_cursor_0x186: 0,
        saved_record_cursor_0x188: 0,
        counter_state_0x18a: 0,
        event_type_class_0x18b: 0,
        vector_pool_root_0x18c: 0,
        record_vector_base_0x18e: 0,
        record_vector_capacity_0x196: record_count,
        mode_flag_0x601: matches!(input.mode, AreSourcePathMode::Stroke),
        mode_subflag_0x602: false,
        bounds,
        records,
        source_segments: input.segments.clone(),
    })
}

pub fn classify_edge_pair_candidate(
    working_set: &AreSourceSamplerWorkingSet,
    record_index: usize,
) -> Option<AreEdgePairCandidate> {
    let segment = working_set.source_segment_for_record_index(record_index)?;
    let kind = if segment.is_curve_flattening_candidate() {
        AreEdgePairCandidateKind::CurveFlatteningCandidate
    } else if segment.is_close_boundary {
        AreEdgePairCandidateKind::CloseBoundaryCandidate
    } else if has_opposite_line_edge_in_contour(working_set, record_index, segment) {
        AreEdgePairCandidateKind::ContourPairCandidate
    } else if matches!(segment.kind, AreSourceSegmentKind::Line) {
        AreEdgePairCandidateKind::NeedsOppositeEdge
    } else {
        AreEdgePairCandidateKind::StateTransitionCandidate
    };

    Some(AreEdgePairCandidate {
        kind,
        record_index,
        contour_id: segment.contour_id,
        segment_id: segment.segment_id,
        source_point_index: segment.source_point_index,
    })
}

fn has_opposite_line_edge_in_contour(
    working_set: &AreSourceSamplerWorkingSet,
    record_index: usize,
    segment: &AreSourceSegment,
) -> bool {
    let Some(previous) = segment.previous_point else {
        return false;
    };
    working_set
        .records
        .iter()
        .enumerate()
        .filter(|(candidate_index, _)| *candidate_index != record_index)
        .filter_map(|(_, record)| working_set.source_segment_for_record(record))
        .any(|candidate| {
            candidate.contour_id == segment.contour_id
                && matches!(candidate.kind, AreSourceSegmentKind::Line)
                && candidate.previous_point.is_some_and(|candidate_previous| {
                    let segment_vertical = (previous.x - segment.endpoint.x).abs() <= 0.001;
                    let candidate_vertical =
                        (candidate_previous.x - candidate.endpoint.x).abs() <= 0.001;
                    segment_vertical
                        && candidate_vertical
                        && (candidate_previous.x - previous.x).abs() > 0.001
                })
        })
}

fn validate_source_input(input: &AreBezierSourcePathInput) -> Result<(), AreSourceSamplerError> {
    if input.provenance == AreSourcePathProvenance::TypedSpanFallback {
        return Err(AreSourceSamplerError::TypedSpanFallbackRejected);
    }
    if input.provenance == AreSourcePathProvenance::CoverageRowFallback {
        return Err(AreSourceSamplerError::CoverageRowFallbackRejected);
    }
    if input.points.is_empty() || input.verbs.is_empty() {
        return Err(AreSourceSamplerError::EmptySourcePath);
    }
    if input.points.len() != input.verbs.len() {
        return Err(AreSourceSamplerError::PointVerbLenMismatch {
            points_len: input.points.len(),
            verbs_len: input.verbs.len(),
        });
    }
    let first_verb = input.verbs[0];
    if first_verb != ArePathVerb::MoveTo {
        return Err(AreSourceSamplerError::FirstVerbMustMoveTo { verb: first_verb });
    }
    for (index, verb) in input.verbs.iter().copied().enumerate() {
        if matches!(verb, ArePathVerb::Unknown(_)) {
            return Err(AreSourceSamplerError::InvalidPathVerb { index, verb });
        }
    }
    Ok(())
}

fn derive_segments_from_points_verbs(
    points: &[ArePathPoint],
    verbs: &[ArePathVerb],
) -> Vec<AreSourceSegment> {
    let mut segments = Vec::with_capacity(points.len());
    let mut contour_id = 0usize;
    let mut next_segment_id = 0usize;
    let mut previous: Option<ArePathPoint> = None;
    let mut contour_start: Option<ArePathPoint> = None;

    for (index, (point, verb)) in points
        .iter()
        .copied()
        .zip(verbs.iter().copied())
        .enumerate()
    {
        if verb == ArePathVerb::MoveTo && index != 0 {
            contour_id += 1;
        }

        let kind = match verb {
            ArePathVerb::MoveTo => {
                contour_start = Some(point);
                AreSourceSegmentKind::Move
            }
            ArePathVerb::LineTo => AreSourceSegmentKind::Line,
            ArePathVerb::QuadTo => AreSourceSegmentKind::Quadratic { control: None },
            ArePathVerb::CubicTo => AreSourceSegmentKind::Cubic { controls: None },
            ArePathVerb::Close => AreSourceSegmentKind::Close(AreContourClose {
                from: previous.unwrap_or(point),
                to: contour_start.unwrap_or(point),
            }),
            ArePathVerb::Unknown(_) => AreSourceSegmentKind::Move,
        };
        let endpoint = match kind {
            AreSourceSegmentKind::Close(close) => close.to,
            _ => point,
        };
        let is_close_boundary = matches!(kind, AreSourceSegmentKind::Close(_));
        segments.push(AreSourceSegment {
            contour_id: AreSourceContourId(contour_id),
            segment_id: AreSourceSegmentId(next_segment_id),
            source_point_index: index,
            verb,
            previous_point: previous,
            endpoint,
            kind,
            glyph_run_index: None,
            glyph_id: None,
            is_close_boundary,
        });
        next_segment_id += 1;

        if verb == ArePathVerb::MoveTo {
            previous = Some(point);
        } else if verb == ArePathVerb::Close {
            previous = contour_start;
        } else {
            previous = Some(point);
        }
    }

    segments
}

fn source_segment_control_points(segment: &AreSourceSegment) -> Vec<ArePathPoint> {
    match segment.kind {
        AreSourceSegmentKind::Quadratic {
            control: Some(control),
        } => vec![control.control],
        AreSourceSegmentKind::Cubic {
            controls: Some(controls),
        } => vec![controls.control_1, controls.control_2],
        AreSourceSegmentKind::Close(close) => vec![close.from, close.to],
        _ => Vec::new(),
    }
}

fn ensure_finite_point(index: usize, point: ArePathPoint) -> Result<(), AreSourceSamplerError> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(AreSourceSamplerError::NonFinitePoint { index })
    }
}

fn record_from_segment(
    source_point_index: usize,
    start: ArePathPoint,
    end: ArePathPoint,
    record_flag: u8,
) -> AreSourceRecord32 {
    AreSourceRecord32 {
        source_point_index_0x00: source_point_index,
        min_x_0x08: round_native_min(start.x.min(end.x)),
        min_y_0x0c: round_native_min(start.y.min(end.y)),
        max_x_0x10: round_native_max(start.x.max(end.x)),
        max_y_0x14: round_native_max(start.y.max(end.y)),
        record_flag_0x18: record_flag,
        merge_state_0x19: 0,
    }
}

fn record_from_input_segment(
    input: &AreBezierSourcePathInput,
    source_point_index: usize,
    fallback_start: ArePathPoint,
    fallback_end: ArePathPoint,
    record_flag: u8,
) -> AreSourceRecord32 {
    let Some(segment) = input
        .segments
        .iter()
        .find(|segment| segment.source_point_index == source_point_index)
    else {
        return record_from_segment(
            source_point_index,
            fallback_start,
            fallback_end,
            record_flag,
        );
    };

    let transformed = segment.transformed(input.transform);
    let mut points = Vec::new();
    points.push(transformed.previous_point.unwrap_or(fallback_start));
    points.push(transformed.endpoint);
    points.extend(source_segment_control_points(&transformed));

    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);

    AreSourceRecord32 {
        source_point_index_0x00: source_point_index,
        min_x_0x08: round_native_min(min_x),
        min_y_0x0c: round_native_min(min_y),
        max_x_0x10: round_native_max(max_x),
        max_y_0x14: round_native_max(max_y),
        record_flag_0x18: record_flag,
        merge_state_0x19: 0,
    }
}

fn round_native_min(value: f32) -> i32 {
    value.floor() as i32
}

fn round_native_max(value: f32) -> i32 {
    value.ceil() as i32
}

#[cfg(test)]
mod e854_working_set_tests {
    use super::*;

    fn source_path() -> AreBezierSourcePathInput {
        AreBezierSourcePathInput::source_owned(
            vec![
                ArePathPoint::new(1.25, 2.75),
                ArePathPoint::new(5.25, 2.75),
                ArePathPoint::new(5.25, 6.25),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo,
            ],
        )
    }

    #[test]
    fn source_input_points_and_verbs_are_source_owned() {
        let input = source_path();

        assert_eq!(input.provenance, AreSourcePathProvenance::SourceOwned);
        assert_eq!(input.points.len(), 3);
        assert_eq!(
            input.verbs,
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::LineTo
            ]
        );
    }

    #[test]
    fn bounds_classifier_uses_points_types_transform() {
        let input = source_path().with_transform(ArePathTransform {
            xx: 2.0,
            xy: 0.0,
            yx: 0.0,
            yy: 1.0,
            tx: 10.0,
            ty: -1.0,
        });

        let bounds = classify_are_source_bounds(&input).unwrap();

        assert_eq!(
            bounds,
            AreSourceBounds {
                x_min: 12,
                y_min: 1,
                x_max: 21,
                y_max: 6,
            }
        );
    }

    #[test]
    fn e854_working_set_initializes_cursors() {
        let working_set = build_e854_working_set(&source_path()).unwrap();

        assert_eq!(working_set.current_point_cursor_0x182, 3);
        assert_eq!(working_set.current_record_cursor_0x184, 2);
        assert_eq!(working_set.saved_point_cursor_0x186, 0);
        assert_eq!(working_set.saved_record_cursor_0x188, 0);
        assert_eq!(working_set.record_vector_capacity_0x196, 2);
        assert!(!working_set.mode_flag_0x601);
        assert!(!working_set.mode_subflag_0x602);
    }

    #[test]
    fn e854_record32_topology() {
        let working_set = build_e854_working_set(&source_path()).unwrap();
        let first = &working_set.records[0];

        assert_eq!(AreSourceRecord32::NATIVE_SIZE_BYTES, 0x20);
        assert_eq!(first.source_point_index_0x00, 1);
        assert_eq!(first.min_x_0x08, 1);
        assert_eq!(first.min_y_0x0c, 2);
        assert_eq!(first.max_x_0x10, 6);
        assert_eq!(first.max_y_0x14, 3);
        assert_eq!(first.record_flag_0x18, AreSourceRecord32::NORMAL_FLAG);
        assert_eq!(first.merge_state_0x19, 0);
    }

    #[test]
    fn multi_segment_path_emits_ordered_records() {
        let working_set = build_e854_working_set(&source_path()).unwrap();

        assert_eq!(working_set.records.len(), 2);
        assert_eq!(
            working_set
                .records
                .iter()
                .map(|record| record.source_point_index_0x00)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(working_set.records[1].min_x_0x08, 5);
        assert_eq!(working_set.records[1].max_y_0x14, 7);
    }

    #[test]
    fn invalid_path_verb_errors() {
        let input = AreBezierSourcePathInput::source_owned(
            vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(1.0, 1.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::Unknown(99)],
        );

        assert_eq!(
            build_e854_working_set(&input),
            Err(AreSourceSamplerError::InvalidPathVerb {
                index: 1,
                verb: ArePathVerb::Unknown(99),
            })
        );
    }

    #[test]
    fn empty_source_path_errors_or_empty_policy_is_explicit() {
        let input = AreBezierSourcePathInput::source_owned(Vec::new(), Vec::new());

        assert_eq!(
            build_e854_working_set(&input),
            Err(AreSourceSamplerError::EmptySourcePath)
        );
    }

    #[test]
    fn typed_span_fallback_rejected() {
        let input = AreBezierSourcePathInput::typed_span_fallback(
            vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(1.0, 1.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        );

        assert_eq!(
            build_e854_working_set(&input),
            Err(AreSourceSamplerError::TypedSpanFallbackRejected)
        );
    }

    #[test]
    fn no_renderer_integration() {
        let working_set = build_e854_working_set(&source_path()).unwrap();

        assert_eq!(working_set.records.len(), 2);
        assert_eq!(working_set.vector_pool_root_0x18c, 0);
    }

    #[test]
    fn quad_to_preserves_control_point() {
        let mut builder = GlyphSourcePathOutlineBuilder::default();
        ttf_parser::OutlineBuilder::move_to(&mut builder, 0.0, 0.0);
        ttf_parser::OutlineBuilder::quad_to(&mut builder, 1.0, 2.0, 3.0, 4.0);

        let curve = builder
            .segments
            .iter()
            .find(|segment| segment.verb == ArePathVerb::QuadTo)
            .unwrap();

        assert_eq!(
            curve.kind,
            AreSourceSegmentKind::Quadratic {
                control: Some(AreQuadraticControl {
                    control: ArePathPoint::new(1.0, 2.0),
                }),
            }
        );
        assert!(curve.is_curve_flattening_candidate());
    }

    #[test]
    fn curve_to_preserves_two_control_points() {
        let mut builder = GlyphSourcePathOutlineBuilder::default();
        ttf_parser::OutlineBuilder::move_to(&mut builder, 0.0, 0.0);
        ttf_parser::OutlineBuilder::curve_to(&mut builder, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0);

        let curve = builder
            .segments
            .iter()
            .find(|segment| segment.verb == ArePathVerb::CubicTo)
            .unwrap();

        assert_eq!(
            curve.kind,
            AreSourceSegmentKind::Cubic {
                controls: Some(AreCubicControl {
                    control_1: ArePathPoint::new(1.0, 2.0),
                    control_2: ArePathPoint::new(3.0, 4.0),
                }),
            }
        );
        assert!(curve.is_curve_flattening_candidate());
    }

    #[test]
    fn contour_id_increments_on_move_to() {
        let mut builder = GlyphSourcePathOutlineBuilder::default();
        ttf_parser::OutlineBuilder::move_to(&mut builder, 0.0, 0.0);
        ttf_parser::OutlineBuilder::line_to(&mut builder, 1.0, 0.0);
        ttf_parser::OutlineBuilder::move_to(&mut builder, 2.0, 0.0);
        ttf_parser::OutlineBuilder::line_to(&mut builder, 3.0, 0.0);

        let line_contours = builder
            .segments
            .iter()
            .filter(|segment| segment.verb == ArePathVerb::LineTo)
            .map(|segment| segment.contour_id)
            .collect::<Vec<_>>();

        assert_eq!(
            line_contours,
            vec![AreSourceContourId(0), AreSourceContourId(1)]
        );
    }

    #[test]
    fn close_path_preserved() {
        let mut builder = GlyphSourcePathOutlineBuilder::default();
        ttf_parser::OutlineBuilder::move_to(&mut builder, 4.0, 5.0);
        ttf_parser::OutlineBuilder::line_to(&mut builder, 7.0, 5.0);
        ttf_parser::OutlineBuilder::close(&mut builder);

        let close = builder
            .segments
            .iter()
            .find(|segment| segment.verb == ArePathVerb::Close)
            .unwrap();

        assert!(close.is_close_boundary);
        assert_eq!(
            close.kind,
            AreSourceSegmentKind::Close(AreContourClose {
                from: ArePathPoint::new(7.0, 5.0),
                to: ArePathPoint::new(4.0, 5.0),
            })
        );
    }

    #[test]
    fn source_record_keeps_contour_and_segment_id() {
        let input = AreBezierSourcePathInput::source_owned(
            vec![
                ArePathPoint::new(0.0, 0.0),
                ArePathPoint::new(1.0, 0.0),
                ArePathPoint::new(2.0, 0.0),
                ArePathPoint::new(3.0, 0.0),
            ],
            vec![
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
                ArePathVerb::MoveTo,
                ArePathVerb::LineTo,
            ],
        );
        let working_set = build_e854_working_set(&input).unwrap();

        let first = working_set.source_segment_for_record_index(0).unwrap();
        let second = working_set.source_segment_for_record_index(1).unwrap();

        assert_eq!(first.contour_id, AreSourceContourId(0));
        assert_eq!(first.segment_id, AreSourceSegmentId(1));
        assert_eq!(second.contour_id, AreSourceContourId(1));
        assert_eq!(second.segment_id, AreSourceSegmentId(3));
    }

    #[test]
    fn one_sided_vertical_edge_is_classified_not_materialized() {
        let input = AreBezierSourcePathInput::source_owned(
            vec![ArePathPoint::new(2.0, 0.0), ArePathPoint::new(2.0, 3.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        );
        let working_set = build_e854_working_set(&input).unwrap();
        let candidate = classify_edge_pair_candidate(&working_set, 0).unwrap();

        assert_eq!(candidate.kind, AreEdgePairCandidateKind::NeedsOppositeEdge);
        assert_eq!(candidate.contour_id, AreSourceContourId(0));
        assert_eq!(candidate.source_point_index, 1);
    }

    #[test]
    fn curve_records_can_be_flattening_candidates() {
        let mut builder = GlyphSourcePathOutlineBuilder::default();
        ttf_parser::OutlineBuilder::move_to(&mut builder, 0.0, 0.0);
        ttf_parser::OutlineBuilder::quad_to(&mut builder, 1.0, 2.0, 3.0, 4.0);
        let input = AreBezierSourcePathInput::source_owned(builder.points, builder.verbs)
            .with_segments(builder.segments);
        let working_set = build_e854_working_set(&input).unwrap();
        let candidate = classify_edge_pair_candidate(&working_set, 0).unwrap();

        assert_eq!(
            candidate.kind,
            AreEdgePairCandidateKind::CurveFlatteningCandidate
        );
        assert!(working_set
            .source_segment_for_record_index(0)
            .unwrap()
            .is_curve_flattening_candidate());
    }

    #[test]
    fn no_coverage_row_fallback() {
        let input = AreBezierSourcePathInput::coverage_row_fallback(
            vec![ArePathPoint::new(0.0, 0.0), ArePathPoint::new(1.0, 1.0)],
            vec![ArePathVerb::MoveTo, ArePathVerb::LineTo],
        );

        assert_eq!(
            build_e854_working_set(&input),
            Err(AreSourceSamplerError::CoverageRowFallbackRejected)
        );
    }
}
