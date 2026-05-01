use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPayload {
    #[serde(default, rename = "payloadVersion", alias = "payload_version")]
    pub payload_version: Option<String>,
    #[serde(rename = "projectSpec")]
    pub project_spec: ProjectSpec,
    #[serde(rename = "compsSpec")]
    pub comps_spec: Vec<CompSpec>,
    #[serde(default)]
    pub footage_layers: Vec<PayloadLayer>,
    #[serde(default)]
    pub text_layers: Vec<PayloadLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSpec {
    #[serde(rename = "mainCompName", alias = "main_comp_name")]
    pub main_comp_name: String,
    #[serde(default, rename = "subtitlesMode", alias = "subtitles_mode")]
    pub subtitles_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompSpec {
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub fps: f64,
    pub dur: f64,
    #[serde(default, rename = "pixelAspect", alias = "pixel_aspect")]
    pub pixel_aspect: Option<f64>,
    #[serde(default, rename = "workAreaStart", alias = "work_area_start")]
    pub work_area_start: Option<f64>,
    #[serde(default, rename = "workAreaDuration", alias = "work_area_duration")]
    pub work_area_duration: Option<f64>,
    #[serde(default, rename = "displayStartTime", alias = "display_start_time")]
    pub display_start_time: Option<f64>,
    #[serde(default, rename = "bgColor", alias = "bg_color")]
    pub bg_color: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadLayer {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub in_point: f64,
    pub out_point: f64,
    pub z_index: i32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub adjustment_layer: bool,
    #[serde(default)]
    pub props: BTreeMap<String, PropertySpec>,
    #[serde(default)]
    pub effects: BTreeMap<String, BTreeMap<String, PropertySpec>>,
    #[serde(default)]
    pub text_data: Value,
    #[serde(default)]
    pub source_rect: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySpec {
    #[serde(default)]
    pub match_name: Option<String>,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub keyframes: Vec<KeyframeSpec>,
    #[serde(default)]
    pub expression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeSpec {
    pub t: f64,
    #[serde(default)]
    pub v: Value,
    #[serde(default)]
    pub iit: Option<String>,
    #[serde(default)]
    pub oit: Option<String>,
    #[serde(default)]
    pub ease_in: Vec<Value>,
    #[serde(default)]
    pub ease_out: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityStatus {
    Supported,
    Ignored,
    Approximate,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityFinding {
    pub status: CapabilityStatus,
    pub feature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadSummary {
    pub payload_version: Option<String>,
    pub main_comp: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub footage_layers: usize,
    pub text_layers: usize,
    pub total_layers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadValidationReport {
    pub ok: bool,
    pub strict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<PayloadSummary>,
    pub layer_types: BTreeMap<String, usize>,
    pub effects: BTreeMap<String, usize>,
    pub expressions: BTreeMap<String, usize>,
    pub keyframed_properties: BTreeMap<String, usize>,
    pub findings: Vec<CapabilityFinding>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadImportResult {
    pub scene: render_ir::Scene,
    pub diagnostics: PayloadImportDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadImportDiagnostics {
    pub main_comp: String,
    pub imported_layers: usize,
    pub skipped_layers: usize,
    pub assets: usize,
    pub findings: Vec<CapabilityFinding>,
}

impl PayloadValidationReport {
    pub fn has_unsupported(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.status == CapabilityStatus::Unsupported)
    }
}

pub fn load_payload(path: impl AsRef<Path>) -> anyhow::Result<GeneratedPayload> {
    let raw = fs::read_to_string(path.as_ref())?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn validate_payload(payload: &GeneratedPayload, strict: bool) -> PayloadValidationReport {
    let mut errors = Vec::new();
    let mut findings = Vec::new();
    let mut layer_types = BTreeMap::new();
    let mut effects = BTreeMap::new();
    let mut expressions = BTreeMap::new();
    let mut keyframed_properties = BTreeMap::new();

    let main_comp = payload
        .comps_spec
        .iter()
        .find(|comp| comp.name == payload.project_spec.main_comp_name);

    if payload.project_spec.main_comp_name.trim().is_empty() {
        errors.push("projectSpec.mainCompName is required".to_string());
    }
    if main_comp.is_none() {
        errors.push(format!(
            "main comp '{}' was not found in compsSpec",
            payload.project_spec.main_comp_name
        ));
    }

    for comp in &payload.comps_spec {
        if comp.w == 0 || comp.h == 0 {
            errors.push(format!("comp '{}' has invalid dimensions {}x{}", comp.name, comp.w, comp.h));
        }
        if comp.fps <= 0.0 {
            errors.push(format!("comp '{}' has invalid fps {}", comp.name, comp.fps));
        }
        if comp.dur <= 0.0 {
            errors.push(format!("comp '{}' has invalid duration {}", comp.name, comp.dur));
        }
    }

    for layer in payload.footage_layers.iter().chain(payload.text_layers.iter()) {
        *layer_types.entry(layer.kind.clone()).or_insert(0) += 1;
        validate_layer_timing(layer, &mut errors);
        classify_layer(layer, &mut findings);

        for (prop_name, prop) in &layer.props {
            if prop.expression.as_deref().is_some_and(|expr| !expr.trim().is_empty()) {
                *expressions.entry(prop_name.clone()).or_insert(0) += 1;
                findings.push(CapabilityFinding {
                    status: CapabilityStatus::Unsupported,
                    feature: format!("expression.{prop_name}"),
                    layer: Some(layer.name.clone()),
                    detail: "raw property expressions are not part of the MVP native path".to_string(),
                });
            }
            if !prop.keyframes.is_empty() {
                *keyframed_properties.entry(prop_name.clone()).or_insert(0) += prop.keyframes.len();
                findings.push(CapabilityFinding {
                    status: CapabilityStatus::Approximate,
                    feature: format!("keyframes.{prop_name}"),
                    layer: Some(layer.name.clone()),
                    detail: "hold/linear keyframes are planned before AE ease parity".to_string(),
                });
            }
        }

        for effect_name in layer.effects.keys() {
            let normalized = normalize_effect_name(effect_name);
            *effects.entry(normalized.to_string()).or_insert(0) += 1;
            findings.push(CapabilityFinding {
                status: effect_status(normalized),
                feature: format!("effect.{normalized}"),
                layer: Some(layer.name.clone()),
                detail: effect_detail(normalized).to_string(),
            });
        }
    }

    let summary = main_comp.map(|comp| PayloadSummary {
        payload_version: payload.payload_version.clone(),
        main_comp: comp.name.clone(),
        width: comp.w,
        height: comp.h,
        fps: comp.fps,
        duration: comp.dur,
        footage_layers: payload.footage_layers.len(),
        text_layers: payload.text_layers.len(),
        total_layers: payload.footage_layers.len() + payload.text_layers.len(),
    });

    let has_unsupported = findings
        .iter()
        .any(|finding| finding.status == CapabilityStatus::Unsupported);
    let ok = errors.is_empty() && (!strict || !has_unsupported);

    PayloadValidationReport {
        ok,
        strict,
        summary,
        layer_types,
        effects,
        expressions,
        keyframed_properties,
        findings,
        errors,
    }
}

pub fn import_payload_to_scene(payload: &GeneratedPayload) -> anyhow::Result<PayloadImportResult> {
    let main_comp = payload
        .comps_spec
        .iter()
        .find(|comp| comp.name == payload.project_spec.main_comp_name)
        .ok_or_else(|| anyhow::anyhow!("main comp '{}' was not found in compsSpec", payload.project_spec.main_comp_name))?;

    let mut diagnostics = PayloadImportDiagnostics {
        main_comp: main_comp.name.clone(),
        imported_layers: 0,
        skipped_layers: 0,
        assets: 0,
        findings: Vec::new(),
    };

    let mut assets = Vec::new();
    let mut layer_items = Vec::new();
    let mut asset_index = 0usize;

    for layer in payload.footage_layers.iter().chain(payload.text_layers.iter()) {
        let target_comp = layer_target_comp(layer).unwrap_or(&main_comp.name);
        if target_comp != main_comp.name {
            diagnostics.skipped_layers += 1;
            diagnostics.findings.push(CapabilityFinding {
                status: CapabilityStatus::Approximate,
                feature: "import.skip_non_main_comp".to_string(),
                layer: Some(layer.name.clone()),
                detail: format!("layer targets comp '{target_comp}'; precomp flattening is planned later"),
            });
            continue;
        }

        if layer.kind == "footage" && is_audio_layer(layer) {
            diagnostics.skipped_layers += 1;
            diagnostics.findings.push(CapabilityFinding {
                status: CapabilityStatus::Ignored,
                feature: "import.skip_audio".to_string(),
                layer: Some(layer.name.clone()),
                detail: "audio is recognized but not part of the current render IR import".to_string(),
            });
            continue;
        }

        match import_layer(layer, &mut assets, &mut asset_index) {
            Some(imported) => {
                diagnostics.imported_layers += 1;
                layer_items.push((layer.z_index, imported));
            }
            None => {
                diagnostics.skipped_layers += 1;
                diagnostics.findings.push(CapabilityFinding {
                    status: CapabilityStatus::Unsupported,
                    feature: format!("import.layer.{}", layer.kind),
                    layer: Some(layer.name.clone()),
                    detail: "layer type cannot be represented in render IR yet".to_string(),
                });
            }
        }
    }

    layer_items.sort_by_key(|(z_index, _)| *z_index);
    let layers = layer_items.into_iter().map(|(_, layer)| layer).collect::<Vec<_>>();
    diagnostics.assets = assets.len();

    let scene = render_ir::Scene {
        version: "0.2-payload".to_string(),
        composition: render_ir::Composition {
            id: main_comp.name.clone(),
            width: main_comp.w,
            height: main_comp.h,
            fps: main_comp.fps,
            duration: main_comp.dur,
            background: bg_color_to_rgba(main_comp.bg_color),
        },
        assets,
        layers,
    };

    Ok(PayloadImportResult { scene, diagnostics })
}

fn validate_layer_timing(layer: &PayloadLayer, errors: &mut Vec<String>) {
    if layer.out_point <= layer.in_point {
        errors.push(format!(
            "layer '{}' has invalid time range {}..{}",
            layer.name, layer.in_point, layer.out_point
        ));
    }
}

fn import_layer(
    layer: &PayloadLayer,
    assets: &mut Vec<render_ir::Asset>,
    asset_index: &mut usize,
) -> Option<render_ir::Layer> {
    let id = layer_id(layer);
    let start = layer.in_point;
    let duration = layer.out_point - layer.in_point;
    let transform = transform_of(layer);
    let effects = effects_of(layer);

    match layer.kind.as_str() {
        "footage" => {
            let asset_id = format!("video_{:04}", *asset_index);
            *asset_index += 1;
            assets.push(render_ir::Asset {
                id: asset_id.clone(),
                kind: render_ir::AssetKind::Video,
                path: footage_path(layer),
            });
            Some(render_ir::Layer::Footage {
                id,
                start,
                duration,
                source: asset_id,
                transform,
                effects,
            })
        }
        "text" => {
            let text_base = &layer.text_data["text_base"];
            Some(render_ir::Layer::Text {
                id,
                start,
                duration,
                text: layer.text.clone(),
                font: text_base
                    .get("font")
                    .and_then(Value::as_str)
                    .unwrap_or("default")
                    .to_string(),
                fontSize: text_base
                    .get("fontSize")
                    .and_then(Value::as_f64)
                    .unwrap_or(64.0) as f32,
                fill: color_value_to_rgba(text_base.get("fillColor"), [255, 255, 255, 255]),
                box_: None,
                transform,
                effects,
            })
        }
        "precomp" => Some(render_ir::Layer::Precomp {
            id,
            start,
            duration,
            composition: layer
                .text_data
                .pointer("/precomp_source/comp_name")
                .and_then(Value::as_str)
                .unwrap_or(&layer.name)
                .to_string(),
            collapse_transformations: layer
                .text_data
                .pointer("/layer_meta/collapseTransformation")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            transform,
            effects,
        }),
        "adjustment" => Some(render_ir::Layer::Adjustment {
            id,
            start,
            duration,
            effects,
        }),
        _ => None,
    }
}

fn classify_layer(layer: &PayloadLayer, findings: &mut Vec<CapabilityFinding>) {
    let (status, detail) = match layer.kind.as_str() {
        "footage" if is_audio_layer(layer) => (
            CapabilityStatus::Ignored,
            "audio layers are recognized but ignored until audio mux support",
        ),
        "footage" => (CapabilityStatus::Supported, "video footage layers are part of the first native render slice"),
        "text" => (
            CapabilityStatus::Supported,
            "static text layers are part of the first native render slice",
        ),
        "precomp" => (
            CapabilityStatus::Approximate,
            "simple text precomps will be flattened before full precomp support",
        ),
        "adjustment" => (
            CapabilityStatus::Unsupported,
            "adjustment layers require the later effect-on-accumulated-buffer pipeline",
        ),
        _ => (CapabilityStatus::Unsupported, "unknown layer type"),
    };

    findings.push(CapabilityFinding {
        status,
        feature: if layer.kind == "footage" && is_audio_layer(layer) {
            "layer.audio".to_string()
        } else {
            format!("layer.{}", layer.kind)
        },
        layer: Some(layer.name.clone()),
        detail: detail.to_string(),
    });
}

fn layer_target_comp(layer: &PayloadLayer) -> Option<&str> {
    layer
        .text_data
        .pointer("/layer_meta/comp_name_target")
        .and_then(Value::as_str)
}

fn is_audio_layer(layer: &PayloadLayer) -> bool {
    let audio_enabled = layer
        .text_data
        .pointer("/layer_meta/audioEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let file_name = layer
        .text_data
        .pointer("/source_footage/file_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();

    audio_enabled || file_name.ends_with(".mp3") || file_name.ends_with(".wav")
}

fn normalize_effect_name(effect_name: &str) -> &str {
    effect_name.split_once(':').map_or(effect_name, |(_, name)| name)
}

fn transform_of(layer: &PayloadLayer) -> render_ir::Transform2D {
    let default = render_ir::Transform2D::default();
    render_ir::Transform2D {
        anchor: prop_vec2(layer, "tf_anchor").unwrap_or(default.anchor),
        position: prop_vec2(layer, "tf_position").unwrap_or(default.position),
        scale: prop_vec2(layer, "tf_scale").unwrap_or(default.scale),
        rotation: prop_f32(layer, "tf_rotation").unwrap_or(default.rotation),
        opacity: prop_f32(layer, "tf_opacity")
            .or_else(|| prop_f32(layer, "layer_opacity"))
            .unwrap_or(default.opacity),
    }
}

fn effects_of(layer: &PayloadLayer) -> Vec<render_ir::EffectSpec> {
    layer
        .effects
        .iter()
        .map(|(name, params)| render_ir::EffectSpec {
            match_name: normalize_effect_name(name).to_string(),
            params: serde_json::to_value(params).unwrap_or_else(|_| json!({})),
        })
        .collect()
}

fn prop_vec2(layer: &PayloadLayer, name: &str) -> Option<[f32; 2]> {
    let value = &layer.props.get(name)?.value;
    let arr = value.as_array()?;
    Some([
        arr.first()?.as_f64()? as f32,
        arr.get(1)?.as_f64()? as f32,
    ])
}

fn prop_f32(layer: &PayloadLayer, name: &str) -> Option<f32> {
    layer.props.get(name)?.value.as_f64().map(|value| value as f32)
}

fn footage_path(layer: &PayloadLayer) -> String {
    let file_path = layer
        .text_data
        .pointer("/source_footage/file_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !file_path.trim().is_empty() {
        return file_path.to_string();
    }

    let file_name = layer
        .text_data
        .pointer("/source_footage/file_name")
        .and_then(Value::as_str)
        .unwrap_or(&layer.name);
    format!("media/video/{file_name}")
}

fn layer_id(layer: &PayloadLayer) -> String {
    let slug = layer
        .name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(48)
        .collect::<String>();
    if slug.is_empty() {
        format!("layer_{:04}", layer.z_index)
    } else {
        format!("layer_{:04}_{slug}", layer.z_index)
    }
}

fn bg_color_to_rgba(color: Option<[f32; 3]>) -> [u8; 4] {
    let color = color.unwrap_or([0.0, 0.0, 0.0]);
    [
        color_component_to_u8(color[0] as f64),
        color_component_to_u8(color[1] as f64),
        color_component_to_u8(color[2] as f64),
        255,
    ]
}

fn color_value_to_rgba(value: Option<&Value>, fallback: [u8; 4]) -> [u8; 4] {
    let Some(arr) = value.and_then(Value::as_array) else {
        return fallback;
    };
    if arr.len() < 3 {
        return fallback;
    }
    [
        arr.first().and_then(Value::as_f64).map(color_component_to_u8).unwrap_or(fallback[0]),
        arr.get(1).and_then(Value::as_f64).map(color_component_to_u8).unwrap_or(fallback[1]),
        arr.get(2).and_then(Value::as_f64).map(color_component_to_u8).unwrap_or(fallback[2]),
        arr.get(3).and_then(Value::as_f64).map(color_component_to_u8).unwrap_or(fallback[3]),
    ]
}

fn color_component_to_u8(value: f64) -> u8 {
    let scaled = if value <= 1.0 { value * 255.0 } else { value };
    scaled.round().clamp(0.0, 255.0) as u8
}

fn effect_status(effect_name: &str) -> CapabilityStatus {
    match effect_name {
        "ADBE Drop Shadow" | "ADBE Glo2" | "ADBE Box Blur2" => CapabilityStatus::Approximate,
        "ADBE Geometry2" | "ADBE Posterize Time" | "ADBE Minimax" | "ADBE Turbulent Displace" => {
            CapabilityStatus::Unsupported
        }
        _ => CapabilityStatus::Unsupported,
    }
}

fn effect_detail(effect_name: &str) -> &'static str {
    match effect_name {
        "ADBE Drop Shadow" => "planned as the first approximate text effect",
        "ADBE Glo2" => "planned after Drop Shadow as an approximate effect",
        "ADBE Box Blur2" => "planned for early blur support",
        "ADBE Geometry2" => "planned later with adjustment layer support",
        "ADBE Posterize Time" => "planned later after keyframe/time pipeline is stable",
        "ADBE Minimax" => "planned later for complex text templates",
        "ADBE Turbulent Displace" => "planned later as an approximate displacement effect",
        _ => "unknown effect is unsupported",
    }
}
