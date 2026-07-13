use crate::{param_f32_at_any, param_rgba_any, param_value, Effect, EffectContext};
use raster_cpu::{composite_normal_pixel, Canvas};
use rayon::prelude::*;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct ShapeOverlay;

impl Effect for ShapeOverlay {
    fn match_name(&self) -> &'static str {
        "ANR Shape Overlay"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let shape = shape_name(params);
        let center_x = param_f32_at_any(
            params,
            &["center_x", "centerX", "x"],
            ctx.time,
            input.width as f32 * 0.5,
        );
        let center_y = param_f32_at_any(
            params,
            &["center_y", "centerY", "y"],
            ctx.time,
            input.height as f32 * 0.5,
        );
        let size = param_f32_at_any(params, &["size", "radius"], ctx.time, 180.0).max(1.0);
        let thickness =
            param_f32_at_any(params, &["thickness", "stroke_width"], ctx.time, 12.0).max(0.5);
        let opacity = param_f32_at_any(params, &["opacity"], ctx.time, 100.0).clamp(0.0, 100.0);
        let color = param_rgba_any(params, &["color", "fill"], [255, 255, 255, 255]);
        let mut output = input.clone();
        let width = input.width as usize;
        output
            .data
            .par_chunks_exact_mut(4)
            .enumerate()
            .for_each(|(index, destination)| {
                let x = index % width.max(1);
                let y = index / width.max(1);
                let p = [x as f32 + 0.5 - center_x, y as f32 + 0.5 - center_y];
                let distance = overlay_distance(shape, p, size, thickness);
                let coverage = (0.5 - distance).clamp(0.0, 1.0);
                if coverage <= 0.0 {
                    return;
                }
                let mut source = color;
                source[3] = (source[3] as f32 * coverage).round().clamp(0.0, 255.0) as u8;
                let base = [
                    destination[0],
                    destination[1],
                    destination[2],
                    destination[3],
                ];
                destination.copy_from_slice(&composite_normal_pixel(base, source, opacity));
            });
        Ok(output)
    }
}

fn shape_name(params: &Value) -> &str {
    ["shape", "device", "mode"]
        .into_iter()
        .find_map(|name| param_value(params, name))
        .and_then(Value::as_str)
        .unwrap_or("ellipse")
}

fn overlay_distance(shape: &str, p: [f32; 2], size: f32, thickness: f32) -> f32 {
    let signed = match shape.to_ascii_lowercase().as_str() {
        "square" => sd_box(p, [size, size]),
        "rhomb" | "rhombus" => (p[0].abs() + p[1].abs() - size) * 0.707_106_77,
        "star1" => sd_star(p, size, 5, 0.45),
        "star2" => sd_star(p, size, 8, 0.55),
        "head" => sd_circle(p, size * 0.62),
        "tap" => sd_ring(p, size * 0.55, thickness),
        "swipe" => sd_swipe(p, size, thickness),
        "pinch" => sd_pinch(p, size, thickness),
        "holdfinger" => sd_hold_finger(p, size, thickness),
        _ => sd_ellipse(p, [size, size * 0.72]),
    };
    if matches!(
        shape.to_ascii_lowercase().as_str(),
        "head" | "tap" | "swipe" | "pinch" | "holdfinger"
    ) {
        signed
    } else {
        signed.abs() - thickness * 0.5
    }
}

fn sd_circle(p: [f32; 2], radius: f32) -> f32 {
    p[0].hypot(p[1]) - radius
}

fn sd_ring(p: [f32; 2], radius: f32, thickness: f32) -> f32 {
    sd_circle(p, radius).abs() - thickness * 0.5
}

fn sd_box(p: [f32; 2], half: [f32; 2]) -> f32 {
    let q = [p[0].abs() - half[0], p[1].abs() - half[1]];
    q[0].max(0.0).hypot(q[1].max(0.0)) + q[0].max(q[1]).min(0.0)
}

fn sd_ellipse(p: [f32; 2], radius: [f32; 2]) -> f32 {
    let normalized = (p[0] / radius[0]).hypot(p[1] / radius[1]);
    (normalized - 1.0) * radius[0].min(radius[1])
}

fn sd_star(p: [f32; 2], radius: f32, points: u32, inner_ratio: f32) -> f32 {
    let angle = p[1].atan2(p[0]);
    let sector = std::f32::consts::PI / points as f32;
    let wave = ((angle / sector).rem_euclid(2.0) - 1.0).abs();
    let boundary = radius * (inner_ratio + (1.0 - inner_ratio) * (1.0 - wave));
    p[0].hypot(p[1]) - boundary
}

fn sd_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let pa = [p[0] - a[0], p[1] - a[1]];
    let ba = [b[0] - a[0], b[1] - a[1]];
    let denominator = ba[0] * ba[0] + ba[1] * ba[1];
    let h = if denominator > f32::EPSILON {
        ((pa[0] * ba[0] + pa[1] * ba[1]) / denominator).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (pa[0] - ba[0] * h).hypot(pa[1] - ba[1] * h)
}

fn sd_swipe(p: [f32; 2], size: f32, thickness: f32) -> f32 {
    let shaft = sd_segment(p, [-size * 0.75, 0.0], [size * 0.65, 0.0]);
    let top = sd_segment(p, [size * 0.25, -size * 0.35], [size * 0.65, 0.0]);
    let bottom = sd_segment(p, [size * 0.25, size * 0.35], [size * 0.65, 0.0]);
    shaft.min(top).min(bottom) - thickness * 0.5
}

fn sd_pinch(p: [f32; 2], size: f32, thickness: f32) -> f32 {
    let left = sd_segment(p, [-size * 0.7, size * 0.55], [-size * 0.12, -size * 0.1]);
    let right = sd_segment(p, [size * 0.7, size * 0.55], [size * 0.12, -size * 0.1]);
    let touch = sd_circle(p, size * 0.12).abs();
    left.min(right).min(touch) - thickness * 0.5
}

fn sd_hold_finger(p: [f32; 2], size: f32, thickness: f32) -> f32 {
    let finger = sd_segment(p, [0.0, size * 0.75], [0.0, -size * 0.25]);
    let hold = sd_ring([p[0], p[1] + size * 0.35], size * 0.36, thickness);
    finger.min(hold + thickness * 0.5) - thickness * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_f2_shapes_draw_visible_bounded_pixels() {
        for shape in ["square", "ellipse", "rhomb", "star1", "star2"] {
            let input = Canvas::transparent(96, 96);
            let output = ShapeOverlay
                .render(
                    &input,
                    &EffectContext {
                        time: 0.0,
                        fps: 24.0,
                    },
                    &json!({"shape":shape, "size":24, "thickness":4}),
                )
                .unwrap();
            let visible = output
                .data
                .chunks_exact(4)
                .filter(|pixel| pixel[3] > 0)
                .count();
            assert!(visible > 16, "{shape} did not draw enough pixels");
            assert!(visible < 96 * 96 / 2, "{shape} escaped its bounds");
        }
    }

    #[test]
    fn gesture_devices_draw_without_external_assets() {
        for device in ["head", "pinch", "holdfinger", "tap", "swipe"] {
            let input = Canvas::transparent(96, 96);
            let output = ShapeOverlay
                .render(
                    &input,
                    &EffectContext {
                        time: 0.0,
                        fps: 24.0,
                    },
                    &json!({"device":device, "size":28, "thickness":5}),
                )
                .unwrap();
            assert!(output.data.chunks_exact(4).any(|pixel| pixel[3] > 0));
        }
    }
}
