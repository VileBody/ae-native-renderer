use crate::{GeneratedPayload, VisualOperation, VisualOperationTarget, VisualOperationTiming};
use anyhow::Context;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

pub fn load_payload_from_jsx(path: impl AsRef<Path>) -> anyhow::Result<GeneratedPayload> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .with_context(|| format!("reading generated JSX {}", path.display()))?;
    extract_payload_from_jsx(&source)
        .with_context(|| format!("extracting generated payload from {}", path.display()))
}

pub fn extract_payload_from_jsx(source: &str) -> anyhow::Result<GeneratedPayload> {
    let project_spec = extract_json_var(source, "projectSpec")?;
    let comps_spec = extract_json_var(source, "compsSpec")?;
    let footage_layers = extract_json_var(source, "footage_layers")?;
    let text_layers = extract_json_var(source, "text_layers")?;
    let mut payload: GeneratedPayload = serde_json::from_value(json!({
        "projectSpec": project_spec,
        "compsSpec": comps_spec,
        "footage_layers": footage_layers,
        "text_layers": text_layers,
        "visualOps": []
    }))?;
    payload.visual_ops = extract_injected_visual_ops(source, &payload);
    Ok(payload)
}

pub fn extract_json_var(source: &str, name: &str) -> anyhow::Result<Value> {
    extract_json_after_marker(source, &format!("var {name} ="))
        .with_context(|| format!("missing or invalid generated JSX var {name}"))
}

fn extract_injected_visual_ops(source: &str, payload: &GeneratedPayload) -> Vec<VisualOperation> {
    let mut operations = Vec::new();
    let subtitles_mode = payload
        .project_spec
        .subtitles_mode
        .as_deref()
        .unwrap_or_default();
    if matches!(subtitles_mode, "brat_5th" | "trendy_5th") {
        if let Ok(subtitles) = extract_json_after_marker(source, "$.global.__BLAST_SUBS_JSON =") {
            let words = subtitles
                .get("word_timings")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let start = words
                .iter()
                .filter_map(|word| word.get("start").and_then(Value::as_f64))
                .min_by(f64::total_cmp);
            let end = words
                .iter()
                .filter_map(|word| word.get("end").and_then(Value::as_f64))
                .max_by(f64::total_cmp);
            let target = extract_js_string_assignment(source, "$.global.__BLAST_TARGET_COMP =")
                .unwrap_or_else(|| payload.project_spec.main_comp_name.clone());
            let kind = if subtitles_mode == "brat_5th" {
                "subtitle.brat.v1"
            } else {
                "subtitle.trendy.v1"
            };
            operations.push(VisualOperation {
                id: Some("generated_subtitles".to_string()),
                kind: kind.to_string(),
                target: VisualOperationTarget {
                    composition: Some(target),
                    layer: None,
                    place: Some("above:footage".to_string()),
                },
                timing: VisualOperationTiming {
                    start,
                    duration: start.zip(end).map(|(start, end)| (end - start).max(0.0)),
                    end,
                    anchor: Some("absolute".to_string()),
                    offset: None,
                },
                params: json!({
                    "word_timings": words,
                    "fill": extract_json_after_marker(source, "$.global.__BLAST_FILL =").ok(),
                    "bpm": extract_js_number_assignment(source, "$.global.__BLAST_BPM ="),
                    "blend_mode": extract_js_string_assignment(source, "$.global.__BLAST_SUBS_BLEND ="),
                    "source_mode": subtitles_mode
                }),
                assets: Vec::new(),
                required: true,
            });
        }
    }

    if let Some(section) = section_between(source, "/* ===== F3 «Эффект» overlay", "// 5.7) F2")
    {
        let drop_time = extract_js_number_assignment(section, "var __f3_drop =");
        let detected = detect_f3_ids(section);
        operations.push(VisualOperation {
            id: Some("generated_f3_effect".to_string()),
            kind: "hook.f3.effect.v1".to_string(),
            target: VisualOperationTarget {
                composition: Some(payload.project_spec.main_comp_name.clone()),
                layer: Some("Текст".to_string()),
                place: Some("below:Текст".to_string()),
            },
            timing: VisualOperationTiming {
                start: drop_time,
                duration: None,
                end: None,
                anchor: Some("drop".to_string()),
                offset: Some(0.0),
            },
            params: json!({
                "drop_time": drop_time,
                "detected_effect_ids": detected
            }),
            assets: Vec::new(),
            required: true,
        });
    }

    operations
}

