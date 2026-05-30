use crate::bee_text_carrier::{
    classify_bee_text_carrier_routes, BeeTextCarrierRouteRecord, BeeTextCarrierRouteStatus,
};
use crate::motion_blur;
use crate::precomp::PrecompGraph;
use effects::{posterize_time::PosterizeTimeParams, EffectContext, EffectRegistry};
use expression_engine::{
    BoundaryPropertyExpressionEvaluator, CompExpressionContext, ExprValue, LayerExpressionContext,
    NamedPatternExpressionEvaluator, NoopPropertyExpressionHost, PropertyExpressionContext,
    PropertyExpressionEvaluator, PropertyExpressionMode, PropertyExpressionRequest,
    PropertyValueType,
};
use raster_cpu::{composite_normal, composite_normal_pixel, BilinearSampler, Canvas, Sampler};
use render_ir::{
    Composition, EffectSpec, Layer, PositionExpression, Rect, ScalarKeyframe, Scene,
    TextAnimatorSpec, TextExpressionSelector, TextSelectorBasedOn, TextSelectorShape, Vec2Keyframe,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;
use text_engine::{
    layout_text, rasterize_text, rasterize_text_with_layout,
    rasterize_text_with_layout_p6_pixels_disabled, rasterize_text_with_layout_vector_transform,
    GlyphLayoutTelemetry, TextLayoutRequest, TextLayoutResult, TextRasterTrace,
    TextVectorTransform,
};
use transform_math::{Mat3, Transform2D, Vec2};

pub trait FootageProvider {
    fn frame_at(&mut self, source: &str, time: f64) -> anyhow::Result<Option<Canvas>>;
}

#[derive(Debug, Clone)]
pub struct FrameRenderTrace {
    pub frame: u32,
    pub time: f64,
    pub layers: Vec<LayerTiming>,
    pub effects: Vec<EffectTiming>,
    pub adjustment_effects: Vec<AdjustmentEffectTrace>,
    pub temporal: Vec<TemporalTraceRecord>,
    pub keyframes: Vec<KeyframeTraceRecord>,
    pub motion_blur: Vec<MotionBlurTrace>,
    pub effect_debug: Vec<EffectDebugRecord>,
    pub text_layouts: Vec<Value>,
    pub text_selector_weights: Vec<Value>,
    pub position_expressions: Vec<Value>,
    pub collapse: Vec<Value>,
}

#[derive(Debug, Clone, Copy)]
struct BeeTextCarrierRenderContext<'a> {
    parent_composition: &'a str,
    precomp_layer_id: &'a str,
    target_composition: &'a str,
    source_time: f64,
}

#[derive(Debug, Clone)]
pub struct LayerTiming {
    pub composition: String,
    pub layer_id: String,
    pub layer_type: &'static str,
    pub content_ms: f64,
    pub effects_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone)]
pub struct EffectTiming {
    pub composition: String,
    pub layer_id: String,
    pub match_name: String,
    pub elapsed_ms: f64,
}

#[derive(Debug, Clone)]
pub struct EffectDebugRecord {
    pub composition: String,
    pub layer_id: String,
    pub effect_index: usize,
    pub match_name: String,
    pub application: &'static str,
    pub effect_time: f64,
    pub trace: Value,
}

#[derive(Debug, Clone)]
pub struct AdjustmentEffectTrace {
    pub composition: String,
    pub layer_id: String,
    pub effect_index: usize,
    pub match_name: String,
    pub comp_time: f64,
    pub layer_time: f64,
    pub lower_stack_time: f64,
    pub param_time: f64,
    pub input_hash: String,
    pub output_hash: String,
    pub posterize: Option<PosterizeTiming>,
}

#[derive(Debug, Clone)]
pub struct TemporalTraceRecord {
    pub event: &'static str,
    pub composition: String,
    pub layer_id: String,
    pub layer_type: &'static str,
    pub comp_time: f64,
    pub layer_start: f64,
    pub layer_time: f64,
    pub posterized_time: f64,
    pub source_id: Option<String>,
    pub source_start: Option<f64>,
    pub source_time: Option<f64>,
    pub source_frame_id: Option<u32>,
    pub source_frame: Option<SourceFrameQuantization>,
    pub adjustment_lower_stack_time: Option<f64>,
    pub posterize: Option<PosterizeTiming>,
}

#[derive(Debug, Clone)]
pub struct KeyframeTraceRecord {
    pub event: &'static str,
    pub composition: String,
    pub layer_id: String,
    pub property: &'static str,
    pub sample_time: f64,
    pub value: Value,
    pub phase: &'static str,
    pub interpolation: &'static str,
    pub segment_index: Option<usize>,
    pub key_start_time: Option<f64>,
    pub key_end_time: Option<f64>,
    pub normalized_time: Option<f32>,
    pub eased_progress: Option<f32>,
    pub hold: bool,
    pub approximate: bool,
    pub ease: Option<render_ir::KeyframeEase>,
}

#[derive(Debug, Clone)]
pub struct SourceFrameQuantization {
    pub frame_id: u32,
    pub frame_rate: f64,
    pub frame_time: f64,
    pub subframe: f64,
    pub policy: &'static str,
}

#[derive(Debug, Clone)]
pub struct MotionBlurTrace {
    pub composition: String,
    pub layer_id: String,
    pub comp_time: f64,
    pub frame_duration: f64,
    pub shutter_open: f64,
    pub shutter_close: f64,
    pub shutter_angle: f64,
    pub shutter_phase: f64,
    pub requested_samples: u32,
    pub effective_samples: usize,
    pub divisor: f32,
    pub samples: Vec<MotionBlurSampleTrace>,
}

#[derive(Debug, Clone, Copy)]
pub struct MotionBlurWeightSummary {
    pub active_samples: usize,
    pub contributing_samples: usize,
    pub total_weight: f32,
}

impl MotionBlurTrace {
    pub fn weight_summary(&self) -> MotionBlurWeightSummary {
        let mut summary = MotionBlurWeightSummary {
            active_samples: 0,
            contributing_samples: 0,
            total_weight: 0.0,
        };
        for sample in &self.samples {
            if sample.active {
                summary.active_samples += 1;
            }
            if sample.weight > 0.0 {
                summary.contributing_samples += 1;
                summary.total_weight += sample.weight;
            }
        }
        summary
    }
}

#[derive(Debug, Clone)]
pub struct MotionBlurSampleTrace {
    pub sample_index: usize,
    pub sample_time: f64,
    pub layer_time: f64,
    pub posterized_time: f64,
    pub source_time: Option<f64>,
    pub source_frame_id: Option<u32>,
    pub source_frame: Option<SourceFrameQuantization>,
    pub active: bool,
    pub opacity: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct PosterizeTiming {
    pub frame_rate: f32,
    pub bucket: Option<i64>,
    pub bucket_time: f64,
}

pub struct CheckerboardFootageProvider;

impl FootageProvider for CheckerboardFootageProvider {
    fn frame_at(&mut self, _source: &str, _time: f64) -> anyhow::Result<Option<Canvas>> {
        Ok(None)
    }
}

pub fn render_frame(scene: &Scene, frame_index: u32) -> anyhow::Result<Canvas> {
    let mut footage = CheckerboardFootageProvider;
    render_frame_with_footage(scene, frame_index, &mut footage)
}

pub fn render_frame_with_footage(
    scene: &Scene,
    frame_index: u32,
    footage: &mut dyn FootageProvider,
) -> anyhow::Result<Canvas> {
    let comp = &scene.composition;
    let time = frame_index as f64 / comp.fps;
    render_composition_frame(
        scene,
        comp,
        &scene.layers,
        time,
        footage,
        &mut Vec::new(),
        TextP6PixelPolicy::Default,
        None,
    )
}

pub fn render_frame_with_footage_traced(
    scene: &Scene,
    frame_index: u32,
    footage: &mut dyn FootageProvider,
) -> anyhow::Result<(Canvas, FrameRenderTrace)> {
    let comp = &scene.composition;
    let time = frame_index as f64 / comp.fps;
    let mut trace = FrameRenderTrace {
        frame: frame_index,
        time,
        layers: Vec::new(),
        effects: Vec::new(),
        adjustment_effects: Vec::new(),
        temporal: Vec::new(),
        keyframes: Vec::new(),
        motion_blur: Vec::new(),
        effect_debug: Vec::new(),
        text_layouts: Vec::new(),
        text_selector_weights: Vec::new(),
        position_expressions: Vec::new(),
        collapse: Vec::new(),
    };
    let canvas = render_composition_frame(
        scene,
        comp,
        &scene.layers,
        time,
        footage,
        &mut Vec::new(),
        TextP6PixelPolicy::Default,
        Some(&mut trace),
    )?;
    Ok((canvas, trace))
}

fn render_composition_frame(
    scene: &Scene,
    comp: &Composition,
    layers: &[Layer],
    time: f64,
    footage: &mut dyn FootageProvider,
    stack: &mut Vec<String>,
    text_p6_pixel_policy: TextP6PixelPolicy,
    mut trace: Option<&mut FrameRenderTrace>,
) -> anyhow::Result<Canvas> {
    let mut canvas = Canvas::new(comp.width, comp.height, comp.background);

    // IR order is top-to-bottom like AE. Render bottom-to-top.
    for layer_index in (0..layers.len()).rev() {
        let layer = &layers[layer_index];
        if !layer.is_active(time) {
            continue;
        }
        if let Layer::Adjustment { start, effects, .. } = layer {
            let layer_started = Instant::now();
            let effects_started = Instant::now();
            let layer_time = (time - *start).max(0.0);
            let posterize = first_posterize_timing(effects, time);
            let lower_stack_time = posterize
                .as_ref()
                .map(|timing| timing.posterize.bucket_time)
                .unwrap_or(time);
            record_adjustment_temporal_trace(
                trace.as_deref_mut(),
                comp,
                layer,
                time,
                layer_time,
                lower_stack_time,
                posterize.map(|timing| timing.posterize),
            );
            let input_canvas = if time_changed(lower_stack_time, time) {
                render_composition_frame(
                    scene,
                    comp,
                    &layers[layer_index + 1..],
                    lower_stack_time,
                    footage,
                    stack,
                    text_p6_pixel_policy,
                    None,
                )?
            } else {
                canvas
            };
            canvas = apply_adjustment_effects_to_canvas(
                effects,
                &input_canvas,
                AdjustmentTimeRouting {
                    comp_time: time,
                    layer_time,
                    lower_stack_time,
                    posterize,
                },
                comp.fps,
                trace.as_deref_mut(),
                &comp.id,
                layer.id(),
            )?;
            let effects_ms = elapsed_ms(effects_started);
            if let Some(trace) = trace.as_deref_mut() {
                trace.layers.push(LayerTiming {
                    composition: comp.id.clone(),
                    layer_id: layer.id().to_string(),
                    layer_type: layer_type(layer),
                    content_ms: 0.0,
                    effects_ms,
                    total_ms: elapsed_ms(layer_started),
                });
            }
            continue;
        }
        let layer_time = posterized_time_for_effects(effects_of(layer), time);
        let layer_opacity = opacity_of(layer, layer_time);
        if layer_opacity <= 0.0 {
            continue;
        }
        if should_motion_blur_layer(comp, layer) {
            let layer_canvas = render_motion_blurred_layer(
                scene,
                comp,
                layer,
                time,
                footage,
                stack,
                text_p6_pixel_policy,
                trace.as_deref_mut(),
            )?;
            composite_normal(&mut canvas, &layer_canvas, 100.0);
        } else {
            let layer_canvas = render_layer_stub(
                scene,
                comp,
                layer,
                time,
                footage,
                stack,
                text_p6_pixel_policy,
                trace.as_deref_mut(),
            )?;
            composite_normal(&mut canvas, &layer_canvas, layer_opacity);
        }
    }

    Ok(canvas)
}

fn should_motion_blur_layer(comp: &Composition, layer: &Layer) -> bool {
    comp.motion_blur.enabled
        && layer_transform(layer)
            .map(|transform| transform.motion_blur)
            .unwrap_or(false)
}

fn layer_transform(layer: &Layer) -> Option<&render_ir::Transform2D> {
    match layer {
        Layer::Solid { transform, .. }
        | Layer::Footage { transform, .. }
        | Layer::Text { transform, .. }
        | Layer::Precomp { transform, .. } => Some(transform),
        Layer::Adjustment { .. } => None,
    }
}

fn render_motion_blurred_layer(
    scene: &Scene,
    comp: &Composition,
    layer: &Layer,
    frame_time: f64,
    footage: &mut dyn FootageProvider,
    stack: &mut Vec<String>,
    text_p6_pixel_policy: TextP6PixelPolicy,
    mut trace: Option<&mut FrameRenderTrace>,
) -> anyhow::Result<Canvas> {
    let frame_duration = if comp.fps > 0.0 {
        1.0 / comp.fps
    } else {
        1.0 / 30.0
    };
    let samples = motion_blur::sample_times(frame_time, frame_duration, comp.motion_blur);
    let divisor = samples.len().max(1) as f32;
    let (shutter_open, shutter_close) = if comp.motion_blur.enabled
        && comp.motion_blur.samples > 1
        && comp.motion_blur.shutter_angle > 0.0
    {
        motion_blur::shutter_interval(
            frame_time,
            frame_duration,
            comp.motion_blur.shutter_angle,
            comp.motion_blur.shutter_phase,
        )
    } else {
        (frame_time, frame_time)
    };
    let mut sample_traces = Vec::with_capacity(samples.len());
    let mut accum = vec![0.0_f32; (comp.width * comp.height * 4) as usize];

    for (sample_index, sample_time) in samples.iter().copied().enumerate() {
        let sample_posterized_time = posterized_time_for_effects(effects_of(layer), sample_time);
        let sample_active = layer.is_active(sample_time);
        let sample_layer_time = layer_local_time(layer, sample_time);
        let sample_opacity = if sample_active {
            opacity_of(layer, sample_posterized_time)
        } else {
            0.0
        };
        let sample_weight = if sample_opacity > 0.0 {
            sample_opacity / (100.0 * divisor)
        } else {
            0.0
        };
        let sample_source_time = source_time_for_layer(layer, sample_posterized_time);
        let sample_source_frame =
            sample_source_time.and_then(|time| source_frame_quantization(time, comp.fps));
        sample_traces.push(MotionBlurSampleTrace {
            sample_index,
            sample_time,
            layer_time: sample_layer_time,
            posterized_time: sample_posterized_time,
            source_time: sample_source_time,
            source_frame_id: sample_source_frame.as_ref().map(|frame| frame.frame_id),
            source_frame: sample_source_frame,
            active: sample_active,
            opacity: sample_opacity,
            weight: sample_weight,
        });
        if !sample_active {
            continue;
        }
        if sample_opacity <= 0.0 {
            continue;
        }
        let sample = render_layer_stub(
            scene,
            comp,
            layer,
            sample_time,
            footage,
            stack,
            text_p6_pixel_policy,
            trace.as_deref_mut(),
        )?;
        accumulate_motion_sample(&mut accum, &sample, sample_weight);
    }

    if let Some(trace) = trace.as_deref_mut() {
        trace.motion_blur.push(MotionBlurTrace {
            composition: comp.id.clone(),
            layer_id: layer.id().to_string(),
            comp_time: frame_time,
            frame_duration,
            shutter_open,
            shutter_close,
            shutter_angle: comp.motion_blur.shutter_angle,
            shutter_phase: comp.motion_blur.shutter_phase,
            requested_samples: comp.motion_blur.samples,
            effective_samples: samples.len(),
            divisor,
            samples: sample_traces,
        });
    }

    let mut canvas = Canvas::transparent(comp.width, comp.height);
    for (index, chunk) in accum.chunks_exact(4).enumerate() {
        let pixel = [
            chunk[0].round().clamp(0.0, 255.0) as u8,
            chunk[1].round().clamp(0.0, 255.0) as u8,
            chunk[2].round().clamp(0.0, 255.0) as u8,
            chunk[3].round().clamp(0.0, 255.0) as u8,
        ];
        let x = index as u32 % comp.width;
        let y = index as u32 / comp.width;
        canvas.set_pixel(x, y, pixel);
    }
    Ok(canvas)
}

fn accumulate_motion_sample(accum: &mut [f32], sample: &Canvas, weight: f32) {
    for (dst, src) in accum.chunks_exact_mut(4).zip(sample.data.chunks_exact(4)) {
        dst[0] += src[0] as f32 * weight;
        dst[1] += src[1] as f32 * weight;
        dst[2] += src[2] as f32 * weight;
        dst[3] += src[3] as f32 * weight;
    }
}

fn opacity_of(layer: &Layer, time: f64) -> f32 {
    match layer {
        Layer::Solid { transform, .. }
        | Layer::Footage { transform, .. }
        | Layer::Text { transform, .. }
        | Layer::Precomp { transform, .. } => {
            evaluate_scalar_keyframes(&transform.animation.opacity, time, transform.opacity)
        }
        Layer::Adjustment { .. } => 100.0,
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

fn effects_are_turbulent_only(effects: &[EffectSpec]) -> bool {
    !effects.is_empty()
        && effects
            .iter()
            .all(|effect| effect.match_name == "ADBE Turbulent Displace")
}

fn posterized_time_for_effects(effects: &[EffectSpec], time: f64) -> f64 {
    effects.iter().fold(time, |current_time, spec| {
        if spec.match_name == "ADBE Posterize Time" {
            PosterizeTimeParams::from_json(&spec.params).quantized_time(current_time)
        } else {
            current_time
        }
    })
}

#[derive(Debug, Clone, Copy)]
struct FirstPosterizeTiming {
    effect_index: usize,
    posterize: PosterizeTiming,
}

#[derive(Debug, Clone, Copy)]
struct AdjustmentTimeRouting {
    comp_time: f64,
    layer_time: f64,
    lower_stack_time: f64,
    posterize: Option<FirstPosterizeTiming>,
}

fn first_posterize_timing(effects: &[EffectSpec], time: f64) -> Option<FirstPosterizeTiming> {
    effects.iter().enumerate().find_map(|(effect_index, spec)| {
        (spec.match_name == "ADBE Posterize Time").then(|| {
            let params = PosterizeTimeParams::from_json(&spec.params);
            FirstPosterizeTiming {
                effect_index,
                posterize: PosterizeTiming {
                    frame_rate: params.frame_rate,
                    bucket: posterize_bucket_id(time, params.frame_rate),
                    bucket_time: params.quantized_time(time),
                },
            }
        })
    })
}

fn posterize_bucket_id(time: f64, frame_rate: f32) -> Option<i64> {
    if !time.is_finite() || !frame_rate.is_finite() || frame_rate <= 0.0 {
        return None;
    }
    Some(((time * frame_rate as f64) + 1.0e-9).floor() as i64)
}

fn time_changed(a: f64, b: f64) -> bool {
    (a - b).abs() > 1.0e-9
}

fn record_layer_temporal_trace(
    trace: Option<&mut FrameRenderTrace>,
    comp: &Composition,
    layer: &Layer,
    comp_time: f64,
    posterized_time: f64,
) {
    let Some(trace) = trace else {
        return;
    };
    let source_time = source_time_for_layer(layer, posterized_time);
    let source_frame = source_time.and_then(|time| source_frame_quantization(time, comp.fps));
    trace.temporal.push(TemporalTraceRecord {
        event: "temporal.layer_time",
        composition: comp.id.clone(),
        layer_id: layer.id().to_string(),
        layer_type: layer_type(layer),
        comp_time,
        layer_start: layer_start(layer),
        layer_time: layer_local_time(layer, comp_time),
        posterized_time,
        source_id: source_id_for_layer(layer),
        source_start: source_start_for_layer(layer),
        source_time,
        source_frame_id: source_frame.as_ref().map(|frame| frame.frame_id),
        source_frame,
        adjustment_lower_stack_time: None,
        posterize: first_posterize_timing(effects_of(layer), comp_time)
            .map(|timing| timing.posterize),
    });
}

fn record_adjustment_temporal_trace(
    trace: Option<&mut FrameRenderTrace>,
    comp: &Composition,
    layer: &Layer,
    comp_time: f64,
    layer_time: f64,
    lower_stack_time: f64,
    posterize: Option<PosterizeTiming>,
) {
    let Some(trace) = trace else {
        return;
    };
    trace.temporal.push(TemporalTraceRecord {
        event: "temporal.adjustment_layer_time",
        composition: comp.id.clone(),
        layer_id: layer.id().to_string(),
        layer_type: layer_type(layer),
        comp_time,
        layer_start: layer_start(layer),
        layer_time,
        posterized_time: lower_stack_time,
        source_id: None,
        source_start: None,
        source_time: None,
        source_frame_id: None,
        source_frame: None,
        adjustment_lower_stack_time: Some(lower_stack_time),
        posterize,
    });
}

fn layer_start(layer: &Layer) -> f64 {
    match layer {
        Layer::Solid { start, .. }
        | Layer::Footage { start, .. }
        | Layer::Text { start, .. }
        | Layer::Precomp { start, .. }
        | Layer::Adjustment { start, .. } => *start,
    }
}

fn layer_local_time(layer: &Layer, comp_time: f64) -> f64 {
    (comp_time - layer_start(layer)).max(0.0)
}

fn source_id_for_layer(layer: &Layer) -> Option<String> {
    match layer {
        Layer::Footage { source, .. } => Some(source.clone()),
        Layer::Precomp { composition, .. } => Some(composition.clone()),
        _ => None,
    }
}

fn source_start_for_layer(layer: &Layer) -> Option<f64> {
    match layer {
        Layer::Footage { source_start, .. } => Some(*source_start),
        Layer::Precomp { .. } => Some(0.0),
        _ => None,
    }
}

fn source_time_for_layer(layer: &Layer, posterized_time: f64) -> Option<f64> {
    match layer {
        Layer::Footage {
            start,
            source_start,
            ..
        } => Some((*source_start + (posterized_time - *start)).max(0.0)),
        Layer::Precomp { start, .. } => Some((posterized_time - *start).max(0.0)),
        _ => None,
    }
}

const SOURCE_FRAME_QUANTIZATION_EPSILON: f64 = 1.0e-9;
const SOURCE_FRAME_QUANTIZATION_POLICY: &str = "floor(source_time*frame_rate+1e-9)";

fn source_frame_quantization(source_time: f64, fps: f64) -> Option<SourceFrameQuantization> {
    if !source_time.is_finite() || !fps.is_finite() || fps <= 0.0 {
        return None;
    }
    let frame = ((source_time * fps) + SOURCE_FRAME_QUANTIZATION_EPSILON).floor();
    if !(0.0..=u32::MAX as f64).contains(&frame) {
        return None;
    }
    let frame_id = frame as u32;
    let frame_time = frame_id as f64 / fps;
    Some(SourceFrameQuantization {
        frame_id,
        frame_rate: fps,
        frame_time,
        subframe: (source_time - frame_time) * fps,
        policy: SOURCE_FRAME_QUANTIZATION_POLICY,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextP6PixelPolicy {
    Default,
    DisabledForPrecompRasterInput,
}

impl TextP6PixelPolicy {
    fn layer_text_render_path(self) -> &'static str {
        match self {
            Self::Default => "layer_text",
            Self::DisabledForPrecompRasterInput => "layer_text_precomp_raster_input_p6_disabled",
        }
    }
}

fn rasterize_text_with_layout_for_policy(
    request: &TextLayoutRequest,
    layout: &TextLayoutResult,
    width: u32,
    height: u32,
    fill: [u8; 4],
    policy: TextP6PixelPolicy,
) -> anyhow::Result<(Canvas, TextRasterTrace)> {
    match policy {
        TextP6PixelPolicy::Default => {
            rasterize_text_with_layout(request, layout, width, height, fill)
        }
        TextP6PixelPolicy::DisabledForPrecompRasterInput => {
            rasterize_text_with_layout_p6_pixels_disabled(request, layout, width, height, fill)
        }
    }
}

fn render_layer_stub(
    scene: &Scene,
    comp: &Composition,
    layer: &Layer,
    time: f64,
    footage: &mut dyn FootageProvider,
    stack: &mut Vec<String>,
    text_p6_pixel_policy: TextP6PixelPolicy,
    mut trace: Option<&mut FrameRenderTrace>,
) -> anyhow::Result<Canvas> {
    let layer_started = Instant::now();
    let content_started = Instant::now();
    let layer_time = posterized_time_for_effects(effects_of(layer), time);
    record_layer_temporal_trace(trace.as_deref_mut(), comp, layer, time, layer_time);
    let mut layer_space_effects_ms = None;
    let mut canvas = match layer {
        Layer::Solid {
            id,
            color,
            rect,
            transform,
            start,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, layer_time, *start, *duration, comp.fps);
            record_transform_sampling_trace(
                trace.as_deref_mut(),
                &comp.id,
                id,
                transform,
                layer_time,
                *start,
                *duration,
                comp.fps,
            );
            let mut c = Canvas::transparent(canvas_dim(rect.w), canvas_dim(rect.h));
            for y in 0..c.height {
                for x in 0..c.width {
                    c.set_pixel(x, y, *color);
                }
            }
            transform_canvas(&c, comp.width, comp.height, &evaluated, [rect.x, rect.y])
        }
        Layer::Text {
            id,
            text,
            font,
            fontSize,
            fill,
            box_,
            transform,
            text_animators,
            start,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, layer_time, *start, *duration, comp.fps);
            record_transform_sampling_trace(
                trace.as_deref_mut(),
                &comp.id,
                id,
                transform,
                layer_time,
                *start,
                *duration,
                comp.fps,
            );
            let rect = box_.clone().unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                w: comp.width as f32,
                h: comp.height as f32,
            });
            let local_width = canvas_dim(rect.w);
            let local_height = canvas_dim(rect.h);
            let request = TextLayoutRequest {
                text: text.clone(),
                font_id: font.clone(),
                font_size: *fontSize,
                box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
            };
            let layout = layout_text(&request).ok();
            let (mut text_canvas, raster_trace) = if let Some(layout) = layout.as_ref() {
                let (canvas, raster_trace) = rasterize_text_with_layout_for_policy(
                    &request,
                    layout,
                    local_width,
                    local_height,
                    *fill,
                    text_p6_pixel_policy,
                )?;
                (canvas, Some(raster_trace))
            } else {
                (
                    rasterize_text(&request, local_width, local_height, *fill)?,
                    None,
                )
            };
            record_text_layout_trace(
                trace.as_deref_mut(),
                &comp.id,
                id,
                time,
                layer_time,
                &request,
                [local_width, local_height],
                1.0,
                transform_to_matrix(&evaluated).matrix(),
                [rect.x, rect.y],
                layout.as_ref(),
                raster_trace.as_ref(),
                text_p6_pixel_policy.layer_text_render_path(),
            );
            let reveal = evaluate_scalar_keyframes(&transform.animation.reveal, layer_time, 100.0);
            if text_animators.is_empty() {
                apply_horizontal_reveal(&mut text_canvas, reveal);
            } else {
                text_canvas = apply_text_animators(
                    text_canvas,
                    text,
                    layout.as_ref(),
                    text_animators,
                    layer_time,
                    *start,
                    1.0,
                    trace.as_deref_mut(),
                    &comp.id,
                    id,
                );
            }
            transform_canvas(
                &text_canvas,
                comp.width,
                comp.height,
                &evaluated,
                [rect.x, rect.y],
            )
        }
        Layer::Footage {
            id,
            start,
            source,
            source_start,
            transform,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, layer_time, *start, *duration, comp.fps);
            record_transform_sampling_trace(
                trace.as_deref_mut(),
                &comp.id,
                id,
                transform,
                layer_time,
                *start,
                *duration,
                comp.fps,
            );
            let source_time = (*source_start + (layer_time - *start)).max(0.0);
            match footage.frame_at(source, source_time)? {
                Some(frame) => {
                    if effects_are_turbulent_only(effects_of(layer)) {
                        let effects_started = Instant::now();
                        let effected = apply_effects_to_canvas(
                            effects_of(layer),
                            &frame,
                            layer_time,
                            comp.fps,
                            trace.as_deref_mut(),
                            &comp.id,
                            layer.id(),
                        )?;
                        layer_space_effects_ms = Some(elapsed_ms(effects_started));
                        transform_canvas(&effected, comp.width, comp.height, &evaluated, [0.0, 0.0])
                    } else {
                        transform_canvas(&frame, comp.width, comp.height, &evaluated, [0.0, 0.0])
                    }
                }
                None => checkerboard_canvas(comp.width, comp.height),
            }
        }
        Layer::Precomp {
            id,
            start,
            duration,
            composition,
            collapse_transformations,
            transform,
            ..
        } => {
            if *collapse_transformations && can_collapse_composition(scene, composition, stack) {
                render_collapsed_precomp(
                    scene,
                    comp,
                    layer,
                    layer_time,
                    footage,
                    stack,
                    trace.as_deref_mut(),
                )?
            } else {
                let node = scene
                    .compositions
                    .iter()
                    .find(|node| node.composition.id == *composition)
                    .ok_or_else(|| {
                        anyhow::anyhow!("precomp composition '{composition}' was not found")
                    })?;
                if stack.iter().any(|id| id == composition) {
                    anyhow::bail!(
                        "precomp cycle detected: {} -> {}",
                        stack.join(" -> "),
                        composition
                    );
                }
                stack.push(composition.clone());
                let source_time = (layer_time - *start).max(0.0);
                let precomp_canvas = render_composition_frame(
                    scene,
                    &node.composition,
                    &node.layers,
                    source_time,
                    footage,
                    stack,
                    TextP6PixelPolicy::DisabledForPrecompRasterInput,
                    trace.as_deref_mut(),
                )?;
                stack.pop();
                let evaluated =
                    evaluate_transform(transform, layer_time, *start, *duration, comp.fps);
                record_transform_sampling_trace(
                    trace.as_deref_mut(),
                    &comp.id,
                    id,
                    transform,
                    layer_time,
                    *start,
                    *duration,
                    comp.fps,
                );
                record_rasterized_precomp_trace(
                    trace.as_deref_mut(),
                    &comp.id,
                    id,
                    composition,
                    *collapse_transformations,
                    layer_time,
                    source_time,
                    transform_to_matrix(&evaluated).matrix(),
                    [node.composition.width, node.composition.height],
                );
                transform_canvas(
                    &precomp_canvas,
                    comp.width,
                    comp.height,
                    &evaluated,
                    [0.0, 0.0],
                )
            }
        }
        Layer::Adjustment { .. } => Canvas::transparent(comp.width, comp.height),
    };
    let content_ms = elapsed_ms(content_started);

