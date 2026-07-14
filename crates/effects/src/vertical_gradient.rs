use crate::{param_f32, param_rgba, param_value, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct VerticalGradient;

impl Effect for VerticalGradient {
    fn match_name(&self) -> &'static str {
        "ANR Vertical Gradient"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let top = param_rgba(params, "top", [255, 255, 255, 255]);
        let bottom = param_rgba(params, "bottom", [170, 170, 170, 255]);
        let brightness = param_f32(params, "brightness", 1.0).max(0.0);
        let composition_gradient = gradient_endpoints(params);
        let mut min_y = input.height;
        let mut max_y = 0u32;
        let mut found = false;
        for y in 0..input.height {
            for x in 0..input.width {
                if input.pixel(x, y)[3] > 0 {
                    found = true;
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }
        if !found {
            return Ok(input.clone());
        }

        let mut output = input.clone();
        for y in min_y..=max_y {
            for x in 0..input.width {
                let source = input.pixel(x, y);
                if source[3] == 0 {
                    continue;
                }
                let t = composition_gradient.map_or_else(
                    || (y - min_y) as f32 / max_y.saturating_sub(min_y).max(1) as f32,
                    |(start, end)| project_point_onto_gradient(x as f32, y as f32, start, end),
                );
                let gradient = [0, 1, 2].map(|channel| {
                    ((top[channel] as f32 + (bottom[channel] as f32 - top[channel] as f32) * t)
                        * brightness)
                        .round()
                        .clamp(0.0, 255.0) as u8
                });
                output.set_pixel(
                    x,
                    y,
                    [
                        ((source[0] as u16 * gradient[0] as u16) / 255) as u8,
                        ((source[1] as u16 * gradient[1] as u16) / 255) as u8,
                        ((source[2] as u16 * gradient[2] as u16) / 255) as u8,
                        source[3],
                    ],
                );
            }
        }
        Ok(output)
    }
}

fn gradient_endpoints(params: &Value) -> Option<([f32; 2], [f32; 2])> {
    Some((
        point_param(params, "start_xy")?,
        point_param(params, "end_xy")?,
    ))
}

fn point_param(params: &Value, name: &str) -> Option<[f32; 2]> {
    let values = param_value(params, name)?.as_array()?;
    Some([
        values.first()?.as_f64()? as f32,
        values.get(1)?.as_f64()? as f32,
    ])
}

fn project_point_onto_gradient(x: f32, y: f32, start: [f32; 2], end: [f32; 2]) -> f32 {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let length_sq = direction[0] * direction[0] + direction[1] * direction[1];
    if length_sq <= f32::EPSILON {
        return 0.0;
    }
    (((x - start[0]) * direction[0] + (y - start[1]) * direction[1]) / length_sq).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_alpha_bounds_from_top_to_bottom_color() {
        let input = Canvas::new(1, 3, [255, 255, 255, 255]);
        let output = VerticalGradient
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 24.0,
                },
                &json!({"top":[255,255,255],"bottom":[128,128,128]}),
            )
            .unwrap();

        assert_eq!(output.pixel(0, 0), [255, 255, 255, 255]);
        assert_eq!(output.pixel(0, 2), [128, 128, 128, 255]);
        assert!(output.pixel(0, 1)[0] < 255);
        assert!(output.pixel(0, 1)[0] > 128);
    }

    #[test]
    fn uses_composition_coordinates_and_brightness_when_supplied() {
        let input = Canvas::new(1, 4, [255, 255, 255, 255]);
        let output = VerticalGradient
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 24.0,
                },
                &json!({
                    "top": [100, 100, 100],
                    "bottom": [0, 0, 0],
                    "start_xy": [0, 1],
                    "end_xy": [0, 3],
                    "brightness": 1.5
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(0, 0), [150, 150, 150, 255]);
        assert_eq!(output.pixel(0, 1), [150, 150, 150, 255]);
        assert_eq!(output.pixel(0, 2), [75, 75, 75, 255]);
        assert_eq!(output.pixel(0, 3), [0, 0, 0, 255]);
    }
}
