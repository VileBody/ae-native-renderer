use crate::motion_blur;
use effects::{posterize_time::PosterizeTimeParams, EffectContext, EffectRegistry};
use raster_cpu::{composite_normal, BilinearSampler, Canvas, Sampler};
use render_ir::{
    Composition, EffectSpec, Layer, PositionExpression, Rect, ScalarKeyframe, Scene,
    TextAnimatorSpec, TextExpressionSelector, TextSelectorBasedOn, TextSelectorShape, Vec2Keyframe,
};
use serde_json::{json, Value};
use std::time::Instant;
use text_engine::{layout_text, rasterize_text, TextLayoutRequest, TextLayoutResult};
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
    pub motion_blur: Vec<MotionBlurTrace>,
    pub effect_debug: Vec<EffectDebugRecord>,
    pub text_layouts: Vec<Value>,
    pub text_selector_weights: Vec<Value>,
    pub position_expressions: Vec<Value>,
    pub collapse: Vec<Value>,
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
    pub adjustment_lower_stack_time: Option<f64>,
    pub posterize: Option<PosterizeTiming>,
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
        sample_traces.push(MotionBlurSampleTrace {
            sample_index,
            sample_time,
            layer_time: sample_layer_time,
            posterized_time: sample_posterized_time,
            source_time: sample_source_time,
            source_frame_id: sample_source_time.and_then(|time| source_frame_id(time, comp.fps)),
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
        source_frame_id: source_time.and_then(|time| source_frame_id(time, comp.fps)),
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

fn source_frame_id(source_time: f64, fps: f64) -> Option<u32> {
    if !source_time.is_finite() || !fps.is_finite() || fps <= 0.0 {
        return None;
    }
    let frame = ((source_time * fps) + 1.0e-9).floor();
    if !(0.0..=u32::MAX as f64).contains(&frame) {
        return None;
    }
    Some(frame as u32)
}

fn render_layer_stub(
    scene: &Scene,
    comp: &Composition,
    layer: &Layer,
    time: f64,
    footage: &mut dyn FootageProvider,
    stack: &mut Vec<String>,
    mut trace: Option<&mut FrameRenderTrace>,
) -> anyhow::Result<Canvas> {
    let layer_started = Instant::now();
    let content_started = Instant::now();
    let layer_time = posterized_time_for_effects(effects_of(layer), time);
    record_layer_temporal_trace(trace.as_deref_mut(), comp, layer, time, layer_time);
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
            record_transform_expression_trace(
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
            record_transform_expression_trace(
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
            record_text_layout_trace(
                trace.as_deref_mut(),
                &comp.id,
                id,
                time,
                layer_time,
                &request,
                [local_width, local_height],
                1.0,
                layout.as_ref(),
                "layer_text",
            );
            let mut text_canvas = rasterize_text(&request, local_width, local_height, *fill)?;
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
            record_transform_expression_trace(
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
                    transform_canvas(&frame, comp.width, comp.height, &evaluated, [0.0, 0.0])
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
                    trace.as_deref_mut(),
                )?;
                stack.pop();
                let evaluated =
                    evaluate_transform(transform, layer_time, *start, *duration, comp.fps);
                record_transform_expression_trace(
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
    let effects_ms = elapsed_ms(effects_started);
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
    record_transform_expression_trace(
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
            record_transform_expression_trace(
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
            record_transform_expression_trace(
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
            let raster_scale = matrix_scale_hint(matrix).clamp(1.0, 4.0);
            let local_width = canvas_dim(rect.w * raster_scale);
            let local_height = canvas_dim(rect.h * raster_scale);
            let request = TextLayoutRequest {
                text: text.clone(),
                font_id: font.clone(),
                font_size: *fontSize * raster_scale,
                box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
            };
            let layout = layout_text(&request).ok();
            record_text_layout_trace(
                trace.as_deref_mut(),
                &parent_comp.id,
                id,
                time,
                time,
                &request,
                [local_width, local_height],
                raster_scale,
                layout.as_ref(),
                "collapsed_text",
            );
            let mut text_canvas = rasterize_text(&request, local_width, local_height, *fill)?;
            let reveal = evaluate_scalar_keyframes(&transform.animation.reveal, time, 100.0);
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
            record_transform_expression_trace(
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
        let input_hash = trace.as_ref().map(|_| canvas_hash(&canvas));
        let debug_trace = trace.as_ref().and_then(|_| {
            effect_debug_trace_json(&spec.match_name, &canvas, &spec.params, param_time)
        });
        let started = Instant::now();
        canvas = effect.render(
            &canvas,
            &EffectContext {
                time: param_time,
                fps,
            },
            &spec.params,
        )?;
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
                    "kernel_radius": trace.params.kernel_radius
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "horizontal_pass_rgba": debug_hash_hex(trace.hashes.horizontal_pass_rgba),
                    "output_rgba": debug_hash_hex(trace.hashes.output_rgba)
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
                    "blur_radius": trace.params.blur_radius
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "source_alpha_rgba": debug_hash_hex(trace.hashes.source_alpha_rgba),
                    "raw_offset_shadow_rgba": debug_hash_hex(trace.hashes.raw_offset_shadow_rgba),
                    "blurred_shadow_rgba": debug_hash_hex(trace.hashes.blurred_shadow_rgba),
                    "final_rgba": debug_hash_hex(trace.hashes.final_rgba)
                }
            }))
        }
        "ADBE Glo2" => {
            let trace = effects::glow::glow_debug_trace(input, params, time);
            Some(json!({
                "params": {
                    "threshold": trace.params.threshold,
                    "radius": trace.params.radius,
                    "intensity": trace.params.intensity,
                    "kernel_radius": trace.params.kernel_radius
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "threshold_source_rgba": debug_hash_hex(trace.hashes.threshold_source_rgba),
                    "blurred_glow_rgba": debug_hash_hex(trace.hashes.blurred_glow_rgba),
                    "intensity_scaled_glow_rgba": debug_hash_hex(trace.hashes.intensity_scaled_glow_rgba),
                    "final_rgba": debug_hash_hex(trace.hashes.final_rgba)
                }
            }))
        }
        "ADBE Minimax" => {
            let trace = effects::minimax::minimax_debug_trace(input, params, time);
            Some(json!({
                "params": {
                    "operation": trace.params.operation,
                    "channels": trace.params.channels,
                    "radius": trace.params.radius,
                    "kernel_radius": trace.params.kernel_radius
                },
                "hashes": {
                    "input_rgba": debug_hash_hex(trace.hashes.input_rgba),
                    "output_rgba": debug_hash_hex(trace.hashes.output_rgba)
                }
            }))
        }
        _ => None,
    }
}

fn debug_hash_hex(hash: u64) -> String {
    format!("0x{hash:016x}")
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
    envelope: f32,
    offset: [f32; 2],
    source: String,
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
                envelope: k,
                offset: [x, y],
                source: source.clone(),
            }
        }
    }
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
        "context": {
            "local_time": sample.local_time,
            "frame_duration": sample.frame_duration,
            "duration": sample.layer_duration
        },
        "expression": match expression {
            PositionExpression::EdgeWobble { intro, outro, amp, freq, .. } => json!({
                "type": "edge_wobble",
                "source": sample.source,
                "intro": intro,
                "outro": outro,
                "amp": amp,
                "freq": freq,
                "envelope": sample.envelope,
                "offset": sample.offset
            }),
        }
    }));
}

