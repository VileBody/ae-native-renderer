use crate::layer_eval::{
    render_frame_with_footage_traced, CheckerboardFootageProvider, EffectTiming,
    FootageProvider, FrameRenderTrace, LayerTiming,
};
use render_ir::{EffectSpec, Layer, Scene, Transform2D};
use serde_json::json;
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
    crate::graph::validate_graph(scene)?;
    let out_dir = out_dir.as_ref();
    let frames_dir = out_dir.join("frames");
    fs::create_dir_all(&frames_dir)?;

    let frame_count = (scene.composition.duration * scene.composition.fps).ceil() as u32;
    let feature_summary = scene_feature_summary(scene);
    let scene_hash = scene_hash(scene)?;
    let asset_hashes = asset_hashes(scene)?;
    let render_started = Instant::now();
    let mut frame_timings = Vec::with_capacity(frame_count as usize);
    let mut layer_profile = BTreeMap::<String, LayerTimingAggregate>::new();
    let mut effect_profile = BTreeMap::<String, EffectTimingAggregate>::new();
    let mut log = File::create(out_dir.join("render-log.jsonl"))?;
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
            "unsupported": feature_summary.unsupported.clone()
        }),
    )?;

    for frame in 0..frame_count {
        let frame_started = Instant::now();
        let frame_render_started = Instant::now();
        let (canvas, trace) = render_frame_with_footage_traced(scene, frame, footage)?;
        let render_ms = elapsed_ms(frame_render_started);
        let path = frames_dir.join(format!("frame_{frame:06}.png"));
        let save_started = Instant::now();
        canvas.save_png(path)?;
        let save_ms = elapsed_ms(save_started);
        let frame_total_ms = elapsed_ms(frame_started);
        let frame_timing = json!({
            "frame": frame,
            "time": frame as f64 / scene.composition.fps,
            "render_ms": render_ms,
            "save_ms": save_ms,
            "total_ms": frame_total_ms,
            "path": format!("frames/frame_{frame:06}.png")
        });
        frame_timings.push(frame_timing.clone());
        record_profile(&trace, &mut layer_profile, &mut effect_profile);
        write_json_line(
            &mut log,
            &json!({
                "event": "frame.rendered",
                "frame": frame,
                "time": frame as f64 / scene.composition.fps,
                "render_ms": render_ms,
                "save_ms": save_ms,
                "duration_ms": frame_total_ms,
                "path": format!("frames/frame_{frame:06}.png"),
                "profile": frame_trace_json(&trace)
            }),
        )?;
    }

    let total_render_ms = elapsed_ms(render_started);
    let manifest = json!({
        "renderer": renderer_info(),
        "composition": scene.composition.id,
        "width": scene.composition.width,
        "height": scene.composition.height,
        "fps": scene.composition.fps,
        "duration": scene.composition.duration,
        "frames": frame_count,
        "scene_hash": scene_hash,
        "layers": scene.layers.len(),
        "assets": scene.assets.len(),
        "asset_hashes": asset_hashes,
        "output": "frames/frame_%06d.png",
        "timing": {
            "total_ms": total_render_ms,
            "frames": frame_timings
        },
        "profile": {
            "layers": layer_profile_json(&layer_profile),
            "effects": effect_profile_json(&effect_profile)
        },
        "feature_counts": feature_counts_json(&feature_summary.counts),
        "approximate": feature_summary.approximate,
        "unsupported": feature_summary.unsupported
    });
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
        let entry = layer_profile.entry(key).or_insert_with(|| LayerTimingAggregate {
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
        "effects": trace.effects.iter().map(effect_timing_json).collect::<Vec<_>>()
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

fn layer_profile_json(profile: &BTreeMap<String, LayerTimingAggregate>) -> serde_json::Value {
    json!(
        profile
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
            .collect::<Vec<_>>()
    )
}

fn effect_profile_json(profile: &BTreeMap<String, EffectTimingAggregate>) -> serde_json::Value {
    json!(
        profile
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
            .collect::<Vec<_>>()
    )
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
                approximate.insert(format!(
                    "layer.{}.effect.{}",
                    layer.id(),
                    effect.match_name
                ));
            } else {
                counts.effects.unsupported += 1;
                unsupported.insert(format!(
                    "layer.{}.effect.{}",
                    layer.id(),
                    effect.match_name
                ));
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
    if transform.animation.position.iter().any(|key| key.approximate) {
        features.push("keyframes.position.bezier_ease_approx");
    }
    if transform.animation.scale.iter().any(|key| key.approximate) {
        features.push("keyframes.scale.bezier_ease_approx");
    }
    if transform.animation.opacity.iter().any(|key| key.approximate) {
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
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "nested".to_string(),
                    width: 4,
                    height: 4,
                    fps: 30.0,
                    duration: 1.0,
                    background: [0, 0, 0, 0],
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
}
