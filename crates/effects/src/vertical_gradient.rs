use crate::{param_rgba, Effect, EffectContext};
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

        let span = max_y.saturating_sub(min_y).max(1) as f32;
        let mut output = input.clone();
        for y in min_y..=max_y {
            let t = (y - min_y) as f32 / span;
            let gradient = [0, 1, 2].map(|channel| {
                (top[channel] as f32 + (bottom[channel] as f32 - top[channel] as f32) * t)
                    .round()
                    .clamp(0.0, 255.0) as u8
            });
            for x in 0..input.width {
                let source = input.pixel(x, y);
                if source[3] == 0 {
                    continue;
                }
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
}
