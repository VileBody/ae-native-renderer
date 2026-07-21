use crate::{
    style_registry::StyleRegistry, CapabilityFinding, CapabilityStatus, CompSpec, GeneratedPayload,
    VisualOperation,
};
use render_ir::{
    BlendMode, EffectSpec, KeyframeEase, Layer, Rect, ScalarKeyframe, TextAnimatorSpec,
    TextCharStyle, TextExpressionSelector, TextJustification, TextPaintSpec, TextRangeSelector,
    TextSelectorBasedOn, TextSelectorShape, Transform2D, Transform2DAnimation,
};
use serde_json::{json, Value};

#[derive(Debug)]
pub struct LoweredVisualLayer {
    pub sort_key: i64,
    pub layer: Layer,
}

#[derive(Debug, Default)]
pub struct VisualLoweringResult {
    pub layers: Vec<LoweredVisualLayer>,
    pub findings: Vec<CapabilityFinding>,
}

#[derive(Debug, Clone)]
struct WordTiming {
    word: String,
    start: f64,
    end: f64,
    voice: bool,
}

const BRAT_TRACKING: f32 = -20.0;
const BRAT_LAYER_SCALE: f32 = 0.80;
const BRAT_BLINK_PEAK: f32 = 0.4;
const BRAT_BLINK_SOFTNESS: f32 = 0.08;
const BRAT_DEFAULT_BPM: f64 = 120.0;
const BRAT_BLINK_EASE: KeyframeEase = KeyframeEase {
    x1: 1.0 / 3.0,
    y1: 0.0,
    x2: 2.0 / 3.0,
    y2: 1.0,
};
const TRENDY_TRACKING: f32 = -55.0;

pub fn lower_visual_operations(
    payload: &GeneratedPayload,
    main_comp: &CompSpec,
) -> VisualLoweringResult {
    let mut result = VisualLoweringResult::default();
    for operation in &payload.visual_ops {
        if operation
            .target
            .composition
            .as_deref()
            .is_some_and(|target| target != main_comp.name)
        {
            result.findings.push(CapabilityFinding {
                status: CapabilityStatus::NotImplemented,
                feature: format!("lower.visual_op.{}", operation.kind),
                layer: operation.id.clone(),
                detail: "visual operation targets a non-main composition".to_string(),
            });
            continue;
        }

        let lowered = match operation.kind.as_str() {
            "subtitle.brat.v1" => lower_brat(operation, main_comp),
            "subtitle.trendy.v1" => lower_trendy(operation, main_comp),
            "subtitle.bot.impulse_2nd.v1" => {
                lower_bot_subtitles(operation, main_comp, "impulse_2nd")
            }
            "subtitle.bot.scenes_3rd.v1" => lower_bot_subtitles(operation, main_comp, "scenes_3rd"),
            "subtitle.bot.scenes_3rd_single_step.v1" => {
                lower_bot_subtitles(operation, main_comp, "scenes_3rd_single_step")
            }
            "subtitle.bot.template_4th.v1" => {
                lower_bot_subtitles(operation, main_comp, "template_4th")
            }
            "subtitle.bot.legacy_blocks.v1" => Some(VisualLoweringResult {
                layers: Vec::new(),
                findings: vec![CapabilityFinding {
                    status: CapabilityStatus::NotImplemented,
                    feature: "lower.visual_op.subtitle.bot.legacy_blocks.v1".to_string(),
                    layer: operation.id.clone(),
                    detail:
                        "legacy macro-block renderer is intentionally out of native P0/P1 scope"
                            .to_string(),
                }],
            }),
            "style.semantic.v1" => lower_semantic_style(operation, main_comp, payload),
            "hook.f1.sound.v1" => lower_f1(operation, main_comp),
            "hook.f2.object.v1" => lower_f2(operation, main_comp),
            "hook.f3.effect.v1" => lower_f3(operation, main_comp, payload),
            "hook.f4.motion.v1" => lower_f4(operation, main_comp),
            "hook.f5.cognition.v1" => lower_f5(operation, main_comp),
            _ => None,
        };
        if let Some(mut lowered) = lowered {
            let layer_count = lowered.layers.len();
            result.layers.append(&mut lowered.layers);
            result.findings.append(&mut lowered.findings);
            if layer_count > 0 {
                result.findings.push(CapabilityFinding {
                    status: CapabilityStatus::Approximate,
                    feature: format!("lower.visual_op.{}", operation.kind),
                    layer: operation.id.clone(),
                    detail: format!(
                        "lowered to {layer_count} native layers; remaining parity gaps are reported by the operation capability"
                    ),
                });
            }
        }
    }
    lower_implicit_impulse_flash(payload, main_comp, &mut result);
    result
}

fn lower_implicit_impulse_flash(
    payload: &GeneratedPayload,
    comp: &CompSpec,
    result: &mut VisualLoweringResult,
) {
    if payload.project_spec.subtitles_mode.as_deref() != Some("impulse_2nd")
        || payload
            .visual_ops
            .iter()
            .any(|operation| f3_has_effect(operation, "flash_on_cuts"))
    {
        return;
    }

    // Raw JSX requests do not carry bot_adapter's generated F3 operation.
    // Recreate the template's cut flash from the imported footage timeline.
    let operation = VisualOperation {
        id: Some("implicit_impulse_flash_on_cuts".to_string()),
        kind: "hook.f3.effect.v1".to_string(),
        target: Default::default(),
        timing: Default::default(),
        params: json!({"detected_effect_ids": ["flash_on_cuts"]}),
        assets: Vec::new(),
        required: false,
    };
    let Some(mut lowered) = lower_flash_on_cuts(&operation, comp, payload) else {
        return;
    };
    // Direct JSX puts the subtitle master precomp at the top (z=1) and
    // footage below it (z>=100). Keep the flash in between those two stacks.
    for layer in &mut lowered.layers {
        layer.sort_key += 1_000_000;
    }
    result.layers.append(&mut lowered.layers);
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Approximate,
        feature: "lower.implicit_impulse_flash_on_cuts".to_string(),
        layer: Some("implicit_impulse_flash_on_cuts".to_string()),
        detail: "raw impulse JSX did not expose visualOps; flash_on_cuts was reconstructed from footage boundaries".to_string(),
    });
}

fn lower_brat(operation: &VisualOperation, comp: &CompSpec) -> Option<VisualLoweringResult> {
    let words = operation_words(operation);
    if words.is_empty() {
        return None;
    }
    let blocks = brat_blocks(&words, 2, 4);
    let box_width = comp.w as f32 * 0.80;
    let box_height = comp.h as f32 * 0.50;
    let longest_line = blocks
        .iter()
        .flat_map(|block| block.lines())
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1) as f32;
    let estimated_line_em =
        longest_line * 0.54 + (longest_line - 1.0).max(0.0) * (BRAT_TRACKING / 1000.0);
    let font_size = (box_width * 0.97 / estimated_line_em.max(1.0))
        .min(130.0)
        .max(56.0);
    let fill = operation_fill(operation, [255, 255, 255, 255]);
    let frame = 1.0 / comp.fps.max(1.0);
    let span_start = words.first()?.start;
    let span_end = words.last()?.end;
    let bpm = operation
        .params
        .get("bpm")
        .and_then(Value::as_f64)
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
        .unwrap_or(BRAT_DEFAULT_BPM)
        .max(1.0);
    let image_wipe_params = json!({
        "completion": {
            "keyframes": brat_completion_keyframes(span_start, span_end, bpm)
        },
        "border_softness": BRAT_BLINK_SOFTNESS
    });
    let mut result = VisualLoweringResult::default();

    for (index, block) in blocks.iter().enumerate() {
        let start = block.words.first()?.start;
        let end = block.words.last()?.end.max(start + frame);
        let word_count = block.words.len().max(1) as f32;
        let mut reveal_keys = Vec::with_capacity(block.words.len() * 2);
        let mut previous = f64::NEG_INFINITY;
        for (word_index, word) in block.words.iter().enumerate() {
            let hold_time = word.start.max(previous + frame * 0.5);
            let jump_time = hold_time + frame;
            reveal_keys.push(scalar_key(
                hold_time,
                word_index as f32 / word_count * 100.0,
            ));
            reveal_keys.push(scalar_key(
                jump_time,
                (word_index + 1) as f32 / word_count * 100.0,
            ));
            previous = jump_time;
        }
        let text = block
            .lines()
            .into_iter()
            .map(|line| line.to_lowercase())
            .collect::<Vec<_>>()
            .join("\n");
        let justification = if block.words.len() < 2 {
            TextJustification::Left
        } else {
            TextJustification::Full
        };
        let transform = Transform2D {
            anchor: [comp.w as f32 * 0.5, comp.h as f32 * 0.5],
            position: [comp.w as f32 * 0.5, comp.h as f32 * 0.5],
            scale: [BRAT_LAYER_SCALE * 100.0, BRAT_LAYER_SCALE * 100.0],
            ..Transform2D::default()
        };
        let effects = vec![
            EffectSpec {
                match_name: "ADBE Drop Shadow".to_string(),
                params: json!({
                    "color": [0, 0, 0, 255],
                    "opacity": 140.25,
                    "direction": 135,
                    "distance": 9,
                    "softness": 22
                }),
            },
            EffectSpec {
                match_name: "ADBE Minimax".to_string(),
                params: json!({
                    "operation": "maximum",
                    "channels": "alpha_and_color",
                    "radius": {
                        "keyframes": [
                            {"time": start, "value": 15.0},
                            {"time": start + frame, "value": 0.0}
                        ]
                    }
                }),
            },
            EffectSpec {
                match_name: "ADBE Gaussian Blur 2".to_string(),
                params: json!({"blurriness": 10.0}),
            },
            EffectSpec {
                match_name: "CC Image Wipe".to_string(),
                params: image_wipe_params.clone(),
            },
        ];
        result.layers.push(LoweredVisualLayer {
            sort_key: -1_000_000 + index as i64 * 100,
            layer: Layer::Text {
                id: format!("visual_brat_{index:04}"),
                start,
                duration: (end - start).max(frame),
                text,
                font: "ArialNarrow".to_string(),
                fontSize: font_size,
                char_styles: Vec::new(),
                blend_mode: BlendMode::Difference,
                tracking: BRAT_TRACKING,
                leading: Some(font_size),
                center_source_rect_y: true,
                justification,
                fill,
                box_: Some(Rect {
                    x: (comp.w as f32 - box_width) * 0.5,
                    y: (comp.h as f32 - box_height) * 0.5,
                    w: box_width,
                    h: box_height,
                }),
                transform,
                text_animators: vec![TextAnimatorSpec {
                    name: "brat_word_reveal".to_string(),
                    opacity: 0.0,
                    position: None,
                    scale: None,
                    rotation: None,
                    blur: None,
                    selector: TextRangeSelector {
                        start: 0.0,
                        end: 100.0,
                        start_keyframes: reveal_keys,
                        end_keyframes: Vec::new(),
                        based_on: TextSelectorBasedOn::Words,
                        smoothness: 0.0,
                        shape: TextSelectorShape::Square,
                        randomize_order: false,
                        wiggly: None,
                    },
                    expression_selector: None,
                }],
                effects,
            },
        });
    }
    Some(result)
}

fn lower_flash_on_cuts(
    operation: &VisualOperation,
    comp: &CompSpec,
    payload: &GeneratedPayload,
) -> Option<VisualLoweringResult> {
    let frame = 1.0 / comp.fps.max(1.0);
    let mut cuts = operation
        .params
        .get("cut_times")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .filter(|time| time.is_finite() && *time >= 0.0 && *time < comp.dur)
        .collect::<Vec<_>>();
    cuts.extend(
        payload
            .footage_layers
            .iter()
            .filter(|layer| {
                (layer.kind == "footage" && !payload_layer_is_audio(layer))
                    || (layer.kind == "solid" && layer.name.starts_with("strobe_bg_"))
            })
            .map(|layer| layer.in_point)
            .filter(|time| time.is_finite() && *time >= 0.0 && *time < comp.dur)
            .collect::<Vec<_>>(),
    );
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|left, right| (*left - *right).abs() <= frame);
    if cuts.is_empty() {
        return None;
    }

    let mut result = VisualLoweringResult::default();
    for (index, start) in cuts.into_iter().enumerate() {
        let end = (start + 0.633).min(comp.dur);
        let fade_end = (start + 0.6).min(end);
        let mut transform = Transform2D::default();
        transform.opacity = 40.0;
        transform.animation.opacity = vec![
            linear_scalar_key(start, 40.0),
            linear_scalar_key(fade_end, 0.0),
        ];
        result.layers.push(LoweredVisualLayer {
            // Dynamic subtitles stay above the flash; footage and strobe backgrounds stay below.
            sort_key: -500_000 + index as i64,
            layer: Layer::Solid {
                id: format!("visual_f3_flash_on_cut_{index:04}"),
                start,
                duration: (end - start).max(frame),
                blend_mode: BlendMode::Add,
                color: [255, 255, 255, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: comp.w as f32,
                    h: comp.h as f32,
                },
                transform,
                effects: Vec::new(),
            },
        });
    }
    Some(result)
}

