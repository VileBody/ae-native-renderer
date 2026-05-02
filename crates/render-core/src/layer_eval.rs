use effects::{EffectContext, EffectRegistry};
use raster_cpu::{composite_normal, BilinearSampler, Canvas, Sampler};
use render_ir::{
    Composition, EffectSpec, Layer, PositionExpression, Rect, ScalarKeyframe, Scene,
    TextAnimatorSpec, TextExpressionSelector, TextSelectorBasedOn, TextSelectorShape, Vec2Keyframe,
};
use text_engine::{rasterize_text, TextLayoutRequest};
use std::time::Instant;
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
    for layer in layers.iter().rev() {
        if !layer.is_active(time) {
            continue;
        }
        if matches!(layer, Layer::Adjustment { .. }) {
            let layer_started = Instant::now();
            let effects_started = Instant::now();
            canvas = apply_effects_to_canvas(
                effects_of(layer),
                &canvas,
                time,
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
        let layer_opacity = opacity_of(layer, time);
        if layer_opacity <= 0.0 {
            continue;
        }
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

    Ok(canvas)
}

fn opacity_of(layer: &Layer, time: f64) -> f32 {
    match layer {
        Layer::Solid { transform, .. }
        | Layer::Footage { transform, .. }
        | Layer::Text { transform, .. }
        | Layer::Precomp { transform, .. } => evaluate_scalar_keyframes(
            &transform.animation.opacity,
            time,
            transform.opacity,
        ),
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
    let mut canvas = match layer {
        Layer::Solid {
            color,
            rect,
            transform,
            start,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, time, *start, *duration, comp.fps);
            let mut c = Canvas::transparent(canvas_dim(rect.w), canvas_dim(rect.h));
            for y in 0..c.height {
                for x in 0..c.width {
                    c.set_pixel(x, y, *color);
                }
            }
            transform_canvas(&c, comp.width, comp.height, &evaluated, [rect.x, rect.y])
        }
        Layer::Text {
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
            let evaluated = evaluate_transform(transform, time, *start, *duration, comp.fps);
            let rect = box_.clone().unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                w: comp.width as f32,
                h: comp.height as f32,
            });
            let local_width = canvas_dim(rect.w);
            let local_height = canvas_dim(rect.h);
            let mut text_canvas = rasterize_text(
                &TextLayoutRequest {
                    text: text.clone(),
                    font_id: font.clone(),
                    font_size: *fontSize,
                    box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
                },
                local_width,
                local_height,
                *fill,
            )?;
            let reveal = evaluate_scalar_keyframes(&transform.animation.reveal, time, 100.0);
            if text_animators.is_empty() {
                apply_horizontal_reveal(&mut text_canvas, reveal);
            } else {
                text_canvas = apply_text_animators(text_canvas, text, text_animators, time, *start);
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
            start,
            source,
            source_start,
            transform,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, time, *start, *duration, comp.fps);
            let source_time = (*source_start + (time - *start)).max(0.0);
            match footage.frame_at(source, source_time)? {
                Some(frame) => {
                    transform_canvas(&frame, comp.width, comp.height, &evaluated, [0.0, 0.0])
                }
                None => checkerboard_canvas(comp.width, comp.height),
            }
        }
        Layer::Precomp {
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
                    time,
                    footage,
                    stack,
                    trace.as_deref_mut(),
                )?
            } else {
                let node = scene
                    .compositions
                    .iter()
                    .find(|node| node.composition.id == *composition)
                    .ok_or_else(|| anyhow::anyhow!("precomp composition '{composition}' was not found"))?;
                if stack.iter().any(|id| id == composition) {
                    anyhow::bail!("precomp cycle detected: {} -> {}", stack.join(" -> "), composition);
                }
                stack.push(composition.clone());
                let source_time = (time - *start).max(0.0);
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
                let evaluated = evaluate_transform(transform, time, *start, *duration, comp.fps);
                transform_canvas(&precomp_canvas, comp.width, comp.height, &evaluated, [0.0, 0.0])
            }
        }
        Layer::Adjustment { .. } => Canvas::transparent(comp.width, comp.height),
    };
    let content_ms = elapsed_ms(content_started);

    let effects_started = Instant::now();
    canvas = apply_effects_to_canvas(
        effects_of(layer),
        &canvas,
        time,
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
        anyhow::bail!("precomp cycle detected: {} -> {}", stack.join(" -> "), composition);
    }
    let node = composition_node(scene, composition)
        .ok_or_else(|| anyhow::anyhow!("precomp composition '{composition}' was not found"))?;
    let source_time = (time - *start).max(0.0);
    let evaluated = evaluate_transform(transform, time, *start, *duration, parent_comp.fps);
    let parent_matrix = transform_to_matrix(&evaluated).matrix();
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
            color,
            rect,
            transform,
            start,
            duration,
            ..
        } => {
            let evaluated = evaluate_transform(transform, time, *start, *duration, parent_comp.fps);
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
            let rect = box_.clone().unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                w: parent_comp.width as f32,
                h: parent_comp.height as f32,
            });
            let local_width = canvas_dim(rect.w);
            let local_height = canvas_dim(rect.h);
            let mut text_canvas = rasterize_text(
                &TextLayoutRequest {
                    text: text.clone(),
                    font_id: font.clone(),
                    font_size: *fontSize,
                    box_rect: Some([0.0, 0.0, local_width as f32, local_height as f32]),
                },
                local_width,
                local_height,
                *fill,
            )?;
            let reveal = evaluate_scalar_keyframes(&transform.animation.reveal, time, 100.0);
            if text_animators.is_empty() {
                apply_horizontal_reveal(&mut text_canvas, reveal);
            } else {
                text_canvas = apply_text_animators(text_canvas, text, text_animators, time, *start);
            }
            let matrix = parent_matrix.mul(transform_to_matrix(&evaluated).matrix());
            Ok(transform_canvas_with_matrix(
                &text_canvas,
                parent_comp.width,
                parent_comp.height,
                matrix,
                [rect.x, rect.y],
            ))
        }
        Layer::Precomp {
            start,
            duration,
            composition,
            collapse_transformations,
            transform,
            ..
        } if *collapse_transformations && can_collapse_composition(scene, composition, stack) => {
            if stack.iter().any(|value| value == composition) {
                anyhow::bail!("precomp cycle detected: {} -> {}", stack.join(" -> "), composition);
            }
            let node = composition_node(scene, composition)
                .ok_or_else(|| anyhow::anyhow!("precomp composition '{composition}' was not found"))?;
            let source_time = (time - *start).max(0.0);
            let evaluated = evaluate_transform(transform, time, *start, *duration, parent_comp.fps);
            let matrix = parent_matrix.mul(transform_to_matrix(&evaluated).matrix());
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
    for spec in effects {
        if let Some(effect) = EffectRegistry::create(&spec.match_name) {
            let started = Instant::now();
            canvas = effect.render(
                &canvas,
                &EffectContext {
                    time,
                    fps,
                },
                &spec.params,
            )?;
            if let Some(trace) = trace.as_deref_mut() {
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
    let Some((x0, y0, x1, y1)) = transformed_bounds(src, width, height, matrix, local_origin) else {
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
    match expression {
        PositionExpression::EdgeWobble {
            intro,
            outro,
            amp,
            freq,
            ..
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
            let y = *amp
                * 0.68
                * k
                * (2.0 * std::f32::consts::PI * (*freq * 0.82) * t).cos();
            [base[0] + x, base[1] + y]
        }
    }
}

fn evaluate_vec2_keyframes(
    keyframes: &[Vec2Keyframe],
    time: f64,
    fallback: [f32; 2],
) -> [f32; 2] {
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

fn apply_text_animators(
    mut canvas: Canvas,
    text: &str,
    animators: &[TextAnimatorSpec],
    time: f64,
    layer_start: f64,
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
        if animator.position.is_some()
            || animator.scale.is_some()
            || animator.rotation.is_some()
            || animator.blur.is_some()
            || animator.expression_selector.is_some()
        {
            canvas = apply_unit_animator_transform(
                &canvas,
                text,
                animator,
                start.min(end),
                start.max(end),
                time,
                layer_start,
            );
        } else {
            apply_range_opacity(&mut canvas, text, animator, start.min(end), start.max(end), time);
        }
    }
    canvas
}

fn apply_range_opacity(
    canvas: &mut Canvas,
    text: &str,
    animator: &TextAnimatorSpec,
    start_percent: f32,
    end_percent: f32,
    time: f64,
) {
    if animator.opacity >= 99.999 && animator.selector.wiggly.is_none() {
        return;
    }

    match animator.selector.based_on {
        TextSelectorBasedOn::Lines => apply_line_range_opacity(
            canvas,
            text.lines().count().max(1),
            animator,
            start_percent,
            end_percent,
            time,
        ),
        TextSelectorBasedOn::Words => apply_inline_unit_range_opacity(
            canvas,
            text,
            UnitMode::Words,
            animator,
            start_percent,
            end_percent,
            time,
        ),
        TextSelectorBasedOn::Characters => apply_inline_unit_range_opacity(
            canvas,
            text,
            UnitMode::Characters,
            animator,
            start_percent,
            end_percent,
            time,
        ),
    }
}

#[derive(Debug, Clone, Copy)]
enum UnitMode {
    Characters,
    Words,
}

fn apply_line_range_opacity(
    canvas: &mut Canvas,
    line_count: usize,
    animator: &TextAnimatorSpec,
    start_percent: f32,
    end_percent: f32,
    time: f64,
) {
    for line in 0..line_count {
        let weight = selector_weight(
            line,
            line_count,
            &animator.selector,
            start_percent,
            end_percent,
            time,
        );
        if weight <= 0.0 {
            continue;
        }
        let alpha_scale = animator_alpha_scale(animator.opacity, weight);
        let y0 = ((line as f32 / line_count as f32) * canvas.height as f32).floor() as u32;
        let y1 = (((line + 1) as f32 / line_count as f32) * canvas.height as f32).ceil() as u32;
        scale_alpha_rect(canvas, 0, y0, canvas.width, y1.min(canvas.height), alpha_scale);
    }
}

fn apply_inline_unit_range_opacity(
    canvas: &mut Canvas,
    text: &str,
    mode: UnitMode,
    animator: &TextAnimatorSpec,
    start_percent: f32,
    end_percent: f32,
    time: f64,
) {
    let lines: Vec<&str> = text.lines().collect();
    let lines = if lines.is_empty() { vec![text] } else { lines };
    let total_units: usize = lines
        .iter()
        .map(|line| unit_count(line, mode))
        .sum::<usize>()
        .max(1);
    let mut global_unit = 0_usize;

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

        for local_unit in 0..line_units {
            let weight = selector_weight(
                global_unit,
                total_units,
                &animator.selector,
                start_percent,
                end_percent,
                time,
            );
            if weight > 0.0 {
                let ux0 =
                    x0 + ((local_unit as f32 / line_units as f32) * span as f32).floor() as u32;
                let ux1 = x0
                    + (((local_unit + 1) as f32 / line_units as f32) * span as f32).ceil() as u32;
                let alpha_scale = animator_alpha_scale(animator.opacity, weight);
                scale_alpha_rect(
                    canvas,
                    ux0,
                    y0,
                    ux1.min(canvas.width),
                    y1.min(canvas.height),
                    alpha_scale,
                );
            }
            global_unit += 1;
        }
    }
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
            pixel[3] = (pixel[3] as f32 * alpha_scale)
                .round()
                .clamp(0.0, 255.0) as u8;
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
    let ordered_index = if selector.randomize_order {
        deterministic_order_index(index, total, selector.wiggly.map(|wiggly| wiggly.seed).unwrap_or(0))
    } else {
        index
    };
    let pos = ((ordered_index as f32 + 0.5) / total as f32) * 100.0;
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
    animator: &TextAnimatorSpec,
    start_percent: f32,
    end_percent: f32,
    time: f64,
    layer_start: f64,
) -> Canvas {
    let units = unit_rects(canvas, text, animator.selector.based_on);
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
        let expression_weight =
            text_expression_weight(animator.expression_selector.as_ref(), unit.index, unit.total, time, layer_start);
        let weight = range_weight * expression_weight;
        draw_transformed_unit(&mut output, canvas, unit, animator, weight);
    }
    output
}

fn text_expression_weight(
    selector: Option<&TextExpressionSelector>,
    index: usize,
    _total: usize,
    time: f64,
    layer_start: f64,
) -> f32 {
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
                return 0.0;
            }
            let amount = *amplitude
                * (freq * t * 2.0 * std::f32::consts::PI).cos()
                / (*decay * t).exp();
            (amount / 100.0).clamp(-2.0, 2.0)
        }
        None => 1.0,
    }
}

