use ae_bridge::{CapabilityFinding, CapabilityStatus, GeneratedPayload};
use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub const REQUEST_SCHEMA: &str = "ae-native-renderer.render-request.v1";
pub const RESPONSE_SCHEMA: &str = "ae-native-renderer.render-response.v1";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RenderJsonRequest {
    pub schema: String,
    #[serde(default, rename = "requestId", alias = "request_id")]
    pub request_id: Option<String>,
    pub action: JsonAction,
    #[serde(flatten)]
    pub payload: GeneratedPayload,
    #[serde(default, rename = "assetsSpec", alias = "assets_spec")]
    pub assets: AssetsSpec,
    #[serde(default, rename = "outputSpec", alias = "output_spec")]
    pub output: OutputSpec,
    #[serde(default, rename = "debugSpec", alias = "debug_spec")]
    pub debug: DebugSpec,
    #[serde(default, rename = "tuningSpec", alias = "tuning_spec")]
    pub tuning: crate::tuning::TuningSpec,
    #[serde(default)]
    pub policy: RenderPolicy,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JsonAction {
    Validate,
    Inspect,
    #[default]
    Render,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AssetsSpec {
    #[serde(default)]
    pub root: Option<PathBuf>,
    #[serde(default, rename = "jobArchive", alias = "job_archive")]
    pub job_archive: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputSpec {
    #[serde(default = "default_output_directory")]
    pub directory: PathBuf,
    #[serde(default)]
    pub video: Option<PathBuf>,
    #[serde(
        default,
        deserialize_with = "deserialize_frame_selection",
        skip_serializing_if = "Option::is_none"
    )]
    pub frames: Option<Vec<u32>>,
    #[serde(default = "default_true", rename = "writeScene", alias = "write_scene")]
    pub write_scene: bool,
}

impl Default for OutputSpec {
    fn default() -> Self {
        Self {
            directory: default_output_directory(),
            video: None,
            frames: None,
            write_scene: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DebugSpec {
    #[serde(
        default,
        rename = "captureEffectStages",
        alias = "capture_effect_stages"
    )]
    pub capture_effect_stages: bool,
    #[serde(default, rename = "sourceLayers", alias = "source_layers")]
    pub source_layers: bool,
    #[serde(default, rename = "preEffects", alias = "pre_effects")]
    pub pre_effects: bool,
    #[serde(default, rename = "textMasks", alias = "text_masks")]
    pub text_masks: bool,
    #[serde(default, rename = "adjustmentResults", alias = "adjustment_results")]
    pub adjustment_results: bool,
    #[serde(default, rename = "precompResults", alias = "precomp_results")]
    pub precomp_results: bool,
    #[serde(default, rename = "finalComposite", alias = "final_composite")]
    pub final_composite: bool,
}

impl DebugSpec {
    fn stage_debug_spec(&self) -> render_core::layer_eval::StageDebugSpec {
        render_core::layer_eval::StageDebugSpec {
            effect_stages: self.capture_effect_stages,
            source_layers: self.source_layers,
            pre_effects: self.pre_effects,
            text_masks: self.text_masks,
            adjustment_results: self.adjustment_results,
            precomp_results: self.precomp_results,
            final_composite: self.final_composite,
        }
    }
}

fn deserialize_frame_selection<'de, D>(deserializer: D) -> Result<Option<Vec<u32>>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut frames = Option::<Vec<u32>>::deserialize(deserializer)?;
    if let Some(frames) = frames.as_mut() {
        frames.sort_unstable();
        frames.dedup();
    }
    Ok(frames)
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RenderPolicy {
    #[serde(default, rename = "onUnsupported", alias = "on_unsupported")]
    pub on_unsupported: UnsupportedPolicy,
    #[serde(default)]
    pub strict: bool,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedPolicy {
    #[default]
    Report,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderJsonResponse {
    pub schema: &'static str,
    pub request_id: Option<String>,
    pub request_schema: Option<String>,
    pub request_hash: Option<String>,
    pub action: JsonAction,
    pub status: ResponseStatus,
    pub ok: bool,
    pub complete: bool,
    pub capabilities: CapabilityReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneSummary>,
    pub artifacts: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render: Option<RenderSummary>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Validated,
    Inspected,
    Rendered,
    Partial,
    Unsupported,
    Invalid,
    Failed,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CapabilityReport {
    pub complete: bool,
    pub supported: Vec<CapabilityItem>,
    pub approximate: Vec<CapabilityItem>,
    pub ignored: Vec<CapabilityItem>,
    pub not_implemented: Vec<CapabilityItem>,
    pub unsupported: Vec<CapabilityItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityItem {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub required: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneSummary {
    pub composition: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub frame_count: u64,
    pub layers: usize,
    pub assets: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderSummary {
    pub elapsed_ms: u128,
    pub frames_directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonApiExit {
    Success,
    Config,
    Render,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AudioCapabilityKey {
    code: String,
    subject: Option<String>,
}

#[derive(Debug, Clone)]
struct AudioDuckEnvelope {
    start: f64,
    end: f64,
    amount_db: f64,
    attack: f64,
    release: f64,
}

#[derive(Debug, Clone)]
struct AudioTrackPlan {
    track_id: String,
    subject: String,
    requested_path: String,
    fallback_file_name: Option<String>,
    role: String,
    required: bool,
    background: bool,
    timeline_start: f64,
    source_start: f64,
    duration: f64,
    composition_duration: f64,
    layer_duration: f64,
    level_db: f64,
    fade_in: f64,
    fade_out: f64,
    min_db: f64,
    duck: Option<AudioDuckEnvelope>,
    capability_keys: Vec<AudioCapabilityKey>,
}

#[derive(Debug, Clone)]
struct ResolvedAudioTrack {
    source: PathBuf,
    plan: AudioTrackPlan,
}

pub fn run(request_path: &str, response_path: Option<&Path>) -> anyhow::Result<JsonApiExit> {
    let (raw, base_dir) = match read_request(request_path) {
        Ok(value) => value,
        Err(error) => {
            let response = failure_response(JsonAction::Render, ResponseStatus::Invalid, error);
            emit_response(&response, response_path)?;
            return Ok(JsonApiExit::Config);
        }
    };
    let request: RenderJsonRequest = match serde_json::from_str(&raw) {
        Ok(request) => request,
        Err(error) => {
            let response = failure_response(
                JsonAction::Render,
                ResponseStatus::Invalid,
                anyhow::anyhow!("invalid render request JSON: {error}"),
            );
            emit_response(&response, response_path)?;
            return Ok(JsonApiExit::Config);
        }
    };

    let result = execute(request, &base_dir);
    let exit = result.1;
    emit_response(&result.0, response_path)?;
    Ok(exit)
}

fn execute(request: RenderJsonRequest, base_dir: &Path) -> (RenderJsonResponse, JsonApiExit) {
    let action = request.action;
    let request_hash = serde_json::to_vec(&request)
        .map(|canonical| sha256_hex(&canonical))
        .unwrap_or_else(|_| sha256_hex(request.schema.as_bytes()));
    if request.schema != REQUEST_SCHEMA {
        return (
            RenderJsonResponse {
                schema: RESPONSE_SCHEMA,
                request_id: request.request_id,
                request_schema: Some(request.schema.clone()),
                request_hash: Some(request_hash),
                action,
                status: ResponseStatus::Invalid,
                ok: false,
                complete: false,
                capabilities: CapabilityReport::default(),
                scene: None,
                artifacts: BTreeMap::new(),
                render: None,
                errors: vec![format!(
                    "unsupported request schema '{}'; expected '{}'",
                    request.schema, REQUEST_SCHEMA
                )],
                warnings: Vec::new(),
            },
            JsonApiExit::Config,
        );
    }
    if let Some(error) = output_spec_config_error(&request.output) {
        return config_failure_response(
            &request,
            &request_hash,
            CapabilityReport::default(),
            None,
            error,
        );
    }

    let validation = ae_bridge::validate_payload(&request.payload, false);
    if !validation.errors.is_empty() {
        return (
            RenderJsonResponse {
                schema: RESPONSE_SCHEMA,
                request_id: request.request_id,
                request_schema: Some(request.schema),
                request_hash: Some(request_hash),
                action,
                status: ResponseStatus::Invalid,
                ok: false,
                complete: false,
                capabilities: capabilities_from_findings(&validation.findings, None),
                scene: None,
                artifacts: BTreeMap::new(),
                render: None,
                errors: validation.errors,
                warnings: Vec::new(),
            },
            JsonApiExit::Config,
        );
    }

    let mut tuning = match crate::tuning::resolve(&request.tuning, base_dir) {
        Ok(tuning) => tuning,
        Err(error) => {
            return config_failure_response(
                &request,
                &request_hash,
                capabilities_from_findings(&validation.findings, None),
                None,
                format!("tuning profile validation failed: {error:#}"),
            );
        }
    };

    let mut imported = match ae_bridge::import_payload_to_scene(&request.payload) {
        Ok(imported) => imported,
        Err(error) => {
            return (
                RenderJsonResponse {
                    schema: RESPONSE_SCHEMA,
                    request_id: request.request_id,
                    request_schema: Some(request.schema),
                    request_hash: Some(request_hash),
                    action,
                    status: ResponseStatus::Failed,
                    ok: false,
                    complete: false,
                    capabilities: capabilities_from_findings(&validation.findings, None),
                    scene: None,
                    artifacts: BTreeMap::new(),
                    render: None,
                    errors: vec![format!("payload import failed: {error:#}")],
                    warnings: Vec::new(),
                },
                JsonApiExit::Config,
            );
        }
    };
    if let Some(tuning) = tuning.as_mut() {
        if let Err(error) = crate::tuning::apply_to_scene(&mut imported.scene, tuning) {
            return config_failure_response(
                &request,
                &request_hash,
                capabilities_from_findings(&validation.findings, None),
                Some(scene_summary(&imported.scene)),
                format!("applying tuning profile failed: {error:#}"),
            );
        }
    }
    if let Err(error) = render_core::graph::validate_graph(&imported.scene) {
        return (
            RenderJsonResponse {
                schema: RESPONSE_SCHEMA,
                request_id: request.request_id,
                request_schema: Some(request.schema),
                request_hash: Some(request_hash),
                action,
                status: ResponseStatus::Invalid,
                ok: false,
                complete: false,
                capabilities: capabilities_from_findings(&validation.findings, None),
                scene: Some(scene_summary(&imported.scene)),
                artifacts: BTreeMap::new(),
                render: None,
                errors: vec![format!("scene graph validation failed: {error:#}")],
                warnings: Vec::new(),
            },
            JsonApiExit::Config,
        );
    }
    let features = render_core::scene_feature_summary(&imported.scene);
    let mut findings = validation.findings.clone();
    findings.extend(imported.diagnostics.findings.clone());
    if let Some(tuning) = tuning.as_ref() {
        findings.push(CapabilityFinding {
            status: CapabilityStatus::Supported,
            feature: format!("tuning.profile.{}", tuning.report.profile),
            layer: None,
            detail: format!(
                "runtime tuning applied from {} (sha256={}, applied={}, contract_only={})",
                tuning.report.source,
                tuning.report.sha256,
                tuning.report.applied.len(),
                tuning.report.contract_only.len()
            ),
        });
    }
    let mut capabilities = capabilities_from_findings(&findings, Some(&features));
    apply_operation_requirements(&mut capabilities, &request.payload.visual_ops);
    apply_audio_asset_requirements(&mut capabilities, &request.payload.visual_ops);
    if request.policy.strict && !capabilities.ignored.is_empty() {
        capabilities.complete = false;
    }
    let initial_complete = capabilities.complete;
    let scene_summary = scene_summary(&imported.scene);
    if let Some(error) = frame_selection_range_error(&request.output, &scene_summary) {
        return config_failure_response(
            &request,
            &request_hash,
            capabilities,
            Some(scene_summary),
            error,
        );
    }
    let audio_track_plans = request
        .output
        .video
        .as_ref()
        .map(|_| audio_track_plans(&request.payload, imported.scene.composition.duration))
        .unwrap_or_default();

    if action != JsonAction::Render {
        let status = if action == JsonAction::Validate {
            ResponseStatus::Validated
        } else {
            ResponseStatus::Inspected
        };
        return (
            RenderJsonResponse {
                schema: RESPONSE_SCHEMA,
                request_id: request.request_id,
                request_schema: Some(request.schema),
                request_hash: Some(request_hash),
                action,
                status,
                ok: true,
                complete: initial_complete,
                capabilities,
                scene: Some(scene_summary),
                artifacts: BTreeMap::new(),
                render: None,
                errors: Vec::new(),
                warnings: partial_warnings(initial_complete),
            },
            JsonApiExit::Success,
        );
    }

    let audio_mux_can_resolve_required_gaps =
        required_gaps_are_deferred_audio(&capabilities, &audio_track_plans, request.policy.strict);
    if !initial_complete
        && request.policy.on_unsupported == UnsupportedPolicy::Error
        && !audio_mux_can_resolve_required_gaps
    {
        return (
            RenderJsonResponse {
                schema: RESPONSE_SCHEMA,
                request_id: request.request_id,
                request_schema: Some(request.schema),
                request_hash: Some(request_hash),
                action,
                status: ResponseStatus::Unsupported,
                ok: false,
                complete: initial_complete,
                capabilities,
                scene: Some(scene_summary),
                artifacts: BTreeMap::new(),
                render: None,
                errors: vec!["native render blocked by policy.onUnsupported=error".to_string()],
                warnings: Vec::new(),
            },
            JsonApiExit::Unsupported,
        );
    }

    let output_dir = resolve_path(base_dir, &request.output.directory);
    let render_dir = output_dir.join("render");
    let scene_path = output_dir.join("scene.json");
    let capabilities_path = output_dir.join("capabilities.json");
    let normalized_request_path = output_dir.join("request.normalized.json");
    let tuning_profile_path = tuning
        .as_ref()
        .map(|_| output_dir.join("tuning-profile.resolved.json"));
    let video = request.output.video.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            output_dir.join(path)
        }
    });
    if let Err(error) = fs::create_dir_all(&output_dir) {
        return render_failure_response(
            request,
            request_hash,
            capabilities,
            scene_summary,
            format!("creating output directory failed: {error:#}"),
        );
    }
    if request.output.write_scene {
        if let Err(error) = write_pretty_json(&scene_path, &imported.scene) {
            return render_failure_response(
                request,
                request_hash,
                capabilities,
                scene_summary,
                format!("writing scene failed: {error:#}"),
            );
        }
    }
    if let Err(error) = write_pretty_json(&capabilities_path, &capabilities) {
        return render_failure_response(
            request,
            request_hash,
            capabilities,
            scene_summary,
            format!("writing capabilities failed: {error:#}"),
        );
    }
    if let Err(error) = write_pretty_json(&normalized_request_path, &request) {
        return render_failure_response(
            request,
            request_hash,
            capabilities,
            scene_summary,
            format!("writing normalized request failed: {error:#}"),
        );
    }
    if let (Some(tuning), Some(path)) = (tuning.as_ref(), tuning_profile_path.as_deref()) {
        let document = serde_json::json!({
            "profile": tuning.normalized,
            "report": tuning.report,
        });
        if let Err(error) = write_pretty_json(path, &document) {
            return render_failure_response(
                request,
                request_hash,
                capabilities,
                scene_summary,
                format!("writing resolved tuning profile failed: {error:#}"),
            );
        }
    }

    let assets_root = request
        .assets
        .root
        .as_ref()
        .map(|path| resolve_path(base_dir, path))
        .or_else(|| Some(base_dir.to_path_buf()));
    let job_archive = request
        .assets
        .job_archive
        .as_ref()
        .map(|path| resolve_path(base_dir, path));
    let started = Instant::now();
    let stage_debug = request.debug.stage_debug_spec();
    let render_result = match video.as_deref() {
        Some(video_path) => super::render_video_output(
            &scene_path,
            &imported.scene,
            &render_dir,
            assets_root.clone(),
            job_archive.clone(),
            video_path,
            false,
            stage_debug,
        ),
        None => super::render_png_output(
            &scene_path,
            &imported.scene,
            &render_dir,
            assets_root.clone(),
            job_archive.clone(),
            None,
            request.output.frames.as_deref(),
            false,
            stage_debug,
        ),
    };
    if let Err(error) = render_result {
        return render_failure_response(
            request,
            request_hash,
            capabilities,
            scene_summary,
            format!("native render failed: {error:#}"),
        );
    }

    let mut warnings = Vec::new();
    if !audio_track_plans.is_empty() {
        if let Some(video_path) = video.as_deref() {
            match super::build_resolver(&scene_path, assets_root.clone(), job_archive.clone()) {
                Ok(resolver) => {
                    let mut resolved = Vec::new();
                    for plan in &audio_track_plans {
                        match resolve_audio_source(&resolver, plan)
                            .and_then(|source| validate_audio_slice(source, plan))
                        {
                            Ok(track) => resolved.push(track),
                            Err(error) => warnings.push(format!(
                                "audio track '{}' ({}) was not muxed; its capability remains not_implemented: {error:#}",
                                plan.subject, plan.role
                            )),
                        }
                    }

                    if !resolved.is_empty() {
                        match mux_audio_tracks_ffmpeg(video_path, &resolved) {
                            Ok(()) => {
                                let promoted = resolved
                                    .iter()
                                    .map(|track| track.plan.clone())
                                    .collect::<Vec<_>>();
                                promote_muxed_audio_capabilities(
                                    &mut capabilities,
                                    &promoted,
                                    &audio_track_plans,
                                );
                            }
                            Err(error) => warnings.push(format!(
                                "multi-track audio mux was not completed; audio capabilities remain not_implemented: {error:#}"
                            )),
                        }
                    }
                }
                Err(error) => warnings.push(format!(
                    "audio asset resolver could not be created; audio capabilities remain not_implemented: {error:#}"
                )),
            }
        }
    }
    recompute_complete(&mut capabilities);
    if request.policy.strict && !capabilities.ignored.is_empty() {
        capabilities.complete = false;
    }
    let complete = capabilities.complete;
    if let Err(error) = write_pretty_json(&capabilities_path, &capabilities) {
        return render_failure_response(
            request,
            request_hash,
            capabilities,
            scene_summary,
            format!("updating capabilities after render failed: {error:#}"),
        );
    }

    let output_manifest_path = output_dir.join("output-manifest.json");
    let frames_path = render_dir.join("frames");
    if let Err(error) = write_output_manifest(
        &output_manifest_path,
        &output_dir,
        &request_hash,
        &imported.scene,
        &capabilities,
        request.output.write_scene.then_some(scene_path.as_path()),
        &capabilities_path,
        &normalized_request_path,
        tuning_profile_path.as_deref(),
        frames_path.is_dir().then_some(frames_path.as_path()),
        video.as_deref(),
    ) {
        return render_failure_response(
            request,
            request_hash,
            capabilities,
            scene_summary,
            format!("writing deterministic output manifest failed: {error:#}"),
        );
    }

    let mut artifacts = BTreeMap::new();
    if request.output.write_scene {
        artifacts.insert("scene".to_string(), scene_path.display().to_string());
    }
    artifacts.insert(
        "capabilities".to_string(),
        capabilities_path.display().to_string(),
    );
    artifacts.insert(
        "normalized_request".to_string(),
        normalized_request_path.display().to_string(),
    );
    if let Some(path) = tuning_profile_path.as_ref() {
        artifacts.insert("tuning_profile".to_string(), path.display().to_string());
    }
    artifacts.insert("render".to_string(), render_dir.display().to_string());
    artifacts.insert(
        "output_manifest".to_string(),
        output_manifest_path.display().to_string(),
    );
    if let Some(video) = &video {
        artifacts.insert("video".to_string(), video.display().to_string());
    }
    let rejected_after_render =
        !complete && request.policy.on_unsupported == UnsupportedPolicy::Error;
    let status = if complete {
        ResponseStatus::Rendered
    } else if rejected_after_render {
        ResponseStatus::Unsupported
    } else {
        ResponseStatus::Partial
    };
    warnings.extend(partial_warnings(complete));
    let errors = if rejected_after_render {
        vec!["native render did not satisfy policy.onUnsupported=error".to_string()]
    } else {
        Vec::new()
    };
    let exit = if rejected_after_render {
        JsonApiExit::Unsupported
    } else {
        JsonApiExit::Success
    };
    (
        RenderJsonResponse {
            schema: RESPONSE_SCHEMA,
            request_id: request.request_id,
            request_schema: Some(request.schema),
            request_hash: Some(request_hash),
            action,
            status,
            ok: !rejected_after_render,
            complete,
            capabilities,
            scene: Some(scene_summary),
            artifacts,
            render: Some(RenderSummary {
                elapsed_ms: started.elapsed().as_millis(),
                frames_directory: render_dir.join("frames").display().to_string(),
                video: video.map(|path| path.display().to_string()),
            }),
            errors,
            warnings,
        },
        exit,
    )
}

fn capabilities_from_findings(
    findings: &[CapabilityFinding],
    features: Option<&render_core::FeatureSummary>,
) -> CapabilityReport {
    let mut report = CapabilityReport::default();
    for finding in findings {
        let item = CapabilityItem {
            code: finding.feature.clone(),
            subject: finding.layer.clone(),
            required: true,
            detail: finding.detail.clone(),
        };
        match finding.status {
            CapabilityStatus::Supported => report.supported.push(item),
            CapabilityStatus::Approximate => report.approximate.push(item),
            CapabilityStatus::Ignored => report.ignored.push(item),
            CapabilityStatus::NotImplemented => report.not_implemented.push(item),
            CapabilityStatus::Unsupported => report.unsupported.push(item),
        }
    }
    if let Some(features) = features {
        report
            .approximate
            .extend(features.approximate.iter().map(|code| CapabilityItem {
                code: code.clone(),
                subject: None,
                required: true,
                detail: "reported by render-core scene capability scan".to_string(),
            }));
        report
            .unsupported
            .extend(features.unsupported.iter().map(|code| CapabilityItem {
                code: code.clone(),
                subject: None,
                required: true,
                detail: "reported by render-core scene capability scan".to_string(),
            }));
    }
    dedup_items(&mut report.supported);
    dedup_items(&mut report.approximate);
    dedup_items(&mut report.ignored);
    dedup_items(&mut report.not_implemented);
    dedup_items(&mut report.unsupported);
    recompute_complete(&mut report);
    report
}

fn apply_operation_requirements(
    report: &mut CapabilityReport,
    operations: &[ae_bridge::VisualOperation],
) {
    for operation in operations.iter().filter(|operation| !operation.required) {
        let code = format!("visual_op.{}", operation.kind);
        for item in report
            .not_implemented
            .iter_mut()
            .chain(report.unsupported.iter_mut())
        {
            if item.code == code && item.subject == operation.id {
                item.required = false;
            }
        }
    }
    recompute_complete(report);
}

fn apply_audio_asset_requirements(
    report: &mut CapabilityReport,
    operations: &[ae_bridge::VisualOperation],
) {
    for operation in operations {
        for (asset_index, asset) in operation.assets.iter().enumerate() {
            let role = asset.role.trim().to_ascii_lowercase();
            if !matches!(role.as_str(), "audio" | "tts_audio") {
                continue;
            }
            report.not_implemented.push(CapabilityItem {
                code: audio_asset_capability_code(&role, asset_index),
                subject: Some(audio_asset_subject(operation, asset_index)),
                required: operation.required && !asset.optional,
                detail: format!(
                    "{role} asset '{}' is pending local ffmpeg mux; network TTS is not performed",
                    asset.path
                ),
            });
        }
    }
    dedup_items(&mut report.not_implemented);
    recompute_complete(report);
}

fn recompute_complete(report: &mut CapabilityReport) {
    report.complete = !report
        .not_implemented
        .iter()
        .chain(report.unsupported.iter())
        .any(|item| item.required);
}

fn dedup_items(items: &mut Vec<CapabilityItem>) {
    items.sort();
    items.dedup();
}

fn audio_track_plans(payload: &GeneratedPayload, composition_duration: f64) -> Vec<AudioTrackPlan> {
    let mut plans = payload
        .footage_layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| layer.kind == "footage" && footage_audio_enabled(layer))
        .filter_map(|(layer_index, layer)| {
            footage_audio_track_plan(payload, layer, layer_index, composition_duration)
        })
        .collect::<Vec<_>>();
    for (operation_index, operation) in payload.visual_ops.iter().enumerate() {
        for (asset_index, asset) in operation.assets.iter().enumerate() {
            if let Some(plan) = visual_operation_audio_track_plan(
                operation,
                asset,
                operation_index,
                asset_index,
                composition_duration,
            ) {
                plans.push(plan);
            }
        }
    }
    plans
}

fn footage_audio_track_plan(
    payload: &GeneratedPayload,
    layer: &ae_bridge::PayloadLayer,
    layer_index: usize,
    composition_duration: f64,
) -> Option<AudioTrackPlan> {
    if layer
        .text_data
        .pointer("/layer_meta/comp_name_target")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|target| {
            !target.trim().is_empty() && target != payload.project_spec.main_comp_name
        })
    {
        return None;
    }
    if layer
        .text_data
        .pointer("/layer_meta/timeRemapEnabled")
        .or_else(|| layer.text_data.pointer("/layer_meta/time_remap_enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let stretch = layer
        .text_data
        .pointer("/layer_meta/stretch")
        .or_else(|| layer.extra.get("stretch"))
        .and_then(serde_json::Value::as_f64);
    if stretch.is_some_and(|stretch| (stretch - 100.0).abs() > f64::EPSILON) {
        return None;
    }
    let level_db = static_layer_audio_level_db(layer)?;

    let source = layer.text_data.get("source_footage");
    let file_path = source
        .and_then(|source| source.get("file_path"))
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(str::to_string);
    let file_name = source
        .and_then(|source| source.get("file_name"))
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(str::to_string);
    let requested_path = file_path
        .as_ref()
        .or(file_name.as_ref())
        .unwrap_or(&layer.name)
        .to_string();
    let fallback_file_name = file_name.filter(|file_name| file_name != &requested_path);

    let layer_start_time = layer
        .text_data
        .pointer("/layer_meta/startTime")
        .or_else(|| layer.text_data.pointer("/layer_meta/start_time"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(layer.in_point);
    let envelope = layer.text_data.get("audio_envelope");
    let delay = value_f64(
        envelope.unwrap_or(&serde_json::Value::Null),
        &["/delay_s", "/delay", "/delaySeconds"],
    )
    .unwrap_or(0.0)
    .max(0.0);
    let base_timeline_start = layer.in_point.max(0.0).max(layer_start_time);
    let timeline_start = (base_timeline_start + delay).max(0.0);
    let timeline_end = (layer.out_point + delay).min(composition_duration);
    let duration = timeline_end - timeline_start;
    if duration <= 0.0 {
        return None;
    }
    let fade_in = envelope
        .and_then(|envelope| envelope.get("fade_in_s"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let fade_out = envelope
        .and_then(|envelope| envelope.get("fade_out_s"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let min_db = envelope
        .and_then(|envelope| envelope.get("min_db"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(-48.0);
    let layer_duration = layer.out_point - layer.in_point;
    if (fade_in > 0.0 || fade_out > 0.0)
        && ((base_timeline_start - layer.in_point).abs() > 1e-9
            || fade_in + fade_out > layer_duration + 1e-9)
    {
        return None;
    }

    Some(AudioTrackPlan {
        track_id: format!("footage:{layer_index}"),
        subject: layer.name.clone(),
        requested_path,
        fallback_file_name,
        role: "audio".to_string(),
        required: true,
        background: envelope
            .and_then(|value| value.get("background"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        timeline_start,
        source_start: (base_timeline_start - layer_start_time).max(0.0),
        duration,
        composition_duration,
        layer_duration,
        level_db,
        fade_in,
        fade_out,
        min_db,
        duck: None,
        capability_keys: vec![
            AudioCapabilityKey {
                code: "layer.audio".to_string(),
                subject: Some(layer.name.clone()),
            },
            AudioCapabilityKey {
                code: "import.skip_audio".to_string(),
                subject: Some(layer.name.clone()),
            },
        ],
    })
}

fn visual_operation_audio_track_plan(
    operation: &ae_bridge::VisualOperation,
    asset: &ae_bridge::VisualOperationAsset,
    operation_index: usize,
    asset_index: usize,
    composition_duration: f64,
) -> Option<AudioTrackPlan> {
    let role = asset.role.trim().to_ascii_lowercase();
    if !matches!(role.as_str(), "audio" | "tts_audio") {
        return None;
    }

    let params = &operation.params;
    let impact_at = value_f64(params, &["/impactAt", "/impact_at"]);
    let placement = impact_at
        .or(operation.timing.start)
        .or_else(|| value_f64(params, &["/timelineStart", "/timeline_start", "/start"]))
        .unwrap_or(0.0);
    let layer_start_time = value_f64(params, &["/startTime", "/start_time"]).unwrap_or(placement);
    let delay = value_f64(params, &["/delay", "/delay_s", "/delaySeconds"])
        .unwrap_or(0.0)
        .max(0.0);
    let base_timeline_start = placement.max(0.0).max(layer_start_time);
    let timeline_start = (base_timeline_start + delay).max(0.0);
    let source_start = value_f64(
        params,
        &[
            "/trim/start",
            "/trim/sourceStart",
            "/trim_start",
            "/trimStart",
            "/sourceStart",
        ],
    )
    .unwrap_or(0.0)
        + (base_timeline_start - layer_start_time).max(0.0);
    let requested_duration = value_f64(
        params,
        &[
            "/trim/duration",
            "/trim_duration",
            "/trimDuration",
            "/duration",
        ],
    )
    .or(operation.timing.duration);
    let requested_end = value_f64(params, &["/trim/end", "/trim_end", "/trimEnd"])
        .map(|source_end| timeline_start + (source_end - source_start).max(0.0));
    let timeline_end = operation
        .timing
        .end
        .map(|end| end + delay)
        .or(requested_end)
        .or_else(|| requested_duration.map(|duration| timeline_start + duration))
        .unwrap_or(composition_duration)
        .min(composition_duration);
    let duration = timeline_end - timeline_start;
    if duration <= 0.0 {
        return None;
    }

    let fade_in = value_f64(params, &["/fade/in", "/fade/in_s", "/fadeIn", "/fade_in_s"])
        .unwrap_or(0.0)
        .max(0.0);
    let fade_out = value_f64(
        params,
        &["/fade/out", "/fade/out_s", "/fadeOut", "/fade_out_s"],
    )
    .unwrap_or(0.0)
    .max(0.0);
    let min_db = value_f64(params, &["/fade/minDb", "/fade/min_db", "/min_db"]).unwrap_or(-48.0);
    let level_db = value_f64(
        params,
        &["/levelDb", "/level_db", "/level", "/audioLevelDb"],
    )
    .unwrap_or(0.0);
    let duck = operation_duck_envelope(params, timeline_start, timeline_end);
    let fallback_file_name = Path::new(&asset.path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name != asset.path)
        .map(str::to_string);

    Some(AudioTrackPlan {
        track_id: format!("visual:{operation_index}:{asset_index}"),
        subject: audio_asset_subject(operation, asset_index),
        requested_path: asset.path.clone(),
        fallback_file_name,
        role: role.clone(),
        required: operation.required && !asset.optional,
        background: value_bool(params, &["/background", "/isBackground"]).unwrap_or(false),
        timeline_start,
        source_start,
        duration,
        composition_duration,
        layer_duration: duration,
        level_db,
        fade_in,
        fade_out,
        min_db,
        duck,
        capability_keys: vec![AudioCapabilityKey {
            code: audio_asset_capability_code(&role, asset_index),
            subject: Some(audio_asset_subject(operation, asset_index)),
        }],
    })
}

fn footage_audio_enabled(layer: &ae_bridge::PayloadLayer) -> bool {
    layer
        .text_data
        .pointer("/layer_meta/audioEnabled")
        .or_else(|| layer.text_data.pointer("/layer_meta/audio_enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn static_layer_audio_level_db(layer: &ae_bridge::PayloadLayer) -> Option<f64> {
    let envelope_level = layer
        .text_data
        .get("audio_envelope")
        .and_then(|value| value_f64(value, &["/level_db", "/levelDb", "/level"]));
    let mut property_level = None;
    for (name, property) in &layer.props {
        let normalized = name.to_ascii_lowercase();
        if !(normalized.contains("audio") && normalized.contains("level")) {
            continue;
        }
        if !property.keyframes.is_empty()
            || property
                .expression
                .as_deref()
                .is_some_and(|expression| !expression.trim().is_empty())
        {
            return None;
        }
        property_level = audio_level_value(&property.value);
        if property_level.is_none() {
            return None;
        }
    }
    Some(property_level.or(envelope_level).unwrap_or(0.0))
}

fn audio_level_value(value: &serde_json::Value) -> Option<f64> {
    if let Some(value) = value.as_f64() {
        return Some(value);
    }
    let channels = value.as_array()?;
    if channels.is_empty() {
        return None;
    }
    let values = channels
        .iter()
        .map(serde_json::Value::as_f64)
        .collect::<Option<Vec<_>>>()?;
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

fn operation_duck_envelope(
    params: &serde_json::Value,
    start: f64,
    end: f64,
) -> Option<AudioDuckEnvelope> {
    let duck = params
        .get("duck")
        .or_else(|| params.get("ducking"))
        .or_else(|| params.get("duckBackground"))?;
    if duck.as_bool() == Some(false) {
        return None;
    }
    if !duck.is_object() && duck.as_bool() != Some(true) {
        return None;
    }
    let amount =
        value_f64(duck, &["/amountDb", "/amount_db", "/levelDb", "/level_db"]).unwrap_or(-12.0);
    Some(AudioDuckEnvelope {
        start,
        end,
        amount_db: if amount > 0.0 { -amount } else { amount }.min(0.0),
        attack: value_f64(duck, &["/attack", "/attack_s", "/attackSeconds"])
            .unwrap_or(0.08)
            .max(0.0),
        release: value_f64(duck, &["/release", "/release_s", "/releaseSeconds"])
            .unwrap_or(0.25)
            .max(0.0),
    })
}

fn value_f64(value: &serde_json::Value, pointers: &[&str]) -> Option<f64> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(serde_json::Value::as_f64))
}

fn value_bool(value: &serde_json::Value, pointers: &[&str]) -> Option<bool> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(serde_json::Value::as_bool))
}

fn audio_asset_subject(operation: &ae_bridge::VisualOperation, asset_index: usize) -> String {
    format!(
        "{}#{asset_index}",
        operation.id.as_deref().unwrap_or(operation.kind.as_str())
    )
}

fn audio_asset_capability_code(role: &str, asset_index: usize) -> String {
    format!("visual_op.asset.{role}.{asset_index}")
}

fn required_gaps_are_deferred_audio(
    capabilities: &CapabilityReport,
    plans: &[AudioTrackPlan],
    strict: bool,
) -> bool {
    if strict && !capabilities.ignored.is_empty() {
        return false;
    }
    if capabilities.unsupported.iter().any(|item| item.required) {
        return false;
    }

    let required = capabilities
        .not_implemented
        .iter()
        .filter(|item| item.required)
        .collect::<Vec<_>>();
    !required.is_empty()
        && required.iter().all(|item| {
            plans
                .iter()
                .any(|plan| plan.required && plan_matches_item(plan, item))
        })
}

fn plan_matches_item(plan: &AudioTrackPlan, item: &CapabilityItem) -> bool {
    plan.capability_keys
        .iter()
        .any(|key| key.code == item.code && key.subject == item.subject)
}

fn promote_muxed_audio_capabilities(
    report: &mut CapabilityReport,
    successful_plans: &[AudioTrackPlan],
    all_plans: &[AudioTrackPlan],
) {
    let mut promoted = Vec::new();
    report.not_implemented.retain_mut(|item| {
        let matching_plans = all_plans
            .iter()
            .filter(|plan| plan_matches_item(plan, item))
            .collect::<Vec<_>>();
        let all_matching_succeeded = !matching_plans.is_empty()
            && matching_plans.iter().all(|plan| {
                successful_plans
                    .iter()
                    .any(|successful| same_audio_track(plan, successful))
            });
        if all_matching_succeeded {
            let plan = matching_plans[0];
            item.detail = format!(
                "{} source muxed into MP4 by ffmpeg with timeline_start={:.6}, source_start={:.6}, duration={:.6}, level_db={:.3}, fade_in={:.6}, fade_out={:.6}, min_db={:.3}",
                plan.role,
                plan.timeline_start,
                plan.source_start,
                plan.duration,
                plan.level_db,
                plan.fade_in,
                plan.fade_out,
                plan.min_db
            );
            promoted.push(item.clone());
            false
        } else {
            true
        }
    });
    report.supported.extend(promoted);
    dedup_items(&mut report.supported);
}

fn same_audio_track(left: &AudioTrackPlan, right: &AudioTrackPlan) -> bool {
    left.track_id == right.track_id
}

fn mux_audio_tracks_ffmpeg(video_path: &Path, tracks: &[ResolvedAudioTrack]) -> anyhow::Result<()> {
    anyhow::ensure!(!tracks.is_empty(), "audio mux requires at least one track");
    anyhow::ensure!(
        video_path.is_file(),
        "rendered MP4 does not exist: {}",
        video_path.display()
    );
    for track in tracks {
        anyhow::ensure!(
            track.source.is_file(),
            "resolved audio source does not exist: {}",
            track.source.display()
        );
    }

    let stem = video_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("output");
    let temporary =
        video_path.with_file_name(format!(".{stem}.audio-mux-{}.mp4", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary)
            .with_context(|| format!("removing stale audio mux output {}", temporary.display()))?;
    }

    let filter = audio_filter(
        &tracks
            .iter()
            .map(|track| track.plan.clone())
            .collect::<Vec<_>>(),
    );
    let mut command = Command::new("ffmpeg");
    command
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(video_path);
    for track in tracks {
        command.arg("-i").arg(&track.source);
    }
    let output = command
        .arg("-filter_complex")
        .arg(filter)
        .args(["-map", "0:v:0", "-map", "[audio]", "-map_metadata", "0"])
        .args(["-c:v", "copy", "-c:a", "aac"])
        .arg("-t")
        .arg(ffmpeg_seconds(tracks[0].plan.composition_duration))
        .args(["-movflags", "+faststart"])
        .arg(&temporary)
        .output()
        .with_context(|| "spawning ffmpeg for JSON API multi-track audio mux")?;

    if !output.status.success() {
        let _ = fs::remove_file(&temporary);
        anyhow::bail!(
            "ffmpeg audio mux failed for {}: {}",
            video_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let size = fs::metadata(&temporary)
        .with_context(|| format!("reading muxed MP4 metadata {}", temporary.display()))?
        .len();
    if size == 0 {
        let _ = fs::remove_file(&temporary);
        anyhow::bail!("ffmpeg produced an empty MP4 for {}", video_path.display());
    }
    let probe = match media_gst::probe::probe(&temporary) {
        Ok(probe) => probe,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| {
                format!("verifying ffmpeg audio mux output {}", temporary.display())
            });
        }
    };
    if probe.audio_sample_rate.is_none()
        || probe.audio_channels.is_none()
        || !probe.audio_duration.is_some_and(|duration| duration > 0.0)
    {
        let _ = fs::remove_file(&temporary);
        anyhow::bail!(
            "ffmpeg completed without an audio stream for {}",
            video_path.display()
        );
    }
    replace_file(&temporary, video_path)
}

fn resolve_audio_source(
    resolver: &media_gst::JobAssetResolver,
    plan: &AudioTrackPlan,
) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !plan.requested_path.trim().is_empty(),
        "required {} asset path is empty; network TTS is not available",
        plan.role
    );
    let mut candidates = Vec::new();
    for requested in
        std::iter::once(plan.requested_path.as_str()).chain(plan.fallback_file_name.as_deref())
    {
        let resolution = resolver.resolve(requested, Some("audio"));
        candidates.extend(resolution.candidates);
        if let Some(path) = resolution.resolved_path {
            return Ok(PathBuf::from(path));
        }
    }
    candidates.sort();
    candidates.dedup();
    anyhow::bail!(
        "audio asset '{}' was not found; tried: {}",
        plan.requested_path,
        candidates.join(", ")
    )
}

fn validate_audio_slice(
    source: PathBuf,
    plan: &AudioTrackPlan,
) -> anyhow::Result<ResolvedAudioTrack> {
    let probe = media_gst::probe::probe(&source)
        .with_context(|| format!("probing audio source {}", source.display()))?;
    anyhow::ensure!(
        probe.audio_sample_rate.is_some() && probe.audio_channels.is_some(),
        "audio source '{}' has no decodable audio stream",
        source.display()
    );
    anyhow::ensure!(
        probe
            .audio_duration
            .is_some_and(|duration| duration > plan.source_start + 1e-6),
        "audio slice for '{}' would produce output without an audio stream: source_start={:.6}, source_duration={:?}",
        plan.subject,
        plan.source_start,
        probe.audio_duration
    );
    Ok(ResolvedAudioTrack {
        source,
        plan: plan.clone(),
    })
}

fn replace_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            let stem = destination
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("output");
            let backup = destination.with_file_name(format!(
                ".{stem}.before-audio-mux-{}.mp4",
                std::process::id()
            ));
            let _ = fs::remove_file(&backup);
            fs::rename(destination, &backup).with_context(|| {
                format!(
                    "preparing to replace {} after rename failed: {first_error}",
                    destination.display()
                )
            })?;
            if let Err(error) = fs::rename(source, destination) {
                let _ = fs::rename(&backup, destination);
                anyhow::bail!(
                    "replacing {} with muxed MP4 failed: {error}",
                    destination.display()
                );
            }
            let _ = fs::remove_file(backup);
            Ok(())
        }
    }
}

fn ffmpeg_seconds(value: f64) -> String {
    format!("{value:.9}")
}

fn ffmpeg_amplitude_from_db(value: f64) -> String {
    ffmpeg_seconds(10.0_f64.powf(value / 20.0).clamp(0.0, 1.0))
}

fn audio_fade_in_filter(duration: f64, min_db: f64) -> String {
    let duration = duration.max(f64::EPSILON);
    let minimum = ffmpeg_amplitude_from_db(min_db);
    format!(
        "volume='if(lt(t,{duration}),{minimum}+(1-{minimum})*sin(PI*t/(2*{duration})),1)':eval=frame",
        duration = ffmpeg_seconds(duration),
    )
}

fn audio_fade_out_filter(start: f64, duration: f64, min_db: f64) -> String {
    let duration = duration.max(f64::EPSILON);
    let end = start + duration;
    let start = ffmpeg_seconds(start.max(0.0));
    let duration = ffmpeg_seconds(duration);
    let end = ffmpeg_seconds(end.max(0.0));
    let minimum = ffmpeg_amplitude_from_db(min_db);
    format!(
        "volume='if(lt(t,{start}),1,if(lt(t,{end}),{minimum}+(1-{minimum})*sin(PI*({end}-t)/(2*{duration})),{minimum}))':eval=frame"
    )
}

fn audio_filter(plans: &[AudioTrackPlan]) -> String {
    assert!(
        !plans.is_empty(),
        "audio filter requires at least one track"
    );
    let duck_envelopes = plans
        .iter()
        .filter_map(|plan| plan.duck.clone())
        .collect::<Vec<_>>();
    let mut chains = Vec::with_capacity(plans.len() + 1);
    for (track_index, plan) in plans.iter().enumerate() {
        let mut filters = vec![
            format!(
                "atrim=start={}:duration={}",
                ffmpeg_seconds(plan.source_start),
                ffmpeg_seconds(plan.duration)
            ),
            "asetpts=PTS-STARTPTS".to_string(),
        ];
        if plan.level_db.abs() > 1e-9 {
            filters.push(format!("volume={:.6}dB", plan.level_db));
        }
        if plan.fade_in > 0.0 {
            filters.push(audio_fade_in_filter(plan.fade_in, plan.min_db));
        }
        if plan.fade_out > 0.0 {
            let start = (plan.layer_duration - plan.fade_out).max(0.0);
            if start < plan.duration {
                filters.push(audio_fade_out_filter(start, plan.fade_out, plan.min_db));
            }
        }
        filters.push(format!(
            "asetpts=PTS+{}/TB",
            ffmpeg_seconds(plan.timeline_start)
        ));
        if plan.background && !duck_envelopes.is_empty() {
            filters.push(format!(
                "volume='{}':eval=frame",
                combined_duck_expression(&duck_envelopes)
            ));
        }
        filters.push(format!("anull[track_{track_index}]"));
        chains.push(format!("[{}:a:0]{}", track_index + 1, filters.join(",")));
    }

    let composition_duration = ffmpeg_seconds(plans[0].composition_duration);
    if plans.len() == 1 {
        chains.push(format!(
            "[track_0]atrim=start=0:end={composition_duration}[audio]"
        ));
    } else {
        let inputs = (0..plans.len())
            .map(|index| format!("[track_{index}]"))
            .collect::<String>();
        chains.push(format!(
            "{inputs}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0,atrim=start=0:end={composition_duration}[audio]",
            plans.len()
        ));
    }
    chains.join(";")
}

fn combined_duck_expression(envelopes: &[AudioDuckEnvelope]) -> String {
    let mut expressions = envelopes.iter().map(duck_expression).collect::<Vec<_>>();
    let expression = expressions
        .drain(..)
        .reduce(|left, right| format!("min({left},{right})"))
        .unwrap_or_else(|| "1".to_string());
    format!("if(isnan(t),1,{expression})")
}

fn duck_expression(envelope: &AudioDuckEnvelope) -> String {
    let gain = 10f64
        .powf(envelope.amount_db.min(0.0) / 20.0)
        .clamp(0.0, 1.0);
    let start = envelope.start.max(0.0);
    let end = envelope.end.max(start);
    let attack = envelope.attack.max(0.0);
    let release = envelope.release.max(0.0);
    let attack_start = (start - attack).max(0.0);
    let attack_expression = if attack > 0.0 && start > attack_start {
        format!(
            "1-(1-{gain:.9})*(t-{attack_start:.9})/{:.9}",
            start - attack_start
        )
    } else {
        format!("{gain:.9}")
    };
    let release_expression = if release > 0.0 {
        format!("{gain:.9}+(1-{gain:.9})*(t-{end:.9})/{release:.9}")
    } else {
        "1".to_string()
    };
    format!(
        "if(lt(t,{attack_start:.9}),1,if(lt(t,{start:.9}),{attack_expression},if(lt(t,{end:.9}),{gain:.9},if(lt(t,{:.9}),{release_expression},1))))",
        end + release
    )
}

fn scene_summary(scene: &render_ir::Scene) -> SceneSummary {
    SceneSummary {
        composition: scene.composition.id.clone(),
        width: scene.composition.width,
        height: scene.composition.height,
        fps: scene.composition.fps,
        duration: scene.composition.duration,
        frame_count: (scene.composition.duration * scene.composition.fps)
            .max(0.0)
            .ceil() as u64,
        layers: scene.layers.len(),
        assets: scene.assets.len(),
    }
}

fn output_spec_config_error(output: &OutputSpec) -> Option<String> {
    let frames = output.frames.as_ref()?;
    if output.video.is_some() {
        return Some(
            "outputSpec.frames cannot be combined with outputSpec.video; sparse frame selection produces PNG frames only, not MP4 video"
                .to_string(),
        );
    }
    if frames.is_empty() {
        return Some("outputSpec.frames must contain at least one frame index".to_string());
    }
    None
}

fn frame_selection_range_error(output: &OutputSpec, scene: &SceneSummary) -> Option<String> {
    let frame = output
        .frames
        .as_deref()?
        .iter()
        .copied()
        .find(|frame| u64::from(*frame) >= scene.frame_count)?;
    Some(format!(
        "outputSpec.frames contains out-of-range frame {frame}; valid indices for composition '{}' are 0..={} ({} frames)",
        scene.composition,
        scene.frame_count.saturating_sub(1),
        scene.frame_count
    ))
}

fn config_failure_response(
    request: &RenderJsonRequest,
    request_hash: &str,
    capabilities: CapabilityReport,
    scene: Option<SceneSummary>,
    error: String,
) -> (RenderJsonResponse, JsonApiExit) {
    (
        RenderJsonResponse {
            schema: RESPONSE_SCHEMA,
            request_id: request.request_id.clone(),
            request_schema: Some(request.schema.clone()),
            request_hash: Some(request_hash.to_string()),
            action: request.action,
            status: ResponseStatus::Invalid,
            ok: false,
            complete: false,
            capabilities,
            scene,
            artifacts: BTreeMap::new(),
            render: None,
            errors: vec![error],
            warnings: Vec::new(),
        },
        JsonApiExit::Config,
    )
}

fn render_failure_response(
    request: RenderJsonRequest,
    request_hash: String,
    capabilities: CapabilityReport,
    scene: SceneSummary,
    error: String,
) -> (RenderJsonResponse, JsonApiExit) {
    (
        RenderJsonResponse {
            schema: RESPONSE_SCHEMA,
            request_id: request.request_id,
            request_schema: Some(request.schema),
            request_hash: Some(request_hash),
            action: request.action,
            status: ResponseStatus::Failed,
            ok: false,
            complete: false,
            capabilities,
            scene: Some(scene),
            artifacts: BTreeMap::new(),
            render: None,
            errors: vec![error],
            warnings: Vec::new(),
        },
        JsonApiExit::Render,
    )
}

fn failure_response(
    action: JsonAction,
    status: ResponseStatus,
    error: anyhow::Error,
) -> RenderJsonResponse {
    RenderJsonResponse {
        schema: RESPONSE_SCHEMA,
        request_id: None,
        request_schema: None,
        request_hash: None,
        action,
        status,
        ok: false,
        complete: false,
        capabilities: CapabilityReport::default(),
        scene: None,
        artifacts: BTreeMap::new(),
        render: None,
        errors: vec![format!("{error:#}")],
        warnings: Vec::new(),
    }
}

fn read_request(path: &str) -> anyhow::Result<(String, PathBuf)> {
    if path == "-" {
        let mut raw = String::new();
        io::stdin().read_to_string(&mut raw)?;
        return Ok((raw, std::env::current_dir()?));
    }
    let path = PathBuf::from(path);
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("reading render request {}", path.display()))?;
    let base = path
        .canonicalize()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    Ok((raw, base))
}

fn emit_response(response: &RenderJsonResponse, path: Option<&Path>) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(response)? + "\n";
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &text)?;
    }
    print!("{text}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_output_manifest(
    path: &Path,
    output_dir: &Path,
    request_hash: &str,
    scene: &render_ir::Scene,
    capabilities: &CapabilityReport,
    scene_path: Option<&Path>,
    capabilities_path: &Path,
    normalized_request_path: &Path,
    tuning_profile_path: Option<&Path>,
    frames_path: Option<&Path>,
    video_path: Option<&Path>,
) -> anyhow::Result<()> {
    let scene_bytes = serde_json::to_vec(scene)?;
    let mut artifacts = BTreeMap::new();
    if let Some(scene_path) = scene_path {
        artifacts.insert(
            "scene".to_string(),
            stable_artifact(output_dir, scene_path)?,
        );
    }
    artifacts.insert(
        "capabilities".to_string(),
        stable_artifact(output_dir, capabilities_path)?,
    );
    artifacts.insert(
        "normalized_request".to_string(),
        stable_artifact(output_dir, normalized_request_path)?,
    );
    if let Some(tuning_profile_path) = tuning_profile_path {
        artifacts.insert(
            "tuning_profile".to_string(),
            stable_artifact(output_dir, tuning_profile_path)?,
        );
    }
    if let Some(frames_path) = frames_path {
        artifacts.insert(
            "frames".to_string(),
            stable_frames_artifact(output_dir, frames_path)?,
        );
    }
    if let Some(video_path) = video_path {
        artifacts.insert(
            "video".to_string(),
            stable_artifact(output_dir, video_path)?,
        );
    }
    let output_probe = video_path
        .map(media_gst::probe::probe)
        .transpose()?
        .map(|mut probe| {
            probe.path = video_path
                .and_then(|path| path.strip_prefix(output_dir).ok())
                .unwrap_or_else(|| video_path.expect("probe requires video path"))
                .to_string_lossy()
                .replace('\\', "/");
            probe
        });
    let frame_count = (scene.composition.duration * scene.composition.fps)
        .max(0.0)
        .ceil() as u64;
    let output = video_path.map(|_| {
        serde_json::json!({
            "video": artifacts.get("video").cloned(),
            "frame_count": frame_count,
            "duration": scene.composition.duration,
            "probe": output_probe,
            "audio_probe": output_probe.as_ref().and_then(|probe| probe.audio_codec.as_ref().map(|codec| serde_json::json!({
                "codec": codec,
                "sample_rate": probe.audio_sample_rate,
                "channels": probe.audio_channels,
                "duration": probe.audio_duration
            })))
        })
    });
    let manifest = serde_json::json!({
        "schema": "ae-native-renderer.output-manifest.v1",
        "renderer": {
            "name": "ae-native-renderer",
            "version": env!("CARGO_PKG_VERSION")
        },
        "request_hash": request_hash,
        "scene_hash": sha256_hex(&scene_bytes),
        "composition": {
            "id": scene.composition.id,
            "width": scene.composition.width,
            "height": scene.composition.height,
            "fps": scene.composition.fps,
            "duration": scene.composition.duration,
            "frame_count": frame_count
        },
        "capabilities": capabilities,
        "artifacts": artifacts,
        "output": output
    });
    write_pretty_json(path, &manifest)
}

fn stable_frames_artifact(
    output_dir: &Path,
    frames_path: &Path,
) -> anyhow::Result<serde_json::Value> {
    let mut files = fs::read_dir(frames_path)
        .with_context(|| format!("reading frames directory {}", frames_path.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();

    let mut aggregate = Sha256::new();
    let mut size_bytes = 0u64;
    for file in &files {
        let bytes = fs::read(file).with_context(|| format!("reading frame {}", file.display()))?;
        let name = file
            .strip_prefix(frames_path)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        aggregate.update(name.as_bytes());
        aggregate.update([0]);
        aggregate.update((bytes.len() as u64).to_le_bytes());
        aggregate.update(&bytes);
        size_bytes += bytes.len() as u64;
    }
    let relative = frames_path
        .strip_prefix(output_dir)
        .unwrap_or(frames_path)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(serde_json::json!({
        "path": relative,
        "count": files.len(),
        "size_bytes": size_bytes,
        "sha256": format!("{:x}", aggregate.finalize())
    }))
}

fn stable_artifact(output_dir: &Path, path: &Path) -> anyhow::Result<serde_json::Value> {
    let bytes = fs::read(path).with_context(|| format!("reading artifact {}", path.display()))?;
    let relative = path
        .strip_prefix(output_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(serde_json::json!({
        "path": relative,
        "size_bytes": bytes.len(),
        "sha256": sha256_hex(&bytes)
    }))
}

fn write_pretty_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}

fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn partial_warnings(complete: bool) -> Vec<String> {
    if complete {
        Vec::new()
    } else {
        vec![
            "result contains the supported native subset; inspect capabilities.not_implemented and capabilities.unsupported"
                .to_string(),
        ]
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn default_output_directory() -> PathBuf {
    PathBuf::from("out")
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_json(extra: &str) -> String {
        format!(
            r#"{{
              "schema":"{REQUEST_SCHEMA}",
              "action":"validate",
              "projectSpec":{{"mainCompName":"Comp 1","subtitlesMode":"brat_5th"}},
              "compsSpec":[{{"name":"Comp 1","w":64,"h":64,"fps":24,"dur":1}}],
              "footage_layers":[],
              "text_layers":[],
              "visualOps":[{{"id":"subs","type":"subtitle.brat.v1","params":{{}}}}]
              {extra}
            }}"#
        )
    }

    #[test]
    fn top_level_payload_fields_parse_without_translation() {
        let request: RenderJsonRequest = serde_json::from_str(&request_json("")).unwrap();
        assert_eq!(request.payload.project_spec.main_comp_name, "Comp 1");
        assert_eq!(request.payload.visual_ops.len(), 1);
        assert_eq!(request.payload.visual_ops[0].kind, "subtitle.brat.v1");
    }

    #[test]
    fn not_implemented_is_distinct_and_can_be_reported() {
        let mut request: RenderJsonRequest = serde_json::from_str(&request_json("")).unwrap();
        request.payload.visual_ops[0].kind = "hook.f3.effect.v1".to_string();
        let validation = ae_bridge::validate_payload(&request.payload, false);
        let capabilities = capabilities_from_findings(&validation.findings, None);
        assert!(!capabilities.complete);
        assert_eq!(capabilities.not_implemented.len(), 1);
        assert!(capabilities.unsupported.is_empty());
    }

    #[test]
    fn optional_unimplemented_operation_does_not_block_completeness() {
        let mut request: RenderJsonRequest = serde_json::from_str(&request_json("")).unwrap();
        request.payload.visual_ops[0].kind = "hook.f3.effect.v1".to_string();
        request.payload.visual_ops[0].required = false;
        let validation = ae_bridge::validate_payload(&request.payload, false);
        let mut capabilities = capabilities_from_findings(&validation.findings, None);
        apply_operation_requirements(&mut capabilities, &request.payload.visual_ops);
        assert!(capabilities.complete);
        assert!(!capabilities.not_implemented[0].required);
    }

    #[test]
    fn brat_audio_plan_seeks_source_time_53_and_keeps_envelope() {
        let request: RenderJsonRequest = serde_json::from_value(serde_json::json!({
            "schema": REQUEST_SCHEMA,
            "action": "render",
            "projectSpec": {"mainCompName": "Comp 1", "subtitlesMode": "brat_5th"},
            "compsSpec": [{"name": "Comp 1", "w": 64, "h": 64, "fps": 24, "dur": 15}],
            "footage_layers": [{
                "name": "required-audio",
                "type": "footage",
                "in_point": 0,
                "out_point": 15,
                "z_index": 1,
                "props": {},
                "effects": {},
                "text_data": {
                    "layer_meta": {
                        "comp_name_target": "Comp 1",
                        "audioEnabled": true,
                        "startTime": -53.0
                    },
                    "source_footage": {
                        "file_name": "audio_source.mp3",
                        "file_path": "/app/work/jobs/job/data/inputs/audio/source.mp3"
                    },
                    "audio_envelope": {
                        "fade_in_s": 0.5,
                        "fade_out_s": 0.5,
                        "min_db": -48.0
                    }
                }
            }],
            "text_layers": [],
            "visualOps": [],
            "outputSpec": {"video": "result.mp4"}
        }))
        .unwrap();
        let plans = audio_track_plans(&request.payload, 15.0);
        assert_eq!(plans.len(), 1);
        let plan = &plans[0];
        assert_eq!(
            plan.requested_path,
            "/app/work/jobs/job/data/inputs/audio/source.mp3"
        );
        assert_eq!(plan.fallback_file_name.as_deref(), Some("audio_source.mp3"));
        assert!((plan.timeline_start - 0.0).abs() < 1e-9);
        assert!((plan.source_start - 53.0).abs() < 1e-9);
        assert!((plan.duration - 15.0).abs() < 1e-9);
        assert!((plan.fade_in - 0.5).abs() < 1e-9);
        assert!((plan.fade_out - 0.5).abs() < 1e-9);
        assert!((plan.min_db + 48.0).abs() < 1e-9);

        let filter = audio_filter(&plans);
        assert!(!filter.contains("silence="));
        assert_eq!(filter.matches("0.003981072").count(), 5);
        assert!(filter.contains(
            "volume='if(lt(t,0.500000000),0.003981072+(1-0.003981072)*sin(PI*t/(2*0.500000000)),1)':eval=frame"
        ));
        assert!(filter.contains(
            "volume='if(lt(t,14.500000000),1,if(lt(t,15.000000000),0.003981072+(1-0.003981072)*sin(PI*(15.000000000-t)/(2*0.500000000)),0.003981072))':eval=frame"
        ));
        assert!(filter.contains("atrim=start=53.000000000:duration=15.000000000"));
    }

    fn audio_request(
        duration: f64,
        footage_layers: serde_json::Value,
        visual_ops: serde_json::Value,
    ) -> RenderJsonRequest {
        serde_json::from_value(serde_json::json!({
            "schema": REQUEST_SCHEMA,
            "action": "render",
            "projectSpec": {"mainCompName": "Comp 1"},
            "compsSpec": [{"name": "Comp 1", "w": 64, "h": 64, "fps": 24, "dur": duration}],
            "footage_layers": footage_layers,
            "text_layers": [],
            "visualOps": visual_ops,
            "outputSpec": {"video": "result.mp4"}
        }))
        .unwrap()
    }

    fn audio_footage(
        name: &str,
        file_name: &str,
        in_point: f64,
        out_point: f64,
        start_time: f64,
    ) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "type": "footage",
            "in_point": in_point,
            "out_point": out_point,
            "z_index": 1,
            "props": {},
            "effects": {},
            "text_data": {
                "layer_meta": {
                    "comp_name_target": "Comp 1",
                    "audioEnabled": true,
                    "startTime": start_time
                },
                "source_footage": {"file_name": file_name, "file_path": ""}
            }
        })
    }

    #[test]
    fn overlapping_footage_tracks_build_amix_and_composition_trim() {
        let request = audio_request(
            6.0,
            serde_json::json!([
                audio_footage("music", "music.wav", 0.0, 6.0, 0.0),
                audio_footage("room", "room.wav", 1.25, 4.0, 1.25)
            ]),
            serde_json::json!([]),
        );
        let plans = audio_track_plans(&request.payload, 6.0);
        assert_eq!(plans.len(), 2);
        let filter = audio_filter(&plans);
        assert!(filter.contains("[1:a:0]atrim=start=0.000000000:duration=6.000000000"));
        assert!(filter.contains("[2:a:0]atrim=start=0.000000000:duration=2.750000000"));
        assert!(filter.contains("asetpts=PTS+1.250000000/TB"));
        assert!(filter.contains(
            "[track_0][track_1]amix=inputs=2:duration=longest:dropout_transition=0:normalize=0"
        ));
        assert!(filter.contains("atrim=start=0:end=6.000000000[audio]"));
    }

    #[test]
    fn f1_audio_asset_uses_impact_at_delay_level_and_fades() {
        let request = audio_request(
            8.0,
            serde_json::json!([]),
            serde_json::json!([{
                "id": "impact",
                "type": "hook.f1.sound.v1",
                "timing": {"duration": 0.75},
                "params": {
                    "impactAt": 2.25,
                    "delay": 0.1,
                    "levelDb": -3.0,
                    "fadeIn": 0.02,
                    "fadeOut": 0.1
                },
                "assets": [{"role": "audio", "path": "impact.wav"}]
            }]),
        );
        let plans = audio_track_plans(&request.payload, 8.0);
        assert_eq!(plans.len(), 1);
        let plan = &plans[0];
        assert_eq!(plan.role, "audio");
        assert!((plan.timeline_start - 2.35).abs() < 1e-9);
        assert!((plan.level_db + 3.0).abs() < 1e-9);
        let filter = audio_filter(&plans);
        assert!(filter.contains("volume=-3.000000dB"));
        assert!(filter.contains("if(lt(t,0.020000000)"));
        assert!(filter.contains("if(lt(t,0.650000000),1,if(lt(t,0.750000000)"));
        assert!(!filter.contains("silence="));
        assert!(filter.contains("asetpts=PTS+2.350000000/TB"));
    }

    #[test]
    fn f5_tts_asset_is_local_trimmed_audio_without_synthesis() {
        let request = audio_request(
            5.0,
            serde_json::json!([]),
            serde_json::json!([{
                "id": "narration",
                "type": "hook.f5.cognition.v1",
                "timing": {"start": 0.75, "duration": 1.5},
                "params": {"trim": {"start": 0.2}, "level": -1.5},
                "assets": [{"role": "tts_audio", "path": "speech.wav"}]
            }]),
        );
        let plans = audio_track_plans(&request.payload, 5.0);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].role, "tts_audio");
        assert_eq!(plans[0].requested_path, "speech.wav");
        assert!((plans[0].source_start - 0.2).abs() < 1e-9);
        let filter = audio_filter(&plans);
        assert!(filter.contains("atrim=start=0.200000000:duration=1.500000000"));
        assert!(filter.contains("asetpts=PTS+0.750000000/TB"));
    }

    #[test]
    fn foreground_duck_envelope_is_applied_only_to_background_tracks() {
        let request = audio_request(
            6.0,
            serde_json::json!([audio_footage("music", "music.wav", 0.0, 6.0, 0.0)]),
            serde_json::json!([{
                "id": "voice",
                "type": "hook.f5.cognition.v1",
                "timing": {"start": 2.0, "duration": 1.0},
                "params": {"duck": {"amountDb": -18.0, "attack": 0.2, "release": 0.4}},
                "assets": [{"role": "tts_audio", "path": "voice.wav"}]
            }]),
        );
        let plans = audio_track_plans(&request.payload, 6.0);
        let filter = audio_filter(&plans);
        let chains = filter.split(';').collect::<Vec<_>>();
        assert!(chains[0].contains("volume='if(isnan(t),1,"));
        assert!(chains[0].contains("0.125892541"));
        assert!(chains[0].contains(":eval=frame"));
        assert!(!chains[1].contains(":eval=frame"));
    }

    #[test]
    fn required_missing_visual_audio_asset_remains_capability_gap() {
        let request = audio_request(
            2.0,
            serde_json::json!([]),
            serde_json::json!([{
                "id": "missing-tts",
                "type": "hook.f5.cognition.v1",
                "timing": {"duration": 1.0},
                "params": {},
                "assets": [{"role": "tts_audio", "path": ""}],
                "required": true
            }]),
        );
        let mut report = CapabilityReport::default();
        apply_audio_asset_requirements(&mut report, &request.payload.visual_ops);
        assert!(!report.complete);
        assert_eq!(report.not_implemented.len(), 1);
        assert!(report.not_implemented[0].required);
        assert_eq!(
            report.not_implemented[0].subject.as_deref(),
            Some("missing-tts#0")
        );
        let plans = audio_track_plans(&request.payload, 2.0);
        assert_eq!(plans.len(), 1);
        assert!(required_gaps_are_deferred_audio(&report, &plans, false));
    }

    #[test]
    fn duplicate_layer_subject_is_promoted_only_when_every_track_succeeds() {
        let request = audio_request(
            2.0,
            serde_json::json!([
                audio_footage("audio", "present.wav", 0.0, 2.0, 0.0),
                audio_footage("audio", "missing.wav", 0.0, 2.0, 0.0)
            ]),
            serde_json::json!([]),
        );
        let plans = audio_track_plans(&request.payload, 2.0);
        let mut report = CapabilityReport::default();
        report.not_implemented.push(CapabilityItem {
            code: "layer.audio".to_string(),
            subject: Some("audio".to_string()),
            required: true,
            detail: "pending".to_string(),
        });
        promote_muxed_audio_capabilities(&mut report, &plans[..1], &plans);
        assert_eq!(report.not_implemented.len(), 1);
        assert!(report.supported.is_empty());

        promote_muxed_audio_capabilities(&mut report, &plans, &plans);
        assert!(report.not_implemented.is_empty());
        assert_eq!(report.supported.len(), 1);
    }

    #[test]
    fn relative_paths_resolve_from_request_directory() {
        assert_eq!(
            resolve_path(Path::new("/tmp/job"), Path::new("media")),
            PathBuf::from("/tmp/job/media")
        );
    }
}
