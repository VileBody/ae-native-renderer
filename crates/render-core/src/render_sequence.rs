use crate::layer_eval::{
    render_frame_with_footage_traced, CheckerboardFootageProvider, EffectTiming, FootageProvider,
    FrameRenderTrace, LayerTiming, MotionBlurTrace, TemporalTraceRecord,
};
use raster_cpu::Canvas;
use render_ir::{EffectSpec, Layer, Scene, Transform2D};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub fn render_png_sequence(scene: &Scene, out_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let mut footage = CheckerboardFootageProvider;
    render_png_sequence_with_footage(scene, out_dir, &mut footage)
}

pub fn render_png_sequence_with_footage(
    scene: &Scene,
    out_dir: impl AsRef<Path>,
    footage: &mut dyn FootageProvider,
) -> anyhow::Result<()> {
    let out_dir = out_dir.as_ref();
    let frames_dir = out_dir.join("frames");
    fs::create_dir_all(&frames_dir)?;

    render_sequence_with_footage_callback(
        scene,
        out_dir,
        footage,
        RenderSequenceOptions::png_sequence(),
        |frame, _time, canvas| {
            let path = frames_dir.join(format!("frame_{frame:06}.png"));
            canvas.save_png(path)?;
            Ok(RenderFrameOutput::path(format!(
                "frames/frame_{frame:06}.png"
            )))
        },
    )
}

#[derive(Debug, Clone)]
pub struct RenderSequenceOptions {
    output: String,
    output_mode: Option<String>,
    output_backend: Option<String>,
    frame_write_timing_key: String,
}

impl RenderSequenceOptions {
    pub fn png_sequence() -> Self {
        Self {
            output: "frames/frame_%06d.png".to_string(),
            output_mode: None,
            output_backend: None,
            frame_write_timing_key: "save_ms".to_string(),
        }
    }

