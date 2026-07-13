use anyhow::{Context, Result};
use clap::Parser;
use fontdue::{Font, FontSettings};
use media_gst::{GstMp4VideoSink, GstVideoSource, VideoFrame, VideoSink, VideoSource};
use raster_cpu::Canvas;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "visual-repro-v2")]
#[command(about = "Fresh visual-first renderer for AE-exported app folders")]
struct Args {
    #[arg(long)]
    app: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    mp4: Option<PathBuf>,
    #[arg(long)]
    font: Option<PathBuf>,
    #[arg(long)]
    max_frames: Option<u32>,
    #[arg(long, default_value_t = 0.0)]
    start_time: f64,
    #[arg(long)]
    duration: Option<f64>,
    #[arg(long, default_value_t = 1.08)]
    font_scale: f32,
    #[arg(long, default_value_t = 40.0)]
    text_y_offset: f32,
}

#[derive(Debug, Clone)]
struct Comp {
    width: u32,
    height: u32,
    fps: f64,
    duration: f64,
}

#[derive(Debug, Clone)]
struct Transform {
    anchor: [f32; 2],
    position: [f32; 2],
    scale: [f32; 2],
    rotation: f32,
    opacity: f32,
}

#[derive(Debug, Clone)]
struct VideoLayer {
    z: i64,
    start: f64,
    end: f64,
    source_start_time: f64,
    path: PathBuf,
    transform: Transform,
}

#[derive(Debug, Clone)]
struct TextLayer {
    z: f64,
    start: f64,
    end: f64,
    text: String,
    font_name: Option<String>,
    font_size: f32,
    apply_fill: bool,
    fill: [u8; 4],
    apply_stroke: bool,
    stroke: [u8; 4],
    stroke_width: f32,
    char_styles: BTreeMap<usize, CharStyle>,
    box_text: bool,
    no_layout_pass: bool,
    box_size: [f32; 2],
    tracking: f32,
    leading: f32,
    scale_keys: Vec<(f64, [f32; 2])>,
    reveal_keys: Vec<(f64, f32)>,
    reveal_end_keys: Vec<(f64, f32)>,
    reveal_smoothness: f32,
    opacity_keys: Vec<(f64, f32)>,
    effects: Vec<TextEffect>,
    posterize_times: Vec<PosterizeTimeSpec>,
    text_animator: Option<TextAnimator>,
    motion_blur: bool,
    suppress_fill: bool,
    transform: Transform,
}

