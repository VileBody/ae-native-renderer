use crate::{
    box_blur::{blur_canvas, blur_radius},
    param_bool, param_f32, param_rgba, Effect, EffectContext,
};
use raster_cpu::{composite_normal, Canvas};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct DropShadow;

impl Effect for DropShadow {
    fn match_name(&self) -> &'static str {
        "ADBE Drop Shadow"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let color = param_rgba(params, "0001", [0, 0, 0, 255]);
        let opacity = normalize_opacity(param_f32(params, "0002", 255.0));
        let direction = param_f32(params, "0003", 135.0).to_radians();
        let distance = param_f32(params, "0004", 5.0);
        let softness = param_f32(params, "0005", 0.0);
        let shadow_only = param_bool(params, "0006", false);
        let dx = (direction.cos() * distance).round() as i32;
        let dy = (direction.sin() * distance).round() as i32;

        let mut shadow = Canvas::transparent(input.width, input.height);
        for y in 0..input.height {
            for x in 0..input.width {
                let source = input.pixel(x, y);
                if source[3] == 0 {
                    continue;
                }
                let sx = x as i32 + dx;
                let sy = y as i32 + dy;
                if sx < 0 || sy < 0 || sx >= input.width as i32 || sy >= input.height as i32 {
                    continue;
                }
                let alpha = (source[3] as f32 * opacity * (color[3] as f32 / 255.0))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let existing = shadow.pixel(sx as u32, sy as u32);
                if alpha > existing[3] {
                    shadow.set_pixel(
                        sx as u32,
                        sy as u32,
                        [color[0], color[1], color[2], alpha],
                    );
                }
            }
        }

        let radius = blur_radius(softness / 2.0);
        if radius > 0 {
            shadow = blur_canvas(&shadow, radius);
        }

        if shadow_only {
            return Ok(shadow);
        }

        let mut output = shadow;
        composite_normal(&mut output, input, 100.0);
        Ok(output)
    }
}

fn normalize_opacity(value: f32) -> f32 {
    if value > 100.0 {
        (value / 255.0).clamp(0.0, 1.0)
    } else {
        (value / 100.0).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn drop_shadow_offsets_input_alpha() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(0, 0, [255, 255, 255, 255]);

        let output = DropShadow::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({
                    "0001": [0, 0, 0, 1],
                    "0002": 255,
                    "0003": 0,
                    "0004": 1,
                    "0005": 0,
                    "0006": true
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(1, 0)[3], 255);
    }
}