    let effects_ms = if let Some(effects_ms) = layer_space_effects_ms {
        effects_ms
    } else {
        let effects_started = Instant::now();
        canvas = apply_effects_to_canvas(
            effects_of(layer),
            &canvas,
            layer_time,
            comp.fps,
            trace.as_deref_mut(),
            &comp.id,
            layer.id(),
        )?;
        elapsed_ms(effects_started)
    };
    if let Some(trace) = trace.as_deref_mut() {
        trace.layers.push(LayerTiming {
            composition: comp.id.clone(),
            layer_id: layer.id().to_string(),
            layer_type: layer_type(layer),
            content_ms,
            effects_ms,
            total_ms: elapsed_ms(layer_started),
        });
    }
    Ok(canvas)
}

fn render_collapsed_precomp(
    scene: &Scene,
    parent_comp: &Composition,
    layer: &Layer,
    time: f64,
    footage: &mut dyn FootageProvider,
    stack: &mut Vec<String>,
    mut trace: Option<&mut FrameRenderTrace>,
) -> anyhow::Result<Canvas> {
    let Layer::Precomp {
        id,
        start,
        duration,
        composition,
        transform,
        ..
    } = layer
    else {
        anyhow::bail!("render_collapsed_precomp called for non-precomp layer");
    };
    if stack.iter().any(|value| value == composition) {
        anyhow::bail!(
            "precomp cycle detected: {} -> {}",
            stack.join(" -> "),
            composition
        );
    }
    let node = composition_node(scene, composition)
        .ok_or_else(|| anyhow::anyhow!("precomp composition '{composition}' was not found"))?;
    let source_time = (time - *start).max(0.0);
    let evaluated = evaluate_transform(transform, time, *start, *duration, parent_comp.fps);
    let parent_matrix = transform_to_matrix(&evaluated).matrix();
    record_transform_sampling_trace(
        trace.as_deref_mut(),
        &parent_comp.id,
        id,
        transform,
        time,
        *start,
        *duration,
        parent_comp.fps,
    );
    record_collapsed_precomp_trace(
        trace.as_deref_mut(),
        &parent_comp.id,
        id,
        composition,
        time,
        source_time,
        parent_matrix,
        node.layers
            .iter()
            .map(|layer| layer.id().to_string())
            .collect(),
    );
    let bee_text_carrier_routes = bee_text_carrier_routes_for_precomp(scene, &parent_comp.id, id);
    record_bee_text_carrier_route_trace(trace.as_deref_mut(), bee_text_carrier_routes);
    let mut canvas = Canvas::transparent(parent_comp.width, parent_comp.height);

    stack.push(composition.clone());
    for child in node.layers.iter().rev() {
        if !child.is_active(source_time) {
            continue;
        }
        let child_canvas = render_layer_with_parent_matrix(
            scene,
            parent_comp,
            child,
            source_time,
            footage,
            stack,
            parent_matrix,
            Some(BeeTextCarrierRenderContext {
                parent_composition: &parent_comp.id,
                precomp_layer_id: id,
                target_composition: composition,
                source_time,
            }),
            trace.as_deref_mut(),
        )?;
        composite_normal(&mut canvas, &child_canvas, opacity_of(child, source_time));
    }
    stack.pop();

    Ok(canvas)
}

fn render_layer_with_parent_matrix(
    scene: &Scene,
    parent_comp: &Composition,
    layer: &Layer,
    time: f64,
    footage: &mut dyn FootageProvider,
    stack: &mut Vec<String>,
    parent_matrix: Mat3,
    bee_text_carrier_context: Option<BeeTextCarrierRenderContext<'_>>,
    mut trace: Option<&mut FrameRenderTrace>,
) -> anyhow::Result<Canvas> {
    match layer {
        Layer::Solid {
            id,
            color,
            rect,
            transform,
            start,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, time, *start, *duration, parent_comp.fps);
            record_transform_sampling_trace(
                trace.as_deref_mut(),
                &parent_comp.id,
                id,
                transform,
                time,
                *start,
                *duration,
                parent_comp.fps,
            );
            let mut c = Canvas::transparent(canvas_dim(rect.w), canvas_dim(rect.h));
            for y in 0..c.height {
                for x in 0..c.width {
                    c.set_pixel(x, y, *color);
                }
            }
            let matrix = parent_matrix.mul(transform_to_matrix(&evaluated).matrix());
            Ok(transform_canvas_with_matrix(
                &c,
                parent_comp.width,
                parent_comp.height,
                matrix,
                [rect.x, rect.y],
            ))
        }
        Layer::Text {
            id,
            text,
            font,
            fontSize,
            fill,
            box_,
            transform,
            text_animators,
            start,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, time, *start, *duration, parent_comp.fps);
            record_transform_sampling_trace(
                trace.as_deref_mut(),
                &parent_comp.id,
                id,
                transform,
                time,
                *start,
                *duration,
                parent_comp.fps,
            );
            let rect = box_.clone().unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                w: parent_comp.width as f32,
                h: parent_comp.height as f32,
            });
            let child_matrix = transform_to_matrix(&evaluated).matrix();
            let matrix = parent_matrix.mul(child_matrix);
            let reveal = evaluate_scalar_keyframes(&transform.animation.reveal, time, 100.0);
            if collapsed_text_vector_deferred_enabled()
                && text_animators.is_empty()
                && reveal >= 99.999
            {
                let raster_scale = collapsed_text_raster_scale(matrix);
                let local_width = canvas_dim(rect.w * raster_scale);
                let local_height = canvas_dim(rect.h * raster_scale);
                let request = TextLayoutRequest {
                    text: text.clone(),
                    font_id: font.clone(),
                    font_size: *fontSize * raster_scale,
                    box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
                };
                if let Ok(layout) = layout_text(&request) {
                    let render_matrix = if raster_scale > 1.0001 {
                        matrix.mul(Mat3::scale(Vec2::new(
                            1.0 / raster_scale,
                            1.0 / raster_scale,
                        )))
                    } else {
                        matrix
                    };
                    match rasterize_text_with_layout_vector_transform(
                        &request,
                        &layout,
                        local_width,
                        local_height,
                        *fill,
                        TextVectorTransform {
                            matrix: Mat3::identity(),
                            local_origin: [0.0, 0.0],
                            scale_carrier: "scale_baked_bee_text_carrier_local_canvas",
                            font_size_baked_scale: raster_scale > 1.0001,
                        },
                    ) {
                        Ok((text_canvas, raster_trace)) => {
                            let rendered = transform_canvas_with_matrix(
                                &text_canvas,
                                parent_comp.width,
                                parent_comp.height,
                                render_matrix,
                                [rect.x * raster_scale, rect.y * raster_scale],
                            );
                            record_text_layout_trace(
                                trace.as_deref_mut(),
                                &parent_comp.id,
                                id,
                                time,
                                time,
                                &request,
                                [local_width, local_height],
                                raster_scale,
                                render_matrix,
                                [rect.x * raster_scale, rect.y * raster_scale],
                                Some(&layout),
                                Some(&raster_trace),
                                "collapsed_text_vector_deferred",
                            );
                            record_collapsed_text_vector_trace(
                                trace.as_deref_mut(),
                                &parent_comp.id,
                                id,
                                time,
                                parent_matrix,
                                child_matrix,
                                matrix,
                                raster_scale,
                                [local_width, local_height],
                                Some(&text_canvas),
                                "deferred_text_vector_matrix_carrier",
                                None,
                            );
                            record_bee_text_carrier_routed_trace(
                                trace.as_deref_mut(),
                                bee_text_carrier_context,
                                &parent_comp.id,
                                id,
                                time,
                                parent_matrix,
                                child_matrix,
                                matrix,
                                [local_width, local_height],
                                &raster_trace,
                                &rendered,
                            );
                            return Ok(rendered);
                        }
                        Err(err) => {
                            record_collapsed_text_vector_trace(
                                trace.as_deref_mut(),
                                &parent_comp.id,
                                id,
                                time,
                                parent_matrix,
                                child_matrix,
                                matrix,
                                raster_scale,
                                [local_width, local_height],
                                None,
                                "fallback_to_intermediate_text_raster",
                                Some(&err.to_string()),
                            );
                        }
                    }
                }
            }
            let raster_scale = collapsed_text_raster_scale(matrix);
            let local_width = canvas_dim(rect.w * raster_scale);
            let local_height = canvas_dim(rect.h * raster_scale);
            let request = TextLayoutRequest {
                text: text.clone(),
                font_id: font.clone(),
                font_size: *fontSize * raster_scale,
                box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
            };
            let layout = layout_text(&request).ok();
            let (mut text_canvas, raster_trace) = if let Some(layout) = layout.as_ref() {
                let (canvas, raster_trace) = rasterize_text_with_layout_p6_pixels_disabled(
                    &request,
                    layout,
                    local_width,
                    local_height,
                    *fill,
                )?;
                (canvas, Some(raster_trace))
            } else {
                (
                    rasterize_text(&request, local_width, local_height, *fill)?,
                    None,
                )
            };
            record_text_layout_trace(
                trace.as_deref_mut(),
                &parent_comp.id,
                id,
                time,
                time,
                &request,
                [local_width, local_height],
                raster_scale,
                if raster_scale > 1.0001 {
                    matrix.mul(Mat3::scale(Vec2::new(
                        1.0 / raster_scale,
                        1.0 / raster_scale,
                    )))
                } else {
                    matrix
                },
                [rect.x * raster_scale, rect.y * raster_scale],
                layout.as_ref(),
                raster_trace.as_ref(),
                "collapsed_text",
            );
            if text_animators.is_empty() {
                apply_horizontal_reveal(&mut text_canvas, reveal);
            } else {
                text_canvas = apply_text_animators(
                    text_canvas,
                    text,
                    layout.as_ref(),
                    text_animators,
                    time,
                    *start,
                    raster_scale,
                    trace.as_deref_mut(),
                    &parent_comp.id,
                    id,
                );
            }
            record_collapsed_text_raster_trace(
                trace.as_deref_mut(),
                &parent_comp.id,
                id,
                time,
                parent_matrix,
                child_matrix,
                matrix,
                raster_scale,
                [local_width, local_height],
                &text_canvas,
            );
            let render_matrix = if raster_scale > 1.0001 {
                matrix.mul(Mat3::scale(Vec2::new(
                    1.0 / raster_scale,
                    1.0 / raster_scale,
                )))
            } else {
                matrix
            };
            Ok(transform_canvas_with_matrix(
                &text_canvas,
                parent_comp.width,
                parent_comp.height,
                render_matrix,
                [rect.x * raster_scale, rect.y * raster_scale],
            ))
        }
        Layer::Precomp {
            id,
            start,
            duration,
            composition,
            collapse_transformations,
            transform,
            ..
        } if *collapse_transformations && can_collapse_composition(scene, composition, stack) => {
            if stack.iter().any(|value| value == composition) {
                anyhow::bail!(
                    "precomp cycle detected: {} -> {}",
                    stack.join(" -> "),
                    composition
                );
            }
            let node = composition_node(scene, composition).ok_or_else(|| {
                anyhow::anyhow!("precomp composition '{composition}' was not found")
            })?;
            let source_time = (time - *start).max(0.0);
            let evaluated = evaluate_transform(transform, time, *start, *duration, parent_comp.fps);
            record_transform_sampling_trace(
                trace.as_deref_mut(),
                &parent_comp.id,
                id,
                transform,
                time,
                *start,
                *duration,
                parent_comp.fps,
            );
            let child_matrix = transform_to_matrix(&evaluated).matrix();
            let matrix = parent_matrix.mul(child_matrix);
            record_collapsed_precomp_trace(
                trace.as_deref_mut(),
                &parent_comp.id,
                id,
                composition,
                time,
                source_time,
                matrix,
                node.layers
                    .iter()
                    .map(|layer| layer.id().to_string())
                    .collect(),
            );
            let bee_text_carrier_routes =
                bee_text_carrier_routes_for_precomp(scene, &parent_comp.id, id);
            record_bee_text_carrier_route_trace(trace.as_deref_mut(), bee_text_carrier_routes);
            let mut canvas = Canvas::transparent(parent_comp.width, parent_comp.height);
            stack.push(composition.clone());
            for child in node.layers.iter().rev() {
                if !child.is_active(source_time) {
                    continue;
                }
                let child_canvas = render_layer_with_parent_matrix(
                    scene,
                    parent_comp,
                    child,
                    source_time,
                    footage,
                    stack,
                    matrix,
                    bee_text_carrier_context,
                    trace.as_deref_mut(),
                )?;
                composite_normal(&mut canvas, &child_canvas, opacity_of(child, source_time));
            }
            stack.pop();
            Ok(canvas)
        }
        _ => render_layer_stub(
            scene,
            parent_comp,
            layer,
            time,
            footage,
            stack,
            TextP6PixelPolicy::DisabledForPrecompRasterInput,
            trace.as_deref_mut(),
        ),
    }
}

fn can_collapse_composition(scene: &Scene, composition: &str, stack: &[String]) -> bool {
    if stack.iter().any(|value| value == composition) {
        return false;
    }
    let Some(node) = composition_node(scene, composition) else {
        return false;
    };
    let mut nested_stack = stack.to_vec();
    nested_stack.push(composition.to_string());
    node.layers.iter().all(|layer| {
        if !effects_of(layer).is_empty() {
            return false;
        }
        match layer {
            Layer::Solid { .. } | Layer::Text { .. } => true,
            Layer::Precomp {
                composition,
                collapse_transformations,
                ..
            } => {
                *collapse_transformations
                    && can_collapse_composition(scene, composition, &nested_stack)
            }
            Layer::Footage { .. } | Layer::Adjustment { .. } => false,
        }
    })
}

fn composition_node<'a>(
    scene: &'a Scene,
    composition: &str,
) -> Option<&'a render_ir::CompositionNode> {
    scene
        .compositions
        .iter()
        .find(|node| node.composition.id == composition)
}

fn apply_effects_to_canvas(
    effects: &[EffectSpec],
    input: &Canvas,
    time: f64,
    fps: f64,
    mut trace: Option<&mut FrameRenderTrace>,
    composition: &str,
    layer_id: &str,
) -> anyhow::Result<Canvas> {
    let mut canvas = input.clone();
    for (effect_index, spec) in effects.iter().enumerate() {
        if let Some(effect) = EffectRegistry::create(&spec.match_name) {
            let debug_trace = trace.as_ref().and_then(|_| {
                effect_debug_trace_json(&spec.match_name, &canvas, &spec.params, time)
            });
            let started = Instant::now();
            canvas = effect.render(&canvas, &EffectContext { time, fps }, &spec.params)?;
            if let Some(trace) = trace.as_deref_mut() {
                if let Some(debug_trace) = debug_trace {
                    trace.effect_debug.push(EffectDebugRecord {
                        composition: composition.to_string(),
                        layer_id: layer_id.to_string(),
                        effect_index,
                        match_name: spec.match_name.clone(),
                        application: "layer",
                        effect_time: time,
                        trace: debug_trace,
                    });
                }
                trace.effects.push(EffectTiming {
                    composition: composition.to_string(),
                    layer_id: layer_id.to_string(),
                    match_name: spec.match_name.clone(),
                    elapsed_ms: elapsed_ms(started),
                });
            }
        } else {
            anyhow::bail!("unknown effect matchName: {}", spec.match_name);
        }
    }
    Ok(canvas)
}

fn apply_adjustment_effects_to_canvas(
    effects: &[EffectSpec],
    input: &Canvas,
    routing: AdjustmentTimeRouting,
    fps: f64,
    mut trace: Option<&mut FrameRenderTrace>,
    composition: &str,
    layer_id: &str,
) -> anyhow::Result<Canvas> {
    let mut canvas = input.clone();
    for (effect_index, spec) in effects.iter().enumerate() {
        let Some(effect) = EffectRegistry::create(&spec.match_name) else {
            anyhow::bail!("unknown effect matchName: {}", spec.match_name);
        };

        let param_time = adjustment_effect_param_time(effect_index, routing);
        let params = adjustment_effect_params(&spec.match_name, &spec.params);
        let crop_bucketed_adjustment_input = routing.posterize.is_some_and(|posterize| {
            effect_index > posterize.effect_index
                && time_changed(routing.lower_stack_time, routing.comp_time)
        });
        let input_plan = adjustment_effect_input_plan(
            &spec.match_name,
            &canvas,
            &params,
            param_time,
            crop_bucketed_adjustment_input,
        );
        let input_hash = trace.as_ref().map(|_| canvas_hash(&canvas));
        let debug_trace = trace.as_ref().and_then(|_| {
            effect_debug_trace_json(&spec.match_name, input_plan.input(), &params, param_time)
        });
        let started = Instant::now();
        let rendered = effect.render(
            input_plan.input(),
            &EffectContext {
                time: param_time,
                fps,
            },
            &params,
        )?;
        canvas = input_plan.place_output(rendered);
        let elapsed_ms = elapsed_ms(started);
        let output_hash = trace.as_ref().map(|_| canvas_hash(&canvas));

        if let Some(trace) = trace.as_deref_mut() {
            if let Some(debug_trace) = debug_trace {
                trace.effect_debug.push(EffectDebugRecord {
                    composition: composition.to_string(),
                    layer_id: layer_id.to_string(),
                    effect_index,
                    match_name: spec.match_name.clone(),
                    application: "adjustment",
                    effect_time: param_time,
                    trace: debug_trace,
                });
            }
            trace.effects.push(EffectTiming {
                composition: composition.to_string(),
                layer_id: layer_id.to_string(),
                match_name: spec.match_name.clone(),
                elapsed_ms,
            });
            trace.adjustment_effects.push(AdjustmentEffectTrace {
                composition: composition.to_string(),
                layer_id: layer_id.to_string(),
                effect_index,
                match_name: spec.match_name.clone(),
                comp_time: routing.comp_time,
                layer_time: routing.layer_time,
                lower_stack_time: routing.lower_stack_time,
                param_time,
                input_hash: input_hash.unwrap_or_else(|| canvas_hash(input)),
                output_hash: output_hash.unwrap_or_else(|| canvas_hash(&canvas)),
                posterize: routing.posterize.and_then(|posterize| {
                    (posterize.effect_index == effect_index).then_some(posterize.posterize)
                }),
            });
        }
    }
    Ok(canvas)
}

enum AdjustmentEffectInputPlan<'a> {
    Full {
        input: &'a Canvas,
    },
    Cropped {
        input: Canvas,
        origin: (u32, u32),
        full_size: (u32, u32),
    },
}

impl<'a> AdjustmentEffectInputPlan<'a> {
    fn input(&self) -> &Canvas {
        match self {
            AdjustmentEffectInputPlan::Full { input } => input,
            AdjustmentEffectInputPlan::Cropped { input, .. } => input,
        }
    }

    fn place_output(self, rendered: Canvas) -> Canvas {
        match self {
            AdjustmentEffectInputPlan::Full { .. } => rendered,
            AdjustmentEffectInputPlan::Cropped {
                origin, full_size, ..
            } => paste_canvas_at(&rendered, full_size.0, full_size.1, origin.0, origin.1),
        }
    }
}

fn adjustment_effect_input_plan<'a>(
    match_name: &str,
    canvas: &'a Canvas,
    params: &Value,
    time: f64,
    crop_bucketed_adjustment_input: bool,
) -> AdjustmentEffectInputPlan<'a> {
    if match_name != "ADBE Turbulent Displace" {
        return AdjustmentEffectInputPlan::Full { input: canvas };
    }
    if !crop_bucketed_adjustment_input {
        return AdjustmentEffectInputPlan::Full { input: canvas };
    }
    let Some((min_x, min_y, max_x, max_y)) = alpha_bounds_exclusive(canvas) else {
        return AdjustmentEffectInputPlan::Full { input: canvas };
    };
    let grow = effects::turbulent_displace::turbulent_displace_extent_grow_pixels(params, time);
    if grow == 0 && min_x == 0 && min_y == 0 && max_x == canvas.width && max_y == canvas.height {
        return AdjustmentEffectInputPlan::Full { input: canvas };
    }
    let x0 = min_x.saturating_sub(grow);
    let y0 = min_y.saturating_sub(grow);
    let x1 = max_x.saturating_add(grow).min(canvas.width);
    let y1 = max_y.saturating_add(grow).min(canvas.height);
    if x0 == 0 && y0 == 0 && x1 == canvas.width && y1 == canvas.height {
        return AdjustmentEffectInputPlan::Full { input: canvas };
    }
    AdjustmentEffectInputPlan::Cropped {
        input: crop_canvas(canvas, x0, y0, x1, y1),
        origin: (x0, y0),
        full_size: (canvas.width, canvas.height),
    }
}

fn adjustment_effect_params(match_name: &str, params: &Value) -> Value {
    if match_name != "ADBE Geometry2" {
        return params.clone();
    }
    let mut adjusted = params.clone();
    if let Some(object) = adjusted.as_object_mut() {
        object.insert("__native_layer_space_origin".to_string(), json!([0.0, 0.0]));
        adjusted
    } else {
        json!({ "__native_layer_space_origin": [0.0, 0.0] })
    }
}

fn alpha_bounds_exclusive(canvas: &Canvas) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = canvas.width;
    let mut min_y = canvas.height;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            if canvas.pixel(x, y)[3] == 0 {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + 1);
            max_y = max_y.max(y + 1);
        }
    }
    (min_x < max_x && min_y < max_y).then_some((min_x, min_y, max_x, max_y))
}

fn crop_canvas(canvas: &Canvas, x0: u32, y0: u32, x1: u32, y1: u32) -> Canvas {
    let width = x1.saturating_sub(x0);
    let height = y1.saturating_sub(y0);
    let mut cropped = Canvas::transparent(width, height);
    for y in 0..height {
        for x in 0..width {
            cropped.set_pixel(x, y, canvas.pixel(x0 + x, y0 + y));
        }
    }
    cropped
}

fn paste_canvas_at(src: &Canvas, width: u32, height: u32, x0: u32, y0: u32) -> Canvas {
    let mut output = Canvas::transparent(width, height);
    let copy_width = src.width.min(width.saturating_sub(x0));
    let copy_height = src.height.min(height.saturating_sub(y0));
    for y in 0..copy_height {
        for x in 0..copy_width {
            output.set_pixel(x0 + x, y0 + y, src.pixel(x, y));
        }
    }
    output
}

fn adjustment_effect_param_time(effect_index: usize, routing: AdjustmentTimeRouting) -> f64 {
    if let Some(posterize) = routing.posterize {
        if effect_index <= posterize.effect_index {
            return posterize.posterize.bucket_time;
        }
    }
    routing.comp_time
}