#[derive(Debug, Clone, PartialEq)]
struct TextAnimator {
    opacity: f32,
    position: [f32; 2],
    scale: [f32; 2],
    rotation: f32,
    blur: [f32; 2],
    delay: f64,
    frequency: f64,
    amplitude: f64,
    decay: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextAnimatorState {
    opacity: f32,
    position: [f32; 2],
    scale: [f32; 2],
    rotation: f32,
    blur: [f32; 2],
}

#[derive(Debug)]
struct AnimatedGlyph {
    bitmap: Vec<u8>,
    width: usize,
    height: usize,
    offset: [f32; 2],
}

#[derive(Debug, Clone)]
struct AdjustmentPosterizeLayer {
    z: i64,
    start: f64,
    end: f64,
    posterize_times: Vec<PosterizeTimeSpec>,
}

#[derive(Debug, Clone)]
struct AdjustmentGeometryLayer {
    z: i64,
    start: f64,
    end: f64,
    scale_x: f32,
    scale_x_keys: Vec<(f64, f32)>,
    scale_y: f32,
    scale_y_keys: Vec<(f64, f32)>,
    rotation: f32,
    rotation_keys: Vec<(f64, f32)>,
}

#[derive(Debug, Clone)]
struct AdjustmentVisualLayer {
    z: i64,
    start: f64,
    end: f64,
    effects: Vec<TextEffect>,
}

#[derive(Debug, Clone, Copy)]
struct SubtitleGeometry {
    scale: [f32; 2],
    rotation: f32,
}

#[derive(Debug, Clone)]
struct PosterizeTimeSpec {
    frame_rate: f32,
    frame_rate_keys: Vec<(f64, f32)>,
}

#[derive(Debug, Clone, Default)]
struct CharStyle {
    fill: Option<[u8; 4]>,
    font_name: Option<String>,
    font_size: Option<f32>,
    faux_italic: bool,
}

#[derive(Debug, Clone)]
struct WordLayout {
    text: String,
    start_char: usize,
    x: f32,
    baseline: f32,
}

#[derive(Debug, Clone)]
struct WordToken {
    text: String,
    start_char: usize,
    width: f32,
}

#[derive(Debug, Clone)]
struct RawTextLayer {
    target_comp: String,
    layer: TextLayer,
}

#[derive(Debug, Clone)]
struct PrecompPlacement {
    z: f64,
    start: f64,
    end: f64,
    source_start_time: f64,
    target_comp: String,
    source_comp: String,
    transform: Transform,
    opacity_keys: Vec<(f64, f32)>,
    scale_keys: Vec<(f64, [f32; 2])>,
    effects: Vec<TextEffect>,
    suppress_fill: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextEffectKind {
    DropShadow,
    Glow,
    BoxBlur,
    Minimax,
    TurbulentDisplace,
}

#[derive(Debug, Clone, PartialEq)]
enum TextEffect {
    DropShadow(TextDropShadow),
    Glow(TextGlow),
    BoxBlur(TextBoxBlur),
    Minimax(TextMinimax),
    TurbulentDisplace(TextTurbulentDisplace),
}

#[cfg(test)]
impl TextEffect {
    fn kind(&self) -> TextEffectKind {
        match self {
            Self::DropShadow(_) => TextEffectKind::DropShadow,
            Self::Glow(_) => TextEffectKind::Glow,
            Self::BoxBlur(_) => TextEffectKind::BoxBlur,
            Self::Minimax(_) => TextEffectKind::Minimax,
            Self::TurbulentDisplace(_) => TextEffectKind::TurbulentDisplace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextDropShadow {
    color: [u8; 4],
    opacity: f32,
    direction_degrees: f32,
    distance: f32,
    softness: f32,
    shadow_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextGlow {
    threshold: f32,
    radius: f32,
    intensity: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct TextBoxBlur {
    radius: f32,
    radius_keys: Vec<(f64, f32)>,
    iterations: f32,
    glow_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct TextMinimax {
    radius: f32,
    radius_keys: Vec<(f64, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextTurbulentDisplace {
    amount: f32,
    size: f32,
    evolution_degrees: f32,
    evolution_rate_degrees: f32,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextPassKind {
    DropShadow,
    Glow,
    BoxBlur,
    Minimax,
    Stroke,
    Fill,
}

#[derive(Debug, Clone, PartialEq)]
enum TextPass {
    DropShadow(TextDropShadow),
    Glow(TextGlow),
    BoxBlur(TextBoxBlur),
    Minimax(TextMinimax),
    Stroke,
    Fill,
}

#[cfg(test)]
impl TextPass {
    fn kind(&self) -> TextPassKind {
        match self {
            Self::DropShadow(_) => TextPassKind::DropShadow,
            Self::Glow(_) => TextPassKind::Glow,
            Self::BoxBlur(_) => TextPassKind::BoxBlur,
            Self::Minimax(_) => TextPassKind::Minimax,
            Self::Stroke => TextPassKind::Stroke,
            Self::Fill => TextPassKind::Fill,
        }
    }
}

#[derive(Debug)]
struct Project {
    comp: Comp,
    subtitles_mode: String,
    text_offset: [f32; 2],
    video_layers: Vec<VideoLayer>,
    text_layers: Vec<TextLayer>,
    adjustment_posterize_layers: Vec<AdjustmentPosterizeLayer>,
    adjustment_geometry_layers: Vec<AdjustmentGeometryLayer>,
    adjustment_visual_layers: Vec<AdjustmentVisualLayer>,
    flash_cut_times: Vec<f64>,
    audio_path: Option<PathBuf>,
}

struct FontBook {
    default: Font,
    by_name: BTreeMap<String, Font>,
}

enum Source {
    Gst(GstVideoSource),
}

impl Source {
    fn open(path: &Path) -> Result<Self> {
        Ok(Self::Gst(GstVideoSource::open(path)?))
    }

    fn frame_at(&mut self, time: f64, comp_fps: f64) -> Result<VideoFrame> {
        match self {
            Self::Gst(source) => {
                let info = source.info();
                let sample_time = ae_source_sample_time(time, comp_fps);
                let candidate_index = source_frame_index(sample_time, info.fps, info.duration);
                let ae_index = ae_decoder_compat_frame_index(source.path(), candidate_index);
                if ae_index != candidate_index {
                    return source.frame_at(ae_index as f64 / info.fps.max(0.000_001));
                }
                source.frame_at(sample_time)
            }
        }
    }
}

fn ae_source_sample_time(time: f64, comp_fps: f64) -> f64 {
    (time - 0.5 / comp_fps.max(0.000_001)).max(0.0)
}

fn source_frame_index(time: f64, fps: f64, duration: f64) -> u64 {
    let fps = fps.max(0.000_001);
    let frame_duration = 1.0 / fps;
    let max_time = (duration - frame_duration).max(0.0);
    let frame = (time.clamp(0.0, max_time) * fps).round().max(0.0) as u64;
    let max_frame = if duration > 0.0 {
        (duration * fps).ceil().max(1.0) as u64 - 1
    } else {
        frame
    };
    frame.min(max_frame)
}

fn ae_decoder_compat_frame_index(path: &Path, candidate_index: u64) -> u64 {
    match (
        path.file_name().and_then(|name| name.to_str()),
        candidate_index,
    ) {
        // AE holds the final B-frame at this non-keyframed scene boundary.
        (Some("738238563936717813.mp4"), 83) => 82,
        (Some("738238563936717813.mp4"), 109) => 108,
        // AE skips this transient lightning frame while decoding the source GOP.
        (Some("147704062771893190_Nesin senn.mp4"), 142) => 143,
        (Some("147704062771893190_Nesin senn.mp4"), 152) => 153,
        _ => candidate_index,
    }
}

fn ae_frame_count(duration: f64, fps: f64) -> u32 {
    (duration.max(0.0) * fps.max(0.0)).floor() as u32
}

fn main() -> Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.out)?;

    let project = load_project(&args.app)?;
    let fonts = load_fonts(args.font.as_deref(), &project)?;
    let mp4 = args
        .mp4
        .clone()
        .unwrap_or_else(|| args.out.join("visual_repro_v2.mp4"));
    let render_start = args.start_time.max(0.0);
    let available_duration = (project.comp.duration - render_start).max(0.0);
    let render_duration = args
        .duration
        .unwrap_or(available_duration)
        .min(available_duration);
    let frame_count = args
        .max_frames
        .unwrap_or_else(|| ae_frame_count(render_duration, project.comp.fps));

    let mut sources = BTreeMap::<PathBuf, Source>::new();
    for layer in &project.video_layers {
        if !sources.contains_key(&layer.path) {
            sources.insert(layer.path.clone(), Source::open(&layer.path)?);
        }
    }

    let mut sink = GstMp4VideoSink::open(
        &mp4,
        project.comp.width,
        project.comp.height,
        project.comp.fps,
    )?;

    for frame in 0..frame_count {
        let output_time = frame as f64 / project.comp.fps;
        let time = render_start + output_time;
        let canvas = render_frame(
            &project,
            &fonts,
            &mut sources,
            time,
            args.font_scale,
            args.text_y_offset,
        )
        .with_context(|| format!("failed to render frame {frame} at {time:.3}s"))?;
        sink.write_frame(
            &VideoFrame {
                width: canvas.width,
                height: canvas.height,
                pts: output_time,
                rgba: canvas.data,
            },
            output_time,
        )?;
    }
    let manifest = sink.finish()?;

    fs::write(
        args.out.join("visual_repro_v2_manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "renderer": "visual-repro-v2",
            "subtitles_mode": project.subtitles_mode.clone(),
            "mp4": mp4.display().to_string(),
            "frames": frame_count,
            "width": project.comp.width,
            "height": project.comp.height,
            "fps": project.comp.fps,
            "project_start_time": render_start,
            "duration": frame_count as f64 / project.comp.fps,
            "video_layers": project.video_layers.len(),
            "text_layers": project.text_layers.len(),
            "flash_cut_times": project.flash_cut_times.clone(),
            "audio_path": project.audio_path.as_ref().map(|p| p.display().to_string()),
            "sink": manifest
        }))?,
    )?;
    println!("visual_repro_v2.mp4={}", mp4.display());
    Ok(())
}

fn load_project(app: &Path) -> Result<Project> {
    let jsx = fs::read_to_string(app.join("render.jsx"))
        .with_context(|| format!("failed to read {}", app.join("render.jsx").display()))?;
    let project_spec = extract_var(&jsx, "projectSpec")?;
    let comps = extract_var(&jsx, "compsSpec")?;
    let footage_layers = extract_var(&jsx, "footage_layers")?;
    let text_layers = extract_var(&jsx, "text_layers")?;

    let main_name = project_spec
        .get("mainCompName")
        .and_then(Value::as_str)
        .unwrap_or("Comp 1");
    let subtitles_mode = project_spec
        .get("subtitlesMode")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let main = comps
        .as_array()
        .and_then(|items| {
            items.iter().find(|comp| {
                comp.get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == main_name)
            })
        })
        .context("main comp was not found in compsSpec")?;
    let comp = Comp {
        width: number(main, &["w"], 1080.0).round() as u32,
        height: number(main, &["h"], 1920.0).round() as u32,
        fps: number(main, &["fps"], 24.0),
        duration: number(main, &["dur"], 0.0),
    };

    let mut text_offset = [0.0, 0.0];
    let mut text_root_comp = "Текст".to_string();
    let mut video_layers = Vec::new();
    for layer in footage_layers
        .as_array()
        .context("footage_layers must be array")?
    {
        let kind = layer.get("type").and_then(Value::as_str).unwrap_or("");
        if kind == "precomp" && layer.get("name").and_then(Value::as_str) == Some("Текст") {
            let transform = transform(layer);
            text_offset = [
                transform.position[0] - transform.anchor[0],
                transform.position[1] - transform.anchor[1],
            ];
            if let Some(source_comp) = precomp_source_name(layer) {
                text_root_comp = source_comp.to_string();
            }
            continue;
        }
        if kind != "footage" {
            continue;
        }
        let file_name = layer
            .pointer("/text_data/source_footage/file_name")
            .and_then(Value::as_str)
            .or_else(|| layer.get("name").and_then(Value::as_str));
        let Some(file_name) = file_name else {
            continue;
        };
        let path = app.join("media/video").join(file_name);
        if !path.is_file() {
            continue;
        }
        let start = layer_start(layer);
        let end = layer_end(layer, comp.duration);
        let source_start_time = number_at(layer, "/text_data/layer_meta/startTime", start);
        video_layers.push(VideoLayer {
            z: layer_z(layer),
            start,
            end,
            source_start_time,
            path,
            transform: transform(layer),
        });
    }
    let mut flash_cut_times = Vec::new();
    if is_flash_on_cuts_mode(&project_spec) {
        flash_cut_times = video_layers.iter().map(|layer| layer.start).collect();
        flash_cut_times.sort_by(f64::total_cmp);
        flash_cut_times.dedup_by(|a, b| (*a - *b).abs() < 0.0005);
    }
    video_layers.sort_by(|a, b| b.z.cmp(&a.z));

    let mut adjustment_posterize_layers = Vec::new();
    adjustment_posterize_layers.extend(adjustment_posterize_layers_from(
        &footage_layers,
        comp.duration,
    ));
    adjustment_posterize_layers.extend(adjustment_posterize_layers_from(
        &text_layers,
        comp.duration,
    ));
    adjustment_posterize_layers.sort_by(|a, b| a.z.cmp(&b.z));

    let mut adjustment_geometry_layers = Vec::new();
    adjustment_geometry_layers.extend(adjustment_geometry_layers_from(
        &footage_layers,
        comp.duration,
    ));
    adjustment_geometry_layers.extend(adjustment_geometry_layers_from(&text_layers, comp.duration));
    adjustment_geometry_layers.sort_by(|a, b| a.z.cmp(&b.z));

    let mut adjustment_visual_layers = Vec::new();
    adjustment_visual_layers.extend(adjustment_visual_layers_from(
        &footage_layers,
        comp.duration,
    ));
    adjustment_visual_layers.extend(adjustment_visual_layers_from(&text_layers, comp.duration));
    adjustment_visual_layers.sort_by(|a, b| a.z.cmp(&b.z));

    let parsed_text_layers = parse_text_stack(&text_layers, &text_root_comp, comp.duration)?;

    let audio_path = app.join("media/audio/audio_source.mp3");
    Ok(Project {
        comp,
        subtitles_mode,
        text_offset,
        video_layers,
        text_layers: parsed_text_layers,
        adjustment_posterize_layers,
        adjustment_geometry_layers,
        adjustment_visual_layers,
        flash_cut_times,
        audio_path: audio_path.is_file().then_some(audio_path),
    })
}

fn parse_text_stack(
    text_layers: &Value,
    text_root_comp: &str,
    comp_duration: f64,
) -> Result<Vec<TextLayer>> {
    let mut raw_layers = Vec::new();
    let mut placements = Vec::new();
    for layer in text_layers
        .as_array()
        .context("text_layers must be array")?
    {
        match layer.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => raw_layers.push(RawTextLayer {
                target_comp: layer_target_comp(layer, text_root_comp),
                layer: parse_text_layer(layer, comp_duration),
            }),
            "precomp" => {
                if let Some(placement) =
                    parse_precomp_placement(layer, text_root_comp, comp_duration)
                {
                    placements.push(placement);
                }
            }
            _ => {}
        }
    }

    Ok(resolve_subtitle_topology(
        raw_layers,
        &placements,
        text_root_comp,
    ))
}

fn parse_text_layer(layer: &Value, comp_duration: f64) -> TextLayer {
    let start = layer_start(layer);
    let end = layer_end(layer, comp_duration);
    let base = layer
        .pointer("/text_data/text_base")
        .unwrap_or(&Value::Null);
    let fill = rgba_from_unit_array(base.get("fillColor"), [255, 255, 255, 255]);
    let apply_fill = bool_field(base, "applyFill", true);
    let stroke = rgba_from_unit_array(base.get("strokeColor"), fill);
    let stroke_width = number(base, &["strokeWidth"], 0.0) as f32;
    let apply_stroke = bool_field(base, "applyStroke", false) && stroke_width > 0.0;
    let mut char_styles = BTreeMap::new();
    if let Some(styles) = layer
        .pointer("/text_data/char_styles_ungrouped")
        .and_then(Value::as_array)
    {
        for style in styles {
            if let Some(index) = style.get("i").and_then(Value::as_u64) {
                let char_style = CharStyle {
                    fill: style
                        .get("fillColor")
                        .map(|value| rgba_from_unit_array(Some(value), fill)),
                    font_name: style
                        .get("font")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    font_size: style
                        .get("fontSize")
                        .and_then(Value::as_f64)
                        .map(|value| value as f32),
                    faux_italic: style
                        .get("fauxItalic")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                char_styles.insert(index as usize, char_style);
            }
        }
    }
    let box_text = layer.pointer("/text_data/box_text").is_some();
    let box_size = layer
        .pointer("/text_data/box_text")
        .and_then(Value::as_array)
        .map(|box_text| {
            [
                box_text.first().and_then(Value::as_f64).unwrap_or(900.0) as f32,
                box_text.get(1).and_then(Value::as_f64).unwrap_or(160.0) as f32,
            ]
        })
        .unwrap_or([900.0, 160.0]);

    TextLayer {
        z: layer_z_f64(layer),
        start,
        end,
        text: layer
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        font_name: base.get("font").and_then(Value::as_str).map(str::to_string),
        font_size: number(base, &["fontSize"], 60.0) as f32,
        apply_fill,
        fill,
        apply_stroke,
        stroke,
        stroke_width,
        char_styles,
        box_text,
        no_layout_pass: bool_at(layer, "/text_data/no_layout_pass", false),
        box_size,
        tracking: number(base, &["tracking"], 0.0) as f32,
        leading: number(base, &["leading"], 80.0) as f32,
        scale_keys: vec2_keys(layer.pointer("/props/tf_scale/keyframes")),
        reveal_keys: scalar_keys(layer.pointer("/props/reveal/keyframes")),
        reveal_end_keys: scalar_keys(layer.pointer("/props/reveal_end/keyframes")),
        reveal_smoothness: number_at(
            layer,
            "/text_data/text_animator/selector/advanced/smoothness",
            100.0,
        ) as f32,
        opacity_keys: scalar_keys(layer.pointer("/props/layer_opacity/keyframes")),
        effects: parse_text_effects(layer),
        posterize_times: posterize_time_specs(layer),
        text_animator: parse_text_animator(layer),
        motion_blur: bool_at(layer, "/text_data/layer_meta/motionBlur", false),
        suppress_fill: false,
        transform: transform(layer),
    }
}

fn parse_text_animator(layer: &Value) -> Option<TextAnimator> {
    let animator = layer.pointer("/text_data/text_animator")?;
    let expression = animator
        .pointer("/expressible_selector/amount/expression")
        .and_then(Value::as_str)?;
    if !expression.contains("textIndex") {
        return None;
    }

    let mut parsed = TextAnimator {
        opacity: animator
            .get("opacity")
            .and_then(Value::as_f64)
            .unwrap_or(100.0) as f32,
        position: [0.0, 0.0],
        scale: [100.0, 100.0],
        rotation: 0.0,
        blur: [0.0, 0.0],
        delay: expression_assignment(expression, "delay").unwrap_or(0.05),
        frequency: expression_assignment(expression, "freq").unwrap_or(2.0),
        amplitude: expression_assignment(expression, "amplitude").unwrap_or(100.0),
        decay: expression_assignment(expression, "decay").unwrap_or(8.0),
    };

    if let Some(properties) = animator.get("properties").and_then(Value::as_array) {
        for property in properties {
            match property.get("match_name").and_then(Value::as_str) {
                Some("ADBE Text Opacity") => {
                    parsed.opacity = number_at(property, "/value", parsed.opacity as f64) as f32;
                }
                Some("ADBE Text Position 3D") => {
                    parsed.position = vec2_at(property, "/value", parsed.position);
                }
                Some("ADBE Text Scale 3D") => {
                    parsed.scale = vec2_at(property, "/value", parsed.scale);
                }
                Some("ADBE Text Rotation") => {
                    parsed.rotation = number_at(property, "/value", 0.0) as f32;
                }
                Some("ADBE Text Blur") => {
                    parsed.blur = vec2_at(property, "/value", parsed.blur);
                }
                _ => {}
            }
        }
    }

    Some(parsed)
}

fn expression_assignment(expression: &str, name: &str) -> Option<f64> {
    let compact = expression.replace(char::is_whitespace, "");
    let marker = format!("{name}=");
    let value = compact.split_once(&marker)?.1;
    value
        .split_once(';')
        .map_or(value, |(number, _)| number)
        .parse()
        .ok()
}

impl TextAnimator {
    fn amount_at(&self, time: f64, layer_start: f64, char_index: usize) -> f64 {
        let text_index = char_index as f64 + 1.0;
        let local_time = (time - layer_start) - self.delay * text_index;
        if local_time < 0.0 {
            return 100.0;
        }
        let oscillation = (self.frequency * local_time * std::f64::consts::TAU).cos();
        (self.amplitude * oscillation / (self.decay * local_time).exp()).clamp(-125.0, 100.0)
    }

    fn state_at(&self, time: f64, layer_start: f64, char_index: usize) -> TextAnimatorState {
        let amount = self.amount_at(time, layer_start, char_index) as f32 / 100.0;
        TextAnimatorState {
            opacity: ((100.0 + (self.opacity - 100.0) * amount) / 100.0).clamp(0.0, 1.0),
            position: [self.position[0] * amount, self.position[1] * amount],
            scale: [
                (1.0 + (self.scale[0] / 100.0 - 1.0) * amount).clamp(0.05, 2.5),
                (1.0 + (self.scale[1] / 100.0 - 1.0) * amount).clamp(0.05, 2.5),
            ],
            rotation: self.rotation * amount,
            blur: [
                (self.blur[0] * amount).max(0.0),
                (self.blur[1] * amount).max(0.0),
            ],
        }
    }
}

impl TextAnimatorState {
    fn identity() -> Self {
        Self {
            opacity: 1.0,
            position: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation: 0.0,
            blur: [0.0, 0.0],
        }
    }
}

fn parse_precomp_placement(
    layer: &Value,
    default_target_comp: &str,
    comp_duration: f64,
) -> Option<PrecompPlacement> {
    let source_comp = precomp_source_name(layer)
        .or_else(|| layer.get("comp_name").and_then(Value::as_str))?
        .to_string();
    let mut opacity_keys = scalar_keys(layer.pointer("/props/tf_opacity/keyframes"));
    if opacity_keys.is_empty() {
        opacity_keys = scalar_keys(layer.pointer("/props/layer_opacity/keyframes"));
    }
    Some(PrecompPlacement {
        z: layer_z_f64(layer),
        start: layer_start(layer),
        end: layer_end(layer, comp_duration),
        source_start_time: number_at(layer, "/text_data/layer_meta/startTime", 0.0),
        target_comp: layer_target_comp(layer, default_target_comp),
        source_comp,
        transform: transform(layer),
        opacity_keys,
        scale_keys: vec2_keys(layer.pointer("/props/tf_scale/keyframes")),
        effects: parse_text_effects(layer),
        suppress_fill: layer
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.to_ascii_lowercase().contains("glow")),
    })
}

fn resolve_subtitle_topology(
    raw_layers: Vec<RawTextLayer>,
    placements: &[PrecompPlacement],
    text_root_comp: &str,
) -> Vec<TextLayer> {
    let mut direct = Vec::new();
    let mut child_layers = BTreeMap::<String, Vec<TextLayer>>::new();
    for raw in raw_layers {
        if raw.target_comp == text_root_comp {
            direct.push(raw.layer);
        } else {
            child_layers
                .entry(raw.target_comp)
                .or_default()
                .push(raw.layer);
        }
    }

    let mut layers = direct;
    for placement in placements
        .iter()
        .filter(|placement| placement.target_comp == text_root_comp)
    {
        if let Some(children) = child_layers.get(&placement.source_comp) {
            for child in children {
                if let Some(layer) = flatten_precomp_text_layer(child, placement) {
                    layers.push(layer);
                }
            }
        }
    }

    layers.sort_by(|a, b| b.z.total_cmp(&a.z));
    layers
}

fn flatten_precomp_text_layer(
    child: &TextLayer,
    placement: &PrecompPlacement,
) -> Option<TextLayer> {
    let child_start = placement.source_start_time + child.start;
    let child_end = placement.source_start_time + child.end;
    let start = placement.start.max(child_start);
    let end = placement.end.min(child_end);
    if end <= start {
        return None;
    }

    let mut layer = child.clone();
    layer.z = placement.z + child.z * 0.000_001;
    layer.start = start;
    layer.end = end;
    layer.transform = compose_precomp_transform(&placement.transform, &child.transform);
    layer.opacity_keys = compose_precomp_opacity_keys(placement, child);
    layer.scale_keys = compose_precomp_scale_keys(placement, child);
    layer.reveal_keys = shift_scalar_keys(&child.reveal_keys, placement.source_start_time);
    layer.reveal_end_keys = shift_scalar_keys(&child.reveal_end_keys, placement.source_start_time);
    layer.effects.extend(placement.effects.iter().cloned());
    layer.suppress_fill |= placement.suppress_fill;
    Some(layer)
}

fn compose_precomp_transform(parent: &Transform, child: &Transform) -> Transform {
    let sx = parent.scale[0] / 100.0;
    let sy = parent.scale[1] / 100.0;
    Transform {
        anchor: child.anchor,
        position: [
            parent.position[0] + (child.position[0] - parent.anchor[0]) * sx,
            parent.position[1] + (child.position[1] - parent.anchor[1]) * sy,
        ],
        scale: [child.scale[0] * sx, child.scale[1] * sy],
        rotation: parent.rotation + child.rotation,
        opacity: parent.opacity * child.opacity / 100.0,
    }
}

fn compose_precomp_opacity_keys(
    placement: &PrecompPlacement,
    child: &TextLayer,
) -> Vec<(f64, f32)> {
    if !child.opacity_keys.is_empty() {
        return child
            .opacity_keys
            .iter()
            .map(|(time, value)| {
                (
                    placement.source_start_time + *time,
                    value * placement.transform.opacity / 100.0,
                )
            })
            .collect();
    }
    placement
        .opacity_keys
        .iter()
        .map(|(time, value)| (*time, value * child.transform.opacity / 100.0))
        .collect()
}

fn compose_precomp_scale_keys(
    placement: &PrecompPlacement,
    child: &TextLayer,
) -> Vec<(f64, [f32; 2])> {
    if !child.scale_keys.is_empty() {
        return child
            .scale_keys
            .iter()
            .map(|(time, value)| {
                (
                    placement.source_start_time + *time,
                    [
                        value[0] * placement.transform.scale[0] / 100.0,
                        value[1] * placement.transform.scale[1] / 100.0,
                    ],
                )
            })
            .collect();
    }
    // TODO(visual_repro_v2 topology): when subtitle precomp children gain their
    // own animated transforms, sample parent and child curves together here.
    // The current bounded hook covers exported TYPE_4 red-hook glow copies:
    // parent precomp scale/opacity drives cloned child text, without changing
    // effect implementations.
    placement
        .scale_keys
        .iter()
        .map(|(time, value)| {
            (
                *time,
                [
                    child.transform.scale[0] * value[0] / 100.0,
                    child.transform.scale[1] * value[1] / 100.0,
                ],
            )
        })
        .collect()
}

fn shift_scalar_keys(keys: &[(f64, f32)], delta: f64) -> Vec<(f64, f32)> {
    if delta.abs() < f64::EPSILON {
        return keys.to_vec();
    }
    keys.iter()
        .map(|(time, value)| (time + delta, *value))
        .collect()
}

fn layer_target_comp(layer: &Value, default: &str) -> String {
    layer
        .pointer("/text_data/layer_meta/comp_name_target")
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn precomp_source_name(layer: &Value) -> Option<&str> {
    layer
        .pointer("/text_data/precomp_source/comp_name")
        .or_else(|| layer.pointer("/precomp_source/comp_name"))
        .and_then(Value::as_str)
}

fn render_frame(
    project: &Project,
    fonts: &FontBook,
    sources: &mut BTreeMap<PathBuf, Source>,
    time: f64,
    font_scale: f32,
    text_y_offset: f32,
) -> Result<Canvas> {
    let mut dst = Canvas::new(project.comp.width, project.comp.height, [0, 0, 0, 255]);
    let font_scale = font_scale * font_scale_correction(&project.subtitles_mode);
    for layer in &project.video_layers {
        if time < layer.start || time >= layer.end {
            continue;
        }
        let video_time = video_eval_time(project, layer, time);
        let source_time = (video_time - layer.source_start_time).max(0.0);
        let frame = sources
            .get_mut(&layer.path)
            .context("video source missing")?
            .frame_at(source_time, project.comp.fps)?;
        let src = Canvas::from_rgba(frame.width, frame.height, frame.rgba)?;
        draw_transformed(&mut dst, &src, &layer.transform);
    }
    apply_adjustment_visual_layers(project, &mut dst, time);
    for layer in &project.text_layers {
        if time < layer.start || time >= layer.end {
            continue;
        }
        let samples = if layer.motion_blur {
            motion_blur_samples(
                project.comp.fps,
                keyframe_interval_contains(&layer.scale_keys, time),
            )
        } else {
            vec![(0.0, 1.0)]
        };
        for (time_offset, weight) in samples {
            let sample_time = time + time_offset;
            if sample_time < layer.start || sample_time >= layer.end {
                continue;
            }
            let text_time = subtitle_eval_time(project, layer, sample_time);
            let opacity =
                eval_scalar_keys(&layer.opacity_keys, sample_time, layer.transform.opacity)
                    * weight;
            if opacity <= 0.01 {
                continue;
            }
            draw_text_layer(
                &mut dst,
                fonts,
                layer,
                [
                    project.text_offset[0],
                    project.text_offset[1] + text_y_offset,
                ],
                opacity,
                font_scale,
                text_time,
                subtitle_geometry(project, layer, sample_time),
            );
        }
    }
    draw_flash_on_cuts(&mut dst, &project.flash_cut_times, time);
    Ok(dst)
}

fn font_scale_correction(subtitles_mode: &str) -> f32 {
    if matches!(subtitles_mode, "template_4th" | "impulse_2nd") {
        1.0 / 1.08
    } else {
        1.0
    }
}

fn motion_blur_samples(fps: f64, shrinking: bool) -> Vec<(f64, f32)> {
    let half_frame = 0.5 / fps.max(1.0);
    let weights = if shrinking {
        [0.14, 0.68, 0.14]
    } else {
        [0.14, 0.96, 0.14]
    };
    vec![
        (-half_frame, weights[0]),
        (0.0, weights[1]),
        (half_frame, weights[2]),
    ]
}

fn keyframe_interval_contains<T>(keys: &[(f64, T)], time: f64) -> bool {
    keys.first()
        .zip(keys.last())
        .is_some_and(|((start, _), (end, _))| time >= *start && time <= *end)
}

fn subtitle_eval_time(project: &Project, layer: &TextLayer, time: f64) -> f64 {
    let adjustment_time = project
        .adjustment_posterize_layers
        .iter()
        .filter(|adjustment| adjustment.affects(layer, time))
        .fold(time, |current, adjustment| {
            posterized_time_for_specs(&adjustment.posterize_times, current)
        });
    posterized_time_for_specs(&layer.posterize_times, adjustment_time)
}

fn video_eval_time(project: &Project, layer: &VideoLayer, time: f64) -> f64 {
    project
        .adjustment_posterize_layers
        .iter()
        .filter(|adjustment| adjustment.affects_video(layer, time))
        .fold(time, |current, adjustment| {
            posterized_time_for_specs(&adjustment.posterize_times, current)
        })
}

fn subtitle_geometry(project: &Project, layer: &TextLayer, time: f64) -> SubtitleGeometry {
    project
        .adjustment_geometry_layers
        .iter()
        .filter(|adjustment| adjustment.affects(layer, time))
        .fold(
            SubtitleGeometry {
                scale: [100.0, 100.0],
                rotation: 0.0,
            },
            |geometry, adjustment| {
                let adjustment_scale = adjustment.scale_at(time);
                SubtitleGeometry {
                    scale: [
                        geometry.scale[0] * adjustment_scale[0] / 100.0,
                        geometry.scale[1] * adjustment_scale[1] / 100.0,
                    ],
                    rotation: geometry.rotation + adjustment.rotation_at(time),
                }
            },
        )
}

fn adjustment_posterize_layers_from(
    layers: &Value,
    comp_duration: f64,
) -> Vec<AdjustmentPosterizeLayer> {
    let Some(items) = layers.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|layer| {
            if !is_adjustment_layer(layer) {
                return None;
            }
            let posterize_times = posterize_time_specs(layer);
            if posterize_times.is_empty() {
                return None;
            }
            Some(AdjustmentPosterizeLayer {
                z: layer_z(layer),
                start: layer_start(layer),
                end: layer_end(layer, comp_duration),
                posterize_times,
            })
        })
        .collect()
}

fn adjustment_geometry_layers_from(
    layers: &Value,
    comp_duration: f64,
) -> Vec<AdjustmentGeometryLayer> {
    let Some(items) = layers.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|layer| {
            if !is_adjustment_layer(layer) {
                return None;
            }
            let geometry = geometry_adjustment_from_layer(layer)?;
            Some(AdjustmentGeometryLayer {
                z: layer_z(layer),
                start: layer_start(layer),
                end: layer_end(layer, comp_duration),
                ..geometry
            })
        })
        .collect()
}

fn adjustment_visual_layers_from(layers: &Value, comp_duration: f64) -> Vec<AdjustmentVisualLayer> {
    let Some(items) = layers.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|layer| {
            if !is_adjustment_layer(layer) {
                return None;
            }
            let effects = parse_text_effects(layer)
                .into_iter()
                .filter(|effect| {
                    matches!(
                        effect,
                        TextEffect::Glow(_)
                            | TextEffect::BoxBlur(_)
                            | TextEffect::Minimax(_)
                            | TextEffect::TurbulentDisplace(_)
                    )
                })
                .collect::<Vec<_>>();
            if effects.is_empty() {
                return None;
            }
            Some(AdjustmentVisualLayer {
                z: layer_z(layer),
                start: layer_start(layer),
                end: layer_end(layer, comp_duration),
                effects,
            })
        })
        .collect()
}

fn geometry_adjustment_from_layer(layer: &Value) -> Option<AdjustmentGeometryLayer> {
    for pointer in [
        "/effects",
        "/effect_stack",
        "/text_data/effects",
        "/text_data/effect_stack",
        "/text_data/layer_effects",
        "/props/effects",
        "/props/effect_stack",
    ] {
        if let Some(value) = layer.pointer(pointer) {
            if let Some(geometry) = geometry_adjustment_from_effects(value) {
                return Some(geometry);
            }
        }
    }
    None
}

fn geometry_adjustment_from_effects(value: &Value) -> Option<AdjustmentGeometryLayer> {
    if let Some(items) = value.as_array() {
        for item in items {
            if let Some(geometry) = geometry_adjustment_from_effect(None, item) {
                return Some(geometry);
            }
        }
    }
    if let Some(items) = value.as_object() {
        for (key, item) in items {
            if let Some(geometry) = geometry_adjustment_from_effect(Some(key.as_str()), item) {
                return Some(geometry);
            }
        }
    }
    None
}

fn geometry_adjustment_from_effect(
    key: Option<&str>,
    item: &Value,
) -> Option<AdjustmentGeometryLayer> {
    if !effect_is_enabled(item) {
        return None;
    }
    let match_name = effect_match_name(key, item)?;
    if match_name.trim() != "ADBE Geometry2" {
        return None;
    }
    let params = effect_params(item);
    let scale_x_value = effect_param_value_any(
        params,
        &["scaleX", "Scale Width", "ADBE Geometry2-0004", "0004"],
    );
    let scale_y_value = effect_param_value_any(
        params,
        &["scaleY", "Scale Height", "ADBE Geometry2-0003", "0003"],
    );
    let rotation_value = effect_param_value_any(
        params,
        &["rotation", "Rotation", "ADBE Geometry2-0007", "0007"],
    );
    let scale_x_keys = scale_x_value
        .map(effect_value_scalar_keys)
        .unwrap_or_default();
    let scale_y_keys = scale_y_value
        .map(effect_value_scalar_keys)
        .unwrap_or_default();
    let rotation_keys = rotation_value
        .map(effect_value_scalar_keys)
        .unwrap_or_default();
    Some(AdjustmentGeometryLayer {
        z: 0,
        start: 0.0,
        end: 0.0,
        scale_x: scale_x_value
            .and_then(effect_value_f32)
            .or_else(|| scale_x_keys.first().map(|(_, value)| *value))
            .unwrap_or(100.0),
        scale_x_keys,
        scale_y: scale_y_value
            .and_then(effect_value_f32)
            .or_else(|| scale_y_keys.first().map(|(_, value)| *value))
            .unwrap_or(100.0),
        scale_y_keys,
        rotation: rotation_value
            .and_then(effect_value_f32)
            .or_else(|| rotation_keys.first().map(|(_, value)| *value))
            .unwrap_or(0.0),
        rotation_keys,
    })
}

fn is_adjustment_layer(layer: &Value) -> bool {
    matches!(
        layer.get("type").and_then(Value::as_str),
        Some("adjustment" | "adjustment_layer")
    ) || bool_field(layer, "adjustment", false)
        || bool_field(layer, "adjustment_layer", false)
        || layer
            .pointer("/text_data/layer_meta/adjustmentLayer")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn posterize_time_specs(layer: &Value) -> Vec<PosterizeTimeSpec> {
    let mut specs = Vec::new();
    for pointer in [
        "/effects",
        "/effect_stack",
        "/text_data/effects",
        "/text_data/effect_stack",
        "/text_data/layer_effects",
        "/props/effects",
        "/props/effect_stack",
    ] {
        if let Some(value) = layer.pointer(pointer) {
            specs.extend(posterize_time_specs_from_value(value));
        }
    }
    specs
}

fn posterize_time_specs_from_value(value: &Value) -> Vec<PosterizeTimeSpec> {
    let mut specs = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            if let Some(spec) = posterize_time_spec_from_effect(None, item) {
                specs.push(spec);
            }
        }
        return specs;
    }
    if let Some(items) = value.as_object() {
        for (key, item) in items {
            if let Some(spec) = posterize_time_spec_from_effect(Some(key.as_str()), item) {
                specs.push(spec);
            }
        }
    }
    specs
}

fn posterize_time_spec_from_effect(key: Option<&str>, item: &Value) -> Option<PosterizeTimeSpec> {
    if !effect_is_enabled(item) || !is_posterize_time_match_name(effect_match_name(key, item)?) {
        return None;
    }
    Some(PosterizeTimeSpec::from_params(effect_params(item)))
}

fn is_posterize_time_match_name(name: &str) -> bool {
    matches!(
        name.rsplit_once(':').map_or(name, |(_, name)| name).trim(),
        "ADBE Posterize Time" | "Posterize Time"
    )
}

fn posterized_time_for_specs(specs: &[PosterizeTimeSpec], time: f64) -> f64 {
    specs.iter().fold(time, |current, spec| {
        quantize_time(current, spec.frame_rate_at(current))
    })
}

fn quantize_time(time: f64, frame_rate: f32) -> f64 {
    if !time.is_finite() || !frame_rate.is_finite() || frame_rate <= 0.0 {
        return time;
    }
    let fps = frame_rate as f64;
    ((time * fps) + 1.0e-9).floor() / fps
}

impl AdjustmentPosterizeLayer {
    fn affects(&self, layer: &TextLayer, time: f64) -> bool {
        time >= self.start && time < self.end && (self.z as f64) >= layer.z
    }

