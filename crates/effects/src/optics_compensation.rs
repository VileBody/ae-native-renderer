use crate::{
    directional_blur::{sample_premultiplied_bilinear, straight_rgba},
    param_bool_any, param_f32_at_any, Effect, EffectContext,
};
use raster_cpu::Canvas;
use rayon::prelude::*;
use serde_json::Value;

const FIELD_OF_VIEW_NAMES: &[&str] = &[
    "field_of_view",
    "fieldOfView",
    "Field of View",
    "fov",
    "0001",
    "ADBE Optics Compensation-0001",
];
const REVERSE_NAMES: &[&str] = &[
    "reverse_lens_distortion",
    "reverseLensDistortion",
    "Reverse Lens Distortion",
    "reverse",
    "0002",
    "ADBE Optics Compensation-0002",
];

#[derive(Debug, Default)]
pub struct OpticsCompensation;

impl Effect for OpticsCompensation {
    fn match_name(&self) -> &'static str {
        "ADBE Optics Compensation"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = OpticsCompensationParams::from_json(params, ctx.time);
        if params.field_of_view_degrees <= f32::EPSILON || input.width == 0 || input.height == 0 {
            return Ok(input.clone());
        }

        Ok(optics_compensation(input, params))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OpticsCompensationParams {
    pub field_of_view_degrees: f32,
    pub reverse_lens_distortion: bool,
}

impl OpticsCompensationParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        let field_of_view_degrees = param_f32_at_any(params, FIELD_OF_VIEW_NAMES, time, 0.0);
        Self {
            field_of_view_degrees: if field_of_view_degrees.is_finite() {
                field_of_view_degrees.abs().clamp(0.0, 179.0)
            } else {
                0.0
            },
            reverse_lens_distortion: param_bool_any(params, REVERSE_NAMES, false),
        }
    }
}

fn optics_compensation(input: &Canvas, params: OpticsCompensationParams) -> Canvas {
    let center_x = (input.width.saturating_sub(1)) as f32 * 0.5;
    let center_y = (input.height.saturating_sub(1)) as f32 * 0.5;
    let normalization_radius = center_x.hypot(center_y).max(0.5);
    let half_fov = (params.field_of_view_degrees * 0.5).to_radians();
    let tan_half_fov = half_fov.tan();
    let mut output = Canvas::transparent(input.width, input.height);

    output
        .data
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(index, destination)| {
            let x = (index % input.width as usize) as u32;
            let y = (index / input.width as usize) as u32;
            let offset_x = x as f32 - center_x;
            let offset_y = y as f32 - center_y;
            let radius = offset_x.hypot(offset_y);
            if radius <= f32::EPSILON {
                destination.copy_from_slice(&input.pixel(x, y));
                return;
            }

            let normalized_radius = radius / normalization_radius;
            let source_radius = mapped_source_radius(
                normalized_radius,
                half_fov,
                tan_half_fov,
                params.reverse_lens_distortion,
            );
            if !source_radius.is_finite() {
                return;
            }
            let scale = source_radius / normalized_radius;
            let source_x = center_x + offset_x * scale;
            let source_y = center_y + offset_y * scale;
            destination.copy_from_slice(&straight_rgba(sample_premultiplied_bilinear(
                input, source_x, source_y,
            )));
        });

    output
}

fn mapped_source_radius(radius: f32, half_fov: f32, tan_half_fov: f32, reverse: bool) -> f32 {
    if reverse {
        (radius * half_fov).tan() / tan_half_fov
    } else {
        (radius * tan_half_fov).atan() / half_fov
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn context(time: f64) -> EffectContext {
        EffectContext { time, fps: 24.0 }
    }

    #[test]
    fn accepts_ae_match_name_params_and_animated_fov() {
        let params = json!({
            "ADBE Optics Compensation-0001": {
                "keyframes": [{"t": 0.0, "v": 40.0}, {"t": 1.0, "v": 120.0}]
            },
            "ADBE Optics Compensation-0002": 1
        });
        let parsed = OpticsCompensationParams::from_json(&params, 0.5);
        assert!((parsed.field_of_view_degrees - 80.0).abs() < 0.001);
        assert!(parsed.reverse_lens_distortion);
    }

    #[test]
    fn zero_fov_is_an_exact_identity() {
        let input = checkerboard(7, 7);
        let output = OpticsCompensation
            .render(
                &input,
                &context(0.0),
                &json!({"field_of_view": 0.0, "reverse": true}),
            )
            .unwrap();
        assert_eq!(output.data, input.data);
    }

    #[test]
    fn reverse_fov_warps_checkerboard_but_preserves_center() {
        let input = checkerboard(9, 9);
        let output = OpticsCompensation
            .render(
                &input,
                &context(0.0),
                &json!({"field_of_view": 100.0, "reverse": true}),
            )
            .unwrap();

        assert_eq!(output.pixel(4, 4), input.pixel(4, 4));
        assert_ne!(output.data, input.data);
    }

    #[test]
    fn non_reverse_distortion_uses_transparent_overscan() {
        let input = Canvas::new(11, 11, [255, 255, 255, 255]);
        let output = OpticsCompensation
            .render(
                &input,
                &context(0.0),
                &json!({"field_of_view": 140.0, "reverse": false}),
            )
            .unwrap();

        assert_eq!(output.pixel(5, 5), [255, 255, 255, 255]);
        assert!(output.pixel(0, 5)[3] < 255);
        assert!(output.pixel(10, 5)[3] < 255);
    }

    #[test]
    fn impulse_is_radially_displaced_without_cross_channel_leakage() {
        let mut input = Canvas::transparent(11, 11);
        input.set_pixel(7, 5, [40, 180, 240, 128]);
        let output = OpticsCompensation
            .render(
                &input,
                &context(0.0),
                &json!({"field_of_view": 90.0, "reverse": true}),
            )
            .unwrap();

        let colored_pixels: Vec<[u8; 4]> = output
            .data
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
            .collect();
        assert!(!colored_pixels.is_empty());
        assert!(colored_pixels
            .iter()
            .all(|pixel| pixel[0] == 40 && pixel[1] == 180 && pixel[2] == 240));
    }

    #[test]
    fn partial_alpha_bilinear_sampling_stays_premultiplied() {
        let input = Canvas::from_rgba(2, 1, vec![255, 0, 0, 0, 0, 255, 0, 128]).unwrap();
        let sampled = straight_rgba(sample_premultiplied_bilinear(&input, 0.5, 0.0));
        assert_eq!(sampled, [0, 255, 0, 64]);
    }

    fn checkerboard(width: u32, height: u32) -> Canvas {
        let mut canvas = Canvas::transparent(width, height);
        for y in 0..height {
            for x in 0..width {
                let value = if (x + y) % 2 == 0 { 255 } else { 0 };
                canvas.set_pixel(x, y, [value, value, value, 255]);
            }
        }
        canvas
    }
}
