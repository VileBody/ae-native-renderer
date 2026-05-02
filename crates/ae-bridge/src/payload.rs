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
            errors.push(format!(
                "comp '{}' has invalid dimensions {}x{}",
                comp.name, comp.w, comp.h
            ));
        }
        if comp.fps <= 0.0 {
            errors.push(format!("comp '{}' has invalid fps {}", comp.name, comp.fps));
        }
        if comp.dur <= 0.0 {
            errors.push(format!(
                "comp '{}' has invalid duration {}",
                comp.name, comp.dur
            ));
        }
    }

    for layer in payload
        .footage_layers
        .iter()
        .chain(payload.text_layers.iter())
    {
        *layer_types.entry(layer.kind.clone()).or_insert(0) += 1;
        validate_layer_timing(layer, &mut errors);
        classify_layer(layer, &mut findings);
        classify_text_animator(layer, &mut findings);

        for (prop_name, prop) in &layer.props {
            if prop
                .expression
                .as_deref()
                .is_some_and(|expr| !expr.trim().is_empty())
            {
                let expression = prop.expression.as_deref().unwrap_or_default();
                let (status, detail) = property_expression_status(prop_name, expression);
                *expressions.entry(prop_name.clone()).or_insert(0) += 1;
                findings.push(CapabilityFinding {
                    status,
                    feature: format!("expression.{prop_name}"),
                    layer: Some(layer.name.clone()),
                    detail: detail.to_string(),
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

        for (effect_name, params) in &layer.effects {
            let normalized = normalize_effect_name(effect_name);
            for (param_name, param) in params {
                if let Some(expression) = param.expression.as_deref() {
                    if expression.trim().is_empty() {
                        continue;
                    }
                    let (status, detail) =
                        effect_expression_status(normalized, param_name, expression);
                    *expressions
                        .entry(format!("{normalized}.{param_name}"))
                        .or_insert(0) += 1;
                    findings.push(CapabilityFinding {
                        status,
                        feature: format!("expression.{normalized}.{param_name}"),
                        layer: Some(layer.name.clone()),
                        detail: detail.to_string(),
                    });
                }
            }
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
        .ok_or_else(|| {
            anyhow::anyhow!(
                "main comp '{}' was not found in compsSpec",
                payload.project_spec.main_comp_name
            )
        })?;

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
    let precomp_sources = main_precomp_sources(payload, &main_comp.name);

    for layer in payload
        .footage_layers
        .iter()
        .chain(payload.text_layers.iter())
    {
        let target_comp = layer_target_comp(layer).unwrap_or(&main_comp.name);
        if target_comp != main_comp.name {
            if let Some(parent) = precomp_sources.get(target_comp) {
                if layer.kind == "text" || layer.kind == "adjustment" {
                    match import_flattened_text_layer(layer, parent, main_comp) {
                        Some(imported) => {
                            diagnostics.imported_layers += 1;
                            diagnostics.findings.push(CapabilityFinding {
                                status: CapabilityStatus::Approximate,
                                feature: format!("import.flatten_{}_precomp", layer.kind),
                                layer: Some(layer.name.clone()),
                                detail: format!("flattened {} layer from comp '{target_comp}' through precomp '{}'", layer.kind, parent.name),
                            });
                            layer_items.push((flattened_sort_key(parent, layer), imported));
                        }
                        None => {
                            diagnostics.skipped_layers += 1;
                            diagnostics.findings.push(CapabilityFinding {
                                status: CapabilityStatus::Unsupported,
                                feature: format!("import.skip_flattened_{}", layer.kind),
                                layer: Some(layer.name.clone()),
                                detail: format!("flattened {} layer has no active time range after parent clipping", layer.kind),
                            });
                        }
                    }
                    continue;
                }
            }

            diagnostics.skipped_layers += 1;
            diagnostics.findings.push(CapabilityFinding {
                status: CapabilityStatus::Approximate,
                feature: "import.skip_non_main_comp".to_string(),
                layer: Some(layer.name.clone()),
                detail: format!("layer targets comp '{target_comp}'; non-text or nested precomp content is not imported yet"),
            });
            continue;
        }

        if layer.kind == "precomp"
            && precomp_source_name(layer)
                .is_some_and(|name| precomp_has_text_children(payload, name))
        {
            diagnostics.skipped_layers += 1;
            diagnostics.findings.push(CapabilityFinding {
                status: CapabilityStatus::Approximate,
                feature: "import.skip_flattened_precomp_placeholder".to_string(),
                layer: Some(layer.name.clone()),
                detail: "precomp placeholder was replaced by flattened child text layers"
                    .to_string(),
            });
            continue;
        }

        if layer.kind == "footage" && is_audio_layer(layer) {
            diagnostics.skipped_layers += 1;
            diagnostics.findings.push(CapabilityFinding {
                status: CapabilityStatus::Ignored,
                feature: "import.skip_audio".to_string(),
                layer: Some(layer.name.clone()),
                detail: "audio is recognized but not part of the current render IR import"
                    .to_string(),
            });
            continue;
        }

        match import_layer(layer, &mut assets, &mut asset_index) {
            Some(imported) => {
                diagnostics.imported_layers += 1;
                layer_items.push((main_sort_key(layer), imported));
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

    layer_items.sort_by_key(|(sort_key, _)| *sort_key);
    let layers = layer_items
        .into_iter()
        .map(|(_, layer)| layer)
        .collect::<Vec<_>>();
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
        compositions: Vec::new(),
        assets,
        layers,
    };

    Ok(PayloadImportResult { scene, diagnostics })
}

fn main_precomp_sources<'a>(
    payload: &'a GeneratedPayload,
    main_comp_name: &str,
) -> BTreeMap<String, &'a PayloadLayer> {
    payload
        .footage_layers
        .iter()
        .chain(payload.text_layers.iter())
        .filter(|layer| layer.kind == "precomp")
        .filter(|layer| layer_target_comp(layer).unwrap_or(main_comp_name) == main_comp_name)
        .filter_map(|layer| precomp_source_name(layer).map(|name| (name.to_string(), layer)))
        .collect()
}

fn precomp_has_text_children(payload: &GeneratedPayload, comp_name: &str) -> bool {
    payload
        .text_layers
        .iter()
        .any(|layer| {
            matches!(layer.kind.as_str(), "text" | "adjustment")
                && layer_target_comp(layer) == Some(comp_name)
        })
}

fn import_flattened_text_layer(
    layer: &PayloadLayer,
    parent: &PayloadLayer,
    main_comp: &CompSpec,
) -> Option<render_ir::Layer> {
    let start = (parent.in_point + layer.in_point).max(parent.in_point);
    let end = (parent.in_point + layer.out_point).min(parent.out_point);
    if end <= start {
        return None;
    }

    if layer.kind == "adjustment" {
        return Some(render_ir::Layer::Adjustment {
            id: layer_id(layer),
            start,
            duration: end - start,
            effects: effects_of(layer),
        });
    }

    let child_transform = transform_of(layer);
    let transform = compose_precomp_transform(transform_of(parent), child_transform);
    Some(import_text_layer(
        layer,
        start,
        end - start,
        transform,
        main_comp,
    ))
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
                source_start: footage_source_start(layer),
                transform,
                effects,
            })
        }
        "text" => {
            let fallback_comp = CompSpec {
                name: String::new(),
                w: 1080,
                h: 1920,
                fps: 30.0,
                dur: duration,
                pixel_aspect: None,
                work_area_start: None,
                work_area_duration: None,
                display_start_time: None,
                bg_color: None,
            };
            Some(import_text_layer(
                layer,
                start,
                duration,
                transform,
                &fallback_comp,
            ))
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

fn import_text_layer(
    layer: &PayloadLayer,
    start: f64,
    duration: f64,
    transform: render_ir::Transform2D,
    comp: &CompSpec,
) -> render_ir::Layer {
    let text_base = &layer.text_data["text_base"];
    let font_size = text_base
        .get("fontSize")
        .and_then(Value::as_f64)
        .unwrap_or(64.0) as f32;
    render_ir::Layer::Text {
        id: layer_id(layer),
        start,
        duration,
        text: layer.text.clone(),
        font: text_base
            .get("font")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string(),
        fontSize: font_size,
        fill: color_value_to_rgba(text_base.get("fillColor"), [255, 255, 255, 255]),
        box_: Some(text_box_for(&transform, font_size, comp)),
        transform,
        text_animators: text_animators_of(layer),
        effects: effects_of(layer),
    }
}

fn classify_layer(layer: &PayloadLayer, findings: &mut Vec<CapabilityFinding>) {
    let (status, detail) = match layer.kind.as_str() {
        "footage" if is_audio_layer(layer) => (
            CapabilityStatus::Ignored,
            "audio layers are recognized but ignored until audio mux support",
        ),
        "footage" => (
            CapabilityStatus::Supported,
            "video footage layers are part of the first native render slice",
        ),
        "text" => (
            CapabilityStatus::Supported,
            "static text layers are part of the first native render slice",
        ),
        "precomp" => (
            CapabilityStatus::Approximate,
            "simple text precomps will be flattened before full precomp support",
        ),
        "adjustment" => (
            CapabilityStatus::Approximate,
            "adjustment layers apply known effects to the accumulated buffer",
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

fn classify_text_animator(layer: &PayloadLayer, findings: &mut Vec<CapabilityFinding>) {
    let Some(animator) = layer.text_data.get("text_animator") else {
        return;
    };

    if animator.get("selector").is_some() {
        findings.push(CapabilityFinding {
            status: CapabilityStatus::Approximate,
            feature: "text_animator.range_selector".to_string(),
            layer: Some(layer.name.clone()),
            detail: "range selector start/end, basedOn, and opacity are approximated".to_string(),
        });
    }

    if let Some(expression) = animator
        .pointer("/expressible_selector/amount/expression")
        .and_then(Value::as_str)
    {
        let (status, detail) = if text_expression_selector_from_expression(expression).is_some() {
            (
                CapabilityStatus::Approximate,
                "recognized generated per-character bounce expression selector",
            )
        } else {
            (
                CapabilityStatus::Unsupported,
                "expression selector is outside the supported native subset",
            )
        };
        findings.push(CapabilityFinding {
            status,
            feature: "text_animator.expressible_selector".to_string(),
            layer: Some(layer.name.clone()),
            detail: detail.to_string(),
        });
    }
}

fn property_expression_status(
    prop_name: &str,
    expression: &str,
) -> (CapabilityStatus, &'static str) {
    if prop_name == "tf_position" && is_edge_wobble_position_expression(expression) {
        return (
            CapabilityStatus::Approximate,
            "recognized generated edge_wobble position expression",
        );
    }

    (
        CapabilityStatus::Unsupported,
        "raw property expression is outside the supported native subset",
    )
}

fn effect_expression_status(
    effect_name: &str,
    param_name: &str,
    expression: &str,
) -> (CapabilityStatus, &'static str) {
    if effect_name == "ADBE Turbulent Displace"
        && param_name == "0006"
        && expression.trim() == "time*500"
    {
        return (
            CapabilityStatus::Approximate,
            "recognized generated Turbulent Displace evolution expression",
        );
    }

    (
        CapabilityStatus::Unsupported,
        "raw effect expression is outside the supported native subset",
    )
}

fn layer_target_comp(layer: &PayloadLayer) -> Option<&str> {
    layer
        .text_data
        .pointer("/layer_meta/comp_name_target")
        .and_then(Value::as_str)
}

fn precomp_source_name(layer: &PayloadLayer) -> Option<&str> {
    layer
        .text_data
        .pointer("/precomp_source/comp_name")
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

fn main_sort_key(layer: &PayloadLayer) -> i64 {
    i64::from(layer.z_index) * 10_000
}

fn flattened_sort_key(parent: &PayloadLayer, child: &PayloadLayer) -> i64 {
    main_sort_key(parent) + i64::from(child.z_index.clamp(0, 9_999))
}

fn normalize_effect_name(effect_name: &str) -> &str {
    effect_name
        .split_once(':')
        .map_or(effect_name, |(_, name)| name)
}

fn transform_of(layer: &PayloadLayer) -> render_ir::Transform2D {
    let default = render_ir::Transform2D::default();
    let opacity_animation = prop_f32_keyframes(layer, "tf_opacity")
        .or_else(|| prop_f32_keyframes(layer, "layer_opacity"))
        .unwrap_or_default();
    render_ir::Transform2D {
        anchor: prop_vec2(layer, "tf_anchor").unwrap_or(default.anchor),
        position: prop_vec2(layer, "tf_position").unwrap_or(default.position),
        scale: prop_vec2(layer, "tf_scale").unwrap_or(default.scale),
        rotation: prop_f32(layer, "tf_rotation").unwrap_or(default.rotation),
        opacity: prop_f32(layer, "tf_opacity")
            .or_else(|| prop_f32(layer, "layer_opacity"))
            .unwrap_or(default.opacity),
        animation: render_ir::Transform2DAnimation {
            position: prop_vec2_keyframes(layer, "tf_position").unwrap_or_default(),
            scale: prop_vec2_keyframes(layer, "tf_scale").unwrap_or_default(),
            opacity: opacity_animation,
            reveal: prop_f32_keyframes(layer, "text_reveal_percent")
                .or_else(|| prop_f32_keyframes(layer, "text_reveal"))
                .or_else(|| prop_f32_keyframes(layer, "reveal"))
                .unwrap_or_default(),
            expression: render_ir::Transform2DExpression {
                position: position_expression(layer),
            },
        },
    }
}

fn compose_precomp_transform(
    parent: render_ir::Transform2D,
    child: render_ir::Transform2D,
) -> render_ir::Transform2D {
    let sx = parent.scale[0] / 100.0;
    let sy = parent.scale[1] / 100.0;
    let animation = compose_precomp_animation(&parent, &child, sx, sy);
    render_ir::Transform2D {
        anchor: child.anchor,
        position: [
            parent.position[0] + (child.position[0] - parent.anchor[0]) * sx,
            parent.position[1] + (child.position[1] - parent.anchor[1]) * sy,
        ],
        scale: [child.scale[0] * sx, child.scale[1] * sy],
        rotation: parent.rotation + child.rotation,
        opacity: parent.opacity * child.opacity / 100.0,
        animation,
    }
}

fn compose_precomp_animation(
    parent: &render_ir::Transform2D,
    child: &render_ir::Transform2D,
    sx: f32,
    sy: f32,
) -> render_ir::Transform2DAnimation {
    let position = if child.animation.position.is_empty() {
        parent
            .animation
            .position
            .iter()
            .map(|key| render_ir::Vec2Keyframe {
                time: key.time,
                value: [
                    key.value[0] + (child.position[0] - parent.anchor[0]) * sx,
                    key.value[1] + (child.position[1] - parent.anchor[1]) * sy,
                ],
                hold: key.hold,
                approximate: true,
            })
            .collect()
    } else {
        child
            .animation
            .position
            .iter()
            .map(|key| render_ir::Vec2Keyframe {
                time: key.time,
                value: [
                    parent.position[0] + (key.value[0] - parent.anchor[0]) * sx,
                    parent.position[1] + (key.value[1] - parent.anchor[1]) * sy,
                ],
                hold: key.hold,
                approximate: key.approximate,
            })
            .collect()
    };

    let scale = if child.animation.scale.is_empty() {
        parent
            .animation
            .scale
            .iter()
            .map(|key| render_ir::Vec2Keyframe {
                time: key.time,
                value: [
                    child.scale[0] * key.value[0] / 100.0,
                    child.scale[1] * key.value[1] / 100.0,
                ],
                hold: key.hold,
                approximate: true,
            })
            .collect()
    } else {
        child
            .animation
            .scale
            .iter()
            .map(|key| render_ir::Vec2Keyframe {
                time: key.time,
                value: [key.value[0] * sx, key.value[1] * sy],
                hold: key.hold,
                approximate: key.approximate,
            })
            .collect()
    };

    let opacity = if child.animation.opacity.is_empty() {
        parent
            .animation
            .opacity
            .iter()
            .map(|key| render_ir::ScalarKeyframe {
                time: key.time,
                value: key.value * child.opacity / 100.0,
                hold: key.hold,
                approximate: true,
            })
            .collect()
    } else {
        child
            .animation
            .opacity
            .iter()
            .map(|key| render_ir::ScalarKeyframe {
                time: key.time,
                value: parent.opacity * key.value / 100.0,
                hold: key.hold,
                approximate: key.approximate,
            })
            .collect()
    };

    render_ir::Transform2DAnimation {
        position,
        scale,
        opacity,
        reveal: child.animation.reveal.clone(),
        expression: child.animation.expression.clone(),
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

fn text_animators_of(layer: &PayloadLayer) -> Vec<render_ir::TextAnimatorSpec> {
    let Some(animator) = layer.text_data.get("text_animator") else {
        return Vec::new();
    };

    let selector = &animator["selector"];
    let advanced = &selector["advanced"];
    let opacity = prop_f32(layer, "anim_opacity").unwrap_or_else(|| {
        animator
            .get("opacity")
            .and_then(Value::as_f64)
            .unwrap_or(100.0) as f32
    });
    let start_keyframes = prop_f32_keyframes(layer, "reveal").unwrap_or_default();
    let end_keyframes = prop_f32_keyframes(layer, "reveal_end")
        .or_else(|| prop_f32_keyframes(layer, "text_reveal_end"))
        .unwrap_or_default();
    let start = prop_f32(layer, "reveal").unwrap_or_else(|| {
        selector
            .get("percentStart")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32
    });
    let end = selector
        .get("percentEnd")
        .and_then(Value::as_f64)
        .unwrap_or(100.0) as f32;

    vec![render_ir::TextAnimatorSpec {
        name: animator
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Animator")
            .to_string(),
        opacity,
        position: text_animator_vec2(animator, "ADBE Text Position 3D"),
        scale: text_animator_vec2(animator, "ADBE Text Scale 3D"),
        rotation: text_animator_f32(animator, "ADBE Text Rotation"),
        blur: text_animator_vec2(animator, "ADBE Text Blur"),
        selector: render_ir::TextRangeSelector {
            start,
            end,
            start_keyframes,
            end_keyframes,
            based_on: based_on_code(
                advanced
                    .get("basedOn")
                    .and_then(Value::as_i64)
                    .unwrap_or(1),
            ),
            smoothness: advanced
                .get("smoothness")
                .and_then(Value::as_f64)
                .unwrap_or(100.0) as f32,
        },
        expression_selector: animator
            .pointer("/expressible_selector/amount/expression")
            .and_then(Value::as_str)
            .and_then(text_expression_selector_from_expression),
    }]
}

fn text_animator_property_value<'a>(animator: &'a Value, match_name: &str) -> Option<&'a Value> {
    animator
        .get("properties")?
        .as_array()?
        .iter()
        .find(|property| property.get("match_name").and_then(Value::as_str) == Some(match_name))?
        .get("value")
}

fn text_animator_vec2(animator: &Value, match_name: &str) -> Option<[f32; 2]> {
    value_to_vec2(text_animator_property_value(animator, match_name)?)
}

fn text_animator_f32(animator: &Value, match_name: &str) -> Option<f32> {
    text_animator_property_value(animator, match_name)?
        .as_f64()
        .map(|value| value as f32)
}

fn text_expression_selector_from_expression(
    expression: &str,
) -> Option<render_ir::TextExpressionSelector> {
    if !(expression.contains("myDelay")
        && expression.contains("textIndex")
        && expression.contains("Math.cos")
        && expression.contains("Math.exp"))
    {
        return None;
    }

    Some(render_ir::TextExpressionSelector::PerCharacterBounce {
        delay: extract_js_assignment(expression, "delay").unwrap_or(0.05),
        freq: extract_js_assignment(expression, "freq").unwrap_or(2.0),
        amplitude: extract_js_assignment(expression, "amplitude").unwrap_or(100.0),
        decay: extract_js_assignment(expression, "decay").unwrap_or(8.0),
        source: expression.to_string(),
    })
}

fn based_on_code(code: i64) -> render_ir::TextSelectorBasedOn {
    match code {
        3 => render_ir::TextSelectorBasedOn::Words,
        4 => render_ir::TextSelectorBasedOn::Lines,
        _ => render_ir::TextSelectorBasedOn::Characters,
    }
}

fn position_expression(layer: &PayloadLayer) -> Option<render_ir::PositionExpression> {
    let expression = layer.props.get("tf_position")?.expression.as_deref()?;
    if !is_edge_wobble_position_expression(expression) {
        return None;
    }

    Some(render_ir::PositionExpression::EdgeWobble {
        intro: extract_js_var(expression, "intro").unwrap_or(0.63),
        outro: extract_js_var(expression, "outro").unwrap_or(0.63),
        amp: extract_js_var(expression, "amp").unwrap_or(22.0),
        freq: extract_js_var(expression, "freq").unwrap_or(3.6),
        source: expression.to_string(),
    })
}

fn is_edge_wobble_position_expression(expression: &str) -> bool {
    expression.contains("var intro=")
        && expression.contains("var outro=")
        && expression.contains("var amp=")
        && expression.contains("var freq=")
        && expression.contains("Math.exp(-2.4")
}

fn extract_js_var(expression: &str, name: &str) -> Option<f32> {
    let needle = format!("var {name}=");
    let rest = expression.split(&needle).nth(1)?;
    let raw = rest.split(';').next()?.trim();
    raw.parse::<f32>().ok()
}

fn extract_js_assignment(expression: &str, name: &str) -> Option<f32> {
    let compact = expression.replace(' ', "");
    let needle = format!("{name}=");
    let rest = compact.split(&needle).nth(1)?;
    let raw = rest.split(';').next()?.trim();
    raw.parse::<f32>().ok()
}

fn prop_vec2(layer: &PayloadLayer, name: &str) -> Option<[f32; 2]> {
    value_to_vec2(&layer.props.get(name)?.value)
}

fn prop_f32(layer: &PayloadLayer, name: &str) -> Option<f32> {
    layer
        .props
        .get(name)?
        .value
        .as_f64()
        .map(|value| value as f32)
}

fn prop_vec2_keyframes(layer: &PayloadLayer, name: &str) -> Option<Vec<render_ir::Vec2Keyframe>> {
    let prop = layer.props.get(name)?;
    let mut keyframes: Vec<_> = prop
        .keyframes
        .iter()
        .filter_map(|key| {
            let value = value_to_vec2(&key.v)?;
            let (hold, approximate) = keyframe_flags(key);
            Some(render_ir::Vec2Keyframe {
                time: key.t,
                value,
                hold,
                approximate,
            })
        })
        .collect();
    keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
    Some(keyframes)
}

fn prop_f32_keyframes(layer: &PayloadLayer, name: &str) -> Option<Vec<render_ir::ScalarKeyframe>> {
    let prop = layer.props.get(name)?;
    let mut keyframes: Vec<_> = prop
        .keyframes
        .iter()
        .filter_map(|key| {
            let value = key.v.as_f64()? as f32;
            let (hold, approximate) = keyframe_flags(key);
            Some(render_ir::ScalarKeyframe {
                time: key.t,
                value,
                hold,
                approximate,
            })
        })
        .collect();
    keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
    Some(keyframes)
}

fn value_to_vec2(value: &Value) -> Option<[f32; 2]> {
    let arr = value.as_array()?;
    Some([arr.first()?.as_f64()? as f32, arr.get(1)?.as_f64()? as f32])
}

fn keyframe_flags(key: &KeyframeSpec) -> (bool, bool) {
    let hold = key.iit.as_deref() == Some("6614") || key.oit.as_deref() == Some("6614");
    let bezier = key.iit.as_deref() == Some("6613") || key.oit.as_deref() == Some("6613");
    let has_ease = !key.ease_in.is_empty() || !key.ease_out.is_empty();
    (hold, bezier || has_ease)
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

fn footage_source_start(layer: &PayloadLayer) -> f64 {
    let layer_start_time = layer
        .text_data
        .pointer("/layer_meta/startTime")
        .and_then(Value::as_f64)
        .unwrap_or(layer.in_point);
    (layer.in_point - layer_start_time).max(0.0)
}

fn layer_id(layer: &PayloadLayer) -> String {
    let slug = layer
        .name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
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
        arr.first()
            .and_then(Value::as_f64)
            .map(color_component_to_u8)
            .unwrap_or(fallback[0]),
        arr.get(1)
            .and_then(Value::as_f64)
            .map(color_component_to_u8)
            .unwrap_or(fallback[1]),
        arr.get(2)
            .and_then(Value::as_f64)
            .map(color_component_to_u8)
            .unwrap_or(fallback[2]),
        arr.get(3)
            .and_then(Value::as_f64)
            .map(color_component_to_u8)
            .unwrap_or(fallback[3]),
    ]
}

fn color_component_to_u8(value: f64) -> u8 {
    let scaled = if value <= 1.0 { value * 255.0 } else { value };
    scaled.round().clamp(0.0, 255.0) as u8
}

fn text_box_for(
    transform: &render_ir::Transform2D,
    font_size: f32,
    comp: &CompSpec,
) -> render_ir::Rect {
    let height = (font_size * 2.0).max(1.0);
    render_ir::Rect {
        x: transform.anchor[0] - transform.position[0],
        y: transform.anchor[1] - height / 2.0,
        w: comp.w as f32,
        h: height,
    }
}

fn effect_status(effect_name: &str) -> CapabilityStatus {
    match effect_name {
        "ADBE Drop Shadow"
        | "ADBE Glo2"
        | "ADBE Box Blur2"
        | "ADBE Geometry2"
        | "ADBE Posterize Time"
        | "ADBE Minimax"
        | "ADBE Turbulent Displace" => CapabilityStatus::Approximate,
        _ => CapabilityStatus::Unsupported,
    }
}

fn effect_detail(effect_name: &str) -> &'static str {
    match effect_name {
        "ADBE Drop Shadow" => "implemented as an approximate text/canvas shadow",
        "ADBE Glo2" => "implemented as an approximate alpha/luminance glow",
        "ADBE Box Blur2" => "implemented as an approximate RGBA box blur",
        "ADBE Geometry2" => "implemented as an approximate canvas transform",
        "ADBE Posterize Time" => "recognized as a stateless canvas-stage no-op",
        "ADBE Minimax" => "implemented as an approximate alpha/RGBA minimax",
        "ADBE Turbulent Displace" => "implemented as an approximate deterministic displacement",
        _ => "unknown effect is unsupported",
    }
}
