use crate::{CompSpec, GeneratedPayload, PayloadLayer, ProjectSpec, VisualOperation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const DEFAULT_FPS: f64 = 24_000.0 / 1_001.0;

/// Stable local bridge from the bot's planner output to render-request.v1.
///
/// The adapter deliberately keeps source mode/type/style metadata in `visualOps`.
/// That makes each approximation auditable instead of collapsing it into generic JSX.
#[derive(Debug, Clone, Deserialize)]
pub struct BotRenderEnvelope {
    #[serde(default, rename = "caseId", alias = "case_id")]
    pub case_id: Option<String>,
    #[serde(rename = "subtitles_mode", alias = "subtitlesMode", alias = "mode")]
    pub subtitles_mode: String,
    #[serde(default)]
    pub composition: BotComposition,
    #[serde(default, rename = "subtitle_payload", alias = "subtitlePayload")]
    pub subtitle_payload: Value,
    #[serde(default, rename = "subtitle_flow_plan", alias = "subtitleFlowPlan")]
    pub subtitle_flow_plan: Value,
    #[serde(default)]
    pub footage_layers: Vec<PayloadLayer>,
    #[serde(default)]
    pub text_layers: Vec<PayloadLayer>,
    #[serde(default, rename = "effectStyleIds", alias = "effect_style_ids")]
    pub effect_style_ids: Vec<String>,
    #[serde(default, rename = "cutTimes", alias = "cut_times")]
    pub cut_times: Vec<f64>,
    #[serde(default, rename = "visualOps", alias = "visual_ops")]
    pub visual_ops: Vec<VisualOperation>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BotComposition {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, alias = "width")]
    pub w: Option<u32>,
    #[serde(default, alias = "height")]
    pub h: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
    #[serde(default, alias = "duration")]
    pub dur: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BotAdaptedRequest {
    pub schema: &'static str,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub action: &'static str,
    #[serde(rename = "projectSpec")]
    pub project_spec: ProjectSpec,
    #[serde(rename = "compsSpec")]
    pub comps_spec: Vec<CompSpec>,
    pub footage_layers: Vec<PayloadLayer>,
    pub text_layers: Vec<PayloadLayer>,
    #[serde(rename = "visualOps")]
    pub visual_ops: Vec<VisualOperation>,
    #[serde(rename = "assetsSpec")]
    pub assets_spec: Value,
    #[serde(rename = "outputSpec")]
    pub output_spec: Value,
    pub policy: Value,
}

impl BotAdaptedRequest {
    pub fn generated_payload(&self) -> GeneratedPayload {
        GeneratedPayload {
            schema_version: Some("render-plan.v1.1".to_string()),
            payload_version: Some("bot-adapter.v1".to_string()),
            project_spec: self.project_spec.clone(),
            comps_spec: self.comps_spec.clone(),
            footage_layers: self.footage_layers.clone(),
            text_layers: self.text_layers.clone(),
            visual_ops: self.visual_ops.clone(),
            requirements: Value::Null,
            style_registry: Vec::new(),
            effect_registry: Vec::new(),
            golden_refs: Vec::new(),
        }
    }
}

pub fn adapt_bot_envelope(envelope: BotRenderEnvelope) -> Result<BotAdaptedRequest, String> {
    let mode = envelope.subtitles_mode.trim().to_string();
    if !matches!(
        mode.as_str(),
        "legacy_blocks" | "impulse_2nd" | "scenes_3rd" | "scenes_3rd_single_step" | "template_4th"
    ) {
        return Err(format!("unsupported bot subtitles_mode={mode:?}"));
    }

    let name = envelope
        .composition
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Comp 1".to_string());
    let dur = envelope
        .composition
        .dur
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .or_else(|| infer_duration(&envelope.subtitle_payload))
        .or_else(|| infer_duration(&envelope.subtitle_flow_plan))
        .unwrap_or(4.0);
    let fps = envelope
        .composition
        .fps
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(DEFAULT_FPS);
    let payload = select_subtitle_payload(&envelope);
    let segments = normalize_segments(&mode, payload)?;
    let mut visual_ops = Vec::new();
    visual_ops.push(VisualOperation {
        id: Some(format!("bot_subtitles_{mode}")),
        kind: format!("subtitle.bot.{mode}.v1"),
        target: Default::default(),
        timing: Default::default(),
        params: json!({
            "source_mode": mode,
            "segments": segments,
        }),
        assets: Vec::new(),
        required: true,
    });

    let mut f3_ids = match mode.as_str() {
        "impulse_2nd" => vec!["extract_flash"],
        "scenes_3rd" | "scenes_3rd_single_step" => vec!["minimax", "layer_shake"],
        "template_4th" => Vec::new(),
        "legacy_blocks" => vec!["minimax"],
        _ => Vec::new(),
    };
    if !envelope.cut_times.is_empty() && matches!(mode.as_str(), "impulse_2nd" | "template_4th") {
        f3_ids.push("flash_on_cuts");
    }
    if !f3_ids.is_empty() {
        visual_ops.push(VisualOperation {
            id: Some("bot_subtitle_effects".to_string()),
            kind: "hook.f3.effect.v1".to_string(),
            target: Default::default(),
            timing: Default::default(),
            params: json!({
                "detected_effect_ids": f3_ids,
                "cut_times": sanitize_times(envelope.cut_times.clone(), dur),
            }),
            assets: Vec::new(),
            required: true,
        });
    }
    for style_id in envelope.effect_style_ids {
        visual_ops.push(VisualOperation {
            id: Some(format!("bot_style_{style_id}")),
            kind: "style.semantic.v1".to_string(),
            target: Default::default(),
            timing: Default::default(),
            params: json!({"styleId": style_id}),
            assets: Vec::new(),
            required: true,
        });
    }
    visual_ops.extend(envelope.visual_ops);

    Ok(BotAdaptedRequest {
        schema: "ae-native-renderer.render-request.v1",
        request_id: envelope
            .case_id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("bot-{mode}")),
        action: "render",
        project_spec: ProjectSpec {
            main_comp_name: name.clone(),
            subtitles_mode: Some(mode),
            extra: BTreeMap::new(),
        },
        comps_spec: vec![CompSpec {
            name,
            w: envelope.composition.w.unwrap_or(1080),
            h: envelope.composition.h.unwrap_or(1920),
            fps,
            dur,
            pixel_aspect: None,
            work_area_start: None,
            work_area_duration: None,
            display_start_time: None,
            bg_color: Some([0.0, 0.0, 0.0]),
            extra: BTreeMap::new(),
        }],
        footage_layers: envelope.footage_layers,
        text_layers: envelope.text_layers,
        visual_ops,
        assets_spec: json!({"root": "."}),
        output_spec: json!({"directory": "out"}),
        policy: json!({"onUnsupported": "error"}),
    })
}

