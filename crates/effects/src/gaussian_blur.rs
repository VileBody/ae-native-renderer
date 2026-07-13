use crate::{
    glow::{ir_gaussian_blur_canvas, IrGaussianBlurOptions, AE_IR_GAUSSIAN_RADIUS_SCALE},
    param_bool_any, param_f32_at_any, param_value, Effect, EffectContext,
};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct GaussianBlur2;

impl Effect for GaussianBlur2 {
    fn match_name(&self) -> &'static str {
        "ADBE Gaussian Blur 2"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = GaussianBlur2Params::from_json(params, ctx.time);
        let blurriness = if params.blurriness.is_finite() {
            params.blurriness.max(0.0)
        } else {
            0.0
        };
        let (horizontal, vertical) = params.dimensions.axes();
        Ok(ir_gaussian_blur_canvas(
            input,
            blurriness * AE_IR_GAUSSIAN_RADIUS_SCALE,
            IrGaussianBlurOptions {
                horizontal,
                vertical,
                repeat_edge_pixels: params.repeat_edge_pixels,
            },
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GaussianBlur2Params {
    pub blurriness: f32,
    pub dimensions: BlurDimensions,
    pub repeat_edge_pixels: bool,
}

impl GaussianBlur2Params {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            blurriness: param_f32_at_any(
                params,
                &[
                    "blurriness",
                    "Blurriness",
                    "blur_radius",
                    "blurRadius",
                    "radius",
                    "0001",
                    "ADBE Gaussian Blur 2-0001",
                ],
                time,
                0.0,
            ),
            dimensions: BlurDimensions::from_params(params),
            repeat_edge_pixels: param_bool_any(
                params,
                &[
                    "repeat_edge_pixels",
                    "repeatEdgePixels",
                    "Repeat Edge Pixels",
                    "0003",
                    "ADBE Gaussian Blur 2-0003",
                ],
                false,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlurDimensions {
    HorizontalAndVertical,
    Horizontal,
    Vertical,
}

impl BlurDimensions {
    fn from_params(params: &Value) -> Self {
        for name in [
            "dimensions",
            "blur_dimensions",
            "blurDimensions",
            "Blur Dimensions",
            "0002",
            "ADBE Gaussian Blur 2-0002",
        ] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if (text.contains("horizontal") && text.contains("vertical"))
                    || text.contains("both")
                {
                    return Self::HorizontalAndVertical;
                }
                if text.contains("horizontal") {
                    return Self::Horizontal;
                }
                if text.contains("vertical") {
                    return Self::Vertical;
                }
                if let Ok(number) = text.trim().parse::<i64>() {
                    return Self::from_ae_number(number);
                }
            }
            if let Some(number) = value
                .as_i64()
                .or_else(|| value.as_f64().map(|number| number.round() as i64))
            {
                return Self::from_ae_number(number);
            }
        }
        Self::HorizontalAndVertical
    }

    fn from_ae_number(number: i64) -> Self {
        match number {
            2 => Self::Horizontal,
            3 => Self::Vertical,
            _ => Self::HorizontalAndVertical,
        }
    }

    fn axes(self) -> (bool, bool) {
        match self {
            Self::HorizontalAndVertical => (true, true),
            Self::Horizontal => (true, false),
            Self::Vertical => (false, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn context(time: f64) -> EffectContext {
        EffectContext { time, fps: 30.0 }
    }

    #[test]
    fn params_accept_named_numbered_and_full_match_names() {
        let named = GaussianBlur2Params::from_json(
            &json!({
                "blurRadius": 10,
                "blurDimensions": "vertical",
                "repeatEdgePixels": true
            }),
            0.0,
        );
        assert_eq!(named.blurriness, 10.0);
        assert_eq!(named.dimensions, BlurDimensions::Vertical);
        assert!(named.repeat_edge_pixels);

        let numbered = GaussianBlur2Params::from_json(
            &json!({
                "0001": { "value": 12 },
                "0002": { "value": 2 },
                "0003": { "value": 1 }
            }),
            0.0,
        );
        assert_eq!(numbered.blurriness, 12.0);
        assert_eq!(numbered.dimensions, BlurDimensions::Horizontal);
        assert!(numbered.repeat_edge_pixels);

        let full_match_names = GaussianBlur2Params::from_json(
            &json!({
                "ADBE Gaussian Blur 2-0001": 8,
                "ADBE Gaussian Blur 2-0002": 3,
                "ADBE Gaussian Blur 2-0003": 0
            }),
            0.0,
        );
        assert_eq!(full_match_names.blurriness, 8.0);
        assert_eq!(full_match_names.dimensions, BlurDimensions::Vertical);
        assert!(!full_match_names.repeat_edge_pixels);
    }

    #[test]
    fn zero_blurriness_is_a_noop() {
        let input = Canvas::new(3, 2, [10, 20, 30, 40]);
        let output = GaussianBlur2
            .render(&input, &context(0.0), &json!({ "0001": 0 }))
            .unwrap();
        assert_eq!(output.data, input.data);
    }

    #[test]
    fn blur_dimensions_enable_the_expected_axes() {
        let mut input = Canvas::transparent(5, 5);
        input.set_pixel(2, 2, [255, 255, 255, 255]);

        let horizontal = GaussianBlur2
            .render(&input, &context(0.0), &json!({ "0001": 2, "0002": 2 }))
            .unwrap();
        assert!(horizontal.pixel(1, 2)[3] > 0);
        assert_eq!(horizontal.pixel(2, 1)[3], 0);

        let vertical = GaussianBlur2
            .render(&input, &context(0.0), &json!({ "0001": 2, "0002": 3 }))
            .unwrap();
        assert_eq!(vertical.pixel(1, 2)[3], 0);
        assert!(vertical.pixel(2, 1)[3] > 0);

        let both = GaussianBlur2
            .render(&input, &context(0.0), &json!({ "0001": 2, "0002": 1 }))
            .unwrap();
        assert!(both.pixel(1, 2)[3] > 0);
        assert!(both.pixel(2, 1)[3] > 0);
    }

    #[test]
    fn repeat_edge_pixels_extends_boundary_values() {
        let input = Canvas::new(5, 5, [180, 120, 60, 200]);
        let repeated = GaussianBlur2
            .render(&input, &context(0.0), &json!({ "0001": 10, "0003": true }))
            .unwrap();
        let transparent_outside = GaussianBlur2
            .render(&input, &context(0.0), &json!({ "0001": 10, "0003": false }))
            .unwrap();

        assert_eq!(repeated.data, input.data);
        assert!(transparent_outside.pixel(0, 0)[3] < input.pixel(0, 0)[3]);
        assert!(transparent_outside.pixel(0, 0)[0] < input.pixel(0, 0)[0]);
    }

    #[test]
    fn blurriness_is_sampled_at_context_time() {
        let params = json!({
            "0001": {
                "keyframes": [
                    { "t": 0.0, "v": 0.0 },
                    { "t": 1.0, "v": 10.0 }
                ]
            }
        });
        assert_eq!(GaussianBlur2Params::from_json(&params, 0.5).blurriness, 5.0);

        let mut input = Canvas::transparent(5, 1);
        input.set_pixel(2, 0, [255, 255, 255, 255]);
        let early = GaussianBlur2
            .render(&input, &context(0.0), &params)
            .unwrap();
        let late = GaussianBlur2
            .render(&input, &context(1.0), &params)
            .unwrap();
        assert_eq!(early.data, input.data);
        assert_ne!(late.data, input.data);
    }
}