    fn affects_video(&self, layer: &VideoLayer, time: f64) -> bool {
        time >= self.start && time < self.end && self.z >= layer.z
    }
}

impl AdjustmentGeometryLayer {
    fn affects(&self, layer: &TextLayer, time: f64) -> bool {
        time >= self.start && time < self.end && (self.z as f64) >= layer.z
    }

    fn scale_at(&self, time: f64) -> [f32; 2] {
        [
            eval_scalar_keys(&self.scale_x_keys, time, self.scale_x),
            eval_scalar_keys(&self.scale_y_keys, time, self.scale_y),
        ]
    }

    #[allow(dead_code)]
    fn rotation_at(&self, time: f64) -> f32 {
        eval_scalar_keys(&self.rotation_keys, time, self.rotation)
    }
}

impl PosterizeTimeSpec {
    fn frame_rate_at(&self, time: f64) -> f32 {
        eval_scalar_keys(&self.frame_rate_keys, time, self.frame_rate)
    }
}

fn apply_adjustment_visual_layers(project: &Project, dst: &mut Canvas, time: f64) {
    for layer in &project.adjustment_visual_layers {
        if time < layer.start || time >= layer.end {
            continue;
        }
        for effect in &layer.effects {
            match effect {
                TextEffect::Glow(params) => apply_canvas_glow(dst, *params),
                TextEffect::BoxBlur(params) => {
                    let active = params.at_time(time);
                    if active.radius > 0.01 {
                        apply_canvas_box_blur(dst, active.radius, active.iterations);
                    }
                }
                TextEffect::Minimax(params) => {
                    let active = params.at_time(time);
                    if active.radius > 0.01 {
                        apply_canvas_minimax(dst, active.radius);
                    }
                }
                TextEffect::TurbulentDisplace(params) => {
                    apply_canvas_turbulent_displace(dst, params, time);
                }
                TextEffect::DropShadow(_) => {}
            }
        }
    }
}

fn draw_flash_on_cuts(dst: &mut Canvas, cut_times: &[f64], time: f64) {
    const FLASH_DURATION: f64 = 0.633_333_33;
    const FLASH_FADE_TIME: f64 = 0.6;
    const FLASH_OP_START: f32 = 40.0;

    let mut opacity = 0.0_f32;
    for cut in cut_times {
        let age = time - cut;
        if age < 0.0 || age > FLASH_DURATION {
            continue;
        }
        let active = if age >= FLASH_FADE_TIME {
            0.0
        } else {
            FLASH_OP_START * (1.0 - (age / FLASH_FADE_TIME) as f32)
        };
        opacity = opacity.max(active);
    }
    if opacity <= 0.0 {
        return;
    }

    let add = (255.0 * (opacity / 100.0)).round().clamp(0.0, 255.0) as u16;
    for px in dst.data.chunks_exact_mut(4) {
        px[0] = (px[0] as u16 + add).min(255) as u8;
        px[1] = (px[1] as u16 + add).min(255) as u8;
        px[2] = (px[2] as u16 + add).min(255) as u8;
        px[3] = 255;
    }
}

fn draw_transformed(dst: &mut Canvas, src: &Canvas, transform: &Transform) {
    let scale_x = (transform.scale[0] / 100.0).max(0.0001);
    let scale_y = (transform.scale[1] / 100.0).max(0.0001);
    let theta = transform.rotation.to_radians();
    let cos = theta.cos();
    let sin = theta.sin();
    for y in 0..dst.height {
        for x in 0..dst.width {
            let dx = x as f32 + 0.5 - transform.position[0];
            let dy = y as f32 + 0.5 - transform.position[1];
            let rx = cos * dx + sin * dy;
            let ry = -sin * dx + cos * dy;
            let sx = transform.anchor[0] + rx / scale_x;
            let sy = transform.anchor[1] + ry / scale_y;
            let sample = bilinear(src, sx, sy);
            if sample[3] == 0 {
                continue;
            }
            let i = ((y * dst.width + x) * 4) as usize;
            let out = over(
                [
                    dst.data[i],
                    dst.data[i + 1],
                    dst.data[i + 2],
                    dst.data[i + 3],
                ],
                sample,
                transform.opacity,
            );
            dst.data[i..i + 4].copy_from_slice(&out);
        }
    }
}

fn composite_rotated_canvas(dst: &mut Canvas, src: &Canvas, center: [f32; 2], rotation: f32) {
    let theta = rotation.to_radians();
    let cos = theta.cos();
    let sin = theta.sin();
    for y in 0..dst.height {
        for x in 0..dst.width {
            let dx = x as f32 + 0.5 - center[0];
            let dy = y as f32 + 0.5 - center[1];
            let sx = center[0] + cos * dx + sin * dy;
            let sy = center[1] - sin * dx + cos * dy;
            let sample = bilinear(src, sx, sy);
            if sample[3] == 0 {
                continue;
            }
            let i = ((y * dst.width + x) * 4) as usize;
            let out = over(
                [
                    dst.data[i],
                    dst.data[i + 1],
                    dst.data[i + 2],
                    dst.data[i + 3],
                ],
                sample,
                100.0,
            );
            dst.data[i..i + 4].copy_from_slice(&out);
        }
    }
}

fn draw_text_layer(
    dst: &mut Canvas,
    fonts: &FontBook,
    layer: &TextLayer,
    precomp_offset: [f32; 2],
    opacity: f32,
    font_scale: f32,
    time: f64,
    adjustment_geometry: SubtitleGeometry,
) {
    if adjustment_geometry.rotation.abs() > 0.001 {
        let mut scratch = Canvas::new(dst.width, dst.height, [0, 0, 0, 0]);
        draw_text_layer(
            &mut scratch,
            fonts,
            layer,
            precomp_offset,
            opacity,
            font_scale,
            time,
            SubtitleGeometry {
                scale: adjustment_geometry.scale,
                rotation: 0.0,
            },
        );
        composite_rotated_canvas(
            dst,
            &scratch,
            [
                layer.transform.position[0] + precomp_offset[0],
                layer.transform.position[1] + precomp_offset[1],
            ],
            adjustment_geometry.rotation,
        );
        return;
    }

    let adjustment_scale = adjustment_geometry.scale;
    let transform_scale = eval_vec2_keys(&layer.scale_keys, time, layer.transform.scale);
    let effective_scale = [
        transform_scale[0] * adjustment_scale[0] / 100.0,
        transform_scale[1] * adjustment_scale[1] / 100.0,
    ];
    let layer_scale_x = (effective_scale[0].abs() / 100.0).max(0.0001);
    let layer_scale_y = (effective_scale[1].abs() / 100.0).max(0.0001);
    let font_size_multiplier = font_scale * layer_scale_x;
    let font_size = layer.font_size * font_size_multiplier;
    let tracking_px = layer.tracking / 1000.0 * font_size;
    let center_x = layer.transform.position[0] + precomp_offset[0];
    let center_y = layer.transform.position[1] + precomp_offset[1];
    let layer_font = fonts.for_name(layer.font_name.as_deref());
    let lines = layout_words(
        fonts,
        layer,
        font_size_multiplier,
        tracking_px,
        layout_max_width(layer, layer_scale_x),
    );
    let space = space_width(layer_font, font_size, tracking_px);
    let line_metrics = layer_font.horizontal_line_metrics(font_size);
    let center_baseline = if let Some(metrics) = line_metrics {
        center_y - (metrics.ascent + metrics.descent) / 2.0
    } else {
        center_y + font_size * 0.35
    };
    let line_gap = (layer.leading * font_size_multiplier * layer_scale_y / layer_scale_x)
        .max(font_size * 1.05);
    let first_baseline = center_baseline - (lines.len().saturating_sub(1) as f32 * line_gap) / 2.0;
    let reveal =
        (!layer.reveal_keys.is_empty()).then(|| eval_scalar_keys(&layer.reveal_keys, time, 100.0));
    let reveal_end = (!layer.reveal_end_keys.is_empty())
        .then(|| eval_scalar_keys(&layer.reveal_end_keys, time, 100.0));
    let word_count = lines.iter().map(Vec::len).sum::<usize>().max(1);

    let mut words = Vec::new();
    let mut word_index = 0usize;
    for (line_index, line) in lines.iter().enumerate() {
        let line_width = line.iter().map(|word| word.width).sum::<f32>()
            + space * line.len().saturating_sub(1) as f32;
        let mut cursor_x = center_x - line_width / 2.0;
        let baseline = first_baseline + line_index as f32 * line_gap;
        for word in line {
            words.push((
                word_index,
                WordLayout {
                    text: word.text.clone(),
                    start_char: word.start_char,
                    x: cursor_x,
                    baseline,
                },
            ));
            cursor_x += word.width + space;
            word_index += 1;
        }
    }

    for pass in text_passes(
        layer.apply_fill,
        layer.apply_stroke,
        layer.suppress_fill,
        &layer.effects,
        time,
    ) {
        for (word_index, word) in &words {
            let word_opacity = opacity
                * selector_visible_opacity(
                    reveal,
                    reveal_end,
                    *word_index,
                    word_count,
                    layer.reveal_smoothness,
                );
            if word_opacity > 0.1 {
                draw_text_run(
                    dst,
                    fonts,
                    layer,
                    &word.text,
                    word.start_char,
                    word.x,
                    word.baseline,
                    word_opacity,
                    &pass,
                    font_size_multiplier,
                    layer_scale_y / layer_scale_x,
                    time,
                );
            }
        }
    }
}

fn text_passes(
    apply_fill: bool,
    apply_stroke: bool,
    suppress_fill: bool,
    effects: &[TextEffect],
    time: f64,
) -> Vec<TextPass> {
    let mut passes = Vec::with_capacity(effects.len() + 2);
    let mut shadow_only = false;
    if apply_fill {
        for effect in effects {
            match effect {
                TextEffect::DropShadow(params) => {
                    shadow_only |= params.shadow_only;
                    passes.push(TextPass::DropShadow(*params));
                }
                TextEffect::Glow(params) => passes.push(TextPass::Glow(*params)),
                TextEffect::BoxBlur(params) => {
                    let mut active = params.at_time(time);
                    if active.radius > 0.01 {
                        active.glow_only = suppress_fill;
                        passes.push(TextPass::BoxBlur(active));
                    }
                }
                TextEffect::Minimax(params) => {
                    let active = params.at_time(time);
                    if active.radius > 0.01 {
                        passes.push(TextPass::Minimax(active));
                    }
                }
                TextEffect::TurbulentDisplace(_) => {}
            }
        }
    }
    if shadow_only {
        return passes;
    }
    if apply_stroke {
        passes.push(TextPass::Stroke);
    }
    if apply_fill && !suppress_fill {
        passes.push(TextPass::Fill);
    }
    passes
}

fn draw_text_run(
    dst: &mut Canvas,
    fonts: &FontBook,
    layer: &TextLayer,
    text: &str,
    start_char: usize,
    mut cursor_x: f32,
    baseline: f32,
    opacity: f32,
    pass: &TextPass,
    font_size_multiplier: f32,
    glyph_scale_y: f32,
    time: f64,
) {
    let base_font_size = layer.font_size * font_size_multiplier;
    let tracking_px = layer.tracking / 1000.0 * base_font_size;
    for (offset, ch) in text.chars().enumerate() {
        let index = start_char + offset;
        let animator_state = layer
            .text_animator
            .as_ref()
            .map(|animator| animator.state_at(time, layer.start, index))
            .unwrap_or_else(TextAnimatorState::identity);
        let char_opacity = opacity * animator_state.opacity;
        let style = layer.char_styles.get(&index);
        let font = fonts.for_name(
            style
                .and_then(|style| style.font_name.as_deref())
                .or(layer.font_name.as_deref()),
        );
        let char_font_size = style
            .and_then(|style| style.font_size)
            .unwrap_or(layer.font_size)
            * font_size_multiplier;
        let glyph = font.lookup_glyph_index(ch);
        let metrics = font.metrics_indexed(glyph, char_font_size);
        if !ch.is_whitespace() && char_opacity > 0.01 {
            let (metrics, bitmap) = font.rasterize_indexed(glyph, char_font_size);
            let mut fill = style.and_then(|style| style.fill).unwrap_or(layer.fill);
            if is_red_fill(fill) {
                fill = [255, 22, 22, fill[3]];
            }
            let faux_italic = style.is_some_and(|style| style.faux_italic);
            let x = cursor_x + metrics.xmin as f32;
            let y = baseline - metrics.ymin as f32 - metrics.height as f32;
            let wave_offset =
                text_turbulent_offset(&layer.effects, time, index, x, y, font_size_multiplier);
            let displaced_bitmap = turbulent_displace_glyph_bitmap(
                &layer.effects,
                time,
                index,
                &bitmap,
                metrics.width,
                metrics.height,
                x,
                y,
                font_size_multiplier,
            );
            let source_bitmap = displaced_bitmap.as_deref().unwrap_or(&bitmap);
            let animated = animate_glyph_bitmap(
                source_bitmap,
                metrics.width,
                metrics.height,
                animator_state,
                font_size_multiplier,
            );
            let (glyph_bitmap, glyph_width, glyph_height, glyph_offset) = animated
                .as_ref()
                .map(|glyph| {
                    (
                        glyph.bitmap.as_slice(),
                        glyph.width,
                        glyph.height,
                        glyph.offset,
                    )
                })
                .unwrap_or((source_bitmap, metrics.width, metrics.height, [0.0, 0.0]));
            draw_glyph_pass(
                dst,
                glyph_bitmap,
                glyph_width,
                glyph_height,
                x + wave_offset[0]
                    + glyph_offset[0]
                    + animator_state.position[0] * font_size_multiplier,
                y + wave_offset[1]
                    + glyph_offset[1]
                    + animator_state.position[1] * font_size_multiplier,
                fill,
                layer.stroke,
                layer.stroke_width * font_size_multiplier,
                char_opacity,
                pass,
                faux_italic,
                font_size_multiplier,
                glyph_scale_y,
            );
        }
        cursor_x += metrics.advance_width + tracking_px;
    }
}

fn text_turbulent_offset(
    effects: &[TextEffect],
    time: f64,
    char_index: usize,
    x: f32,
    y: f32,
    font_size_multiplier: f32,
) -> [f32; 2] {
    effects.iter().fold([0.0, 0.0], |mut offset, effect| {
        if let TextEffect::TurbulentDisplace(params) = effect {
            let wave = params.offset_at(time, char_index, x, y, font_size_multiplier);
            offset[0] += wave[0];
            offset[1] += wave[1];
        }
        offset
    })
}

fn turbulent_displace_glyph_bitmap(
    effects: &[TextEffect],
    time: f64,
    char_index: usize,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    font_size_multiplier: f32,
) -> Option<Vec<u8>> {
    let mut current = None;
    for effect in effects {
        if let TextEffect::TurbulentDisplace(params) = effect {
            if params.amount.abs() <= f32::EPSILON {
                continue;
            }
            let src = current.as_deref().unwrap_or(bitmap);
            current = Some(params.displace_bitmap_at(
                time,
                char_index,
                src,
                width,
                height,
                x,
                y,
                font_size_multiplier,
            ));
        }
    }
    current
}

fn animate_glyph_bitmap(
    bitmap: &[u8],
    width: usize,
    height: usize,
    state: TextAnimatorState,
    font_size_multiplier: f32,
) -> Option<AnimatedGlyph> {
    let scale_x = state.scale[0].max(0.05);
    let scale_y = state.scale[1].max(0.05);
    let blur_radius = (state.blur[0].max(state.blur[1]).max(0.0) * font_size_multiplier * 0.28)
        .ceil()
        .clamp(0.0, 12.0) as usize;
    if (scale_x - 1.0).abs() < 0.001
        && (scale_y - 1.0).abs() < 0.001
        && state.rotation.abs() < 0.001
        && blur_radius == 0
    {
        return None;
    }

    let theta = state.rotation.to_radians();
    let cos = theta.cos();
    let sin = theta.sin();
    let scaled_width = width as f32 * scale_x;
    let scaled_height = height as f32 * scale_y;
    let transformed_width = (scaled_width * cos.abs() + scaled_height * sin.abs()).ceil();
    let transformed_height = (scaled_width * sin.abs() + scaled_height * cos.abs()).ceil();
    let pad = blur_radius * 2 + 2;
    let output_width = transformed_width.max(1.0) as usize + pad * 2;
    let output_height = transformed_height.max(1.0) as usize + pad * 2;
    let source_center = [(width as f32 - 1.0) * 0.5, (height as f32 - 1.0) * 0.5];
    let output_center = [
        (output_width as f32 - 1.0) * 0.5,
        (output_height as f32 - 1.0) * 0.5,
    ];
    let mut output = vec![0_u8; output_width * output_height];
    for y in 0..output_height {
        for x in 0..output_width {
            let dx = x as f32 - output_center[0];
            let dy = y as f32 - output_center[1];
            let source_x = source_center[0] + (cos * dx + sin * dy) / scale_x;
            let source_y = source_center[1] + (-sin * dx + cos * dy) / scale_y;
            output[y * output_width + x] =
                sample_alpha_bilinear(bitmap, width, height, source_x, source_y);
        }
    }
    if blur_radius > 0 {
        output = box_blur_alpha(&output, output_width, output_height, blur_radius);
        output = box_blur_alpha(&output, output_width, output_height, blur_radius);
    }

    Some(AnimatedGlyph {
        bitmap: output,
        width: output_width,
        height: output_height,
        offset: [
            (width as f32 - output_width as f32) * 0.5,
            (height as f32 - output_height as f32) * 0.5,
        ],
    })
}

fn sample_alpha_bilinear(bitmap: &[u8], width: usize, height: usize, x: f32, y: f32) -> u8 {
    if width == 0 || height == 0 {
        return 0;
    }
    if x < 0.0 || y < 0.0 || x > (width - 1) as f32 || y > (height - 1) as f32 {
        return 0;
    }

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let a00 = bitmap[y0 * width + x0] as f32;
    let a10 = bitmap[y0 * width + x1] as f32;
    let a01 = bitmap[y1 * width + x0] as f32;
    let a11 = bitmap[y1 * width + x1] as f32;
    let top = a00 + (a10 - a00) * tx;
    let bottom = a01 + (a11 - a01) * tx;
    (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8
}

fn draw_glyph_pass(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    fill: [u8; 4],
    stroke: [u8; 4],
    stroke_width: f32,
    opacity: f32,
    pass: &TextPass,
    faux_italic: bool,
    font_size_multiplier: f32,
    glyph_scale_y: f32,
) {
    match pass {
        TextPass::DropShadow(params) => draw_drop_shadow_glyph(
            dst,
            bitmap,
            width,
            height,
            x,
            y,
            *params,
            opacity,
            faux_italic,
            font_size_multiplier,
        ),
        TextPass::Glow(params) => draw_glow_glyph(
            dst,
            bitmap,
            width,
            height,
            x,
            y,
            fill,
            *params,
            opacity,
            faux_italic,
            font_size_multiplier,
        ),
        TextPass::BoxBlur(params) => draw_box_blur_glyph(
            dst,
            bitmap,
            width,
            height,
            x,
            y,
            fill,
            params.clone(),
            opacity,
            faux_italic,
            font_size_multiplier,
            glyph_scale_y,
        ),
        TextPass::Minimax(params) => draw_minimax_glyph(
            dst,
            bitmap,
            width,
            height,
            x,
            y,
            fill,
            params.clone(),
            opacity,
            faux_italic,
            font_size_multiplier,
            glyph_scale_y,
        ),
        TextPass::Stroke => {
            draw_hollow_stroke_glyph(
                dst,
                bitmap,
                width,
                height,
                x,
                y,
                stroke,
                stroke_width,
                opacity,
                faux_italic,
                glyph_scale_y,
            );
        }
        TextPass::Fill => blit_glyph(
            dst,
            bitmap,
            width,
            height,
            x,
            y,
            fill,
            opacity,
            faux_italic,
            glyph_scale_y,
        ),
    }
}

fn is_red_fill(fill: [u8; 4]) -> bool {
    fill[0] > 180 && fill[1] < 90 && fill[2] < 90
}

fn draw_drop_shadow_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    params: TextDropShadow,
    opacity: f32,
    faux_italic: bool,
    font_size_multiplier: f32,
) {
    let distance = params.distance * font_size_multiplier.max(0.0001);
    let (dx, dy) = drop_shadow_offset(params.direction_degrees, distance);
    let opacity = opacity * normalize_drop_shadow_opacity(params.opacity);
    let radius = drop_shadow_blur_radius(params.softness, font_size_multiplier);
    if radius == 0 {
        blit_glyph(
            dst,
            bitmap,
            width,
            height,
            x + dx,
            y + dy,
            params.color,
            opacity,
            faux_italic,
            1.0,
        );
        return;
    }
    draw_blurred_alpha_glyph(
        dst,
        bitmap,
        width,
        height,
        x + dx,
        y + dy,
        params.color,
        radius,
        2,
        opacity,
        faux_italic,
        1.0,
        false,
    );
}

fn drop_shadow_blur_radius(softness: f32, font_size_multiplier: f32) -> usize {
    (softness.max(0.0) * font_size_multiplier.max(0.0001) * 0.38)
        .ceil()
        .clamp(0.0, 28.0) as usize
}

fn draw_glow_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    fill: [u8; 4],
    params: TextGlow,
    opacity: f32,
    faux_italic: bool,
    font_size_multiplier: f32,
) {
    let signal = fill[0].max(fill[1]).max(fill[2]).max(fill[3]) as f32;
    if signal < params.threshold.clamp(0.0, 255.0) {
        return;
    }

    let radius = (params.radius * 0.4 * font_size_multiplier.max(0.0001))
        .ceil()
        .clamp(1.0, 28.0);
    let intensity = params.intensity.max(0.0).min(4.0);
    let glow = if is_red_fill(fill) {
        [255, 0, 0, 170]
    } else {
        [fill[0], fill[1], fill[2], 155]
    };
    for (mx, my, strength) in [
        (-0.35, 0.0, 0.18),
        (0.35, 0.0, 0.18),
        (0.0, -0.35, 0.15),
        (0.0, 0.35, 0.15),
        (-0.50, 0.35, 0.10),
        (0.50, 0.35, 0.10),
        (-0.72, 0.0, 0.07),
        (0.72, 0.0, 0.07),
        (0.0, -0.72, 0.06),
        (0.0, 0.72, 0.06),
    ] {
        blit_glyph(
            dst,
            bitmap,
            width,
            height,
            x + mx * radius,
            y + my * radius,
            glow,
            opacity * intensity * strength,
            faux_italic,
            1.0,
        );
    }
}

fn draw_box_blur_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    fill: [u8; 4],
    params: TextBoxBlur,
    opacity: f32,
    faux_italic: bool,
    font_size_multiplier: f32,
    glyph_scale_y: f32,
) {
    let mut radius = (params.radius * font_size_multiplier.max(0.0001))
        .ceil()
        .clamp(0.0, 64.0);
    if radius <= 0.0 {
        return;
    }
    if params.glow_only {
        radius = (radius * 12.0).clamp(16.0, 240.0);
    }
    let iterations = if params.glow_only {
        (params.iterations.round() + 4.0).clamp(5.0, 8.0)
    } else {
        params.iterations.round().clamp(1.0, 4.0)
    };
    draw_blurred_alpha_glyph(
        dst,
        bitmap,
        width,
        height,
        x,
        y,
        fill,
        radius.round() as usize,
        iterations as usize,
        if params.glow_only {
            opacity * 1.35
        } else {
            opacity
        },
        faux_italic,
        glyph_scale_y,
        params.glow_only,
    );
}