fn lower_f1(operation: &VisualOperation, comp: &CompSpec) -> Option<VisualLoweringResult> {
    let start = f3_operation_start(operation).clamp(0.0, comp.dur);
    let duration = 0.35_f64.min((comp.dur - start).max(0.0));
    if duration <= 0.0 {
        return None;
    }
    let mut result = VisualLoweringResult::default();
    result.layers.push(light_flash_layer(
        "visual_f1_hook_light".to_string(),
        start,
        duration,
        comp,
        -500_410,
    ));
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Approximate,
        feature: "lower.visual_op.hook.f1.sound.v1.visual_chain".to_string(),
        layer: operation.id.clone(),
        detail: "native drop light is lowered; supplied SFX, trim, level, fades, and ducking are handled by the multi-track audio plan"
            .to_string(),
    });
    Some(result)
}

fn lower_f2(operation: &VisualOperation, comp: &CompSpec) -> Option<VisualLoweringResult> {
    let requested_shape =
        operation_string_param(operation, &["shape", "device", "object"]).unwrap_or("square");
    let shape = match requested_shape {
        "elipse" => "ellipse",
        other => other,
    };
    if !matches!(shape, "square" | "ellipse" | "rhomb" | "star1" | "star2") {
        return None;
    }
    let drop = f3_operation_start(operation).clamp(0.0, comp.dur);
    let duration = operation.timing.duration.unwrap_or(0.55).max(0.1);
    let start = (drop - duration).max(0.0);
    let duration = (drop - start).max(0.1).min(comp.dur - start);
    let fill = operation_fill(operation, [255, 255, 255, 255]);
    let shape_effect = EffectSpec {
        match_name: "ANR Shape Overlay".to_string(),
        params: json!({
            "shape": shape,
            "center_x": comp.w as f32 * 0.5,
            "center_y": comp.h as f32 * 0.5,
            "size": {
                "keyframes": [
                    {"time": start, "value": 24.0},
                    {"time": drop, "value": comp.w.min(comp.h) as f32 * 0.42}
                ]
            },
            "thickness": 14.0,
            "opacity": 100.0,
            "color": fill
        }),
    };
    let mut result = VisualLoweringResult::default();
    result.layers.push(LoweredVisualLayer {
        sort_key: -500_420,
        layer: Layer::Solid {
            id: format!("visual_f2_shape_{shape}"),
            start,
            duration,
            blend_mode: BlendMode::Add,
            color: [0, 0, 0, 0],
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: comp.w as f32,
                h: comp.h as f32,
            },
            transform: Transform2D::default(),
            effects: vec![shape_effect],
        },
    });
    if drop < comp.dur {
        result.layers.push(light_flash_layer(
            "visual_f2_hook_light".to_string(),
            drop,
            0.35_f64.min(comp.dur - drop),
            comp,
            -500_419,
        ));
    }
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Approximate,
        feature: format!("lower.visual_op.hook.f2.object.v1.{shape}"),
        layer: operation.id.clone(),
        detail: if requested_shape == shape {
            format!("{shape} lowered to a native procedural vector overlay plus drop light")
        } else {
            format!(
                "{requested_shape} normalized to {shape} and lowered to a native procedural vector overlay plus drop light"
            )
        },
    });
    Some(result)
}

fn lower_f4(operation: &VisualOperation, comp: &CompSpec) -> Option<VisualLoweringResult> {
    let device = operation_string_param(operation, &["device", "motion", "gesture"])?;
    if !matches!(device, "head" | "pinch" | "holdfinger" | "tap" | "swipe") {
        return None;
    }
    let start = f3_operation_start(operation).clamp(0.0, comp.dur);
    let duration = operation
        .timing
        .duration
        .unwrap_or(0.8)
        .max(0.1)
        .min(comp.dur - start);
    if duration <= 0.0 {
        return None;
    }
    let effect = EffectSpec {
        match_name: "ANR Shape Overlay".to_string(),
        params: json!({
            "device": device,
            "center_x": comp.w as f32 * 0.5,
            "center_y": comp.h as f32 * 0.38,
            "size": {
                "keyframes": [
                    {"time": start, "value": 80.0},
                    {"time": start + duration * 0.35, "value": 210.0},
                    {"time": start + duration, "value": 170.0}
                ]
            },
            "thickness": 16.0,
            "color": operation_fill(operation, [255, 255, 255, 255])
        }),
    };
    Some(VisualLoweringResult {
        layers: vec![LoweredVisualLayer {
            sort_key: -1_100_000,
            layer: Layer::Solid {
                id: format!("visual_f4_{device}"),
                start,
                duration,
                blend_mode: BlendMode::Add,
                color: [0, 0, 0, 0],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: comp.w as f32,
                    h: comp.h as f32,
                },
                transform: Transform2D::default(),
                effects: vec![effect],
            },
        }],
        findings: vec![CapabilityFinding {
            status: CapabilityStatus::Approximate,
            feature: format!("lower.visual_op.hook.f4.motion.v1.{device}"),
            layer: operation.id.clone(),
            detail: format!("{device} lowered to a native procedural gesture overlay"),
        }],
    })
}

fn lower_f5(operation: &VisualOperation, comp: &CompSpec) -> Option<VisualLoweringResult> {
    let start = f3_operation_start(operation).clamp(0.0, comp.dur);
    let duration = 0.35_f64.min((comp.dur - start).max(0.0));
    if duration <= 0.0 {
        return None;
    }
    let mut result = VisualLoweringResult::default();
    result.layers.push(light_flash_layer(
        "visual_f5_hook_light".to_string(),
        start,
        duration,
        comp,
        -500_430,
    ));
    if !operation_words(operation).is_empty() {
        if let Some(mut subtitles) = lower_trendy(operation, comp) {
            for lowered in &mut subtitles.layers {
                prefix_layer_id(&mut lowered.layer, "visual_f5_");
            }
            append_lowering(&mut result, subtitles);
        }
    }
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Approximate,
        feature: "lower.visual_op.hook.f5.cognition.v1".to_string(),
        layer: operation.id.clone(),
        detail: "native light and supplied word-timed subtitle clone are lowered; local tts_audio and ducking are handled by the multi-track audio plan"
            .to_string(),
    });
    Some(result)
}

fn prefix_layer_id(layer: &mut Layer, prefix: &str) {
    match layer {
        Layer::Solid { id, .. }
        | Layer::Footage { id, .. }
        | Layer::Text { id, .. }
        | Layer::Precomp { id, .. }
        | Layer::Adjustment { id, .. } => id.insert_str(0, prefix),
    }
}

fn operation_string_param<'a>(operation: &'a VisualOperation, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| operation.params.get(*name))
        .and_then(|value| match value {
            Value::String(value) => Some(value.as_str()),
            Value::Object(value) => value
                .get("id")
                .or_else(|| value.get("name"))
                .and_then(Value::as_str),
            _ => None,
        })
}

fn lower_f3(
    operation: &VisualOperation,
    comp: &CompSpec,
    payload: &GeneratedPayload,
) -> Option<VisualLoweringResult> {
    let mut result = VisualLoweringResult::default();
    if f3_has_effect(operation, "flash_on_cuts") {
        if let Some(lowered) = lower_flash_on_cuts(operation, comp, payload) {
            append_lowering(&mut result, lowered);
        }
    }
    if f3_has_effect(operation, "analog_glitch") {
        append_lowering(&mut result, lower_analog_glitch(comp));
    }
    if f3_has_effect(operation, "shutter_effect") {
        append_lowering(&mut result, lower_shutter_effect(operation, comp));
    }
    if f3_has_effect(operation, "snap_wipe") {
        append_lowering(&mut result, lower_snap_wipe(comp, payload));
    }
    for effect_id in [
        "invert_flash",
        "extract_flash",
        "minimax",
        "layer_shake",
        "hook_light",
        "flash_slow_shutter",
        "negative_zoom",
        "xerox",
        "neon_extract",
        "old_camera",
        "blackwhite",
        "crystal_glow",
        "night_vision",
        "wave",
    ] {
        if f3_has_effect(operation, effect_id) {
            append_lowering(
                &mut result,
                lower_native_f3_effect(operation, comp, payload, effect_id),
            );
        }
    }
    (!result.layers.is_empty()).then_some(result)
}

fn append_lowering(target: &mut VisualLoweringResult, mut source: VisualLoweringResult) {
    target.layers.append(&mut source.layers);
    target.findings.append(&mut source.findings);
}

fn f3_has_effect(operation: &VisualOperation, effect_id: &str) -> bool {
    if operation
        .params
        .get("detected_effect_ids")
        .and_then(Value::as_array)
        .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(effect_id)))
    {
        return true;
    }
    ["hook", "transition", "extra", "effect", "device"]
        .into_iter()
        .filter_map(|key| operation.params.get(key))
        .any(|value| match value {
            Value::String(value) => value == effect_id,
            Value::Array(values) => values.iter().any(|value| value.as_str() == Some(effect_id)),
            Value::Object(values) => {
                values
                    .get("id")
                    .or_else(|| values.get("name"))
                    .and_then(Value::as_str)
                    == Some(effect_id)
            }
            _ => false,
        })
}

fn lower_native_f3_effect(
    operation: &VisualOperation,
    comp: &CompSpec,
    payload: &GeneratedPayload,
    effect_id: &str,
) -> VisualLoweringResult {
    // The build worker reuses one F3 operation for hook/transition/extra, but
    // every EXTRA block receives startTime:0 in JSX. Do not inherit dropTime
    // here: it would silently skip persistent looks when the drop is at the end.
    let start = if f3_is_persistent_extra(effect_id) {
        0.0
    } else {
        f3_operation_start(operation).clamp(0.0, comp.dur)
    };
    let cut_times = f3_cut_times(comp, payload, false);
    let windows = match effect_id {
        "invert_flash" | "extract_flash" | "minimax" | "layer_shake" => {
            if cut_times.is_empty() {
                vec![start]
            } else {
                cut_times
            }
        }
        _ => vec![start],
    };
    let mut result = VisualLoweringResult::default();

    for (index, window_start) in windows.into_iter().enumerate() {
        let duration = match effect_id {
            "invert_flash" | "minimax" => 0.12,
            "extract_flash" => 0.18,
            "layer_shake" => 0.24,
            "hook_light" => 0.35,
            "flash_slow_shutter" => 0.50,
            "negative_zoom" => 0.50,
            _ => operation
                .timing
                .duration
                .unwrap_or_else(|| (comp.dur - window_start).max(0.0)),
        }
        .min((comp.dur - window_start).max(0.0));
        if duration <= 0.0 {
            continue;
        }
        let end = window_start + duration;
        if effect_id == "hook_light" {
            append_lowering(
                &mut result,
                lower_hook_light(
                    comp,
                    window_start,
                    format!("visual_f3_hook_light_{index:04}"),
                ),
            );
            continue;
        }

        let effects = match effect_id {
            "invert_flash" => vec![EffectSpec {
                match_name: "ADBE Invert".to_string(),
                params: json!({
                    "channel": 8,
                    "blend_with_original": {
                        "keyframes": [
                            {"time": window_start, "value": 0.0},
                            {"time": end, "value": 100.0}
                        ]
                    }
                }),
            }],
            "extract_flash" => vec![
                EffectSpec {
                    match_name: "ANR F3 Stylize".to_string(),
                    params: json!({
                        "mode": "extract",
                        "threshold": 175.0,
                        "softness": 36.0,
                        "amount": {
                            "keyframes": [
                                {"time": window_start, "value": 1.8},
                                {"time": end, "value": 0.0}
                            ]
                        },
                        "composite_original": true
                    }),
                },
                EffectSpec {
                    match_name: "ADBE Glo2".to_string(),
                    params: json!({
                        "based_on": "color channels",
                        "threshold": 170.0,
                        "radius": 24.0,
                        "intensity": 0.65,
                        "operation": "add",
                        "composite_original": "on top"
                    }),
                },
            ],
            "minimax" => vec![EffectSpec {
                match_name: "ADBE Minimax".to_string(),
                params: json!({
                    "operation": "maximum",
                    "channels": "alpha_and_color",
                    "direction": 1,
                    "radius": {
                        "keyframes": [
                            {"time": window_start, "value": 36.0},
                            {"time": end, "value": 0.0}
                        ]
                    }
                }),
            }],
            "layer_shake" => {
                let cx = comp.w as f32 * 0.5;
                let cy = comp.h as f32 * 0.5;
                vec![EffectSpec {
                    match_name: "ADBE Geometry2".to_string(),
                    params: json!({
                        "anchor": [cx, cy],
                        "position": {
                            "keyframes": [
                                {"time": window_start, "value": [cx, cy]},
                                {"time": window_start + duration * 0.25, "value": [cx + 18.0, cy - 12.0]},
                                {"time": window_start + duration * 0.50, "value": [cx - 14.0, cy + 10.0]},
                                {"time": window_start + duration * 0.75, "value": [cx + 8.0, cy - 5.0]},
                                {"time": end, "value": [cx, cy]}
                            ]
                        }
                    }),
                }]
            }
            "flash_slow_shutter" => vec![
                EffectSpec {
                    // AE: Echo(-.2, 5, .8, Add). Directional Blur is the
                    // available native temporal-trail approximation.
                    match_name: "ADBE Motion Blur".to_string(),
                    params: json!({"direction":90.0, "blur_length":42.0}),
                },
                EffectSpec {
                    match_name: "ADBE Posterize Time".to_string(),
                    params: json!({"frameRate":8.0}),
                },
            ],
            "negative_zoom" => {
                let cx = comp.w as f32 * 0.5;
                let cy = comp.h as f32 * 0.5;
                vec![
                    EffectSpec {
                        match_name: "ADBE Invert".to_string(),
                        params: json!({
                            "channel": 8,
                            "blend_with_original": {
                                "keyframes": [
                                    {"time": window_start, "value": 0.0},
                                    {"time": end, "value": 100.0}
                                ]
                            }
                        }),
                    },
                    EffectSpec {
                        match_name: "ADBE Geometry2".to_string(),
                        params: json!({
                            "anchor": [cx, cy],
                            "position": [cx, cy],
                            "scale_width": {
                                "keyframes": [
                                    {"time": window_start, "value": 130.0},
                                    {"time": end, "value": 100.0}
                                ]
                            },
                            "scale_height": {
                                "keyframes": [
                                    {"time": window_start, "value": 130.0},
                                    {"time": end, "value": 100.0}
                                ]
                            }
                        }),
                    },
                ]
            }
            "blackwhite" => vec![EffectSpec {
                match_name: "ANR F3 Stylize".to_string(),
                params: json!({
                    "mode": "blackwhite",
                    "magentas": -100.0,
                    "tint": true,
                    "tint_black": [0.0078160008, 0.006920415, 0.019607844, 1.0]
                }),
            }],
            // Production: Sharpen(50) -> Gaussian Blur(2) -> Sapphire S_Glint
            // (brightness .8, threshold .4, size 240). Native Glow has no star
            // streak kernel, so two ordered highlight blooms preserve the look.
            "crystal_glow" => vec![
                EffectSpec {
                    match_name: "ADBE Gaussian Blur 2".to_string(),
                    params: json!({"blurriness": 2.0, "repeat_edge_pixels": false}),
                },
                EffectSpec {
                    match_name: "ADBE Glo2".to_string(),
                    params: json!({
                        "based_on": "color channels", "threshold": 102.0,
                        "radius": 18.0, "intensity": 0.8, "operation": "add",
                        "composite_original": "on top"
                    }),
                },
                EffectSpec {
                    match_name: "ADBE Glo2".to_string(),
                    params: json!({
                        "based_on": "color channels", "threshold": 102.0,
                        "radius": 96.0, "intensity": 0.32, "operation": "add",
                        "composite_original": "on top"
                    }),
                },
            ],
            "night_vision" => vec![
                EffectSpec {
                    match_name: "ANR F3 Stylize".to_string(),
                    params: json!({"mode":"night_vision", "amount":1.0}),
                },
                EffectSpec {
                    match_name: "ADBE Gaussian Blur 2".to_string(),
                    params: json!({"blurriness":5.0}),
                },
                EffectSpec {
                    match_name: "ADBE Geometry2".to_string(),
                    params: json!({"scale_width":115.0, "scale_height":115.0}),
                },
            ],
            "wave" => vec![
                EffectSpec {
                    match_name: "ANR F3 Stylize".to_string(),
                    params: json!({"mode":"wave", "height":2.0, "width":125.4, "speed":-0.62}),
                },
                EffectSpec {
                    match_name: "ADBE Turbulent Displace".to_string(),
                    params: json!({"amount":4.0, "size":1000.0, "evolution":1.97}),
                },
                EffectSpec {
                    match_name: "ADBE Geometry2".to_string(),
                    params: json!({"scale_width":103.0, "scale_height":103.0}),
                },
            ],
            "xerox" | "neon_extract" | "old_camera" => vec![EffectSpec {
                match_name: "ANR F3 Stylize".to_string(),
                params: json!({
                    "mode": effect_id,
                    "amount": 1.0,
                    "threshold": 128.0,
                    "softness": 22.0
                }),
            }],
            _ => Vec::new(),
        };
        if effects.is_empty() {
            continue;
        }
        result.layers.push(LoweredVisualLayer {
            sort_key: -500_340 + index as i64,
            layer: Layer::Adjustment {
                id: format!("visual_f3_{effect_id}_{index:04}"),
                start: window_start,
                duration,
                effects,
            },
        });
    }

    if !result.layers.is_empty() {
        result.findings.push(CapabilityFinding {
            status: CapabilityStatus::Approximate,
            feature: format!("lower.visual_op.hook.f3.effect.v1.{effect_id}"),
            layer: operation.id.clone(),
            detail: format!("{effect_id} lowered to deterministic native Rust primitives"),
        });
    }
    result
}