    pub fn video_sink(output: impl Into<String>, backend: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            output_mode: Some("video_sink".to_string()),
            output_backend: Some(backend.into()),
            frame_write_timing_key: "write_ms".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderFrameOutput {
    pub path: Option<String>,
}

impl RenderFrameOutput {
    pub fn path(path: impl Into<String>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }
}

pub fn render_sequence_with_footage_callback<F>(
    scene: &Scene,
    out_dir: impl AsRef<Path>,
    footage: &mut dyn FootageProvider,
    options: RenderSequenceOptions,
    mut on_frame: F,
) -> anyhow::Result<()>
where
    F: FnMut(u32, f64, &Canvas) -> anyhow::Result<RenderFrameOutput>,
{
    crate::graph::validate_graph(scene)?;
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    let frame_count = (scene.composition.duration * scene.composition.fps).ceil() as u32;
    let feature_summary = scene_feature_summary(scene);
    let scene_hash = scene_hash(scene)?;
    let asset_hashes = asset_hashes(scene)?;
    let render_started = Instant::now();
    let mut frame_timings = Vec::with_capacity(frame_count as usize);
    let mut layer_profile = BTreeMap::<String, LayerTimingAggregate>::new();
    let mut effect_profile = BTreeMap::<String, EffectTimingAggregate>::new();
    let mut log = File::create(out_dir.join("render-log.jsonl"))?;
    let mut adjustment_effects_log = File::create(out_dir.join("adjustment_effects.jsonl"))?;
    let mut temporal_telemetry_log = File::create(out_dir.join("temporal_telemetry.jsonl"))?;
    let mut text_telemetry_log = File::create(out_dir.join("text_telemetry.jsonl"))?;
    let mut expression_telemetry_log = File::create(out_dir.join("expression_telemetry.jsonl"))?;
    let mut collapse_telemetry_log = File::create(out_dir.join("collapse_telemetry.jsonl"))?;
    write_json_line(
        &mut log,
        &json!({
            "event": "render.start",
            "renderer": renderer_info(),
            "scene_hash": scene_hash.clone(),
            "frames": frame_count,
            "fps": scene.composition.fps,
            "feature_counts": feature_counts_json(&feature_summary.counts),
            "asset_hashes": asset_hashes.clone(),
            "approximate": feature_summary.approximate.clone(),
            "unsupported": feature_summary.unsupported.clone(),
            "output": render_output_json(&options)
        }),
    )?;

    for frame in 0..frame_count {
        let frame_started = Instant::now();
        let frame_render_started = Instant::now();
        let (canvas, trace) = render_frame_with_footage_traced(scene, frame, footage)?;
        let render_ms = elapsed_ms(frame_render_started);
        let time = frame as f64 / scene.composition.fps;
        let write_started = Instant::now();
        let frame_output = on_frame(frame, time, &canvas)?;
        let write_ms = elapsed_ms(write_started);
        let frame_total_ms = elapsed_ms(frame_started);
        let frame_timing = frame_timing_json(
            frame,
            time,
            render_ms,
            &options.frame_write_timing_key,
            write_ms,
            frame_total_ms,
            frame_output.path.as_deref(),
        );
        frame_timings.push(frame_timing.clone());
        record_profile(&trace, &mut layer_profile, &mut effect_profile);
        write_adjustment_effect_trace_lines(&mut adjustment_effects_log, &trace)?;
        write_temporal_trace_lines(&mut temporal_telemetry_log, &trace)?;
        write_trace_value_lines(
            &mut text_telemetry_log,
            trace.frame,
            trace.time,
            "text.layout",
            &trace.text_layouts,
        )?;
        write_trace_value_lines(
            &mut text_telemetry_log,
            trace.frame,
            trace.time,
            "text.selector_weights",
            &trace.text_selector_weights,
        )?;
        write_trace_value_lines(
            &mut expression_telemetry_log,
            trace.frame,
            trace.time,
            "expression.position",
            &trace.position_expressions,
        )?;
        write_trace_value_lines(
            &mut collapse_telemetry_log,
            trace.frame,
            trace.time,
            "collapse",
            &trace.collapse,
        )?;
        write_json_line(
            &mut log,
            &frame_log_json(
                frame,
                time,
                render_ms,
                &options.frame_write_timing_key,
                write_ms,
                frame_total_ms,
                frame_output.path.as_deref(),
                &trace,
            ),
        )?;
    }

    let total_render_ms = elapsed_ms(render_started);
    let manifest = render_manifest_json(
        scene,
        frame_count,
        scene_hash,
        asset_hashes,
        &options,
        total_render_ms,
        frame_timings,
        layer_profile_json(&layer_profile),
        effect_profile_json(&effect_profile),
        feature_counts_json(&feature_summary.counts),
        feature_summary.approximate,
        feature_summary.unsupported,
    );
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    write_json_line(
        &mut log,
        &json!({
            "event": "render.done",
            "frames": frame_count,
            "duration_ms": total_render_ms,
            "manifest": "manifest.json"
        }),
    )?;
    Ok(())
}

fn render_output_json(options: &RenderSequenceOptions) -> Value {
    let mut output = Map::new();
    output.insert("target".to_string(), json!(options.output));
    if let Some(mode) = &options.output_mode {
        output.insert("mode".to_string(), json!(mode));
    }
    if let Some(backend) = &options.output_backend {
        output.insert("backend".to_string(), json!(backend));
    }
    Value::Object(output)
}

fn frame_timing_json(
    frame: u32,
    time: f64,
    render_ms: f64,
    write_timing_key: &str,
    write_ms: f64,
    total_ms: f64,
    path: Option<&str>,
) -> Value {
    let mut timing = Map::new();
    timing.insert("frame".to_string(), json!(frame));
    timing.insert("time".to_string(), json!(time));
    timing.insert("render_ms".to_string(), json!(render_ms));
    timing.insert(write_timing_key.to_string(), json!(write_ms));
    timing.insert("total_ms".to_string(), json!(total_ms));
    if let Some(path) = path {
        timing.insert("path".to_string(), json!(path));
    }
    Value::Object(timing)
}

fn frame_log_json(
    frame: u32,
    time: f64,
    render_ms: f64,
    write_timing_key: &str,
    write_ms: f64,
    duration_ms: f64,
    path: Option<&str>,
    trace: &FrameRenderTrace,
) -> Value {
    let mut event = Map::new();
    event.insert("event".to_string(), json!("frame.rendered"));
    event.insert("frame".to_string(), json!(frame));
    event.insert("time".to_string(), json!(time));
    event.insert("render_ms".to_string(), json!(render_ms));
    event.insert(write_timing_key.to_string(), json!(write_ms));
    event.insert("duration_ms".to_string(), json!(duration_ms));
    if let Some(path) = path {
        event.insert("path".to_string(), json!(path));
    }
    event.insert("profile".to_string(), frame_trace_json(trace));
    Value::Object(event)
}

#[allow(clippy::too_many_arguments)]
fn render_manifest_json(
    scene: &Scene,
    frame_count: u32,
    scene_hash: String,
    asset_hashes: Vec<Value>,
    options: &RenderSequenceOptions,
    total_render_ms: f64,
    frame_timings: Vec<Value>,
    layer_profile: Value,
    effect_profile: Value,
    feature_counts: Value,
    approximate: Vec<String>,
    unsupported: Vec<String>,
) -> Value {
    let mut manifest = Map::new();
    manifest.insert("renderer".to_string(), renderer_info());
    manifest.insert("composition".to_string(), json!(scene.composition.id));
    manifest.insert("width".to_string(), json!(scene.composition.width));
    manifest.insert("height".to_string(), json!(scene.composition.height));
    manifest.insert("fps".to_string(), json!(scene.composition.fps));
    manifest.insert("duration".to_string(), json!(scene.composition.duration));
    manifest.insert("frames".to_string(), json!(frame_count));
    manifest.insert("scene_hash".to_string(), json!(scene_hash));
    manifest.insert("layers".to_string(), json!(scene.layers.len()));
    manifest.insert("assets".to_string(), json!(scene.assets.len()));
    manifest.insert("asset_hashes".to_string(), json!(asset_hashes));
    manifest.insert("output".to_string(), json!(options.output));
    if let Some(mode) = &options.output_mode {
        manifest.insert("output_mode".to_string(), json!(mode));
    }
    if let Some(backend) = &options.output_backend {
        manifest.insert("output_backend".to_string(), json!(backend));
    }
    manifest.insert(
        "timing".to_string(),
        json!({
            "total_ms": total_render_ms,
            "frames": frame_timings
        }),
    );
    manifest.insert(
        "profile".to_string(),
        json!({
            "layers": layer_profile,
            "effects": effect_profile
        }),
    );
    manifest.insert("feature_counts".to_string(), feature_counts);
    manifest.insert("approximate".to_string(), json!(approximate));
    manifest.insert("unsupported".to_string(), json!(unsupported));
    Value::Object(manifest)
}

#[derive(Debug, Clone, Default)]
struct LayerTimingAggregate {
    layer_type: &'static str,
    calls: u64,
    content_ms: f64,
    effects_ms: f64,
    total_ms: f64,
    max_total_ms: f64,
}

#[derive(Debug, Clone, Default)]
struct EffectTimingAggregate {
    calls: u64,
    total_ms: f64,
    max_ms: f64,
}

fn record_profile(
    trace: &FrameRenderTrace,
    layer_profile: &mut BTreeMap<String, LayerTimingAggregate>,
    effect_profile: &mut BTreeMap<String, EffectTimingAggregate>,
) {
    for layer in &trace.layers {
        let key = format!("{}:{}", layer.composition, layer.layer_id);
        let entry = layer_profile
            .entry(key)
            .or_insert_with(|| LayerTimingAggregate {
                layer_type: layer.layer_type,
                ..LayerTimingAggregate::default()
            });
        entry.calls += 1;
        entry.content_ms += layer.content_ms;
        entry.effects_ms += layer.effects_ms;
        entry.total_ms += layer.total_ms;
        entry.max_total_ms = entry.max_total_ms.max(layer.total_ms);
    }

    for effect in &trace.effects {
        let key = effect.match_name.clone();
        let entry = effect_profile.entry(key).or_default();
        entry.calls += 1;
        entry.total_ms += effect.elapsed_ms;
        entry.max_ms = entry.max_ms.max(effect.elapsed_ms);
    }
}

fn frame_trace_json(trace: &FrameRenderTrace) -> serde_json::Value {
    json!({
        "frame": trace.frame,
        "time": trace.time,
        "layers": trace.layers.iter().map(layer_timing_json).collect::<Vec<_>>(),
        "effects": trace.effects.iter().map(effect_timing_json).collect::<Vec<_>>(),
        "adjustment_effects": trace.adjustment_effects.iter().map(adjustment_effect_trace_json).collect::<Vec<_>>(),
        "temporal": trace.temporal.iter().map(temporal_trace_record_json).collect::<Vec<_>>(),
        "motion_blur": trace.motion_blur.iter().map(motion_blur_trace_json).collect::<Vec<_>>(),
        "text_layouts": &trace.text_layouts,
        "text_selector_weights": &trace.text_selector_weights,
        "position_expressions": &trace.position_expressions,
        "collapse": &trace.collapse
    })
}

fn layer_timing_json(timing: &LayerTiming) -> serde_json::Value {
    json!({
        "composition": timing.composition,
        "layer_id": timing.layer_id,
        "type": timing.layer_type,
        "content_ms": timing.content_ms,
        "effects_ms": timing.effects_ms,
        "total_ms": timing.total_ms
    })
}

fn effect_timing_json(timing: &EffectTiming) -> serde_json::Value {
    json!({
        "composition": timing.composition,
        "layer_id": timing.layer_id,
        "match_name": timing.match_name,
        "elapsed_ms": timing.elapsed_ms
    })
}

fn adjustment_effect_trace_json(
    trace: &crate::layer_eval::AdjustmentEffectTrace,
) -> serde_json::Value {
    json!({
        "composition": trace.composition,
        "layer_id": trace.layer_id,
        "effect_index": trace.effect_index,
        "match_name": trace.match_name,
        "comp_time": trace.comp_time,
        "layer_time": trace.layer_time,
        "lower_stack_time": trace.lower_stack_time,
        "param_time": trace.param_time,
        "input_hash": trace.input_hash,
        "output_hash": trace.output_hash,
        "posterize": trace.posterize.map(|posterize| {
            json!({
                "frame_rate": posterize.frame_rate,
                "bucket": posterize.bucket,
                "bucket_time": posterize.bucket_time
            })
        })
    })
}

fn adjustment_effect_sidecar_json(
    frame: u32,
    time: f64,
    trace: &crate::layer_eval::AdjustmentEffectTrace,
) -> serde_json::Value {
    let bucket_time = trace
        .posterize
        .map(|posterize| posterize.bucket_time)
        .unwrap_or(trace.lower_stack_time);
    json!({
        "frame": frame,
        "time": time,
        "composition": trace.composition,
        "layer_id": trace.layer_id,
        "effect_index": trace.effect_index,
        "match_name": trace.match_name,
        "comp_time": trace.comp_time,
        "layer_time": trace.layer_time,
        "lower_stack_time": trace.lower_stack_time,
        "bucket_time": bucket_time,
        "param_time": trace.param_time,
        "input_hash": trace.input_hash,
        "output_hash": trace.output_hash,
        "posterize": trace.posterize.map(|posterize| {
            json!({
                "frame_rate": posterize.frame_rate,
                "bucket": posterize.bucket,
                "bucket_time": posterize.bucket_time
            })
        })
    })
}

fn temporal_trace_record_json(record: &TemporalTraceRecord) -> serde_json::Value {
    json!({
        "event": record.event,
        "composition": record.composition,
        "layer_id": record.layer_id,
        "layer_type": record.layer_type,
        "comp_time": record.comp_time,
        "layer_start": record.layer_start,
        "layer_time": record.layer_time,
        "posterized_time": record.posterized_time,
        "source_id": record.source_id,
        "source_start": record.source_start,
        "source_time": record.source_time,
        "source_frame_id": record.source_frame_id,
        "adjustment_lower_stack_time": record.adjustment_lower_stack_time,
        "posterize": record.posterize.map(|posterize| {
            json!({
                "frame_rate": posterize.frame_rate,
                "bucket": posterize.bucket,
                "bucket_time": posterize.bucket_time
            })
        })
    })
}

fn motion_blur_trace_json(trace: &MotionBlurTrace) -> serde_json::Value {
    json!({
        "event": "temporal.motion_blur",
        "composition": trace.composition,
        "layer_id": trace.layer_id,
        "comp_time": trace.comp_time,
        "frame_duration": trace.frame_duration,
        "shutter_open": trace.shutter_open,
        "shutter_close": trace.shutter_close,
        "shutter_angle": trace.shutter_angle,
        "shutter_phase": trace.shutter_phase,
        "requested_samples": trace.requested_samples,
        "effective_samples": trace.effective_samples,
        "divisor": trace.divisor,
        "samples": trace.samples.iter().map(|sample| {
            json!({
                "sample_index": sample.sample_index,
                "sample_time": sample.sample_time,
                "layer_time": sample.layer_time,
                "posterized_time": sample.posterized_time,
                "source_time": sample.source_time,
                "source_frame_id": sample.source_frame_id,
                "active": sample.active,
                "opacity": sample.opacity,
                "weight": sample.weight
            })
        }).collect::<Vec<_>>()
    })
}

fn write_adjustment_effect_trace_lines(
    log: &mut File,
    trace: &FrameRenderTrace,
) -> anyhow::Result<()> {
    for effect_trace in &trace.adjustment_effects {
        write_json_line(
            log,
            &adjustment_effect_sidecar_json(trace.frame, trace.time, effect_trace),
        )?;
    }
    Ok(())
}

fn write_temporal_trace_lines(log: &mut File, trace: &FrameRenderTrace) -> anyhow::Result<()> {
    for record in &trace.temporal {
        write_json_line(
            log,
            &json!({
                "frame": trace.frame,
                "time": trace.time,
                "record": temporal_trace_record_json(record)
            }),
        )?;
    }
    for motion_blur in &trace.motion_blur {
        write_json_line(
            log,
            &json!({
                "frame": trace.frame,
                "time": trace.time,
                "record": motion_blur_trace_json(motion_blur)
            }),
        )?;
    }
    Ok(())
}

fn write_trace_value_lines(
    log: &mut File,
    frame: u32,
    time: f64,
    event: &str,
    records: &[serde_json::Value],
) -> anyhow::Result<()> {
    for record in records {
        write_json_line(
            log,
            &json!({
                "event": event,
                "frame": frame,
                "time": time,
                "record": record
            }),
        )?;
    }
    Ok(())
}

fn layer_profile_json(profile: &BTreeMap<String, LayerTimingAggregate>) -> serde_json::Value {
    json!(profile
        .iter()
        .map(|(key, aggregate)| {
            json!({
                "layer": key,
                "type": aggregate.layer_type,
                "calls": aggregate.calls,
                "content_ms": aggregate.content_ms,
                "effects_ms": aggregate.effects_ms,
                "total_ms": aggregate.total_ms,
                "avg_total_ms": aggregate.total_ms / aggregate.calls.max(1) as f64,
                "max_total_ms": aggregate.max_total_ms
            })
        })
        .collect::<Vec<_>>())
}

fn effect_profile_json(profile: &BTreeMap<String, EffectTimingAggregate>) -> serde_json::Value {
    json!(profile
        .iter()
        .map(|(match_name, aggregate)| {
            json!({
                "match_name": match_name,
                "calls": aggregate.calls,
                "total_ms": aggregate.total_ms,
                "avg_ms": aggregate.total_ms / aggregate.calls.max(1) as f64,
                "max_ms": aggregate.max_ms
            })
        })
        .collect::<Vec<_>>())
}

pub fn mux_png_sequence_to_mp4(
    frames_dir: impl AsRef<Path>,
    fps: f64,
    out: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let frames_dir = frames_dir.as_ref();
    let out = out.as_ref();
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }

    let pattern = frames_dir.join("frame_%06d.png");
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-framerate")
        .arg(format!("{fps}"))
        .arg("-i")
        .arg(&pattern)
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-movflags")
        .arg("+faststart")
        .arg(out)
        .status()?;
    if !status.success() {
        anyhow::bail!("ffmpeg mux failed for {}", pattern.display());
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct FeatureSummary {
    pub approximate: Vec<String>,
    pub unsupported: Vec<String>,
    pub counts: FeatureCounts,
}

#[derive(Debug, Clone, Default)]
pub struct FeatureCounts {
    pub layers: LayerCounts,
    pub effects: EffectCounts,
}

#[derive(Debug, Clone, Default)]
pub struct LayerCounts {
    pub total: usize,
    pub top_level: usize,
    pub nested_compositions: usize,
    pub by_type: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default)]
pub struct EffectCounts {
    pub total: usize,
    pub supported: usize,
    pub approximate: usize,
    pub unsupported: usize,
    pub by_match_name: BTreeMap<String, usize>,
}

pub fn scene_feature_summary(scene: &Scene) -> FeatureSummary {
    let mut approximate = BTreeSet::new();
    let mut unsupported = BTreeSet::new();
    let mut counts = FeatureCounts::default();
    counts.layers.top_level = scene.layers.len();
    counts.layers.nested_compositions = scene.compositions.len();
    collect_layer_features(
        &scene.layers,
        &mut approximate,
        &mut unsupported,
        &mut counts,
    );
    for composition in &scene.compositions {
        collect_layer_features(
            &composition.layers,
            &mut approximate,
            &mut unsupported,
            &mut counts,
        );
    }
    collect_collapse_features(scene, &mut approximate);
    FeatureSummary {
        approximate: approximate.into_iter().collect(),
        unsupported: unsupported.into_iter().collect(),
        counts,
    }
}

fn collect_layer_features(
    layers: &[Layer],
    approximate: &mut BTreeSet<String>,
    unsupported: &mut BTreeSet<String>,
    counts: &mut FeatureCounts,
) {
    for layer in layers {
        counts.layers.total += 1;
        *counts
            .layers
            .by_type
            .entry(layer_kind(layer).to_string())
            .or_insert(0) += 1;
        for feature in approximate_keyframes(transform_of(layer)) {
            approximate.insert(format!("layer.{}.{}", layer.id(), feature));
        }
        if let Layer::Text { text_animators, .. } = layer {
            if !text_animators.is_empty() {
                approximate.insert(format!("layer.{}.text_animator.range_selector", layer.id()));
                if text_animators.iter().any(|animator| {
                    animator.position.is_some()
                        || animator.scale.is_some()
                        || animator.rotation.is_some()
                        || animator.blur.is_some()
                }) {
                    approximate.insert(format!(
                        "layer.{}.text_animator.per_glyph_transform",
                        layer.id()
                    ));
                }
                if text_animators
                    .iter()
                    .any(|animator| animator.expression_selector.is_some())
                {
                    approximate.insert(format!(
                        "layer.{}.text_animator.expression_selector",
                        layer.id()
                    ));
                }
            }
        }
        for effect in effects_of(layer) {
            counts.effects.total += 1;
            *counts
                .effects
                .by_match_name
                .entry(effect.match_name.clone())
                .or_insert(0) += 1;
            if is_supported_effect(&effect.match_name) {
                counts.effects.supported += 1;
                counts.effects.approximate += 1;
                approximate.insert(format!("layer.{}.effect.{}", layer.id(), effect.match_name));
            } else {
                counts.effects.unsupported += 1;
                unsupported.insert(format!("layer.{}.effect.{}", layer.id(), effect.match_name));
            }
        }
    }
}

fn collect_collapse_features(scene: &Scene, approximate: &mut BTreeSet<String>) {
    let Ok(plan) = crate::graph::precomp_render_plan(scene) else {
        return;
    };
    for entry in plan.entries {
        if !entry.collapse_requested {
            continue;
        }
        if entry.collapse_mode == crate::precomp::CollapseMode::RasterizeFirst {
            approximate.insert(format!(
                "layer.{}.precomp.collapse_transformations.rasterize_first",
                entry.layer_id
            ));
        }
    }
}

fn is_supported_effect(match_name: &str) -> bool {
    effects::EffectRegistry::known_match_names()
        .iter()
        .any(|known| *known == match_name)
}

fn layer_kind(layer: &Layer) -> &'static str {
    match layer {
        Layer::Solid { .. } => "solid",
        Layer::Footage { .. } => "footage",
        Layer::Text { .. } => "text",
        Layer::Precomp { .. } => "precomp",
        Layer::Adjustment { .. } => "adjustment",
    }
}

fn renderer_info() -> serde_json::Value {
    json!({
        "crate": "render-core",
        "version": env!("CARGO_PKG_VERSION")
    })
}

fn feature_counts_json(counts: &FeatureCounts) -> serde_json::Value {
    json!({
        "layers": {
            "total": counts.layers.total,
            "top_level": counts.layers.top_level,
            "nested_compositions": counts.layers.nested_compositions,
            "by_type": &counts.layers.by_type
        },
        "effects": {
            "total": counts.effects.total,
            "supported": counts.effects.supported,
            "approximate": counts.effects.approximate,
            "unsupported": counts.effects.unsupported,
            "by_match_name": &counts.effects.by_match_name
        }
    })
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn asset_hashes(scene: &Scene) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut hashes = Vec::with_capacity(scene.assets.len());
    for asset in &scene.assets {
        let bytes = serde_json::to_vec(asset)?;
        let digest = Sha256::digest(bytes);
        let kind = serde_json::to_value(&asset.kind)?;
        hashes.push(json!({
            "id": &asset.id,
            "type": kind,
            "path": &asset.path,
            "hash": to_hex(&digest),
            "algorithm": "sha256",
            "source": "asset_spec"
        }));
    }
    Ok(hashes)
}

fn transform_of(layer: &Layer) -> Option<&Transform2D> {
    match layer {
        Layer::Solid { transform, .. }
        | Layer::Footage { transform, .. }
        | Layer::Text { transform, .. }
        | Layer::Precomp { transform, .. } => Some(transform),
        Layer::Adjustment { .. } => None,
    }
}

fn effects_of(layer: &Layer) -> &[EffectSpec] {
    match layer {
        Layer::Solid { effects, .. }
        | Layer::Footage { effects, .. }
        | Layer::Text { effects, .. }
        | Layer::Precomp { effects, .. }
        | Layer::Adjustment { effects, .. } => effects,
    }
}

fn approximate_keyframes(transform: Option<&Transform2D>) -> Vec<&'static str> {
    let Some(transform) = transform else {
        return Vec::new();
    };
    let mut features = Vec::new();
    if transform
        .animation
        .position
        .iter()
        .any(|key| key.approximate)
    {
        features.push("keyframes.position.bezier_ease_approx");
    }
    if transform.animation.scale.iter().any(|key| key.approximate) {
        features.push("keyframes.scale.bezier_ease_approx");
    }
    if transform
        .animation
        .opacity
        .iter()
        .any(|key| key.approximate)
    {
        features.push("keyframes.opacity.bezier_ease_approx");
    }
    if transform.animation.reveal.iter().any(|key| key.approximate) {
        features.push("keyframes.reveal.bezier_ease_approx");
    }
    if transform.animation.expression.position.is_some() {
        features.push("expression.position.edge_wobble");
    }
    features
}