fn select_subtitle_payload(envelope: &BotRenderEnvelope) -> &Value {
    if envelope.subtitle_flow_plan.is_object() {
        &envelope.subtitle_flow_plan
    } else {
        &envelope.subtitle_payload
    }
}

fn normalize_segments(mode: &str, payload: &Value) -> Result<Vec<Value>, String> {
    let source = payload
        .as_object()
        .ok_or_else(|| "subtitle payload must be an object".to_string())?;
    let segments = match mode {
        "impulse_2nd" => source.get("segments").and_then(Value::as_array),
        "scenes_3rd" | "scenes_3rd_single_step" => source
            .get("segments")
            .and_then(Value::as_array)
            .or_else(|| source.get("scenes").and_then(Value::as_array)),
        "template_4th" => source
            .get("segments")
            .and_then(Value::as_array)
            .or_else(|| source.get("subtitles").and_then(Value::as_array)),
        "legacy_blocks" => source.get("segments").and_then(Value::as_array),
        _ => None,
    }
    .ok_or_else(|| format!("{mode} payload has no segments/scenes/subtitles array"))?;
    let global_words = source
        .get("word_timings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut normalized = Vec::with_capacity(segments.len());
    for (index, segment) in segments.iter().enumerate() {
        let segment = segment
            .as_object()
            .ok_or_else(|| format!("segment {index} must be an object"))?;
        let start = number(segment, &["in_point", "in", "start"])
            .ok_or_else(|| format!("segment {index} has no start"))?;
        let end = number(segment, &["out_point", "out", "end"])
            .ok_or_else(|| format!("segment {index} has no end"))?;
        if !start.is_finite() || !end.is_finite() || end <= start {
            return Err(format!("segment {index} has invalid time {start}..{end}"));
        }
        let words = segment
            .get("tokens")
            .and_then(Value::as_array)
            .or_else(|| segment.get("word_timings").and_then(Value::as_array))
            .cloned()
            .unwrap_or_else(|| words_in_window(&global_words, start, end));
        let text = segment
            .get("text")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                segment.get("words").and_then(Value::as_array).map(|words| {
                    words
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            })
            .unwrap_or_else(|| {
                words
                    .iter()
                    .filter_map(word_text)
                    .collect::<Vec<_>>()
                    .join(" ")
            });
        if text.trim().is_empty() {
            return Err(format!("segment {index} has no text"));
        }
        let lines = segment.get("lines").cloned().unwrap_or_else(|| json!([]));
        normalized.push(json!({
            "id": segment.get("id").cloned().unwrap_or_else(|| json!(index + 1)),
            "text": text,
            "start": start,
            "end": end,
            "lines": lines,
            "words": normalize_words(words, start, end),
            "type": segment.get("type").or_else(|| segment.get("style_tag")).cloned().unwrap_or_else(|| json!(mode)),
            "focusWord": segment.get("focus_word").cloned().unwrap_or(Value::Null),
            "focusStyle": segment.get("focus_style").cloned().unwrap_or(Value::Null),
        }));
    }
    if normalized.is_empty() {
        return Err(format!("{mode} payload has no subtitle segments"));
    }
    Ok(normalized)
}

fn number(object: &serde_json::Map<String, Value>, names: &[&str]) -> Option<f64> {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(Value::as_f64))
}