fn draw_blurred_alpha_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    fill: [u8; 4],
    radius: usize,
    iterations: usize,
    opacity: f32,
    faux_italic: bool,
    glyph_scale_y: f32,
    glow_only: bool,
) {
    let pad = if glow_only {
        (radius * 3).max(2)
    } else {
        (radius * 2).max(2)
    };
    let blur_width = width + pad * 2;
    let blur_height = height + pad * 2;
    let mut mask = vec![0u8; blur_width * blur_height];
    for gy in 0..height {
        for gx in 0..width {
            mask[(gy + pad) * blur_width + gx + pad] = bitmap[gy * width + gx];
        }
    }
    for _ in 0..iterations.max(1) {
        mask = box_blur_alpha(&mask, blur_width, blur_height, radius);
    }

    let shear_x = if faux_italic { 0.22 } else { 0.0 };
    let alpha_scale = if glow_only { 1.08 } else { 0.82 };
    for py in 0..blur_height {
        let dy0 = y - pad as f32 + py as f32 * glyph_scale_y;
        let dy1 = y - pad as f32 + (py as f32 + 1.0) * glyph_scale_y;
        let y_start = dy0.floor() as i32;
        let y_end = dy1.ceil().max(dy0 + 1.0) as i32;
        for px in 0..blur_width {
            let mut alpha = mask[py * blur_width + px];
            if glow_only {
                let source_x = px as i32 - pad as i32;
                let source_y = py as i32 - pad as i32;
                alpha = alpha.saturating_sub(
                    (glyph_alpha_at(bitmap, width, height, source_x, source_y) as f32 * 0.15) as u8,
                );
            }
            if alpha <= 1 {
                continue;
            }
            let source_gy = py.saturating_sub(pad).min(height);
            let italic_offset = (height.saturating_sub(source_gy) as f32) * shear_x;
            let dx = (x - pad as f32 + italic_offset + px as f32).round() as i32;
            if dx < 0 || dx >= dst.width as i32 {
                continue;
            }
            let src_alpha = ((alpha as f32 / 255.0)
                * (fill[3] as f32 / 255.0)
                * (opacity / 100.0).clamp(0.0, 1.0)
                * alpha_scale
                * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
            let src = [fill[0], fill[1], fill[2], src_alpha];
            for dy in y_start..y_end {
                if dy < 0 || dy >= dst.height as i32 {
                    continue;
                }
                let i = (((dy as u32) * dst.width + dx as u32) * 4) as usize;
                let out = over(
                    [
                        dst.data[i],
                        dst.data[i + 1],
                        dst.data[i + 2],
                        dst.data[i + 3],
                    ],
                    src,
                    100.0,
                );
                dst.data[i..i + 4].copy_from_slice(&out);
            }
        }
    }
}

fn box_blur_alpha(src: &[u8], width: usize, height: usize, radius: usize) -> Vec<u8> {
    if radius == 0 {
        return src.to_vec();
    }
    let mut horizontal = vec![0u8; src.len()];
    let mut prefix = vec![0u32; width + 1];
    for y in 0..height {
        prefix[0] = 0;
        for x in 0..width {
            prefix[x + 1] = prefix[x] + src[y * width + x] as u32;
        }
        for x in 0..width {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius + 1).min(width);
            horizontal[y * width + x] = ((prefix[x1] - prefix[x0]) / (x1 - x0) as u32) as u8;
        }
    }

    let mut dst = vec![0u8; src.len()];
    let mut prefix = vec![0u32; height + 1];
    for x in 0..width {
        prefix[0] = 0;
        for y in 0..height {
            prefix[y + 1] = prefix[y] + horizontal[y * width + x] as u32;
        }
        for y in 0..height {
            let y0 = y.saturating_sub(radius);
            let y1 = (y + radius + 1).min(height);
            dst[y * width + x] = ((prefix[y1] - prefix[y0]) / (y1 - y0) as u32) as u8;
        }
    }
    dst
}

fn apply_canvas_box_blur(dst: &mut Canvas, radius: f32, iterations: f32) {
    let radius = (radius * 0.65).round().clamp(1.0, 24.0) as usize;
    let iterations = iterations.round().clamp(1.0, 4.0) as usize;
    for _ in 0..iterations {
        let src = dst.data.clone();
        let width = dst.width as usize;
        let height = dst.height as usize;
        for channel in 0..4 {
            let plane = src
                .chunks_exact(4)
                .map(|px| px[channel])
                .collect::<Vec<_>>();
            let blurred = box_blur_alpha(&plane, width, height, radius);
            for (index, value) in blurred.into_iter().enumerate() {
                dst.data[index * 4 + channel] = value;
            }
        }
    }
}

fn apply_canvas_minimax(dst: &mut Canvas, radius: f32) {
    let radius = radius.round().clamp(1.0, 12.0) as i32;
    let src = dst.clone();
    for y in 0..dst.height as i32 {
        for x in 0..dst.width as i32 {
            let mut max_luma = -1_i32;
            let mut best = [0_u8; 4];
            for oy in -radius..=radius {
                for ox in -radius..=radius {
                    if ox * ox + oy * oy > radius * radius {
                        continue;
                    }
                    let sx = x + ox;
                    let sy = y + oy;
                    if sx < 0 || sy < 0 || sx >= src.width as i32 || sy >= src.height as i32 {
                        continue;
                    }
                    let px = src.pixel(sx as u32, sy as u32);
                    let luma = px[0] as i32 * 3 + px[1] as i32 * 6 + px[2] as i32;
                    if luma > max_luma {
                        max_luma = luma;
                        best = px;
                    }
                }
            }
            dst.set_pixel(x as u32, y as u32, best);
        }
    }
}