fn draw_transformed_unit(
    output: &mut Canvas,
    input: &Canvas,
    unit: UnitRect,
    animator: &TextAnimatorSpec,
    weight: f32,
) {
    let cx = (unit.x0 + unit.x1) as f32 * 0.5;
    let cy = (unit.y0 + unit.y1) as f32 * 0.5;
    let position = animator.position.unwrap_or([0.0, 0.0]);
    let scale = animator.scale.unwrap_or([100.0, 100.0]);
    let sx = (100.0 + (scale[0] - 100.0) * weight) / 100.0;
    let sy = (100.0 + (scale[1] - 100.0) * weight) / 100.0;
    let rotation = animator.rotation.unwrap_or(0.0) * weight;
    let tx = position[0] * weight;
    let ty = position[1] * weight;
    let alpha_scale = animator_alpha_scale(animator.opacity, weight);
    let blur_radius = animator
        .blur
        .map(|blur| ((blur[0].abs().max(blur[1].abs()) * weight) / 8.0).round() as i32)
        .unwrap_or(0)
        .clamp(0, 4);
    let matrix = Mat3::translate(Vec2::new(cx + tx, cy + ty))
        .mul(Mat3::rotate_degrees(rotation))
        .mul(Mat3::scale(Vec2::new(sx, sy)))
        .mul(Mat3::translate(Vec2::new(-cx, -cy)));

    for y in unit.y0..unit.y1 {
        for x in unit.x0..unit.x1 {
            let mut pixel = input.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            pixel[3] = (pixel[3] as f32 * alpha_scale)
                .round()
                .clamp(0.0, 255.0) as u8;
            let p = matrix.transform_point(Vec2::new(x as f32, y as f32));
            let dx = p.x.round() as i32;
            let dy = p.y.round() as i32;
            if dx < 0 || dy < 0 || dx >= output.width as i32 || dy >= output.height as i32 {
                continue;
            }
            splat_blurred_pixel(output, dx, dy, pixel, blur_radius);
        }
    }
}