fn canvas_hash(canvas: &Canvas) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in canvas
        .width
        .to_le_bytes()
        .into_iter()
        .chain(canvas.height.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in &canvas.data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn effect_debug_trace_json(
    match_name: &str,
    input: &Canvas,
    params: &Value,
    time: f64,
) -> Option<Value> {
    match match_name {
        "ADBE Box Blur2" => {
            let trace = effects::box_blur::box_blur_debug_trace(input, params, time);
            Some(json!({
                "params": {
                    "radius": trace.params.radius,
                    "iterations": trace.params.iterations,
                    "iterations_applied": trace.params.iterations_applied,
                    "edge_policy": trace.params.edge_policy,
                    "kernel_radius": trace.params.kernel_radius
                },
                "alpha_policy": {
                    "canvas_storage": "straight_rgba8",
                    "blur_input": "straight_rgba8_all_channels",
                    "blur_kernel": "separable_box_average",
                    "edge_policy": trace.params.edge_policy,
                    "gf_src_alpha_option": "standard_options_unresolved_m19",
                    "gf_dest_alpha_option": "standard_options_unresolved_m19",
                    "premult_unpremultiply_applied": false,
                    "diagnostic_only": true
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "horizontal_pass_rgba": debug_hash_hex(trace.hashes.horizontal_pass_rgba),
                    "first_iteration_rgba": debug_hash_hex(trace.hashes.first_iteration_rgba),
                    "output_rgba": debug_hash_hex(trace.hashes.output_rgba)
                },
                "alpha_stats": {
                    "input": alpha_stats_json(trace.alpha.input),
                    "horizontal_pass": alpha_stats_json(trace.alpha.horizontal_pass),
                    "first_iteration": alpha_stats_json(trace.alpha.first_iteration),
                    "output": alpha_stats_json(trace.alpha.output)
                }
            }))
        }
        "ADBE Drop Shadow" => {
            let trace = effects::drop_shadow::drop_shadow_debug_trace(input, params);
            Some(json!({
                "params": {
                    "color": trace.params.color,
                    "opacity": trace.params.opacity,
                    "opacity_normalized": trace.params.opacity_normalized,
                    "direction_degrees": trace.params.direction_degrees,
                    "distance": trace.params.distance,
                    "softness": trace.params.softness,
                    "shadow_only": trace.params.shadow_only,
                    "dx": trace.params.dx,
                    "dy": trace.params.dy,
                    "blur_radius": trace.params.blur_radius,
                    "blur_iterations": trace.params.blur_iterations
                },
                "alpha_policy": {
                    "canvas_storage": "straight_rgba8",
                    "source_alpha": "input_alpha_channel",
                    "shadow_mask": "offset_alpha_scaled_by_ae_raw_opacity_0_255",
                    "shadow_blur_alpha_option": "gf_set_blur_alpha_channel_only_confirmed",
                    "softness": "alpha_channel_only_box_blur_then_recolor",
                    "composite": "straight_rgba8_normal_source_over",
                    "final_composite_policy": "native_approximation_pending_m19",
                    "premult_unpremultiply_applied": false,
                    "diagnostic_only": true
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "source_alpha_rgba": debug_hash_hex(trace.hashes.source_alpha_rgba),
                    "raw_offset_shadow_rgba": debug_hash_hex(trace.hashes.raw_offset_shadow_rgba),
                    "blurred_shadow_rgba": debug_hash_hex(trace.hashes.blurred_shadow_rgba),
                    "final_rgba": debug_hash_hex(trace.hashes.final_rgba)
                },
                "alpha_stats": {
                    "input": alpha_stats_json(trace.alpha.input),
                    "source_alpha": alpha_stats_json(trace.alpha.source_alpha),
                    "raw_offset_shadow": alpha_stats_json(trace.alpha.raw_offset_shadow),
                    "blurred_shadow": alpha_stats_json(trace.alpha.blurred_shadow),
                    "final": alpha_stats_json(trace.alpha.final_output)
                }
            }))
        }
        "ADBE Glo2" => {
            let trace = effects::glow::glow_debug_trace(input, params, time);
            Some(json!({
                "params": {
                    "based_on": trace.params.based_on,
                    "based_on_param_source": trace.params.based_on_param_source,
                    "based_on_raw_number": trace.params.based_on_raw_number,
                    "threshold": trace.params.threshold,
                    "radius": trace.params.radius,
                    "ir_gaussian_radius": trace.params.ir_gaussian_radius,
                    "intensity": trace.params.intensity,
                    "kernel_radius": trace.params.kernel_radius
                },
                "alpha_policy": {
                    "canvas_storage": "straight_rgba8",
                    "threshold_source": trace.params.based_on,
                    "based_on_param_source": trace.params.based_on_param_source,
                    "based_on_raw_number": trace.params.based_on_raw_number,
                    "blur_input": "thresholded_straight_rgba8",
                    "blur_kernel": "ae_ir_gaussian_radius_scale_0_4_then_native_box_approximation",
                    "intensity_scale": "straight_rgba8_channels",
                    "composite": "straight_rgba8_normal_source_over",
                    "composite_policy": "native_approximation_pending_ir_composite_and_m19",
                    "premult_unpremultiply_applied": false,
                    "diagnostic_only": true
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "threshold_source_rgba": debug_hash_hex(trace.hashes.threshold_source_rgba),
                    "blurred_glow_rgba": debug_hash_hex(trace.hashes.blurred_glow_rgba),
                    "intensity_scaled_glow_rgba": debug_hash_hex(
                        trace.hashes.intensity_scaled_glow_rgba
                    ),
                    "final_rgba": debug_hash_hex(trace.hashes.final_rgba)
                },
                "alpha_stats": {
                    "input": alpha_stats_json(trace.alpha.input),
                    "threshold_source": alpha_stats_json(trace.alpha.threshold_source),
                    "blurred_glow": alpha_stats_json(trace.alpha.blurred_glow),
                    "intensity_scaled_glow": alpha_stats_json(trace.alpha.intensity_scaled_glow),
                    "final": alpha_stats_json(trace.alpha.final_output)
                }
            }))
        }
        "ADBE Minimax" => {
            let trace = effects::minimax::minimax_debug_trace(input, params, time);
            Some(json!({
                "params": {
                    "operation": trace.params.operation,
                    "channels": trace.params.channels,
                    "direction": trace.params.direction,
                    "radius": trace.params.radius,
                    "kernel_radius": trace.params.kernel_radius,
                    "dont_shrink_edges": trace.params.dont_shrink_edges,
                    "stage_count": trace.params.stage_count,
                    "edge_policy": trace.params.edge_policy
                },
                "alpha_policy": {
                    "canvas_storage": "straight_rgba8",
                    "selected_channels": trace.params.channels,
                    "unselected_channels": "preserved_from_stage_input",
                    "edge_policy": trace.params.edge_policy,
                    "dont_shrink_edges_parsed": trace.params.dont_shrink_edges,
                    "dont_shrink_edges_applied": true,
                    "premult_unpremultiply_applied": false,
                    "diagnostic_only": true
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "first_pass_rgba": debug_hash_hex(trace.hashes.first_pass_rgba),
                    "first_stage_rgba": debug_hash_hex(trace.hashes.first_stage_rgba),
                    "second_stage_rgba": debug_hash_hex(trace.hashes.second_stage_rgba),
                    "output_rgba": debug_hash_hex(trace.hashes.output_rgba)
                },
                "alpha_stats": {
                    "input": alpha_stats_json(trace.alpha.input),
                    "first_pass": alpha_stats_json(trace.alpha.first_pass),
                    "first_stage": alpha_stats_json(trace.alpha.first_stage),
                    "second_stage": alpha_stats_json(trace.alpha.second_stage),
                    "output": alpha_stats_json(trace.alpha.output)
                }
            }))
        }
        "ADBE Geometry2" => {
            let debug = effects::geometry::geometry2_debug_data(input, params, time);
            Some(json!({
                "schema": "ae-native-renderer.geometry2-debug.v1",
                "raw_params": debug.raw_params,
                "property_mapping": geometry2_property_mapping_json(&debug.property_mapping),
                "resolved": {
                    "anchor": debug.resolved.anchor,
                    "position": debug.resolved.position,
                    "scale": debug.resolved.scale,
                    "rotation": debug.resolved.rotation,
                    "skew": debug.resolved.skew,
                    "skew_axis": debug.resolved.skew_axis,
                    "pixel_aspect": debug.resolved.pixel_aspect,
                    "sampling": debug.resolved.sampling
                },
                "forward_matrix": debug.forward_matrix,
                "inverse_matrix": debug.inverse_matrix,
                "samples": debug.samples.iter().map(|sample| json!({
                    "output_xy": sample.output_xy,
                    "source_uv": sample.source_uv,
                    "sample_xy": sample.sample_xy,
                    "sample_rgba": sample.sample_rgba,
                    "out_of_bounds": sample.out_of_bounds
                })).collect::<Vec<_>>(),
                "sampler_mode": debug.sampler_mode,
                "edge_policy": debug.edge_policy,
                "out_of_bounds_count": debug.out_of_bounds_count
            }))
        }
        "ADBE Turbulent Displace" => {
            let debug = effects::turbulent_displace::turbulent_displace_field_telemetry(
                input, params, time,
            );
            Some(json!({
                "schema": "ae-native-renderer.turbulent-displace-field.v1",
                "input": {
                    "width": input.width,
                    "height": input.height,
                    "alpha_bounds_exclusive": alpha_bounds_exclusive(input)
                        .map(|(x0, y0, x1, y1)| json!([x0, y0, x1, y1]))
                },
                "raw_params": debug.raw_params,
                "resolved": {
                    "displacement_type": debug.resolved.displacement_type,
                    "amount": debug.resolved.amount,
                    "size": debug.resolved.size,
                    "offset": debug.resolved.offset,
                    "complexity": debug.resolved.complexity,
                    "evolution": debug.resolved.evolution,
                    "cycle_evolution": debug.resolved.cycle_evolution,
                    "cycle_revolutions": debug.resolved.cycle_revolutions,
                    "random_seed": debug.resolved.random_seed,
                    "antialiasing_best_quality": debug.resolved.antialiasing_best_quality,
                    "pinning": debug.resolved.pinning,
                    "resize_layer": debug.resolved.resize_layer,
                    "amplitude": debug.resolved.amplitude,
                    "phase_radians": debug.resolved.phase_radians
                },
                "ae_wrapper": {
                    "state_block_bytes": debug.ae_wrapper.state_block_bytes,
                    "gpu_param_block_bytes": debug.ae_wrapper.gpu_param_block_bytes,
                    "noise_table_rows": debug.ae_wrapper.noise_table_rows,
                    "inferred_internal_displacement_mode": debug.ae_wrapper.inferred_internal_displacement_mode,
                    "kernel_path": debug.ae_wrapper.kernel_path,
                    "uses_h_lookup": debug.ae_wrapper.uses_h_lookup,
                    "uses_v_lookup": debug.ae_wrapper.uses_v_lookup,
                    "h_lookup_len": debug.ae_wrapper.h_lookup_len,
                    "v_lookup_len": debug.ae_wrapper.v_lookup_len,
                    "amount_fixed16": debug.ae_wrapper.amount_fixed16,
                    "size_fixed16": debug.ae_wrapper.size_fixed16,
                    "offset_fixed16": debug.ae_wrapper.offset_fixed16,
                    "evolution_fixed16": debug.ae_wrapper.evolution_fixed16,
                    "cycle_evolution": debug.ae_wrapper.cycle_evolution,
                    "cycle_revolutions_fixed16": debug.ae_wrapper.cycle_revolutions_fixed16,
                    "random_seed_fixed16": debug.ae_wrapper.random_seed_fixed16,
                    "antialiasing_best_quality": debug.ae_wrapper.antialiasing_best_quality,
                    "complexity_octaves": debug.ae_wrapper.complexity_octaves,
                    "complexity_fraction": debug.ae_wrapper.complexity_fraction
                },
                "field_state": {
                    "model": debug.field_state.model,
                    "coordinate_space": debug.field_state.coordinate_space,
                    "dispatch_path": debug.field_state.dispatch_path,
                    "complexity_octaves": debug.field_state.complexity_octaves,
                    "complexity_fraction": debug.field_state.complexity_fraction,
                    "evolution_degrees": debug.field_state.evolution_degrees,
                    "cycle_evolution": debug.field_state.cycle_evolution,
                    "cycle_revolutions": debug.field_state.cycle_revolutions,
                    "antialiasing_best_quality": debug.field_state.antialiasing_best_quality,
                    "phase_radians": debug.field_state.phase_radians,
                    "amplitude": debug.field_state.amplitude,
                    "source_uv_convention": debug.field_state.source_uv_convention,
                    "hash_coverage": debug.field_state.hash_coverage,
                    "property_mapping_status": debug.field_state.property_mapping_status,
                    "tuning_guardrail": debug.field_state.tuning_guardrail
                },
                "samples": debug.samples.iter().map(|sample| json!({
                    "output_xy": sample.output_xy,
                    "noise": sample.noise,
                    "displacement": sample.displacement,
                    "source_uv": sample.source_uv,
                    "sample_xy": sample.sample_xy,
                    "out_of_bounds": sample.out_of_bounds
                })).collect::<Vec<_>>(),
                "field_hash": format!("{:016x}", debug.field_hash),
                "field_hash_u64": debug.field_hash,
                "sampler_mode": debug.sampler_mode,
                "edge_policy": debug.edge_policy,
                "out_of_bounds_count": debug.out_of_bounds_count
            }))
        }
        _ => None,
    }
}

fn geometry2_property_mapping_json(mapping: &effects::geometry::Geometry2PropertyMapping) -> Value {
    json!({
        "0003": geometry2_property_mapping_entry_json(&mapping.payload_0003),
        "0004": geometry2_property_mapping_entry_json(&mapping.payload_0004),
        "0005": geometry2_property_mapping_entry_json(&mapping.payload_0005),
        "0008": geometry2_property_mapping_entry_json(&mapping.payload_0008),
        "0009": geometry2_property_mapping_entry_json(&mapping.payload_0009),
        "0012": geometry2_property_mapping_entry_json(&mapping.payload_0012)
    })
}

fn geometry2_property_mapping_entry_json(
    entry: &effects::geometry::Geometry2PropertyMappingEntry,
) -> Value {
    json!({
        "payload_key": entry.payload_key,
        "match_name": entry.match_name,
        "ui_label": entry.ui_label,
        "native_role": entry.native_role,
        "present": entry.present,
        "raw_value": entry.raw_value.clone(),
        "note": entry.note
    })
}

fn debug_hash_hex(hash: u64) -> String {
    format!("0x{hash:016x}")
}

fn alpha_stats_json(stats: effects::box_blur::CanvasAlphaStats) -> Value {
    let total = stats.total_pixels.max(1) as f64;
    json!({
        "total_pixels": stats.total_pixels,
        "nonzero_pixels": stats.nonzero_pixels,
        "full_pixels": stats.full_pixels,
        "coverage_ratio": stats.nonzero_pixels as f64 / total,
        "full_coverage_ratio": stats.full_pixels as f64 / total,
        "alpha_sum": stats.alpha_sum,
        "alpha_mean": stats.alpha_sum as f64 / total,
        "alpha_min_nonzero": stats.alpha_min_nonzero,
        "alpha_max": stats.alpha_max
    })
}

fn layer_type(layer: &Layer) -> &'static str {
    match layer {
        Layer::Solid { .. } => "solid",
        Layer::Footage { .. } => "footage",
        Layer::Text { .. } => "text",
        Layer::Precomp { .. } => "precomp",
        Layer::Adjustment { .. } => "adjustment",
    }
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn checkerboard_canvas(width: u32, height: u32) -> Canvas {
    let mut c = Canvas::transparent(width, height);
    for y in 0..height {
        for x in 0..width {
            if (x / 32 + y / 32) % 2 == 0 {
                c.set_pixel(x, y, [30, 30, 36, 255]);
            }
        }
    }
    c
}

fn transform_canvas(
    src: &Canvas,
    width: u32,
    height: u32,
    transform: &render_ir::Transform2D,
    local_origin: [f32; 2],
) -> Canvas {
    transform_canvas_with_matrix(
        src,
        width,
        height,
        transform_to_matrix(transform).matrix(),
        local_origin,
    )
}

fn transform_canvas_with_matrix(
    src: &Canvas,
    width: u32,
    height: u32,
    matrix: Mat3,
    local_origin: [f32; 2],
) -> Canvas {
    let Some(inverse) = matrix.inverse() else {
        return Canvas::transparent(width, height);
    };

    let mut dst = Canvas::transparent(width, height);
    let Some((x0, y0, x1, y1)) = transformed_bounds(src, width, height, matrix, local_origin)
    else {
        return dst;
    };
    for y in y0..y1 {
        for x in x0..x1 {
            let local = inverse.transform_point(Vec2::new(x as f32, y as f32));
            let sample_x = local.x - local_origin[0];
            let sample_y = local.y - local_origin[1];
            let sx = sample_x.round() as i32;
            let sy = sample_y.round() as i32;
            if sx < 0 || sy < 0 || sx >= src.width as i32 || sy >= src.height as i32 {
                continue;
            }
            let pixel = BilinearSampler.sample(src, sample_x, sample_y);
            if pixel[3] > 0 {
                dst.set_pixel(x, y, pixel);
            }
        }
    }
    dst
}

fn transformed_bounds(
    src: &Canvas,
    width: u32,
    height: u32,
    matrix: Mat3,
    local_origin: [f32; 2],
) -> Option<(u32, u32, u32, u32)> {
    if src.width == 0 || src.height == 0 || width == 0 || height == 0 {
        return None;
    }
    let left = local_origin[0];
    let top = local_origin[1];
    let right = local_origin[0] + src.width as f32;
    let bottom = local_origin[1] + src.height as f32;
    let corners = [
        matrix.transform_point(Vec2::new(left, top)),
        matrix.transform_point(Vec2::new(right, top)),
        matrix.transform_point(Vec2::new(right, bottom)),
        matrix.transform_point(Vec2::new(left, bottom)),
    ];
    let min_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min)
        .floor()
        - 1.0;
    let min_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min)
        .floor()
        - 1.0;
    let max_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        + 1.0;
    let max_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        + 1.0;
    let x0 = min_x.max(0.0).min(width as f32) as u32;
    let y0 = min_y.max(0.0).min(height as f32) as u32;
    let x1 = max_x.max(0.0).min(width as f32) as u32;
    let y1 = max_y.max(0.0).min(height as f32) as u32;
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

fn matrix_scale_hint(matrix: Mat3) -> f32 {
    let origin = matrix.transform_point(Vec2::new(0.0, 0.0));
    let x = matrix.transform_point(Vec2::new(1.0, 0.0));
    let y = matrix.transform_point(Vec2::new(0.0, 1.0));
    let sx = ((x.x - origin.x).powi(2) + (x.y - origin.y).powi(2)).sqrt();
    let sy = ((y.x - origin.x).powi(2) + (y.y - origin.y).powi(2)).sqrt();
    sx.max(sy).max(1.0)
}

const COLLAPSED_TEXT_VECTOR_DEFERRED_ENV_VAR: &str =
    "AE_NATIVE_RENDERER_P5_COLLAPSED_TEXT_VECTOR_DEFERRED";

fn collapsed_text_vector_deferred_enabled() -> bool {
    std::env::var(COLLAPSED_TEXT_VECTOR_DEFERRED_ENV_VAR)
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes" | "enabled" | "vector" | "vector_deferred"
            )
        })
}

fn collapsed_text_raster_scale(matrix: Mat3) -> f32 {
    matrix_scale_hint(matrix).clamp(1.0, 4.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MatrixReport {
    translation: [f32; 2],
    scale: [f32; 2],
    scale_max: f32,
    determinant: f32,
    rotation_degrees: f32,
    axis_dot: f32,
    affine_2d: bool,
}

fn matrix_report(matrix: Mat3) -> MatrixReport {
    let m = matrix.m;
    let x_axis = [m[0][0], m[1][0]];
    let y_axis = [m[0][1], m[1][1]];
    let sx = (x_axis[0].powi(2) + x_axis[1].powi(2)).sqrt();
    let sy = (y_axis[0].powi(2) + y_axis[1].powi(2)).sqrt();
    let determinant = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    MatrixReport {
        translation: [clean_float(m[0][2]), clean_float(m[1][2])],
        scale: [clean_float(sx), clean_float(sy)],
        scale_max: clean_float(sx.max(sy).max(1.0)),
        determinant: clean_float(determinant),
        rotation_degrees: clean_float(x_axis[1].atan2(x_axis[0]).to_degrees()),
        axis_dot: clean_float(x_axis[0] * y_axis[0] + x_axis[1] * y_axis[1]),
        affine_2d: m[2][0].abs() <= 1.0e-6
            && m[2][1].abs() <= 1.0e-6
            && (m[2][2] - 1.0).abs() <= 1.0e-6,
    }
}

fn matrix_report_json(matrix: Mat3) -> Value {
    let report = matrix_report(matrix);
    json!({
        "translation": report.translation,
        "scale": report.scale,
        "scale_max": report.scale_max,
        "determinant": report.determinant,
        "rotation_degrees": report.rotation_degrees,
        "axis_dot": report.axis_dot,
        "affine_2d": report.affine_2d
    })
}

fn clean_float(value: f32) -> f32 {
    if value.abs() <= 1.0e-6 {
        0.0
    } else {
        value
    }
}

fn transform_to_matrix(transform: &render_ir::Transform2D) -> Transform2D {
    Transform2D {
        anchor: Vec2::from(transform.anchor),
        position: Vec2::from(transform.position),
        scale_percent: Vec2::from(transform.scale),
        rotation_deg: transform.rotation,
        opacity_percent: transform.opacity,
    }
}

fn evaluate_transform(
    transform: &render_ir::Transform2D,
    time: f64,
    layer_start: f64,
    layer_duration: f64,
    fps: f64,
) -> render_ir::Transform2D {
    let mut evaluated = transform.clone();
    evaluated.position =
        evaluate_vec2_keyframes(&transform.animation.position, time, transform.position);
    evaluated.scale = evaluate_vec2_keyframes(&transform.animation.scale, time, transform.scale);
    evaluated.opacity =
        evaluate_scalar_keyframes(&transform.animation.opacity, time, transform.opacity);
    if let Some(expression) = &transform.animation.expression.position {
        evaluated.position = evaluate_position_expression(
            expression,
            evaluated.position,
            time,
            layer_start,
            layer_duration,
            fps,
        );
    }
    evaluated
}

fn evaluate_position_expression(
    expression: &PositionExpression,
    base: [f32; 2],
    time: f64,
    layer_start: f64,
    layer_duration: f64,
    fps: f64,
) -> [f32; 2] {
    evaluate_position_expression_sample(expression, base, time, layer_start, layer_duration, fps)
        .position
}

#[derive(Debug, Clone)]
struct PositionExpressionSample {
    position: [f32; 2],
    local_time: f32,
    frame_duration: f32,
    layer_duration: f32,
    envelope: Option<f32>,
    offset: Option<[f32; 2]>,
    source: String,
    mode: &'static str,
    error: Option<String>,
}

fn evaluate_position_expression_sample(
    expression: &PositionExpression,
    base: [f32; 2],
    time: f64,
    layer_start: f64,
    layer_duration: f64,
    fps: f64,
) -> PositionExpressionSample {
    match expression {
        PositionExpression::EdgeWobble {
            intro,
            outro,
            amp,
            freq,
            source,
        } => {
            let t = (time - layer_start).max(0.0) as f32;
            let frame_duration = if fps > 0.0 {
                (1.0 / fps) as f32
            } else {
                1.0 / 30.0
            };
            let dur = (layer_duration as f32).max(frame_duration);
            let mut k = 0.0_f32;
            if t >= 0.0 && t < *intro {
                let p = (t / *intro).clamp(0.0, 1.0);
                k = (p * std::f32::consts::PI).sin() * (-2.4 * p).exp();
            } else if t > dur - *outro && t <= dur {
                let p = ((dur - t) / *outro).clamp(0.0, 1.0);
                k = (p * std::f32::consts::PI).sin() * (-2.4 * p).exp();
            }
            let x = *amp * k * (2.0 * std::f32::consts::PI * *freq * t).sin();
            let y = *amp * 0.68 * k * (2.0 * std::f32::consts::PI * (*freq * 0.82) * t).cos();
            PositionExpressionSample {
                position: [base[0] + x, base[1] + y],
                local_time: t,
                frame_duration,
                layer_duration: dur,
                envelope: Some(k),
                offset: Some([x, y]),
                source: source.clone(),
                mode: "edge_wobble",
                error: None,
            }
        }
        PositionExpression::ParsedProperty { source } => {
            let t = (time - layer_start).max(0.0) as f32;
            let frame_duration = if fps > 0.0 {
                (1.0 / fps) as f32
            } else {
                1.0 / 30.0
            };
            let dur = (layer_duration as f32).max(frame_duration);
            let evaluator =
                BoundaryPropertyExpressionEvaluator::new(NamedPatternExpressionEvaluator);
            let request = PropertyExpressionRequest {
                source,
                property_path: "transform.position",
                target_type: PropertyValueType::Vector2,
                fingerprint: None,
                context: PropertyExpressionContext {
                    time,
                    value: ExprValue::Vec2([base[0] as f64, base[1] as f64]),
                    comp: CompExpressionContext {
                        frame_duration: frame_duration as f64,
                        duration: layer_duration,
                        ..CompExpressionContext::default()
                    },
                    layer: LayerExpressionContext {
                        in_point: layer_start,
                        out_point: layer_start + layer_duration,
                        start_time: layer_start,
                        ..LayerExpressionContext::default()
                    },
                    vars: std::collections::HashMap::new(),
                },
            };
            let result = evaluator.eval_property(&request, &NoopPropertyExpressionHost);
            let (position, error) = match result {
                Ok(result) if result.mode == PropertyExpressionMode::ParsedSubset => {
                    match result.value {
                        ExprValue::Vec2(value) => ([value[0] as f32, value[1] as f32], None),
                        other => (
                            base,
                            Some(format!(
                                "parsed transform.position returned non-Vec2 value {other:?}"
                            )),
                        ),
                    }
                }
                Ok(result) => (
                    base,
                    Some(format!(
                        "parsed transform.position used unexpected mode {:?}",
                        result.mode
                    )),
                ),
                Err(err) => (base, Some(err.to_string())),
            };
            PositionExpressionSample {
                position,
                local_time: t,
                frame_duration,
                layer_duration: dur,
                envelope: None,
                offset: Some([position[0] - base[0], position[1] - base[1]]),
                source: source.clone(),
                mode: "parsed_property_subset",
                error,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_transform_sampling_trace(
    mut trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    transform: &render_ir::Transform2D,
    time: f64,
    layer_start: f64,
    layer_duration: f64,
    fps: f64,
) {
    record_transform_keyframe_trace(
        trace.as_deref_mut(),
        composition_id,
        layer_id,
        transform,
        time,
    );
    record_transform_expression_trace(
        trace,
        composition_id,
        layer_id,
        transform,
        time,
        layer_start,
        layer_duration,
        fps,
    );
}

fn record_transform_keyframe_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    transform: &render_ir::Transform2D,
    time: f64,
) {
    let Some(trace) = trace else {
        return;
    };
    record_vec2_keyframe_trace(
        trace,
        composition_id,
        layer_id,
        "transform.position",
        &transform.animation.position,
        time,
        transform.position,
    );
    record_vec2_keyframe_trace(
        trace,
        composition_id,
        layer_id,
        "transform.scale",
        &transform.animation.scale,
        time,
        transform.scale,
    );
    record_scalar_keyframe_trace(
        trace,
        composition_id,
        layer_id,
        "transform.opacity",
        &transform.animation.opacity,
        time,
        transform.opacity,
    );
    record_scalar_keyframe_trace(
        trace,
        composition_id,
        layer_id,
        "transform.reveal",
        &transform.animation.reveal,
        time,
        100.0,
    );
}

fn record_vec2_keyframe_trace(
    trace: &mut FrameRenderTrace,
    composition_id: &str,
    layer_id: &str,
    property: &'static str,
    keyframes: &[Vec2Keyframe],
    time: f64,
    fallback: [f32; 2],
) {
    if keyframes.is_empty() {
        return;
    }
    let sample = sample_vec2_keyframes(keyframes, time, fallback);
    push_keyframe_trace_record(
        trace,
        composition_id,
        layer_id,
        property,
        time,
        json!(sample.value),
        sample.diagnostics,
    );
}

fn record_scalar_keyframe_trace(
    trace: &mut FrameRenderTrace,
    composition_id: &str,
    layer_id: &str,
    property: &'static str,
    keyframes: &[ScalarKeyframe],
    time: f64,
    fallback: f32,
) {
    if keyframes.is_empty() {
        return;
    }
    let sample = sample_scalar_keyframes(keyframes, time, fallback);
    push_keyframe_trace_record(
        trace,
        composition_id,
        layer_id,
        property,
        time,
        json!(sample.value),
        sample.diagnostics,
    );
}

fn push_keyframe_trace_record(
    trace: &mut FrameRenderTrace,
    composition_id: &str,
    layer_id: &str,
    property: &'static str,
    time: f64,
    value: Value,
    diagnostics: KeyframeDiagnostics,
) {
    trace.keyframes.push(KeyframeTraceRecord {
        event: "temporal.keyframe_sample",
        composition: composition_id.to_string(),
        layer_id: layer_id.to_string(),
        property,
        sample_time: time,
        value,
        phase: diagnostics.phase,
        interpolation: diagnostics.interpolation,
        segment_index: diagnostics.segment_index,
        key_start_time: diagnostics.key_start_time,
        key_end_time: diagnostics.key_end_time,
        normalized_time: diagnostics.normalized_time,
        eased_progress: diagnostics.eased_progress,
        hold: diagnostics.hold,
        approximate: diagnostics.approximate,
        ease: diagnostics.ease,
    });
}

#[allow(clippy::too_many_arguments)]
fn record_transform_expression_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    transform: &render_ir::Transform2D,
    time: f64,
    layer_start: f64,
    layer_duration: f64,
    fps: f64,
) {
    let Some(trace) = trace else {
        return;
    };
    let Some(expression) = &transform.animation.expression.position else {
        return;
    };
    let base_position =
        evaluate_vec2_keyframes(&transform.animation.position, time, transform.position);
    let sample = evaluate_position_expression_sample(
        expression,
        base_position,
        time,
        layer_start,
        layer_duration,
        fps,
    );
    trace.position_expressions.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "property": "transform.position",
        "time": time,
        "layer_start": layer_start,
        "layer_duration": layer_duration,
        "fps": fps,
        "base_position": base_position,
        "sampled_position": sample.position,
        "evaluator": {
            "subset": match expression {
                PositionExpression::EdgeWobble { .. } => "generated_named_position_expression",
                PositionExpression::ParsedProperty { .. } => "parsed_property_expression",
            },
            "mode": sample.mode,
            "target_type": "vector2",
            "fingerprint": sample.source.as_str()
        },
        "context": {
            "local_time": sample.local_time,
            "frame_duration": sample.frame_duration,
            "duration": sample.layer_duration,
            "thisComp": {
                "frameDuration": sample.frame_duration,
                "fps": fps
            },
            "thisLayer": {
                "inPoint": layer_start,
                "outPoint": layer_start + layer_duration,
                "startTime": layer_start
            }
        },
        "expression": match expression {
            PositionExpression::EdgeWobble { intro, outro, amp, freq, .. } => json!({
                "type": "edge_wobble",
                "source": sample.source.as_str(),
                "intro": intro,
                "outro": outro,
                "amp": amp,
                "freq": freq,
                "envelope": sample.envelope,
                "offset": sample.offset
            }),
            PositionExpression::ParsedProperty { .. } => json!({
                "type": "parsed_property",
                "source": sample.source.as_str(),
                "offset": sample.offset,
                "error": sample.error
            }),
        }
    }));
}

#[derive(Debug, Clone)]
struct KeyframeEvaluation<T> {
    value: T,
    diagnostics: KeyframeDiagnostics,
}

#[derive(Debug, Clone, Copy)]
struct KeyframeDiagnostics {
    phase: &'static str,
    interpolation: &'static str,
    segment_index: Option<usize>,
    key_start_time: Option<f64>,
    key_end_time: Option<f64>,
    normalized_time: Option<f32>,
    eased_progress: Option<f32>,
    hold: bool,
    approximate: bool,
    ease: Option<render_ir::KeyframeEase>,
}

fn evaluate_vec2_keyframes(keyframes: &[Vec2Keyframe], time: f64, fallback: [f32; 2]) -> [f32; 2] {
    sample_vec2_keyframes(keyframes, time, fallback).value
}

fn sample_vec2_keyframes(
    keyframes: &[Vec2Keyframe],
    time: f64,
    fallback: [f32; 2],
) -> KeyframeEvaluation<[f32; 2]> {
    if keyframes.is_empty() {
        return KeyframeEvaluation {
            value: fallback,
            diagnostics: keyframe_static_diagnostics("empty_fallback"),
        };
    }
    if time <= keyframes[0].time {
        return KeyframeEvaluation {
            value: keyframes[0].value,
            diagnostics: keyframe_edge_diagnostics("before_first", &keyframes[0]),
        };
    }
    for (segment_index, pair) in keyframes.windows(2).enumerate() {
        let a = &pair[0];
        let b = &pair[1];
        if time <= b.time {
            let diagnostics = keyframe_segment_diagnostics(segment_index, a, b, time);
            if diagnostics.interpolation == "hold" {
                return KeyframeEvaluation {
                    value: a.value,
                    diagnostics,
                };
            }
            let t = diagnostics.eased_progress.unwrap_or(0.0);
            return KeyframeEvaluation {
                value: [
                    a.value[0] + (b.value[0] - a.value[0]) * t,
                    a.value[1] + (b.value[1] - a.value[1]) * t,
                ],
                diagnostics,
            };
        }
    }
    let key = keyframes.last().unwrap();
    KeyframeEvaluation {
        value: key.value,
        diagnostics: keyframe_edge_diagnostics("after_last", key),
    }
}

fn evaluate_scalar_keyframes(keyframes: &[ScalarKeyframe], time: f64, fallback: f32) -> f32 {
    sample_scalar_keyframes(keyframes, time, fallback).value
}

fn sample_scalar_keyframes(
    keyframes: &[ScalarKeyframe],
    time: f64,
    fallback: f32,
) -> KeyframeEvaluation<f32> {
    if keyframes.is_empty() {
        return KeyframeEvaluation {
            value: fallback,
            diagnostics: keyframe_static_diagnostics("empty_fallback"),
        };
    }
    if time <= keyframes[0].time {
        return KeyframeEvaluation {
            value: keyframes[0].value,
            diagnostics: keyframe_edge_diagnostics("before_first", &keyframes[0]),
        };
    }
    for (segment_index, pair) in keyframes.windows(2).enumerate() {
        let a = &pair[0];
        let b = &pair[1];
        if time <= b.time {
            let diagnostics = keyframe_segment_diagnostics(segment_index, a, b, time);
            if diagnostics.interpolation == "hold" {
                return KeyframeEvaluation {
                    value: a.value,
                    diagnostics,
                };
            }
            let t = diagnostics.eased_progress.unwrap_or(0.0);
            return KeyframeEvaluation {
                value: a.value + (b.value - a.value) * t,
                diagnostics,
            };
        }
    }
    let key = keyframes.last().unwrap();
    KeyframeEvaluation {
        value: key.value,
        diagnostics: keyframe_edge_diagnostics("after_last", key),
    }
}

trait KeyframeLike {
    fn time(&self) -> f64;
    fn hold(&self) -> bool;
    fn approximate(&self) -> bool;
    fn ease(&self) -> Option<render_ir::KeyframeEase>;
}

impl KeyframeLike for Vec2Keyframe {
    fn time(&self) -> f64 {
        self.time
    }

    fn hold(&self) -> bool {
        self.hold
    }

    fn approximate(&self) -> bool {
        self.approximate
    }

    fn ease(&self) -> Option<render_ir::KeyframeEase> {
        self.ease
    }
}

impl KeyframeLike for ScalarKeyframe {
    fn time(&self) -> f64 {
        self.time
    }

    fn hold(&self) -> bool {
        self.hold
    }

    fn approximate(&self) -> bool {
        self.approximate
    }

    fn ease(&self) -> Option<render_ir::KeyframeEase> {
        self.ease
    }
}

fn keyframe_static_diagnostics(phase: &'static str) -> KeyframeDiagnostics {
    KeyframeDiagnostics {
        phase,
        interpolation: "none",
        segment_index: None,
        key_start_time: None,
        key_end_time: None,
        normalized_time: None,
        eased_progress: None,
        hold: false,
        approximate: false,
        ease: None,
    }
}

fn keyframe_edge_diagnostics<K: KeyframeLike>(phase: &'static str, key: &K) -> KeyframeDiagnostics {
    KeyframeDiagnostics {
        phase,
        interpolation: "edge",
        segment_index: None,
        key_start_time: Some(key.time()),
        key_end_time: Some(key.time()),
        normalized_time: None,
        eased_progress: None,
        hold: key.hold(),
        approximate: key.approximate(),
        ease: key.ease(),
    }
}

fn keyframe_segment_diagnostics<K: KeyframeLike>(
    segment_index: usize,
    a: &K,
    b: &K,
    time: f64,
) -> KeyframeDiagnostics {
    let interpolation = if a.hold() || b.time() <= a.time() {
        "hold"
    } else if a.ease().is_some() {
        "bezier"
    } else {
        "linear"
    };
    let normalized_time = (b.time() > a.time())
        .then(|| ((time - a.time()) / (b.time() - a.time())).clamp(0.0, 1.0) as f32);
    let eased_progress = normalized_time.map(|t| {
        if interpolation == "hold" {
            0.0
        } else {
            eased_progress(t, a.ease())
        }
    });
    KeyframeDiagnostics {
        phase: "segment",
        interpolation,
        segment_index: Some(segment_index),
        key_start_time: Some(a.time()),
        key_end_time: Some(b.time()),
        normalized_time,
        eased_progress,
        hold: a.hold(),
        approximate: a.approximate(),
        ease: a.ease(),
    }
}

fn eased_progress(t: f32, ease: Option<render_ir::KeyframeEase>) -> f32 {
    let Some(ease) = ease else {
        return t;
    };
    cubic_bezier_y_for_x(t, ease).clamp(0.0, 1.0)
}

fn cubic_bezier_y_for_x(x: f32, ease: render_ir::KeyframeEase) -> f32 {
    let mut u = x;
    for _ in 0..6 {
        let current_x = cubic_bezier(u, 0.0, ease.x1, ease.x2, 1.0);
        let dx = cubic_bezier_derivative(u, 0.0, ease.x1, ease.x2, 1.0);
        if dx.abs() < 1.0e-5 {
            break;
        }
        u = (u - (current_x - x) / dx).clamp(0.0, 1.0);
    }
    cubic_bezier(u, 0.0, ease.y1, ease.y2, 1.0)
}

fn cubic_bezier(t: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let mt = 1.0 - t;
    mt * mt * mt * p0 + 3.0 * mt * mt * t * p1 + 3.0 * mt * t * t * p2 + t * t * t * p3
}

fn cubic_bezier_derivative(t: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let mt = 1.0 - t;
    3.0 * mt * mt * (p1 - p0) + 6.0 * mt * t * (p2 - p1) + 3.0 * t * t * (p3 - p2)
}

fn apply_horizontal_reveal(canvas: &mut Canvas, reveal_percent: f32) {
    let reveal = reveal_percent.clamp(0.0, 100.0);
    if reveal >= 99.999 {
        return;
    }
    let visible_width = (canvas.width as f32 * reveal / 100.0).round() as u32;
    for y in 0..canvas.height {
        for x in visible_width..canvas.width {
            let mut pixel = canvas.pixel(x, y);
            pixel[3] = 0;
            canvas.set_pixel(x, y, pixel);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_text_layout_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    comp_time: f64,
    layer_time: f64,
    request: &TextLayoutRequest,
    raster_size: [u32; 2],
    raster_scale: f32,
    layer_matrix: Mat3,
    local_origin: [f32; 2],
    layout: Option<&TextLayoutResult>,
    raster_trace: Option<&TextRasterTrace>,
    render_path: &str,
) {
    let Some(trace) = trace else {
        return;
    };
    let layout_telemetry = layout
        .map(|layout| {
            let telemetry = text_layout_trace_telemetry(layout, layer_matrix, local_origin);
            serde_json::to_value(&telemetry)
                .unwrap_or_else(|err| json!({ "serialization_error": err.to_string() }))
        })
        .unwrap_or_else(|| json!(null));
    let draw_char_telemetry = raster_trace
        .map(|trace| {
            serde_json::to_value(trace)
                .unwrap_or_else(|err| json!({ "serialization_error": err.to_string() }))
        })
        .unwrap_or_else(|| json!(null));
    trace.text_layouts.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "render_path": render_path,
        "comp_time": comp_time,
        "layer_time": layer_time,
        "request": {
            "text": &request.text,
            "font_id": &request.font_id,
            "font_size": request.font_size,
            "box_rect": request.box_rect
        },
        "raster": {
            "size": raster_size,
            "scale": raster_scale
        },
        "layout": layout_telemetry,
        "draw_char": draw_char_telemetry
    }));
}