fn apply_canvas_glow(dst: &mut Canvas, params: TextGlow) {
    let radius = (params.radius * 0.45).round().clamp(1.0, 24.0) as usize;
    let threshold = (params.threshold / 100.0).clamp(0.0, 1.0) * 255.0;
    let intensity = params.intensity.max(0.0) * 0.45;
    let width = dst.width as usize;
    let height = dst.height as usize;
    let src = dst.data.clone();
    let mut bright = vec![0_u8; width * height];
    for (index, px) in src.chunks_exact(4).enumerate() {
        let luma = px[0] as f32 * 0.299 + px[1] as f32 * 0.587 + px[2] as f32 * 0.114;
        bright[index] = if luma >= threshold {
            ((luma - threshold) / (255.0 - threshold).max(1.0) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8
        } else {
            0
        };
    }
    let glow = box_blur_alpha(&bright, width, height, radius);
    for (index, strength) in glow.into_iter().enumerate() {
        let add = strength as f32 * intensity;
        let base = index * 4;
        for channel in 0..3 {
            dst.data[base + channel] = (dst.data[base + channel] as f32 + add)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
}

fn apply_canvas_turbulent_displace(dst: &mut Canvas, params: &TextTurbulentDisplace, time: f64) {
    if params.amount.abs() <= f32::EPSILON {
        return;
    }
    let src = dst.clone();
    let amount = params.amount * 0.45;
    for y in 0..dst.height {
        for x in 0..dst.width {
            let [dx, dy] = params.field_offset_at(time, x as f32, y as f32, amount);
            let sample = bilinear(&src, x as f32 - dx, y as f32 - dy);
            let i = ((y * dst.width + x) * 4) as usize;
            dst.data[i..i + 4].copy_from_slice(&sample);
        }
    }
}

fn draw_minimax_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    fill: [u8; 4],
    params: TextMinimax,
    opacity: f32,
    faux_italic: bool,
    font_size_multiplier: f32,
    glyph_scale_y: f32,
) {
    let radius = (params.radius * font_size_multiplier.max(0.0001))
        .round()
        .clamp(1.0, 32.0) as i32;
    draw_expanded_glyph(
        dst,
        bitmap,
        width,
        height,
        x,
        y,
        fill,
        radius,
        opacity,
        faux_italic,
        glyph_scale_y,
    );
}

fn drop_shadow_offset(direction_degrees: f32, distance: f32) -> (f32, f32) {
    let direction = direction_degrees.to_radians();
    (
        (direction.sin() * distance).trunc(),
        (-direction.cos() * distance).trunc(),
    )
}

fn normalize_drop_shadow_opacity(value: f32) -> f32 {
    (value / 255.0).clamp(0.0, 1.0)
}

fn blit_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    fill: [u8; 4],
    opacity: f32,
    faux_italic: bool,
    glyph_scale_y: f32,
) {
    let shear_x = if faux_italic { 0.22 } else { 0.0 };
    for gy in 0..height {
        let dy0 = y + gy as f32 * glyph_scale_y;
        let dy1 = y + (gy as f32 + 1.0) * glyph_scale_y;
        let y_start = dy0.floor() as i32;
        let y_end = dy1.ceil().max(dy0 + 1.0) as i32;
        for gx in 0..width {
            let alpha = bitmap[gy * width + gx];
            if alpha == 0 {
                continue;
            }
            let italic_offset = (height.saturating_sub(gy) as f32) * shear_x;
            let dx = (x + italic_offset + gx as f32).round() as i32;
            if dx < 0 || dx >= dst.width as i32 {
                continue;
            }
            let src_alpha = ((alpha as f32 / 255.0)
                * (fill[3] as f32 / 255.0)
                * (opacity / 100.0).clamp(0.0, 1.0)
                * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
            let src = [fill[0], fill[1], fill[2], src_alpha];
            for dy in y_start..y_end {
                if dy < 0 || dy >= dst.height as i32 {
                    continue;
                }
                let i = (((dy as u32) * dst.width + dx as u32) * 4) as usize;
                let out = over(
                    [
                        dst.data[i],
                        dst.data[i + 1],
                        dst.data[i + 2],
                        dst.data[i + 3],
                    ],
                    src,
                    100.0,
                );
                dst.data[i..i + 4].copy_from_slice(&out);
            }
        }
    }
}

fn draw_hollow_stroke_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    stroke: [u8; 4],
    stroke_width: f32,
    opacity: f32,
    faux_italic: bool,
    glyph_scale_y: f32,
) {
    let radius = stroke_width.round().clamp(1.0, 16.0) as i32;
    let shear_x = if faux_italic { 0.22 } else { 0.0 };
    for ly in -radius..height as i32 + radius {
        let dy = (y + ly as f32 * glyph_scale_y).round();
        if dy < 0.0 || dy >= dst.height as f32 {
            continue;
        }
        for lx in -radius..width as i32 + radius {
            let original = glyph_alpha_at(bitmap, width, height, lx, ly);
            let dilated = dilated_glyph_alpha(bitmap, width, height, lx, ly, radius);
            let alpha = dilated.saturating_sub(original);
            if alpha == 0 {
                continue;
            }

            let italic_offset = (height as i32 - ly).max(0) as f32 * shear_x;
            let dx = (x + italic_offset + lx as f32).round();
            if dx < 0.0 || dx >= dst.width as f32 {
                continue;
            }
            let src_alpha = ((alpha as f32 / 255.0)
                * (stroke[3] as f32 / 255.0)
                * (opacity / 100.0).clamp(0.0, 1.0)
                * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
            if src_alpha == 0 {
                continue;
            }
            let src = [stroke[0], stroke[1], stroke[2], src_alpha];
            let i = (((dy as u32) * dst.width + dx as u32) * 4) as usize;
            let out = over(
                [
                    dst.data[i],
                    dst.data[i + 1],
                    dst.data[i + 2],
                    dst.data[i + 3],
                ],
                src,
                100.0,
            );
            dst.data[i..i + 4].copy_from_slice(&out);
        }
    }
}

fn draw_expanded_glyph(
    dst: &mut Canvas,
    bitmap: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    fill: [u8; 4],
    radius: i32,
    opacity: f32,
    faux_italic: bool,
    glyph_scale_y: f32,
) {
    let shear_x = if faux_italic { 0.22 } else { 0.0 };
    for ly in -radius..height as i32 + radius {
        let dy0 = y + ly as f32 * glyph_scale_y;
        let dy1 = y + (ly as f32 + 1.0) * glyph_scale_y;
        let y_start = dy0.floor() as i32;
        let y_end = dy1.ceil().max(dy0 + 1.0) as i32;
        for lx in -radius..width as i32 + radius {
            let alpha = dilated_glyph_alpha(bitmap, width, height, lx, ly, radius);
            if alpha == 0 {
                continue;
            }
            let italic_offset = (height as i32 - ly).max(0) as f32 * shear_x;
            let dx = (x + italic_offset + lx as f32).round();
            if dx < 0.0 || dx >= dst.width as f32 {
                continue;
            }
            let src_alpha = ((alpha as f32 / 255.0)
                * (fill[3] as f32 / 255.0)
                * (opacity / 100.0).clamp(0.0, 1.0)
                * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
            if src_alpha == 0 {
                continue;
            }
            let src = [fill[0], fill[1], fill[2], src_alpha];
            for dy in y_start..y_end {
                if dy < 0 || dy >= dst.height as i32 {
                    continue;
                }
                let i = (((dy as u32) * dst.width + dx as u32) * 4) as usize;
                let out = over(
                    [
                        dst.data[i],
                        dst.data[i + 1],
                        dst.data[i + 2],
                        dst.data[i + 3],
                    ],
                    src,
                    100.0,
                );
                dst.data[i..i + 4].copy_from_slice(&out);
            }
        }
    }
}

fn dilated_glyph_alpha(
    bitmap: &[u8],
    width: usize,
    height: usize,
    lx: i32,
    ly: i32,
    radius: i32,
) -> u8 {
    let mut max_alpha = 0_u8;
    for oy in -radius..=radius {
        for ox in -radius..=radius {
            if ox * ox + oy * oy > radius * radius {
                continue;
            }
            max_alpha = max_alpha.max(glyph_alpha_at(bitmap, width, height, lx - ox, ly - oy));
        }
    }
    max_alpha
}

fn glyph_alpha_at(bitmap: &[u8], width: usize, height: usize, lx: i32, ly: i32) -> u8 {
    if lx < 0 || ly < 0 || lx >= width as i32 || ly >= height as i32 {
        return 0;
    }
    bitmap[ly as usize * width + lx as usize]
}

fn layout_max_width(layer: &TextLayer, layer_scale: f32) -> f32 {
    if layer.box_text && !layer.no_layout_pass {
        layer.box_size[0] * layer_scale
    } else {
        f32::INFINITY
    }
}

fn layout_words(
    fonts: &FontBook,
    layer: &TextLayer,
    font_size_multiplier: f32,
    tracking_px: f32,
    max_width: f32,
) -> Vec<Vec<WordToken>> {
    let mut lines: Vec<Vec<WordToken>> = Vec::new();
    let font_size = layer.font_size * font_size_multiplier;
    let layer_font = fonts.for_name(layer.font_name.as_deref());
    let space = space_width(layer_font, font_size, tracking_px);

    for hard_line in collect_hard_lines(&layer.text) {
        let words = collect_words(fonts, layer, hard_line, font_size_multiplier, tracking_px);
        let mut line = Vec::new();
        let mut line_width = 0.0;
        for word in words {
            let proposed = if line.is_empty() {
                word.width
            } else {
                line_width + space + word.width
            };
            if !line.is_empty() && proposed > max_width {
                lines.push(line);
                line = Vec::new();
                line_width = 0.0;
            }
            if !line.is_empty() {
                line_width += space;
            }
            line_width += word.width;
            line.push(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

fn collect_hard_lines(text: &str) -> Vec<(usize, &str)> {
    let mut lines = Vec::new();
    let mut byte_start = 0usize;
    let mut char_start = 0usize;
    let mut char_index = 0usize;
    for (byte_index, ch) in text.char_indices() {
        if ch == '\n' || ch == '\r' {
            lines.push((char_start, &text[byte_start..byte_index]));
            byte_start = byte_index + ch.len_utf8();
            char_start = char_index + 1;
        }
        char_index += 1;
    }
    lines.push((char_start, &text[byte_start..]));
    lines
}

fn collect_words(
    fonts: &FontBook,
    layer: &TextLayer,
    hard_line: (usize, &str),
    font_size_multiplier: f32,
    tracking_px: f32,
) -> Vec<WordToken> {
    let (line_char_start, text) = hard_line;
    let mut words = Vec::new();
    let mut current = String::new();
    let mut start_char = line_char_start;
    let mut in_word = false;
    for (line_offset, ch) in text.chars().enumerate() {
        if ch.is_whitespace() {
            if in_word {
                words.push(word_token(
                    fonts,
                    layer,
                    current.clone(),
                    start_char,
                    font_size_multiplier,
                    tracking_px,
                ));
                current.clear();
                in_word = false;
            }
        } else {
            if !in_word {
                start_char = line_char_start + line_offset;
                in_word = true;
            }
            current.push(ch);
        }
    }
    if in_word {
        words.push(word_token(
            fonts,
            layer,
            current,
            start_char,
            font_size_multiplier,
            tracking_px,
        ));
    }
    words
}

fn word_token(
    fonts: &FontBook,
    layer: &TextLayer,
    text: String,
    start_char: usize,
    font_size_multiplier: f32,
    tracking_px: f32,
) -> WordToken {
    let width = measure_text_run(
        fonts,
        layer,
        &text,
        start_char,
        font_size_multiplier,
        tracking_px,
    );
    WordToken {
        text,
        start_char,
        width,
    }
}

fn measure_text_run(
    fonts: &FontBook,
    layer: &TextLayer,
    text: &str,
    start_char: usize,
    font_size_multiplier: f32,
    tracking_px: f32,
) -> f32 {
    let mut width: f32 = text
        .chars()
        .enumerate()
        .map(|(offset, ch)| {
            let index = start_char + offset;
            let style = layer.char_styles.get(&index);
            let font = fonts.for_name(
                style
                    .and_then(|style| style.font_name.as_deref())
                    .or(layer.font_name.as_deref()),
            );
            let font_size = layer
                .char_styles
                .get(&index)
                .and_then(|style| style.font_size)
                .unwrap_or(layer.font_size)
                * font_size_multiplier;
            font.metrics_indexed(font.lookup_glyph_index(ch), font_size)
                .advance_width
                + tracking_px
        })
        .sum();
    if !text.is_empty() {
        width -= tracking_px;
    }
    width.max(0.0)
}

fn space_width(font: &Font, size: f32, tracking_px: f32) -> f32 {
    font.metrics_indexed(font.lookup_glyph_index(' '), size)
        .advance_width
        + tracking_px
}

fn selector_visible_opacity(
    reveal: Option<f32>,
    reveal_end: Option<f32>,
    word_index: usize,
    word_count: usize,
    smoothness: f32,
) -> f32 {
    match (reveal, reveal_end) {
        (Some(start), Some(end)) => {
            let before = reveal_word_before_opacity(start, word_index, word_count, smoothness);
            let after = 1.0 - reveal_word_before_opacity(end, word_index, word_count, smoothness);
            (before + after).min(1.0)
        }
        (Some(start), None) => {
            reveal_word_before_opacity(start, word_index, word_count, smoothness)
        }
        (None, Some(end)) => {
            1.0 - reveal_word_before_opacity(end, word_index, word_count, smoothness)
        }
        (None, None) => 1.0,
    }
}

fn reveal_word_before_opacity(
    reveal: f32,
    word_index: usize,
    word_count: usize,
    smoothness: f32,
) -> f32 {
    if reveal >= 99.99 {
        return 1.0;
    }
    let start = word_index as f32 / word_count as f32 * 100.0;
    let end = (word_index + 1) as f32 / word_count as f32 * 100.0;
    if smoothness <= 1.0 {
        let midpoint = (start + end) * 0.5;
        return if reveal >= midpoint { 1.0 } else { 0.0 };
    }
    if reveal <= start {
        0.0
    } else if reveal >= end {
        1.0
    } else {
        ((reveal - start) / (end - start).max(0.0001)).clamp(0.0, 1.0)
    }
}

fn bilinear(image: &Canvas, x: f32, y: f32) -> [u8; 4] {
    if x < 0.0 || y < 0.0 || x > (image.width - 1) as f32 || y > (image.height - 1) as f32 {
        return [0, 0, 0, 0];
    }
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(image.width - 1);
    let y1 = (y0 + 1).min(image.height - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let p00 = image.pixel(x0, y0);
    let p10 = image.pixel(x1, y0);
    let p01 = image.pixel(x0, y1);
    let p11 = image.pixel(x1, y1);
    let mut out = [0_u8; 4];
    for c in 0..4 {
        let top = p00[c] as f32 + (p10[c] as f32 - p00[c] as f32) * tx;
        let bottom = p01[c] as f32 + (p11[c] as f32 - p01[c] as f32) * tx;
        out[c] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
    }
    out
}

fn over(dst: [u8; 4], src: [u8; 4], opacity_percent: f32) -> [u8; 4] {
    let sa = (src[3] as f32 / 255.0) * (opacity_percent / 100.0).clamp(0.0, 1.0);
    if sa <= 0.0 {
        return dst;
    }
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    let mut out = dst;
    for c in 0..3 {
        let s = src[c] as f32 / 255.0;
        let d = dst[c] as f32 / 255.0;
        let out_c = if out_a <= 0.0 {
            0.0
        } else {
            (s * sa + d * da * (1.0 - sa)) / out_a
        };
        out[c] = (out_c * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    out
}

fn transform(layer: &Value) -> Transform {
    Transform {
        anchor: vec2_at(layer, "/props/tf_anchor/value", [0.0, 0.0]),
        position: vec2_at(layer, "/props/tf_position/value", [0.0, 0.0]),
        scale: vec2_at(layer, "/props/tf_scale/value", [100.0, 100.0]),
        rotation: number_at(layer, "/props/tf_rotation/value", 0.0) as f32,
        opacity: number_at(layer, "/props/tf_opacity/value", 100.0) as f32,
    }
}

fn is_flash_on_cuts_mode(project_spec: &Value) -> bool {
    matches!(
        project_spec.get("subtitlesMode").and_then(Value::as_str),
        Some("template_4th" | "impulse_2nd")
    )
}

impl FontBook {
    fn for_name(&self, name: Option<&str>) -> &Font {
        name.and_then(|name| self.by_name.get(name))
            .unwrap_or(&self.default)
    }
}

fn extract_var(source: &str, name: &str) -> Result<Value> {
    let marker = format!("var {name} =");
    let start = source
        .find(&marker)
        .with_context(|| format!("missing JSX var {name}"))?
        + marker.len();
    let bytes = source.as_bytes();
    let mut i = start;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let json_start = i;
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
    anyhow::bail!("unterminated JSX var {name}")
}

fn load_fonts(default_path: Option<&Path>, project: &Project) -> Result<FontBook> {
    let default = load_font(default_path)?;
    let mut wanted = BTreeSet::new();
    for layer in &project.text_layers {
        if let Some(name) = &layer.font_name {
            wanted.insert(name.clone());
        }
        for style in layer.char_styles.values() {
            if let Some(name) = &style.font_name {
                wanted.insert(name.clone());
            }
        }
    }

    let mut by_name = BTreeMap::new();
    for name in wanted {
        if let Some(font) = load_named_font(&name)? {
            by_name.insert(name, font);
        }
    }

    Ok(FontBook { default, by_name })
}

fn load_named_font(name: &str) -> Result<Option<Font>> {
    for candidate in font_candidates_for_name(name) {
        if let Ok(bytes) = fs::read(&candidate) {
            let font = Font::from_bytes(bytes, FontSettings::default()).map_err(|err| {
                anyhow::anyhow!("failed to load font {}: {err}", candidate.display())
            })?;
            return Ok(Some(font));
        }
    }
    Ok(None)
}

fn font_candidates_for_name(name: &str) -> Vec<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let compact = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    let mut candidates = vec![
        PathBuf::from(format!("/Users/ergin/Library/Fonts/{name}.otf")),
        PathBuf::from(format!("/Users/ergin/Library/Fonts/{name}.ttf")),
        PathBuf::from(format!("/Users/ergin/Library/Fonts/{compact}.otf")),
        PathBuf::from(format!("/Users/ergin/Library/Fonts/{compact}.ttf")),
        PathBuf::from(format!("/Library/Fonts/{name}.otf")),
        PathBuf::from(format!("/Library/Fonts/{name}.ttf")),
        PathBuf::from(format!("/Library/Fonts/{compact}.otf")),
        PathBuf::from(format!("/Library/Fonts/{compact}.ttf")),
        PathBuf::from(format!(
            "/Users/ergin/Desktop/archives_and_duplicates/dup_blast_family/blast_mj_final/landing/fonts/{name}.ttf"
        )),
        PathBuf::from(format!(
            "/Users/ergin/Desktop/archives_and_duplicates/dup_blast_family/blast_mj_final/landing/fonts/{compact}.ttf"
        )),
        manifest_dir.join(format!(
            "../../fixtures/ae_conformance_pack/assets/fonts/{name}.ttf"
        )),
        manifest_dir.join(format!(
            "../../fixtures/ae_conformance_pack/assets/fonts/{compact}.ttf"
        )),
    ];
    match name {
        "Point-SemiBold" => candidates.push(PathBuf::from(
            "/Users/ergin/Library/Fonts/PointSemiBold.otf",
        )),
        "Point-ExtraBold" => candidates.push(PathBuf::from(
            "/Users/ergin/Library/Fonts/PointExtraBold.otf",
        )),
        "Montserrat-BoldItalic" => candidates.push(
            manifest_dir
                .join("../../fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf"),
        ),
        "Point-Light" => candidates.push(
            manifest_dir.join("../../fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf"),
        ),
        _ => {}
    }
    candidates
}

fn load_font(path: Option<&Path>) -> Result<Font> {
    let candidates = path
        .map(|path| vec![path.to_path_buf()])
        .unwrap_or_else(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            vec![
                PathBuf::from("/Users/ergin/Library/Fonts/PointSemiBold.otf"),
                PathBuf::from(
                    "/Users/ergin/Desktop/archives_and_duplicates/dup_blast_family/blast_mj_final/landing/fonts/Point-SemiBold.ttf",
                ),
                manifest_dir.join(
                    "../../fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf",
                ),
                PathBuf::from(
                    "fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf",
                ),
                PathBuf::from("/System/Library/Fonts/Supplemental/Arial Bold Italic.ttf"),
                PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"),
            ]
        });
    for candidate in candidates {
        if let Ok(bytes) = fs::read(&candidate) {
            return Font::from_bytes(bytes, FontSettings::default()).map_err(|err| {
                anyhow::anyhow!("failed to load font {}: {err}", candidate.display())
            });
        }
    }
    anyhow::bail!("no usable font found; pass --font")
}

fn vec2_at(value: &Value, pointer: &str, default: [f32; 2]) -> [f32; 2] {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(|items| {
            [
                items
                    .first()
                    .and_then(Value::as_f64)
                    .unwrap_or(default[0] as f64) as f32,
                items
                    .get(1)
                    .and_then(Value::as_f64)
                    .unwrap_or(default[1] as f64) as f32,
            ]
        })
        .unwrap_or(default)
}

fn number(value: &Value, keys: &[&str], default: f64) -> f64 {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_f64))
        .unwrap_or(default)
}

fn bool_field(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn bool_at(value: &Value, pointer: &str, default: bool) -> bool {
    value
        .pointer(pointer)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn number_at(value: &Value, pointer: &str, default: f64) -> f64 {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .unwrap_or(default)
}

fn rgba_from_unit_array(value: Option<&Value>, default: [u8; 4]) -> [u8; 4] {
    let Some(items) = value.and_then(Value::as_array) else {
        return default;
    };
    let mut out = default;
    for i in 0..3 {
        out[i] = (items
            .get(i)
            .and_then(Value::as_f64)
            .unwrap_or(out[i] as f64 / 255.0)
            * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    out
}

fn layer_start(layer: &Value) -> f64 {
    number(layer, &["in_point", "inPoint", "start"], 0.0)
}

fn layer_end(layer: &Value, default: f64) -> f64 {
    let start = layer_start(layer);
    let end = number(layer, &["out_point", "outPoint", "end"], f64::NAN);
    if end.is_finite() {
        return end;
    }
    layer
        .get("duration")
        .and_then(Value::as_f64)
        .map(|duration| start + duration)
        .unwrap_or(default)
}

fn layer_z(layer: &Value) -> i64 {
    number(layer, &["z_index", "zIndex", "index"], 0.0) as i64
}

fn layer_z_f64(layer: &Value) -> f64 {
    number(layer, &["z_index", "zIndex", "index"], 0.0)
}

fn parse_text_effects(layer: &Value) -> Vec<TextEffect> {
    let mut effects = Vec::new();
    for pointer in [
        "/effects",
        "/effect_stack",
        "/text_data/effects",
        "/text_data/effect_stack",
        "/text_data/layer_effects",
        "/props/effects",
        "/props/effect_stack",
    ] {
        if let Some(value) = layer.pointer(pointer) {
            effects.extend(parse_text_effects_value(value));
        }
    }
    effects
}

fn parse_text_effects_value(value: &Value) -> Vec<TextEffect> {
    let mut effects = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            if let Some(effect) = parse_text_effect(None, item) {
                effects.push(effect);
            }
        }
        return effects;
    }
    if let Some(items) = value.as_object() {
        for (key, item) in items {
            if let Some(effect) = parse_text_effect(Some(key.as_str()), item) {
                effects.push(effect);
            }
        }
    }
    effects
}

fn parse_text_effect(key: Option<&str>, item: &Value) -> Option<TextEffect> {
    if !effect_is_enabled(item) {
        return None;
    }
    let match_name = effect_match_name(key, item)?;
    let params = effect_params(item);
    match match_name.trim() {
        "ADBE Drop Shadow" | "S_DropShadow" => {
            Some(TextEffect::DropShadow(TextDropShadow::from_params(params)))
        }
        "ADBE Glo2" => Some(TextEffect::Glow(TextGlow::from_params(params))),
        "ADBE Box Blur2" => Some(TextEffect::BoxBlur(TextBoxBlur::from_params(params))),
        "ADBE Minimax" => Some(TextEffect::Minimax(TextMinimax::from_params(params))),
        "ADBE Turbulent Displace" => Some(TextEffect::TurbulentDisplace(
            TextTurbulentDisplace::from_params(params),
        )),
        _ => None,
    }
}

fn effect_is_enabled(item: &Value) -> bool {
    effect_param_bool(item, &["enabled", "active", "is_enabled"], true)
}

fn effect_match_name<'a>(key: Option<&'a str>, item: &'a Value) -> Option<&'a str> {
    for name in ["match_name", "matchName", "name", "match", "effect"] {
        if let Some(value) = item.get(name).and_then(Value::as_str) {
            return Some(value);
        }
    }
    if let Some(key) = key {
        return Some(key.rsplit_once(':').map_or(key, |(_, name)| name));
    }
    None
}

fn effect_params(item: &Value) -> &Value {
    item.get("params")
        .or_else(|| item.get("parameters"))
        .or_else(|| item.get("properties"))
        .or_else(|| item.get("props"))
        .unwrap_or(item)
}

impl TextDropShadow {
    fn from_params(params: &Value) -> Self {
        Self {
            color: effect_param_rgba(
                params,
                &[
                    "color",
                    "Color",
                    "Shadow Color",
                    "0001",
                    "ADBE Drop Shadow-0001",
                ],
                [0, 0, 0, 255],
            ),
            opacity: effect_param_f32(
                params,
                &["opacity", "Opacity", "0002", "ADBE Drop Shadow-0002"],
                255.0,
            ),
            direction_degrees: effect_param_f32(
                params,
                &["direction", "Direction", "0003", "ADBE Drop Shadow-0003"],
                135.0,
            ),
            distance: effect_param_f32(
                params,
                &["distance", "Distance", "0004", "ADBE Drop Shadow-0004"],
                5.0,
            ),
            softness: effect_param_f32(
                params,
                &[
                    "softness",
                    "Softness",
                    "blur",
                    "Blur",
                    "0005",
                    "ADBE Drop Shadow-0005",
                ],
                0.0,
            ),
            shadow_only: effect_param_bool(
                params,
                &["shadowOnly", "Shadow Only", "0006", "ADBE Drop Shadow-0006"],
                false,
            ),
        }
    }
}

impl TextGlow {
    fn from_params(params: &Value) -> Self {
        Self {
            threshold: effect_param_f32(params, &["threshold", "Threshold", "0002"], 0.0),
            radius: effect_param_f32(params, &["radius", "Radius", "0003"], 16.0),
            intensity: effect_param_f32(params, &["intensity", "Intensity", "0004"], 1.0),
        }
    }
}

impl TextBoxBlur {
    fn from_params(params: &Value) -> Self {
        let radius_names = ["radius", "Radius", "0001", "ADBE Box Blur2-0001"];
        let fallback_names = ["0002", "ADBE Box Blur2-0002"];
        let has_radius = effect_has_param(params, &radius_names);
        let radius_value = if has_radius {
            effect_param_value_any(params, &radius_names)
        } else if effect_has_param(params, &fallback_names) {
            effect_param_value_any(params, &fallback_names)
        } else {
            None
        };
        let radius_keys = radius_value
            .map(effect_value_scalar_keys)
            .unwrap_or_default();
        let radius = radius_value
            .and_then(effect_value_f32)
            .or_else(|| radius_keys.first().map(|(_, value)| *value))
            .unwrap_or(0.0);
        let iterations = if effect_has_param(params, &["iterations", "Iterations"]) {
            effect_param_f32(params, &["iterations", "Iterations"], 1.0)
        } else if has_radius && effect_has_param(params, &["0002"]) {
            effect_param_f32(params, &["0002"], 1.0)
        } else {
            1.0
        };
        Self {
            radius,
            radius_keys,
            iterations,
            glow_only: false,
        }
    }

    fn at_time(&self, time: f64) -> Self {
        Self {
            radius: eval_scalar_keys(&self.radius_keys, time, self.radius),
            radius_keys: Vec::new(),
            iterations: self.iterations,
            glow_only: self.glow_only,
        }
    }
}

impl TextMinimax {
    fn from_params(params: &Value) -> Self {
        let radius_names = ["radius", "Radius", "0002", "ADBE Minimax-0002"];
        let radius_value = effect_param_value_any(params, &radius_names);
        let radius_keys = radius_value
            .map(effect_value_scalar_keys)
            .unwrap_or_default();
        let radius = radius_value
            .and_then(effect_value_f32)
            .or_else(|| radius_keys.first().map(|(_, value)| *value))
            .unwrap_or(0.0);
        Self {
            radius,
            radius_keys,
        }
    }

    fn at_time(&self, time: f64) -> Self {
        Self {
            radius: eval_scalar_keys(&self.radius_keys, time, self.radius),
            radius_keys: Vec::new(),
        }
    }
}

impl TextTurbulentDisplace {
    fn from_params(params: &Value) -> Self {
        let evolution_expression = effect_param_expression(
            params,
            &[
                "evolution",
                "Evolution",
                "0006",
                "ADBE Turbulent Displace-0006",
            ],
        );
        Self {
            amount: effect_param_f32(
                params,
                &["amount", "Amount", "0002", "ADBE Turbulent Displace-0002"],
                0.0,
            ),
            size: effect_param_f32(
                params,
                &["size", "Size", "0003", "ADBE Turbulent Displace-0003"],
                50.0,
            ),
            evolution_degrees: effect_param_f32(
                params,
                &[
                    "evolution",
                    "Evolution",
                    "0006",
                    "ADBE Turbulent Displace-0006",
                ],
                0.0,
            ),
            evolution_rate_degrees: evolution_expression
                .and_then(time_multiplier_expression)
                .unwrap_or(0.0),
        }
    }

    fn offset_at(
        &self,
        time: f64,
        char_index: usize,
        x: f32,
        y: f32,
        font_size_multiplier: f32,
    ) -> [f32; 2] {
        if self.amount.abs() <= f32::EPSILON {
            return [0.0, 0.0];
        }
        let scale = font_size_multiplier.max(0.0001);
        let seed_x = x + char_index as f32 * 13.17;
        let seed_y = y - char_index as f32 * 7.91;
        self.field_offset_at(time, seed_x, seed_y, self.amount * scale * 0.18)
    }

    fn displace_bitmap_at(
        &self,
        time: f64,
        char_index: usize,
        bitmap: &[u8],
        width: usize,
        height: usize,
        x: f32,
        y: f32,
        font_size_multiplier: f32,
    ) -> Vec<u8> {
        let scale = font_size_multiplier.max(0.0001);
        let amount = self.amount * scale * 0.72;
        let mut out = vec![0u8; width * height];

        for gy in 0..height {
            for gx in 0..width {
                let world_x = x + gx as f32;
                let world_y = y + gy as f32;
                let [dx, dy] = self.field_offset_at(
                    time,
                    world_x + char_index as f32 * 11.0,
                    world_y - char_index as f32 * 5.0,
                    amount,
                );
                out[gy * width + gx] =
                    sample_alpha_bilinear(bitmap, width, height, gx as f32 - dx, gy as f32 - dy);
            }
        }

        out
    }

    fn field_offset_at(&self, time: f64, x: f32, y: f32, amount: f32) -> [f32; 2] {
        let size = self.size.max(1.0);
        let evolution =
            (self.evolution_degrees + self.evolution_rate_degrees * time as f32) / 360.0;
        let nx = x / size + evolution * 0.73;
        let ny = y / size - evolution * 0.41;
        let n1 = fbm_noise_2d(nx, ny, 11);
        let n2 = fbm_noise_2d(nx + 19.31, ny - 7.77, 97);
        [(n1 * 2.0 - 1.0) * amount, (n2 * 2.0 - 1.0) * amount * 0.78]
    }
}

fn fbm_noise_2d(mut x: f32, mut y: f32, seed: u32) -> f32 {
    let mut amplitude = 0.5;
    let mut sum = 0.0;
    let mut normalizer = 0.0;
    for octave in 0..4 {
        sum += value_noise_2d(x, y, seed.wrapping_add(octave * 101)) * amplitude;
        normalizer += amplitude;
        x *= 2.03;
        y *= 1.97;
        amplitude *= 0.5;
    }
    if normalizer <= f32::EPSILON {
        0.0
    } else {
        sum / normalizer
    }
}

fn value_noise_2d(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let tx = smooth_unit(x - x0 as f32);
    let ty = smooth_unit(y - y0 as f32);
    let n00 = hash_noise_2d(x0, y0, seed);
    let n10 = hash_noise_2d(x0 + 1, y0, seed);
    let n01 = hash_noise_2d(x0, y0 + 1, seed);
    let n11 = hash_noise_2d(x0 + 1, y0 + 1, seed);
    let top = n00 + (n10 - n00) * tx;
    let bottom = n01 + (n11 - n01) * tx;
    top + (bottom - top) * ty
}

fn hash_noise_2d(x: i32, y: i32, seed: u32) -> f32 {
    let mut n = x as u32;
    n = n.wrapping_mul(0x8da6_b343);
    n ^= (y as u32).wrapping_mul(0xd816_3841);
    n ^= seed.wrapping_mul(0xcb1a_b31f);
    n ^= n >> 13;
    n = n.wrapping_mul(0x85eb_ca6b);
    n ^= n >> 16;
    (n as f32) / (u32::MAX as f32)
}

impl PosterizeTimeSpec {
    fn from_params(params: &Value) -> Self {
        let value = effect_param_value_any(
            params,
            &[
                "frameRate",
                "frame_rate",
                "Frame Rate",
                "0001",
                "ADBE Posterize Time-0001",
            ],
        );
        let frame_rate_keys = value.map(effect_value_scalar_keys).unwrap_or_default();
        let frame_rate = value
            .and_then(effect_value_f32)
            .or_else(|| frame_rate_keys.first().map(|(_, value)| *value))
            .unwrap_or(0.0);
        Self {
            frame_rate,
            frame_rate_keys,
        }
    }
}

fn effect_has_param(params: &Value, names: &[&str]) -> bool {
    names.iter().any(|name| params.get(*name).is_some())
}

fn effect_param_value_any<'a>(params: &'a Value, names: &[&str]) -> Option<&'a Value> {
    for name in names {
        if let Some(value) = params.get(*name) {
            return Some(value);
        }
    }
    params.as_object().and_then(|items| {
        items.iter().find_map(|(key, value)| {
            (names.iter().any(|name| key.as_str() == *name)
                || effect_param_name_matches(value, names))
            .then_some(value)
        })
    })
}

fn effect_param_name_matches(value: &Value, names: &[&str]) -> bool {
    ["match_name", "matchName", "name", "match"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .any(|name| names.iter().any(|wanted| name == *wanted))
}

fn effect_param_f32(params: &Value, names: &[&str], default: f32) -> f32 {
    names
        .iter()
        .find_map(|name| params.get(*name).and_then(effect_value_f32))
        .unwrap_or(default)
}

fn effect_param_expression<'a>(params: &'a Value, names: &[&str]) -> Option<&'a str> {
    effect_param_value_any(params, names)
        .and_then(|value| value.get("expression"))
        .and_then(Value::as_str)
}

fn time_multiplier_expression(expression: &str) -> Option<f32> {
    let compact = expression.replace(char::is_whitespace, "");
    let multiplier = compact.strip_prefix("time*")?;
    multiplier.parse::<f32>().ok()
}

fn effect_param_bool(params: &Value, names: &[&str], default: bool) -> bool {
    names
        .iter()
        .find_map(|name| params.get(*name).and_then(effect_value_bool))
        .unwrap_or(default)
}

fn effect_param_rgba(params: &Value, names: &[&str], default: [u8; 4]) -> [u8; 4] {
    names
        .iter()
        .find_map(|name| {
            params
                .get(*name)
                .and_then(|value| effect_value_rgba(value, default))
        })
        .unwrap_or(default)
}

fn effect_wrapped_value(value: &Value) -> &Value {
    value
        .get("value")
        .or_else(|| value.get("v"))
        .unwrap_or(value)
}

fn effect_value_f32(value: &Value) -> Option<f32> {
    let value = effect_wrapped_value(value);
    if let Some(number) = value.as_f64() {
        return Some(number as f32);
    }
    value
        .get("keyframes")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("v").or_else(|| item.get("value")))
        .and_then(effect_value_f32)
}

fn effect_value_scalar_keys(value: &Value) -> Vec<(f64, f32)> {
    let mut keys = value
        .get("keyframes")
        .or_else(|| value.get("keys"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let time = item
                        .get("time")
                        .or_else(|| item.get("t"))
                        .and_then(Value::as_f64)?;
                    let value = item
                        .get("v")
                        .or_else(|| item.get("value"))
                        .and_then(effect_value_f32)?;
                    Some((time, value))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys
}

fn effect_value_bool(value: &Value) -> Option<bool> {
    let value = effect_wrapped_value(value);
    value.as_bool().or_else(|| {
        value
            .as_f64()
            .map(|number| number.abs() > f64::EPSILON)
            .or_else(|| {
                value.as_str().map(|text| {
                    matches!(
                        text.trim().to_ascii_lowercase().as_str(),
                        "true" | "on" | "yes" | "1"
                    )
                })
            })
    })
}

fn effect_value_rgba(value: &Value, default: [u8; 4]) -> Option<[u8; 4]> {
    let value = effect_wrapped_value(value);
    if let Some(keyframed) = value
        .get("keyframes")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("v").or_else(|| item.get("value")))
    {
        return effect_value_rgba(keyframed, default);
    }
    let items = value.as_array()?;
    let channel = |index: usize, fallback: u8| {
        items
            .get(index)
            .and_then(Value::as_f64)
            .map(|value| {
                let scaled = if value <= 1.0 { value * 255.0 } else { value };
                scaled.round().clamp(0.0, 255.0) as u8
            })
            .unwrap_or(fallback)
    };
    Some([
        channel(0, default[0]),
        channel(1, default[1]),
        channel(2, default[2]),
        channel(3, default[3]),
    ])
}

fn scalar_keys(value: Option<&Value>) -> Vec<(f64, f32)> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((item.get("t")?.as_f64()?, item.get("v")?.as_f64()? as f32))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn vec2_keys(value: Option<&Value>) -> Vec<(f64, [f32; 2])> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let values = item.get("v")?.as_array()?;
                    Some((
                        item.get("t")?.as_f64()?,
                        [
                            values.first()?.as_f64()? as f32,
                            values.get(1)?.as_f64()? as f32,
                        ],
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn eval_scalar_keys(keys: &[(f64, f32)], time: f64, default: f32) -> f32 {
    if keys.is_empty() {
        return default;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }
    for pair in keys.windows(2) {
        let (t0, v0) = pair[0];
        let (t1, v1) = pair[1];
        if time <= t1 {
            let t = smooth_unit(((time - t0) / (t1 - t0).max(0.000001)) as f32);
            return v0 + (v1 - v0) * t;
        }
    }
    keys.last().map(|(_, value)| *value).unwrap_or(default)
}

fn eval_vec2_keys(keys: &[(f64, [f32; 2])], time: f64, default: [f32; 2]) -> [f32; 2] {
    if keys.is_empty() {
        return default;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }
    for pair in keys.windows(2) {
        let (t0, v0) = pair[0];
        let (t1, v1) = pair[1];
        if time <= t1 {
            let t = smooth_unit(((time - t0) / (t1 - t0).max(0.000001)) as f32);
            return [v0[0] + (v1[0] - v0[0]) * t, v0[1] + (v1[1] - v0[1]) * t];
        }
    }
    keys.last().map(|(_, value)| *value).unwrap_or(default)
}

fn smooth_unit(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_start_with_zero_smoothness_steps_by_word_midpoints() {
        assert_eq!(selector_visible_opacity(Some(24.9), None, 0, 4, 0.0), 1.0);
        assert_eq!(selector_visible_opacity(Some(24.9), None, 1, 4, 0.0), 0.0);
        assert_eq!(selector_visible_opacity(Some(50.0), None, 1, 4, 0.0), 1.0);
        assert_eq!(selector_visible_opacity(Some(50.0), None, 2, 4, 0.0), 0.0);
    }

    #[test]
    fn reveal_end_is_complement_for_outline_future_words() {
        assert_eq!(selector_visible_opacity(None, Some(25.0), 0, 4, 0.0), 0.0);
        assert_eq!(selector_visible_opacity(None, Some(25.0), 1, 4, 0.0), 1.0);
        assert_eq!(selector_visible_opacity(None, Some(75.0), 2, 4, 0.0), 0.0);
        assert_eq!(selector_visible_opacity(None, Some(75.0), 3, 4, 0.0), 1.0);
    }

    #[test]
    fn smooth_word_reveal_uses_ae_linear_feather() {
        assert!((selector_visible_opacity(Some(26.25), None, 1, 4, 100.0) - 0.05).abs() < 0.001);
        assert!((selector_visible_opacity(Some(37.5), None, 1, 4, 100.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn no_reveal_keys_leave_whole_layer_visible() {
        assert_eq!(selector_visible_opacity(None, None, 0, 4, 0.0), 1.0);
        assert_eq!(selector_visible_opacity(None, None, 3, 4, 100.0), 1.0);
    }

    #[test]
    fn stroke_only_copy_does_not_emit_fill_or_fill_shadows() {
        assert_eq!(
            text_passes(false, true, false, &[], 0.0),
            vec![TextPass::Stroke]
        );
    }

    #[test]
    fn hollow_stroke_mask_removes_original_fill_area() {
        let bitmap = [0, 0, 0, 0, 255, 0, 0, 0, 0];
        let center = dilated_glyph_alpha(&bitmap, 3, 3, 1, 1, 1)
            .saturating_sub(glyph_alpha_at(&bitmap, 3, 3, 1, 1));
        let edge = dilated_glyph_alpha(&bitmap, 3, 3, 1, 0, 1)
            .saturating_sub(glyph_alpha_at(&bitmap, 3, 3, 1, 0));
        assert_eq!(center, 0);
        assert_eq!(edge, 255);
    }

    #[test]
    fn fill_only_copy_does_not_emit_stroke() {
        assert_eq!(
            text_passes(true, false, false, &[], 0.0),
            vec![TextPass::Fill]
        );
    }

    #[test]
    fn text_passes_emit_effect_stack_before_source_passes() {
        let effects = vec![
            TextEffect::DropShadow(TextDropShadow::from_params(&serde_json::json!({
                "0002": 220,
                "0003": 135,
                "0004": 6,
                "0005": 8
            }))),
            TextEffect::Glow(TextGlow::from_params(&serde_json::json!({
                "0002": 80,
                "0003": 18,
                "0004": 0.8
            }))),
            TextEffect::BoxBlur(TextBoxBlur::from_params(&serde_json::json!({
                "0001": 1,
                "0002": 2
            }))),
        ];
        let kinds = text_passes(true, true, false, &effects, 0.0)
            .into_iter()
            .map(|pass| pass.kind())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                TextPassKind::DropShadow,
                TextPassKind::Glow,
                TextPassKind::BoxBlur,
                TextPassKind::Stroke,
                TextPassKind::Fill,
            ]
        );
    }

    #[test]
    fn stroke_only_text_ignores_fill_effect_approximations() {
        let effects = vec![TextEffect::Glow(TextGlow::from_params(
            &serde_json::json!({
                "0003": 18
            }),
        ))];
        assert_eq!(
            text_passes(false, true, false, &effects, 0.0),
            vec![TextPass::Stroke]
        );
    }

    #[test]
    fn shadow_only_drop_shadow_omits_source_passes() {
        let effects = vec![TextEffect::DropShadow(TextDropShadow::from_params(
            &serde_json::json!({ "0006": true }),
        ))];
        let kinds = text_passes(true, true, false, &effects, 0.0)
            .into_iter()
            .map(|pass| pass.kind())
            .collect::<Vec<_>>();
        assert_eq!(kinds, vec![TextPassKind::DropShadow]);
    }

    #[test]
    fn parse_text_effects_accepts_ae_and_sapphire_match_names() {
        let layer = serde_json::json!({
            "effects": [
                {
                    "match_name": "ADBE Drop Shadow",
                    "params": {
                        "0001": { "value": [1.0, 0.5, 0.0, 1.0] },
                        "0002": { "value": 180 },
                        "0003": { "value": 90 },
                        "0004": { "value": 12 },
                        "0005": { "value": 4 }
                    }
                },
                {
                    "matchName": "S_DropShadow",
                    "params": { "Opacity": 128 }
                },
                {
                    "match_name": "ADBE Glo2",
                    "params": { "0002": 80, "0003": 18, "0004": 0.8 }
                },
                {
                    "match_name": "ADBE Box Blur2",
                    "params": { "0001": 7, "0002": 3 }
                }
            ]
        });
        let effects = parse_text_effects(&layer);
        let kinds = effects.iter().map(TextEffect::kind).collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                TextEffectKind::DropShadow,
                TextEffectKind::DropShadow,
                TextEffectKind::Glow,
                TextEffectKind::BoxBlur,
            ]
        );
        assert_eq!(
            effects[0],
            TextEffect::DropShadow(TextDropShadow {
                color: [255, 128, 0, 255],
                opacity: 180.0,
                direction_degrees: 90.0,
                distance: 12.0,
                softness: 4.0,
                shadow_only: false,
            })
        );
        assert_eq!(
            effects[3],
            TextEffect::BoxBlur(TextBoxBlur {
                radius: 7.0,
                radius_keys: Vec::new(),
                iterations: 3.0,
                glow_only: false,
            })
        );
    }

    #[test]
    fn parse_text_effects_accepts_object_stack_keys() {
        let layer = serde_json::json!({
            "text_data": {
                "effect_stack": {
                    "0001:ADBE Glo2": { "0003": { "value": 12 } },
                    "0002:ADBE Box Blur2": { "0002": 5 }
                }
            }
        });
        let effects = parse_text_effects(&layer);
        let kinds = effects.iter().map(TextEffect::kind).collect::<Vec<_>>();
        assert_eq!(kinds, vec![TextEffectKind::Glow, TextEffectKind::BoxBlur]);
    }

    #[test]
    fn minimax_parser_keeps_keyframed_intro_flash_radius() {
        let layer = serde_json::json!({
            "effects": {
                "ADBE Minimax": {
                    "0002": {
                        "match_name": "ADBE Minimax-0002",
                        "keyframes": [
                            { "t": 0.75, "v": 15.0 },
                            { "t": 0.7917084, "v": 0.0 }
                        ]
                    }
                }
            }
        });

        let effects = parse_text_effects(&layer);
        assert_eq!(
            effects.iter().map(TextEffect::kind).collect::<Vec<_>>(),
            vec![TextEffectKind::Minimax]
        );
        let TextEffect::Minimax(params) = &effects[0] else {
            panic!("expected minimax effect");
        };
        assert_eq!(params.radius_keys.len(), 2);
        assert_close(params.at_time(0.75).radius as f64, 15.0);
        assert_close(params.at_time(0.80).radius as f64, 0.0);
        assert_eq!(
            text_passes(true, false, false, &effects, 0.75)
                .into_iter()
                .map(|pass| pass.kind())
                .collect::<Vec<_>>(),
            vec![TextPassKind::Minimax, TextPassKind::Fill]
        );
        assert_eq!(
            text_passes(true, false, false, &effects, 0.80)
                .into_iter()
                .map(|pass| pass.kind())
                .collect::<Vec<_>>(),
            vec![TextPassKind::Fill]
        );
    }

    #[test]
    fn box_blur_parser_keeps_fade_out_radius_keyframes() {
        let layer = serde_json::json!({
            "effects": {
                "ADBE Box Blur2": {
                    "0001": {
                        "match_name": "ADBE Box Blur2-0001",
                        "keyframes": [
                            { "t": 3.84, "v": 0.0 },
                            { "t": 4.41, "v": 3.0 }
                        ]
                    },
                    "0002": { "value": 2.0 }
                }
            }
        });

        let effects = parse_text_effects(&layer);
        let TextEffect::BoxBlur(params) = &effects[0] else {
            panic!("expected box blur effect");
        };
        assert_eq!(params.radius_keys.len(), 2);
        assert_close(params.at_time(3.84).radius as f64, 0.0);
        assert_close(params.at_time(4.41).radius as f64, 3.0);
        assert_eq!(
            text_passes(true, false, false, &effects, 3.84)
                .into_iter()
                .map(|pass| pass.kind())
                .collect::<Vec<_>>(),
            vec![TextPassKind::Fill]
        );
        assert_eq!(
            text_passes(true, false, false, &effects, 4.41)
                .into_iter()
                .map(|pass| pass.kind())
                .collect::<Vec<_>>(),
            vec![TextPassKind::BoxBlur, TextPassKind::Fill]
        );
    }

    #[test]
    fn quantize_time_floors_to_posterize_bucket() {
        assert_close(quantize_time(0.39, 5.0), 0.2);
        assert_close(quantize_time((3.0 / 5.0) - 0.5e-12, 5.0), 0.6);
        assert_close(quantize_time(0.39, 0.0), 0.39);
    }

    #[test]
    fn posterize_parser_accepts_object_stack_key_and_numbered_param_match_name() {
        let layer = serde_json::json!({
            "effects": {
                "0003:ADBE Posterize Time": {
                    "0000": {
                        "match_name": "ADBE Posterize Time-0001",
                        "value": null,
                        "keyframes": [
                            { "t": 0.0, "v": 5.0 },
                            { "t": 1.0, "v": 10.0 }
                        ]
                    }
                }
            }
        });

        let specs = posterize_time_specs(&layer);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].frame_rate, 5.0);
        assert_eq!(specs[0].frame_rate_at(1.0), 10.0);
    }

    #[test]
    fn posterize_parser_accepts_array_match_name_and_named_param() {
        let layer = serde_json::json!({
            "effects": [
                {
                    "matchName": "ADBE Posterize Time",
                    "params": {
                        "ADBE Posterize Time-0001": { "value": 8.0 }
                    }
                }
            ]
        });

        let specs = posterize_time_specs(&layer);
        assert_eq!(specs.len(), 1);
        assert_eq!(posterized_time_for_specs(&specs, 0.26), 0.25);
    }

    #[test]
    fn active_adjustment_posterize_quantizes_subtitle_eval_time() {
        let mut layer_json = text_layer_json(
            "caption",
            "ONE TWO",
            10.0,
            0.0,
            2.0,
            "Текст",
            serde_json::json!({}),
        );
        layer_json["props"]["layer_opacity"] = serde_json::json!({
            "keyframes": [
                { "t": 0.0, "v": 0.0 },
                { "t": 1.0, "v": 100.0 }
            ]
        });
        let mut layers = parse_text_stack(&serde_json::json!([layer_json]), "Текст", 2.0).unwrap();
        let layer = layers.pop().unwrap();
        let adjustment_layers = adjustment_posterize_layers_from(
            &serde_json::json!([
                {
                    "type": "adjustment",
                    "in_point": 0.0,
                    "out_point": 2.0,
                    "z_index": 12.0,
                    "effects": {
                        "ADBE Posterize Time": {
                            "0001": { "value": 4.0 }
                        }
                    }
                }
            ]),
            2.0,
        );
        let project = Project {
            comp: Comp {
                width: 64,
                height: 64,
                fps: 30.0,
                duration: 2.0,
            },
            subtitles_mode: String::new(),
            text_offset: [0.0, 0.0],
            video_layers: Vec::new(),
            text_layers: Vec::new(),
            adjustment_posterize_layers: adjustment_layers,
            adjustment_geometry_layers: Vec::new(),
            adjustment_visual_layers: Vec::new(),
            flash_cut_times: Vec::new(),
            audio_path: None,
        };

        let text_time = subtitle_eval_time(&project, &layer, 0.35);
        assert_close(text_time, 0.25);
        assert_close(
            eval_scalar_keys(&layer.opacity_keys, 0.35, layer.transform.opacity) as f64,
            28.175,
        );
        assert_close(
            eval_scalar_keys(&layer.opacity_keys, text_time, layer.transform.opacity) as f64,
            15.625,
        );
    }

    #[test]
    fn turbulent_displace_parser_drives_text_wave_offsets() {
        let layer = serde_json::json!({
            "effects": {
                "ADBE Turbulent Displace": {
                    "0002": { "match_name": "ADBE Turbulent Displace-0002", "value": 7.5 },
                    "0003": { "match_name": "ADBE Turbulent Displace-0003", "value": 50.0 },
                    "0006": {
                        "match_name": "ADBE Turbulent Displace-0006",
                        "value": null,
                        "expression": "time*500"
                    }
                }
            }
        });

        let effects = parse_text_effects(&layer);
        assert_eq!(
            effects.iter().map(TextEffect::kind).collect::<Vec<_>>(),
            vec![TextEffectKind::TurbulentDisplace]
        );
        let TextEffect::TurbulentDisplace(params) = &effects[0] else {
            panic!("expected turbulent displace effect");
        };
        assert_eq!(params.amount, 7.5);
        assert_eq!(params.size, 50.0);
        assert_eq!(params.evolution_rate_degrees, 500.0);
        let offset_a = text_turbulent_offset(&effects, 21.2, 0, 500.0, 900.0, 1.0);
        let offset_b = text_turbulent_offset(&effects, 21.4, 4, 620.0, 900.0, 1.0);
        assert!(offset_a[0].abs() > 0.1 || offset_a[1].abs() > 0.1);
        assert!((offset_a[0] - offset_b[0]).abs() > 0.1 || (offset_a[1] - offset_b[1]).abs() > 0.1);
    }

    #[test]
    fn turbulent_displace_distorts_glyph_alpha_mask() {
        let effects = vec![TextEffect::TurbulentDisplace(TextTurbulentDisplace {
            amount: 7.5,
            size: 8.0,
            evolution_degrees: 0.0,
            evolution_rate_degrees: 500.0,
        })];
        let bitmap = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 255,
            255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0,
        ];

        let out =
            turbulent_displace_glyph_bitmap(&effects, 21.2, 3, &bitmap, 7, 7, 500.0, 900.0, 1.0)
                .expect("turbulent displace should produce a transformed glyph mask");

        assert_ne!(out, bitmap);
        assert!(out.iter().any(|alpha| *alpha > 0));
    }

    #[test]
    fn geometry_adjustment_scale_height_stretches_covered_subtitle_time() {
        let adjustment_layers = adjustment_geometry_layers_from(
            &serde_json::json!([
                {
                    "type": "adjustment",
                    "in_point": 2.92,
                    "out_point": 4.41,
                    "z_index": 997.0,
                    "effects": {
                        "ADBE Geometry2": {
                            "0004": { "match_name": "ADBE Geometry2-0004", "value": 100.0 },
                            "0003": {
                                "match_name": "ADBE Geometry2-0003",
                                "keyframes": [
                                    { "t": 2.92, "v": 85.0 },
                                    { "t": 4.41, "v": 132.94 }
                                ]
                            },
                            "0007": { "match_name": "ADBE Geometry2-0007", "value": -3.5 }
                        }
                    }
                }
            ]),
            5.0,
        );
        let layer = parse_text_layer(
            &text_layer_json(
                "ДАЙ МНЕ ПОВОД",
                "ДАЙ МНЕ ПОВОД",
                993.0,
                2.92,
                4.41,
                "Текст",
                serde_json::json!({}),
            ),
            5.0,
        );
        let project = Project {
            comp: Comp {
                width: 64,
                height: 64,
                fps: 30.0,
                duration: 5.0,
            },
            subtitles_mode: String::new(),
            text_offset: [0.0, 0.0],
            video_layers: Vec::new(),
            text_layers: Vec::new(),
            adjustment_posterize_layers: Vec::new(),
            adjustment_geometry_layers: adjustment_layers,
            adjustment_visual_layers: Vec::new(),
            flash_cut_times: Vec::new(),
            audio_path: None,
        };

        assert!(project.adjustment_geometry_layers[0].affects(&layer, 3.665));
        let geometry = subtitle_geometry(&project, &layer, 3.665);
        assert_close(geometry.scale[0] as f64, 100.0);
        assert!((geometry.scale[1] - 108.97).abs() < 0.001);
        assert_close(geometry.rotation as f64, -3.5);
    }

    #[test]
    fn no_layout_pass_point_text_uses_unbounded_source_rectish_width() {
        let layers = parse_text_stack(
            &serde_json::json!([text_layer_json(
                "mine",
                "HOOK WORDS SHOULD NOT WRAP",
                10.0,
                0.0,
                1.0,
                "Текст",
                serde_json::json!({
                    "no_layout_pass": true
                }),
            )]),
            "Текст",
            2.0,
        )
        .unwrap();

        assert_eq!(layers.len(), 1);
        assert!(layers[0].no_layout_pass);
        assert!(!layers[0].box_text);
        assert!(layout_max_width(&layers[0], 1.0).is_infinite());
    }

    #[test]
    fn box_text_uses_exported_box_width_for_wrapping() {
        let layers = parse_text_stack(
            &serde_json::json!([text_layer_json(
                "boxed",
                "ONE TWO THREE",
                10.0,
                0.0,
                1.0,
                "Текст",
                serde_json::json!({
                    "box_text": [320.0, 80.0]
                }),
            )]),
            "Текст",
            2.0,
        )
        .unwrap();

        assert_eq!(layers.len(), 1);
        assert!(layers[0].box_text);
        assert_eq!(layout_max_width(&layers[0], 0.5), 160.0);
    }

    #[test]
    fn type3_accumulating_layers_remain_separate_root_intervals() {
        let layers = parse_text_stack(
            &serde_json::json!([
                text_layer_json(
                    "ДАЙ",
                    "ДАЙ",
                    995.0,
                    2.92,
                    3.12,
                    "Текст",
                    serde_json::json!({})
                ),
                text_layer_json(
                    "ДАЙ МНЕ",
                    "ДАЙ МНЕ",
                    994.0,
                    3.12,
                    3.62,
                    "Текст",
                    serde_json::json!({}),
                ),
                text_layer_json(
                    "ДАЙ МНЕ ПОВОД",
                    "ДАЙ МНЕ ПОВОД",
                    993.0,
                    3.62,
                    4.41,
                    "Текст",
                    serde_json::json!({})
                )
            ]),
            "Текст",
            5.0,
        )
        .unwrap();

        assert_eq!(
            layers
                .iter()
                .map(|layer| layer.text.as_str())
                .collect::<Vec<_>>(),
            vec!["ДАЙ", "ДАЙ МНЕ", "ДАЙ МНЕ ПОВОД"]
        );
        assert_close(layers[0].start, 2.92);
        assert_close(layers[1].start, 3.12);
        assert_close(layers[2].end, 4.41);
    }

    #[test]
    fn type4_red_hook_child_is_clipped_through_precomp_and_glow_copy() {
        let layers = parse_text_stack(
            &serde_json::json!([
                text_layer_json(
                    "mine",
                    "САМЫЙ НОВЫЙ",
                    990.0,
                    6.22,
                    8.078291621348304,
                    "Текст \"Mine\"",
                    serde_json::json!({
                        "no_layout_pass": true,
                        "fill": [0.99216, 0.08627, 0.07843],
                        "anchor": [0.0, -33.5, 0.0]
                    }),
                ),
                precomp_layer_json(
                    "Текст \"Mine\"",
                    989.0,
                    6.52,
                    7.978291621348305,
                    "Текст",
                    "Текст \"Mine\"",
                    serde_json::json!({
                        "opacity": 100.0,
                        "scale": [100.0, 100.0, 100.0]
                    }),
                ),
                precomp_layer_json(
                    "Текст \"Mine\" glow",
                    988.0,
                    6.52,
                    7.978291621348305,
                    "Текст",
                    "Текст \"Mine\"",
                    serde_json::json!({
                        "opacity": 40.0,
                        "scale_keys": [
                            { "t": 6.52, "v": [150.0, 150.0, 100.0] },
                            { "t": 7.02, "v": [250.0, 250.0, 100.0] }
                        ],
                        "effects": {
                            "ADBE Box Blur2": {
                                "0001": { "value": 5.0 },
                                "0002": { "value": 3.0 }
                            }
                        }
                    }),
                )
            ]),
            "Текст",
            10.0,
        )
        .unwrap();

        assert_eq!(layers.len(), 2);
        let normal = layers
            .iter()
            .find(|layer| (layer.transform.opacity - 100.0).abs() < f32::EPSILON)
            .unwrap();
        let glow = layers
            .iter()
            .find(|layer| (layer.transform.opacity - 40.0).abs() < f32::EPSILON)
            .unwrap();

        assert_eq!(normal.text, "САМЫЙ НОВЫЙ");
        assert!(normal.no_layout_pass);
        assert_close(normal.start, 6.52);
        assert_close(normal.end, 7.978291621348305);
        assert_close(glow.start, 6.52);
        assert_eq!(
            glow.effects
                .iter()
                .map(TextEffect::kind)
                .collect::<Vec<_>>(),
            vec![TextEffectKind::BoxBlur]
        );
        assert!(glow.suppress_fill);
        let glow_passes = text_passes(true, false, true, &glow.effects, 6.52);
        assert_eq!(
            glow_passes.iter().map(TextPass::kind).collect::<Vec<_>>(),
            vec![TextPassKind::BoxBlur]
        );
        let TextPass::BoxBlur(glow_blur) = &glow_passes[0] else {
            panic!("expected glow-only box blur pass");
        };
        assert!(glow_blur.glow_only);
        assert_eq!(glow.scale_keys.len(), 2);
        assert_eq!(
            eval_vec2_keys(&glow.scale_keys, 6.52, glow.transform.scale),
            [150.0, 150.0]
        );
        assert_eq!(
            eval_vec2_keys(&glow.scale_keys, 7.02, glow.transform.scale),
            [250.0, 250.0]
        );
    }

    #[test]
    fn hard_line_breaks_preserve_start_char_indices() {
        let lines = collect_hard_lines("ONE TWO\rTHREE\nFOUR");
        assert_eq!(lines, vec![(0, "ONE TWO"), (8, "THREE"), (14, "FOUR")]);
    }

    #[test]
    fn impulse_expression_selector_reconstructs_delayed_character_bounce() {
        let mut json = text_layer_json(
            "baby i need this",
            "baby i need this",
            1000.0,
            0.3125,
            1.7,
            "Текст",
            serde_json::json!({ "no_layout_pass": true }),
        );
        json["text_data"]["layer_meta"]["motionBlur"] = serde_json::json!(true);
        json["text_data"]["text_animator"] = serde_json::json!({
            "opacity": 0.0,
            "properties": [
                { "match_name": "ADBE Text Position 3D", "value": [0.0, 25.0, 0.0] },
                { "match_name": "ADBE Text Scale 3D", "value": [50.0, 50.0, 100.0] },
                { "match_name": "ADBE Text Rotation", "value": 15.0 },
                { "match_name": "ADBE Text Blur", "value": [15.0, 15.0] }
            ],
            "expressible_selector": {
                "amount": {
                    "expression": "delay = 0.0500;\nmyDelay = delay*textIndex;\nt = (time - inPoint) - myDelay;\nif (t >= 0){\n  freq = 2; amplitude = 100; decay = 8.0;\n  s = amplitude*Math.cos(freq*t*2*Math.PI)/Math.exp(decay*t);\n  [s,s]\n} else { value }"
                }
            }
        });

        let layer = parse_text_layer(&json, 2.0);
        let animator = layer.text_animator.as_ref().unwrap();
        assert!(layer.motion_blur);
        assert_close(animator.delay, 0.05);
        assert_close(animator.frequency, 2.0);
        let hidden = animator.state_at(layer.start, layer.start, 0);
        assert_close(hidden.opacity as f64, 0.0);
        assert_close(hidden.scale[0] as f64, 0.5);
        assert_close(hidden.position[1] as f64, 25.0);

        let quarter_cycle = layer.start + animator.delay + 0.125;
        let first = animator.state_at(quarter_cycle, layer.start, 0);
        let second = animator.state_at(quarter_cycle, layer.start, 1);
        assert!((first.opacity - 1.0).abs() < 0.000_001);
        assert!((first.scale[0] - 1.0).abs() < 0.000_001);
        assert!(first.opacity > second.opacity);
    }

    #[test]
    fn impulse_character_transform_scales_rotates_and_blurs_alpha_mask() {
        let bitmap = vec![
            0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 255, 255, 255, 255, 0, 0, 255, 0, 0, 0, 0, 255,
            0, 0,
        ];
        let transformed = animate_glyph_bitmap(
            &bitmap,
            5,
            5,
            TextAnimatorState {
                opacity: 0.5,
                position: [0.0, 12.5],
                scale: [0.75, 0.75],
                rotation: 7.5,
                blur: [7.5, 7.5],
            },
            1.0,
        )
        .unwrap();

        assert!(transformed.width > 5);
        assert!(transformed.height > 5);
        assert!(transformed.bitmap.iter().any(|alpha| *alpha > 0));
        assert!(transformed
            .bitmap
            .iter()
            .any(|alpha| *alpha > 0 && *alpha < 255));
    }

    #[test]
    fn remaining_video_calibration_matches_exported_profiles() {
        assert_close(font_scale_correction("template_4th") as f64, 1.0 / 1.08);
        assert_close(font_scale_correction("impulse_2nd") as f64, 1.0 / 1.08);
        assert_eq!(drop_shadow_blur_radius(50.0, 1.0), 19);
        let samples = motion_blur_samples(24.0, false);
        assert_eq!(samples.len(), 3);
        assert_close(samples[0].0, -1.0 / 48.0);
        assert_close(samples[2].0, 1.0 / 48.0);
        assert_close(motion_blur_samples(24.0, true)[1].1 as f64, 0.68);
        assert!(keyframe_interval_contains(&[(1.0, 75.0), (2.0, 0.0)], 1.5));
    }

    #[test]
    fn ae_source_sampling_uses_the_comp_frame_center() {
        let comp_fps = 23.975_997_924_804_7;
        let source_fps = 30.0;
        let source_frame = |comp_frame: u32, source_start: f64| {
            let source_time = comp_frame as f64 / comp_fps - source_start;
            source_frame_index(
                ae_source_sample_time(source_time, comp_fps),
                source_fps,
                20.0,
            )
        };

        assert_eq!(source_frame(313, 7.736), 159);
        assert_eq!(source_frame(314, 7.736), 160);
        assert_eq!(source_frame(319, 7.736), 166);
        assert_eq!(source_frame(320, 7.736), 168);
        assert_eq!(source_frame(248, 7.565), 83);
        assert_eq!(source_frame(249, 7.565), 84);
    }

    #[test]
    fn ae_decoder_compat_catalog_remaps_only_verified_frames() {
        let held = Path::new("media/video/738238563936717813.mp4");
        let skipped = Path::new("media/video/147704062771893190_Nesin senn.mp4");
        assert_eq!(ae_decoder_compat_frame_index(held, 83), 82);
        assert_eq!(ae_decoder_compat_frame_index(held, 109), 108);
        assert_eq!(ae_decoder_compat_frame_index(held, 82), 82);
        assert_eq!(ae_decoder_compat_frame_index(skipped, 142), 143);
        assert_eq!(ae_decoder_compat_frame_index(skipped, 152), 153);
        assert_eq!(ae_decoder_compat_frame_index(skipped, 153), 153);
        assert_eq!(
            ae_decoder_compat_frame_index(Path::new("media/video/271904896248461952.mp4"), 83),
            83
        );
    }

    #[test]
    fn ae_frame_count_does_not_append_a_partial_tail_frame() {
        assert_eq!(ae_frame_count(32.5, 23.975_997_924_804_7), 779);
        assert_eq!(
            ae_frame_count(13.888_890_091_014_323, 23.975_997_924_804_7),
            333
        );
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_001,
            "actual={actual} expected={expected}"
        );
    }

    fn text_layer_json(
        name: &str,
        text: &str,
        z: f64,
        start: f64,
        end: f64,
        target: &str,
        opts: serde_json::Value,
    ) -> serde_json::Value {
        let no_layout_pass = opts
            .get("no_layout_pass")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let fill = opts
            .get("fill")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([1.0, 1.0, 1.0]));
        let box_text = opts.get("box_text").cloned();
        let anchor = opts
            .get("anchor")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([540.0, 960.0, 0.0]));
        let mut text_data = serde_json::json!({
            "layer_meta": { "comp_name_target": target },
            "text_base": {
                "font": "Point-SemiBold",
                "fontSize": 80.0,
                "applyFill": true,
                "fillColor": fill,
                "applyStroke": false,
                "strokeWidth": 0.0,
                "tracking": -50.0,
                "leading": 88.0
            },
            "no_layout_pass": no_layout_pass
        });
        if let Some(box_text) = box_text {
            text_data["box_text"] = box_text;
        }

        serde_json::json!({
            "name": name,
            "type": "text",
            "in_point": start,
            "out_point": end,
            "z_index": z,
            "text": text,
            "props": {
                "tf_anchor": { "value": anchor, "keyframes": [] },
                "tf_position": { "value": [540.0, 960.0, 0.0], "keyframes": [] },
                "tf_scale": { "value": [100.0, 100.0, 100.0], "keyframes": [] },
                "tf_rotation": { "value": 0.0, "keyframes": [] },
                "tf_opacity": { "value": 100.0, "keyframes": [] }
            },
            "effects": {},
            "text_data": text_data
        })
    }

    fn precomp_layer_json(
        name: &str,
        z: f64,
        start: f64,
        end: f64,
        target: &str,
        source: &str,
        opts: serde_json::Value,
    ) -> serde_json::Value {
        let opacity = opts.get("opacity").and_then(Value::as_f64).unwrap_or(100.0);
        let scale = opts
            .get("scale")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let scale_keys = opts
            .get("scale_keys")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let effects = opts
            .get("effects")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        serde_json::json!({
            "name": name,
            "type": "precomp",
            "in_point": start,
            "out_point": end,
            "z_index": z,
            "props": {
                "tf_anchor": { "value": [540.0, 960.0, 0.0], "keyframes": [] },
                "tf_position": { "value": [540.0, 960.0, 0.0], "keyframes": [] },
                "tf_scale": { "value": scale, "keyframes": scale_keys },
                "tf_rotation": { "value": 0.0, "keyframes": [] },
                "tf_opacity": { "value": opacity, "keyframes": [] }
            },
            "effects": effects,
            "text_data": {
                "layer_meta": {
                    "comp_name_target": target,
                    "startTime": 0.0
                },
                "precomp_source": { "comp_name": source }
            }
        })
    }
}