fn splat_blurred_pixel(output: &mut Canvas, x: i32, y: i32, pixel: [u8; 4], radius: i32) {
    if radius <= 0 {
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

fn unit_rects(canvas: &Canvas, text: &str, based_on: TextSelectorBasedOn) -> Vec<UnitRect> {
    match based_on {
        TextSelectorBasedOn::Lines => line_unit_rects(canvas, text),
        TextSelectorBasedOn::Words => inline_unit_rects(canvas, text, UnitMode::Words),
        TextSelectorBasedOn::Characters => inline_unit_rects(canvas, text, UnitMode::Characters),
    }
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

        assert!(eased < 50.0, "expected ease-in midpoint below linear, got {eased}");
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
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "child".to_string(),
                    width: 4,
                    height: 4,
                    fps: 1.0,
                    duration: 1.0,
                    background: [0, 0, 0, 0],
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

        let animated = apply_unit_animator_transform(&canvas, "A", &animator, 0.0, 100.0, 0.0, 0.0);

        assert_eq!(animated.pixel(1, 0), [255, 255, 255, 255]);
        assert_eq!(animated.pixel(0, 0), [0, 0, 0, 0]);
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
            },
            compositions: vec![CompositionNode {
                composition: Composition {
                    id: "child".to_string(),
                    width: 4,
                    height: 4,
                    fps: 1.0,
                    duration: 1.0,
                    background: [0, 0, 0, 0],
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
}