fn text_layout_trace_telemetry(
    layout: &TextLayoutResult,
    layer_matrix: Mat3,
    local_origin: [f32; 2],
) -> text_engine::TextLayoutTelemetry {
    let mut telemetry = layout.telemetry.clone();
    for glyph in &mut telemetry.glyphs {
        glyph.bbox = transform_layout_bbox(layer_matrix, local_origin, glyph.bbox);
        glyph.cooltype_bbox_minmax = [
            glyph.bbox[0],
            glyph.bbox[1],
            glyph.bbox[0] + glyph.bbox[2],
            glyph.bbox[1] + glyph.bbox[3],
        ];
        glyph.bbox_center = transform_layout_point(layer_matrix, local_origin, glyph.bbox_center);
        glyph.baseline =
            transform_layout_point(layer_matrix, local_origin, [0.0, glyph.baseline])[1];
    }
    for line_box in &mut telemetry.line_boxes {
        line_box.line_box = transform_layout_bbox(layer_matrix, local_origin, line_box.line_box);
        line_box.glyph_bbox = line_box
            .glyph_bbox
            .map(|bbox| transform_layout_bbox(layer_matrix, local_origin, bbox));
        line_box.baseline =
            transform_layout_point(layer_matrix, local_origin, [0.0, line_box.baseline])[1];
    }
    telemetry.source_rect_union = telemetry
        .source_rect_union
        .map(|bbox| transform_layout_bbox(layer_matrix, local_origin, bbox));
    telemetry.text_box_rect =
        transform_layout_bbox(layer_matrix, local_origin, telemetry.text_box_rect);
    telemetry
}

fn transform_layout_bbox(matrix: Mat3, local_origin: [f32; 2], bbox: [f32; 4]) -> [f32; 4] {
    let x0 = bbox[0];
    let y0 = bbox[1];
    let x1 = bbox[0] + bbox[2];
    let y1 = bbox[1] + bbox[3];
    let corners = [
        transform_layout_point(matrix, local_origin, [x0, y0]),
        transform_layout_point(matrix, local_origin, [x1, y0]),
        transform_layout_point(matrix, local_origin, [x1, y1]),
        transform_layout_point(matrix, local_origin, [x0, y1]),
    ];
    let min_x = corners
        .iter()
        .map(|point| point[0])
        .fold(f32::INFINITY, f32::min);
    let min_y = corners
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners
        .iter()
        .map(|point| point[1])
        .fold(f32::NEG_INFINITY, f32::max);
    [
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    ]
}

fn transform_layout_point(matrix: Mat3, local_origin: [f32; 2], point: [f32; 2]) -> [f32; 2] {
    let transformed = matrix.transform_point(Vec2::new(
        local_origin[0] + point[0],
        local_origin[1] + point[1],
    ));
    [transformed.x, transformed.y]
}

#[allow(clippy::too_many_arguments)]
fn record_text_selector_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    text: &str,
    layout: Option<&TextLayoutResult>,
    canvas: &Canvas,
    animator: &TextAnimatorSpec,
    start_percent: f32,
    end_percent: f32,
    time: f64,
    layer_start: f64,
    unit_scale: f32,
) {
    let Some(trace) = trace else {
        return;
    };
    let units = unit_rects(canvas, text, layout, animator.selector.based_on);
    let unit_records = units
        .iter()
        .map(|unit| {
            let selector_index = selector_order_index(unit.index, unit.total, &animator.selector);
            let selector_position_percent = unit_center_percent(selector_index, unit.total);
            let range_weight = selector_weight(
                unit.index,
                unit.total,
                &animator.selector,
                start_percent,
                end_percent,
                time,
            );
            let expression = text_expression_weight_detail(
                animator.expression_selector.as_ref(),
                unit.index,
                unit.total,
                time,
                layer_start,
            );
            let final_weight = range_weight * expression.weight;
            let mut record = json!({
                "index": unit.index,
                "selector_index": selector_index,
                "selector_position_percent": selector_position_percent,
                "total": unit.total,
                "rect": [unit.x0, unit.y0, unit.x1, unit.y1],
                "range_weight": range_weight,
                "expression_weight": expression.weight,
                "final_weight": final_weight,
                "expression": expression.to_json(),
                "glyph_passport": selector_unit_glyph_passport(
                    layout,
                    animator.selector.based_on,
                    unit.index
                )
            });
            record["animator_contribution"] =
                animator_contribution_json(*unit, animator, final_weight, unit_scale);
            record
        })
        .collect::<Vec<_>>();

    trace.text_selector_weights.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "animator": &animator.name,
        "time": time,
        "layer_start": layer_start,
        "unit_scale": unit_scale,
        "selector": {
            "based_on": animator.selector.based_on,
            "shape": animator.selector.shape,
            "start": start_percent,
            "end": end_percent,
            "smoothness": animator.selector.smoothness,
            "randomize_order": animator.selector.randomize_order,
            "wiggly": animator.selector.wiggly
        },
        "expression_selector": &animator.expression_selector,
        "units": unit_records
    }));
}

fn record_rasterized_precomp_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    source_composition: &str,
    collapse_requested: bool,
    time: f64,
    source_time: f64,
    layer_matrix: Mat3,
    raster_size: [u32; 2],
) {
    let Some(trace) = trace else {
        return;
    };
    trace.collapse.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "source_composition": source_composition,
        "collapse_requested": collapse_requested,
        "mode": if collapse_requested { "rasterize_first" } else { "rasterized_precomp" },
        "time": time,
        "source_time": source_time,
        "layer_matrix": layer_matrix.m,
        "layer_matrix_report": matrix_report_json(layer_matrix),
        "raster_size": raster_size
    }));
    let reason = if collapse_requested {
        "collapse_requested_but_rasterize_first"
    } else {
        "collapse_not_requested"
    };
    trace.collapse.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "source_composition": source_composition,
        "collapse_requested": collapse_requested,
        "mode": "bee_text_carrier_route",
        "time": time,
        "source_time": source_time,
        "bee_text_carrier_route": {
            "parent_composition": composition_id,
            "precomp_layer_id": layer_id,
            "target_composition": source_composition,
            "source_composition": null,
            "source_layer_id": null,
            "status": BeeTextCarrierRouteStatus::PrecompRasterizeFirst.as_str(),
            "reason": reason,
            "transform_step_count": 0,
            "source_time_step_count": 0,
            "counts_as_direct_p6_success": BeeTextCarrierRouteStatus::PrecompRasterizeFirst.counts_as_direct_p6_success(),
            "counts_as_p5_green": BeeTextCarrierRouteStatus::PrecompRasterizeFirst.counts_as_p5_green()
        },
        "deferred_raster_checkpoint": {
            "status": BeeTextCarrierRouteStatus::PrecompRasterizeFirst.as_str(),
            "blocker": reason
        }
    }));
}

fn record_collapsed_precomp_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    source_composition: &str,
    time: f64,
    source_time: f64,
    parent_matrix: Mat3,
    flattened_layers: Vec<String>,
) {
    let Some(trace) = trace else {
        return;
    };
    trace.collapse.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "source_composition": source_composition,
        "collapse_requested": true,
        "mode": "collapse_supported_vectors",
        "time": time,
        "source_time": source_time,
        "parent_matrix": parent_matrix.m,
        "parent_matrix_report": matrix_report_json(parent_matrix),
        "deferred_raster_checkpoint": {
            "status": "matrix_pushdown_only",
            "blocker": "true text/vector deferred rasterization is not parity-locked by this trace"
        },
        "flattened_layers": flattened_layers
    }));
}

fn bee_text_carrier_routes_for_precomp(
    scene: &Scene,
    parent_composition: &str,
    precomp_layer_id: &str,
) -> Vec<Value> {
    let graph = match PrecompGraph::from_scene(scene) {
        Ok(graph) => graph,
        Err(err) => {
            return vec![json!({
                "parent_composition": parent_composition,
                "precomp_layer_id": precomp_layer_id,
                "target_composition": null,
                "source_composition": null,
                "source_layer_id": null,
                "status": BeeTextCarrierRouteStatus::BeeTextCarrierUnsupported.as_str(),
                "reason": "precomp_graph_error",
                "error": err.to_string(),
                "transform_step_count": 0,
                "source_time_step_count": 0,
                "counts_as_direct_p6_success": false,
                "counts_as_p5_green": false
            })];
        }
    };
    let plan =
        match graph.deferred_raster_plan_for_precomp_layer(parent_composition, precomp_layer_id) {
            Ok(plan) => plan,
            Err(err) => {
                return vec![json!({
                    "parent_composition": parent_composition,
                    "precomp_layer_id": precomp_layer_id,
                    "target_composition": null,
                    "source_composition": null,
                    "source_layer_id": null,
                    "status": BeeTextCarrierRouteStatus::BeeTextCarrierUnsupported.as_str(),
                    "reason": "deferred_raster_plan_error",
                    "error": err.to_string(),
                    "transform_step_count": 0,
                    "source_time_step_count": 0,
                    "counts_as_direct_p6_success": false,
                    "counts_as_p5_green": false
                })];
            }
        };

    classify_bee_text_carrier_routes(&plan)
        .iter()
        .map(bee_text_carrier_route_json)
        .collect()
}

fn bee_text_carrier_route_json(record: &BeeTextCarrierRouteRecord<'_>) -> Value {
    json!({
        "parent_composition": record.parent_composition,
        "precomp_layer_id": record.precomp_layer_id,
        "target_composition": record.target_composition,
        "source_composition": record.source_composition,
        "source_layer_id": record.source_layer_id,
        "status": record.status.as_str(),
        "reason": record.reason,
        "transform_step_count": record.transform_step_count,
        "source_time_step_count": record.source_time_step_count,
        "counts_as_direct_p6_success": record.status.counts_as_direct_p6_success(),
        "counts_as_p5_green": record.status.counts_as_p5_green()
    })
}

fn record_bee_text_carrier_route_trace(trace: Option<&mut FrameRenderTrace>, routes: Vec<Value>) {
    let Some(trace) = trace else {
        return;
    };
    for route in routes {
        let status = route
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("bee_text_carrier_unsupported");
        let reason = route
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("missing_reason");
        trace.collapse.push(json!({
            "composition": route
                .get("parent_composition")
                .and_then(Value::as_str)
                .unwrap_or(""),
            "layer_id": route
                .get("precomp_layer_id")
                .and_then(Value::as_str)
                .unwrap_or(""),
            "source_composition": route.get("target_composition").cloned().unwrap_or(Value::Null),
            "source_layer_id": route.get("source_layer_id").cloned().unwrap_or(Value::Null),
            "collapse_requested": true,
            "mode": "bee_text_carrier_route",
            "bee_text_carrier_route": route,
            "deferred_raster_checkpoint": {
                "status": status,
                "blocker": reason
            }
        }));
    }
}

#[allow(clippy::too_many_arguments)]
fn record_bee_text_carrier_routed_trace(
    trace: Option<&mut FrameRenderTrace>,
    context: Option<BeeTextCarrierRenderContext<'_>>,
    parent_composition: &str,
    source_layer_id: &str,
    time: f64,
    parent_matrix: Mat3,
    child_matrix: Mat3,
    effective_matrix: Mat3,
    source_size: [u32; 2],
    raster_trace: &TextRasterTrace,
    rendered_canvas: &Canvas,
) {
    let Some(trace) = trace else {
        return;
    };
    let status = BeeTextCarrierRouteStatus::BeeTextCarrierRouted;
    let routed_pixel_events = raster_trace
        .events
        .iter()
        .filter(|event| {
            event.stage == "p6_ad68_pixel_write"
                && event.payload.get("status").and_then(Value::as_str) == Some("routed_pixels")
        })
        .count();
    let expected_advance_only_events = raster_trace
        .events
        .iter()
        .filter(|event| {
            event.stage == "p6_ad68_pixel_write"
                && event.payload.get("status").and_then(Value::as_str)
                    == Some("expected_advance_only_space")
        })
        .count();
    let unsupported_events = raster_trace
        .events
        .iter()
        .filter(|event| {
            event.stage == "p6_ad68_pixel_write"
                && event.payload.get("status").and_then(Value::as_str)
                    == Some("unsupported_source_path")
        })
        .count();
    let pixel_write_count = raster_trace
        .events
        .iter()
        .filter(|event| event.stage == "p6_ad68_pixel_write")
        .filter_map(|event| {
            event
                .payload
                .get("pixel_write_count")
                .and_then(Value::as_u64)
        })
        .sum::<u64>();
    let target_composition = context
        .map(|context| Value::String(context.target_composition.to_string()))
        .unwrap_or(Value::Null);
    let precomp_layer_id = context
        .map(|context| Value::String(context.precomp_layer_id.to_string()))
        .unwrap_or(Value::Null);
    let route = json!({
        "parent_composition": context
            .map(|context| context.parent_composition)
            .unwrap_or(parent_composition),
        "precomp_layer_id": precomp_layer_id,
        "target_composition": target_composition,
        "source_composition": context
            .map(|context| Value::String(context.target_composition.to_string()))
            .unwrap_or(Value::Null),
        "source_layer_id": source_layer_id,
        "status": status.as_str(),
        "reason": "source_glyph_payload_routed_to_p6_ad68_vector_pixels",
        "producer": "p6_source_owned_ad68_event_stream_to_canvas_pixels_vector_transform_v1",
        "payload_bridge": "layer_text_source_outline_to_p6_ad68_vector_transform",
        "source_trace": raster_trace.source,
        "source_time": context.map(|context| context.source_time),
        "glyph_count": raster_trace.draw_chars.len(),
        "routed_pixel_event_count": routed_pixel_events,
        "expected_advance_only_event_count": expected_advance_only_events,
        "unsupported_event_count": unsupported_events,
        "pixel_write_count": pixel_write_count,
        "transform_step_count": if context.is_some() { 2 } else { 0 },
        "source_time_step_count": usize::from(context.is_some()),
        "counts_as_direct_p6_success": status.counts_as_direct_p6_success(),
        "counts_as_p5_green": status.counts_as_p5_green(),
        "text_are_matrix_carrier": {
            "font_size_baked_scale": true,
            "scale_carrier": "scale_baked_bee_text_carrier_local_canvas",
            "path": "scale-baked BEE carrier glyph paths -> P6 AD68 rows -> local carrier pixels -> destination canvas pixels"
        }
    });
    trace.collapse.push(json!({
        "composition": parent_composition,
        "layer_id": context
            .map(|context| context.precomp_layer_id)
            .unwrap_or(source_layer_id),
        "source_composition": route.get("source_composition").cloned().unwrap_or(Value::Null),
        "source_layer_id": source_layer_id,
        "collapse_requested": true,
        "mode": "bee_text_carrier_route",
        "time": time,
        "parent_matrix": parent_matrix.m,
        "child_matrix": child_matrix.m,
        "effective_matrix": effective_matrix.m,
        "parent_matrix_report": matrix_report_json(parent_matrix),
        "child_matrix_report": matrix_report_json(child_matrix),
        "effective_matrix_report": matrix_report_json(effective_matrix),
        "source_canvas_size": source_size,
        "bee_text_carrier_route": route,
        "deferred_raster_checkpoint": {
            "status": status.as_str(),
            "blocker": "none_for_bounded_gph010_vector_payload_bridge"
        },
        "sharpness_probe": alpha_sharpness_probe(rendered_canvas)
    }));
}

fn record_collapsed_text_raster_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    time: f64,
    parent_matrix: Mat3,
    child_matrix: Mat3,
    effective_matrix: Mat3,
    raster_scale: f32,
    raster_size: [u32; 2],
    text_canvas: &Canvas,
) {
    let Some(trace) = trace else {
        return;
    };
    trace.collapse.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "mode": "collapsed_text_raster",
        "time": time,
        "parent_matrix": parent_matrix.m,
        "child_matrix": child_matrix.m,
        "effective_matrix": effective_matrix.m,
        "parent_matrix_report": matrix_report_json(parent_matrix),
        "child_matrix_report": matrix_report_json(child_matrix),
        "effective_matrix_report": matrix_report_json(effective_matrix),
        "effective_raster_scale": raster_scale,
        "raster_size": raster_size,
        "deferred_raster_checkpoint": {
            "status": "intermediate_text_raster",
            "blocker": "text is rasterized to a scale-hint canvas before final sampling"
        },
        "sharpness_probe": alpha_sharpness_probe(text_canvas)
    }));
}

#[allow(clippy::too_many_arguments)]
fn record_collapsed_text_vector_trace(
    trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
    time: f64,
    parent_matrix: Mat3,
    child_matrix: Mat3,
    effective_matrix: Mat3,
    raster_scale: f32,
    source_size: [u32; 2],
    rendered_canvas: Option<&Canvas>,
    status: &str,
    blocker: Option<&str>,
) {
    let Some(trace) = trace else {
        return;
    };
    trace.collapse.push(json!({
        "composition": composition_id,
        "layer_id": layer_id,
        "mode": "collapsed_text_vector_deferred",
        "time": time,
        "parent_matrix": parent_matrix.m,
        "child_matrix": child_matrix.m,
        "effective_matrix": effective_matrix.m,
        "parent_matrix_report": matrix_report_json(parent_matrix),
        "child_matrix_report": matrix_report_json(child_matrix),
        "effective_matrix_report": matrix_report_json(effective_matrix),
        "effective_raster_scale": raster_scale,
        "source_canvas_size": source_size,
        "deferred_raster_checkpoint": {
            "status": status,
            "blocker": blocker.unwrap_or("none_for_p6_source_path_to_destination_pixels")
        },
        "text_are_matrix_carrier": {
            "font_size_baked_scale": raster_scale > 1.0001,
            "scale_carrier": "scale_baked_bee_text_carrier_local_canvas",
            "path": "scale-baked BEE carrier glyph paths -> P6 AD68 rows -> local carrier pixels -> destination canvas pixels"
        },
        "sharpness_probe": rendered_canvas
            .map(alpha_sharpness_probe)
            .unwrap_or_else(|| json!(null))
    }));
}

