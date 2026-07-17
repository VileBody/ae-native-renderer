use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPayload {
    #[serde(default, rename = "payloadVersion", alias = "payload_version")]
    pub payload_version: Option<String>,
    #[serde(rename = "projectSpec", alias = "project")]
    pub project_spec: ProjectSpec,
    #[serde(rename = "compsSpec", alias = "comps")]
    pub comps_spec: Vec<CompSpec>,
    #[serde(default)]
    pub footage_layers: Vec<PayloadLayer>,
    #[serde(default)]
    pub text_layers: Vec<PayloadLayer>,
    #[serde(default, rename = "visualOps", alias = "visual_ops")]
    pub visual_ops: Vec<VisualOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualOperation {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", alias = "kind", alias = "op")]
    pub kind: String,
    #[serde(default)]
    pub target: VisualOperationTarget,
    #[serde(default)]
    pub timing: VisualOperationTiming,
    #[serde(default = "empty_object")]
    pub params: Value,
    #[serde(default)]
    pub assets: Vec<VisualOperationAsset>,
    #[serde(default = "default_required")]
    pub required: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisualOperationTarget {
    #[serde(default)]
    pub composition: Option<String>,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub place: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisualOperationTiming {
    #[serde(default)]
    pub start: Option<f64>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub end: Option<f64>,
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub offset: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualOperationAsset {
    pub role: String,
    pub path: String,
    #[serde(default)]
    pub optional: bool,
}

fn default_required() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSpec {
    #[serde(rename = "mainCompName", alias = "main_comp_name")]
    pub main_comp_name: String,
    #[serde(default, rename = "subtitlesMode", alias = "subtitles_mode")]
    pub subtitles_mode: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadLayer {
    pub name: String,
    #[serde(rename = "type", alias = "kind")]
    pub kind: String,
    #[serde(alias = "inPoint")]
    pub in_point: f64,
    #[serde(alias = "outPoint")]
    pub out_point: f64,
    #[serde(alias = "zIndex")]
    pub z_index: i32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub adjustment_layer: bool,
    #[serde(default)]
    pub props: BTreeMap<String, PropertySpec>,
    #[serde(default)]
    pub effects: BTreeMap<String, BTreeMap<String, PropertySpec>>,
    #[serde(default = "empty_object")]
    pub text_data: Value,
    #[serde(default = "empty_object")]
    pub source_rect: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn empty_object() -> Value {
    json!({})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityStatus {
    Supported,
    Ignored,
    Approximate,
    #[serde(rename = "not_implemented")]
    NotImplemented,
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
    pub visual_ops: usize,
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
    pub visual_operations: BTreeMap<String, usize>,
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
        self.findings.iter().any(|finding| {
            matches!(
                finding.status,
                CapabilityStatus::NotImplemented | CapabilityStatus::Unsupported
            )
        })
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
    let mut visual_operations = BTreeMap::new();

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
    classify_color_management(&payload.project_spec, &mut findings);

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
                if !effect_param_is_known(normalized, param_name) {
                    findings.push(CapabilityFinding {
                        status: CapabilityStatus::Unsupported,
                        feature: format!("effect_param.{normalized}.{param_name}"),
                        layer: Some(layer.name.clone()),
                        detail: format!(
                            "effect parameter '{param_name}' is not in the native registry metadata for {normalized}"
                        ),
                    });
                }
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

    for operation in &payload.visual_ops {
        let kind = operation.kind.trim();
        if kind.is_empty() {
            errors.push("visualOps[].type is required".to_string());
            continue;
        }
        *visual_operations.entry(kind.to_string()).or_insert(0) += 1;
        let (status, detail) = visual_operation_status(operation);
        findings.push(CapabilityFinding {
            status,
            feature: format!("visual_op.{kind}"),
            layer: operation.id.clone(),
            detail: detail.to_string(),
        });
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
        visual_ops: payload.visual_ops.len(),
        total_layers: payload.footage_layers.len() + payload.text_layers.len(),
    });

    let has_unsupported = findings.iter().any(|finding| {
        matches!(
            finding.status,
            CapabilityStatus::NotImplemented | CapabilityStatus::Unsupported
        )
    });
    let ok = errors.is_empty() && (!strict || !has_unsupported);

    PayloadValidationReport {
        ok,
        strict,
        summary,
        layer_types,
        effects,
        expressions,
        keyframed_properties,
        visual_operations,
        findings,
        errors,
    }
}

fn classify_color_management(project: &ProjectSpec, findings: &mut Vec<CapabilityFinding>) {
    let Some(settings) = project.extra.get("colorManagement") else {
        return;
    };
    let working_space = settings
        .get("workingSpace")
        .and_then(Value::as_str)
        .unwrap_or("none")
        .to_ascii_lowercase();
    let output_space = settings
        .get("outputSpace")
        .and_then(Value::as_str)
        .unwrap_or("srgb")
        .to_ascii_lowercase();
    let linear_blending = settings
        .get("linearBlending")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let supported = matches!(working_space.as_str(), "none" | "srgb")
        && output_space == "srgb"
        && !linear_blending;
    findings.push(CapabilityFinding {
        status: if supported {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::NotImplemented
        },
        feature: "project.color_management".to_string(),
        layer: None,
        detail: if supported {
            format!(
                "native color contract workingSpace={working_space}, linearBlending=false, outputSpace=srgb"
            )
        } else {
            format!(
                "native v1 supports workingSpace none|srgb with linearBlending=false and outputSpace=srgb; requested workingSpace={working_space}, linearBlending={linear_blending}, outputSpace={output_space}"
            )
        },
    });
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
    let mut nested_layer_items = BTreeMap::<String, Vec<(i64, render_ir::Layer)>>::new();
    let mut asset_index = 0usize;
    let precomp_sources = main_precomp_sources(payload, &main_comp.name);
    let raster_boundary_comps = root_raster_boundary_comps(payload, main_comp, &precomp_sources);

    for layer in payload
        .footage_layers
        .iter()
        .chain(payload.text_layers.iter())
    {
        let target_comp = layer_target_comp(layer).unwrap_or(&main_comp.name);
        if target_comp != main_comp.name {
            if raster_boundary_comps.contains(target_comp) {
                let Some(target_spec) = payload
                    .comps_spec
                    .iter()
                    .find(|comp| comp.name == target_comp)
                else {
                    diagnostics.skipped_layers += 1;
                    diagnostics.findings.push(CapabilityFinding {
                        status: CapabilityStatus::Unsupported,
                        feature: "import.missing_precomp_composition".to_string(),
                        layer: Some(layer.name.clone()),
                        detail: format!("layer targets unknown comp '{target_comp}'"),
                    });
                    continue;
                };
                if layer.kind == "footage" && is_audio_layer(layer) {
                    diagnostics.skipped_layers += 1;
                    diagnostics.findings.push(CapabilityFinding {
                        status: CapabilityStatus::NotImplemented,
                        feature: "import.skip_audio".to_string(),
                        layer: Some(layer.name.clone()),
                        detail: "required audio is recognized but native mux/envelope lowering is not implemented".to_string(),
                    });
                    continue;
                }
                match import_layer(layer, &mut assets, &mut asset_index, target_spec) {
                    Some(imported) => {
                        diagnostics.imported_layers += 1;
                        nested_layer_items
                            .entry(target_comp.to_string())
                            .or_default()
                            .push((main_sort_key(layer), imported));
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
                continue;
            }
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

        if layer.kind == "precomp" && dynamic_subtitle_replaces_precomp(payload, layer) {
            diagnostics.skipped_layers += 1;
            diagnostics.findings.push(CapabilityFinding {
                status: CapabilityStatus::Approximate,
                feature: "import.replace_dynamic_subtitle_precomp".to_string(),
                layer: Some(layer.name.clone()),
                detail: "empty JSX subtitle placeholder was replaced by lowered visualOps layers"
                    .to_string(),
            });
            continue;
        }

        if layer.kind == "precomp"
            && precomp_source_name(layer)
                .is_some_and(|name| precomp_has_text_children(payload, name))
            && !precomp_source_name(layer).is_some_and(|name| raster_boundary_comps.contains(name))
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
                status: CapabilityStatus::NotImplemented,
                feature: "import.skip_audio".to_string(),
                layer: Some(layer.name.clone()),
                detail: "required audio is recognized but native mux/envelope lowering is not implemented"
                    .to_string(),
            });
            continue;
        }

        match import_layer(layer, &mut assets, &mut asset_index, main_comp) {
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

    let lowered = crate::visual_lowering::lower_visual_operations(payload, main_comp);
    diagnostics.imported_layers += lowered.layers.len();
    diagnostics.findings.extend(lowered.findings);
    layer_items.extend(
        lowered
            .layers
            .into_iter()
            .map(|lowered| (lowered.sort_key, lowered.layer)),
    );

    layer_items.sort_by_key(|(sort_key, _)| *sort_key);
    let layers = layer_items
        .into_iter()
        .map(|(_, layer)| layer)
        .collect::<Vec<_>>();
    let compositions = raster_boundary_comps
        .iter()
        .filter_map(|name| {
            let comp = payload.comps_spec.iter().find(|comp| &comp.name == name)?;
            let mut items = nested_layer_items.remove(name).unwrap_or_default();
            items.sort_by_key(|(sort_key, _)| *sort_key);
            let layers = items
                .into_iter()
                .map(|(_, layer)| layer)
                .collect::<Vec<_>>();
            Some(render_ir::CompositionNode {
                composition: ir_composition(
                    comp,
                    layers.iter().any(ir_layer_motion_blur_enabled),
                    true,
                ),
                layers,
            })
        })
        .collect::<Vec<_>>();
    diagnostics.assets = assets.len();
    let comp_motion_blur_enabled = layers.iter().any(ir_layer_motion_blur_enabled);

    let scene = render_ir::Scene {
        version: "0.2-payload".to_string(),
        composition: ir_composition(main_comp, comp_motion_blur_enabled, false),
        compositions,
        assets,
        layers,
    };

    Ok(PayloadImportResult { scene, diagnostics })
}

fn root_raster_boundary_comps(
    payload: &GeneratedPayload,
    main_comp: &CompSpec,
    precomp_sources: &BTreeMap<String, &PayloadLayer>,
) -> std::collections::BTreeSet<String> {
    let known_comps = payload
        .comps_spec
        .iter()
        .map(|comp| comp.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut boundaries = payload
        .comps_spec
        .iter()
        .filter(|comp| comp.name != main_comp.name)
        .filter_map(|comp| {
            let parent = precomp_sources.get(&comp.name)?;
            let spans_main =
                parent.in_point.abs() <= 1.0e-6 && parent.out_point + 1.0e-6 >= main_comp.dur;
            spans_main.then(|| comp.name.clone())
        })
        .collect::<std::collections::BTreeSet<_>>();

    // A rasterized precomp is still a composition graph. Keep every nested
    // composition it references so the renderer never sees a dangling source.
    loop {
        let dependencies = payload
            .footage_layers
            .iter()
            .chain(payload.text_layers.iter())
            .filter(|layer| {
                layer.kind == "precomp"
                    && layer_target_comp(layer).is_some_and(|target| boundaries.contains(target))
            })
            .filter_map(precomp_source_name)
            .filter(|source| known_comps.contains(source))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut changed = false;
        for dependency in dependencies {
            changed |= boundaries.insert(dependency);
        }
        if !changed {
            break;
        }
    }
    boundaries
}

fn ir_composition(
    comp: &CompSpec,
    motion_blur_enabled: bool,
    transparent_boundary: bool,
) -> render_ir::Composition {
    render_ir::Composition {
        id: comp.name.clone(),
        width: comp.w,
        height: comp.h,
        fps: comp.fps,
        duration: comp.dur,
        // AE's composition panel colour is a root preview/output background;
        // precomps themselves composite over their parent with transparent RGBA.
        background: transparent_boundary
            .then_some([0, 0, 0, 0])
            .unwrap_or_else(|| bg_color_to_rgba(comp.bg_color)),
        motion_blur: render_ir::MotionBlurSettings {
            enabled: motion_blur_enabled,
            ..render_ir::MotionBlurSettings::default()
        },
    }
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
    payload.text_layers.iter().any(|layer| {
        matches!(layer.kind.as_str(), "text" | "adjustment")
            && layer_target_comp(layer) == Some(comp_name)
    })
}

fn dynamic_subtitle_replaces_precomp(payload: &GeneratedPayload, layer: &PayloadLayer) -> bool {
    let is_subtitle_placeholder =
        layer.name == "Текст" || precomp_source_name(layer).is_some_and(|name| name == "Текст");
    is_subtitle_placeholder
        && payload.visual_ops.iter().any(|operation| {
            matches!(
                operation.kind.as_str(),
                "subtitle.brat.v1" | "subtitle.trendy.v1"
            )
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
    comp: &CompSpec,
) -> Option<render_ir::Layer> {
    let id = layer_id(layer);
    let start = layer.in_point;
    let duration = layer.out_point - layer.in_point;
    let transform = transform_of(layer);
    let effects = effects_of(layer);

    match layer.kind.as_str() {
        "solid" => {
            let source = layer.text_data.get("solid_source").unwrap_or(&Value::Null);
            let color = color_value_to_rgba(source.get("color_rgb01"), [0, 0, 0, 255]);
            let width = source
                .get("width")
                .and_then(Value::as_f64)
                .unwrap_or(1080.0)
                .max(1.0) as f32;
            let height = source
                .get("height")
                .and_then(Value::as_f64)
                .unwrap_or(1920.0)
                .max(1.0) as f32;
            Some(render_ir::Layer::Solid {
                id,
                start,
                duration,
                blend_mode: layer_blend_mode(layer),
                color,
                rect: render_ir::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: width,
                    h: height,
                },
                transform,
                effects,
            })
        }
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
                blend_mode: layer_blend_mode(layer),
                transform,
                effects,
            })
        }
        "text" => Some(import_text_layer(layer, start, duration, transform, comp)),
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
            blend_mode: layer_blend_mode(layer),
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
    let text = normalize_ae_line_breaks(&layer.text);
    let font_size = text_base
        .get("fontSize")
        .and_then(Value::as_f64)
        .unwrap_or(64.0) as f32;
    let leading = text_base
        .get("leading")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    let mut char_styles = text_char_styles(&layer.text_data);
    fit_sparse_point_styles_to_hard_lines(
        &mut char_styles,
        text_base.get("font").and_then(Value::as_str),
        &text,
    );
    let mut effects = effects_of(layer);
    effects.push(text_paint_effect(text_base));
    render_ir::Layer::Text {
        id: layer_id(layer),
        start,
        duration,
        text: text.clone(),
        font: text_base
            .get("font")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string(),
        fontSize: font_size,
        char_styles,
        blend_mode: layer_blend_mode(layer),
        tracking: text_base
            .get("tracking")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
        leading,
        // `applyFinalTextLayoutPass` in the production JSX measures
        // `sourceRectAtTime`, then centers the layer anchor on that real glyph
        // bounds before placing it at the composition centre. Preserve that
        // point-text contract instead of centering the synthetic raster box.
        center_source_rect_y: true,
        justification: render_ir::TextJustification::Center,
        fill: color_value_to_rgba(text_base.get("fillColor"), [255, 255, 255, 255]),
        box_: Some(text_box_for(
            &layer.text_data,
            &transform,
            font_size,
            leading,
            &text,
            comp,
        )),
        transform,
        text_animators: text_animators_of(layer),
        effects,
    }
}

fn text_paint_effect(text_base: &Value) -> render_ir::EffectSpec {
    let fill_enabled = text_base
        .get("applyFill")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let stroke_enabled = text_base
        .get("applyStroke")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let fill =
        fill_enabled.then(|| color_value_to_rgba(text_base.get("fillColor"), [255, 255, 255, 255]));
    let stroke_color =
        stroke_enabled.then(|| color_value_to_rgba(text_base.get("strokeColor"), [0, 0, 0, 255]));
    let stroke_width = if stroke_enabled {
        text_base
            .get("strokeWidth")
            .and_then(Value::as_f64)
            .filter(|width| width.is_finite() && *width >= 0.0)
            .unwrap_or(0.0) as f32
    } else {
        0.0
    };
    render_ir::EffectSpec {
        match_name: render_ir::TEXT_PAINT_MATCH_NAME.to_string(),
        params: json!(render_ir::TextPaintSpec {
            fill,
            fill_enabled,
            stroke_color,
            stroke_width,
            stroke_over_fill: text_base
                .get("strokeOverFill")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
    }
}

fn text_char_styles(text_data: &Value) -> Vec<render_ir::TextCharStyle> {
    text_data
        .get("char_styles_ungrouped")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|style| {
            let index = style.get("i")?.as_u64()? as usize;
            let font_size = style
                .get("fontSize")
                .and_then(Value::as_f64)
                .map(|value| value as f32)
                .filter(|value| value.is_finite() && *value > 0.0);
            let font = style
                .get("font")
                .or_else(|| style.get("fontFamily"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|font| !font.is_empty())
                .map(ToOwned::to_owned);
            let faux_italic = style
                .get("fauxItalic")
                .or_else(|| style.get("faux_italic"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            // TextDocument sparse styles carry RGB triples. Unlike a raster
            // pixel, a missing fourth component means opaque, not transparent.
            let fill = style
                .get("fillColor")
                .map(|value| color_value_to_rgba(Some(value), [0, 0, 0, 255]));
            (font.is_some() || font_size.is_some() || fill.is_some() || faux_italic).then_some(
                render_ir::TextCharStyle {
                    index,
                    font,
                    font_size,
                    fill,
                    faux_italic,
                },
            )
        })
        .collect()
}

fn fit_sparse_point_styles_to_hard_lines(
    styles: &mut [render_ir::TextCharStyle],
    base_font: Option<&str>,
    text: &str,
) {
    // The Point SemiBold JSX scenes keep their intended two hard lines, but
    // CoolType's width is slightly narrower than fontdue's. The sparse 120px
    // focus spans can otherwise force a third line in Rust. Only cap those
    // oversized spans when an explicit line is long enough to need the fit.
    let needs_fit =
        base_font == Some("Point-SemiBold") && text.lines().any(|line| line.chars().count() >= 14);
    if !needs_fit {
        return;
    }
    for style in styles {
        if style.font.as_deref() == Some("Point-SemiBold") {
            style.font_size = style.font_size.map(|size| size.min(110.0));
        }
    }
}

fn normalize_ae_line_breaks(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn classify_layer(layer: &PayloadLayer, findings: &mut Vec<CapabilityFinding>) {
    let (status, detail) = match layer.kind.as_str() {
        "solid" => (
            CapabilityStatus::Supported,
            "solid layers are imported with color, bounds, timing, transform, and effects",
        ),
        "footage" if is_audio_layer(layer) => (
            CapabilityStatus::NotImplemented,
            "audio layers require native mux and envelope support before output is complete",
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

    if layer_motion_blur(layer) {
        findings.push(CapabilityFinding {
            status: CapabilityStatus::Approximate,
            feature: "transform.motion_blur".to_string(),
            layer: Some(layer.name.clone()),
            detail: "native temporal supersampling is enabled with fixed shutter defaults"
                .to_string(),
        });
    }
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

fn layer_motion_blur(layer: &PayloadLayer) -> bool {
    layer
        .text_data
        .pointer("/layer_meta/motionBlur")
        .or_else(|| layer.text_data.pointer("/layer_meta/motion_blur"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn layer_blend_mode(layer: &PayloadLayer) -> render_ir::BlendMode {
    if layer
        .text_data
        .pointer("/layer_meta/blendingMode")
        .or_else(|| layer.text_data.pointer("/layer_meta/blend_mode"))
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("screen"))
    {
        return render_ir::BlendMode::Screen;
    }
    let value = layer.text_data.pointer("/layer_meta/blendingModeCode");
    let code = value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    });
    match code {
        Some(5220) => render_ir::BlendMode::Add,
        Some(5233) => render_ir::BlendMode::Difference,
        _ => render_ir::BlendMode::Normal,
    }
}

fn ir_layer_motion_blur_enabled(layer: &render_ir::Layer) -> bool {
    match layer {
        render_ir::Layer::Solid { transform, .. }
        | render_ir::Layer::Footage { transform, .. }
        | render_ir::Layer::Text { transform, .. }
        | render_ir::Layer::Precomp { transform, .. } => transform.motion_blur,
        render_ir::Layer::Adjustment { .. } => false,
    }
}

fn main_sort_key(layer: &PayloadLayer) -> i64 {
    i64::from(layer.z_index) * 10_000
}

fn flattened_sort_key(parent: &PayloadLayer, child: &PayloadLayer) -> i64 {
    main_sort_key(parent) + i64::from(child.z_index.clamp(0, 9_999))
}

fn normalize_effect_name(effect_name: &str) -> &str {
    let name = effect_name
        .split_once(':')
        .map_or(effect_name, |(_, name)| name);
    match name {
        // Sapphire's drop shadow is lowered to the native shadow kernel. Its
        // parameter IDs are handled by DropShadowParams as an approximation.
        "S_DropShadow" => "ADBE Drop Shadow",
        _ => name,
    }
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
        motion_blur: layer_motion_blur(layer),
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
        motion_blur: parent.motion_blur || child.motion_blur,
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
                ease: key.ease,
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
                ease: key.ease,
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
                ease: key.ease,
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
                ease: key.ease,
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
                ease: key.ease,
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
                ease: key.ease,
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
            based_on: based_on_code(advanced.get("basedOn").and_then(Value::as_i64).unwrap_or(1)),
            smoothness: advanced
                .get("smoothness")
                .and_then(Value::as_f64)
                .unwrap_or(100.0) as f32,
            shape: selector_shape_code(advanced.get("shape").and_then(Value::as_i64).unwrap_or(1)),
            randomize_order: advanced
                .get("randomizeOrder")
                .and_then(Value::as_i64)
                .map(|value| value != 0)
                .unwrap_or(false),
            wiggly: text_wiggly_selector(advanced),
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

fn selector_shape_code(code: i64) -> render_ir::TextSelectorShape {
    match code {
        2 => render_ir::TextSelectorShape::RampUp,
        3 => render_ir::TextSelectorShape::RampDown,
        4 => render_ir::TextSelectorShape::Triangle,
        5 => render_ir::TextSelectorShape::Round,
        6 => render_ir::TextSelectorShape::Smooth,
        _ => render_ir::TextSelectorShape::Square,
    }
}

fn text_wiggly_selector(advanced: &Value) -> Option<render_ir::TextWigglySelector> {
    let amount = advanced
        .get("wigglyAmount")
        .or_else(|| advanced.get("amount"))
        .and_then(Value::as_f64)? as f32;
    let frequency = advanced
        .get("wigglesPerSecond")
        .or_else(|| advanced.get("frequency"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0) as f32;
    let seed = advanced
        .get("randomSeed")
        .or_else(|| advanced.get("seed"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    Some(render_ir::TextWigglySelector {
        amount,
        frequency,
        seed,
    })
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
                ease: keyframe_ease(key),
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
                ease: keyframe_ease(key),
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

fn keyframe_ease(key: &KeyframeSpec) -> Option<render_ir::KeyframeEase> {
    let (_, approximate) = keyframe_flags(key);
    if !approximate {
        return None;
    }

    let out_influence = key
        .ease_out
        .first()
        .and_then(ease_influence)
        .unwrap_or(33.333);
    let in_influence = key
        .ease_in
        .first()
        .and_then(ease_influence)
        .unwrap_or(33.333);
    Some(render_ir::KeyframeEase {
        x1: (out_influence / 100.0).clamp(0.05, 0.95),
        y1: 0.0,
        x2: (1.0 - in_influence / 100.0).clamp(0.05, 0.95),
        y2: 1.0,
    })
}

fn ease_influence(value: &Value) -> Option<f32> {
    value
        .get("influence")
        .or_else(|| value.get("i"))
        .and_then(Value::as_f64)
        .map(|value| value as f32)
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
    text_data: &Value,
    transform: &render_ir::Transform2D,
    font_size: f32,
    leading: Option<f32>,
    text: &str,
    comp: &CompSpec,
) -> render_ir::Rect {
    let explicit_box = text_data
        .get("box_text")
        .and_then(Value::as_array)
        .and_then(|box_text| {
            let width = box_text.first()?.as_f64()? as f32;
            let height = box_text.get(1)?.as_f64()? as f32;
            (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
                .then_some((width, height))
        });
    let line_count = text.lines().count().max(1) as f32;
    let line_height = leading.unwrap_or(font_size * 1.2).max(font_size);
    let (width, height) = explicit_box.unwrap_or_else(|| {
        (
            comp.w as f32,
            (line_height * line_count).max(font_size * 2.0).max(1.0),
        )
    });
    render_ir::Rect {
        // The JSX gives boxText dimensions in the local TextDocument space;
        // sourceRect centering then places that rectangle around the layer's
        // anchor/position pair. Flattened precomps keep the same local origin.
        x: transform.anchor[0] - width / 2.0,
        y: transform.anchor[1] - height / 2.0,
        w: width,
        h: height,
    }
}

fn effect_status(effect_name: &str) -> CapabilityStatus {
    match effect_name {
        "ADBE Drop Shadow"
        | "ADBE Glo2"
        | "ADBE Box Blur2"
        | "ADBE Gaussian Blur 2"
        | "ADBE Geometry2"
        | "ADBE Invert"
        | "ADBE Motion Blur"
        | "ADBE Optics Compensation"
        | "ADBE Posterize Time"
        | "ADBE Minimax"
        | "ADBE Turbulent Displace"
        | "CC Image Wipe"
        | "ANR Analog Glitch"
        | "ANR F3 Stylize"
        | "ANR Shape Overlay"
        | "ANR Vertical Gradient"
        | "ANR Text Paint" => CapabilityStatus::Approximate,
        _ => CapabilityStatus::Unsupported,
    }
}

fn effect_detail(effect_name: &str) -> &'static str {
    match effect_name {
        "ADBE Drop Shadow" => "implemented as an approximate text/canvas shadow",
        "ADBE Glo2" => "implemented as an approximate alpha/luminance glow",
        "ADBE Box Blur2" => "implemented as an approximate RGBA box blur",
        "ADBE Gaussian Blur 2" => "implemented as an approximate AE-style Gaussian blur",
        "ADBE Geometry2" => "implemented as an approximate canvas transform",
        "ADBE Motion Blur" => "implemented as a native directional premultiplied blur",
        "ADBE Optics Compensation" => {
            "implemented as a native radial distortion with premultiplied sampling"
        }
        "ADBE Invert" => "implemented for AE RGB, HLS, YIQ, alpha, and component channels",
        "CC Image Wipe" => "implemented for the production Brat luminance/coverage gradient path",
        "ADBE Posterize Time" => concat!(
            "implemented as temporal quantization in render-core; ",
            "canvas stage is pass-through"
        ),
        "ADBE Minimax" => "implemented as an approximate alpha/RGBA minimax",
        "ADBE Turbulent Displace" => "implemented as an approximate deterministic displacement",
        "ANR Analog Glitch" => "native approximation of the production analog-glitch stack",
        "ANR F3 Stylize" => "native extract/xerox/neon/old-camera approximation",
        "ANR Shape Overlay" => "native procedural F2/F4 vector overlay",
        "ANR Vertical Gradient" => "native approximation of the production Sapphire gradient",
        "ANR Text Paint" => "native fill/stroke paint marker consumed during text rasterization",
        _ => "unknown effect is unsupported",
    }
}

fn visual_operation_status(operation: &VisualOperation) -> (CapabilityStatus, &'static str) {
    match operation.kind.as_str() {
        "subtitle.brat.v1" => (
            CapabilityStatus::Approximate,
            "native glyph tracking, leading, full justification, Difference/Add, Minimax, Gaussian Blur, shadow, and BPM Image Wipe are lowered; flattened precomp and final pixel parity remain approximate",
        ),
        "subtitle.trendy.v1" => (
            CapabilityStatus::Approximate,
            "tracking 7->-1, unified fill/stroke TextDocument, fit, gradient, shadow, and analog stack are lowered; proprietary Sapphire pixel parity remains approximate",
        ),
        "subtitle.bot.impulse_2nd.v1"
        | "subtitle.bot.scenes_3rd.v1"
        | "subtitle.bot.scenes_3rd_single_step.v1"
        | "subtitle.bot.template_4th.v1" => (
            CapabilityStatus::Approximate,
            "bot planner segments lower directly to native text/reveal layers; source family typography and AE effect-stack parity remain approximate",
        ),
        "subtitle.bot.legacy_blocks.v1" => (
            CapabilityStatus::NotImplemented,
            "legacy macro-block renderer is intentionally out of native P0/P1 scope; request is preserved and rejected explicitly",
        ),
        "style.semantic.v1" => (
            CapabilityStatus::Approximate,
            "frozen bot semantic style IDs lower to deterministic native effect-stack approximations",
        ),
        "hook.f3.effect.v1" if f3_is_flash_on_cuts_only(operation) => (
            CapabilityStatus::Approximate,
            "flash_on_cuts is lowered to native timed Add solids with AE opacity timing",
        ),
        "hook.f3.effect.v1" if f3_is_analog_only(operation) => (
            CapabilityStatus::Approximate,
            "analog_glitch is lowered to native posterize, red tone, wave, calibrated highlight-selective 4x8 CRT grid, and glow stages",
        ),
        "hook.f3.effect.v1" if f3_all_effects_are_native(operation) => (
            CapabilityStatus::Approximate,
            "all requested active F3 ids lower to deterministic native Rust primitives; final AE pixel parity remains approximate",
        ),
        "hook.f1.sound.v1" => (
            CapabilityStatus::Approximate,
            "native drop-light visual chain is lowered; supplied audio is promoted after local multi-track mux verification",
        ),
        "hook.f2.object.v1" => (
            CapabilityStatus::Approximate,
            "square, ellipse/elipse, rhomb, star1, and star2 lower to native procedural overlays with a drop-light chain",
        ),
        "hook.f4.motion.v1" => (
            CapabilityStatus::Approximate,
            "head, pinch, holdfinger, tap, and swipe lower to native procedural gesture overlays",
        ),
        "hook.f5.cognition.v1" => (
            CapabilityStatus::Approximate,
            "native light and supplied word-timed subtitle clone are lowered; local tts_audio is promoted after mux verification",
        ),
        "hook.f3.effect.v1" => (
            CapabilityStatus::NotImplemented,
            "one or more requested F3 ids are not part of the active native palette",
        ),
        _ => (
            CapabilityStatus::Unsupported,
            "unknown visual operation; it is preserved in diagnostics and never silently dropped",
        ),
    }
}

fn effect_param_is_known(effect_name: &str, param_name: &str) -> bool {
    if is_ae_numeric_param(param_name) {
        return true;
    }
    let Some(known) = effect_known_params(effect_name) else {
        return true;
    };
    known.contains(&param_name)
}

fn is_ae_numeric_param(param_name: &str) -> bool {
    let bytes = param_name.as_bytes();
    bytes.len() == 4 && bytes.iter().all(u8::is_ascii_digit)
}

fn effect_known_params(effect_name: &str) -> Option<&'static [&'static str]> {
    match effect_name {
        "ANR Analog Glitch" => Some(&[
            "contrast",
            "red_gain",
            "wave_amplitude",
            "wave_width",
            "grid_w",
            "grid_h",
            "glow_radius",
        ]),
        "ANR F3 Stylize" => Some(&[
            "mode",
            "amount",
            "threshold",
            "softness",
            "composite_original",
            "magentas",
            "tint",
            "tint_black",
            "height",
            "width",
            "speed",
        ]),
        "ANR Shape Overlay" => Some(&[
            "shape",
            "opacity",
            "thickness",
            "size",
            "fill",
            "stroke",
            "seed",
        ]),
        "ANR Vertical Gradient" => Some(&[
            "text_paint",
            "top",
            "bottom",
            "brightness",
            "start_xy",
            "end_xy",
            "source_match_name",
            "sapphire_params",
        ]),
        "ADBE Box Blur2" => Some(&[
            "radius",
            "iterations",
            "repeat_edge_pixels",
            "horizontal",
            "vertical",
        ]),
        "ADBE Drop Shadow" => Some(&[
            "color",
            "opacity",
            "direction",
            "distance",
            "softness",
            "source_match_name",
            "sapphire_params",
        ]),
        "ADBE Motion Blur" => Some(&["direction", "blur_length"]),
        "ADBE Gaussian Blur 2" => Some(&["blurriness", "repeat_edge_pixels"]),
        "ADBE Glo2" => Some(&[
            "threshold",
            "radius",
            "intensity",
            "operation",
            "color",
            "based_on",
            "composite_original",
        ]),
        "CC Image Wipe" => Some(&["completion", "border_softness"]),
        "ADBE Invert" => Some(&["channel", "blend_with_original"]),
        "ADBE Minimax" => Some(&["operation", "channels", "direction", "radius"]),
        "ADBE Optics Compensation" => Some(&["field_of_view", "reverse", "center"]),
        "ADBE Posterize Time" => Some(&["frameRate", "frame_rate"]),
        "ADBE Geometry2" => Some(&[
            "anchor",
            "position",
            "scale",
            "scale_width",
            "scale_height",
            "rotation",
            "opacity",
        ]),
        "ADBE Turbulent Displace" => Some(&[
            "amount",
            "size",
            "offset",
            "complexity",
            "evolution",
            "seed",
            "pinning",
            "resize_layer",
        ]),
        _ => None,
    }
}

fn f3_is_flash_on_cuts_only(operation: &VisualOperation) -> bool {
    let Some(ids) = operation
        .params
        .get("detected_effect_ids")
        .and_then(Value::as_array)
    else {
        return false;
    };
    !ids.is_empty() && ids.iter().all(|id| id.as_str() == Some("flash_on_cuts"))
}

fn f3_is_analog_only(operation: &VisualOperation) -> bool {
    let Some(ids) = operation
        .params
        .get("detected_effect_ids")
        .and_then(Value::as_array)
    else {
        return false;
    };
    !ids.is_empty() && ids.iter().all(|id| id.as_str() == Some("analog_glitch"))
}

fn f3_all_effects_are_native(operation: &VisualOperation) -> bool {
    let ids = f3_effect_ids(operation);
    !ids.is_empty()
        && ids.iter().all(|id| {
            matches!(
                id.as_str(),
                "hook_light"
                    | "shutter_effect"
                    | "flash_slow_shutter"
                    | "negative_zoom"
                    | "snap_wipe"
                    | "minimax"
                    | "invert_flash"
                    | "extract_flash"
                    | "flash_on_cuts"
                    | "layer_shake"
                    | "xerox"
                    | "analog_glitch"
                    | "neon_extract"
                    | "old_camera"
                    | "blackwhite"
                    | "crystal_glow"
                    | "night_vision"
                    | "wave"
            )
        })
}

fn f3_effect_ids(operation: &VisualOperation) -> Vec<String> {
    let mut ids = operation
        .params
        .get("detected_effect_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    for key in ["hook", "transition", "extra", "effect", "device"] {
        let Some(value) = operation.params.get(key) else {
            continue;
        };
        match value {
            Value::String(value) => ids.push(value.clone()),
            Value::Array(values) => {
                ids.extend(values.iter().filter_map(Value::as_str).map(str::to_string))
            }
            Value::Object(value) => {
                if let Some(id) = value
                    .get("id")
                    .or_else(|| value.get("name"))
                    .and_then(Value::as_str)
                {
                    ids.push(id.to_string());
                }
            }
            _ => {}
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod color_management_tests {
    use super::*;
    use serde_json::json;

    fn project(settings: Value) -> ProjectSpec {
        ProjectSpec {
            main_comp_name: "Comp 1".to_string(),
            subtitles_mode: None,
            extra: BTreeMap::from([("colorManagement".to_string(), settings)]),
        }
    }

    #[test]
    fn unmanaged_srgb_output_is_supported() {
        let mut findings = Vec::new();
        classify_color_management(
            &project(json!({
                "workingSpace":"none",
                "linearBlending":false,
                "outputSpace":"srgb"
            })),
            &mut findings,
        );
        assert!(matches!(findings[0].status, CapabilityStatus::Supported));
    }

    #[test]
    fn wide_gamut_or_linear_blending_stays_explicit() {
        let mut findings = Vec::new();
        classify_color_management(
            &project(json!({
                "workingSpace":"acescg",
                "linearBlending":true,
                "outputSpace":"display-p3"
            })),
            &mut findings,
        );
        assert!(matches!(
            findings[0].status,
            CapabilityStatus::NotImplemented
        ));
    }

    #[test]
    fn unknown_effect_param_is_reported_explicitly() {
        let payload = GeneratedPayload {
            project_spec: ProjectSpec {
                main_comp_name: "Comp 1".to_string(),
                subtitles_mode: None,
                extra: BTreeMap::new(),
            },
            comps_spec: vec![CompSpec {
                name: "Comp 1".to_string(),
                w: 64,
                h: 64,
                fps: 24.0,
                dur: 1.0,
                pixel_aspect: None,
                work_area_start: None,
                work_area_duration: None,
                display_start_time: None,
                bg_color: None,
                extra: BTreeMap::new(),
            }],
            footage_layers: vec![PayloadLayer {
                name: "adjustment".to_string(),
                kind: "adjustment".to_string(),
                in_point: 0.0,
                out_point: 1.0,
                z_index: 1,
                text: String::new(),
                adjustment_layer: true,
                props: BTreeMap::new(),
                effects: BTreeMap::from([(
                    "ADBE Drop Shadow".to_string(),
                    BTreeMap::from([(
                        "mystery".to_string(),
                        PropertySpec {
                            match_name: None,
                            value: Value::Null,
                            keyframes: Vec::new(),
                            expression: None,
                            extra: BTreeMap::new(),
                        },
                    )]),
                )]),
                text_data: Value::Null,
                source_rect: json!({}),
                extra: BTreeMap::new(),
            }],
            text_layers: Vec::new(),
            visual_ops: Vec::new(),
            payload_version: None,
        };

        let report = validate_payload(&payload, true);
        assert!(!report.ok);
        assert!(report.findings.iter().any(|finding| {
            finding.status == CapabilityStatus::Unsupported
                && finding.feature == "effect_param.ADBE Drop Shadow.mystery"
                && finding.layer.as_deref() == Some("adjustment")
        }));
    }

    #[test]
    fn jsx_carriage_returns_become_layout_line_breaks() {
        assert_eq!(
            normalize_ae_line_breaks("FIRST\rSECOND\r\nTHIRD"),
            "FIRST\nSECOND\nTHIRD"
        );
    }

    #[test]
    fn text_document_font_size_overrides_keep_ae_indices_across_line_breaks() {
        let styles = text_char_styles(&json!({
            "char_styles_ungrouped": [
                {"i": 0, "font": "Point-SemiBold"},
                {"i": 10, "fontSize": 120},
                {"i": 12, "fontSize": 120},
                {"i": 13, "fontSize": 120}
            ]
        }));
        assert_eq!(
            styles
                .iter()
                .map(|style| (
                    style.index,
                    style.font.as_deref(),
                    style.font_size,
                    style.faux_italic
                ))
                .collect::<Vec<_>>(),
            vec![
                (0, Some("Point-SemiBold"), None, false),
                (10, None, Some(120.0), false),
                (12, None, Some(120.0), false),
                (13, None, Some(120.0), false)
            ]
        );
    }

    #[test]
    fn text_document_preserves_faux_italic_without_font_override() {
        let styles = text_char_styles(&json!({
            "char_styles_ungrouped": [{"i": 3, "fauxItalic": true}]
        }));
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].index, 3);
        assert!(styles[0].faux_italic);
        assert_eq!(styles[0].font, None);
    }

    #[test]
    fn text_document_preserves_opaque_rgb_focus_fill() {
        let styles = text_char_styles(&json!({
            "char_styles_ungrouped": [{"i": 3, "fillColor": [0.898, 0.082, 0.082]}]
        }));
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].index, 3);
        assert_eq!(styles[0].fill, Some([229, 21, 21, 255]));
    }

    #[test]
    fn long_point_semibold_hard_line_caps_sparse_focus_size() {
        let mut styles = vec![render_ir::TextCharStyle {
            index: 8,
            font: Some("Point-SemiBold".to_string()),
            font_size: Some(120.0),
            fill: None,
            faux_italic: false,
        }];
        fit_sparse_point_styles_to_hard_lines(
            &mut styles,
            Some("Point-SemiBold"),
            "ТЕБЕ НЕ\nПЛАТЯТ ПРОЦЕНТЫ",
        );
        assert_eq!(styles[0].font_size, Some(110.0));
    }

    #[test]
    fn short_point_semibold_line_keeps_sparse_focus_size() {
        let mut styles = vec![render_ir::TextCharStyle {
            index: 0,
            font: Some("Point-SemiBold".to_string()),
            font_size: Some(120.0),
            fill: None,
            faux_italic: false,
        }];
        fit_sparse_point_styles_to_hard_lines(&mut styles, Some("Point-SemiBold"), "СЕН-ЛОРАНЕ");
        assert_eq!(styles[0].font_size, Some(120.0));
    }

    #[test]
    fn text_document_preserves_outline_only_paint() {
        let effect = text_paint_effect(&json!({
            "applyFill": false,
            "applyStroke": true,
            "strokeColor": [1.0, 1.0, 1.0],
            "strokeWidth": 5,
            "strokeOverFill": false
        }));
        let paint = render_ir::TextPaintSpec::from_effect(&effect).unwrap();
        assert!(!paint.fill_enabled);
        assert_eq!(paint.fill, None);
        assert_eq!(paint.stroke_color, Some([255, 255, 255, 255]));
        assert_eq!(paint.stroke_width, 5.0);
    }

    #[test]
    fn text_box_keeps_every_explicit_line_inside_the_raster() {
        let transform = render_ir::Transform2D {
            anchor: [540.0, 960.0],
            position: [540.0, 980.0],
            ..render_ir::Transform2D::default()
        };
        let comp = CompSpec {
            name: "Comp 1".to_string(),
            w: 1080,
            h: 1960,
            fps: 24.0,
            dur: 1.0,
            pixel_aspect: None,
            work_area_start: None,
            work_area_duration: None,
            display_start_time: None,
            bg_color: None,
            extra: BTreeMap::new(),
        };
        let rect = text_box_for(
            &Value::Null,
            &transform,
            80.0,
            Some(114.0),
            "FIRST\nSECOND",
            &comp,
        );
        assert_eq!(rect.h, 228.0);
    }

    #[test]
    fn text_document_box_text_is_preserved_for_wrapping() {
        let transform = render_ir::Transform2D::default();
        let comp = CompSpec {
            name: "Comp 1".to_string(),
            w: 1080,
            h: 1920,
            fps: 24.0,
            dur: 1.0,
            pixel_aspect: None,
            work_area_start: None,
            work_area_duration: None,
            display_start_time: None,
            bg_color: None,
            extra: BTreeMap::new(),
        };
        let rect = text_box_for(
            &json!({"box_text": [900, 160]}),
            &transform,
            60.0,
            Some(80.0),
            "ПРИЛОЖИЛИ ТЕ КТО ДОСТИГ",
            &comp,
        );
        assert_eq!(
            (rect.x, rect.y, rect.w, rect.h),
            (-450.0, -80.0, 900.0, 160.0)
        );
    }
}