fn word_text(value: &Value) -> Option<&str> {
    value
        .as_object()?
        .get("word")
        .or_else(|| value.as_object()?.get("text"))?
        .as_str()
}

fn words_in_window(words: &[Value], start: f64, end: f64) -> Vec<Value> {
    words
        .iter()
        .filter(|word| {
            let Some(word) = word.as_object() else {
                return false;
            };
            let word_start = number(word, &["start", "t_start"]);
            let word_end = number(word, &["end", "t_end"]);
            word_start.is_some_and(|time| time >= start - 1e-6)
                && word_end.is_some_and(|time| time <= end + 1e-6)
        })
        .cloned()
        .collect()
}

fn normalize_words(words: Vec<Value>, start: f64, end: f64) -> Vec<Value> {
    let fallback_duration = ((end - start) / words.len().max(1) as f64).max(1.0 / 60.0);
    words
        .into_iter()
        .enumerate()
        .filter_map(|(index, word)| {
            let text = word_text(&word)?.trim().to_string();
            if text.is_empty() {
                return None;
            }
            let object = word.as_object();
            let word_start = object.and_then(|object| number(object, &["start", "t_start"]))
                .unwrap_or(start + index as f64 * fallback_duration);
            let word_end = object.and_then(|object| number(object, &["end", "t_end"]))
                .unwrap_or((word_start + fallback_duration).min(end));
            Some(json!({
                "word": text,
                "start": word_start.max(start),
                "end": word_end.max(word_start + 1.0 / 600.0).min(end),
                "focus": object.and_then(|object| object.get("focus")).and_then(Value::as_bool).unwrap_or(false),
            }))
        })
        .collect()
}

fn infer_duration(payload: &Value) -> Option<f64> {
    let object = payload.as_object()?;
    object
        .get("clip")
        .and_then(|clip| clip.get("end"))
        .and_then(Value::as_f64)
        .or_else(|| {
            ["segments", "scenes", "subtitles"]
                .into_iter()
                .filter_map(|key| object.get(key).and_then(Value::as_array))
                .flatten()
                .filter_map(|segment| {
                    segment
                        .get("out")
                        .or_else(|| segment.get("out_point"))
                        .or_else(|| segment.get("end"))
                })
                .filter_map(Value::as_f64)
                .max_by(f64::total_cmp)
        })
}