fn alpha_sharpness_probe(canvas: &Canvas) -> Value {
    let mut sum = 0_u64;
    let mut max_edge = 0_u8;
    let mut samples = 0_u64;
    let mut covered = 0_u64;

    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let alpha = canvas.pixel(x, y)[3];
            if alpha > 0 {
                covered += 1;
            }
            if x > 0 {
                let diff = alpha.abs_diff(canvas.pixel(x - 1, y)[3]);
                sum += u64::from(diff);
                max_edge = max_edge.max(diff);
                samples += 1;
            }
            if y > 0 {
                let diff = alpha.abs_diff(canvas.pixel(x, y - 1)[3]);
                sum += u64::from(diff);
                max_edge = max_edge.max(diff);
                samples += 1;
            }
        }
    }

    json!({
        "alpha_edge_mean": sum as f64 / samples.max(1) as f64,
        "alpha_edge_max": max_edge,
        "alpha_coverage_ratio": covered as f64 / (canvas.width as u64 * canvas.height as u64).max(1) as f64
    })
}

fn apply_text_animators(
    mut canvas: Canvas,
    text: &str,
    layout: Option<&TextLayoutResult>,
    animators: &[TextAnimatorSpec],
    time: f64,
    layer_start: f64,
    unit_scale: f32,
    mut trace: Option<&mut FrameRenderTrace>,
    composition_id: &str,
    layer_id: &str,
) -> Canvas {
    for animator in animators {
        let start = evaluate_scalar_keyframes(
            &animator.selector.start_keyframes,
            time,
            animator.selector.start,
        )
        .clamp(0.0, 100.0);
        let end = evaluate_scalar_keyframes(
            &animator.selector.end_keyframes,
            time,
            animator.selector.end,
        )
        .clamp(0.0, 100.0);
        record_text_selector_trace(
            trace.as_deref_mut(),
            composition_id,
            layer_id,
            text,
            layout,
            &canvas,
            animator,
            start.min(end),
            start.max(end),
            time,
            layer_start,
            unit_scale,
        );
        if animator.position.is_some()
            || animator.scale.is_some()
            || animator.rotation.is_some()
            || animator.blur.is_some()
            || animator.expression_selector.is_some()
        {
            canvas = apply_unit_animator_transform(
                &canvas,
                text,
                layout,
                animator,
                start.min(end),
                start.max(end),
                time,
                layer_start,
                unit_scale,
            );
        } else {
            apply_range_opacity(
                &mut canvas,
                text,
                layout,
                animator,
                start.min(end),
                start.max(end),
                time,
            );
        }
    }
    canvas
}

fn apply_range_opacity(
    canvas: &mut Canvas,
    text: &str,
    layout: Option<&TextLayoutResult>,
    animator: &TextAnimatorSpec,
    start_percent: f32,
    end_percent: f32,
    time: f64,
) {
    if animator.opacity >= 99.999 && animator.selector.wiggly.is_none() {
        return;
    }

    let units = unit_rects(canvas, text, layout, animator.selector.based_on);
    for unit in units {
        let weight = selector_weight(
            unit.index,
            unit.total,
            &animator.selector,
            start_percent,
            end_percent,
            time,
        );
        if weight <= 0.0 {
            continue;
        }
        let alpha_scale = animator_alpha_scale(animator.opacity, weight);
        scale_alpha_rect(canvas, unit.x0, unit.y0, unit.x1, unit.y1, alpha_scale);
    }
}

#[derive(Debug, Clone, Copy)]
enum UnitMode {
    Characters,
    Words,
}

fn unit_count(line: &str, mode: UnitMode) -> usize {
    match mode {
        UnitMode::Characters => line.chars().filter(|ch| !ch.is_whitespace()).count(),
        UnitMode::Words => line.split_whitespace().count(),
    }
}

fn alpha_bounds_x(canvas: &Canvas, y0: u32, y1: u32) -> Option<(u32, u32)> {
    let mut min_x = canvas.width;
    let mut max_x = 0_u32;
    for y in y0..y1 {
        for x in 0..canvas.width {
            if canvas.pixel(x, y)[3] > 0 {
                min_x = min_x.min(x);
                max_x = max_x.max(x + 1);
            }
        }
    }
    (min_x < max_x).then_some((min_x, max_x))
}

fn scale_alpha_rect(canvas: &mut Canvas, x0: u32, y0: u32, x1: u32, y1: u32, alpha_scale: f32) {
    for y in y0..y1 {
        for x in x0..x1 {
            let mut pixel = canvas.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            pixel[3] = (pixel[3] as f32 * alpha_scale).round().clamp(0.0, 255.0) as u8;
            canvas.set_pixel(x, y, pixel);
        }
    }
}

fn animator_alpha_scale(opacity_percent: f32, weight: f32) -> f32 {
    (1.0 + ((opacity_percent / 100.0) - 1.0) * weight).clamp(0.0, 2.0)
}

fn selector_weight(
    index: usize,
    total: usize,
    selector: &render_ir::TextRangeSelector,
    start_percent: f32,
    end_percent: f32,
    time: f64,
) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let ordered_index = selector_order_index(index, total, selector);
    let pos = unit_center_percent(ordered_index, total);
    let start = start_percent.min(end_percent);
    let end = start_percent.max(end_percent);
    if matches!(selector.shape, TextSelectorShape::Square) {
        let unit_start = ordered_index as f32 * 100.0 / total as f32;
        let unit_end = (ordered_index + 1) as f32 * 100.0 / total as f32;
        let overlap = unit_end.min(end) - unit_start.max(start);
        if overlap <= 0.0 {
            return 0.0;
        }
        let unit_span = (unit_end - unit_start).max(0.0001);
        return (overlap / unit_span).clamp(0.0, 1.0);
    }
    if pos < start || pos > end {
        return 0.0;
    }
    let span = (end - start).max(0.0001);
    let t = ((pos - start) / span).clamp(0.0, 1.0);
    let mut weight = match selector.shape {
        TextSelectorShape::Square => 1.0,
        TextSelectorShape::RampUp => t,
        TextSelectorShape::RampDown => 1.0 - t,
        TextSelectorShape::Triangle => (1.0 - (2.0 * t - 1.0).abs()).clamp(0.0, 1.0),
        TextSelectorShape::Round => (std::f32::consts::PI * t).sin().max(0.0),
        TextSelectorShape::Smooth => t * t * (3.0 - 2.0 * t),
    };
    if let Some(wiggly) = selector.wiggly {
        let phase = deterministic_unit_noise(index, wiggly.seed) * std::f32::consts::TAU
            + time as f32 * wiggly.frequency * std::f32::consts::TAU;
        weight *= 1.0 + (wiggly.amount / 100.0) * phase.sin();
    }
    weight.clamp(0.0, 1.0)
}

fn selector_order_index(
    index: usize,
    total: usize,
    selector: &render_ir::TextRangeSelector,
) -> usize {
    if selector.randomize_order {
        deterministic_order_index(
            index,
            total,
            selector.wiggly.map(|wiggly| wiggly.seed).unwrap_or(0),
        )
    } else {
        index
    }
}

fn unit_center_percent(index: usize, total: usize) -> f32 {
    ((index as f32 + 0.5) / total as f32) * 100.0
}

fn deterministic_order_index(index: usize, total: usize, seed: u32) -> usize {
    let score = deterministic_hash(index as u32 ^ seed);
    let mut rank = 0_usize;
    for other in 0..total {
        let other_score = deterministic_hash(other as u32 ^ seed);
        if other_score < score || (other_score == score && other < index) {
            rank += 1;
        }
    }
    rank
}

fn deterministic_unit_noise(index: usize, seed: u32) -> f32 {
    let hash = deterministic_hash(index as u32 ^ seed);
    hash as f32 / u32::MAX as f32
}

fn deterministic_hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

#[derive(Debug, Clone, Copy)]
struct UnitRect {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    index: usize,
    total: usize,
    layout_center: Option<[f32; 2]>,
    layout_bottom_center: Option<[f32; 2]>,
    telemetry_center: Option<[f32; 2]>,
    telemetry_bottom_center: Option<[f32; 2]>,
}

fn apply_unit_animator_transform(
    canvas: &Canvas,
    text: &str,
    layout: Option<&TextLayoutResult>,
    animator: &TextAnimatorSpec,
    start_percent: f32,
    end_percent: f32,
    time: f64,
    layer_start: f64,
    unit_scale: f32,
) -> Canvas {
    let units = unit_rects(canvas, text, layout, animator.selector.based_on);
    if units.is_empty() {
        return canvas.clone();
    }

    let mut output = Canvas::transparent(canvas.width, canvas.height);
    for unit in units {
        let range_weight = selector_weight(
            unit.index,
            unit.total,
            &animator.selector,
            start_percent,
            end_percent,
            time,
        );
        let expression_weight = text_expression_weight(
            animator.expression_selector.as_ref(),
            unit.index,
            unit.total,
            time,
            layer_start,
        );
        let weight = range_weight * expression_weight;
        draw_transformed_unit(&mut output, canvas, unit, animator, weight, unit_scale);
    }
    output
}

fn text_expression_weight(
    selector: Option<&TextExpressionSelector>,
    index: usize,
    total: usize,
    time: f64,
    layer_start: f64,
) -> f32 {
    text_expression_weight_detail(selector, index, total, time, layer_start).weight
}

#[derive(Debug, Clone, Copy)]
struct TextExpressionWeightDetail {
    weight: f32,
    raw_amount: Option<f32>,
    clamped_amount: Option<f32>,
    local_time_after_delay: Option<f32>,
    text_index: Option<usize>,
    text_total: usize,
}

impl TextExpressionWeightDetail {
    fn to_json(self) -> Value {
        json!({
            "weight": self.weight,
            "raw_amount": self.raw_amount,
            "clamped_amount": self.clamped_amount,
            "local_time_after_delay": self.local_time_after_delay,
            "text_index": self.text_index,
            "text_total": self.text_total
        })
    }
}

