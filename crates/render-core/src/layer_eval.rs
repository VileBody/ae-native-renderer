use effects::{EffectContext, EffectRegistry};
use raster_cpu::{composite_normal, Canvas};
use render_ir::{EffectSpec, Layer, Scene};
use text_engine::{layout_text_stub, rasterize_text_debug, TextLayoutRequest};

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
        composite_normal(&mut canvas, &layer_canvas, opacity_of(layer));
    }

    Ok(canvas)
}

fn opacity_of(layer: &Layer) -> f32 {
    match layer {
        Layer::Solid { transform, .. }
        | Layer::Footage { transform, .. }
        | Layer::Text { transform, .. }
        | Layer::Precomp { transform, .. } => transform.opacity,
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
        Layer::Solid { color, rect, .. } => {
            let mut c = Canvas::transparent(comp.width, comp.height);
            let x0 = rect.x.max(0.0) as u32;
            let y0 = rect.y.max(0.0) as u32;
            let x1 = (rect.x + rect.w).max(0.0) as u32;
            let y1 = (rect.y + rect.h).max(0.0) as u32;
            for y in y0..y1.min(comp.height) {
                for x in x0..x1.min(comp.width) {
                    c.set_pixel(x, y, *color);
                }
            }
            c
        }
        Layer::Text {
            text,
            font,
            fontSize,
            fill,
            box_,
            ..
        } => {
            let box_rect = box_.as_ref().map(|r| [r.x, r.y, r.w, r.h]);
            let layout = layout_text_stub(&TextLayoutRequest {
                text: text.clone(),
                font_id: font.clone(),
                font_size: *fontSize,
                box_rect,
            });
            rasterize_text_debug(&layout, comp.width, comp.height, *fill)
        }
        Layer::Footage {
            start,
            source,
            source_start,
            ..
        } => {
            let source_time = (*source_start + (time - *start)).max(0.0);
            match footage.frame_at(source, source_time)? {
                Some(frame) => cover_canvas(&frame, comp.width, comp.height),
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

fn cover_canvas(src: &Canvas, width: u32, height: u32) -> Canvas {
    if src.width == width && src.height == height {
        return src.clone();
    }

    let scale_x = width as f32 / src.width.max(1) as f32;
    let scale_y = height as f32 / src.height.max(1) as f32;
    let scale = scale_x.max(scale_y);
    let scaled_w = src.width as f32 * scale;
    let scaled_h = src.height as f32 * scale;
    let offset_x = (width as f32 - scaled_w) * 0.5;
    let offset_y = (height as f32 - scaled_h) * 0.5;

    let mut dst = Canvas::transparent(width, height);
    for y in 0..height {
        for x in 0..width {
            let sx = ((x as f32 - offset_x) / scale)
                .round()
                .clamp(0.0, src.width.saturating_sub(1) as f32) as u32;
            let sy = ((y as f32 - offset_y) / scale)
                .round()
                .clamp(0.0, src.height.saturating_sub(1) as f32) as u32;
            dst.set_pixel(x, y, src.pixel(sx, sy));
        }
    }
    dst
}
