use crate::layer_eval::{
    render_frame_with_footage, CheckerboardFootageProvider, FootageProvider,
};
use render_ir::{EffectSpec, Layer, Scene, Transform2D};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

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
    let mut log = File::create(out_dir.join("render-log.jsonl"))?;
    write_json_line(
        &mut log,
        &json!({
            "event": "render.start",
            "scene_hash": scene_hash.clone(),
            "frames": frame_count,
            "fps": scene.composition.fps,
            "approximate": feature_summary.approximate.clone(),
            "unsupported": feature_summary.unsupported.clone()
        }),
    )?;

    for frame in 0..frame_count {
        let canvas = render_frame_with_footage(scene, frame, footage)?;
        let path = frames_dir.join(format!("frame_{frame:06}.png"));
        canvas.save_png(path)?;
        write_json_line(
            &mut log,
            &json!({
                "event": "frame.rendered",
                "frame": frame,
                "time": frame as f64 / scene.composition.fps,
                "path": format!("frames/frame_{frame:06}.png")
            }),
        )?;
    }

    let manifest = json!({
        "composition": scene.composition.id,
        "width": scene.composition.width,
        "height": scene.composition.height,
        "fps": scene.composition.fps,
        "duration": scene.composition.duration,
        "frames": frame_count,
        "scene_hash": scene_hash,
        "layers": scene.layers.len(),
        "assets": scene.assets.len(),
        "output": "frames/frame_%06d.png",
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
            "manifest": "manifest.json"
        }),
    )?;
    Ok(())
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
}

pub fn scene_feature_summary(scene: &Scene) -> FeatureSummary {
    let mut approximate = BTreeSet::new();
    let mut unsupported = BTreeSet::new();
    collect_layer_features(&scene.layers, &mut approximate, &mut unsupported);
    for composition in &scene.compositions {
        collect_layer_features(&composition.layers, &mut approximate, &mut unsupported);
    }
    FeatureSummary {
        approximate: approximate.into_iter().collect(),
        unsupported: unsupported.into_iter().collect(),
    }
}

fn collect_layer_features(
    layers: &[Layer],
    approximate: &mut BTreeSet<String>,
    unsupported: &mut BTreeSet<String>,
) {
    for layer in layers {
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
        if let Layer::Precomp {
            collapse_transformations: true,
            ..
        } = layer
        {
            approximate.insert(format!(
                "layer.{}.precomp.collapse_transformations",
                layer.id()
            ));
        }
        for effect in effects_of(layer) {
            match effect.match_name.as_str() {
                "ADBE Drop Shadow"
                | "ADBE Glo2"
                | "ADBE Box Blur2"
                | "ADBE Geometry2"
                | "ADBE Posterize Time"
                | "ADBE Minimax"
                | "ADBE Turbulent Displace" => {
                    approximate.insert(format!(
                        "layer.{}.effect.{}",
                        layer.id(),
                        effect.match_name
                    ));
                }
                _ => {
                    unsupported.insert(format!(
                        "layer.{}.effect.{}",
                        layer.id(),
                        effect.match_name
                    ));
                }
            }
        }
    }
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
        features.push("keyframes.position.bezier_as_linear");
    }
    if transform.animation.scale.iter().any(|key| key.approximate) {
        features.push("keyframes.scale.bezier_as_linear");
    }
    if transform.animation.opacity.iter().any(|key| key.approximate) {
        features.push("keyframes.opacity.bezier_as_linear");
    }
    if transform.animation.reveal.iter().any(|key| key.approximate) {
        features.push("keyframes.reveal.bezier_as_linear");
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