fn evaluate_vec2_keyframes(keyframes: &[Vec2Keyframe], time: f64, fallback: [f32; 2]) -> [f32; 2] {
    if keyframes.is_empty() {
        return fallback;
    }
    if time <= keyframes[0].time {
        return keyframes[0].value;
    }
    for pair in keyframes.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        if time <= b.time {
            if a.hold || b.time <= a.time {
                return a.value;
            }
            let t = eased_progress(
                ((time - a.time) / (b.time - a.time)).clamp(0.0, 1.0) as f32,
                a.ease,
            );
            return [
                a.value[0] + (b.value[0] - a.value[0]) * t,
                a.value[1] + (b.value[1] - a.value[1]) * t,
            ];
        }
    }
    keyframes.last().map(|key| key.value).unwrap_or(fallback)
}

fn evaluate_scalar_keyframes(keyframes: &[ScalarKeyframe], time: f64, fallback: f32) -> f32 {
    if keyframes.is_empty() {
        return fallback;
    }
    if time <= keyframes[0].time {
        return keyframes[0].value;
    }
    for pair in keyframes.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        if time <= b.time {
            if a.hold || b.time <= a.time {
                return a.value;
            }
            let t = eased_progress(
                ((time - a.time) / (b.time - a.time)).clamp(0.0, 1.0) as f32,
                a.ease,
            );
            return a.value + (b.value - a.value) * t;
        }
    }
    keyframes.last().map(|key| key.value).unwrap_or(fallback)
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
    layout: Option<&TextLayoutResult>,
    render_path: &str,
) {
    let Some(trace) = trace else {
        return;
    };
    let layout_telemetry = layout
        .map(|layout| {
            serde_json::to_value(&layout.telemetry)
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
        "layout": layout_telemetry
    }));
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
    let transforms_units = animator.position.is_some()
        || animator.scale.is_some()
        || animator.rotation.is_some()
        || animator.blur.is_some()
        || animator.expression_selector.is_some();
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
                "expression": expression.to_json()
            });
            if transforms_units {
                record["animator_contribution"] =
                    animator_contribution_json(*unit, animator, final_weight, unit_scale);
            }
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
        "raster_size": raster_size
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
        "flattened_layers": flattened_layers
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
        "effective_raster_scale": raster_scale,
        "raster_size": raster_size,
        "sharpness_probe": alpha_sharpness_probe(text_canvas)
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
    if pos < start || pos > end {
        return 0.0;
    }
    let span = (end - start).max(0.0001);
    let t = ((pos - start) / span).clamp(0.0, 1.0);
    let mut weight = match selector.shape {
        TextSelectorShape::Square => square_selector_weight(pos, start, end, selector.smoothness),
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

fn square_selector_weight(pos: f32, start: f32, end: f32, smoothness: f32) -> f32 {
    if smoothness <= 0.0 {
        return 1.0;
    }
    let edge = ((end - start) * (smoothness / 100.0) * 0.5).max(0.0001);
    let fade_in = ((pos - start) / edge).clamp(0.0, 1.0);
    let fade_out = ((end - pos) / edge).clamp(0.0, 1.0);
    fade_in.min(fade_out)
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
            ..
        }) => {
            let text_index = index as f32 + 1.0;
            let t = (time - layer_start) as f32 - *delay * text_index;
            if t < 0.0 {
                return TextExpressionWeightDetail {
                    weight: 0.0,
                    raw_amount: Some(0.0),
                    clamped_amount: Some(0.0),
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
    blur_radius: i32,
    matrix: Mat3,
}

const TEXT_ANIMATOR_BLUR_RADIUS_LIMIT: i32 = 128;

fn animator_unit_transform(
    unit: UnitRect,
    animator: &TextAnimatorSpec,
    weight: f32,
    unit_scale: f32,
) -> AnimatorUnitTransform {
    let cx = (unit.x0 + unit.x1) as f32 * 0.5;
    let cy = (unit.y0 + unit.y1) as f32 * 0.5;
    let position = animator.position.unwrap_or([0.0, 0.0]);
    let scale = animator.scale.unwrap_or([100.0, 100.0]);
    let sx = (100.0 + (scale[0] - 100.0) * weight) / 100.0;
    let sy = (100.0 + (scale[1] - 100.0) * weight) / 100.0;
    let rotation = animator.rotation.unwrap_or(0.0) * weight;
    let tx = position[0] * weight * unit_scale;
    let ty = position[1] * weight * unit_scale;
    let alpha_scale = animator_alpha_scale(animator.opacity, weight);
    let blur_radius = text_animator_blur_radius(animator, weight, unit_scale);
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
        blur_radius,
        matrix,
    }
}

fn text_animator_blur_radius(animator: &TextAnimatorSpec, weight: f32, unit_scale: f32) -> i32 {
    animator
        .blur
        .map(|blur| {
            (blur[0].abs().max(blur[1].abs()) * weight.max(0.0) * unit_scale.abs()).round() as i32
        })
        .unwrap_or(0)
        .clamp(0, TEXT_ANIMATOR_BLUR_RADIUS_LIMIT)
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
        "position": transform.position,
        "scale": transform.scale,
        "rotation": transform.rotation,
        "opacity_alpha_scale": transform.alpha_scale,
        "blur_radius": transform.blur_radius,
        "matrix": transform.matrix.m
    })
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

    for y in unit.y0..unit.y1 {
        for x in unit.x0..unit.x1 {
            let mut pixel = input.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            pixel[3] = (pixel[3] as f32 * transform.alpha_scale)
                .round()
                .clamp(0.0, 255.0) as u8;
            let p = transform
                .matrix
                .transform_point(Vec2::new(x as f32, y as f32));
            let dx = p.x.round() as i32;
            let dy = p.y.round() as i32;
            splat_blurred_pixel(output, dx, dy, pixel, transform.blur_radius);
        }
    }
}

