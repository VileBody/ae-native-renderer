use effects::{EffectContext, EffectRegistry};
use raster_cpu::{composite_normal, Canvas};
use render_ir::{EffectSpec, Layer, Rect, ScalarKeyframe, Scene, Vec2Keyframe};
use text_engine::{rasterize_text, TextLayoutRequest};
use transform_math::{Transform2D, Vec2};

pub trait FootageProvider {
    fn frame_at(&mut self, source: &str, time: f64) -> anyhow::Result<Option<Canvas>>;
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
    let mut canvas = Canvas::new(comp.width, comp.height, comp.background);

    // IR order is top-to-bottom like AE. Render bottom-to-top.
    for layer in scene.layers.iter().rev() {
        if !layer.is_active(time) {
            continue;
        }
        let layer_canvas = render_layer_stub(scene, layer, time, footage)?;
        composite_normal(&mut canvas, &layer_canvas, opacity_of(layer, time));
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
    layer: &Layer,
    time: f64,
    footage: &mut dyn FootageProvider,
) -> anyhow::Result<Canvas> {
    let comp = &scene.composition;
    let mut canvas = match layer {
        Layer::Solid {
            color,
            rect,
            transform,
            ..
        } => {
            let evaluated = evaluate_transform(transform, time);
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
            ..
        } => {
            let evaluated = evaluate_transform(transform, time);
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
            apply_horizontal_reveal(&mut text_canvas, reveal);
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
            ..
        } => {
            let evaluated = evaluate_transform(transform, time);
            let source_time = (*source_start + (time - *start)).max(0.0);
            match footage.frame_at(source, source_time)? {
                Some(frame) => {
                    transform_canvas(&frame, comp.width, comp.height, &evaluated, [0.0, 0.0])
                }
                None => checkerboard_canvas(comp.width, comp.height),
            }
        }
        Layer::Precomp { .. } => Canvas::transparent(comp.width, comp.height),
        Layer::Adjustment { .. } => Canvas::transparent(comp.width, comp.height),
    };

    for spec in effects_of(layer) {
        if let Some(effect) = EffectRegistry::create(&spec.match_name) {
            canvas = effect.render(
                &canvas,
                &EffectContext {
                    time,
                    fps: scene.composition.fps,
                },
                &spec.params,
            )?;
        } else {
            anyhow::bail!("unknown effect matchName: {}", spec.match_name);
        }
    }

    Ok(canvas)
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
    let Some(inverse) = transform_to_matrix(transform).matrix().inverse() else {
        return Canvas::transparent(width, height);
    };

    let mut dst = Canvas::transparent(width, height);
    for y in 0..height {
        for x in 0..width {
            let local = inverse.transform_point(Vec2::new(x as f32, y as f32));
            let sx = (local.x - local_origin[0]).round() as i32;
            let sy = (local.y - local_origin[1]).round() as i32;
            if sx < 0 || sy < 0 || sx >= src.width as i32 || sy >= src.height as i32 {
                continue;
            }
            let pixel = src.pixel(sx as u32, sy as u32);
            if pixel[3] > 0 {
                dst.set_pixel(x, y, pixel);
            }
        }
    }
    dst
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

fn evaluate_transform(transform: &render_ir::Transform2D, time: f64) -> render_ir::Transform2D {
    let mut evaluated = transform.clone();
    evaluated.position =
        evaluate_vec2_keyframes(&transform.animation.position, time, transform.position);
    evaluated.scale = evaluate_vec2_keyframes(&transform.animation.scale, time, transform.scale);
    evaluated.opacity =
        evaluate_scalar_keyframes(&transform.animation.opacity, time, transform.opacity);
    evaluated
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
            let t = ((time - a.time) / (b.time - a.time)).clamp(0.0, 1.0) as f32;
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
            let t = ((time - a.time) / (b.time - a.time)).clamp(0.0, 1.0) as f32;
            return a.value + (b.value - a.value) * t;
        }
    }
    keyframes.last().map(|key| key.value).unwrap_or(fallback)
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

fn canvas_dim(value: f32) -> u32 {
    value.ceil().max(1.0).min(u32::MAX as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_keyframes_interpolate_linearly() {
        let keyframes = vec![
            ScalarKeyframe {
                time: 1.0,
                value: 20.0,
                hold: false,
                approximate: false,
            },
            ScalarKeyframe {
                time: 3.0,
                value: 60.0,
                hold: false,
                approximate: false,
            },
        ];

        assert_eq!(evaluate_scalar_keyframes(&keyframes, 2.0, 100.0), 40.0);
    }

    #[test]
    fn vec2_keyframes_respect_hold() {
        let keyframes = vec![
            Vec2Keyframe {
                time: 1.0,
                value: [10.0, 20.0],
                hold: true,
                approximate: false,
            },
            Vec2Keyframe {
                time: 3.0,
                value: [50.0, 80.0],
                hold: false,
                approximate: false,
            },
        ];

        assert_eq!(
            evaluate_vec2_keyframes(&keyframes, 2.0, [0.0, 0.0]),
            [10.0, 20.0]
        );
    }
}