fn detect_f3_ids(section: &str) -> Vec<String> {
    const TOKENS: &[(&str, &str)] = &[
        ("flash_on_cuts", "flash_on_cuts"),
        ("snap_wipe", "snap_wipe"),
        ("extract_flash", "extract_flash"),
        ("invert_flash", "invert_flash"),
        ("layer_shake", "layer_shake"),
        ("negative_zoom", "negative_zoom"),
        ("flash_slow_shutter", "flash_slow_shutter"),
        ("slow shutter", "flash_slow_shutter"),
        ("hook_light", "hook_light"),
        ("rebuild_light", "hook_light"),
        ("shutter_effect", "shutter_effect"),
        ("shutter effect", "shutter_effect"),
        ("shutter (hook)", "shutter_effect"),
        ("analog_glitch", "analog_glitch"),
        ("analog glitch", "analog_glitch"),
        ("neon_extract", "neon_extract"),
        ("old_camera", "old_camera"),
        ("xerox", "xerox"),
    ];
    let lower = section.to_ascii_lowercase();
    let mut detected = TOKENS
        .iter()
        .filter(|(needle, _)| lower.contains(needle))
        .map(|(_, id)| (*id).to_string())
        .collect::<Vec<_>>();
    detected.sort();
    detected.dedup();
    detected
}

fn section_between<'a>(source: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_index = source.find(start)?;
    let tail = &source[start_index..];
    let end_index = tail.find(end).unwrap_or(tail.len());
    let section = &tail[..end_index];
    section.contains("$.global.__BLAST").then_some(section)
}

fn extract_json_after_marker(source: &str, marker: &str) -> anyhow::Result<Value> {
    let start = source
        .find(marker)
        .with_context(|| format!("missing marker {marker}"))?
        + marker.len();
    let bytes = source.as_bytes();
    let mut index = start;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let json_start = index;
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source[json_start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '[' | '{' => stack.push(ch),
            ']' | '}' => {
                stack.pop();
                if stack.is_empty() {
                    let end = json_start + offset + ch.len_utf8();
                    return Ok(serde_json::from_str(&source[json_start..end])?);
                }
            }
            _ => {}
        }
    }
    anyhow::bail!("unterminated JSON after marker {marker}")
}

fn extract_js_number_assignment(source: &str, marker: &str) -> Option<f64> {
    let start = source.find(marker)? + marker.len();
    source[start..]
        .split(';')
        .next()?
        .trim()
        .parse::<f64>()
        .ok()
}

fn extract_js_string_assignment(source: &str, marker: &str) -> Option<String> {
    let start = source.find(marker)? + marker.len();
    let raw = source[start..].split(';').next()?.trim();
    serde_json::from_str(raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_payload_and_dynamic_subtitle_contract() {
        let source = r#"
            var projectSpec = {"mainCompName":"Comp 1","subtitlesMode":"brat_5th"};
            var compsSpec = [{"name":"Comp 1","w":64,"h":64,"fps":24,"dur":2}];
            var footage_layers = [];
            var text_layers = [];
            $.global.__BLAST_SUBS_JSON = {"word_timings":[{"word":"hi","start":0.1,"end":0.5}]};
            $.global.__BLAST_TARGET_COMP = "Comp 1";
            $.global.__BLAST_BPM = 120;
            $.global.__BLAST_FILL = [1,1,1];
            $.global.__BLAST_SUBS_BLEND = "difference";
        "#;
        let payload = extract_payload_from_jsx(source).unwrap();
        assert_eq!(payload.project_spec.main_comp_name, "Comp 1");
        assert_eq!(payload.visual_ops.len(), 1);
        assert_eq!(payload.visual_ops[0].kind, "subtitle.brat.v1");
        assert_eq!(payload.visual_ops[0].timing.start, Some(0.1));
        assert_eq!(payload.visual_ops[0].timing.end, Some(0.5));
    }

    #[test]
    fn f3_section_is_preserved_as_operation() {
        let source = r#"
            var projectSpec = {"mainCompName":"Comp 1","subtitlesMode":"impulse_2nd"};
            var compsSpec = [{"name":"Comp 1","w":64,"h":64,"fps":24,"dur":2}];
            var footage_layers = [];
            var text_layers = [];
            /* ===== F3 «Эффект» overlay (injected by build worker) ===== */
            var __f3_drop = 1.25;
            $.global.__BLAST = {dropTime: __f3_drop};
            /*** flash_on_cuts (transition) ***/
            // ==========================================================
    // 5.7)
        "#;
        let payload = extract_payload_from_jsx(source).unwrap();
        assert_eq!(payload.visual_ops.len(), 1);
        assert_eq!(payload.visual_ops[0].kind, "hook.f3.effect.v1");
        assert_eq!(payload.visual_ops[0].timing.start, Some(1.25));
        assert_eq!(
            payload.visual_ops[0].params["detected_effect_ids"][0],
            "flash_on_cuts"
        );
    }

    #[test]
    fn f3_detector_reports_palette_tools_not_internal_primitives() {
        let detected = detect_f3_ids(
            "/*** shutter (hook) ***/ /*** snap_wipe ***/ /*** analog glitch ***/ ADBE Minimax",
        );
        assert_eq!(
            detected,
            vec!["analog_glitch", "shutter_effect", "snap_wipe"]
        );
    }
}