fn splat_blurred_pixel(output: &mut Canvas, x: i32, y: i32, pixel: [u8; 4], radius: i32) {
    if radius <= 0 {
        if x < 0 || y < 0 || x >= output.width as i32 || y >= output.height as i32 {
            return;
        }
        output.set_pixel(x as u32, y as u32, pixel);
        return;
    }
    let divisor = ((radius * 2 + 1) * (radius * 2 + 1)).max(1) as f32;
    for oy in -radius..=radius {
        for ox in -radius..=radius {
            let dx = x + ox;
            let dy = y + oy;
            if dx < 0 || dy < 0 || dx >= output.width as i32 || dy >= output.height as i32 {
                continue;
            }
            let mut blurred = pixel;
            blurred[3] = (blurred[3] as f32 / divisor).round().max(1.0) as u8;
            let existing = output.pixel(dx as u32, dy as u32);
            let combined_alpha = existing[3].saturating_add(blurred[3]);
            blurred[3] = combined_alpha;
            output.set_pixel(dx as u32, dy as u32, blurred);
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
        .filter(|glyph| glyph_char(text, glyph.char_index).is_some_and(|ch| !ch.is_whitespace()))
        .filter_map(|glyph| rect_from_bbox(canvas, glyph.bbox))
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
    let mut groups: Vec<(usize, [f32; 4])> = Vec::new();
    for glyph in &layout.glyphs {
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
        if let Some((_, bounds)) = groups.iter_mut().find(|(group_key, _)| *group_key == key) {
            bounds[0] = bounds[0].min(x0);
            bounds[1] = bounds[1].min(y0);
            bounds[2] = bounds[2].max(x1);
            bounds[3] = bounds[3].max(y1);
        } else {
            groups.push((key, [x0, y0, x1, y1]));
        }
    }
    groups.sort_by_key(|(key, _)| *key);
    let rects = groups
        .into_iter()
        .filter_map(|(_, bounds)| {
            rect_from_bbox(
                canvas,
                [
                    bounds[0],
                    bounds[1],
                    (bounds[2] - bounds[0]).max(0.0),
                    (bounds[3] - bounds[1]).max(0.0),
                ],
            )
        })
        .collect::<Vec<_>>();
    with_unit_totals(rects)
}

fn rect_from_bbox(canvas: &Canvas, bbox: [f32; 4]) -> Option<UnitRect> {
    let x0 = bbox[0].floor().max(0.0).min(canvas.width as f32) as u32;
    let y0 = bbox[1].floor().max(0.0).min(canvas.height as f32) as u32;
    let x1 = (bbox[0] + bbox[2]).ceil().max(0.0).min(canvas.width as f32) as u32;
    let y1 = (bbox[1] + bbox[3])
        .ceil()
        .max(0.0)
        .min(canvas.height as f32) as u32;
    (x0 < x1 && y0 < y1).then_some(UnitRect {
        x0,
        y0,
        x1,
        y1,
        index: 0,
        total: 0,
    })
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
    use render_ir::{Composition, CompositionNode, TextRangeSelector};
    use serde_json::json;

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
    fn text_animator_blur_uses_weighted_pixel_radius() {
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

        assert_eq!(text_animator_blur_radius(&animator, 1.0, 1.0), 10);
        assert_eq!(text_animator_blur_radius(&animator, 0.45, 1.0), 5);
        assert_eq!(text_animator_blur_radius(&animator, -1.0, 1.0), 0);
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

        let frame = render_frame(&scene, 0).unwrap();

        assert_eq!(frame.pixel(2, 0), [255, 0, 0, 255]);
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

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-9,
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