fn scene_hash(scene: &Scene) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(scene)?;
    let digest = Sha256::digest(bytes);
    Ok(to_hex(&digest))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn write_json_line(log: &mut File, value: &serde_json::Value) -> anyhow::Result<()> {
    writeln!(log, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer_eval::{AdjustmentEffectTrace, PosterizeTiming};
    use render_ir::{Asset, AssetKind, Composition, CompositionNode, Rect};

    #[test]
    fn feature_summary_counts_layers_and_effect_match_names() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "main".to_string(),
                width: 4,
                height: 4,
                fps: 30.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "nested".to_string(),
                    width: 4,
                    height: 4,
                    fps: 30.0,
                    duration: 1.0,
                    background: [0, 0, 0, 0],
                    motion_blur: render_ir::MotionBlurSettings::default(),
                },
                layers: vec![Layer::Adjustment {
                    id: "adjust".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    effects: vec![EffectSpec {
                        match_name: "ADBE Missing".to_string(),
                        params: json!({}),
                    }],
                }],
            }],
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "solid".to_string(),
                start: 0.0,
                duration: 1.0,
                color: [255, 0, 0, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 4.0,
                    h: 4.0,
                },
                transform: Transform2D::default(),
                effects: vec![EffectSpec {
                    match_name: "ADBE Box Blur2".to_string(),
                    params: json!({ "0001": 1 }),
                }],
            }],
        };

        let summary = scene_feature_summary(&scene);

        assert_eq!(summary.counts.layers.total, 2);
        assert_eq!(summary.counts.layers.top_level, 1);
        assert_eq!(summary.counts.layers.nested_compositions, 1);
        assert_eq!(summary.counts.layers.by_type.get("solid"), Some(&1));
        assert_eq!(summary.counts.layers.by_type.get("adjustment"), Some(&1));
        assert_eq!(summary.counts.effects.total, 2);
        assert_eq!(summary.counts.effects.supported, 1);
        assert_eq!(summary.counts.effects.unsupported, 1);
        assert_eq!(
            summary.counts.effects.by_match_name.get("ADBE Box Blur2"),
            Some(&1)
        );
        assert_eq!(
            summary.counts.effects.by_match_name.get("ADBE Missing"),
            Some(&1)
        );
    }

    #[test]
    fn asset_hashes_are_deterministic_asset_spec_hashes() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "main".to_string(),
                width: 1,
                height: 1,
                fps: 30.0,
                duration: 0.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: vec![Asset {
                id: "image-1".to_string(),
                kind: AssetKind::Image,
                path: "assets/image.png".to_string(),
            }],
            layers: Vec::new(),
        };

        let first = asset_hashes(&scene).unwrap();
        let second = asset_hashes(&scene).unwrap();

        assert_eq!(first, second);
        assert_eq!(first[0]["id"].as_str(), Some("image-1"));
        assert_eq!(first[0]["type"].as_str(), Some("image"));
        assert_eq!(first[0]["source"].as_str(), Some("asset_spec"));
        assert_eq!(first[0]["algorithm"].as_str(), Some("sha256"));
    }

    #[test]
    fn adjustment_effect_sidecar_writes_one_record_per_effect() {
        let trace = FrameRenderTrace {
            frame: 5,
            time: 0.16666666666666666,
            layers: Vec::new(),
            effects: Vec::new(),
            adjustment_effects: vec![AdjustmentEffectTrace {
                composition: "main".to_string(),
                layer_id: "adjust".to_string(),
                effect_index: 1,
                match_name: "ADBE Posterize Time".to_string(),
                comp_time: 0.16666666666666666,
                layer_time: 0.16666666666666666,
                lower_stack_time: 0.0,
                param_time: 0.0,
                input_hash: "input".to_string(),
                output_hash: "output".to_string(),
                posterize: Some(PosterizeTiming {
                    frame_rate: 6.0,
                    bucket: Some(1),
                    bucket_time: 0.0,
                }),
            }],
            temporal: Vec::new(),
            motion_blur: Vec::new(),
            effect_debug: Vec::new(),
            text_layouts: Vec::new(),
            text_selector_weights: Vec::new(),
            position_expressions: Vec::new(),
            collapse: Vec::new(),
        };
        let path = std::env::temp_dir().join(format!(
            "ae_native_adjustment_effects_{}_{}.jsonl",
            std::process::id(),
            trace.frame
        ));

        {
            let mut file = File::create(&path).unwrap();
            write_adjustment_effect_trace_lines(&mut file, &trace).unwrap();
        }

        let raw = std::fs::read_to_string(&path).unwrap();
        let lines = raw.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        let record: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(record["frame"].as_u64(), Some(5));
        assert_eq!(record["match_name"].as_str(), Some("ADBE Posterize Time"));
        assert_eq!(record["bucket_time"].as_f64(), Some(0.0));
        assert_eq!(record["param_time"].as_f64(), Some(0.0));
        assert_eq!(record["input_hash"].as_str(), Some("input"));
        assert_eq!(record["output_hash"].as_str(), Some("output"));
        assert_eq!(record["posterize"]["frame_rate"].as_f64(), Some(6.0));
        let _ = std::fs::remove_file(path);
    }
}