fn text_expression_weight_detail(
    selector: Option<&TextExpressionSelector>,
    index: usize,
    total: usize,
    time: f64,
    layer_start: f64,
) -> TextExpressionWeightDetail {
    match selector {
        Some(TextExpressionSelector::PerCharacterBounce {
            delay,
            freq,
            amplitude,
            decay,
            pre_delay_amount,
            ..
        }) => {
            let text_index = index as f32 + 1.0;
            let t = (time - layer_start) as f32 - *delay * text_index;
            if t < 0.0 {
                let amount = pre_delay_amount.unwrap_or(0.0);
                let weight = (amount / 100.0).clamp(-2.0, 2.0);
                return TextExpressionWeightDetail {
                    weight,
                    raw_amount: Some(amount),
                    clamped_amount: Some(weight * 100.0),
                    local_time_after_delay: Some(t),
                    text_index: Some(index + 1),
                    text_total: total,
                };
            }
            let amount =
                *amplitude * (freq * t * 2.0 * std::f32::consts::PI).cos() / (*decay * t).exp();
            let weight = (amount / 100.0).clamp(-2.0, 2.0);
            TextExpressionWeightDetail {
                weight,
                raw_amount: Some(amount),
                clamped_amount: Some(weight * 100.0),
                local_time_after_delay: Some(t),
                text_index: Some(index + 1),
                text_total: total,
            }
        }
        None => TextExpressionWeightDetail {
            weight: 1.0,
            raw_amount: None,
            clamped_amount: None,
            local_time_after_delay: None,
            text_index: None,
            text_total: total,
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct AnimatorUnitTransform {
    center: [f32; 2],
    position: [f32; 2],
    scale: [f32; 2],
    rotation: f32,
    alpha_scale: f32,
    blur_radii: [i32; 2],
    blur_sigmas: [f32; 2],
    matrix: Mat3,
}

const TEXT_ANIMATOR_BLUR_RADIUS_LIMIT: i32 = 128;
const TEXT_ANIMATOR_BLUR_RADIUS_AE85_GAIN: f32 = 0.8;
const TEXT_ANIMATOR_BLUR_SIGMA_AE85_GAIN: f32 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextAnimatorBlurKernel {
    RectSplat,
    Gaussian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextAnimatorUnitPivotMode {
    BottomCenter,
    RectCenter,
    LayoutCenter,
    LayoutBottomCenter,
    TelemetryCenter,
    TelemetryBottomCenter,
}

fn text_animator_unit_pivot_mode() -> TextAnimatorUnitPivotMode {
    #[cfg(debug_assertions)]
    {
        return match std::env::var("AE_NATIVE_M07_UNIT_PIVOT").as_deref() {
            Ok("bottom_center") | Ok("integer_bottom_center") | Ok("legacy") => {
                TextAnimatorUnitPivotMode::BottomCenter
            }
            Ok("rect_center") | Ok("center") => TextAnimatorUnitPivotMode::RectCenter,
            Ok("layout_center") | Ok("glyph_center") => TextAnimatorUnitPivotMode::LayoutCenter,
            Ok("layout_bottom_center") | Ok("glyph_bottom_center") => {
                TextAnimatorUnitPivotMode::LayoutBottomCenter
            }
            Ok("telemetry_center") | Ok("cooltype_center") => {
                TextAnimatorUnitPivotMode::TelemetryCenter
            }
            Ok("telemetry_bottom_center") | Ok("cooltype_bottom_center") => {
                TextAnimatorUnitPivotMode::TelemetryBottomCenter
            }
            _ => TextAnimatorUnitPivotMode::LayoutBottomCenter,
        };
    }
    #[cfg(not(debug_assertions))]
    {
        TextAnimatorUnitPivotMode::LayoutBottomCenter
    }
}

fn text_animator_unit_pivot(unit: UnitRect) -> [f32; 2] {
    let rect_center = || {
        [
            (unit.x0 + unit.x1) as f32 * 0.5,
            (unit.y0 + unit.y1) as f32 * 0.5,
        ]
    };
    let rect_bottom_center = || [(unit.x0 + unit.x1) as f32 * 0.5, unit.y1 as f32];
    match text_animator_unit_pivot_mode() {
        TextAnimatorUnitPivotMode::BottomCenter => rect_bottom_center(),
        TextAnimatorUnitPivotMode::RectCenter => rect_center(),
        TextAnimatorUnitPivotMode::LayoutCenter => unit.layout_center.unwrap_or_else(rect_center),
        TextAnimatorUnitPivotMode::LayoutBottomCenter => {
            unit.layout_bottom_center.unwrap_or_else(rect_bottom_center)
        }
        TextAnimatorUnitPivotMode::TelemetryCenter => {
            unit.telemetry_center.unwrap_or_else(rect_center)
        }
        TextAnimatorUnitPivotMode::TelemetryBottomCenter => unit
            .telemetry_bottom_center
            .unwrap_or_else(rect_bottom_center),
    }
}

fn animator_unit_transform(
    unit: UnitRect,
    animator: &TextAnimatorSpec,
    weight: f32,
    unit_scale: f32,
) -> AnimatorUnitTransform {
    let [cx, cy] = text_animator_unit_pivot(unit);
    let position = animator.position.unwrap_or([0.0, 0.0]);
    let scale = animator.scale.unwrap_or([100.0, 100.0]);
    let sx = (100.0 + (scale[0] - 100.0) * weight) / 100.0;
    let sy = (100.0 + (scale[1] - 100.0) * weight) / 100.0;
    let rotation = animator.rotation.unwrap_or(0.0) * weight;
    let tx = position[0] * weight * unit_scale;
    let ty = position[1] * weight * unit_scale;
    let alpha_scale = animator_alpha_scale(animator.opacity, weight);
    let blur_sigmas = text_animator_blur_sigmas(animator, weight, unit_scale);
    let blur_radii = text_animator_blur_radii(animator, weight, unit_scale);
    let matrix = Mat3::translate(Vec2::new(cx + tx, cy + ty))
        .mul(Mat3::rotate_degrees(rotation))
        .mul(Mat3::scale(Vec2::new(sx, sy)))
        .mul(Mat3::translate(Vec2::new(-cx, -cy)));

    AnimatorUnitTransform {
        center: [cx, cy],
        position: [tx, ty],
        scale: [sx, sy],
        rotation,
        alpha_scale,
        blur_radii,
        blur_sigmas,
        matrix,
    }
}

fn text_animator_blur_radii(animator: &TextAnimatorSpec, weight: f32, unit_scale: f32) -> [i32; 2] {
    let blur_sigmas = text_animator_blur_sigmas(animator, weight, unit_scale);
    text_animator_blur_radii_for_kernel(
        animator,
        weight,
        unit_scale,
        text_animator_blur_kernel(),
        blur_sigmas,
    )
}

fn text_animator_blur_radii_for_kernel(
    animator: &TextAnimatorSpec,
    weight: f32,
    unit_scale: f32,
    kernel: TextAnimatorBlurKernel,
    blur_sigmas: [f32; 2],
) -> [i32; 2] {
    if kernel == TextAnimatorBlurKernel::Gaussian {
        return [
            gaussian_kernel_radius(blur_sigmas[0]),
            gaussian_kernel_radius(blur_sigmas[1]),
        ];
    }
    animator
        .blur
        .map(|blur| {
            [
                text_animator_blur_axis_radius(blur[0], weight, unit_scale),
                text_animator_blur_axis_radius(blur[1], weight, unit_scale),
            ]
        })
        .unwrap_or([0, 0])
}

fn text_animator_blur_sigmas(
    animator: &TextAnimatorSpec,
    weight: f32,
    unit_scale: f32,
) -> [f32; 2] {
    animator
        .blur
        .map(|blur| {
            [
                text_animator_blur_axis_sigma(blur[0], weight, unit_scale),
                text_animator_blur_axis_sigma(blur[1], weight, unit_scale),
            ]
        })
        .unwrap_or([0.0, 0.0])
}

fn text_animator_blur_axis_radius(value: f32, weight: f32, unit_scale: f32) -> i32 {
    ((value.abs() * weight.max(0.0) * unit_scale.abs() * text_animator_blur_radius_gain()).round()
        as i32)
        .clamp(0, TEXT_ANIMATOR_BLUR_RADIUS_LIMIT)
}

fn text_animator_blur_axis_sigma(value: f32, weight: f32, unit_scale: f32) -> f32 {
    value.abs() * weight.max(0.0) * unit_scale.abs() * text_animator_blur_sigma_gain()
}

fn gaussian_kernel_radius(sigma: f32) -> i32 {
    if sigma < 0.001 {
        0
    } else {
        (sigma * 3.0).ceil() as i32
    }
    .clamp(0, TEXT_ANIMATOR_BLUR_RADIUS_LIMIT)
}

fn text_animator_blur_kernel() -> TextAnimatorBlurKernel {
    #[cfg(debug_assertions)]
    {
        return match std::env::var("AE_NATIVE_M07_BLUR_KERNEL").as_deref() {
            Ok("rect") | Ok("rect_splat") | Ok("splat") | Ok("box_splat") => {
                TextAnimatorBlurKernel::RectSplat
            }
            Ok("gaussian") | Ok("separable_gaussian") => TextAnimatorBlurKernel::Gaussian,
            _ => TextAnimatorBlurKernel::Gaussian,
        };
    }
    #[cfg(not(debug_assertions))]
    {
        TextAnimatorBlurKernel::Gaussian
    }
}

fn text_animator_blur_radius_gain() -> f32 {
    #[cfg(debug_assertions)]
    {
        return std::env::var("AE_NATIVE_M07_BLUR_GAIN")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(TEXT_ANIMATOR_BLUR_RADIUS_AE85_GAIN);
    }
    #[cfg(not(debug_assertions))]
    {
        TEXT_ANIMATOR_BLUR_RADIUS_AE85_GAIN
    }
}

fn text_animator_blur_sigma_gain() -> f32 {
    #[cfg(debug_assertions)]
    {
        return std::env::var("AE_NATIVE_M07_BLUR_SIGMA_GAIN")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(TEXT_ANIMATOR_BLUR_SIGMA_AE85_GAIN);
    }
    #[cfg(not(debug_assertions))]
    {
        TEXT_ANIMATOR_BLUR_SIGMA_AE85_GAIN
    }
}

fn text_animator_use_preblur_inverse_stage() -> bool {
    #[cfg(debug_assertions)]
    {
        return std::env::var("AE_NATIVE_M07_BLUR_STAGE")
            .is_ok_and(|value| value == "preblur_inverse");
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

fn text_animator_use_blur_alpha_area() -> bool {
    #[cfg(debug_assertions)]
    {
        return std::env::var("AE_NATIVE_M07_BLUR_ALPHA_AREA")
            .is_ok_and(|value| value == "1" || value == "true");
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

fn text_animator_blur_alpha_gain() -> f32 {
    #[cfg(debug_assertions)]
    {
        return std::env::var("AE_NATIVE_M07_BLUR_ALPHA_GAIN")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(1.0);
    }
    #[cfg(not(debug_assertions))]
    {
        1.0
    }
}

fn text_animator_use_blur_alpha_post_scale() -> bool {
    #[cfg(debug_assertions)]
    {
        return std::env::var("AE_NATIVE_M07_BLUR_ALPHA_POST")
            .is_ok_and(|value| value == "1" || value == "true");
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

fn text_animator_use_blur_float_accum() -> bool {
    #[cfg(debug_assertions)]
    {
        return std::env::var("AE_NATIVE_M07_BLUR_FLOAT_ACCUM")
            .is_ok_and(|value| value == "1" || value == "true");
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

fn text_animator_blur_alpha_scale(transform: AnimatorUnitTransform) -> f32 {
    let area = if text_animator_use_blur_alpha_area() {
        transform.scale[0].abs() * transform.scale[1].abs()
    } else {
        1.0
    };
    transform.alpha_scale * area * text_animator_blur_alpha_gain()
}

fn splat_text_animator_blur_pixel(
    output: &mut Canvas,
    x: i32,
    y: i32,
    pixel: [u8; 4],
    radius_x: i32,
    radius_y: i32,
    alpha_scale: f32,
) {
    if text_animator_use_blur_alpha_post_scale() {
        splat_blurred_pixel_alpha_scaled(output, x, y, pixel, radius_x, radius_y, alpha_scale);
        return;
    }
    let mut pixel = pixel;
    pixel[3] = (pixel[3] as f32 * alpha_scale).round().clamp(0.0, 255.0) as u8;
    splat_blurred_pixel(output, x, y, pixel, radius_x, radius_y);
}

fn draw_transformed_unit_preblur_inverse(
    output: &mut Canvas,
    input: &Canvas,
    unit: UnitRect,
    transform: AnimatorUnitTransform,
) {
    let mut blurred = Canvas::transparent(input.width, input.height);
    let alpha_scale = text_animator_blur_alpha_scale(transform);
    for y in unit.y0..unit.y1 {
        for x in unit.x0..unit.x1 {
            let pixel = input.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            splat_text_animator_blur_pixel(
                &mut blurred,
                x as i32,
                y as i32,
                pixel,
                transform.blur_radii[0],
                transform.blur_radii[1],
                alpha_scale,
            );
        }
    }

    let Some(inverse) = transform.matrix.inverse() else {
        return;
    };
    let source_unit = expanded_unit_for_blur(input, unit, transform.blur_radii);
    let Some((x0, y0, x1, y1)) =
        transformed_unit_bounds(input, source_unit, transform.matrix, [0, 0])
    else {
        return;
    };
    for y in y0..y1 {
        for x in x0..x1 {
            let local = inverse.transform_point(Vec2::new(x as f32, y as f32));
            let sx = local.x.round() as i32;
            let sy = local.y.round() as i32;
            if sx < source_unit.x0 as i32
                || sy < source_unit.y0 as i32
                || sx >= source_unit.x1 as i32
                || sy >= source_unit.y1 as i32
            {
                continue;
            }
            let pixel = BilinearSampler.sample(&blurred, local.x, local.y);
            if pixel[3] == 0 {
                continue;
            }
            let existing = output.pixel(x as u32, y as u32);
            output.set_pixel(
                x as u32,
                y as u32,
                composite_normal_pixel(existing, pixel, 100.0),
            );
        }
    }
}

fn animator_contribution_json(
    unit: UnitRect,
    animator: &TextAnimatorSpec,
    weight: f32,
    unit_scale: f32,
) -> Value {
    let transform = animator_unit_transform(unit, animator, weight, unit_scale);
    json!({
        "center": transform.center,
        "center_mode": format!("{:?}", text_animator_unit_pivot_mode()),
        "unit_layout_center": unit.layout_center,
        "unit_layout_bottom_center": unit.layout_bottom_center,
        "unit_telemetry_center": unit.telemetry_center,
        "unit_telemetry_bottom_center": unit.telemetry_bottom_center,
        "position": transform.position,
        "scale": transform.scale,
        "rotation": transform.rotation,
        "opacity_alpha_scale": transform.alpha_scale,
        "blur_radius": transform.blur_radii[0].max(transform.blur_radii[1]),
        "blur_radii": transform.blur_radii,
        "matrix": transform.matrix.m,
        "final_matrix": transform.matrix.m,
        "final_opacity_alpha_scale": transform.alpha_scale,
        "blur_radius_px": transform.blur_radii[0].max(transform.blur_radii[1]),
        "blur_radius_px_xy": transform.blur_radii
    })
}

fn selector_unit_glyph_passport(
    layout: Option<&TextLayoutResult>,
    based_on: TextSelectorBasedOn,
    unit_index: usize,
) -> Value {
    let Some(layout) = layout else {
        return json!({
            "available": false,
            "reason": "layout_unavailable"
        });
    };
    let units = telemetry_glyph_units(&layout.telemetry.glyphs, based_on);
    let Some(glyphs) = units.get(unit_index) else {
        return json!({
            "available": false,
            "reason": "unit_without_layout_glyphs",
            "layout_glyph_count": layout.telemetry.glyphs.len()
        });
    };
    glyph_passport_json(glyphs)
}

fn telemetry_glyph_units(
    glyphs: &[GlyphLayoutTelemetry],
    based_on: TextSelectorBasedOn,
) -> Vec<Vec<&GlyphLayoutTelemetry>> {
    match based_on {
        TextSelectorBasedOn::Characters => glyphs
            .iter()
            .filter(|glyph| !telemetry_glyph_is_whitespace(glyph))
            .map(|glyph| vec![glyph])
            .collect(),
        TextSelectorBasedOn::Words => grouped_telemetry_glyph_units(glyphs, LayoutGroup::Word),
        TextSelectorBasedOn::Lines => grouped_telemetry_glyph_units(glyphs, LayoutGroup::Line),
    }
}

fn grouped_telemetry_glyph_units(
    glyphs: &[GlyphLayoutTelemetry],
    group: LayoutGroup,
) -> Vec<Vec<&GlyphLayoutTelemetry>> {
    let mut grouped = BTreeMap::<usize, Vec<&GlyphLayoutTelemetry>>::new();
    for glyph in glyphs {
        if telemetry_glyph_is_whitespace(glyph) {
            continue;
        }
        let key = match group {
            LayoutGroup::Word => glyph.word_index,
            LayoutGroup::Line => glyph.line_index,
        };
        grouped.entry(key).or_default().push(glyph);
    }
    grouped.into_values().collect()
}

fn telemetry_glyph_is_whitespace(glyph: &GlyphLayoutTelemetry) -> bool {
    glyph.character.chars().all(char::is_whitespace)
}

fn glyph_passport_json(glyphs: &[&GlyphLayoutTelemetry]) -> Value {
    json!({
        "available": true,
        "glyph_count": glyphs.len(),
        "metric_source": collapsed_text_values(
            glyphs.iter().map(|glyph| glyph.metric_source.as_str())
        ),
        "cooltype_reference_status": collapsed_text_values(
            glyphs.iter().map(|glyph| glyph.cooltype_reference_status.as_str())
        ),
        "glyph_run_indices": glyphs.iter().map(|glyph| glyph.glyph_run_index).collect::<Vec<_>>(),
        "char_indices": glyphs.iter().map(|glyph| glyph.char_index).collect::<Vec<_>>(),
        "word_indices": glyphs.iter().map(|glyph| glyph.word_index).collect::<Vec<_>>(),
        "line_indices": glyphs.iter().map(|glyph| glyph.line_index).collect::<Vec<_>>(),
        "font_glyph_ids": glyphs.iter().map(|glyph| glyph.font_glyph_id).collect::<Vec<_>>(),
        "characters": glyphs.iter().map(|glyph| glyph.character.clone()).collect::<Vec<_>>(),
        "advances": glyphs.iter().map(|glyph| glyph.advance).collect::<Vec<_>>(),
        "advance_x": glyphs.iter().map(|glyph| glyph.advance_x).collect::<Vec<_>>(),
        "advance_y": glyphs.iter().map(|glyph| glyph.advance_y).collect::<Vec<_>>(),
        "bboxes": glyphs.iter().map(|glyph| glyph.bbox).collect::<Vec<_>>(),
        "cooltype_bbox_minmax": glyphs
            .iter()
            .map(|glyph| glyph.cooltype_bbox_minmax)
            .collect::<Vec<_>>(),
        "bbox_centers": glyphs.iter().map(|glyph| glyph.bbox_center).collect::<Vec<_>>(),
        "baselines": glyphs.iter().map(|glyph| glyph.baseline).collect::<Vec<_>>(),
        "baseline_deltas": glyphs
            .iter()
            .map(|glyph| glyph.baseline_delta)
            .collect::<Vec<_>>(),
        "font_paths": glyphs
            .iter()
            .map(|glyph| glyph.font_path.as_ref().map(|path| path.display().to_string()))
            .collect::<Vec<_>>(),
        "font_postscript_names": glyphs
            .iter()
            .map(|glyph| glyph.font_postscript_name.clone())
            .collect::<Vec<_>>()
    })
}

fn collapsed_text_values<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let mut unique = Vec::<&str>::new();
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    match unique.as_slice() {
        [] => "unavailable".to_string(),
        [single] => (*single).to_string(),
        _ => "mixed".to_string(),
    }
}

fn draw_transformed_unit(
    output: &mut Canvas,
    input: &Canvas,
    unit: UnitRect,
    animator: &TextAnimatorSpec,
    weight: f32,
    unit_scale: f32,
) {
    let transform = animator_unit_transform(unit, animator, weight, unit_scale);
    if transform.blur_radii != [0, 0] {
        if text_animator_use_preblur_inverse_stage() {
            draw_transformed_unit_preblur_inverse(output, input, unit, transform);
            return;
        }
        draw_transformed_unit_forward(output, input, unit, transform);
        return;
    }
    let Some(inverse) = transform.matrix.inverse() else {
        return;
    };
    let Some((x0, y0, x1, y1)) =
        transformed_unit_bounds(input, unit, transform.matrix, transform.blur_radii)
    else {
        return;
    };

    for y in y0..y1 {
        for x in x0..x1 {
            let local = inverse.transform_point(Vec2::new(x as f32, y as f32));
            let sx = local.x.round() as i32;
            let sy = local.y.round() as i32;
            if sx < unit.x0 as i32
                || sy < unit.y0 as i32
                || sx >= unit.x1 as i32
                || sy >= unit.y1 as i32
            {
                continue;
            }
            let mut pixel = BilinearSampler.sample(input, local.x, local.y);
            if pixel[3] == 0 {
                continue;
            }
            pixel[3] = (pixel[3] as f32 * transform.alpha_scale)
                .round()
                .clamp(0.0, 255.0) as u8;
            splat_blurred_pixel(
                output,
                x,
                y,
                pixel,
                transform.blur_radii[0],
                transform.blur_radii[1],
            );
        }
    }
}

fn expanded_unit_for_blur(canvas: &Canvas, unit: UnitRect, blur_radii: [i32; 2]) -> UnitRect {
    let rx = blur_radii[0].max(0) as u32;
    let ry = blur_radii[1].max(0) as u32;
    UnitRect {
        x0: unit.x0.saturating_sub(rx),
        y0: unit.y0.saturating_sub(ry),
        x1: unit.x1.saturating_add(rx).min(canvas.width),
        y1: unit.y1.saturating_add(ry).min(canvas.height),
        index: unit.index,
        total: unit.total,
        layout_center: unit.layout_center,
        layout_bottom_center: unit.layout_bottom_center,
        telemetry_center: unit.telemetry_center,
        telemetry_bottom_center: unit.telemetry_bottom_center,
    }
}

fn draw_transformed_unit_forward(
    output: &mut Canvas,
    input: &Canvas,
    unit: UnitRect,
    transform: AnimatorUnitTransform,
) {
    let alpha_scale = text_animator_blur_alpha_scale(transform);
    if text_animator_blur_kernel() == TextAnimatorBlurKernel::Gaussian {
        draw_transformed_unit_forward_gaussian(output, input, unit, transform, alpha_scale);
        return;
    }
    if text_animator_use_blur_float_accum() {
        draw_transformed_unit_forward_float_accum(output, input, unit, transform, alpha_scale);
        return;
    }
    for y in unit.y0..unit.y1 {
        for x in unit.x0..unit.x1 {
            let pixel = input.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            let p = transform
                .matrix
                .transform_point(Vec2::new(x as f32, y as f32));
            splat_text_animator_blur_pixel(
                output,
                p.x.round() as i32,
                p.y.round() as i32,
                pixel,
                transform.blur_radii[0],
                transform.blur_radii[1],
                alpha_scale,
            );
        }
    }
}

fn draw_transformed_unit_forward_gaussian(
    output: &mut Canvas,
    input: &Canvas,
    unit: UnitRect,
    transform: AnimatorUnitTransform,
    alpha_scale: f32,
) {
    let kernel_x = gaussian_kernel_offsets(transform.blur_sigmas[0]);
    let kernel_y = gaussian_kernel_offsets(transform.blur_sigmas[1]);
    let len = output.width as usize * output.height as usize;
    let mut alpha_accum = vec![0.0_f32; len];
    let mut color_accum = vec![[0.0_f32; 3]; len];
    for y in unit.y0..unit.y1 {
        for x in unit.x0..unit.x1 {
            let pixel = input.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            let p = transform
                .matrix
                .transform_point(Vec2::new(x as f32, y as f32));
            splat_gaussian_pixel_float_accum(
                &mut alpha_accum,
                &mut color_accum,
                output.width,
                output.height,
                p.x.round() as i32,
                p.y.round() as i32,
                pixel,
                &kernel_x,
                &kernel_y,
                alpha_scale,
            );
        }
    }

    for (index, alpha) in alpha_accum.into_iter().enumerate() {
        let alpha = alpha.round().clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            continue;
        }
        let x = (index % output.width as usize) as u32;
        let y = (index / output.width as usize) as u32;
        let existing = output.pixel(x, y);
        let colors = color_accum[index];
        let denom = alpha.max(1) as f32;
        let color = [
            (colors[0] / denom).round().clamp(0.0, 255.0) as u8,
            (colors[1] / denom).round().clamp(0.0, 255.0) as u8,
            (colors[2] / denom).round().clamp(0.0, 255.0) as u8,
        ];
        output.set_pixel(
            x,
            y,
            [
                color[0],
                color[1],
                color[2],
                existing[3].saturating_add(alpha),
            ],
        );
    }
}

fn draw_transformed_unit_forward_float_accum(
    output: &mut Canvas,
    input: &Canvas,
    unit: UnitRect,
    transform: AnimatorUnitTransform,
    alpha_scale: f32,
) {
    let len = output.width as usize * output.height as usize;
    let mut alpha_accum = vec![0.0_f32; len];
    let mut colors = vec![[0_u8; 3]; len];
    for y in unit.y0..unit.y1 {
        for x in unit.x0..unit.x1 {
            let pixel = input.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            let p = transform
                .matrix
                .transform_point(Vec2::new(x as f32, y as f32));
            splat_blurred_pixel_float_accum(
                &mut alpha_accum,
                &mut colors,
                output.width,
                output.height,
                p.x.round() as i32,
                p.y.round() as i32,
                pixel,
                transform.blur_radii[0],
                transform.blur_radii[1],
                alpha_scale,
            );
        }
    }

    for (index, alpha) in alpha_accum.into_iter().enumerate() {
        let alpha = alpha.round().clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            continue;
        }
        let x = (index % output.width as usize) as u32;
        let y = (index / output.width as usize) as u32;
        let existing = output.pixel(x, y);
        let color = colors[index];
        output.set_pixel(
            x,
            y,
            [
                color[0],
                color[1],
                color[2],
                existing[3].saturating_add(alpha),
            ],
        );
    }
}

fn transformed_unit_bounds(
    canvas: &Canvas,
    unit: UnitRect,
    matrix: Mat3,
    blur_radii: [i32; 2],
) -> Option<(i32, i32, i32, i32)> {
    if unit.x0 >= unit.x1 || unit.y0 >= unit.y1 {
        return None;
    }
    let left = unit.x0 as f32;
    let top = unit.y0 as f32;
    let right = unit.x1 as f32;
    let bottom = unit.y1 as f32;
    let points = [
        matrix.transform_point(Vec2::new(left, top)),
        matrix.transform_point(Vec2::new(right, top)),
        matrix.transform_point(Vec2::new(right, bottom)),
        matrix.transform_point(Vec2::new(left, bottom)),
    ];
    let min_x = points
        .iter()
        .map(|p| p.x)
        .fold(f32::INFINITY, f32::min)
        .floor() as i32
        - 1;
    let max_x = points
        .iter()
        .map(|p| p.x)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil() as i32
        + 1;
    let min_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min)
        .floor() as i32
        - 1;
    let max_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil() as i32
        + 1;
    let rx = blur_radii[0].max(0) + 1;
    let ry = blur_radii[1].max(0) + 1;
    let x0 = min_x.clamp(-rx, canvas.width as i32 + rx);
    let y0 = min_y.clamp(-ry, canvas.height as i32 + ry);
    let x1 = max_x.clamp(-rx, canvas.width as i32 + rx);
    let y1 = max_y.clamp(-ry, canvas.height as i32 + ry);
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

fn splat_blurred_pixel(
    output: &mut Canvas,
    x: i32,
    y: i32,
    pixel: [u8; 4],
    radius_x: i32,
    radius_y: i32,
) {
    let radius_x = radius_x.max(0);
    let radius_y = radius_y.max(0);
    if radius_x == 0 && radius_y == 0 {
        if x < 0 || y < 0 || x >= output.width as i32 || y >= output.height as i32 {
            return;
        }
        let existing = output.pixel(x as u32, y as u32);
        output.set_pixel(
            x as u32,
            y as u32,
            composite_normal_pixel(existing, pixel, 100.0),
        );
        return;
    }
    let divisor = ((radius_x * 2 + 1) * (radius_y * 2 + 1)).max(1) as f32;
    for oy in -radius_y..=radius_y {
        for ox in -radius_x..=radius_x {
            let dx = x + ox;
            let dy = y + oy;
            if dx < 0 || dy < 0 || dx >= output.width as i32 || dy >= output.height as i32 {
                continue;
            }
            let mut blurred = pixel;
            let alpha = (blurred[3] as f32 / divisor).round().clamp(0.0, 255.0) as u8;
            if alpha == 0 {
                continue;
            }
            blurred[3] = alpha;
            let existing = output.pixel(dx as u32, dy as u32);
            let combined_alpha = existing[3].saturating_add(blurred[3]);
            blurred[3] = combined_alpha;
            output.set_pixel(dx as u32, dy as u32, blurred);
        }
    }
}

fn splat_blurred_pixel_alpha_scaled(
    output: &mut Canvas,
    x: i32,
    y: i32,
    pixel: [u8; 4],
    radius_x: i32,
    radius_y: i32,
    alpha_scale: f32,
) {
    let radius_x = radius_x.max(0);
    let radius_y = radius_y.max(0);
    if radius_x == 0 && radius_y == 0 {
        if x < 0 || y < 0 || x >= output.width as i32 || y >= output.height as i32 {
            return;
        }
        let mut scaled = pixel;
        scaled[3] = (scaled[3] as f32 * alpha_scale).round().clamp(0.0, 255.0) as u8;
        let existing = output.pixel(x as u32, y as u32);
        output.set_pixel(
            x as u32,
            y as u32,
            composite_normal_pixel(existing, scaled, 100.0),
        );
        return;
    }
    let divisor = ((radius_x * 2 + 1) * (radius_y * 2 + 1)).max(1) as f32;
    for oy in -radius_y..=radius_y {
        for ox in -radius_x..=radius_x {
            let dx = x + ox;
            let dy = y + oy;
            if dx < 0 || dy < 0 || dx >= output.width as i32 || dy >= output.height as i32 {
                continue;
            }
            let mut blurred = pixel;
            let alpha = (blurred[3] as f32 * alpha_scale / divisor)
                .round()
                .clamp(0.0, 255.0) as u8;
            if alpha == 0 {
                continue;
            }
            blurred[3] = output.pixel(dx as u32, dy as u32)[3].saturating_add(alpha);
            output.set_pixel(dx as u32, dy as u32, blurred);
        }
    }
}

fn splat_blurred_pixel_float_accum(
    alpha_accum: &mut [f32],
    colors: &mut [[u8; 3]],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    pixel: [u8; 4],
    radius_x: i32,
    radius_y: i32,
    alpha_scale: f32,
) {
    let radius_x = radius_x.max(0);
    let radius_y = radius_y.max(0);
    let divisor = ((radius_x * 2 + 1) * (radius_y * 2 + 1)).max(1) as f32;
    let contribution = pixel[3] as f32 * alpha_scale.max(0.0) / divisor;
    if contribution <= 0.0 {
        return;
    }
    for oy in -radius_y..=radius_y {
        for ox in -radius_x..=radius_x {
            let dx = x + ox;
            let dy = y + oy;
            if dx < 0 || dy < 0 || dx >= width as i32 || dy >= height as i32 {
                continue;
            }
            let index = dy as usize * width as usize + dx as usize;
            alpha_accum[index] += contribution;
            colors[index] = [pixel[0], pixel[1], pixel[2]];
        }
    }
}

fn gaussian_kernel_offsets(sigma: f32) -> Vec<(i32, f32)> {
    let radius = gaussian_kernel_radius(sigma);
    if radius == 0 {
        return vec![(0, 1.0)];
    }
    let sigma = sigma.max(0.001);
    let mut values = Vec::with_capacity((radius * 2 + 1) as usize);
    let mut sum = 0.0_f32;
    for offset in -radius..=radius {
        let value = (-(offset * offset) as f32 / (2.0 * sigma * sigma)).exp();
        values.push((offset, value));
        sum += value;
    }
    if sum > 0.0 {
        for (_, value) in &mut values {
            *value /= sum;
        }
    }
    values
}

fn splat_gaussian_pixel_float_accum(
    alpha_accum: &mut [f32],
    color_accum: &mut [[f32; 3]],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    pixel: [u8; 4],
    kernel_x: &[(i32, f32)],
    kernel_y: &[(i32, f32)],
    alpha_scale: f32,
) {
    let base_alpha = pixel[3] as f32 * alpha_scale.max(0.0);
    if base_alpha <= 0.0 {
        return;
    }
    for (oy, wy) in kernel_y {
        for (ox, wx) in kernel_x {
            let dx = x + *ox;
            let dy = y + *oy;
            if dx < 0 || dy < 0 || dx >= width as i32 || dy >= height as i32 {
                continue;
            }
            let contribution = base_alpha * *wx * *wy;
            if contribution <= 0.0 {
                continue;
            }
            let index = dy as usize * width as usize + dx as usize;
            alpha_accum[index] += contribution;
            color_accum[index][0] += pixel[0] as f32 * contribution;
            color_accum[index][1] += pixel[1] as f32 * contribution;
            color_accum[index][2] += pixel[2] as f32 * contribution;
        }
    }
}

fn unit_rects(
    canvas: &Canvas,
    text: &str,
    layout: Option<&TextLayoutResult>,
    based_on: TextSelectorBasedOn,
) -> Vec<UnitRect> {
    if let Some(layout) = layout {
        let units = layout_unit_rects(canvas, text, layout, based_on);
        if !units.is_empty() {
            return units;
        }
    }
    match based_on {
        TextSelectorBasedOn::Lines => line_unit_rects(canvas, text),
        TextSelectorBasedOn::Words => inline_unit_rects(canvas, text, UnitMode::Words),
        TextSelectorBasedOn::Characters => inline_unit_rects(canvas, text, UnitMode::Characters),
    }
}

fn layout_unit_rects(
    canvas: &Canvas,
    text: &str,
    layout: &TextLayoutResult,
    based_on: TextSelectorBasedOn,
) -> Vec<UnitRect> {
    match based_on {
        TextSelectorBasedOn::Characters => layout_character_unit_rects(canvas, text, layout),
        TextSelectorBasedOn::Words => {
            layout_grouped_unit_rects(canvas, text, layout, LayoutGroup::Word)
        }
        TextSelectorBasedOn::Lines => {
            layout_grouped_unit_rects(canvas, text, layout, LayoutGroup::Line)
        }
    }
}

fn layout_character_unit_rects(
    canvas: &Canvas,
    text: &str,
    layout: &TextLayoutResult,
) -> Vec<UnitRect> {
    let rects = layout
        .glyphs
        .iter()
        .enumerate()
        .filter(|(_, glyph)| {
            glyph_char(text, glyph.char_index).is_some_and(|ch| !ch.is_whitespace())
        })
        .map(|(index, glyph)| {
            let telemetry_bbox = layout.telemetry.glyphs.get(index).map(|glyph| glyph.bbox);
            rect_from_bbox_with_pivots(canvas, glyph.bbox, telemetry_bbox)
        })
        .collect::<Vec<_>>();
    with_unit_totals(rects)
}

#[derive(Debug, Clone, Copy)]
enum LayoutGroup {
    Word,
    Line,
}

fn layout_grouped_unit_rects(
    canvas: &Canvas,
    text: &str,
    layout: &TextLayoutResult,
    group: LayoutGroup,
) -> Vec<UnitRect> {
    let mut groups: Vec<(usize, [f32; 4], Option<[f32; 4]>)> = Vec::new();
    for (index, glyph) in layout.glyphs.iter().enumerate() {
        if !glyph_char(text, glyph.char_index).is_some_and(|ch| !ch.is_whitespace()) {
            continue;
        }
        let key = match group {
            LayoutGroup::Word => glyph.word_index,
            LayoutGroup::Line => glyph.line_index,
        };
        let x0 = glyph.bbox[0];
        let y0 = glyph.bbox[1];
        let x1 = glyph.bbox[0] + glyph.bbox[2];
        let y1 = glyph.bbox[1] + glyph.bbox[3];
        let telemetry_bounds = layout.telemetry.glyphs.get(index).map(|glyph| {
            [
                glyph.bbox[0],
                glyph.bbox[1],
                glyph.bbox[0] + glyph.bbox[2],
                glyph.bbox[1] + glyph.bbox[3],
            ]
        });
        if let Some((_, bounds, telemetry)) = groups
            .iter_mut()
            .find(|(group_key, _, _)| *group_key == key)
        {
            bounds[0] = bounds[0].min(x0);
            bounds[1] = bounds[1].min(y0);
            bounds[2] = bounds[2].max(x1);
            bounds[3] = bounds[3].max(y1);
            if let Some(source) = telemetry_bounds {
                match telemetry {
                    Some(current) => {
                        current[0] = current[0].min(source[0]);
                        current[1] = current[1].min(source[1]);
                        current[2] = current[2].max(source[2]);
                        current[3] = current[3].max(source[3]);
                    }
                    None => *telemetry = Some(source),
                }
            }
        } else {
            groups.push((key, [x0, y0, x1, y1], telemetry_bounds));
        }
    }
    groups.sort_by_key(|(key, _, _)| *key);
    let rects = groups
        .into_iter()
        .map(|(_, bounds, telemetry_bounds)| {
            let layout_bbox = [
                bounds[0],
                bounds[1],
                (bounds[2] - bounds[0]).max(0.0),
                (bounds[3] - bounds[1]).max(0.0),
            ];
            let telemetry_bbox = telemetry_bounds.map(|bounds| {
                [
                    bounds[0],
                    bounds[1],
                    (bounds[2] - bounds[0]).max(0.0),
                    (bounds[3] - bounds[1]).max(0.0),
                ]
            });
            rect_from_bbox_with_pivots(canvas, layout_bbox, telemetry_bbox)
        })
        .collect::<Vec<_>>();
    with_unit_totals(rects)
}

fn rect_from_bbox_with_pivots(
    canvas: &Canvas,
    bbox: [f32; 4],
    telemetry_bbox: Option<[f32; 4]>,
) -> UnitRect {
    let x0 = bbox[0].floor().max(0.0).min(canvas.width as f32) as u32;
    let y0 = bbox[1].floor().max(0.0).min(canvas.height as f32) as u32;
    let x1 = (bbox[0] + bbox[2]).ceil().max(0.0).min(canvas.width as f32) as u32;
    let y1 = (bbox[1] + bbox[3])
        .ceil()
        .max(0.0)
        .min(canvas.height as f32) as u32;
    UnitRect {
        x0,
        y0,
        x1,
        y1,
        index: 0,
        total: 0,
        layout_center: Some(bbox_center(bbox)),
        layout_bottom_center: Some(bbox_bottom_center(bbox)),
        telemetry_center: telemetry_bbox.map(bbox_center),
        telemetry_bottom_center: telemetry_bbox.map(bbox_bottom_center),
    }
}

fn bbox_center(bbox: [f32; 4]) -> [f32; 2] {
    [bbox[0] + bbox[2] * 0.5, bbox[1] + bbox[3] * 0.5]
}

fn bbox_bottom_center(bbox: [f32; 4]) -> [f32; 2] {
    [bbox[0] + bbox[2] * 0.5, bbox[1] + bbox[3]]
}

fn with_unit_totals(mut rects: Vec<UnitRect>) -> Vec<UnitRect> {
    let total = rects.len();
    for (index, rect) in rects.iter_mut().enumerate() {
        rect.index = index;
        rect.total = total;
    }
    rects
}

fn glyph_char(text: &str, char_index: usize) -> Option<char> {
    text.chars().nth(char_index)
}

fn line_unit_rects(canvas: &Canvas, text: &str) -> Vec<UnitRect> {
    let total = text.lines().count().max(1);
    let mut units = Vec::new();
    for index in 0..total {
        let y0 = ((index as f32 / total as f32) * canvas.height as f32).floor() as u32;
        let y1 = (((index + 1) as f32 / total as f32) * canvas.height as f32).ceil() as u32;
        units.push(UnitRect {
            x0: 0,
            y0,
            x1: canvas.width,
            y1: y1.min(canvas.height),
            index,
            total,
            layout_center: None,
            layout_bottom_center: None,
            telemetry_center: None,
            telemetry_bottom_center: None,
        });
    }
    units
}

fn inline_unit_rects(canvas: &Canvas, text: &str, mode: UnitMode) -> Vec<UnitRect> {
    let lines: Vec<&str> = text.lines().collect();
    let lines = if lines.is_empty() { vec![text] } else { lines };
    let total = lines
        .iter()
        .map(|line| unit_count(line, mode))
        .sum::<usize>()
        .max(1);
    let mut units = Vec::new();
    let mut global = 0_usize;
    for (line_index, line) in lines.iter().enumerate() {
        let line_units = unit_count(line, mode);
        if line_units == 0 {
            continue;
        }
        let y0 = ((line_index as f32 / lines.len() as f32) * canvas.height as f32).floor() as u32;
        let y1 =
            (((line_index + 1) as f32 / lines.len() as f32) * canvas.height as f32).ceil() as u32;
        let (x0, x1) =
            alpha_bounds_x(canvas, y0, y1.min(canvas.height)).unwrap_or((0, canvas.width));
        let span = (x1.saturating_sub(x0)).max(1);
        for local in 0..line_units {
            let ux0 = x0 + ((local as f32 / line_units as f32) * span as f32).floor() as u32;
            let ux1 = x0 + (((local + 1) as f32 / line_units as f32) * span as f32).ceil() as u32;
            units.push(UnitRect {
                x0: ux0.min(canvas.width),
                y0,
                x1: ux1.min(canvas.width),
                y1: y1.min(canvas.height),
                index: global,
                total,
                layout_center: None,
                layout_bottom_center: None,
                telemetry_center: None,
                telemetry_bottom_center: None,
            });
            global += 1;
        }
    }
    units
}

fn canvas_dim(value: f32) -> u32 {
    value.ceil().max(1.0).min(u32::MAX as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_ir::{
        Composition, CompositionNode, Layer, MotionBlurSettings, Rect, Scene, TextRangeSelector,
        Transform2D,
    };
    use serde_json::json;
    use std::sync::{Mutex, OnceLock};

    fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn scalar_keyframes_interpolate_linearly() {
        let keyframes = vec![
            ScalarKeyframe {
                time: 1.0,
                value: 20.0,
                hold: false,
                approximate: false,
                ease: None,
            },
            ScalarKeyframe {
                time: 3.0,
                value: 60.0,
                hold: false,
                approximate: false,
                ease: None,
            },
        ];

        assert_eq!(evaluate_scalar_keyframes(&keyframes, 2.0, 100.0), 40.0);
    }

    #[test]
    fn scalar_keyframes_apply_cubic_ease() {
        let keyframes = vec![
            ScalarKeyframe {
                time: 0.0,
                value: 0.0,
                hold: false,
                approximate: false,
                ease: Some(render_ir::KeyframeEase {
                    x1: 0.42,
                    y1: 0.0,
                    x2: 1.0,
                    y2: 1.0,
                }),
            },
            ScalarKeyframe {
                time: 1.0,
                value: 100.0,
                hold: false,
                approximate: false,
                ease: None,
            },
        ];

        let eased = evaluate_scalar_keyframes(&keyframes, 0.5, 0.0);

        assert!(
            eased < 50.0,
            "expected ease-in midpoint below linear, got {eased}"
        );
    }

    #[test]
    fn keyframe_sample_reports_bezier_diagnostics_without_changing_value() {
        let ease = render_ir::KeyframeEase {
            x1: 0.42,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let keyframes = vec![
            ScalarKeyframe {
                time: 0.0,
                value: 0.0,
                hold: false,
                approximate: true,
                ease: Some(ease),
            },
            ScalarKeyframe {
                time: 1.0,
                value: 100.0,
                hold: false,
                approximate: false,
                ease: None,
            },
        ];

        let sample = sample_scalar_keyframes(&keyframes, 0.5, 25.0);
        let evaluated = evaluate_scalar_keyframes(&keyframes, 0.5, 25.0);

        assert_eq!(sample.value, evaluated);
        assert_eq!(sample.diagnostics.phase, "segment");
        assert_eq!(sample.diagnostics.interpolation, "bezier");
        assert_eq!(sample.diagnostics.segment_index, Some(0));
        assert_eq!(sample.diagnostics.key_start_time, Some(0.0));
        assert_eq!(sample.diagnostics.key_end_time, Some(1.0));
        assert_close(sample.diagnostics.normalized_time.unwrap() as f64, 0.5);
        assert!(sample.diagnostics.eased_progress.unwrap() < 0.5);
        assert_eq!(sample.diagnostics.ease.unwrap().x1, ease.x1);
        assert!(sample.diagnostics.approximate);
    }

    #[test]
    fn render_trace_records_transform_keyframe_sampling_passport() {
        let ease = render_ir::KeyframeEase {
            x1: 0.42,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 4,
                height: 4,
                fps: 10.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "ease_probe".to_string(),
                start: 0.0,
                duration: 1.0,
                color: [255, 255, 255, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1.0,
                    h: 1.0,
                },
                transform: render_ir::Transform2D {
                    animation: render_ir::Transform2DAnimation {
                        position: vec![
                            Vec2Keyframe {
                                time: 0.0,
                                value: [0.0, 0.0],
                                hold: false,
                                approximate: true,
                                ease: Some(ease),
                            },
                            Vec2Keyframe {
                                time: 1.0,
                                value: [2.0, 0.0],
                                hold: false,
                                approximate: false,
                                ease: None,
                            },
                        ],
                        ..render_ir::Transform2DAnimation::default()
                    },
                    ..render_ir::Transform2D::default()
                },
                effects: Vec::new(),
            }],
        };
        let mut footage = CheckerboardFootageProvider;

        let (_frame, trace) = render_frame_with_footage_traced(&scene, 5, &mut footage).unwrap();

        let record = trace
            .keyframes
            .iter()
            .find(|record| record.property == "transform.position")
            .unwrap();
        assert_eq!(record.event, "temporal.keyframe_sample");
        assert_eq!(record.layer_id, "ease_probe");
        assert_eq!(record.interpolation, "bezier");
        assert_eq!(record.segment_index, Some(0));
        assert_close(record.normalized_time.unwrap() as f64, 0.5);
        assert!(record.eased_progress.unwrap() < 0.5);
        assert_eq!(record.ease.unwrap().x1, ease.x1);
    }

    #[test]
    fn effect_debug_trace_reports_alpha_policy_and_intermediate_stats() {
        let mut input = Canvas::transparent(5, 1);
        input.set_pixel(2, 0, [255, 255, 255, 255]);

        let blur = effect_debug_trace_json(
            "ADBE Box Blur2",
            &input,
            &json!({ "radius": 1, "iterations": 2 }),
            0.0,
        )
        .unwrap();
        assert_eq!(
            blur["alpha_policy"]["canvas_storage"],
            json!("straight_rgba8")
        );
        assert_eq!(
            blur["alpha_policy"]["gf_src_alpha_option"],
            json!("standard_options_unresolved_m19")
        );
        assert_eq!(blur["params"]["iterations_applied"], json!(2));
        assert_eq!(blur["alpha_stats"]["input"]["nonzero_pixels"], json!(1));
        assert!(
            blur["alpha_stats"]["output"]["nonzero_pixels"]
                .as_u64()
                .unwrap()
                > 1
        );

        let shadow = effect_debug_trace_json(
            "ADBE Drop Shadow",
            &input,
            &json!({ "0002": 50, "0003": 180, "0004": 1, "0005": 2 }),
            0.0,
        )
        .unwrap();
        assert_eq!(
            shadow["alpha_policy"]["softness"],
            json!("alpha_channel_only_box_blur_then_recolor")
        );
        assert_eq!(
            shadow["alpha_policy"]["shadow_blur_alpha_option"],
            json!("gf_set_blur_alpha_channel_only_confirmed")
        );
        assert_eq!(
            shadow["alpha_stats"]["source_alpha"]["nonzero_pixels"],
            json!(1)
        );

        let glow = effect_debug_trace_json(
            "ADBE Glo2",
            &input,
            &json!({ "0001": "alpha channel", "0002": 1, "0003": 2, "0004": 1.0 }),
            0.0,
        )
        .unwrap();
        assert_eq!(glow["params"]["based_on"], json!("alpha_channel"));
        assert_eq!(glow["params"]["based_on_param_source"], json!("0001"));
        assert_eq!(
            glow["alpha_policy"]["threshold_source"],
            json!("alpha_channel")
        );
        assert_eq!(
            glow["alpha_stats"]["threshold_source"]["nonzero_pixels"],
            json!(1)
        );

        let minimax = effect_debug_trace_json(
            "ADBE Minimax",
            &input,
            &json!({ "0001": 2, "0002": 1, "0003": 6, "0004": 1 }),
            0.0,
        )
        .unwrap();
        assert_eq!(minimax["params"]["stage_count"], json!(1));
        assert_eq!(minimax["alpha_policy"]["selected_channels"], json!("alpha"));
        assert_eq!(
            minimax["alpha_stats"]["first_pass"]["nonzero_pixels"],
            json!(3)
        );
        assert!(minimax["hashes"]["first_pass_rgba"]
            .as_str()
            .unwrap()
            .starts_with("0x"));
    }

    #[test]
    fn vec2_keyframes_respect_hold() {
        let keyframes = vec![
            Vec2Keyframe {
                time: 1.0,
                value: [10.0, 20.0],
                hold: true,
                approximate: false,
                ease: None,
            },
            Vec2Keyframe {
                time: 3.0,
                value: [50.0, 80.0],
                hold: false,
                approximate: false,
                ease: None,
            },
        ];

        assert_eq!(
            evaluate_vec2_keyframes(&keyframes, 2.0, [0.0, 0.0]),
            [10.0, 20.0]
        );
    }

    #[test]
    fn source_frame_quantization_reports_floor_epsilon_policy() {
        let quantized = source_frame_quantization(0.7, 10.0).unwrap();

        assert_eq!(quantized.frame_id, 7);
        assert_eq!(quantized.frame_rate, 10.0);
        assert_close(quantized.frame_time, 0.7);
        assert_close(quantized.subframe, 0.0);
        assert_eq!(quantized.policy, "floor(source_time*frame_rate+1e-9)");

        let near_boundary = source_frame_quantization((3.0 / 30.0) - 0.5e-12, 30.0).unwrap();
        assert_eq!(near_boundary.frame_id, 3);
    }

    #[test]
    fn transform_canvas_applies_position() {
        let mut src = Canvas::transparent(2, 2);
        src.set_pixel(0, 0, [255, 0, 0, 255]);
        let transform = render_ir::Transform2D {
            position: [2.0, 1.0],
            ..render_ir::Transform2D::default()
        };

        let dst = transform_canvas(&src, 4, 4, &transform, [0.0, 0.0]);

        assert_eq!(dst.pixel(2, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn matrix_report_normalizes_affine_components() {
        let matrix = Mat3::translate(Vec2::new(5.0, -3.0))
            .mul(Mat3::rotate_degrees(90.0))
            .mul(Mat3::scale(Vec2::new(2.0, 3.0)));

        let report = matrix_report(matrix);

        assert_close_f32(report.translation[0], 5.0);
        assert_close_f32(report.translation[1], -3.0);
        assert_close_f32(report.scale[0], 2.0);
        assert_close_f32(report.scale[1], 3.0);
        assert_close_f32(report.scale_max, 3.0);
        assert_close_f32(report.determinant, 6.0);
        assert_close_f32(report.rotation_degrees, 90.0);
        assert_close_f32(report.axis_dot, 0.0);
        assert!(report.affine_2d);
    }

    #[test]
    fn expression_trace_reports_named_subset_context() {
        let mut transform = render_ir::Transform2D {
            position: [2.0, 2.0],
            ..render_ir::Transform2D::default()
        };
        transform.animation.expression = render_ir::Transform2DExpression {
            position: Some(PositionExpression::EdgeWobble {
                intro: 0.25,
                outro: 0.25,
                amp: 10.0,
                freq: 2.0,
                source: "unit_edge_wobble".to_string(),
            }),
        };
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 4,
                height: 4,
                fps: 4.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "expr".to_string(),
                start: 0.0,
                duration: 1.0,
                color: [255, 0, 0, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1.0,
                    h: 1.0,
                },
                transform,
                effects: Vec::new(),
            }],
        };
        let mut footage = CheckerboardFootageProvider;

        let (_frame, trace) = render_frame_with_footage_traced(&scene, 1, &mut footage).unwrap();

        assert_eq!(trace.position_expressions.len(), 1);
        let record = &trace.position_expressions[0];
        assert_eq!(record["evaluator"]["mode"], json!("edge_wobble"));
        assert_eq!(record["evaluator"]["target_type"], json!("vector2"));
        assert_eq!(
            record["evaluator"]["fingerprint"],
            json!("unit_edge_wobble")
        );
        assert_eq!(record["context"]["thisComp"]["frameDuration"], json!(0.25));
        assert_eq!(record["context"]["thisLayer"]["inPoint"], json!(0.0));
        assert_eq!(record["context"]["thisLayer"]["outPoint"], json!(1.0));
    }

    #[test]
    fn parsed_position_expression_matches_exp010_expected_rows() {
        let expression = PositionExpression::ParsedProperty {
            source: "intro = 0.25; outro = 0.25; amp = 34; freq = 2.0;\n\
edge = Math.min(time - inPoint, outPoint - time);\n\
env = Math.max(0, Math.min(1, edge / intro));\n\
value + [Math.sin(time * freq * 2 * Math.PI) * amp * env, 0];"
                .to_string(),
        };
        let expected = [
            (0, [256.0, 256.0]),
            (1, [257.843872782, 256.0]),
            (2, [262.737846418, 256.0]),
            (5, [275.629909152, 256.0]),
            (10, [226.555136271, 256.0]),
            (15, [256.0, 256.0]),
            (20, [285.444863729, 256.0]),
            (30, [256.0, 256.0]),
            (45, [256.0, 256.0]),
            (59, [254.156127218, 256.0]),
        ];

        for (frame, expected_position) in expected {
            let time = frame as f64 / 30.0;
            let sample = evaluate_position_expression_sample(
                &expression,
                [256.0, 256.0],
                time,
                0.0,
                2.0,
                30.0,
            );

            assert_eq!(sample.error, None, "frame {frame}");
            assert_eq!(sample.mode, "parsed_property_subset");
            assert_close_f32(sample.position[0], expected_position[0] as f32);
            assert_close_f32(sample.position[1], expected_position[1] as f32);
        }
    }

    #[test]
    fn parsed_position_expression_trace_reports_parsed_subset_mode() {
        let mut transform = render_ir::Transform2D {
            position: [256.0, 256.0],
            ..render_ir::Transform2D::default()
        };
        transform.animation.expression = render_ir::Transform2DExpression {
            position: Some(PositionExpression::ParsedProperty {
                source: "intro = 0.25; outro = 0.25; amp = 34; freq = 2.0;\n\
edge = Math.min(time - inPoint, outPoint - time);\n\
env = Math.max(0, Math.min(1, edge / intro));\n\
value + [Math.sin(time * freq * 2 * Math.PI) * amp * env, 0];"
                    .to_string(),
            }),
        };
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 512,
                height: 512,
                fps: 30.0,
                duration: 2.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "expr".to_string(),
                start: 0.0,
                duration: 2.0,
                color: [255, 0, 0, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1.0,
                    h: 1.0,
                },
                transform,
                effects: Vec::new(),
            }],
        };
        let mut footage = CheckerboardFootageProvider;

        let (_frame, trace) = render_frame_with_footage_traced(&scene, 5, &mut footage).unwrap();

        assert_eq!(trace.position_expressions.len(), 1);
        let record = &trace.position_expressions[0];
        assert_eq!(record["evaluator"]["mode"], json!("parsed_property_subset"));
        assert_eq!(record["evaluator"]["target_type"], json!("vector2"));
        assert_eq!(record["expression"]["type"], json!("parsed_property"));
        assert_eq!(record["expression"]["error"], json!(null));
        assert_close_f32(
            record["sampled_position"][0].as_f64().unwrap() as f32,
            275.6299,
        );
        assert_eq!(record["sampled_position"][1], json!(256.0));
    }

    #[test]
    fn render_frame_renders_nested_precomp_composition() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 4,
                height: 4,
                fps: 1.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "child".to_string(),
                    width: 4,
                    height: 4,
                    fps: 1.0,
                    duration: 1.0,
                    background: [0, 0, 0, 0],
                    motion_blur: render_ir::MotionBlurSettings::default(),
                },
                layers: vec![Layer::Solid {
                    id: "red".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    color: [255, 0, 0, 255],
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 1.0,
                        h: 1.0,
                    },
                    transform: render_ir::Transform2D::default(),
                    effects: Vec::new(),
                }],
            }],
            assets: Vec::new(),
            layers: vec![Layer::Precomp {
                id: "child_pre".to_string(),
                start: 0.0,
                duration: 1.0,
                composition: "child".to_string(),
                collapse_transformations: false,
                transform: render_ir::Transform2D::default(),
                effects: Vec::new(),
            }],
        };

        let frame = render_frame(&scene, 0).unwrap();

        assert_eq!(frame.pixel(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn text_animator_position_moves_character_unit() {
        let mut canvas = Canvas::transparent(4, 1);
        canvas.set_pixel(0, 0, [255, 255, 255, 255]);
        let animator = TextAnimatorSpec {
            name: "move".to_string(),
            opacity: 100.0,
            position: Some([1.0, 0.0]),
            scale: None,
            rotation: None,
            blur: None,
            selector: TextRangeSelector {
                start: 0.0,
                end: 100.0,
                ..TextRangeSelector::default()
            },
            expression_selector: None,
        };

        let animated =
            apply_unit_animator_transform(&canvas, "A", None, &animator, 0.0, 100.0, 0.0, 0.0, 1.0);

        assert_eq!(animated.pixel(1, 0), [255, 255, 255, 255]);
        assert_eq!(animated.pixel(0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn text_animator_transform_uses_bottom_center_unit_pivot() {
        let animator = TextAnimatorSpec {
            name: "scale".to_string(),
            opacity: 100.0,
            position: None,
            scale: Some([125.0, 125.0]),
            rotation: None,
            blur: None,
            selector: TextRangeSelector {
                start: 0.0,
                end: 100.0,
                ..TextRangeSelector::default()
            },
            expression_selector: None,
        };
        let unit = UnitRect {
            x0: 10,
            y0: 20,
            x1: 20,
            y1: 50,
            index: 0,
            total: 1,
            layout_center: None,
            layout_bottom_center: None,
            telemetry_center: None,
            telemetry_bottom_center: None,
        };

        let transform = animator_unit_transform(unit, &animator, 1.0, 1.0);
        let pivot = transform.matrix.transform_point(Vec2::new(15.0, 50.0));
        let top = transform.matrix.transform_point(Vec2::new(15.0, 20.0));

        assert_eq!(transform.center, [15.0, 50.0]);
        assert!((pivot.x - 15.0).abs() < 0.001);
        assert!((pivot.y - 50.0).abs() < 0.001);
        assert!((top.y - 12.5).abs() < 0.001);
    }

    #[test]
    fn text_animator_transform_prefers_layout_float_bottom_center_pivot() {
        let animator = TextAnimatorSpec {
            name: "scale".to_string(),
            opacity: 100.0,
            position: None,
            scale: Some([125.0, 125.0]),
            rotation: None,
            blur: None,
            selector: TextRangeSelector {
                start: 0.0,
                end: 100.0,
                ..TextRangeSelector::default()
            },
            expression_selector: None,
        };
        let unit = UnitRect {
            x0: 10,
            y0: 20,
            x1: 20,
            y1: 50,
            index: 0,
            total: 1,
            layout_center: Some([14.25, 32.0]),
            layout_bottom_center: Some([14.25, 47.5]),
            telemetry_center: None,
            telemetry_bottom_center: None,
        };

        let transform = animator_unit_transform(unit, &animator, 1.0, 1.0);

        assert_eq!(transform.center, [14.25, 47.5]);
    }

    #[test]
    fn text_animator_no_blur_scale_inverse_samples_destination_area() {
        let mut canvas = Canvas::transparent(24, 24);
        for y in 10..14 {
            for x in 8..12 {
                canvas.set_pixel(x, y, [255, 255, 255, 255]);
            }
        }
        let animator = TextAnimatorSpec {
            name: "scale".to_string(),
            opacity: 100.0,
            position: None,
            scale: Some([200.0, 200.0]),
            rotation: None,
            blur: None,
            selector: TextRangeSelector {
                start: 0.0,
                end: 100.0,
                ..TextRangeSelector::default()
            },
            expression_selector: None,
        };
        let unit = UnitRect {
            x0: 8,
            y0: 10,
            x1: 12,
            y1: 14,
            index: 0,
            total: 1,
            layout_center: None,
            layout_bottom_center: None,
            telemetry_center: None,
            telemetry_bottom_center: None,
        };
        let mut output = Canvas::transparent(24, 24);

        draw_transformed_unit(&mut output, &canvas, unit, &animator, 1.0, 1.0);

        let covered = (0..output.height)
            .flat_map(|y| (0..output.width).map(move |x| (x, y)))
            .filter(|(x, y)| output.pixel(*x, *y)[3] > 0)
            .count();
        assert!(covered > 16);
    }

    #[test]
    fn text_animator_blur_tracks_rect_and_gaussian_bounds() {
        let animator = TextAnimatorSpec {
            name: "blur".to_string(),
            opacity: 100.0,
            position: None,
            scale: None,
            rotation: None,
            blur: Some([10.0, 10.0]),
            selector: TextRangeSelector {
                start: 0.0,
                end: 100.0,
                ..TextRangeSelector::default()
            },
            expression_selector: None,
        };

        assert_eq!(
            text_animator_blur_radii_for_kernel(
                &animator,
                1.0,
                1.0,
                TextAnimatorBlurKernel::RectSplat,
                [0.0, 0.0],
            ),
            [8, 8]
        );
        assert_eq!(text_animator_blur_radii(&animator, 1.0, 1.0), [14, 14]);
        assert_eq!(text_animator_blur_radii(&animator, 0.45, 1.0), [7, 7]);
        assert_eq!(text_animator_blur_radii(&animator, -1.0, 1.0), [0, 0]);

        let x_only = TextAnimatorSpec {
            blur: Some([10.0, 0.0]),
            ..animator.clone()
        };
        assert_eq!(text_animator_blur_radii(&x_only, 1.0, 1.0), [14, 0]);
    }

    #[test]
    fn text_animator_blur_can_bleed_from_off_canvas_centers() {
        let mut canvas = Canvas::transparent(3, 1);
        canvas.set_pixel(0, 0, [255, 255, 255, 255]);
        let animator = TextAnimatorSpec {
            name: "blur_bleed".to_string(),
            opacity: 100.0,
            position: Some([-1.0, 0.0]),
            scale: None,
            rotation: None,
            blur: Some([1.0, 1.0]),
            selector: TextRangeSelector {
                start: 0.0,
                end: 100.0,
                ..TextRangeSelector::default()
            },
            expression_selector: None,
        };

        let animated =
            apply_unit_animator_transform(&canvas, "A", None, &animator, 0.0, 100.0, 0.0, 0.0, 1.0);

        assert!(animated.pixel(0, 0)[3] > 0);
    }

    #[test]
    fn bounce_expression_selector_honors_pre_delay_value_branch() {
        let selector = TextExpressionSelector::PerCharacterBounce {
            delay: 0.05,
            freq: 2.0,
            amplitude: 100.0,
            decay: 8.0,
            pre_delay_amount: Some(100.0),
            source: "else { value }".to_string(),
        };

        let detail = text_expression_weight_detail(Some(&selector), 0, 4, 0.0, 0.0);

        assert_eq!(detail.text_index, Some(1));
        assert!(detail.local_time_after_delay.unwrap() < 0.0);
        assert_eq!(detail.raw_amount, Some(100.0));
        assert_eq!(detail.clamped_amount, Some(100.0));
        assert_eq!(detail.weight, 1.0);
    }

    #[test]
    fn bounce_expression_selector_keeps_zero_pre_delay_fallback() {
        let selector = TextExpressionSelector::PerCharacterBounce {
            delay: 0.05,
            freq: 2.0,
            amplitude: 100.0,
            decay: 8.0,
            pre_delay_amount: None,
            source: String::new(),
        };

        let detail = text_expression_weight_detail(Some(&selector), 0, 4, 0.0, 0.0);

        assert_eq!(detail.raw_amount, Some(0.0));
        assert_eq!(detail.clamped_amount, Some(0.0));
        assert_eq!(detail.weight, 0.0);
    }

    #[test]
    fn selector_glyph_passport_maps_word_units_to_glyph_runs() {
        let layout = text_engine::layout_text_stub(&TextLayoutRequest {
            text: "Hi all".to_string(),
            font_id: "missing".to_string(),
            font_size: 10.0,
            box_rect: Some([0.0, 0.0, 100.0, 40.0]),
        });

        let passport =
            selector_unit_glyph_passport(Some(&layout), render_ir::TextSelectorBasedOn::Words, 1);

        assert_eq!(passport["available"], json!(true));
        assert_eq!(passport["metric_source"], json!("stub"));
        assert_eq!(
            passport["cooltype_reference_status"],
            json!("not_cooltype_verified")
        );
        assert_eq!(passport["glyph_count"], json!(3));
        assert_eq!(passport["characters"], json!(["a", "l", "l"]));
        assert_eq!(passport["glyph_run_indices"], json!([3, 4, 5]));
        assert_eq!(passport["word_indices"], json!([1, 1, 1]));
    }

    #[test]
    fn layout_character_units_preserve_clipped_glyphs_for_selector_totals() {
        let canvas = Canvas::transparent(10, 10);
        let layout = text_engine::layout_text_stub(&TextLayoutRequest {
            text: "ABCD".to_string(),
            font_id: "missing".to_string(),
            font_size: 10.0,
            box_rect: Some([0.0, 0.0, 40.0, 10.0]),
        });

        let units = unit_rects(
            &canvas,
            "ABCD",
            Some(&layout),
            render_ir::TextSelectorBasedOn::Characters,
        );

        assert_eq!(units.len(), 4);
        assert!(units.iter().all(|unit| unit.total == 4));
        assert_eq!(units[2].index, 2);
        assert_eq!(units[2].x0, canvas.width);
        assert_eq!(units[2].x1, canvas.width);
    }

    #[test]
    fn layout_character_units_exclude_whitespace_like_ae_selector_units() {
        let canvas = Canvas::transparent(80, 20);
        let layout = text_engine::layout_text_stub(&TextLayoutRequest {
            text: "AB CD".to_string(),
            font_id: "missing".to_string(),
            font_size: 10.0,
            box_rect: Some([0.0, 0.0, 80.0, 20.0]),
        });

        let units = unit_rects(
            &canvas,
            "AB CD",
            Some(&layout),
            render_ir::TextSelectorBasedOn::Characters,
        );

        assert_eq!(units.len(), 4);
        assert!(units.iter().all(|unit| unit.total == 4));
        assert_eq!(unit_count("AB CD", UnitMode::Characters), 4);
    }

    #[test]
    fn text_layout_trace_telemetry_reports_comp_coordinates() {
        let layout = text_engine::layout_text_stub(&TextLayoutRequest {
            text: "A".to_string(),
            font_id: "missing".to_string(),
            font_size: 10.0,
            box_rect: Some([0.0, 0.0, 40.0, 20.0]),
        });

        let telemetry =
            text_layout_trace_telemetry(&layout, Mat3::translate(Vec2::new(7.0, -3.0)), [2.0, 5.0]);

        assert_eq!(telemetry.glyphs[0].bbox, [9.0, 2.0, 6.0, 10.0]);
        assert_eq!(
            telemetry.glyphs[0].cooltype_bbox_minmax,
            [9.0, 2.0, 15.0, 12.0]
        );
        assert_eq!(telemetry.glyphs[0].bbox_center, [12.0, 7.0]);
    }

    #[test]
    fn animator_contribution_reports_opacity_and_matrix_for_opacity_only_units() {
        let animator = TextAnimatorSpec {
            name: "fade".to_string(),
            opacity: 0.0,
            position: None,
            scale: None,
            rotation: None,
            blur: None,
            selector: TextRangeSelector::default(),
            expression_selector: None,
        };
        let unit = UnitRect {
            x0: 0,
            y0: 0,
            x1: 2,
            y1: 2,
            index: 0,
            total: 1,
            layout_center: None,
            layout_bottom_center: None,
            telemetry_center: None,
            telemetry_bottom_center: None,
        };

        let contribution = animator_contribution_json(unit, &animator, 1.0, 1.0);

        assert_eq!(contribution["final_opacity_alpha_scale"], json!(0.0));
        assert_eq!(contribution["blur_radius_px"], json!(0));
        assert!(contribution["final_matrix"].is_array());
    }

    #[test]
    fn selector_weight_supports_ramp_and_random_order() {
        let selector = TextRangeSelector {
            shape: TextSelectorShape::RampUp,
            randomize_order: true,
            ..TextRangeSelector::default()
        };

        let first = selector_weight(0, 6, &selector, 0.0, 100.0, 0.0);
        let second = selector_weight(0, 6, &selector, 0.0, 100.0, 0.0);
        let other = selector_weight(1, 6, &selector, 0.0, 100.0, 0.0);

        assert_eq!(first, second);
        assert_ne!(first, other);
    }

    #[test]
    fn square_selector_full_range_ignores_smoothness_edge_fade() {
        let selector = TextRangeSelector {
            shape: TextSelectorShape::Square,
            smoothness: 100.0,
            ..TextRangeSelector::default()
        };

        let weights: Vec<f32> = (0..4)
            .map(|index| selector_weight(index, 4, &selector, 0.0, 100.0, 0.0))
            .collect();

        assert_eq!(weights, vec![1.0, 1.0, 1.0, 1.0]);
        assert_eq!(selector_weight(0, 4, &selector, 100.0, 100.0, 0.0), 0.0);
        assert!((selector_weight(3, 4, &selector, 95.0, 100.0, 0.0) - 0.2).abs() < 1.0e-6);
    }

    #[test]
    fn posterize_time_quantizes_layer_sampling_time() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 10.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "moving".to_string(),
                start: 0.0,
                duration: 1.0,
                color: [255, 255, 255, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 2.0,
                    h: 2.0,
                },
                transform: render_ir::Transform2D {
                    animation: render_ir::Transform2DAnimation {
                        position: vec![
                            Vec2Keyframe {
                                time: 0.0,
                                value: [0.0, 0.0],
                                hold: false,
                                approximate: false,
                                ease: None,
                            },
                            Vec2Keyframe {
                                time: 1.0,
                                value: [8.0, 0.0],
                                hold: false,
                                approximate: false,
                                ease: None,
                            },
                        ],
                        ..render_ir::Transform2DAnimation::default()
                    },
                    ..render_ir::Transform2D::default()
                },
                effects: vec![posterize_effect(1.0)],
            }],
        };

        let frame = render_frame(&scene, 5).unwrap();

        assert_eq!(frame.pixel(0, 0), [255, 255, 255, 255]);
        assert_eq!(frame.pixel(4, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn temporal_trace_records_posterized_footage_source_time() {
        struct TraceFootageProvider {
            requested_times: Vec<f64>,
        }

        impl FootageProvider for TraceFootageProvider {
            fn frame_at(&mut self, _source: &str, time: f64) -> anyhow::Result<Option<Canvas>> {
                self.requested_times.push(time);
                Ok(Some(Canvas::transparent(2, 2)))
            }
        }

        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 10.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Footage {
                id: "clip".to_string(),
                start: 0.2,
                duration: 1.0,
                source: "numbered_frames".to_string(),
                source_start: 0.4,
                transform: render_ir::Transform2D::default(),
                effects: vec![posterize_effect(2.0)],
            }],
        };
        let mut footage = TraceFootageProvider {
            requested_times: Vec::new(),
        };

        let (_frame, trace) = render_frame_with_footage_traced(&scene, 7, &mut footage).unwrap();

        assert_close(footage.requested_times[0], 0.7);
        let temporal = trace
            .temporal
            .iter()
            .find(|record| record.layer_id == "clip")
            .unwrap();
        assert_eq!(temporal.event, "temporal.layer_time");
        assert_close(temporal.comp_time, 0.7);
        assert_close(temporal.layer_time, 0.5);
        assert_close(temporal.posterized_time, 0.5);
        assert_eq!(temporal.source_id.as_deref(), Some("numbered_frames"));
        assert_close(temporal.source_start.unwrap(), 0.4);
        assert_close(temporal.source_time.unwrap(), 0.7);
        assert_eq!(temporal.source_frame_id, Some(7));
        let source_frame = temporal.source_frame.as_ref().unwrap();
        assert_eq!(source_frame.frame_id, 7);
        assert_eq!(source_frame.frame_rate, 10.0);
        assert_close(source_frame.frame_time, 0.7);
        assert_close(source_frame.subframe, 0.0);
        assert_eq!(source_frame.policy, "floor(source_time*frame_rate+1e-9)");
        assert_eq!(temporal.posterize.unwrap().bucket, Some(1));
        assert_close(temporal.posterize.unwrap().bucket_time, 0.5);
    }

    #[test]
    fn posterize_time_on_adjustment_layer_resamples_lower_stack() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 10.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![
                Layer::Adjustment {
                    id: "posterize_adjustment".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    effects: vec![posterize_effect(1.0)],
                },
                Layer::Solid {
                    id: "moving".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    color: [255, 255, 255, 255],
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 2.0,
                        h: 2.0,
                    },
                    transform: render_ir::Transform2D {
                        animation: render_ir::Transform2DAnimation {
                            position: vec![
                                Vec2Keyframe {
                                    time: 0.0,
                                    value: [0.0, 0.0],
                                    hold: false,
                                    approximate: false,
                                    ease: None,
                                },
                                Vec2Keyframe {
                                    time: 1.0,
                                    value: [8.0, 0.0],
                                    hold: false,
                                    approximate: false,
                                    ease: None,
                                },
                            ],
                            ..render_ir::Transform2DAnimation::default()
                        },
                        ..render_ir::Transform2D::default()
                    },
                    effects: Vec::new(),
                },
            ],
        };

        let frame = render_frame(&scene, 5).unwrap();

        assert_eq!(frame.pixel(0, 0), [255, 255, 255, 255]);
        assert_eq!(frame.pixel(4, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn adjustment_posterize_keeps_downstream_param_time_live_inside_bucket() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 10.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![
                Layer::Adjustment {
                    id: "posterize_adjustment".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    effects: vec![
                        posterize_effect(1.0),
                        minimax_effect(json!({
                            "0001": 1,
                            "0002": {
                                "keyframes": [
                                    { "t": 0.0, "v": 0.0 },
                                    { "t": 1.0, "v": 2.0 }
                                ]
                            },
                            "0003": 1
                        })),
                    ],
                },
                Layer::Solid {
                    id: "moving".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    color: [255, 255, 255, 255],
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 2.0,
                        h: 2.0,
                    },
                    transform: render_ir::Transform2D {
                        animation: render_ir::Transform2DAnimation {
                            position: vec![
                                Vec2Keyframe {
                                    time: 0.0,
                                    value: [0.0, 0.0],
                                    hold: false,
                                    approximate: false,
                                    ease: None,
                                },
                                Vec2Keyframe {
                                    time: 1.0,
                                    value: [8.0, 0.0],
                                    hold: false,
                                    approximate: false,
                                    ease: None,
                                },
                            ],
                            ..render_ir::Transform2DAnimation::default()
                        },
                        ..render_ir::Transform2D::default()
                    },
                    effects: Vec::new(),
                },
            ],
        };
        let mut footage = CheckerboardFootageProvider;

        let (frame, trace) = render_frame_with_footage_traced(&scene, 5, &mut footage).unwrap();

        assert_eq!(
            frame.pixel(0, 0)[3],
            255,
            "lower stack should stay on bucket frame 0"
        );
        assert_eq!(
            frame.pixel(4, 0)[3],
            0,
            "lower stack should not use frame 5 position"
        );

        let posterize_trace = trace
            .adjustment_effects
            .iter()
            .find(|effect| effect.match_name == "ADBE Posterize Time")
            .unwrap();
        assert_eq!(posterize_trace.comp_time, 0.5);
        assert_eq!(posterize_trace.lower_stack_time, 0.0);
        assert_eq!(posterize_trace.param_time, 0.0);
        assert_eq!(posterize_trace.posterize.unwrap().frame_rate, 1.0);
        assert_eq!(posterize_trace.posterize.unwrap().bucket, Some(0));
        let adjustment_temporal = trace
            .temporal
            .iter()
            .find(|record| record.event == "temporal.adjustment_layer_time")
            .unwrap();
        assert_eq!(adjustment_temporal.layer_id, "posterize_adjustment");
        assert_eq!(adjustment_temporal.comp_time, 0.5);
        assert_eq!(adjustment_temporal.layer_time, 0.5);
        assert_eq!(adjustment_temporal.posterized_time, 0.0);
        assert_eq!(adjustment_temporal.adjustment_lower_stack_time, Some(0.0));

        let minimax_trace = trace
            .adjustment_effects
            .iter()
            .find(|effect| effect.match_name == "ADBE Minimax")
            .unwrap();
        assert_eq!(minimax_trace.param_time, 0.5);
        assert_eq!(minimax_trace.lower_stack_time, 0.0);
        assert!(minimax_trace.posterize.is_none());
    }

    #[test]
    fn adjustment_stack_debug_records_geometry_minimax_and_turbulent_checkpoints() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 10.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![
                Layer::Adjustment {
                    id: "stk_adjustment".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    effects: vec![
                        effect(
                            "ADBE Geometry2",
                            json!({ "0003": 96, "0004": 110, "0008": 92 }),
                        ),
                        posterize_effect(1.0),
                        minimax_effect(json!({ "0001": 2, "0002": 1, "0003": 1 })),
                        effect(
                            "ADBE Turbulent Displace",
                            json!({
                                "0002": 24,
                                "0003": 72,
                                "0005": 2,
                                "0006": {
                                    "keyframes": [
                                        { "t": 0.0, "v": 0.0 },
                                        { "t": 1.0, "v": 90.0 }
                                    ]
                                }
                            }),
                        ),
                    ],
                },
                Layer::Solid {
                    id: "below".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    color: [255, 255, 255, 255],
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 2.0,
                        h: 2.0,
                    },
                    transform: render_ir::Transform2D::default(),
                    effects: Vec::new(),
                },
            ],
        };
        let mut footage = CheckerboardFootageProvider;

        let (_frame, trace) = render_frame_with_footage_traced(&scene, 5, &mut footage).unwrap();

        let adjustment_order = trace
            .adjustment_effects
            .iter()
            .map(|effect| effect.match_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            adjustment_order,
            vec![
                "ADBE Geometry2",
                "ADBE Posterize Time",
                "ADBE Minimax",
                "ADBE Turbulent Displace"
            ]
        );
        for record in &trace.adjustment_effects {
            assert_eq!(record.comp_time, 0.5);
            assert_eq!(record.lower_stack_time, 0.0);
            assert!(!record.input_hash.is_empty());
            assert!(!record.output_hash.is_empty());
        }
        assert_eq!(trace.adjustment_effects[0].param_time, 0.0);
        assert_eq!(trace.adjustment_effects[1].param_time, 0.0);
        assert_eq!(trace.adjustment_effects[2].param_time, 0.5);
        assert_eq!(trace.adjustment_effects[3].param_time, 0.5);

        let geometry = trace
            .effect_debug
            .iter()
            .find(|record| record.match_name == "ADBE Geometry2")
            .unwrap();
        assert_eq!(geometry.application, "adjustment");
        assert_eq!(geometry.effect_index, 0);
        assert_eq!(geometry.effect_time, 0.0);
        assert_eq!(
            geometry.trace["schema"],
            "ae-native-renderer.geometry2-debug.v1"
        );
        assert_eq!(geometry.trace["resolved"]["scale"], json!([110.0, 110.0]));
        assert_eq!(geometry.trace["resolved"]["rotation"], json!(92.0));
        assert_eq!(
            geometry.trace["property_mapping"]["0003"]["native_role"],
            json!("uniform_scale_fallback")
        );
        assert_eq!(
            geometry.trace["property_mapping"]["0004"]["native_role"],
            json!("scale_height")
        );
        assert_eq!(
            geometry.trace["property_mapping"]["0008"]["native_role"],
            json!("rotation_degrees")
        );
        assert_eq!(
            geometry.trace["sampler_mode"],
            "bilinear_premult_unpremultiply_partial_footprint_transparent"
        );
        assert!(geometry.trace["samples"].as_array().unwrap().len() > 0);

        let minimax = trace
            .effect_debug
            .iter()
            .find(|record| record.match_name == "ADBE Minimax")
            .unwrap();
        assert_eq!(minimax.effect_index, 2);
        assert_eq!(minimax.effect_time, 0.5);
        assert_eq!(
            minimax.trace["params"]["direction"],
            "horizontal_and_vertical"
        );
        assert_eq!(minimax.trace["params"]["dont_shrink_edges"], false);

        let turbulent = trace
            .effect_debug
            .iter()
            .find(|record| record.match_name == "ADBE Turbulent Displace")
            .unwrap();
        assert_eq!(turbulent.application, "adjustment");
        assert_eq!(turbulent.effect_index, 3);
        assert_eq!(turbulent.effect_time, 0.5);
        assert_eq!(
            turbulent.trace["schema"],
            "ae-native-renderer.turbulent-displace-field.v1"
        );
        assert_close(
            turbulent.trace["resolved"]["evolution"].as_f64().unwrap(),
            45.0,
        );
        assert_eq!(
            turbulent.trace["ae_wrapper"]["kernel_path"],
            "TurbulentDisplaceFracAllKernel"
        );
        assert_eq!(
            turbulent.trace["field_state"]["tuning_guardrail"],
            "do_not_tune_from_final_png_only"
        );
        assert_eq!(
            turbulent.trace["field_state"]["dispatch_path"],
            turbulent.trace["ae_wrapper"]["kernel_path"]
        );
        assert!(turbulent.trace["field_hash"].as_str().unwrap().len() == 16);
        assert!(turbulent.trace["samples"].as_array().unwrap().len() > 0);
    }

    #[test]
    fn adjustment_turbulent_uses_full_world_on_posterize_bucket_boundary() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 64,
                height: 64,
                fps: 10.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![
                Layer::Adjustment {
                    id: "posterized_turbulent_adjustment".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    effects: vec![
                        posterize_effect(1.0),
                        effect(
                            "ADBE Turbulent Displace",
                            json!({ "0002": 24, "0003": 72, "0005": 2 }),
                        ),
                    ],
                },
                Layer::Solid {
                    id: "below".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    color: [255, 255, 255, 255],
                    rect: Rect {
                        x: 8.0,
                        y: 8.0,
                        w: 8.0,
                        h: 8.0,
                    },
                    transform: render_ir::Transform2D::default(),
                    effects: Vec::new(),
                },
            ],
        };
        let mut footage = CheckerboardFootageProvider;

        let (_boundary_frame, boundary_trace) =
            render_frame_with_footage_traced(&scene, 0, &mut footage).unwrap();
        let boundary_turbulent = boundary_trace
            .effect_debug
            .iter()
            .find(|record| record.match_name == "ADBE Turbulent Displace")
            .unwrap();
        assert_eq!(boundary_turbulent.trace["input"]["width"], json!(64));
        assert_eq!(boundary_turbulent.trace["input"]["height"], json!(64));

        let (_inside_frame, inside_trace) =
            render_frame_with_footage_traced(&scene, 5, &mut footage).unwrap();
        let inside_turbulent = inside_trace
            .effect_debug
            .iter()
            .find(|record| record.match_name == "ADBE Turbulent Displace")
            .unwrap();
        assert_eq!(inside_turbulent.effect_time, 0.5);
        assert_eq!(inside_turbulent.trace["input"]["width"], json!(14));
        assert_eq!(inside_turbulent.trace["input"]["height"], json!(14));
        assert_eq!(
            inside_turbulent.trace["input"]["alpha_bounds_exclusive"],
            json!([3, 3, 11, 11])
        );
    }

    #[test]
    fn motion_blur_supersamples_moving_layer() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 4.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings {
                    enabled: true,
                    samples: 4,
                    shutter_angle: 1440.0,
                    shutter_phase: -720.0,
                },
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "moving".to_string(),
                start: 0.0,
                duration: 1.0,
                color: [255, 255, 255, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 2.0,
                    h: 2.0,
                },
                transform: render_ir::Transform2D {
                    motion_blur: true,
                    animation: render_ir::Transform2DAnimation {
                        position: vec![
                            Vec2Keyframe {
                                time: 0.0,
                                value: [0.0, 0.0],
                                hold: false,
                                approximate: false,
                                ease: None,
                            },
                            Vec2Keyframe {
                                time: 1.0,
                                value: [8.0, 0.0],
                                hold: false,
                                approximate: false,
                                ease: None,
                            },
                        ],
                        ..render_ir::Transform2DAnimation::default()
                    },
                    ..render_ir::Transform2D::default()
                },
                effects: Vec::new(),
            }],
        };

        let frame = render_frame(&scene, 2).unwrap();
        let alpha_pixels = frame
            .data
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();

        assert!(
            alpha_pixels > 4,
            "motion blur should spread a moving layer over multiple pixels, got {alpha_pixels}"
        );
    }

    #[test]
    fn motion_blur_trace_records_sample_times_and_weights() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 4.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings {
                    enabled: true,
                    samples: 4,
                    shutter_angle: 1440.0,
                    shutter_phase: -720.0,
                },
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "moving".to_string(),
                start: 0.0,
                duration: 1.0,
                color: [255, 255, 255, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 2.0,
                    h: 2.0,
                },
                transform: render_ir::Transform2D {
                    motion_blur: true,
                    animation: render_ir::Transform2DAnimation {
                        position: vec![
                            Vec2Keyframe {
                                time: 0.0,
                                value: [0.0, 0.0],
                                hold: false,
                                approximate: false,
                                ease: None,
                            },
                            Vec2Keyframe {
                                time: 1.0,
                                value: [8.0, 0.0],
                                hold: false,
                                approximate: false,
                                ease: None,
                            },
                        ],
                        ..render_ir::Transform2DAnimation::default()
                    },
                    ..render_ir::Transform2D::default()
                },
                effects: Vec::new(),
            }],
        };
        let mut footage = CheckerboardFootageProvider;

        let (_frame, trace) = render_frame_with_footage_traced(&scene, 2, &mut footage).unwrap();

        assert_eq!(trace.motion_blur.len(), 1);
        let blur = &trace.motion_blur[0];
        assert_eq!(blur.layer_id, "moving");
        assert_close(blur.comp_time, 0.5);
        assert_close(blur.shutter_open, 0.0);
        assert_close(blur.shutter_close, 1.0);
        assert_eq!(blur.effective_samples, 4);
        let sample_times = blur
            .samples
            .iter()
            .map(|sample| sample.sample_time)
            .collect::<Vec<_>>();
        assert_close_slice(&sample_times, &[0.125, 0.375, 0.625, 0.875]);
        for sample in &blur.samples {
            assert!(sample.active);
            assert_eq!(sample.opacity, 100.0);
            assert_close(sample.weight as f64, 0.25);
            assert_eq!(sample.source_time, None);
        }
    }

    #[test]
    fn motion_blur_trace_records_phase_offset_and_posterized_sample_times() {
        struct TraceFootageProvider {
            requested_times: Vec<f64>,
        }

        impl FootageProvider for TraceFootageProvider {
            fn frame_at(&mut self, _source: &str, time: f64) -> anyhow::Result<Option<Canvas>> {
                self.requested_times.push(time);
                Ok(Some(Canvas::transparent(2, 2)))
            }
        }

        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 12,
                height: 3,
                fps: 8.0,
                duration: 2.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings {
                    enabled: true,
                    samples: 4,
                    shutter_angle: 180.0,
                    shutter_phase: 90.0,
                },
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Footage {
                id: "clip".to_string(),
                start: 0.125,
                duration: 2.0,
                source: "numbered_frames".to_string(),
                source_start: 1.0,
                transform: render_ir::Transform2D {
                    motion_blur: true,
                    ..render_ir::Transform2D::default()
                },
                effects: vec![posterize_effect(4.0)],
            }],
        };
        let mut footage = TraceFootageProvider {
            requested_times: Vec::new(),
        };

        let (_frame, trace) = render_frame_with_footage_traced(&scene, 4, &mut footage).unwrap();

        assert_eq!(trace.motion_blur.len(), 1);
        let blur = &trace.motion_blur[0];
        assert_close(blur.comp_time, 0.5);
        assert_close(blur.frame_duration, 0.125);
        assert_close(blur.shutter_open, 0.53125);
        assert_close(blur.shutter_close, 0.59375);
        let sample_times = blur
            .samples
            .iter()
            .map(|sample| sample.sample_time)
            .collect::<Vec<_>>();
        assert_close_slice(&sample_times, &[0.5390625, 0.5546875, 0.5703125, 0.5859375]);
        let posterized_times = blur
            .samples
            .iter()
            .map(|sample| sample.posterized_time)
            .collect::<Vec<_>>();
        assert_close_slice(&posterized_times, &[0.5; 4]);
        let source_times = blur
            .samples
            .iter()
            .map(|sample| sample.source_time.unwrap())
            .collect::<Vec<_>>();
        assert_close_slice(&source_times, &[1.375; 4]);
        assert_eq!(
            blur.samples
                .iter()
                .map(|sample| sample.source_frame_id)
                .collect::<Vec<_>>(),
            vec![Some(11); 4]
        );
        for sample in &blur.samples {
            let source_frame = sample.source_frame.as_ref().unwrap();
            assert_eq!(source_frame.frame_id, 11);
            assert_eq!(source_frame.frame_rate, 8.0);
            assert_close(source_frame.frame_time, 1.375);
            assert_close(source_frame.subframe, 0.0);
        }
        assert_close_slice(&footage.requested_times, &[1.375; 4]);

        let summary = blur.weight_summary();
        assert_eq!(summary.active_samples, 4);
        assert_eq!(summary.contributing_samples, 4);
        assert_close(summary.total_weight as f64, 1.0);

        println!(
            "motion_blur representative: comp_time={:.3} shutter=[{:.5},{:.5}] summary={:?}",
            blur.comp_time, blur.shutter_open, blur.shutter_close, summary
        );
        for sample in [blur.samples.first().unwrap(), blur.samples.last().unwrap()] {
            println!(
                "motion_blur sample[{}]: sample_time={:.7} posterized_time={:.3} source_time={:.3} source_frame_id={:?} weight={:.2}",
                sample.sample_index,
                sample.sample_time,
                sample.posterized_time,
                sample.source_time.unwrap(),
                sample.source_frame_id,
                sample.weight
            );
        }
    }

    #[test]
    fn collapsed_precomp_flattens_solid_with_parent_transform() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 8,
                height: 4,
                fps: 1.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "child".to_string(),
                    width: 4,
                    height: 4,
                    fps: 1.0,
                    duration: 1.0,
                    background: [0, 0, 0, 0],
                    motion_blur: render_ir::MotionBlurSettings::default(),
                },
                layers: vec![Layer::Solid {
                    id: "red".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    color: [255, 0, 0, 255],
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 1.0,
                        h: 1.0,
                    },
                    transform: render_ir::Transform2D::default(),
                    effects: Vec::new(),
                }],
            }],
            assets: Vec::new(),
            layers: vec![Layer::Precomp {
                id: "child_pre".to_string(),
                start: 0.0,
                duration: 1.0,
                composition: "child".to_string(),
                collapse_transformations: true,
                transform: render_ir::Transform2D {
                    position: [2.0, 0.0],
                    ..render_ir::Transform2D::default()
                },
                effects: Vec::new(),
            }],
        };

        let mut footage = CheckerboardFootageProvider;
        let (frame, trace) = render_frame_with_footage_traced(&scene, 0, &mut footage).unwrap();

        assert_eq!(frame.pixel(2, 0), [255, 0, 0, 255]);
        let collapsed_precomp = trace
            .collapse
            .iter()
            .find(|record| record["mode"] == "collapse_supported_vectors")
            .unwrap();
        assert_eq!(
            collapsed_precomp["parent_matrix_report"]["translation"],
            json!([2.0, 0.0])
        );
        assert_eq!(
            collapsed_precomp["deferred_raster_checkpoint"]["status"],
            json!("matrix_pushdown_only")
        );
    }

    #[test]
    fn collapsed_plain_text_uses_p6_vector_deferred_pixels() {
        let _guard = test_env_lock();
        unsafe {
            std::env::set_var(COLLAPSED_TEXT_VECTOR_DEFERRED_ENV_VAR, "1");
        }
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf");
        if !font_path.exists() {
            unsafe {
                std::env::remove_var(COLLAPSED_TEXT_VECTOR_DEFERRED_ENV_VAR);
            }
            return;
        }
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 256,
                height: 256,
                fps: 30.0,
                duration: 1.0 / 30.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "text_child".to_string(),
                    width: 128,
                    height: 128,
                    fps: 30.0,
                    duration: 1.0 / 30.0,
                    background: [0, 0, 0, 0],
                    motion_blur: render_ir::MotionBlurSettings::default(),
                },
                layers: vec![Layer::Text {
                    id: "glyph_o".to_string(),
                    start: 0.0,
                    duration: 1.0 / 30.0,
                    text: "O".to_string(),
                    font: font_path.display().to_string(),
                    fontSize: 48.0,
                    fill: [255, 255, 255, 255],
                    box_: Some(Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 128.0,
                        h: 128.0,
                    }),
                    transform: render_ir::Transform2D::default(),
                    text_animators: Vec::new(),
                    effects: Vec::new(),
                }],
            }],
            assets: Vec::new(),
            layers: vec![Layer::Precomp {
                id: "collapsed_text_child".to_string(),
                start: 0.0,
                duration: 1.0 / 30.0,
                composition: "text_child".to_string(),
                collapse_transformations: true,
                transform: render_ir::Transform2D {
                    scale: [180.0, 180.0],
                    ..render_ir::Transform2D::default()
                },
                effects: Vec::new(),
            }],
        };

        let mut footage = CheckerboardFootageProvider;
        let (frame, trace) = render_frame_with_footage_traced(&scene, 0, &mut footage).unwrap();

        assert!(frame.data.chunks_exact(4).any(|pixel| pixel[3] > 0));
        let text_layout = trace
            .text_layouts
            .iter()
            .find(|record| record["render_path"] == "collapsed_text_vector_deferred")
            .expect("collapsed plain text should use vector-deferred P6 path");
        assert!(text_layout["request"]["font_size"]
            .as_f64()
            .is_some_and(|font_size| font_size > 48.0));
        assert_eq!(
            text_layout["draw_char"]["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|event| event["stage"] == "p6_ad68_pixel_write")
                .unwrap()["payload"]["vector_transform"]["font_size_baked_scale"],
            json!(true)
        );
        let collapsed_text = trace
            .collapse
            .iter()
            .find(|record| record["mode"] == "collapsed_text_vector_deferred")
            .unwrap();
        assert_eq!(
            collapsed_text["deferred_raster_checkpoint"]["status"],
            json!("deferred_text_vector_matrix_carrier")
        );
        let bee_route = trace
            .collapse
            .iter()
            .find(|record| {
                record["mode"] == "bee_text_carrier_route"
                    && record["bee_text_carrier_route"]["status"] == "bee_text_carrier_required"
            })
            .expect("collapsed text should declare the BEE carrier owner");
        assert_eq!(
            bee_route["bee_text_carrier_route"]["source_layer_id"],
            json!("glyph_o")
        );
        assert_eq!(
            bee_route["bee_text_carrier_route"]["counts_as_direct_p6_success"],
            json!(false)
        );
        let routed_route = trace
            .collapse
            .iter()
            .find(|record| {
                record["mode"] == "bee_text_carrier_route"
                    && record["bee_text_carrier_route"]["status"] == "bee_text_carrier_routed"
            })
            .expect("collapsed text vector path should route the BEE carrier payload");
        assert_eq!(
            routed_route["bee_text_carrier_route"]["source_layer_id"],
            json!("glyph_o")
        );
        assert_eq!(
            routed_route["bee_text_carrier_route"]["payload_bridge"],
            json!("layer_text_source_outline_to_p6_ad68_vector_transform")
        );
        assert!(
            routed_route["bee_text_carrier_route"]["pixel_write_count"]
                .as_u64()
                .unwrap_or(0)
                > 0
        );
        unsafe {
            std::env::remove_var(COLLAPSED_TEXT_VECTOR_DEFERRED_ENV_VAR);
        }
    }

    #[test]
    fn rasterized_precomp_text_disables_p6_pixels_for_source_input() {
        let _guard = test_env_lock();
        unsafe {
            std::env::remove_var(COLLAPSED_TEXT_VECTOR_DEFERRED_ENV_VAR);
            std::env::set_var("AE_NATIVE_RENDERER_P6_AD68_TEXT_PIXELS_OPT_IN", "1");
        }
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf");
        if !font_path.exists() {
            unsafe {
                std::env::remove_var("AE_NATIVE_RENDERER_P6_AD68_TEXT_PIXELS_OPT_IN");
            }
            return;
        }
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 256,
                height: 256,
                fps: 30.0,
                duration: 1.0 / 30.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "text_child".to_string(),
                    width: 128,
                    height: 128,
                    fps: 30.0,
                    duration: 1.0 / 30.0,
                    background: [0, 0, 0, 0],
                    motion_blur: render_ir::MotionBlurSettings::default(),
                },
                layers: vec![Layer::Text {
                    id: "glyph_o".to_string(),
                    start: 0.0,
                    duration: 1.0 / 30.0,
                    text: "O".to_string(),
                    font: font_path.display().to_string(),
                    fontSize: 48.0,
                    fill: [255, 255, 255, 255],
                    box_: Some(Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 128.0,
                        h: 128.0,
                    }),
                    transform: render_ir::Transform2D::default(),
                    text_animators: Vec::new(),
                    effects: Vec::new(),
                }],
            }],
            assets: Vec::new(),
            layers: vec![Layer::Precomp {
                id: "rasterized_text_child".to_string(),
                start: 0.0,
                duration: 1.0 / 30.0,
                composition: "text_child".to_string(),
                collapse_transformations: false,
                transform: render_ir::Transform2D::default(),
                effects: Vec::new(),
            }],
        };

        let mut footage = CheckerboardFootageProvider;
        let (frame, trace) = render_frame_with_footage_traced(&scene, 0, &mut footage).unwrap();

        assert!(frame.data.chunks_exact(4).any(|pixel| pixel[3] > 0));
        let text_layout = trace
            .text_layouts
            .iter()
            .find(|record| record["render_path"] == "layer_text_precomp_raster_input_p6_disabled")
            .expect("precomp source text should use the M17-safe P6-disabled text path");
        let events = text_layout["draw_char"]["events"].as_array().unwrap();
        assert!(events.iter().any(|event| {
            event["stage"] == "p6_ad68_pixel_opt_in_flag"
                && event["payload"]["state"] == "disabled_explicit"
                && event["payload"]["enabled"] == false
        }));
        assert!(
            !events
                .iter()
                .any(|event| event["stage"] == "p6_ad68_pixel_write"),
            "precomp source text must not count P6 AD68 pixels as M17/P5 proof yet"
        );
        let bee_route = trace
            .collapse
            .iter()
            .find(|record| {
                record["mode"] == "bee_text_carrier_route"
                    && record["bee_text_carrier_route"]["status"] == "precomp_rasterize_first"
            })
            .expect("rasterized precomp should declare non-carrier route status");
        assert_eq!(
            bee_route["bee_text_carrier_route"]["reason"],
            json!("collapse_not_requested")
        );
        unsafe {
            std::env::remove_var("AE_NATIVE_RENDERER_P6_AD68_TEXT_PIXELS_OPT_IN");
        }
    }

    fn posterize_effect(frame_rate: f32) -> EffectSpec {
        EffectSpec {
            match_name: "ADBE Posterize Time".to_string(),
            params: json!({ "0001": frame_rate }),
        }
    }

    fn minimax_effect(params: serde_json::Value) -> EffectSpec {
        EffectSpec {
            match_name: "ADBE Minimax".to_string(),
            params,
        }
    }

    fn effect(match_name: &str, params: serde_json::Value) -> EffectSpec {
        EffectSpec {
            match_name: match_name.to_string(),
            params,
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-9,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_close_f32(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1.0e-4,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_close_slice(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_close(*actual, *expected);
        }
    }
}
