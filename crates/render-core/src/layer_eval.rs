use effects::{EffectContext, EffectRegistry};
use raster_cpu::{composite_normal, Canvas};
use render_ir::{EffectSpec, Layer, Scene};
use text_engine::{layout_text_stub, rasterize_text_debug, TextLayoutRequest};

pub fn render_frame(scene: &Scene, frame_index: u32) -> anyhow::Result<Canvas> {
    let comp = &scene.composition;
    let time = frame_index as f64 / comp.fps;
    let mut canvas = Canvas::new(comp.width, comp.height, comp.background);

    // IR order is top-to-bottom like AE. Render bottom-to-top.
    for layer in scene.layers.iter().rev() {
        if !layer.is_active(time) {
            continue;
        }
        let layer_canvas = render_layer_stub(scene, layer, time)?;
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

fn render_layer_stub(scene: &Scene, layer: &Layer, time: f64) -> anyhow::Result<Canvas> {
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
        Layer::Text { text, font, fontSize, fill, box_, .. } => {
            let box_rect = box_.as_ref().map(|r| [r.x, r.y, r.w, r.h]);
            let layout = layout_text_stub(&TextLayoutRequest {
                text: text.clone(),
                font_id: font.clone(),
                font_size: *fontSize,
                box_rect,
            });
            rasterize_text_debug(&layout, comp.width, comp.height, *fill)
        }
        Layer::Footage { .. } => {
            // TODO: call media backend through a trait.
            let mut c = Canvas::transparent(comp.width, comp.height);
            for y in 0..comp.height {
                for x in 0..comp.width {
                    if (x / 32 + y / 32) % 2 == 0 {
                        c.set_pixel(x, y, [30, 30, 36, 255]);
                    }
                }
            }
            c
        }
        Layer::Precomp { .. } => Canvas::transparent(comp.width, comp.height),
        Layer::Adjustment { .. } => Canvas::transparent(comp.width, comp.height),
    };

    for spec in effects_of(layer) {
        if let Some(effect) = EffectRegistry::create(&spec.match_name) {
            canvas = effect.render(&canvas, &EffectContext { time, fps: scene.composition.fps })?;
        } else {
            anyhow::bail!("unknown effect matchName: {}", spec.match_name);
        }
    }

    Ok(canvas)
}
