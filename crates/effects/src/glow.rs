use crate::{
    box_blur::{blur_canvas, blur_radius},
    param_f32_any, Effect, EffectContext,
};
use raster_cpu::{composite_normal, Canvas};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct Glow;

impl Effect for Glow {
    fn match_name(&self) -> &'static str {
        "ADBE Glo2"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = GlowParams::from_json(params);
        let threshold = params.threshold.clamp(0.0, 255.0);
        let radius = blur_radius(params.radius / 2.0);
        let intensity = params.intensity.max(0.0);

        let mut source = Canvas::transparent(input.width, input.height);
        for y in 0..input.height {
            for x in 0..input.width {
                let pixel = input.pixel(x, y);
                if pixel[3] == 0 {
                    continue;
                }
                let luminance = 0.2126 * pixel[0] as f32
                    + 0.7152 * pixel[1] as f32
                    + 0.0722 * pixel[2] as f32;
                if luminance >= threshold || pixel[3] as f32 >= threshold {
                    source.set_pixel(x, y, pixel);
                }
            }
        }

        let mut glow = blur_canvas(&source, radius);
        scale_canvas(&mut glow, intensity);

        let mut output = glow;
        composite_normal(&mut output, input, 100.0);
        Ok(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlowParams {
    pub threshold: f32,
    pub radius: f32,
    pub intensity: f32,
}

impl GlowParams {
    pub(crate) fn from_json(params: &Value) -> Self {
        Self {
            threshold: param_f32_any(params, &["threshold", "Threshold", "0002"], 0.0),
            radius: param_f32_any(params, &["radius", "Radius", "0003"], 16.0),
            intensity: param_f32_any(params, &["intensity", "Intensity", "0004"], 1.0),
        }
    }
}

fn scale_canvas(canvas: &mut Canvas, intensity: f32) {
    for chunk in canvas.data.chunks_exact_mut(4) {
        let alpha_scale = intensity.clamp(0.0, 8.0);
        chunk[0] = (chunk[0] as f32 * intensity)
            .round()
            .clamp(0.0, 255.0) as u8;
        chunk[1] = (chunk[1] as f32 * intensity)
            .round()
            .clamp(0.0, 255.0) as u8;
        chunk[2] = (chunk[2] as f32 * intensity)
            .round()
            .clamp(0.0, 255.0) as u8;
        chunk[3] = (chunk[3] as f32 * alpha_scale)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn glow_keeps_original_and_adds_blurred_alpha() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [255, 255, 255, 255]);

        let output = Glow::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({
                    "0002": 0,
                    "0003": 2,
                    "0004": 1.0
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(1, 0)[3], 255);
        assert!(output.pixel(0, 0)[3] > 0);
        assert!(output.pixel(2, 0)[3] > 0);
    }

    #[test]
    fn params_accept_ae_numbered_values() {
        let params = GlowParams::from_json(&json!({
            "0002": { "value": 42 },
            "0003": { "value": 20 },
            "0004": { "value": 1.5 }
        }));

        assert_eq!(params.threshold, 42.0);
        assert_eq!(params.radius, 20.0);
        assert_eq!(params.intensity, 1.5);
    }
}