fn lower_hook_light(comp: &CompSpec, start: f64, id: String) -> VisualLoweringResult {
    let flash_duration = 0.334_f64.min((comp.dur - start).max(0.0));
    let tail = 1.668_f64.min((comp.dur - start).max(0.0));
    if flash_duration <= 0.0 || tail <= 0.0 {
        return VisualLoweringResult::default();
    }
    let end = start + tail;
    let cx = comp.w as f32 * 0.5;
    let cy = comp.h as f32 * 0.5;
    let mut result = VisualLoweringResult::default();
    result
        .layers
        .push(light_flash_layer(id, start, flash_duration, comp, -500_360));
    result.layers.push(LoweredVisualLayer {
        sort_key: -500_359,
        layer: Layer::Adjustment {
            id: "visual_f3_hook_light_bolts".to_string(),
            start,
            duration: flash_duration,
            effects: vec![
                EffectSpec { match_name: "ANR Shape Overlay".to_string(), params: json!({"shape":"lightning_left", "opacity":85.0, "thickness":18.0, "size":540.0}) },
                EffectSpec { match_name: "ANR Shape Overlay".to_string(), params: json!({"shape":"lightning_right", "opacity":85.0, "thickness":18.0, "size":540.0}) },
            ],
        },
    });
    result.layers.push(LoweredVisualLayer {
        sort_key: -500_358,
        layer: Layer::Adjustment {
            id: "visual_f3_hook_light_zoom_shake".to_string(),
            start,
            duration: tail,
            effects: vec![
                EffectSpec { match_name: "ADBE Geometry2".to_string(), params: json!({
                    "anchor":[cx,cy], "position": {"keyframes":[
                        {"time":start,"value":[cx,cy]},
                        {"time":start + tail * 0.12,"value":[cx + 10.0,cy - 8.0]},
                        {"time":start + tail * 0.28,"value":[cx - 7.0,cy + 6.0]},
                        {"time":end,"value":[cx,cy]}
                    ]},
                    "scale_width":{"keyframes":[{"time":start,"value":110.0},{"time":end,"value":100.0}]},
                    "scale_height":{"keyframes":[{"time":start,"value":110.0},{"time":end,"value":100.0}]}
                }) },
            ],
        },
    });
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Approximate,
        feature: "lower.visual_op.hook.f3.effect.v1.hook_light".to_string(),
        layer: None,
        detail: "AE light hook is represented by Add flash, two lightning traces, drop-relative zoom, and deterministic shake".to_string(),
    });
    result
}

fn f3_is_persistent_extra(effect_id: &str) -> bool {
    matches!(
        effect_id,
        "xerox"
            | "neon_extract"
            | "old_camera"
            | "blackwhite"
            | "crystal_glow"
            | "night_vision"
            | "wave"
    )
}

fn light_flash_layer(
    id: String,
    start: f64,
    duration: f64,
    comp: &CompSpec,
    sort_key: i64,
) -> LoweredVisualLayer {
    let mut transform = Transform2D::default();
    transform.opacity = 0.0;
    transform.animation.opacity = vec![
        linear_scalar_key(start, 0.0),
        scalar_key(start + duration * 0.18, 65.0),
        scalar_key(start + duration, 0.0),
    ];
    LoweredVisualLayer {
        sort_key,
        layer: Layer::Solid {
            id,
            start,
            duration,
            blend_mode: BlendMode::Add,
            color: [255, 255, 255, 255],
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: comp.w as f32,
                h: comp.h as f32,
            },
            transform: transform.clone(),
            effects: Vec::new(),
        },
    }
}

fn lower_analog_glitch(comp: &CompSpec) -> VisualLoweringResult {
    VisualLoweringResult {
        layers: vec![LoweredVisualLayer {
            // The F3 adjustment belongs below subtitles and above footage.
            // EXTRA is injected after snap/shutter and moveAfter(text), so it is topmost in F3.
            sort_key: -500_300,
            layer: Layer::Adjustment {
                id: "visual_f3_analog_glitch".to_string(),
                start: 0.0,
                duration: comp.dur,
                effects: vec![
                    EffectSpec {
                        match_name: "ADBE Posterize Time".to_string(),
                        params: json!({"frameRate": 12.0}),
                    },
                    EffectSpec {
                        match_name: "ANR Analog Glitch".to_string(),
                        params: json!({
                            "contrast": 1.3,
                            "red_gain": 1.25,
                            "scanline_period": 4,
                            "scanline_on": 2,
                            "dot_period": 8,
                            "highlight_grid_strength": 1.0,
                            "wave_amplitude": 6,
                            "wave_width": 1200
                        }),
                    },
                    EffectSpec {
                        match_name: "ADBE Glo2".to_string(),
                        params: json!({
                            "based_on": "color channels",
                            "threshold": 12.75,
                            "radius": 17.0,
                            "intensity": 0.55,
                            "operation": "add",
                            "composite_original": "on top"
                        }),
                    },
                    EffectSpec {
                        match_name: "ADBE Glo2".to_string(),
                        params: json!({
                            "based_on": "color channels",
                            "threshold": 125.0,
                            "radius": 17.0,
                            "intensity": 0.35,
                            "operation": "add",
                            "composite_original": "on top"
                        }),
                    },
                ],
            },
        }],
        findings: Vec::new(),
    }
}

fn lower_shutter_effect(operation: &VisualOperation, comp: &CompSpec) -> VisualLoweringResult {
    const CHUNK: f64 = 0.100_100_100_100_1;
    const BLUR_FADE: f64 = 0.066_733_400_066_73;

    let start = f3_operation_start(operation).clamp(0.0, comp.dur);
    let mut result = VisualLoweringResult::default();
    for index in 0..6 {
        let chunk_start = start + index as f64 * CHUNK;
        let chunk_end = (start + (index + 1) as f64 * CHUNK).min(comp.dur);
        if chunk_end <= chunk_start {
            continue;
        }

        let mut effects = Vec::new();
        if index % 2 == 0 {
            effects.push(EffectSpec {
                match_name: "ADBE Invert".to_string(),
                params: json!({"channel": 8}),
            });
        }
        if index < 5 {
            effects.push(EffectSpec {
                match_name: "ADBE Motion Blur".to_string(),
                params: json!({
                    "direction": {
                        "keyframes": [
                            {"time": chunk_start, "value": 90.0},
                            {"time": (chunk_start + BLUR_FADE).min(chunk_end), "value": 0.0}
                        ]
                    },
                    "blur_length": {
                        "keyframes": [
                            {"time": chunk_start, "value": 15.0},
                            {"time": (chunk_start + BLUR_FADE).min(chunk_end), "value": 0.0}
                        ]
                    }
                }),
            });
        }
        if effects.is_empty() {
            continue;
        }
        result.layers.push(LoweredVisualLayer {
            sort_key: -500_100 + index as i64,
            layer: Layer::Adjustment {
                id: format!("visual_f3_shutter_{index:02}"),
                start: chunk_start,
                duration: chunk_end - chunk_start,
                effects,
            },
        });
    }
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Supported,
        feature: "lower.visual_op.hook.f3.effect.v1.shutter_effect".to_string(),
        layer: operation.id.clone(),
        detail: "six 0.1001001s shutter chunks, alternating Lightness Invert, and animated ADBE Motion Blur direction/length are native"
            .to_string(),
    });
    result
}

fn lower_snap_wipe(comp: &CompSpec, payload: &GeneratedPayload) -> VisualLoweringResult {
    const DURATION: f64 = 0.067;
    let mut result = VisualLoweringResult::default();
    let center_x = comp.w as f32 * 0.5;
    let center_y = comp.h as f32 * 0.5;
    for (index, start) in f3_cut_times(comp, payload, false).into_iter().enumerate() {
        let end = (start + DURATION).min(comp.dur);
        if end <= start {
            continue;
        }
        let duration = end - start;
        let ease = json!({"x1": 1.0 / 3.0, "y1": 0.0, "x2": 2.0 / 3.0, "y2": 1.0});
        result.layers.push(LoweredVisualLayer {
            sort_key: -500_200 + index as i64,
            layer: Layer::Adjustment {
                id: format!("visual_f3_snap_wipe_{index:04}"),
                start,
                duration,
                effects: vec![
                    EffectSpec {
                        match_name: "ADBE Geometry2".to_string(),
                        params: json!({
                            "anchor": {
                                "keyframes": [
                                    {"time": start, "value": [center_x, center_y], "ease": ease},
                                    {"time": start + duration * 0.5, "value": [center_x + 140.0, center_y], "ease": ease},
                                    {"time": end, "value": [center_x + 340.0, center_y], "ease": ease}
                                ]
                            }
                        }),
                    },
                    EffectSpec {
                        match_name: "ADBE Motion Blur".to_string(),
                        params: json!({
                            "direction": 90.0,
                            "blur_length": 100.0
                        }),
                    },
                    EffectSpec {
                        match_name: "ADBE Minimax".to_string(),
                        params: json!({
                            "operation": "maximum",
                            "radius": 165.0,
                            "direction": 2
                        }),
                    },
                    EffectSpec {
                        match_name: "ADBE Optics Compensation".to_string(),
                        params: json!({
                            "field_of_view": 120.0,
                            "reverse_lens_distortion": true
                        }),
                    },
                ],
            },
        });
    }
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Supported,
        feature: "lower.visual_op.hook.f3.effect.v1.snap_wipe".to_string(),
        layer: None,
        detail: "cut-relative 0.067s Geometry2, directional Motion Blur, full-radius Minimax, and reverse Optics Compensation are native"
            .to_string(),
    });
    result
}

