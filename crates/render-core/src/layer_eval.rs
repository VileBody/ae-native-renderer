use crate::motion_blur;
use effects::{posterize_time::PosterizeTimeParams, EffectContext, EffectRegistry};
use raster_cpu::{composite_normal, BilinearSampler, Canvas, Sampler, AE_ALPHA_GAIN_EPSILON};
use render_ir::{
    BlendMode, Composition, EffectSpec, Layer, PositionExpression, Rect, ScalarKeyframe, Scene,
    TextAnimatorSpec, TextExpressionSelector, TextPaintSpec, TextSelectorBasedOn,
    TextSelectorShape, Vec2Keyframe, TEXT_PAINT_MATCH_NAME,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use text_engine::{
    layout_text, rasterize_text_with_layout_and_paint, rasterize_text_with_paint,
    GlyphLayoutTelemetry, TextJustification as LayoutTextJustification, TextLayoutRequest,
    TextLayoutResult, TextPaintStyle, TextRasterTrace,
};
use transform_math::{Mat3, Transform2D, Vec2};

pub trait FootageProvider {
    fn frame_at(&mut self, source: &str, time: f64) -> anyhow::Result<Option<Canvas>>;

    /// Returns immutable source frames for a contiguous render batch when the provider can
    /// decode them ahead of time. The default keeps custom/test providers sequential.
    fn prefetch_frames(
        &mut self,
        _frames: &[u32],
    ) -> anyhow::Result<Option<PrefetchedFootageProvider>> {
        Ok(None)
    }
}

const PREFETCH_TIME_SCALE: f64 = 1_000_000.0;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PrefetchedFootageKey {
    source: String,
    time_key: i64,
}

/// Immutable footage snapshot for a render batch. Each worker gets a cheap clone and never
/// touches a stateful decoder while rendering.
#[derive(Debug, Clone, Default)]
pub struct PrefetchedFootageProvider {
    frames: Arc<BTreeMap<PrefetchedFootageKey, Arc<Canvas>>>,
}

impl PrefetchedFootageProvider {
    pub fn insert(&mut self, source: impl Into<String>, time: f64, frame: Canvas) {
        let key = PrefetchedFootageKey {
            source: source.into(),
            time_key: prefetch_time_key(time),
        };
        Arc::make_mut(&mut self.frames).insert(key, Arc::new(frame));
    }
}

impl FootageProvider for PrefetchedFootageProvider {
    fn frame_at(&mut self, source: &str, time: f64) -> anyhow::Result<Option<Canvas>> {
        let key = PrefetchedFootageKey {
            source: source.to_string(),
            time_key: prefetch_time_key(time),
        };
        Ok(self.frames.get(&key).map(|frame| (**frame).clone()))
    }
}

fn prefetch_time_key(time: f64) -> i64 {
    (time.max(0.0) * PREFETCH_TIME_SCALE).round() as i64
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
    pub capture_effect_debug: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LowerStackCacheKey {
    composition: String,
    adjustment_layer: String,
    time_bits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PrecompCacheKey {
    composition: String,
    time_bits: u64,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TextCanvasCacheKey {
    composition: String,
    layer: String,
    width: u32,
    height: u32,
}

/// Per-render CPU state. The cache is deliberately small enough to stay well below the
/// production RSS budget while retaining adjacent Posterize Time buckets.
#[derive(Debug)]
pub struct EffectRuntime {
    lower_stack_cache: BTreeMap<LowerStackCacheKey, Arc<Canvas>>,
    lower_stack_lru: VecDeque<LowerStackCacheKey>,
    lower_stack_bytes: usize,
    max_lower_stack_bytes: usize,
    precomp_cache: BTreeMap<PrecompCacheKey, Arc<Canvas>>,
    precomp_lru: VecDeque<PrecompCacheKey>,
    precomp_bytes: usize,
    max_precomp_bytes: usize,
    text_cache: BTreeMap<TextCanvasCacheKey, Arc<Canvas>>,
    text_lru: VecDeque<TextCanvasCacheKey>,
    text_bytes: usize,
    max_text_bytes: usize,
    pub ping: Canvas,
    pub pong: Canvas,
    pub worker_scratch: Vec<Vec<f32>>,
}

impl Default for EffectRuntime {
    fn default() -> Self {
        let workers = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(8);
        Self {
            lower_stack_cache: BTreeMap::new(),
            lower_stack_lru: VecDeque::new(),
            lower_stack_bytes: 0,
            max_lower_stack_bytes: 192 * 1024 * 1024,
            precomp_cache: BTreeMap::new(),
            precomp_lru: VecDeque::new(),
            precomp_bytes: 0,
            max_precomp_bytes: 128 * 1024 * 1024,
            text_cache: BTreeMap::new(),
            text_lru: VecDeque::new(),
            text_bytes: 0,
            max_text_bytes: 64 * 1024 * 1024,
            ping: Canvas::transparent(0, 0),
            pong: Canvas::transparent(0, 0),
            worker_scratch: (0..workers).map(|_| Vec::new()).collect(),
        }
    }
}

impl EffectRuntime {
    fn lower_stack(&mut self, key: &LowerStackCacheKey) -> Option<Arc<Canvas>> {
        let canvas = self.lower_stack_cache.get(key).cloned()?;
        self.lower_stack_lru.retain(|candidate| candidate != key);
        self.lower_stack_lru.push_back(key.clone());
        Some(canvas)
    }

    fn store_lower_stack(&mut self, key: LowerStackCacheKey, canvas: Arc<Canvas>) {
        let canvas_bytes = canvas.data.len();
        if canvas_bytes > self.max_lower_stack_bytes {
            return;
        }
        if let Some(replaced) = self.lower_stack_cache.insert(key.clone(), canvas) {
            self.lower_stack_bytes = self.lower_stack_bytes.saturating_sub(replaced.data.len());
        }
        self.lower_stack_bytes += canvas_bytes;
        self.lower_stack_lru.retain(|candidate| candidate != &key);
        self.lower_stack_lru.push_back(key);
        while self.lower_stack_bytes > self.max_lower_stack_bytes {
            let Some(oldest) = self.lower_stack_lru.pop_front() else {
                break;
            };
            if let Some(removed) = self.lower_stack_cache.remove(&oldest) {
                self.lower_stack_bytes = self.lower_stack_bytes.saturating_sub(removed.data.len());
            }
        }
    }

    fn precomp(&mut self, key: &PrecompCacheKey) -> Option<Arc<Canvas>> {
        let canvas = self.precomp_cache.get(key).cloned()?;
        self.precomp_lru.retain(|candidate| candidate != key);
        self.precomp_lru.push_back(key.clone());
        Some(canvas)
    }

    fn store_precomp(&mut self, key: PrecompCacheKey, canvas: Arc<Canvas>) {
        let canvas_bytes = canvas.data.len();
        if canvas_bytes > self.max_precomp_bytes {
            return;
        }
        if let Some(replaced) = self.precomp_cache.insert(key.clone(), canvas) {
            self.precomp_bytes = self.precomp_bytes.saturating_sub(replaced.data.len());
        }
        self.precomp_bytes += canvas_bytes;
        self.precomp_lru.retain(|candidate| candidate != &key);
        self.precomp_lru.push_back(key);
        while self.precomp_bytes > self.max_precomp_bytes {
            let Some(oldest) = self.precomp_lru.pop_front() else {
                break;
            };
            if let Some(removed) = self.precomp_cache.remove(&oldest) {
                self.precomp_bytes = self.precomp_bytes.saturating_sub(removed.data.len());
            }
        }
    }

    fn text_canvas(&mut self, key: &TextCanvasCacheKey) -> Option<Arc<Canvas>> {
        let canvas = self.text_cache.get(key).cloned()?;
        self.text_lru.retain(|candidate| candidate != key);
        self.text_lru.push_back(key.clone());
        Some(canvas)
    }

    fn store_text_canvas(&mut self, key: TextCanvasCacheKey, canvas: Arc<Canvas>) {
        let canvas_bytes = canvas.data.len();
        if canvas_bytes > self.max_text_bytes {
            return;
        }
        if let Some(replaced) = self.text_cache.insert(key.clone(), canvas) {
            self.text_bytes = self.text_bytes.saturating_sub(replaced.data.len());
        }
        self.text_bytes += canvas_bytes;
        self.text_lru.retain(|candidate| candidate != &key);
        self.text_lru.push_back(key);
        while self.text_bytes > self.max_text_bytes {
            let Some(oldest) = self.text_lru.pop_front() else {
                break;
            };
            if let Some(removed) = self.text_cache.remove(&oldest) {
                self.text_bytes = self.text_bytes.saturating_sub(removed.data.len());
            }
        }
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
    let mut runtime = EffectRuntime::default();
    render_frame_with_footage_runtime(scene, frame_index, footage, &mut runtime)
}

fn render_frame_with_footage_runtime(
    scene: &Scene,
    frame_index: u32,
    footage: &mut dyn FootageProvider,
    runtime: &mut EffectRuntime,
) -> anyhow::Result<Canvas> {
    let comp = &scene.composition;
    let time = frame_index as f64 / comp.fps;
    render_composition_frame(
        scene,
        comp,
        &scene.layers,
        time,
        footage,
        runtime,
        &mut Vec::new(),
        None,
    )
}

pub fn render_frame_with_footage_traced(
    scene: &Scene,
    frame_index: u32,
    footage: &mut dyn FootageProvider,
) -> anyhow::Result<(Canvas, FrameRenderTrace)> {
    render_frame_with_footage_profiled(scene, frame_index, footage, true)
}

pub fn render_frame_with_footage_profiled(
    scene: &Scene,
    frame_index: u32,
    footage: &mut dyn FootageProvider,
    capture_effect_debug: bool,
) -> anyhow::Result<(Canvas, FrameRenderTrace)> {
    let mut runtime = EffectRuntime::default();
    render_frame_with_footage_profiled_runtime(
        scene,
        frame_index,
        footage,
        &mut runtime,
        capture_effect_debug,
    )
}

pub fn render_frame_with_footage_profiled_runtime(
    scene: &Scene,
    frame_index: u32,
    footage: &mut dyn FootageProvider,
    runtime: &mut EffectRuntime,
    capture_effect_debug: bool,
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
        capture_effect_debug,
    };
    let canvas = render_composition_frame(
        scene,
        comp,
        &scene.layers,
        time,
        footage,
        runtime,
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
    runtime: &mut EffectRuntime,
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
            let lower_stack_canvas = if time_changed(lower_stack_time, time) {
                let key = LowerStackCacheKey {
                    composition: comp.id.clone(),
                    adjustment_layer: layer.id().to_string(),
                    time_bits: lower_stack_time.to_bits(),
                };
                if let Some(cached) = runtime.lower_stack(&key) {
                    Some(cached)
                } else {
                    let rendered = Arc::new(render_composition_frame(
                        scene,
                        comp,
                        &layers[layer_index + 1..],
                        lower_stack_time,
                        footage,
                        runtime,
                        stack,
                        None,
                    )?);
                    runtime.store_lower_stack(key, Arc::clone(&rendered));
                    Some(rendered)
                }
            } else {
                None
            };
            let input_canvas = lower_stack_canvas.as_deref().unwrap_or(&canvas);
            canvas = apply_adjustment_effects_to_canvas(
                effects,
                input_canvas,
                AdjustmentTimeRouting {
                    comp_time: time,
                    layer_time,
                    lower_stack_time,
                    posterize,
                },
                comp.fps,
                runtime,
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
        // Posterize Time samples footage/source pixels. AE keeps a text layer's
        // own animator and opacity clocks live, then applies its pixel effects.
        let layer_opacity = if matches!(layer, Layer::Text { .. }) {
            opacity_of(layer, time)
        } else {
            opacity_of(layer, layer_time)
        };
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
                runtime,
                stack,
                trace.as_deref_mut(),
            )?;
            composite_layer(&mut canvas, &layer_canvas, 100.0, layer.blend_mode());
        } else {
            let layer_canvas = render_layer_stub(
                scene,
                comp,
                layer,
                time,
                footage,
                runtime,
                stack,
                trace.as_deref_mut(),
            )?;
            composite_layer(
                &mut canvas,
                &layer_canvas,
                layer_opacity,
                layer.blend_mode(),
            );
        }
    }

    Ok(canvas)
}

fn composite_layer(dst: &mut Canvas, src: &Canvas, opacity_percent: f32, blend_mode: BlendMode) {
    if blend_mode == BlendMode::Normal {
        composite_normal(dst, src, opacity_percent);
        return;
    }

    let opacity = (opacity_percent / 100.0).clamp(0.0, 1.0);
    if opacity <= AE_ALPHA_GAIN_EPSILON {
        return;
    }

    let width = dst.width.min(src.width);
    let height = dst.height.min(src.height);
    for y in 0..height {
        for x in 0..width {
            let dst_index = ((y * dst.width + x) * 4) as usize;
            let src_index = ((y * src.width + x) * 4) as usize;
            let src_alpha = src.data[src_index + 3] as f32 / 255.0 * opacity;
            if src_alpha <= 0.0 {
                continue;
            }

            let dst_alpha = dst.data[dst_index + 3] as f32 / 255.0;
            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
            for channel in 0..3 {
                let backdrop = dst.data[dst_index + channel] as f32 / 255.0;
                let source = src.data[src_index + channel] as f32 / 255.0;
                let blended = match blend_mode {
                    BlendMode::Add => (backdrop + source).min(1.0),
                    BlendMode::Screen => 1.0 - (1.0 - backdrop) * (1.0 - source),
                    BlendMode::Difference => (backdrop - source).abs(),
                    BlendMode::Normal => unreachable!("normal uses composite_normal"),
                };
                let premultiplied = source * src_alpha * (1.0 - dst_alpha)
                    + backdrop * dst_alpha * (1.0 - src_alpha)
                    + blended * src_alpha * dst_alpha;
                let output = if out_alpha > 0.0 {
                    premultiplied / out_alpha
                } else {
                    backdrop
                };
                dst.data[dst_index + channel] = quantize_blend_unit(output);
            }
            dst.data[dst_index + 3] = quantize_blend_unit(out_alpha);
        }
    }
}

fn quantize_blend_unit(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
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
    runtime: &mut EffectRuntime,
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
            runtime,
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

fn render_layer_stub(
    scene: &Scene,
    comp: &Composition,
    layer: &Layer,
    time: f64,
    footage: &mut dyn FootageProvider,
    runtime: &mut EffectRuntime,
    stack: &mut Vec<String>,
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
            let c = Canvas::new(canvas_dim(rect.w), canvas_dim(rect.h), *color);
            transform_canvas(&c, comp.width, comp.height, &evaluated, [rect.x, rect.y])
        }
        Layer::Text {
            id,
            text,
            font,
            fontSize,
            char_styles,
            tracking,
            leading,
            center_source_rect_y,
            justification,
            fill,
            box_,
            transform,
            text_animators,
            effects,
            start,
            duration,
            ..
        } => {
            // Posterize Time in the layer stack holds the text animator's
            // rendered source frame. Dynamic effect parameters remain on the
            // composition clock below when the effect stack is applied.
            let text_time = layer_time;
            let evaluated = evaluate_transform(transform, text_time, *start, *duration, comp.fps);
            record_transform_sampling_trace(
                trace.as_deref_mut(),
                &comp.id,
                id,
                transform,
                text_time,
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
            let effective_tracking = evaluate_text_tracking(*tracking, text_animators, text_time);
            let paint = resolve_text_paint(*fill, effects);
            let request = TextLayoutRequest {
                text: text.clone(),
                font_id: font.clone(),
                font_size: *fontSize,
                font_overrides: char_styles
                    .iter()
                    .filter_map(|style| style.font.clone().map(|font| (style.index, font)))
                    .collect(),
                font_size_overrides: char_styles
                    .iter()
                    .filter_map(|style| style.font_size.map(|font_size| (style.index, font_size)))
                    .collect(),
                faux_italic_chars: char_styles
                    .iter()
                    .filter(|style| style.faux_italic)
                    .map(|style| style.index)
                    .collect(),
                tracking: effective_tracking,
                leading: *leading,
                center_source_rect_y: *center_source_rect_y,
                justification: layout_text_justification(*justification),
                box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
            };
            let layout = layout_text(&request).ok();
            let static_cache_key = (text_animators.is_empty()
                && transform.animation.reveal.is_empty()
                && effects
                    .iter()
                    .all(|effect| effect.match_name == TEXT_PAINT_MATCH_NAME))
            .then(|| TextCanvasCacheKey {
                composition: comp.id.clone(),
                layer: id.clone(),
                width: local_width,
                height: local_height,
            });
            let cached_text = static_cache_key
                .as_ref()
                .and_then(|key| runtime.text_canvas(key));
            let (mut text_canvas, raster_trace) = if let Some(cached) = cached_text {
                ((*cached).clone(), None)
            } else if let Some(layout) = layout.as_ref() {
                let (canvas, raster_trace) = rasterize_text_with_layout_and_paint(
                    &request,
                    layout,
                    local_width,
                    local_height,
                    paint,
                )?;
                if let Some(key) = static_cache_key.clone() {
                    runtime.store_text_canvas(key, Arc::new(canvas.clone()));
                }
                (canvas, Some(raster_trace))
            } else {
                let canvas = rasterize_text_with_paint(&request, local_width, local_height, paint)?;
                if let Some(key) = static_cache_key {
                    runtime.store_text_canvas(key, Arc::new(canvas.clone()));
                }
                (canvas, None)
            };
            record_text_layout_trace(
                trace.as_deref_mut(),
                &comp.id,
                id,
                time,
                text_time,
                &request,
                [local_width, local_height],
                1.0,
                transform_to_matrix(&evaluated).matrix(),
                [rect.x, rect.y],
                layout.as_ref(),
                raster_trace.as_ref(),
                "layer_text",
            );
            let reveal = evaluate_scalar_keyframes(&transform.animation.reveal, text_time, 100.0);
            if text_animators.is_empty() {
                apply_horizontal_reveal(&mut text_canvas, reveal);
            } else {
                text_canvas = apply_text_animators(
                    text_canvas,
                    text,
                    layout.as_ref(),
                    text_animators,
                    text_time,
                    *start,
                    1.0,
                    trace.as_deref_mut(),
                    &comp.id,
                    id,
                );
            }
            let effects_started = Instant::now();
            text_canvas = apply_effects_to_canvas(
                effects,
                &text_canvas,
                time,
                comp.fps,
                runtime,
                trace.as_deref_mut(),
                &comp.id,
                id,
            )?;
            layer_space_effects_ms = Some(elapsed_ms(effects_started));
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
                            runtime,
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
                    runtime,
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
                let source_time = (layer_time - *start).max(0.0);
                let use_cache = trace
                    .as_ref()
                    .is_some_and(|trace| !trace.capture_effect_debug);
                let cache_key = PrecompCacheKey {
                    composition: composition.clone(),
                    time_bits: source_time.to_bits(),
                    width: node.composition.width,
                    height: node.composition.height,
                };
                let precomp_canvas = if use_cache {
                    runtime.precomp(&cache_key)
                } else {
                    None
                };
                let precomp_canvas = if let Some(cached) = precomp_canvas {
                    cached
                } else {
                    stack.push(composition.clone());
                    let rendered = Arc::new(render_composition_frame(
                        scene,
                        &node.composition,
                        &node.layers,
                        source_time,
                        footage,
                        runtime,
                        stack,
                        trace.as_deref_mut(),
                    )?);
                    stack.pop();
                    if use_cache {
                        runtime.store_precomp(cache_key, Arc::clone(&rendered));
                    }
                    rendered
                };
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
                    precomp_canvas.as_ref(),
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
            runtime,
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
    runtime: &mut EffectRuntime,
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
            runtime,
            stack,
            parent_matrix,
            trace.as_deref_mut(),
        )?;
        composite_layer(
            &mut canvas,
            &child_canvas,
            opacity_of(child, source_time),
            child.blend_mode(),
        );
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
    runtime: &mut EffectRuntime,
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
            let c = Canvas::new(canvas_dim(rect.w), canvas_dim(rect.h), *color);
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
            char_styles,
            tracking,
            leading,
            center_source_rect_y,
            justification,
            fill,
            box_,
            transform,
            text_animators,
            effects,
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
            let raster_scale = matrix_scale_hint(matrix).clamp(1.0, 4.0);
            let local_width = canvas_dim(rect.w * raster_scale);
            let local_height = canvas_dim(rect.h * raster_scale);
            let effective_tracking = evaluate_text_tracking(*tracking, text_animators, time);
            let mut paint = resolve_text_paint(*fill, effects);
            paint.stroke_width *= raster_scale;
            let request = TextLayoutRequest {
                text: text.clone(),
                font_id: font.clone(),
                font_size: *fontSize * raster_scale,
                font_overrides: char_styles
                    .iter()
                    .filter_map(|style| style.font.clone().map(|font| (style.index, font)))
                    .collect(),
                font_size_overrides: char_styles
                    .iter()
                    .filter_map(|style| {
                        style
                            .font_size
                            .map(|font_size| (style.index, font_size * raster_scale))
                    })
                    .collect(),
                faux_italic_chars: char_styles
                    .iter()
                    .filter(|style| style.faux_italic)
                    .map(|style| style.index)
                    .collect(),
                tracking: effective_tracking,
                leading: leading.map(|value| value * raster_scale),
                center_source_rect_y: *center_source_rect_y,
                justification: layout_text_justification(*justification),
                box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
            };
            let layout = layout_text(&request).ok();
            let (mut text_canvas, raster_trace) = if let Some(layout) = layout.as_ref() {
                let (canvas, raster_trace) = rasterize_text_with_layout_and_paint(
                    &request,
                    layout,
                    local_width,
                    local_height,
                    paint,
                )?;
                (canvas, Some(raster_trace))
            } else {
                (
                    rasterize_text_with_paint(&request, local_width, local_height, paint)?,
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
                    runtime,
                    stack,
                    matrix,
                    trace.as_deref_mut(),
                )?;
                composite_layer(
                    &mut canvas,
                    &child_canvas,
                    opacity_of(child, source_time),
                    child.blend_mode(),
                );
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
            runtime,
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
    runtime: &mut EffectRuntime,
    mut trace: Option<&mut FrameRenderTrace>,
    composition: &str,
    layer_id: &str,
) -> anyhow::Result<Canvas> {
    let mut canvas = None::<Canvas>;
    for (effect_index, spec) in effects.iter().enumerate() {
        if spec.match_name == TEXT_PAINT_MATCH_NAME {
            continue;
        }
        if let Some(effect) = EffectRegistry::create(&spec.match_name) {
            let effect_input = canvas.as_ref().unwrap_or(input);
            let debug_trace = trace
                .as_ref()
                .is_some_and(|trace| trace.capture_effect_debug)
                .then(|| {
                    effect_debug_trace_json(&spec.match_name, effect_input, &spec.params, time)
                })
                .flatten();
            let started = Instant::now();
            effect.render_into(
                effect_input,
                &EffectContext { time, fps },
                &spec.params,
                &mut runtime.ping,
            )?;
            let rendered = std::mem::replace(
                &mut runtime.ping,
                canvas.take().unwrap_or_else(|| Canvas::transparent(0, 0)),
            );
            canvas = Some(rendered);
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
    Ok(canvas.unwrap_or_else(|| input.clone()))
}

fn apply_adjustment_effects_to_canvas(
    effects: &[EffectSpec],
    input: &Canvas,
    routing: AdjustmentTimeRouting,
    fps: f64,
    runtime: &mut EffectRuntime,
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
        let capture_diagnostics = trace
            .as_ref()
            .is_some_and(|trace| trace.capture_effect_debug);
        let input_hash = capture_diagnostics.then(|| canvas_hash(&canvas));
        let debug_trace = trace
            .as_ref()
            .is_some_and(|trace| trace.capture_effect_debug)
            .then(|| {
                effect_debug_trace_json(&spec.match_name, input_plan.input(), &params, param_time)
            })
            .flatten();
        let started = Instant::now();
        effect.render_into(
            input_plan.input(),
            &EffectContext {
                time: param_time,
                fps,
            },
            &params,
            &mut runtime.pong,
        )?;
        let rendered = std::mem::replace(&mut runtime.pong, Canvas::transparent(0, 0));
        canvas = input_plan.place_output(rendered);
        let elapsed_ms = elapsed_ms(started);
        let output_hash = capture_diagnostics.then(|| canvas_hash(&canvas));

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
                input_hash: input_hash.unwrap_or_default(),
                output_hash: output_hash.unwrap_or_default(),
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

fn layout_text_justification(
    justification: render_ir::TextJustification,
) -> LayoutTextJustification {
    match justification {
        render_ir::TextJustification::Left => LayoutTextJustification::Left,
        render_ir::TextJustification::Center => LayoutTextJustification::Center,
        render_ir::TextJustification::Full => LayoutTextJustification::Full,
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
            "subset": "generated_named_position_expression",
            "mode": "edge_wobble",
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
            "tracking": request.tracking,
            "leading": request.leading,
            "center_source_rect_y": request.center_source_rect_y,
            "justification": request.justification,
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
        if matches!(
            animator.expression_selector,
            Some(TextExpressionSelector::TrackingAmount { .. })
        ) && animator.position.is_none()
            && animator.scale.is_none()
            && animator.rotation.is_none()
            && animator.blur.is_none()
            && (animator.opacity - 100.0).abs() <= f32::EPSILON
        {
            continue;
        }
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

fn evaluate_text_tracking(base_tracking: f32, animators: &[TextAnimatorSpec], time: f64) -> f32 {
    animators.iter().fold(base_tracking, |tracking, animator| {
        let Some(TextExpressionSelector::TrackingAmount { value, keyframes }) =
            animator.expression_selector.as_ref()
        else {
            return tracking;
        };
        tracking + evaluate_scalar_keyframes(keyframes, time, *value)
    })
}

fn resolve_text_paint(fill: [u8; 4], effects: &[EffectSpec]) -> TextPaintStyle {
    let extension = effects.iter().find_map(TextPaintSpec::from_effect);
    TextPaintStyle {
        fill: match extension.as_ref() {
            Some(paint) if !paint.fill_enabled => [0, 0, 0, 0],
            Some(paint) => paint.fill.unwrap_or(fill),
            None => fill,
        },
        stroke_color: extension.as_ref().and_then(|paint| paint.stroke_color),
        stroke_width: extension
            .as_ref()
            .map(|paint| paint.stroke_width.max(0.0))
            .unwrap_or(0.0),
        stroke_over_fill: extension
            .as_ref()
            .is_some_and(|paint| paint.stroke_over_fill),
    }
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
    // AE percentage range selectors address the leading boundary of each
    // text unit. With four words, starts 25/43.75/62.5 reveal 1/2/3 words.
    (index as f32 / total.max(1) as f32) * 100.0
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
        Some(TextExpressionSelector::TrackingAmount { .. }) => TextExpressionWeightDetail {
            weight: 1.0,
            raw_amount: None,
            clamped_amount: None,
            local_time_after_delay: None,
            text_index: None,
            text_total: total,
        },
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
        "matrix": transform.matrix.m,
        "final_matrix": transform.matrix.m,
        "final_opacity_alpha_scale": transform.alpha_scale,
        "blur_radius_px": transform.blur_radius
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
        .map(|glyph| rect_from_bbox(canvas, glyph.bbox))
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
        .map(|(_, bounds)| {
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

fn rect_from_bbox(canvas: &Canvas, bbox: [f32; 4]) -> UnitRect {
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
    }
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
    fn normal_blend_dispatch_preserves_existing_compositor_bytes() {
        let mut expected = Canvas::new(2, 1, [40, 100, 200, 128]);
        expected.set_pixel(1, 0, [5, 5, 6, 0]);
        let mut actual = expected.clone();
        let mut source = Canvas::new(2, 1, [220, 80, 20, 128]);
        source.set_pixel(1, 0, [17, 200, 90, 0]);

        composite_normal(&mut expected, &source, 37.5);
        composite_layer(&mut actual, &source, 37.5, BlendMode::Normal);

        assert_eq!(actual.data, expected.data);
    }

    #[test]
    fn add_and_difference_respect_source_alpha_and_layer_opacity() {
        let source = Canvas::new(1, 1, [220, 80, 20, 128]);

        let mut add = Canvas::new(1, 1, [40, 100, 200, 128]);
        composite_layer(&mut add, &source, 50.0, BlendMode::Add);
        assert_eq!(add.pixel(0, 0), [119, 112, 168, 160]);

        let mut difference = Canvas::new(1, 1, [40, 100, 200, 128]);
        composite_layer(&mut difference, &source, 50.0, BlendMode::Difference);
        assert_eq!(difference.pixel(0, 0), [104, 80, 160, 160]);

        let mut screen = Canvas::new(1, 1, [40, 100, 200, 128]);
        composite_layer(&mut screen, &source, 50.0, BlendMode::Screen);
        assert_eq!(screen.pixel(0, 0), [113, 106, 165, 160]);
    }

    #[test]
    fn blend_modes_preserve_straight_source_color_over_transparency() {
        let source = Canvas::new(1, 1, [200, 100, 50, 128]);
        for mode in [BlendMode::Add, BlendMode::Screen, BlendMode::Difference] {
            let mut destination = Canvas::new(1, 1, [5, 6, 7, 0]);
            composite_layer(&mut destination, &source, 50.0, mode);
            assert_eq!(destination.pixel(0, 0), [200, 100, 50, 64]);
        }
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
                blend_mode: BlendMode::Normal,
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
                blend_mode: BlendMode::Normal,
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
                    blend_mode: BlendMode::Normal,
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
                blend_mode: BlendMode::Normal,
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
    fn selector_glyph_passport_maps_word_units_to_glyph_runs() {
        let layout = text_engine::layout_text_stub(&TextLayoutRequest {
            text: "Hi all".to_string(),
            font_id: "missing".to_string(),
            font_size: 10.0,
            font_overrides: Vec::new(),
            font_size_overrides: Vec::new(),
            faux_italic_chars: Vec::new(),
            tracking: 0.0,
            leading: None,
            center_source_rect_y: false,
            justification: LayoutTextJustification::Center,
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
            font_overrides: Vec::new(),
            font_size_overrides: Vec::new(),
            faux_italic_chars: Vec::new(),
            tracking: 0.0,
            leading: None,
            center_source_rect_y: false,
            justification: LayoutTextJustification::Center,
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
    fn text_layout_trace_telemetry_reports_comp_coordinates() {
        let layout = text_engine::layout_text_stub(&TextLayoutRequest {
            text: "A".to_string(),
            font_id: "missing".to_string(),
            font_size: 10.0,
            font_overrides: Vec::new(),
            font_size_overrides: Vec::new(),
            faux_italic_chars: Vec::new(),
            tracking: 0.0,
            leading: None,
            center_source_rect_y: false,
            justification: LayoutTextJustification::Center,
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
    fn text_render_threads_tracking_and_leading_into_layout_trace() {
        let scene = Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "main".to_string(),
                width: 200,
                height: 100,
                fps: 1.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Text {
                id: "tracked".to_string(),
                start: 0.0,
                duration: 1.0,
                text: "AB".to_string(),
                font: "DejaVu Sans".to_string(),
                fontSize: 20.0,
                char_styles: Vec::new(),
                blend_mode: BlendMode::Normal,
                tracking: -50.0,
                leading: Some(20.0),
                center_source_rect_y: false,
                justification: render_ir::TextJustification::Center,
                fill: [255, 255, 255, 255],
                box_: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 200.0,
                    h: 100.0,
                }),
                transform: render_ir::Transform2D::default(),
                text_animators: Vec::new(),
                effects: Vec::new(),
            }],
        };
        let mut footage = CheckerboardFootageProvider;

        let (_, trace) = render_frame_with_footage_traced(&scene, 0, &mut footage).unwrap();
        let text = &trace.text_layouts[0];
        assert_eq!(text["request"]["tracking"], json!(-50.0));
        assert_eq!(text["request"]["leading"], json!(20.0));
        assert_eq!(text["request"]["justification"], json!("center"));
        assert_eq!(text["layout"]["tracking_px"], json!(-1.0));
        assert_eq!(text["layout"]["line_height"], json!(20.0));
    }

    #[test]
    fn tracking_amount_evaluates_before_layout_at_start_mid_and_end() {
        let ease = render_ir::KeyframeEase {
            x1: 0.333_333_33,
            y1: 0.0,
            x2: 0.666_666_7,
            y2: 1.0,
        };
        let animator = TextAnimatorSpec {
            name: "tracking".to_string(),
            opacity: 100.0,
            expression_selector: Some(TextExpressionSelector::TrackingAmount {
                value: 7.0,
                keyframes: vec![
                    ScalarKeyframe {
                        time: 1.0,
                        value: 7.0,
                        hold: false,
                        approximate: false,
                        ease: Some(ease),
                    },
                    ScalarKeyframe {
                        time: 3.0,
                        value: -1.0,
                        hold: false,
                        approximate: false,
                        ease: Some(ease),
                    },
                ],
            }),
            ..TextAnimatorSpec::default()
        };
        let values = [1.0, 2.0, 3.0]
            .map(|time| evaluate_text_tracking(-55.0, std::slice::from_ref(&animator), time));
        assert!((values[0] - -48.0).abs() < 0.0001);
        assert!((values[1] - -52.0).abs() < 0.0001);
        assert!((values[2] - -56.0).abs() < 0.0001);

        let layout_at = |tracking| {
            text_engine::layout_text_stub(&TextLayoutRequest {
                text: "AB".to_string(),
                font_id: "missing".to_string(),
                font_size: 100.0,
                font_overrides: Vec::new(),
                font_size_overrides: Vec::new(),
                faux_italic_chars: Vec::new(),
                tracking,
                leading: None,
                center_source_rect_y: false,
                justification: LayoutTextJustification::Left,
                box_rect: Some([0.0, 0.0, 300.0, 120.0]),
            })
        };
        let start_layout = layout_at(values[0]);
        let end_layout = layout_at(values[2]);
        let start_gap = start_layout.glyphs[1].x - start_layout.glyphs[0].x;
        let end_gap = end_layout.glyphs[1].x - end_layout.glyphs[0].x;
        assert!((start_gap - end_gap - 0.8).abs() < 0.0001);
    }

    #[test]
    fn text_paint_extension_overrides_stroke_without_breaking_legacy_fill() {
        let effects = vec![EffectSpec {
            match_name: TEXT_PAINT_MATCH_NAME.to_string(),
            params: json!({
                "stroke_color": [10, 20, 30, 255],
                "stroke_width": 5.0,
                "stroke_over_fill": false
            }),
        }];
        let paint = resolve_text_paint([240, 241, 242, 255], &effects);
        assert_eq!(paint.fill, [240, 241, 242, 255]);
        assert_eq!(paint.stroke_color, Some([10, 20, 30, 255]));
        assert_eq!(paint.stroke_width, 5.0);
        assert!(!paint.stroke_over_fill);
    }

    #[test]
    fn text_paint_can_explicitly_disable_fill_for_hollow_outline_layers() {
        let effects = vec![EffectSpec {
            match_name: TEXT_PAINT_MATCH_NAME.to_string(),
            params: json!({
                "fill_enabled": false,
                "stroke_color": [255, 255, 255, 255],
                "stroke_width": 5.0
            }),
        }];
        let paint = resolve_text_paint([240, 241, 242, 255], &effects);
        assert_eq!(paint.fill, [0, 0, 0, 0]);
        assert_eq!(paint.stroke_color, Some([255, 255, 255, 255]));
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
    }

    #[test]
    fn word_selector_uses_ae_left_boundaries_for_step_reveals() {
        let selector = TextRangeSelector::default();

        let initial: Vec<f32> = (0..4)
            .map(|index| selector_weight(index, 4, &selector, 25.0, 100.0, 0.0))
            .collect();
        let third_word_reveal: Vec<f32> = (0..4)
            .map(|index| selector_weight(index, 4, &selector, 62.5, 100.0, 0.0))
            .collect();

        // TYPE_1's animator has opacity 0: the selected suffix is hidden.
        // AE places four word units at 0, 25, 50 and 75 percent, so a start
        // of 25 keeps the first word visible and a start of 62.5 keeps the
        // first three visible.
        assert_eq!(initial, vec![0.0, 1.0, 1.0, 1.0]);
        assert_eq!(third_word_reveal, vec![0.0, 0.0, 0.0, 1.0]);
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
                blend_mode: BlendMode::Normal,
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
                blend_mode: BlendMode::Normal,
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
                    blend_mode: BlendMode::Normal,
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
                    blend_mode: BlendMode::Normal,
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
                    blend_mode: BlendMode::Normal,
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
                    blend_mode: BlendMode::Normal,
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
                blend_mode: BlendMode::Normal,
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
                blend_mode: BlendMode::Normal,
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
                blend_mode: BlendMode::Normal,
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
                    blend_mode: BlendMode::Normal,
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
                blend_mode: BlendMode::Normal,
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
    fn effect_runtime_lower_stack_cache_is_arc_backed_and_bounded() {
        let mut runtime = EffectRuntime::default();
        runtime.max_lower_stack_bytes = 64;
        let first_key = LowerStackCacheKey {
            composition: "root".to_string(),
            adjustment_layer: "posterize".to_string(),
            time_bits: 0.0_f64.to_bits(),
        };
        let second_key = LowerStackCacheKey {
            time_bits: 1.0_f64.to_bits(),
            ..first_key.clone()
        };
        let first = Arc::new(Canvas::transparent(4, 4));
        runtime.store_lower_stack(first_key.clone(), Arc::clone(&first));
        assert!(Arc::ptr_eq(
            &runtime.lower_stack(&first_key).unwrap(),
            &first
        ));

        let second = Arc::new(Canvas::transparent(4, 4));
        runtime.store_lower_stack(second_key.clone(), Arc::clone(&second));
        assert!(runtime.lower_stack(&first_key).is_none());
        assert!(Arc::ptr_eq(
            &runtime.lower_stack(&second_key).unwrap(),
            &second
        ));
        assert!(!runtime.worker_scratch.is_empty());
        assert!(runtime.worker_scratch.len() <= 8);
    }

    #[test]
    fn prefetched_footage_is_immutable_and_quantizes_equivalent_times() {
        let mut prefetched = PrefetchedFootageProvider::default();
        let mut source = Canvas::transparent(2, 1);
        source.set_pixel(1, 0, [11, 22, 33, 44]);
        prefetched.insert("clip", 1.25, source.clone());

        let mut worker_a = prefetched.clone();
        let mut worker_b = prefetched;
        let restored = worker_a.frame_at("clip", 1.2500001).unwrap().unwrap();
        assert_eq!(restored.data, source.data);
        assert!(worker_b.frame_at("missing", 1.25).unwrap().is_none());
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
