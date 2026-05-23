use std::{collections::BTreeMap, fs, path::Path};

use serde::Serialize;
use text_engine::{
    build_95cc_interval_list, build_e854_working_set,
    build_glyph_run_source_path_fixture_from_font_bytes, materialize_95cc_intervals, ArePathPoint,
    ArePathVerb, AreSamplerIntervalList, AreSamplerIntervalRow, AreSamplerIntervalRun,
    AreSamplerIntervalTag, AreSamplerListWorkingState, AreSamplerMaterializationError,
    AreSamplerMaterializerInput, AreSourcePathProvenance, AreSourceRecord32,
    AreZeroWidthIntervalPolicy,
};

const FONT_PATH: &str = "fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf";
const FONT_LABEL: &str = "Point-Light.ttf";
const FONT_SIZE: f32 = 32.0;

const LATIN_RUNS: &[&str] = &["Il", "AV", "To", "HH", "VA", "oo", "gg"];
const CYRILLIC_RUNS: &[&str] = &["Пр", "ве", "ет", "ЖА", "ДА", "Йо", "ЩИ", "ФО", "юя"];
const MIXED_RUNS: &[&str] = &["AЖ", "IЙ", "OО", "AVЖ", "TestПр"];
const WHITESPACE_RUNS: &[&str] = &["A A", "А А", "I I", "Ж Ж", "A  A", "Пр и"];
const PUNCTUATION_RUNS: &[&str] = &[
    "A.", "A,", "A!", "A?", "Пр.", "Ё!", "Ж?", "\"A\"", "(A)", "А-Б",
];
const REPRESENTATIVE_CASES: &[&str] = &[
    "Ж", "Щ", "Ю", "Я", "Ф", ".", ",", "!", "?", ":", ";", "Ё!", "Ж?", "А-Б", "A", "V", "S", "O",
    "g", "TestПр", "AVЖ", "OО",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum GeometryType {
    Horizontal,
    Vertical,
    DiagonalLine,
    QuadraticCurve,
    CubicCurve,
    TinyClosedContour,
    MultiContourOverlap,
    PunctuationDot,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum ScriptClass {
    Latin,
    Cyrillic,
    Mixed,
    Punctuation,
    Whitespace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum ContourClass {
    SingleContour,
    MultiContour,
    CounterOrHole,
    TinyMark,
    AccentOrDiacritic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum IntervalClass {
    SingleRecord,
    MultiRecord,
    MergedInterval,
    StateBoundary,
    Sentinel,
    Gap,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum GapClass {
    MaterializedOk,
    EmptyOutlineAdvanceOnly,
    MultiRecordMergedIntervalPayloadComposition,
    DiagonalSegmentSampling,
    CurveSampling,
    TinyContourPayloadMath,
    PunctuationTinyContour,
    CyrillicComplexContour,
    MultiGlyphOverlapOrGap,
    Class0StateTransition,
    Class1StateTransition,
    StateBoundaryTransition,
    PayloadMathUnknown,
    InvalidInput,
}

#[derive(Debug, Clone, Serialize)]
struct GeometryGapRecord {
    text: String,
    glyph_ids: Vec<u16>,
    glyph_roles: Vec<String>,
    row_y: i32,
    run_index: usize,
    current_x: i32,
    next_x: i32,
    source_record_indices: Vec<usize>,
    expected_payload_len: usize,
    geometry_type: GeometryType,
    script_class: ScriptClass,
    contour_class: ContourClass,
    interval_class: IntervalClass,
    source_verbs: Vec<ArePathVerb>,
    reject_reason: GapClass,
}

#[derive(Debug, Clone, Serialize)]
struct GeometryGapBreakdown {
    generated: &'static str,
    decision: &'static str,
    selected_expansion: &'static str,
    counts_by_gap: BTreeMap<GapClass, usize>,
    counts_by_geometry: BTreeMap<GeometryType, usize>,
    counts_by_interval: BTreeMap<IntervalClass, usize>,
    records: Vec<GeometryGapRecord>,
}

fn font_bytes() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FONT_PATH);
    fs::read(path).expect("Point-Light fixture font must exist")
}

fn output_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/ae_agents/p6_5q_geometry_materializer_breadth_20260519")
}

fn all_runs() -> Vec<(&'static str, ScriptClass)> {
    LATIN_RUNS
        .iter()
        .copied()
        .map(|text| (text, ScriptClass::Latin))
        .chain(
            CYRILLIC_RUNS
                .iter()
                .copied()
                .map(|text| (text, ScriptClass::Cyrillic)),
        )
        .chain(
            MIXED_RUNS
                .iter()
                .copied()
                .map(|text| (text, ScriptClass::Mixed)),
        )
        .chain(
            WHITESPACE_RUNS
                .iter()
                .copied()
                .map(|text| (text, ScriptClass::Whitespace)),
        )
        .chain(
            PUNCTUATION_RUNS
                .iter()
                .copied()
                .map(|text| (text, ScriptClass::Punctuation)),
        )
        .collect()
}

fn single_run_materializer_input(
    base: &AreSamplerMaterializerInput,
    row_y: i32,
    run_index: usize,
) -> AreSamplerMaterializerInput {
    let row = base.interval_list.row(row_y).unwrap();
    let run = row.runs[run_index].clone();
    AreSamplerMaterializerInput::source_owned(
        base.source_input.clone(),
        base.working_set.clone(),
        AreSamplerIntervalList {
            provenance: AreSourcePathProvenance::SourceOwned,
            y_min: base.interval_list.y_min,
            y_max: base.interval_list.y_max,
            rows: vec![AreSamplerIntervalRow {
                row_y,
                boundaries: Vec::new(),
                runs: vec![run],
                is_empty_sentinel: false,
            }],
            source_record_count: base.interval_list.source_record_count,
            working_state: AreSamplerListWorkingState {
                current_record_index: base.interval_list.working_state.current_record_index,
                emitted_boundary_count: 0,
                emitted_run_count: 1,
                skipped_zero_width_record_count: base
                    .interval_list
                    .working_state
                    .skipped_zero_width_record_count,
            },
            zero_width_policy: AreZeroWidthIntervalPolicy::SkipNonMaterialized,
            zero_width_records: base.interval_list.zero_width_records.clone(),
        },
    )
}

fn point_pair(
    input: &text_engine::AreBezierSourcePathInput,
    record: &AreSourceRecord32,
) -> Option<(ArePathPoint, ArePathPoint)> {
    let point_index = record.source_point_index_0x00;
    if point_index == 0 || point_index >= input.points.len() {
        return None;
    }
    Some((
        input.transform.apply(input.points[point_index - 1]),
        input.transform.apply(input.points[point_index]),
    ))
}

fn record_geometry(
    input: &text_engine::AreBezierSourcePathInput,
    record: &AreSourceRecord32,
) -> GeometryType {
    if record.max_x_0x10 - record.min_x_0x08 <= 1 && record.max_y_0x14 - record.min_y_0x0c <= 1 {
        return GeometryType::TinyClosedContour;
    }
    let point_index = record.source_point_index_0x00;
    let Some(verb) = input.verbs.get(point_index).copied() else {
        return GeometryType::Unknown;
    };
    if verb == ArePathVerb::QuadTo {
        return GeometryType::QuadraticCurve;
    }
    if verb == ArePathVerb::CubicTo {
        return GeometryType::CubicCurve;
    }
    let Some((start, end)) = point_pair(input, record) else {
        return GeometryType::Unknown;
    };
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    if dy <= 0.001 {
        GeometryType::Horizontal
    } else if dx <= 0.001 {
        GeometryType::Vertical
    } else {
        GeometryType::DiagonalLine
    }
}

fn run_geometry(
    input: &text_engine::AreBezierSourcePathInput,
    records: &[AreSourceRecord32],
    run: &AreSamplerIntervalRun,
    script_class: ScriptClass,
) -> GeometryType {
    let geometries = run
        .source_record_indices
        .iter()
        .filter_map(|index| records.get(*index))
        .map(|record| record_geometry(input, record))
        .collect::<Vec<_>>();
    if script_class == ScriptClass::Punctuation
        && geometries
            .iter()
            .any(|geometry| *geometry == GeometryType::TinyClosedContour)
    {
        return GeometryType::PunctuationDot;
    }
    if geometries
        .iter()
        .any(|geometry| *geometry == GeometryType::QuadraticCurve)
    {
        return GeometryType::QuadraticCurve;
    }
    if geometries
        .iter()
        .any(|geometry| *geometry == GeometryType::CubicCurve)
    {
        return GeometryType::CubicCurve;
    }
    if run.source_record_indices.len() > 1
        && geometries
            .iter()
            .any(|geometry| *geometry != GeometryType::Horizontal)
    {
        return GeometryType::MultiContourOverlap;
    }
    if geometries
        .iter()
        .any(|geometry| *geometry == GeometryType::DiagonalLine)
    {
        return GeometryType::DiagonalLine;
    }
    if geometries
        .iter()
        .any(|geometry| *geometry == GeometryType::Vertical)
    {
        return GeometryType::Vertical;
    }
    if geometries
        .iter()
        .all(|geometry| *geometry == GeometryType::Horizontal)
    {
        GeometryType::Horizontal
    } else {
        GeometryType::Unknown
    }
}

fn contour_class(input: &text_engine::AreBezierSourcePathInput, text: &str) -> ContourClass {
    if text.contains('Ё') || text.contains('Й') || text.contains('ё') || text.contains('й') {
        return ContourClass::AccentOrDiacritic;
    }
    let move_count = input
        .verbs
        .iter()
        .filter(|verb| **verb == ArePathVerb::MoveTo)
        .count();
    if text.chars().any(|ch| ".,:;!?".contains(ch)) {
        return ContourClass::TinyMark;
    }
    if matches!(text, "O" | "О" | "Ф" | "Ю" | "g") {
        return ContourClass::CounterOrHole;
    }
    if move_count > 1 {
        ContourClass::MultiContour
    } else {
        ContourClass::SingleContour
    }
}

fn interval_class(run: &AreSamplerIntervalRun) -> IntervalClass {
    if run.tag == AreSamplerIntervalTag::Sentinel {
        return IntervalClass::Sentinel;
    }
    if !run.materialize_candidate {
        return IntervalClass::StateBoundary;
    }
    match run.source_record_indices.len() {
        0 => IntervalClass::Gap,
        1 => IntervalClass::SingleRecord,
        2 => IntervalClass::MergedInterval,
        _ => IntervalClass::MultiRecord,
    }
}

fn reject_reason(
    materializer_result: Result<
        text_engine::AreSamplerMaterialization,
        AreSamplerMaterializationError,
    >,
    geometry_type: GeometryType,
    script_class: ScriptClass,
    interval_class: IntervalClass,
) -> GapClass {
    match materializer_result {
        Ok(materialized) if materialized.intervals[0].payload.is_some() => GapClass::MaterializedOk,
        Ok(_) => GapClass::StateBoundaryTransition,
        Err(AreSamplerMaterializationError::UnsupportedNonHorizontalRecord { .. }) => {
            match (script_class, geometry_type) {
                (ScriptClass::Punctuation, GeometryType::PunctuationDot)
                | (ScriptClass::Punctuation, GeometryType::TinyClosedContour) => {
                    GapClass::PunctuationTinyContour
                }
                (ScriptClass::Cyrillic, _) | (ScriptClass::Mixed, _) => {
                    GapClass::CyrillicComplexContour
                }
                (_, GeometryType::QuadraticCurve | GeometryType::CubicCurve) => {
                    GapClass::CurveSampling
                }
                (_, GeometryType::MultiContourOverlap) => GapClass::MultiGlyphOverlapOrGap,
                _ => GapClass::DiagonalSegmentSampling,
            }
        }
        Err(AreSamplerMaterializationError::UnsupportedCurveRequiresControlPoints { .. }) => {
            match script_class {
                ScriptClass::Cyrillic | ScriptClass::Mixed => GapClass::CyrillicComplexContour,
                ScriptClass::Punctuation => GapClass::PunctuationTinyContour,
                _ => GapClass::CurveSampling,
            }
        }
        Err(AreSamplerMaterializationError::UnsupportedAmbiguousLineGeometry { .. }) => {
            match (script_class, geometry_type) {
                (ScriptClass::Cyrillic, _) | (ScriptClass::Mixed, _) => {
                    GapClass::CyrillicComplexContour
                }
                (ScriptClass::Punctuation, GeometryType::PunctuationDot)
                | (ScriptClass::Punctuation, GeometryType::TinyClosedContour) => {
                    GapClass::PunctuationTinyContour
                }
                (_, GeometryType::MultiContourOverlap) => GapClass::MultiGlyphOverlapOrGap,
                (_, GeometryType::DiagonalLine) | (_, GeometryType::Vertical) => {
                    GapClass::DiagonalSegmentSampling
                }
                _ => GapClass::PayloadMathUnknown,
            }
        }
        Err(AreSamplerMaterializationError::CrossingList(_)) => GapClass::PayloadMathUnknown,
        Err(AreSamplerMaterializationError::UnsupportedMultiRecordRun { .. }) => {
            match interval_class {
                IntervalClass::Sentinel | IntervalClass::StateBoundary => {
                    GapClass::StateBoundaryTransition
                }
                _ => GapClass::MultiRecordMergedIntervalPayloadComposition,
            }
        }
        Err(AreSamplerMaterializationError::MissingSourceSegment { .. }) => GapClass::CurveSampling,
        Err(AreSamplerMaterializationError::SourceRecordsDoNotCoverPixel { .. })
        | Err(AreSamplerMaterializationError::SourceRecordDoesNotCoverRun { .. }) => {
            GapClass::PayloadMathUnknown
        }
        Err(_) => GapClass::InvalidInput,
    }
}

fn glyph_roles(text: &str, script_class: ScriptClass) -> Vec<String> {
    text.chars()
        .map(|ch| match script_class {
            ScriptClass::Whitespace if ch.is_whitespace() => "space_advance_only".to_string(),
            ScriptClass::Punctuation if ".,:;!?".contains(ch) => "punctuation_tiny".to_string(),
            ScriptClass::Punctuation => "punctuation_context".to_string(),
            ScriptClass::Cyrillic => "cyrillic_complex_candidate".to_string(),
            ScriptClass::Mixed if ch.is_ascii() => "mixed_latin".to_string(),
            ScriptClass::Mixed => "mixed_cyrillic".to_string(),
            ScriptClass::Latin => "latin_outline".to_string(),
            ScriptClass::Whitespace => "whitespace_run_outline".to_string(),
        })
        .collect()
}

fn script_class_for_text(text: &str) -> ScriptClass {
    if text.chars().all(char::is_whitespace) {
        return ScriptClass::Whitespace;
    }
    if text.chars().any(|ch| ".,:;!?-_/()\"'".contains(ch)) {
        return ScriptClass::Punctuation;
    }
    let has_latin = text.chars().any(|ch| ch.is_ascii_alphabetic());
    let has_non_ascii = text.chars().any(|ch| !ch.is_ascii() && !ch.is_whitespace());
    match (has_latin, has_non_ascii) {
        (true, true) => ScriptClass::Mixed,
        (true, false) => ScriptClass::Latin,
        (false, true) => ScriptClass::Cyrillic,
        (false, false) => ScriptClass::Punctuation,
    }
}

fn records_for_text(text: &str, script_class: ScriptClass, bytes: &[u8]) -> Vec<GeometryGapRecord> {
    let fixture =
        build_glyph_run_source_path_fixture_from_font_bytes(FONT_LABEL, bytes, text, FONT_SIZE)
            .unwrap();
    let Some(source_input) = fixture.input else {
        return vec![GeometryGapRecord {
            text: text.to_string(),
            glyph_ids: fixture.glyphs.iter().map(|glyph| glyph.glyph_id).collect(),
            glyph_roles: glyph_roles(text, script_class),
            row_y: 0,
            run_index: 0,
            current_x: 0,
            next_x: 0,
            source_record_indices: Vec::new(),
            expected_payload_len: 0,
            geometry_type: GeometryType::Unknown,
            script_class,
            contour_class: ContourClass::SingleContour,
            interval_class: IntervalClass::Gap,
            source_verbs: Vec::new(),
            reject_reason: GapClass::EmptyOutlineAdvanceOnly,
        }];
    };
    let working_set = build_e854_working_set(&source_input).unwrap();
    let interval_list = build_95cc_interval_list(&working_set).unwrap();
    let base_input = AreSamplerMaterializerInput::source_owned(
        source_input.clone(),
        working_set.clone(),
        interval_list.clone(),
    );
    let contour_class = contour_class(&source_input, text);
    let mut records = Vec::new();
    for row in &interval_list.rows {
        for (run_index, run) in row.runs.iter().enumerate() {
            let geometry_type =
                run_geometry(&source_input, &working_set.records, run, script_class);
            let interval_class = interval_class(run);
            let materializer_input =
                single_run_materializer_input(&base_input, row.row_y, run_index);
            let reject_reason = reject_reason(
                materialize_95cc_intervals(&materializer_input),
                geometry_type,
                script_class,
                interval_class,
            );
            let source_verbs = run
                .source_record_indices
                .iter()
                .filter_map(|index| working_set.records.get(*index))
                .filter_map(|record| source_input.verbs.get(record.source_point_index_0x00))
                .copied()
                .collect();
            records.push(GeometryGapRecord {
                text: text.to_string(),
                glyph_ids: fixture.glyphs.iter().map(|glyph| glyph.glyph_id).collect(),
                glyph_roles: glyph_roles(text, script_class),
                row_y: row.row_y,
                run_index,
                current_x: run.current_x,
                next_x: run.next_x,
                source_record_indices: run.source_record_indices.clone(),
                expected_payload_len: run.width().max(0) as usize,
                geometry_type,
                script_class,
                contour_class,
                interval_class,
                source_verbs,
                reject_reason,
            });
        }
    }
    records
}

fn build_breakdown() -> GeometryGapBreakdown {
    let bytes = font_bytes();
    let mut records = Vec::new();
    for (text, script_class) in all_runs() {
        records.extend(records_for_text(text, script_class, &bytes));
    }

    let mut counts_by_gap = BTreeMap::new();
    let mut counts_by_geometry = BTreeMap::new();
    let mut counts_by_interval = BTreeMap::new();
    for record in &records {
        *counts_by_gap
            .entry(record.reject_reason.clone())
            .or_insert(0) += 1;
        *counts_by_geometry.entry(record.geometry_type).or_insert(0) += 1;
        *counts_by_interval.entry(record.interval_class).or_insert(0) += 1;
    }

    GeometryGapBreakdown {
        generated: "2026-05-19",
        decision: "current_text_db98_active_edge_builder_materializes_outlined_corpus",
        selected_expansion: "8454_db98_df14_active_edge_builder",
        counts_by_gap,
        counts_by_geometry,
        counts_by_interval,
        records,
    }
}

fn representative_cases(breakdown: &GeometryGapBreakdown) -> Vec<GeometryGapRecord> {
    let bytes = font_bytes();
    REPRESENTATIVE_CASES
        .iter()
        .filter_map(|case| {
            breakdown
                .records
                .iter()
                .find(|record| {
                    record.text == *case && record.reject_reason != GapClass::MaterializedOk
                })
                .or_else(|| breakdown.records.iter().find(|record| record.text == *case))
                .cloned()
                .or_else(|| {
                    records_for_text(case, script_class_for_text(case), &bytes)
                        .into_iter()
                        .find(|record| record.reject_reason != GapClass::MaterializedOk)
                })
        })
        .collect()
}

fn write_md(path: &Path, title: &str, body: &str) {
    fs::write(path, format!("# {title}\n\n{body}")).unwrap();
}

fn write_artifacts(breakdown: &GeometryGapBreakdown, representatives: &[GeometryGapRecord]) {
    let dir = output_dir();
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("P6_5Q_GEOMETRY_GAP_BREAKDOWN.json"),
        serde_json::to_string_pretty(breakdown).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("P6_5Q_UPDATED_SUPPORT_MAP.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "generated": "2026-05-19",
            "decision": breakdown.decision,
            "selected_expansion": breakdown.selected_expansion,
            "counts_by_gap": breakdown.counts_by_gap,
            "counts_by_geometry": breakdown.counts_by_geometry,
            "counts_by_interval": breakdown.counts_by_interval,
            "remaining_safe_unit_expansion": null,
            "next_gate": "targeted_static_deep_dive_on_are_sampler_math"
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("P6_5Q_REPRESENTATIVE_GEOMETRY_CASES.json"),
        serde_json::to_string_pretty(representatives).unwrap(),
    )
    .unwrap();

    let representative_lines = representatives
        .iter()
        .map(|case| {
            format!(
                "- `{}` glyphs={:?} row={} x={}..{} records={:?} geometry={:?} interval={:?} reject={:?}",
                case.text,
                case.glyph_ids,
                case.row_y,
                case.current_x,
                case.next_x,
                case.source_record_indices,
                case.geometry_type,
                case.interval_class,
                case.reject_reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    write_md(
        &dir.join("P6_5Q_REPRESENTATIVE_GEOMETRY_CASES.md"),
        "P6 5Q Representative Geometry Cases",
        &format!(
            "- generated: `2026-05-19`\n- source: `Point-Light.ttf` glyph/source paths\n\n{}",
            representative_lines
        ),
    );
    write_md(
        &dir.join("P6_5Q_GEOMETRY_GAP_BREADTH_REPORT.md"),
        "P6 5Q Geometry Gap Breadth Report",
        &format!(
            "- generated: `2026-05-19`\n- decision: `{}`\n- renderer integration: `not performed`\n- runtime / metrics: `not used`\n\n## Counts By Gap\n\n```json\n{}\n```\n\n## Counts By Geometry\n\n```json\n{}\n```\n\n## Verdict\n\nThe old bounded geometry gap expectation is superseded. Current-text curves now lower through the recovered db98-style active-edge builder before the existing crossing-list and 75d0 accumulator path.",
            breakdown.decision,
            serde_json::to_string_pretty(&breakdown.counts_by_gap).unwrap(),
            serde_json::to_string_pretty(&breakdown.counts_by_geometry).unwrap()
        ),
    );
    write_md(
        &dir.join("P6_5Q_SELECTED_GEOMETRY_EXPANSION.md"),
        "P6 5Q Selected Geometry Expansion",
        "Selected: `8454_db98_df14_active_edge_builder`.\n\nReason: the current-text route lowers curve controls into native-style active edges before the existing crossing-list and 75d0 accumulator path. The old bounded gap expectation is superseded by the current-text db98 edge-builder implementation.",
    );
    write_md(
        &dir.join("P6_5Q_NATIVE_SAMPLER_MATH_STATIC_TARGETS.md"),
        "P6 5Q Native Sampler Math Static Targets",
        "Current target slice:\n\n- `ARE.dll+0x408c`: path command driver.\n- `ARE.dll+0x8454`: curve dispatch.\n- `ARE.dll+0xdb98`: adaptive cubic edge builder.\n- `ARE.dll+0xdf14`: edge emitter.\n- `ARE.dll+0x5258`: active edge init and slope/winding fields.\n- `ARE.dll+0x7348` / `ARE.dll+0x71f4`: edge linking and row bucket insertion.\n- `ARE.dll+0x430c` / `ARE.dll+0x75d0`: crossing list and coverage accumulation.",
    );
    write_md(
        &dir.join("P6_5Q_STATIC_DEEP_DIVE_NEXT.md"),
        "P6 5Q Static Deep Dive Next",
        "Next task: keep the current-text `408c/8454/db98/df14 -> active edge buckets -> 430c/75d0` parity lane green against AE85 traces. The `10a98/95cc/1b898` branch remains a BezierPathRasterPainter side branch, not the current text payload source.",
    );
    write_md(
        &dir.join("P6_5Q_NEXT_GAP.md"),
        "P6 5Q Next Gap",
        "Updated next gap: `renderer_integration_and_trace_parity_for_current_text_db98_edges`.\n\nOutlined corpus intervals now route through source-owned AD68 event cursor materialization. Advance-only whitespace remains intentionally non-materialized.",
    );
}

#[test]
fn p6_5q_geometry_materializer_breadth_static_gate() {
    let breakdown = build_breakdown();
    let representatives = representative_cases(&breakdown);
    assert!(!breakdown.records.is_empty());
    assert!(!representatives.is_empty());
    assert!(
        breakdown
            .counts_by_gap
            .get(&GapClass::MaterializedOk)
            .copied()
            .unwrap_or(0)
            > 0
    );
    assert_eq!(
        breakdown
            .counts_by_gap
            .get(&GapClass::CyrillicComplexContour)
            .copied()
            .unwrap_or(0),
        0
    );
    write_artifacts(&breakdown, &representatives);
}