fn sanitize_times(times: Vec<f64>, duration: f64) -> Vec<f64> {
    let mut times = times
        .into_iter()
        .filter(|time| time.is_finite() && *time >= 0.0 && *time < duration)
        .collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| (*left - *right).abs() < 1e-6);
    times
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CapabilityStatus;

    #[test]
    fn scenes_payload_keeps_every_type_as_a_native_operation() {
        let request = adapt_bot_envelope(serde_json::from_value(json!({
            "caseId": "types",
            "subtitles_mode": "scenes_3rd",
            "composition": {"dur": 7.0},
            "subtitle_payload": {
                "clip": {"start": 0.0, "end": 7.0},
                "scenes": (1..=6).map(|index| json!({
                    "id": index,
                    "type": format!("TYPE_{index}"),
                    "words": ["одна", "сцена"],
                    "start": (index - 1) as f64,
                    "end": index as f64,
                    "word_timings": [
                        {"word": "одна", "start": (index - 1) as f64, "end": (index - 1) as f64 + 0.4},
                        {"word": "сцена", "start": (index - 1) as f64 + 0.4, "end": index as f64}
                    ]
                })).collect::<Vec<_>>()
            }
        })).unwrap()).unwrap();
        let operation = &request.visual_ops[0];
        assert_eq!(operation.kind, "subtitle.bot.scenes_3rd.v1");
        assert_eq!(operation.params["segments"].as_array().unwrap().len(), 6);
        assert_eq!(operation.params["segments"][4]["type"], "TYPE_5");
    }

    #[test]
    fn template_payload_assigns_global_words_to_each_subtitle_window() {
        let request = adapt_bot_envelope(
            serde_json::from_value(json!({
                "subtitles_mode": "template_4th",
                "subtitle_payload": {
                    "word_timings": [
                        {"word": "дай", "start": 0.0, "end": 0.3, "focus": false},
                        {"word": "мне", "start": 0.3, "end": 0.6, "focus": true}
                    ],
                    "subtitles": [{"text": "дай мне", "in": 0.0, "out": 0.8}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(request.comps_spec[0].dur, 0.8);
        assert_eq!(
            request.visual_ops[0].params["segments"][0]["words"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn active_bot_modes_and_semantic_styles_are_accepted_as_approximate() {
        for (mode, subtitle_payload) in [
            (
                "impulse_2nd",
                json!({"segments":[{"text":"x","in":0.0,"out":1.0,"type":"long"}]}),
            ),
            (
                "scenes_3rd",
                json!({"scenes":[{"id":1,"type":"TYPE_1","words":["x"],"start":0.0,"end":1.0}]}),
            ),
            (
                "scenes_3rd_single_step",
                json!({"scenes":[{"id":1,"type":"TYPE_6","words":["x"],"start":0.0,"end":1.0}]}),
            ),
            (
                "template_4th",
                json!({"subtitles":[{"text":"x","in":0.0,"out":1.0}]}),
            ),
        ] {
            let request = adapt_bot_envelope(
                serde_json::from_value(json!({
                    "subtitles_mode": mode,
                    "subtitle_payload": subtitle_payload,
                    "effectStyleIds": ["txt_soft_v1"]
                }))
                .unwrap(),
            )
            .unwrap();
            let report = crate::validate_payload(&request.generated_payload(), true);
            assert!(report.ok, "{mode}: {:#?}", report.errors);
            assert!(!report.has_unsupported(), "{mode}: {:#?}", report.findings);
        }
    }

    #[test]
    fn legacy_blocks_are_preserved_but_not_implemented() {
        let request = adapt_bot_envelope(
            serde_json::from_value(json!({
                "subtitles_mode": "legacy_blocks",
                "subtitle_payload": {"segments":[{"text":"x","in_point":0.0,"out_point":1.0}]}
            }))
            .unwrap(),
        )
        .unwrap();
        let report = crate::validate_payload(&request.generated_payload(), true);
        assert!(!report.ok);
        assert!(report.findings.iter().any(|finding| {
            finding.status == CapabilityStatus::NotImplemented
                && finding.feature == "visual_op.subtitle.bot.legacy_blocks.v1"
        }));
    }

    #[test]
    fn active_bot_modes_lower_without_required_capability_gaps() {
        for (mode, segment_type) in [
            ("impulse_2nd", "long"),
            ("scenes_3rd", "TYPE_5"),
            ("scenes_3rd_single_step", "TYPE_6"),
            ("template_4th", "focus"),
        ] {
            let request = adapt_bot_envelope(
                serde_json::from_value(json!({
                    "subtitles_mode": mode,
                    "composition": {"w": 1080, "h": 1920, "fps": 23.976, "dur": 1.0},
                    "subtitle_payload": {
                        "segments": [{
                            "id": 1,
                            "type": segment_type,
                            "text": "одна сцена",
                            "start": 0.0,
                            "end": 1.0,
                            "word_timings": [
                                {"word": "одна", "start": 0.0, "end": 0.4},
                                {"word": "сцена", "start": 0.4, "end": 1.0}
                            ]
                        }]
                    }
                }))
                .unwrap(),
            )
            .unwrap();
            let lowered = crate::lower_visual_operations(
                &request.generated_payload(),
                &request.comps_spec[0],
            );
            assert!(
                !lowered.layers.is_empty(),
                "{mode} produced no native layers"
            );
            assert!(
                !lowered.findings.iter().any(|finding| {
                    finding.status == CapabilityStatus::NotImplemented
                        || finding.status == CapabilityStatus::Unsupported
                }),
                "{mode}: {:#?}",
                lowered.findings
            );
        }
    }
}