fn f3_operation_start(operation: &VisualOperation) -> f64 {
    operation
        .timing
        .start
        .or_else(|| operation.params.get("drop_time").and_then(Value::as_f64))
        .unwrap_or(0.0)
}

fn f3_cut_times(
    comp: &CompSpec,
    payload: &GeneratedPayload,
    include_strobe_backgrounds: bool,
) -> Vec<f64> {
    let frame = 1.0 / comp.fps.max(1.0);
    let mut cuts = payload
        .footage_layers
        .iter()
        .filter(|layer| {
            (layer.kind == "footage" && !payload_layer_is_audio(layer))
                || (include_strobe_backgrounds
                    && layer.kind == "solid"
                    && layer.name.starts_with("strobe_bg_"))
        })
        .map(|layer| layer.in_point)
        .filter(|time| time.is_finite() && *time >= 0.0 && *time < comp.dur)
        .collect::<Vec<_>>();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|left, right| (*left - *right).abs() <= frame);
    cuts
}

fn payload_layer_is_audio(layer: &crate::PayloadLayer) -> bool {
    if layer
        .text_data
        .pointer("/layer_meta/audioEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    let file_name = layer
        .text_data
        .pointer("/source_footage/file_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    file_name.ends_with(".mp3")
        || file_name.ends_with(".wav")
        || file_name.ends_with(".m4a")
        || file_name.ends_with(".aac")
}

fn lower_trendy(operation: &VisualOperation, comp: &CompSpec) -> Option<VisualLoweringResult> {
    let words = operation_words(operation);
    if words.is_empty() {
        return None;
    }
    let fill = operation_fill(operation, [255, 255, 255, 255]);
    let mut result = VisualLoweringResult::default();
    for (index, word) in words.iter().enumerate() {
        let next_start = words.get(index + 1).map(|next| next.start);
        let end = next_start.unwrap_or(word.end + 0.30);
        let duration = (end - word.start).max(0.1);
        let phrase = word.word.to_uppercase();
        let char_count = phrase.chars().count().max(1) as f32;
        let estimated_width_em =
            char_count * 0.58 + (char_count - 1.0).max(0.0) * (TRENDY_TRACKING / 1000.0);
        let width_fit = comp.w as f32 * 0.90 / estimated_width_em.max(1.0);
        let height_fit = comp.h as f32 * 0.92 / 4.0;
        let font_size = 130.0_f32.min(width_fit * 0.99).min(height_fit).max(40.0);
        let y_offset = font_size * 4.0 * 0.346;
        let local_width = comp.w as f32 * 0.90;
        let local_height = (font_size * 2.2).max(120.0);
        let transform = Transform2D {
            anchor: [comp.w as f32 * 0.5, comp.h as f32 * 0.5],
            position: [comp.w as f32 * 0.5, comp.h as f32 * 0.5 + y_offset],
            scale: [100.0, 400.0],
            rotation: 0.0,
            opacity: 100.0,
            motion_blur: false,
            animation: Transform2DAnimation::default(),
        };
        let box_rect = Rect {
            x: (comp.w as f32 - local_width) * 0.5,
            y: (comp.h as f32 - local_height) * 0.5,
            w: local_width,
            h: local_height,
        };
        let tracking_ease = KeyframeEase {
            x1: 0.333_333_33,
            y1: 0.0,
            x2: 0.666_666_7,
            y2: 1.0,
        };
        let base_key = -1_000_000 + index as i64;
        result.layers.push(LoweredVisualLayer {
            sort_key: base_key,
            layer: Layer::Text {
                id: format!("visual_trendy_{index:04}"),
                start: word.start,
                duration,
                text: phrase,
                font: "Montserrat-Bold".to_string(),
                fontSize: font_size,
                char_styles: Vec::new(),
                blend_mode: BlendMode::Normal,
                tracking: TRENDY_TRACKING,
                leading: None,
                center_source_rect_y: false,
                justification: TextJustification::Center,
                fill,
                box_: Some(box_rect.clone()),
                transform,
                text_animators: vec![TextAnimatorSpec {
                    name: "trendy_tracking_amount".to_string(),
                    opacity: 100.0,
                    position: None,
                    scale: None,
                    rotation: None,
                    blur: None,
                    selector: TextRangeSelector::default(),
                    expression_selector: Some(TextExpressionSelector::TrackingAmount {
                        value: 7.0,
                        keyframes: vec![
                            ScalarKeyframe {
                                time: word.start,
                                value: 7.0,
                                hold: false,
                                approximate: false,
                                ease: Some(tracking_ease),
                            },
                            ScalarKeyframe {
                                time: word.start + duration,
                                value: -1.0,
                                hold: false,
                                approximate: false,
                                ease: Some(tracking_ease),
                            },
                        ],
                    }),
                }],
                effects: vec![
                    EffectSpec {
                        match_name: "ANR Vertical Gradient".to_string(),
                        params: json!({
                            "text_paint": TextPaintSpec {
                                fill: Some(fill),
                                fill_enabled: true,
                                stroke_color: Some([0, 0, 0, 255]),
                                stroke_width: 5.0,
                                stroke_over_fill: false,
                            },
                            "top": [255, 255, 255, 255],
                            "bottom": [1, 1, 1, 255],
                            "brightness": 1.26,
                            "start_xy": [540.0, 772.0],
                            "end_xy": [565.33332824707, 1569.66667175293],
                            "source_match_name": "S_Gradient",
                            "sapphire_params": {
                                "0050": [540.0, 772.0],
                                "0051": [565.33332824707, 1569.66667175293],
                                "0052": [1.0, 1.0, 1.0, 1.0],
                                "0053": [0.00392156885937, 0.00392156885937, 0.00392156885937, 1.0],
                                "0054": 1.26,
                                "0057": 0
                            }
                        }),
                    },
                    EffectSpec {
                        match_name: "ADBE Drop Shadow".to_string(),
                        params: json!({
                            "color": [0, 0, 0, 255],
                            "opacity": 255,
                            "direction": 0,
                            "distance": 0,
                            "softness": 100,
                            "source_match_name": "S_DropShadow",
                            "sapphire_params": {
                                "0050": [0.0, 0.0, 0.0, 1.0],
                                "0051": 1.14,
                                "0052": 100.0,
                                "0053": 0,
                                "0054": 0
                            }
                        }),
                    },
                ],
            },
        });
    }
    Some(result)
}

#[derive(Debug, Clone)]
struct BotSubtitleSegment {
    id: String,
    text: String,
    start: f64,
    end: f64,
    lines: Vec<String>,
    words: Vec<WordTiming>,
    kind: String,
    focus_word: Option<String>,
    focus_style: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScenesRevealMode {
    Words,
    TwoGroups,
}

#[derive(Debug, Clone, Copy)]
struct ScenesTypeSpec {
    tracking: f32,
    reveal: ScenesRevealMode,
    accumulates: bool,
    echo: bool,
    outline_fill: bool,
}

fn scenes_type_spec(kind: &str) -> ScenesTypeSpec {
    match kind {
        "TYPE_2" => ScenesTypeSpec {
            tracking: -18.0,
            reveal: ScenesRevealMode::Words,
            accumulates: false,
            echo: false,
            outline_fill: false,
        },
        "TYPE_3" => ScenesTypeSpec {
            tracking: -24.0,
            reveal: ScenesRevealMode::Words,
            accumulates: true,
            echo: false,
            outline_fill: false,
        },
        "TYPE_4" => ScenesTypeSpec {
            tracking: -20.0,
            reveal: ScenesRevealMode::Words,
            accumulates: false,
            echo: true,
            outline_fill: false,
        },
        "TYPE_5" => ScenesTypeSpec {
            tracking: -18.0,
            reveal: ScenesRevealMode::Words,
            accumulates: false,
            echo: false,
            outline_fill: true,
        },
        "TYPE_6" => ScenesTypeSpec {
            tracking: -18.0,
            reveal: ScenesRevealMode::TwoGroups,
            accumulates: false,
            echo: false,
            outline_fill: false,
        },
        _ => ScenesTypeSpec {
            tracking: -18.0,
            reveal: ScenesRevealMode::Words,
            accumulates: false,
            echo: false,
            outline_fill: false,
        },
    }
}

fn lower_bot_subtitles(
    operation: &VisualOperation,
    comp: &CompSpec,
    source_mode: &str,
) -> Option<VisualLoweringResult> {
    let segments = bot_subtitle_segments(operation);
    if segments.is_empty() {
        return None;
    }
    let mut result = VisualLoweringResult::default();
    for (index, segment) in segments.iter().enumerate() {
        let type_spec = scenes_type_spec(&segment.kind);
        let red = segment.kind == "TYPE_4"
            || segment.focus_style.as_deref() == Some("red")
            || (source_mode == "template_4th" && segment.words.iter().any(|word| word.voice));
        let text = if segment.lines.is_empty() {
            segment.text.clone()
        } else {
            segment.lines.join("\n")
        };
        let font_size = bot_font_size(&text, comp, source_mode);
        let fill = if red {
            [238, 32, 30, 255]
        } else {
            [255, 255, 255, 255]
        };
        let box_width = comp.w as f32 * 0.88;
        let box_height = (font_size * 2.8).max(160.0);
        let y = match source_mode {
            "impulse_2nd" => comp.h as f32 * 0.50,
            "template_4th" => comp.h as f32 * 0.52,
            _ => comp.h as f32 * 0.50,
        };
        let mut transform = Transform2D {
            anchor: [comp.w as f32 * 0.5, comp.h as f32 * 0.5],
            position: [comp.w as f32 * 0.5, y],
            ..Transform2D::default()
        };
        transform.animation.opacity = bot_opacity_keys(segment, comp.fps);
        let effects = bot_scene_effects(source_mode, &segment.kind, fill);
        let char_styles = bot_focus_char_styles(&text, segment);
        let text_layer = Layer::Text {
            id: format!("bot_{source_mode}_{}_fill", segment.id),
            start: segment.start,
            duration: (segment.end - segment.start).max(1.0 / comp.fps.max(1.0)),
            text: text.clone(),
            font: "Montserrat-Bold".to_string(),
            fontSize: font_size,
            char_styles: char_styles.clone(),
            blend_mode: BlendMode::Normal,
            tracking: if source_mode == "template_4th" {
                -25.0
            } else {
                type_spec.tracking
            },
            leading: Some(font_size * 0.92),
            center_source_rect_y: true,
            justification: TextJustification::Center,
            fill,
            box_: Some(Rect {
                x: (comp.w as f32 - box_width) * 0.5,
                y: (comp.h as f32 - box_height) * 0.5,
                w: box_width,
                h: box_height,
            }),
            transform: transform.clone(),
            text_animators: vec![bot_scene_reveal(
                &segment.words,
                segment.start,
                segment.end,
                comp.fps,
                type_spec.reveal,
            )],
            effects,
        };

        if type_spec.echo {
            let mut echo_transform = transform.clone();
            echo_transform.animation.opacity = bot_echo_opacity_keys(segment, comp.fps);
            result.layers.push(LoweredVisualLayer {
                sort_key: -1_200_010 + index as i64 * 10,
                layer: Layer::Text {
                    id: format!("bot_{source_mode}_{}_echo", segment.id),
                    start: segment.start,
                    duration: (segment.end - segment.start).max(1.0 / comp.fps.max(1.0)),
                    text: text.clone(),
                    font: "Montserrat-Bold".to_string(),
                    fontSize: font_size * 1.18,
                    char_styles: Vec::new(),
                    blend_mode: BlendMode::Normal,
                    tracking: type_spec.tracking,
                    leading: Some(font_size * 0.92),
                    center_source_rect_y: true,
                    justification: TextJustification::Center,
                    fill: [238, 32, 30, 170],
                    box_: Some(Rect {
                        x: (comp.w as f32 - box_width) * 0.5,
                        y: (comp.h as f32 - box_height) * 0.5,
                        w: box_width,
                        h: box_height,
                    }),
                    transform: echo_transform,
                    text_animators: vec![bot_scene_reveal(
                        &segment.words,
                        segment.start,
                        segment.end,
                        comp.fps,
                        type_spec.reveal,
                    )],
                    effects: vec![EffectSpec {
                        match_name: "ADBE Gaussian Blur 2".to_string(),
                        params: json!({"blurriness": 52.0, "repeat_edge_pixels": false}),
                    }],
                },
            });
        }

        if type_spec.outline_fill {
            let mut outline_transform = Transform2D {
                anchor: [comp.w as f32 * 0.5, comp.h as f32 * 0.5],
                position: [comp.w as f32 * 0.5, y],
                ..Transform2D::default()
            };
            let fill_start = (segment.start + (segment.end - segment.start) * 0.46)
                .min(segment.end - 1.0 / comp.fps.max(1.0));
            outline_transform.animation.opacity = vec![
                scalar_key(segment.start, 100.0),
                scalar_key(fill_start, 100.0),
                linear_scalar_key((fill_start + 0.12).min(segment.end), 0.0),
            ];
            result.layers.push(LoweredVisualLayer {
                sort_key: -1_200_000 + index as i64 * 10,
                layer: Layer::Text {
                    id: format!("bot_{source_mode}_{}_outline", segment.id),
                    start: segment.start,
                    duration: (segment.end - segment.start).max(1.0 / comp.fps.max(1.0)),
                    text: text.clone(),
                    font: "Montserrat-Bold".to_string(),
                    fontSize: font_size,
                    char_styles: char_styles.clone(),
                    blend_mode: BlendMode::Normal,
                    tracking: type_spec.tracking,
                    leading: Some(font_size * 0.92),
                    center_source_rect_y: true,
                    justification: TextJustification::Center,
                    fill: [0, 0, 0, 0],
                    box_: Some(Rect {
                        x: (comp.w as f32 - box_width) * 0.5,
                        y: (comp.h as f32 - box_height) * 0.5,
                        w: box_width,
                        h: box_height,
                    }),
                    transform: outline_transform,
                    text_animators: vec![bot_scene_reveal(
                        &segment.words,
                        segment.start,
                        fill_start,
                        comp.fps,
                        type_spec.reveal,
                    )],
                    effects: vec![EffectSpec {
                        match_name: "ANR Text Paint".to_string(),
                        params: json!(TextPaintSpec {
                            fill: Some([0, 0, 0, 0]),
                            fill_enabled: true,
                            stroke_color: Some([255, 255, 255, 255]),
                            stroke_width: 5.0,
                            stroke_over_fill: false,
                        }),
                    }],
                },
            });
        }
        if type_spec.accumulates {
            for (word_index, word) in segment.words.iter().enumerate() {
                let prefix = segment.words[..=word_index]
                    .iter()
                    .map(|item| item.word.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let next_start = segment
                    .words
                    .get(word_index + 1)
                    .map(|item| item.start)
                    .unwrap_or(segment.end);
                let mut prefix_transform = transform.clone();
                prefix_transform.animation.opacity = vec![
                    scalar_key(word.start, 0.0),
                    scalar_key(
                        (word.start + 1.0 / comp.fps.max(1.0)).min(segment.end),
                        100.0,
                    ),
                    scalar_key(next_start.max(word.start + 1.0 / comp.fps.max(1.0)), 100.0),
                    linear_scalar_key((next_start + 1.0 / comp.fps.max(1.0)).min(segment.end), 0.0),
                ];
                result.layers.push(LoweredVisualLayer {
                    sort_key: -1_200_005 + index as i64 * 100 + word_index as i64,
                    layer: Layer::Text {
                        id: format!("bot_{source_mode}_{}_accumulate_{word_index}", segment.id),
                        start: word.start,
                        duration: (segment.end - word.start).max(1.0 / comp.fps.max(1.0)),
                        text: prefix.clone(),
                        font: "Montserrat-Bold".to_string(),
                        fontSize: font_size,
                        char_styles: Vec::new(),
                        blend_mode: BlendMode::Normal,
                        tracking: type_spec.tracking,
                        leading: Some(font_size * 0.92),
                        center_source_rect_y: true,
                        justification: TextJustification::Center,
                        fill,
                        box_: Some(Rect {
                            x: (comp.w as f32 - box_width) * 0.5,
                            y: (comp.h as f32 - box_height) * 0.5,
                            w: box_width,
                            h: box_height,
                        }),
                        transform: prefix_transform,
                        text_animators: Vec::new(),
                        effects: bot_scene_effects(source_mode, "TYPE_3", fill),
                    },
                });

                if word_index + 1 == segment.words.len() {
                    let frame = 1.0 / comp.fps.max(1.0);
                    let tail_start = (segment.end - frame * 4.0).max(word.start);
                    let mut tail_transform = transform.clone();
                    tail_transform.animation.opacity = vec![
                        linear_scalar_key(tail_start, 0.0),
                        scalar_key((tail_start + frame).min(segment.end), 78.0),
                        linear_scalar_key(segment.end, 0.0),
                    ];
                    result.layers.push(LoweredVisualLayer {
                        sort_key: -1_200_004 + index as i64 * 100 + word_index as i64,
                        layer: Layer::Text {
                            id: format!("bot_{source_mode}_{}_tail", segment.id),
                            start: tail_start,
                            duration: (segment.end - tail_start).max(frame),
                            text: prefix.clone(),
                            font: "Montserrat-Bold".to_string(),
                            fontSize: font_size,
                            char_styles: Vec::new(),
                            blend_mode: BlendMode::Normal,
                            tracking: type_spec.tracking,
                            leading: Some(font_size * 0.92),
                            center_source_rect_y: true,
                            justification: TextJustification::Center,
                            fill,
                            box_: Some(Rect {
                                x: (comp.w as f32 - box_width) * 0.5,
                                y: (comp.h as f32 - box_height) * 0.5,
                                w: box_width,
                                h: box_height,
                            }),
                            transform: tail_transform,
                            text_animators: Vec::new(),
                            effects: bot_type_3_tail_effects(comp, tail_start, segment.end, fill),
                        },
                    });
                }
            }
        } else {
            result.layers.push(LoweredVisualLayer {
                sort_key: -1_199_990 + index as i64 * 10,
                layer: text_layer,
            });
        }
    }
    result.findings.push(CapabilityFinding {
        status: CapabilityStatus::Approximate,
        feature: format!("lower.visual_op.subtitle.bot.{source_mode}.v1"),
        layer: operation.id.clone(),
        detail: "bot flow is lowered from its planner payload; typography, reveal, outline/fill, focus and effect timing remain native approximations"
            .to_string(),
    });
    Some(result)
}

fn bot_subtitle_segments(operation: &VisualOperation) -> Vec<BotSubtitleSegment> {
    operation
        .params
        .get("segments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let object = value.as_object()?;
            let text = object.get("text")?.as_str()?.trim().to_string();
            let start = object.get("start")?.as_f64()?;
            let end = object.get("end")?.as_f64()?;
            if text.is_empty() || !start.is_finite() || !end.is_finite() || end <= start {
                return None;
            }
            let lines = object
                .get("lines")
                .and_then(Value::as_array)
                .map(|lines| {
                    lines
                        .iter()
                        .filter_map(|line| match line {
                            Value::String(line) => Some(line.trim().to_string()),
                            Value::Array(words) => Some(
                                words
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            ),
                            _ => None,
                        })
                        .filter(|line| !line.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let words = object
                .get("words")
                .and_then(Value::as_array)
                .map(|words| {
                    words
                        .iter()
                        .filter_map(|word| {
                            let word = word.as_object()?;
                            let word_text = word.get("word")?.as_str()?.trim().to_string();
                            let word_start = word.get("start")?.as_f64()?;
                            let word_end = word.get("end")?.as_f64()?;
                            (!word_text.is_empty() && word_end > word_start).then_some(WordTiming {
                                word: word_text,
                                start: word_start,
                                end: word_end,
                                voice: word.get("focus").and_then(Value::as_bool).unwrap_or(false),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(BotSubtitleSegment {
                id: object
                    .get("id")
                    .and_then(bot_segment_id)
                    .unwrap_or_else(|| "segment".to_string()),
                text,
                start,
                end,
                lines,
                words,
                kind: object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                focus_word: object
                    .get("focusWord")
                    .or_else(|| object.get("focus_word"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|word| !word.is_empty())
                    .map(ToOwned::to_owned),
                focus_style: object
                    .get("focusStyle")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        })
        .collect()
}

fn bot_segment_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .or_else(|| value.as_u64().map(|value| value.to_string()))
}

fn bot_font_size(text: &str, comp: &CompSpec, source_mode: &str) -> f32 {
    let glyphs = text.chars().filter(|char| *char != '\n').count().max(1) as f32;
    let mode_max: f32 = match source_mode {
        "impulse_2nd" => 124.0,
        "template_4th" => 74.0,
        "legacy_blocks" => 94.0,
        _ => 108.0,
    };
    let width = comp.w as f32 * 0.86 / (glyphs * 0.54).max(1.0);
    mode_max.min(width).max(38.0)
}

fn bot_scene_reveal(
    words: &[WordTiming],
    start: f64,
    end: f64,
    fps: f64,
    mode: ScenesRevealMode,
) -> TextAnimatorSpec {
    let frame = 1.0 / fps.max(1.0);
    let source = if words.is_empty() {
        vec![WordTiming {
            word: "segment".to_string(),
            start,
            end,
            voice: false,
        }]
    } else {
        words.to_vec()
    };
    let count = source.len().max(1) as f32;
    let mut keys = Vec::with_capacity(source.len() * 2);
    for (index, word) in source.iter().enumerate() {
        let reveal = match mode {
            ScenesRevealMode::Words => word.start,
            ScenesRevealMode::TwoGroups => {
                let split = (source.len() + 1) / 2;
                if index < split {
                    source[0].start
                } else {
                    source[split].start
                }
            }
        }
        .clamp(start, end);
        keys.push(scalar_key(reveal, index as f32 / count * 100.0));
        keys.push(scalar_key(
            (reveal + frame).min(end),
            (index + 1) as f32 / count * 100.0,
        ));
    }
    TextAnimatorSpec {
        name: match mode {
            ScenesRevealMode::Words => "scenes_word_reveal",
            ScenesRevealMode::TwoGroups => "scenes_two_group_reveal",
        }
        .to_string(),
        opacity: 0.0,
        position: None,
        scale: None,
        rotation: None,
        blur: None,
        selector: TextRangeSelector {
            start: 0.0,
            end: 100.0,
            start_keyframes: keys,
            end_keyframes: Vec::new(),
            based_on: TextSelectorBasedOn::Words,
            smoothness: 0.0,
            shape: TextSelectorShape::Square,
            randomize_order: false,
            wiggly: None,
        },
        expression_selector: None,
    }
}

fn bot_focus_char_styles(text: &str, segment: &BotSubtitleSegment) -> Vec<TextCharStyle> {
    if segment.focus_style.as_deref() != Some("italic") {
        return Vec::new();
    }
    let focus = segment.focus_word.as_deref().or_else(|| {
        segment
            .words
            .iter()
            .find(|word| word.voice)
            .map(|word| word.word.as_str())
    });
    let Some(focus) = focus else {
        return Vec::new();
    };
    let mut styles = Vec::new();
    let mut cursor = 0usize;
    for token in text.split_inclusive(|character: char| character.is_whitespace()) {
        let word = token.trim();
        let word_start = cursor;
        let word_len = word.chars().count();
        if word.eq_ignore_ascii_case(focus) {
            styles.extend(
                (word_start..word_start + word_len).map(|index| TextCharStyle {
                    index,
                    font: Some("Montserrat-BoldItalic".to_string()),
                    font_size: None,
                    fill: None,
                    faux_italic: false,
                }),
            );
            break;
        }
        cursor += token.chars().count();
    }
    styles
}

fn bot_echo_opacity_keys(segment: &BotSubtitleSegment, fps: f64) -> Vec<ScalarKeyframe> {
    let frame = 1.0 / fps.max(1.0);
    let peak = (segment.start + frame * 2.0).min(segment.end);
    let exit = (segment.end - frame * 3.0).max(peak);
    vec![
        linear_scalar_key(segment.start, 0.0),
        scalar_key(peak, 72.0),
        scalar_key(exit, 50.0),
        linear_scalar_key(segment.end, 0.0),
    ]
}

fn bot_opacity_keys(segment: &BotSubtitleSegment, fps: f64) -> Vec<ScalarKeyframe> {
    let frame = 1.0 / fps.max(1.0);
    let in_end = (segment.start + frame * 2.0).min(segment.end);
    let out_start = (segment.end - frame * 2.0).max(in_end);
    vec![
        linear_scalar_key(segment.start, 0.0),
        scalar_key(in_end, 100.0),
        scalar_key(out_start, 100.0),
        linear_scalar_key(segment.end, 0.0),
    ]
}

fn bot_scene_effects(source_mode: &str, kind: &str, fill: [u8; 4]) -> Vec<EffectSpec> {
    let mut effects = vec![EffectSpec {
        match_name: "ADBE Drop Shadow".to_string(),
        params: json!({"color": [0, 0, 0, 255], "opacity": 130.0, "direction": 135.0, "distance": 5.0, "softness": 18.0}),
    }];
    if kind == "TYPE_4" || source_mode == "impulse_2nd" {
        effects.push(EffectSpec {
            match_name: "ADBE Glo2".to_string(),
            params: json!({"threshold": 90.0, "radius": 34.0, "intensity": 0.75, "operation": "add", "color": fill}),
        });
    }
    if matches!(kind, "TYPE_1" | "TYPE_2" | "TYPE_4" | "TYPE_5" | "TYPE_6") {
        effects.push(EffectSpec {
            match_name: "ADBE Minimax".to_string(),
            params: json!({"operation": "maximum", "channels": "alpha_and_color", "radius": 4.0}),
        });
    }
    if matches!(kind, "TYPE_2" | "TYPE_4" | "TYPE_5" | "TYPE_6") {
        effects.push(EffectSpec {
            match_name: "ADBE Posterize Time".to_string(),
            params: json!({"frameRate": 5.0}),
        });
        effects.push(EffectSpec {
            match_name: "ADBE Turbulent Displace".to_string(),
            params: json!({"amount": 7.5, "size": 50.0, "evolution": 180.0, "complexity": 1.0}),
        });
    }
    if kind == "TYPE_3" {
        effects.push(EffectSpec {
            match_name: "ADBE Geometry2".to_string(),
            params: json!({"scale_width": 100.0, "scale_height": 92.0, "anchor": [540.0, 960.0]}),
        });
        effects.push(EffectSpec {
            match_name: "ADBE Box Blur2".to_string(),
            params: json!({"radius": 2.5, "iterations": 1.0}),
        });
    }
    if kind == "TYPE_5" {
        effects.push(EffectSpec {
            match_name: "ADBE Box Blur2".to_string(),
            params: json!({"radius": 1.5, "iterations": 1.0}),
        });
    }
    effects
}

fn bot_type_3_tail_effects(
    comp: &CompSpec,
    start: f64,
    end: f64,
    fill: [u8; 4],
) -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            match_name: "ADBE Drop Shadow".to_string(),
            params: json!({"color": [0, 0, 0, 255], "opacity": 110.0, "direction": 135.0, "distance": 5.0, "softness": 18.0}),
        },
        EffectSpec {
            match_name: "ADBE Geometry2".to_string(),
            params: json!({
                "scale_width": 100.0,
                "scale_height": {"keyframes": [
                    {"t": start, "v": 92.0},
                    {"t": end, "v": 146.0}
                ]},
                "anchor": [comp.w as f32 * 0.5, comp.h as f32 * 0.5]
            }),
        },
        EffectSpec {
            match_name: "ADBE Box Blur2".to_string(),
            params: json!({
                "radius": {"keyframes": [
                    {"t": start, "v": 2.5},
                    {"t": end, "v": 25.0}
                ]},
                "iterations": 1.0
            }),
        },
        EffectSpec {
            match_name: "ADBE Glo2".to_string(),
            params: json!({"threshold": 130.0, "radius": 18.0, "intensity": 0.28, "operation": "add", "color": fill}),
        },
    ]
}

fn lower_semantic_style(
    operation: &VisualOperation,
    comp: &CompSpec,
    payload: &GeneratedPayload,
) -> Option<VisualLoweringResult> {
    let style = operation_string_param(operation, &["styleId", "style_id"])?;
    let request_recipe = payload.style_registry.iter().find(|entry| {
        entry.style_id.as_deref().or(entry.stable_id.as_deref()) == Some(style)
            && !entry.effect_graph.is_empty()
    });
    let effects = request_recipe
        .map(|entry| entry.effect_graph.clone())
        .or_else(|| StyleRegistry::effects(style));
    let Some(effects) = effects else {
        return Some(VisualLoweringResult {
            layers: Vec::new(),
            findings: vec![CapabilityFinding {
                status: CapabilityStatus::NotImplemented,
                feature: format!("lower.visual_op.style.semantic.v1.{style}"),
                layer: operation.id.clone(),
                detail: "style id has no effectGraph in the request or built-in recipe catalog"
                    .to_string(),
            }],
        });
    };
    let effect_names = effects
        .iter()
        .map(|effect| effect.match_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Some(VisualLoweringResult {
        layers: vec![LoweredVisualLayer {
            sort_key: -500_100,
            layer: Layer::Adjustment {
                id: format!("bot_style_{style}"),
                start: 0.0,
                duration: comp.dur,
                effects,
            },
        }],
        findings: vec![CapabilityFinding {
            status: CapabilityStatus::Approximate,
            feature: format!("lower.visual_op.style.semantic.v1.{style}"),
            layer: operation.id.clone(),
            detail: format!(
                "semantic AE/plugin stack is represented by a deterministic native approximation: {}",
                effect_names
            ),
        }],
    })
}

#[derive(Debug)]
struct BratBlock {
    words: Vec<WordTiming>,
    line_sizes: Vec<usize>,
}

impl BratBlock {
    fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut offset = 0usize;
        for count in &self.line_sizes {
            let line = self.words[offset..offset + *count]
                .iter()
                .map(|word| word.word.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(line);
            offset += *count;
        }
        lines
    }
}

fn brat_blocks(words: &[WordTiming], words_per_line: usize, max_lines: usize) -> Vec<BratBlock> {
    let mut runs: Vec<Vec<WordTiming>> = Vec::new();
    for word in words {
        if runs
            .last()
            .and_then(|run| run.last())
            .is_none_or(|previous| previous.voice != word.voice)
        {
            runs.push(Vec::new());
        }
        runs.last_mut().expect("run was created").push(word.clone());
    }

    let per_block = words_per_line * max_lines;
    let mut blocks = Vec::new();
    for run in runs {
        for chunk in run.chunks(per_block) {
            let mut line_sizes = chunk
                .chunks(words_per_line)
                .map(<[WordTiming]>::len)
                .collect::<Vec<_>>();
            if line_sizes.len() > 1 && line_sizes.last() == Some(&1) {
                line_sizes.pop();
                if let Some(previous) = line_sizes.last_mut() {
                    *previous += 1;
                }
            }
            blocks.push(BratBlock {
                words: chunk.to_vec(),
                line_sizes,
            });
        }
    }
    blocks
}

fn operation_words(operation: &VisualOperation) -> Vec<WordTiming> {
    operation
        .params
        .get("word_timings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|word| {
            let text = word
                .get("word")
                .or_else(|| word.get("text"))
                .and_then(Value::as_str)?
                .trim()
                .to_string();
            let start = word
                .get("start")
                .or_else(|| word.get("t_start"))
                .and_then(Value::as_f64)?;
            let end = word
                .get("end")
                .or_else(|| word.get("t_end"))
                .and_then(Value::as_f64)
                .unwrap_or(start + 0.1);
            (!text.is_empty() && start.is_finite() && end.is_finite()).then_some(WordTiming {
                word: text,
                start,
                end: end.max(start + 0.001),
                voice: word.get("voice").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

fn operation_fill(operation: &VisualOperation, fallback: [u8; 4]) -> [u8; 4] {
    let Some(values) = operation.params.get("fill").and_then(Value::as_array) else {
        return fallback;
    };
    if values.len() < 3 {
        return fallback;
    }
    let channel = |index: usize, fallback: u8| {
        values
            .get(index)
            .and_then(Value::as_f64)
            .map(|value| {
                let scaled = if value <= 1.0 { value * 255.0 } else { value };
                scaled.round().clamp(0.0, 255.0) as u8
            })
            .unwrap_or(fallback)
    };
    [
        channel(0, fallback[0]),
        channel(1, fallback[1]),
        channel(2, fallback[2]),
        channel(3, fallback[3]),
    ]
}

fn brat_completion_keyframes(span_start: f64, span_end: f64, bpm: f64) -> Vec<ScalarKeyframe> {
    if !span_start.is_finite() || !span_end.is_finite() || span_end <= span_start {
        return Vec::new();
    }

    let period = 30.0 / bpm.max(1.0);
    let mut boundary = (span_start / period).floor() * period;
    let mut keys = Vec::new();
    keys.push(brat_completion_key(span_start, 0.0));

    let mut guard = 0usize;
    while boundary < span_end && guard < 100_000 {
        let midpoint = boundary + period * 0.5;
        if boundary > span_start && boundary < span_end {
            keys.push(brat_completion_key(boundary, 0.0));
        }
        if midpoint > span_start && midpoint < span_end {
            keys.push(brat_completion_key(midpoint, BRAT_BLINK_PEAK));
        }
        boundary += period;
        guard += 1;
    }
    keys.push(brat_completion_key(span_end, 0.0));
    keys
}

fn brat_completion_key(time: f64, value: f32) -> ScalarKeyframe {
    ScalarKeyframe {
        time,
        value,
        hold: false,
        approximate: false,
        ease: Some(BRAT_BLINK_EASE),
    }
}

fn scalar_key(time: f64, value: f32) -> ScalarKeyframe {
    ScalarKeyframe {
        time,
        value,
        hold: false,
        approximate: true,
        ease: Some(KeyframeEase {
            x1: 0.333,
            y1: 0.0,
            x2: 0.667,
            y2: 1.0,
        }),
    }
}

fn linear_scalar_key(time: f64, value: f32) -> ScalarKeyframe {
    ScalarKeyframe {
        time,
        value,
        hold: false,
        approximate: false,
        ease: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PayloadLayer, ProjectSpec, VisualOperationTarget, VisualOperationTiming};
    use std::collections::BTreeMap;

    fn comp() -> CompSpec {
        CompSpec {
            name: "Comp 1".to_string(),
            w: 1080,
            h: 1920,
            fps: 24.0,
            dur: 10.0,
            pixel_aspect: None,
            work_area_start: None,
            work_area_duration: None,
            display_start_time: None,
            bg_color: None,
            extra: std::collections::BTreeMap::new(),
        }
    }

    fn payload(kind: &str, words: Value) -> GeneratedPayload {
        GeneratedPayload {
            schema_version: Some("render-plan.v1.1".to_string()),
            payload_version: None,
            project_spec: ProjectSpec {
                main_comp_name: "Comp 1".to_string(),
                subtitles_mode: None,
                extra: std::collections::BTreeMap::new(),
            },
            comps_spec: vec![comp()],
            footage_layers: Vec::new(),
            text_layers: Vec::new(),
            visual_ops: vec![VisualOperation {
                id: Some("subs".to_string()),
                kind: kind.to_string(),
                target: VisualOperationTarget::default(),
                timing: VisualOperationTiming::default(),
                params: json!({"word_timings": words}),
                assets: Vec::new(),
                required: true,
            }],
            requirements: Value::Null,
            style_registry: Vec::new(),
            effect_registry: Vec::new(),
            golden_refs: Vec::new(),
        }
    }

    fn layer(name: &str, kind: &str, start: f64, text_data: Value) -> PayloadLayer {
        PayloadLayer {
            name: name.to_string(),
            kind: kind.to_string(),
            in_point: start,
            out_point: start + 1.0,
            z_index: 100,
            text: String::new(),
            adjustment_layer: false,
            props: BTreeMap::new(),
            effects: BTreeMap::new(),
            text_data,
            source_rect: json!({}),
            extra: BTreeMap::new(),
        }
    }

    fn scenes_payload(segment: Value) -> GeneratedPayload {
        let mut result = payload("subtitle.bot.scenes_3rd.v1", json!([]));
        result.visual_ops[0].params = json!({"segments": [segment]});
        result
    }

    #[test]
    fn type_2_applies_italic_only_to_the_focus_word() {
        let source = scenes_payload(json!({
                "id": "type-2", "type": "TYPE_2", "text": "я снова\nвижу тебя",
            "start": 0.2, "end": 1.7, "focusWord": "снова", "focusStyle": "italic",
            "words": [
                {"word":"я", "start":0.2, "end":0.5},
                {"word":"снова", "start":0.5, "end":0.9},
                {"word":"вижу", "start":0.9, "end":1.2},
                {"word":"тебя", "start":1.2, "end":1.7}
            ]
        }));
        let segments = bot_subtitle_segments(&source.visual_ops[0]);
        assert_eq!(segments[0].focus_word.as_deref(), Some("снова"));
        assert_eq!(segments[0].focus_style.as_deref(), Some("italic"));
        assert_eq!(
            bot_focus_char_styles("я снова\nвижу тебя", &segments[0]).len(),
            "снова".chars().count()
        );
        let result = lower_visual_operations(&source, &comp());
        let Layer::Text {
            font,
            char_styles,
            effects,
            ..
        } = &result.layers[0].layer
        else {
            panic!("expected type 2 fill text");
        };
        assert_eq!(font, "Montserrat-Bold");
        assert_eq!(char_styles.len(), "снова".chars().count());
        assert!(char_styles
            .iter()
            .all(|style| style.font.as_deref() == Some("Montserrat-BoldItalic")));
        assert!(effects
            .iter()
            .any(|effect| effect.match_name == "ADBE Posterize Time"));
        assert!(effects
            .iter()
            .any(|effect| effect.match_name == "ADBE Turbulent Displace"));
    }

    #[test]
    fn type_3_uses_prefix_layers_instead_of_a_generic_word_reveal() {
        let result = lower_visual_operations(
            &scenes_payload(json!({
                "id": "type-3", "type": "TYPE_3", "text": "ты оставил след",
                "start": 0.2, "end": 1.7,
                "words": [
                    {"word":"ты", "start":0.2, "end":0.55},
                    {"word":"оставил", "start":0.55, "end":1.0},
                    {"word":"след", "start":1.0, "end":1.7}
                ]
            })),
            &comp(),
        );
        assert_eq!(result.layers.len(), 4);
        assert!(result
            .layers
            .iter()
            .filter(|layer| !layer.layer.id().ends_with("_tail"))
            .all(|layer| layer.layer.id().contains("accumulate_")));
        let tail = result
            .layers
            .iter()
            .find(|layer| layer.layer.id().ends_with("_tail"))
            .expect("TYPE_3 has a separate exit tail");
        let Layer::Text { effects, .. } = &tail.layer else {
            panic!("expected text tail");
        };
        assert!(effects
            .iter()
            .any(|effect| effect.match_name == "ADBE Box Blur2"));
    }

    #[test]
    fn type_4_builds_a_blurred_echo_below_the_red_fill() {
        let result = lower_visual_operations(
            &scenes_payload(json!({
                "id": "type-4", "type": "TYPE_4", "text": "ангел возьми",
                "start": 0.2, "end": 1.5, "focusStyle": "red",
                "words": [{"word":"ангел", "start":0.2, "end":0.72}, {"word":"возьми", "start":0.72, "end":1.5}]
            })),
            &comp(),
        );
        assert_eq!(result.layers.len(), 2);
        let echo = result
            .layers
            .iter()
            .find(|layer| layer.layer.id().ends_with("_echo"))
            .unwrap();
        let Layer::Text { fill, effects, .. } = &echo.layer else {
            panic!("expected text echo");
        };
        assert_eq!(*fill, [238, 32, 30, 170]);
        assert_eq!(effects[0].match_name, "ADBE Gaussian Blur 2");
    }

    #[test]
    fn type_5_keeps_hollow_outline_and_type_6_reveals_two_groups() {
        let type_5 = lower_visual_operations(
            &scenes_payload(json!({
                "id": "type-5", "type": "TYPE_5", "text": "делятся ночью\nсо мной", "start": 0.2, "end": 3.2,
                "words": [{"word":"делятся", "start":0.2, "end":0.8}, {"word":"ночью", "start":0.8, "end":1.4}, {"word":"со", "start":1.4, "end":2.0}, {"word":"мной", "start":2.0, "end":2.8}]
            })),
            &comp(),
        );
        let outline = type_5
            .layers
            .iter()
            .find(|layer| layer.layer.id().ends_with("_outline"))
            .unwrap();
        let Layer::Text { fill, effects, .. } = &outline.layer else {
            panic!("expected outline text");
        };
        assert_eq!(*fill, [0, 0, 0, 0]);
        assert_eq!(effects[0].match_name, "ANR Text Paint");

        let type_6 = lower_visual_operations(
            &scenes_payload(json!({
                "id": "type-6", "type": "TYPE_6", "text": "но я въебал\nкаждый профиль", "start": 0.2, "end": 2.0,
                "words": [{"word":"но", "start":0.2, "end":0.48}, {"word":"я", "start":0.48, "end":0.75}, {"word":"въебал", "start":0.75, "end":1.12}, {"word":"каждый", "start":1.12, "end":1.52}, {"word":"профиль", "start":1.52, "end":2.0}]
            })),
            &comp(),
        );
        let Layer::Text { text_animators, .. } = &type_6.layers[0].layer else {
            panic!("expected type 6 text");
        };
        assert_eq!(text_animators[0].name, "scenes_two_group_reveal");
    }

    #[test]
    fn brat_tail_word_merges_into_previous_line() {
        let words = (0..5)
            .map(|index| WordTiming {
                word: format!("w{index}"),
                start: index as f64,
                end: index as f64 + 0.5,
                voice: false,
            })
            .collect::<Vec<_>>();
        let blocks = brat_blocks(&words, 2, 4);
        assert_eq!(blocks[0].line_sizes, vec![2, 3]);
    }

    #[test]
    fn trendy_lowers_to_one_painted_tracking_layer_per_word() {
        let payload = payload(
            "subtitle.trendy.v1",
            json!([
                {"word":"one","start":0.0,"end":0.4},
                {"word":"two","start":0.5,"end":0.9}
            ]),
        );
        let result = lower_visual_operations(&payload, &comp());
        assert_eq!(result.layers.len(), 2);
        for lowered in &result.layers {
            let Layer::Text {
                id,
                blend_mode,
                tracking,
                leading,
                center_source_rect_y,
                justification,
                text_animators,
                effects,
                ..
            } = &lowered.layer
            else {
                panic!("expected text layer");
            };
            assert_eq!(*blend_mode, BlendMode::Normal);
            assert_eq!(*tracking, -55.0);
            assert_eq!(*leading, None);
            assert!(!center_source_rect_y);
            assert_eq!(*justification, TextJustification::Center);
            assert!(id.starts_with("visual_trendy_"));
            assert_eq!(text_animators.len(), 1);
            let Some(TextExpressionSelector::TrackingAmount { value, keyframes }) =
                text_animators[0].expression_selector.as_ref()
            else {
                panic!("expected tracking amount animator");
            };
            assert_eq!(*value, 7.0);
            assert_eq!(keyframes.len(), 2);
            assert_eq!(keyframes[0].value, 7.0);
            assert_eq!(keyframes[1].value, -1.0);
            assert_eq!(keyframes[0].ease.expect("tracking ease").x1, 0.333_333_33);

            let paint = effects
                .iter()
                .find_map(TextPaintSpec::from_effect)
                .expect("unified text paint");
            assert_eq!(paint.stroke_color, Some([0, 0, 0, 255]));
            assert_eq!(paint.stroke_width, 5.0);
            assert!(!paint.stroke_over_fill);
            assert!(!effects
                .iter()
                .any(|effect| effect.match_name == "ADBE Minimax"));

            let gradient = effects
                .iter()
                .find(|effect| effect.match_name == "ANR Vertical Gradient")
                .expect("native gradient approximation");
            assert_eq!(gradient.params["brightness"], json!(1.26));
            assert_eq!(
                gradient.params["sapphire_params"]["0050"],
                json!([540.0, 772.0])
            );
            let shadow = effects
                .iter()
                .find(|effect| effect.match_name == "ADBE Drop Shadow")
                .expect("native shadow approximation");
            assert_eq!(shadow.params["sapphire_params"]["0051"], json!(1.14));
            assert_eq!(shadow.params["sapphire_params"]["0052"], json!(100.0));
        }
        assert!(!result.findings.iter().any(|finding| {
            finding.feature == "lower.visual_op.subtitle.trendy.v1.tracking_animator"
        }));
    }

    #[test]
    fn brat_lowers_to_word_reveal_blocks() {
        let payload = payload(
            "subtitle.brat.v1",
            json!([
                {"word":"one","start":0.0,"end":0.4},
                {"word":"two","start":0.5,"end":0.9}
            ]),
        );
        let result = lower_visual_operations(&payload, &comp());
        assert_eq!(result.layers.len(), 1);
        let Layer::Text {
            fontSize,
            blend_mode,
            tracking,
            leading,
            center_source_rect_y,
            justification,
            box_,
            transform,
            text,
            text_animators,
            effects,
            ..
        } = &result.layers[0].layer
        else {
            panic!("expected text layer");
        };
        assert_eq!(*fontSize, 130.0);
        assert_eq!(*blend_mode, BlendMode::Difference);
        assert_eq!(*tracking, -20.0);
        assert_eq!(*leading, Some(*fontSize));
        assert!(*center_source_rect_y);
        assert_eq!(*justification, TextJustification::Full);
        assert_eq!(text, "one two");
        assert_eq!(transform.scale, [80.0, 80.0]);
        let effective_leading = leading.unwrap() * transform.scale[1] / 100.0;
        let effective_tracking = tracking / 1000.0 * fontSize * transform.scale[0] / 100.0;
        assert!((effective_leading - 104.0).abs() < 1e-6);
        assert!((effective_tracking - -2.08).abs() < 1e-6);
        assert!((box_.as_ref().unwrap().w * transform.scale[0] / 100.0 - 691.2).abs() < 1e-4);
        assert_eq!(
            text_animators[0].selector.based_on as u8,
            TextSelectorBasedOn::Words as u8
        );
        assert_eq!(effects[0].params["opacity"], json!(140.25));
        assert_eq!(effects[1].params["channels"], json!("alpha_and_color"));
        assert_eq!(effects[2].match_name, "ADBE Gaussian Blur 2");
        assert_eq!(effects[2].params["blurriness"], json!(10.0));
        assert_eq!(effects[3].match_name, "CC Image Wipe");
        assert_eq!(
            effects[3].params["border_softness"],
            json!(BRAT_BLINK_SOFTNESS)
        );
    }

    #[test]
    fn brat_production_bpm_generates_144_global_smoothstep_keys_per_block() {
        let words = (0..26)
            .map(|index| {
                let start = 0.511 + index as f64 * 12.0 / 25.0;
                let end = if index == 25 { 13.311 } else { start + 0.3 };
                json!({"word": format!("w{index}"), "start": start, "end": end})
            })
            .collect::<Vec<_>>();
        let mut payload = payload("subtitle.brat.v1", Value::Array(words));
        payload.visual_ops[0].params["bpm"] = json!(166.71);

        let result = lower_visual_operations(&payload, &comp());
        assert_eq!(result.layers.len(), 4);
        for lowered in &result.layers {
            let Layer::Text {
                blend_mode,
                effects,
                ..
            } = &lowered.layer
            else {
                panic!("expected text layer");
            };
            assert_eq!(*blend_mode, BlendMode::Difference);
            assert_eq!(effects[3].match_name, "CC Image Wipe");
            assert_eq!(
                effects[3].params["completion"]["keyframes"]
                    .as_array()
                    .unwrap()
                    .len(),
                144
            );
        }

        let Layer::Text { effects, .. } = &result.layers[0].layer else {
            panic!("expected text layer");
        };
        let keys: Vec<ScalarKeyframe> =
            serde_json::from_value(effects[3].params["completion"]["keyframes"].clone()).unwrap();
        assert_eq!(keys.first().unwrap().time, 0.511);
        assert_eq!(keys.last().unwrap().time, 13.311);
        assert_eq!(keys[1].value, 0.0);
        assert_eq!(keys[2].value, BRAT_BLINK_PEAK);
        for key in &keys {
            let ease = key.ease.unwrap();
            assert_eq!(ease.x1, 1.0 / 3.0);
            assert_eq!(ease.y1, 0.0);
            assert_eq!(ease.x2, 2.0 / 3.0);
            assert_eq!(ease.y2, 1.0);
        }

        let expected = [
            (14, 0.193_790_8),
            (15, 0.3974533),
            (17, 0.01582441),
            (19, 0.3611951),
        ];
        for (frame, expected) in expected {
            let time = frame as f64 * 1_001.0 / 24_000.0;
            let sampled = sample_smoothstep_keys(&keys, time);
            assert!(
                (sampled - expected).abs() < 1.0e-5,
                "frame {frame}: expected {expected}, got {sampled}"
            );
        }
    }

    #[test]
    fn brat_keeps_one_difference_layer_per_block_across_strobes() {
        let mut payload = payload(
            "subtitle.brat.v1",
            json!([
                {"word":"one","start":0.0,"end":0.4},
                {"word":"two","start":0.5,"end":0.9}
            ]),
        );
        payload.visual_ops[0].params["fill"] = json!([10, 20, 30, 255]);
        payload.footage_layers = vec![
            layer(
                "strobe_bg_0",
                "solid",
                0.0,
                json!({"solid_source":{"color_rgb01":[1.0, 1.0, 1.0]}}),
            ),
            layer(
                "strobe_bg_1",
                "solid",
                0.5,
                json!({"solid_source":{"color_rgb01":[0.0, 0.0, 0.0]}}),
            ),
        ];

        let result = lower_visual_operations(&payload, &comp());

        assert_eq!(result.layers.len(), 1);
        let Layer::Text {
            id,
            blend_mode,
            fill,
            ..
        } = &result.layers[0].layer
        else {
            panic!("expected text layer");
        };
        assert_eq!(id, "visual_brat_0000");
        assert_eq!(*blend_mode, BlendMode::Difference);
        assert_eq!(*fill, [10, 20, 30, 255]);
    }

    #[test]
    fn brat_single_word_block_uses_left_justification() {
        let payload = payload(
            "subtitle.brat.v1",
            json!([{"word":"one","start":0.0,"end":0.4}]),
        );
        let result = lower_visual_operations(&payload, &comp());
        let Layer::Text { justification, .. } = &result.layers[0].layer else {
            panic!("expected text layer");
        };

        assert_eq!(*justification, TextJustification::Left);
    }

    #[test]
    fn flash_on_cuts_uses_video_and_strobe_boundaries() {
        let mut payload = payload("hook.f3.effect.v1", json!([]));
        payload.visual_ops[0].params = json!({"detected_effect_ids":["flash_on_cuts"]});
        payload.footage_layers = vec![
            layer(
                "clip.mp4",
                "footage",
                1.0,
                json!({"source_footage":{"file_name":"clip.mp4"}}),
            ),
            layer(
                "near_duplicate.mp4",
                "footage",
                1.01,
                json!({"source_footage":{"file_name":"near_duplicate.mp4"}}),
            ),
            layer(
                "audio",
                "footage",
                2.0,
                json!({"layer_meta":{"audioEnabled":true},"source_footage":{"file_name":"audio.mp3"}}),
            ),
            layer("strobe_bg_0", "solid", 3.0, json!({})),
        ];

        let result = lower_visual_operations(&payload, &comp());
        assert_eq!(result.layers.len(), 2);
        let Layer::Solid {
            start,
            duration,
            blend_mode,
            transform,
            ..
        } = &result.layers[0].layer
        else {
            panic!("expected flash solid");
        };
        assert_eq!(*start, 1.0);
        assert!((*duration - 0.633).abs() < 1e-9);
        assert_eq!(*blend_mode, BlendMode::Add);
        assert_eq!(transform.opacity, 40.0);
        assert_eq!(transform.animation.opacity[1].value, 0.0);
    }

    #[test]
    fn raw_impulse_jsx_reconstructs_flash_without_visual_ops() {
        let mut payload = payload("unused", json!([]));
        payload.visual_ops.clear();
        payload.project_spec.subtitles_mode = Some("impulse_2nd".to_string());
        payload.footage_layers = vec![layer(
            "clip.mp4",
            "footage",
            1.0,
            json!({"source_footage":{"file_name":"clip.mp4"}}),
        )];

        let result = lower_visual_operations(&payload, &comp());
        assert_eq!(result.layers.len(), 1);
        assert_eq!(result.layers[0].sort_key, 500_000);
        assert!(matches!(result.layers[0].layer, Layer::Solid { .. }));
        assert!(result
            .findings
            .iter()
            .any(|finding| finding.feature == "lower.implicit_impulse_flash_on_cuts"));
    }

    #[test]
    fn flash_on_cut_clamps_fade_key_to_clipped_layer_tail() {
        let mut tail_comp = comp();
        tail_comp.fps = 30.0;
        tail_comp.dur = 1.4;
        let mut payload = payload("hook.f3.effect.v1", json!([]));
        payload.visual_ops[0].params = json!({"detected_effect_ids":["flash_on_cuts"]});
        payload.footage_layers = vec![layer(
            "tail.mp4",
            "footage",
            1.2,
            json!({"source_footage":{"file_name":"tail.mp4"}}),
        )];

        let result = lower_visual_operations(&payload, &tail_comp);
        let Layer::Solid {
            start,
            duration,
            transform,
            ..
        } = &result.layers[0].layer
        else {
            panic!("expected flash solid");
        };
        assert!((*duration - 0.2).abs() < 1e-9);
        assert!((transform.animation.opacity[1].time - (*start + *duration)).abs() < 1e-9);
        assert!((transform.animation.opacity[1].time - tail_comp.dur).abs() < 1e-9);
    }

    #[test]
    fn analog_glitch_lowers_as_adjustment_below_subtitles() {
        let mut payload = payload("hook.f3.effect.v1", json!([]));
        payload.visual_ops[0].params = json!({"detected_effect_ids":["analog_glitch"]});

        let result = lower_visual_operations(&payload, &comp());
        assert_eq!(result.layers.len(), 1);
        assert_eq!(result.layers[0].sort_key, -500_300);
        let Layer::Adjustment { effects, .. } = &result.layers[0].layer else {
            panic!("expected analog glitch adjustment");
        };
        assert_eq!(effects[0].match_name, "ADBE Posterize Time");
        assert_eq!(effects[1].match_name, "ANR Analog Glitch");
        assert_eq!(effects[1].params["contrast"], json!(1.3));
        assert_eq!(effects[1].params["red_gain"], json!(1.25));
        assert_eq!(effects[2].match_name, "ADBE Glo2");
        assert_eq!(effects[3].match_name, "ADBE Glo2");
    }

    #[test]
    fn shutter_and_snap_wipe_lower_the_native_f3_subset() {
        let mut payload = payload("hook.f3.effect.v1", json!([]));
        payload.visual_ops[0].params = json!({
            "detected_effect_ids":["shutter_effect", "snap_wipe"],
            "drop_time": 1.0
        });
        payload.visual_ops[0].timing.start = Some(1.0);
        payload.footage_layers = vec![
            layer(
                "first.mp4",
                "footage",
                0.0,
                json!({"source_footage":{"file_name":"first.mp4"}}),
            ),
            layer(
                "second.mp4",
                "footage",
                2.0,
                json!({"source_footage":{"file_name":"second.mp4"}}),
            ),
            layer(
                "audio",
                "footage",
                3.0,
                json!({"layer_meta":{"audioEnabled":true},"source_footage":{"file_name":"audio.mp3"}}),
            ),
        ];

        let result = lower_visual_operations(&payload, &comp());
        assert_eq!(result.layers.len(), 7);

        let shutter = result
            .layers
            .iter()
            .find(|layer| layer.layer.id() == "visual_f3_shutter_00")
            .unwrap();
        let Layer::Adjustment {
            start,
            duration,
            effects,
            ..
        } = &shutter.layer
        else {
            panic!("expected shutter adjustment");
        };
        assert_eq!(*start, 1.0);
        assert!((*duration - 0.100_100_100_100_1).abs() < 1e-9);
        assert_eq!(effects[0].match_name, "ADBE Invert");
        assert_eq!(effects[0].params["channel"], json!(8));
        assert_eq!(effects[1].match_name, "ADBE Motion Blur");
        assert_eq!(
            effects[1].params["direction"]["keyframes"][0]["value"],
            json!(90.0)
        );

        let snap = result
            .layers
            .iter()
            .find(|layer| layer.layer.id() == "visual_f3_snap_wipe_0001")
            .unwrap();
        let Layer::Adjustment { start, effects, .. } = &snap.layer else {
            panic!("expected snap adjustment");
        };
        assert_eq!(*start, 2.0);
        assert_eq!(effects[0].match_name, "ADBE Geometry2");
        assert_eq!(
            effects[0].params["anchor"]["keyframes"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(effects[1].match_name, "ADBE Motion Blur");
        assert_eq!(effects[2].match_name, "ADBE Minimax");
        assert_eq!(effects[2].params["radius"], json!(165.0));
        assert_eq!(effects[3].match_name, "ADBE Optics Compensation");
        assert_eq!(effects[3].params["field_of_view"], json!(120.0));
        assert!(result.findings.iter().any(|finding| {
            finding.feature.ends_with("shutter_effect")
                && finding.status == CapabilityStatus::Supported
        }));
        assert!(result.findings.iter().any(|finding| {
            finding.feature.ends_with("snap_wipe") && finding.status == CapabilityStatus::Supported
        }));
    }

    #[test]
    fn direct_f3_fields_lower_remaining_active_palette() {
        let mut payload = payload("hook.f3.effect.v1", json!([]));
        payload.visual_ops[0].params = json!({
            "hook":["hook_light", "flash_slow_shutter", "negative_zoom"],
            "transition":["invert_flash", "extract_flash", "minimax", "layer_shake"],
            "extra":["xerox", "neon_extract", "old_camera", "blackwhite"],
            "drop_time": 1.0
        });
        payload.footage_layers = vec![
            layer(
                "first.mp4",
                "footage",
                0.0,
                json!({"source_footage":{"file_name":"first.mp4"}}),
            ),
            layer(
                "second.mp4",
                "footage",
                2.0,
                json!({"source_footage":{"file_name":"second.mp4"}}),
            ),
        ];

        let result = lower_visual_operations(&payload, &comp());
        for expected in [
            "visual_f3_hook_light_0000",
            "visual_f3_flash_slow_shutter_0000",
            "visual_f3_negative_zoom_0000",
            "visual_f3_invert_flash_0000",
            "visual_f3_extract_flash_0000",
            "visual_f3_minimax_0000",
            "visual_f3_layer_shake_0000",
            "visual_f3_xerox_0000",
            "visual_f3_neon_extract_0000",
            "visual_f3_old_camera_0000",
            "visual_f3_blackwhite_0000",
        ] {
            assert!(
                result
                    .layers
                    .iter()
                    .any(|layer| layer.layer.id() == expected),
                "missing {expected}"
            );
        }
    }

    #[test]
    fn persistent_f3_extras_start_at_the_composition_boundary() {
        let mut payload = payload("hook.f3.effect.v1", json!([]));
        payload.visual_ops[0].params = json!({
            "detected_effect_ids":["blackwhite"],
            "drop_time": 12.0
        });
        payload.visual_ops[0].timing.start = Some(12.0);

        let result = lower_visual_operations(&payload, &comp());
        let Layer::Adjustment {
            start,
            duration,
            effects,
            ..
        } = &result.layers[0].layer
        else {
            panic!("expected blackwhite adjustment");
        };
        assert_eq!(*start, 0.0);
        assert_eq!(*duration, comp().dur);
        assert_eq!(effects[0].match_name, "ANR F3 Stylize");
        assert_eq!(effects[0].params["magentas"], json!(-100.0));
    }

    #[test]
    fn crystal_glow_keeps_the_production_blur_then_two_glow_passes() {
        let mut payload = payload("hook.f3.effect.v1", json!([]));
        payload.visual_ops[0].params = json!({"detected_effect_ids":["crystal_glow"]});
        let result = lower_visual_operations(&payload, &comp());
        let Layer::Adjustment { effects, .. } = &result.layers[0].layer else {
            panic!("expected crystal glow adjustment");
        };
        assert_eq!(effects.len(), 3);
        assert_eq!(effects[0].match_name, "ADBE Gaussian Blur 2");
        assert_eq!(effects[1].match_name, "ADBE Glo2");
        assert_eq!(effects[2].params["radius"], json!(96.0));
    }

    #[test]
    fn f1_f2_f4_and_f5_have_native_visual_lowerings() {
        let mut f1 = payload("hook.f1.sound.v1", json!([]));
        f1.visual_ops[0].timing.start = Some(1.0);
        assert!(lower_visual_operations(&f1, &comp())
            .layers
            .iter()
            .any(|layer| layer.layer.id() == "visual_f1_hook_light"));

        let mut f2 = payload("hook.f2.object.v1", json!([]));
        f2.visual_ops[0].params = json!({"shape":"star2", "drop_time":2.0});
        let f2_result = lower_visual_operations(&f2, &comp());
        let Layer::Solid { effects, .. } = &f2_result.layers[0].layer else {
            panic!("expected F2 shape solid");
        };
        assert_eq!(effects[0].match_name, "ANR Shape Overlay");
        assert_eq!(effects[0].params["shape"], json!("star2"));

        let mut f4 = payload("hook.f4.motion.v1", json!([]));
        f4.visual_ops[0].params = json!({"device":"swipe", "drop_time":1.0});
        let f4_result = lower_visual_operations(&f4, &comp());
        assert_eq!(f4_result.layers[0].layer.id(), "visual_f4_swipe");

        let mut f5 = payload(
            "hook.f5.cognition.v1",
            json!([{"word":"answer","start":1.0,"end":1.5}]),
        );
        f5.visual_ops[0].timing.start = Some(1.0);
        let f5_result = lower_visual_operations(&f5, &comp());
        assert!(f5_result
            .layers
            .iter()
            .any(|layer| layer.layer.id() == "visual_f5_hook_light"));
        assert!(f5_result
            .layers
            .iter()
            .any(|layer| layer.layer.id().starts_with("visual_f5_visual_trendy_")));
    }

    #[test]
    fn active_f2_f3_and_f4_palette_ids_all_lower_to_native_layers() {
        for (shape, lowered_shape) in [
            ("square", "square"),
            ("ellipse", "ellipse"),
            ("elipse", "ellipse"),
            ("rhomb", "rhomb"),
            ("star1", "star1"),
            ("star2", "star2"),
        ] {
            let mut payload = payload("hook.f2.object.v1", json!([]));
            payload.visual_ops[0].params = json!({"shape": shape, "drop_time": 1.0});
            let result = lower_visual_operations(&payload, &comp());
            let shape_layer = result
                .layers
                .iter()
                .find(|layer| layer.layer.id().starts_with("visual_f2_shape_"))
                .unwrap_or_else(|| panic!("missing native F2 shape layer for {shape}"));
            let Layer::Solid { effects, .. } = &shape_layer.layer else {
                panic!("F2 {shape} did not lower to a solid overlay");
            };
            assert_eq!(effects[0].match_name, "ANR Shape Overlay");
            assert_eq!(effects[0].params["shape"], json!(lowered_shape));
        }

        for device in ["head", "pinch", "holdfinger", "tap", "swipe"] {
            let mut payload = payload("hook.f4.motion.v1", json!([]));
            payload.visual_ops[0].params = json!({"device": device, "drop_time": 1.0});
            let result = lower_visual_operations(&payload, &comp());
            assert!(
                result
                    .layers
                    .iter()
                    .any(|layer| layer.layer.id() == format!("visual_f4_{device}")),
                "missing native F4 device layer for {device}"
            );
        }

        for effect_id in [
            "hook_light",
            "shutter_effect",
            "flash_slow_shutter",
            "negative_zoom",
            "snap_wipe",
            "minimax",
            "invert_flash",
            "extract_flash",
            "flash_on_cuts",
            "layer_shake",
            "analog_glitch",
            "xerox",
            "neon_extract",
            "old_camera",
            "blackwhite",
            "crystal_glow",
            "night_vision",
            "wave",
        ] {
            let mut payload = payload("hook.f3.effect.v1", json!([]));
            payload.visual_ops[0].params = json!({
                "detected_effect_ids": [effect_id],
                "drop_time": 1.0
            });
            payload.footage_layers = vec![
                layer(
                    "first.mp4",
                    "footage",
                    0.0,
                    json!({"source_footage":{"file_name":"first.mp4"}}),
                ),
                layer(
                    "second.mp4",
                    "footage",
                    2.0,
                    json!({"source_footage":{"file_name":"second.mp4"}}),
                ),
            ];
            let result = lower_visual_operations(&payload, &comp());
            assert!(
                !result.layers.is_empty(),
                "missing native F3 layers for {effect_id}"
            );
            assert!(
                !result.findings.iter().any(|finding| {
                    finding.status == CapabilityStatus::NotImplemented
                        && finding.feature.ends_with(effect_id)
                }),
                "F3 {effect_id} regressed to not_implemented"
            );
        }
    }

    fn sample_smoothstep_keys(keys: &[ScalarKeyframe], time: f64) -> f32 {
        if time <= keys[0].time {
            return keys[0].value;
        }
        for pair in keys.windows(2) {
            if time <= pair[1].time {
                let progress =
                    ((time - pair[0].time) / (pair[1].time - pair[0].time)).clamp(0.0, 1.0) as f32;
                let eased = progress * progress * (3.0 - 2.0 * progress);
                return pair[0].value + (pair[1].value - pair[0].value) * eased;
            }
        }
        keys.last().unwrap().value
    }
}
