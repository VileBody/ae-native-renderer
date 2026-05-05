use crate::{
    box_blur::{blur_canvas, blur_radius, canvas_alpha_stats, canvas_debug_hash, CanvasAlphaStats},
    param_f32_at_any, param_value, Effect, EffectContext,
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
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = GlowParams::from_json(params, ctx.time);
        let threshold = params.threshold.clamp(0.0, 255.0);
        let radius = blur_radius(params.radius / 2.0);
        let intensity = params.intensity.max(0.0);

        let source = glow_source(input, threshold, params.based_on);

        let mut glow = blur_canvas(&source, radius);
        scale_canvas(&mut glow, intensity);

        let mut output = glow;
        composite_normal(&mut output, input, 100.0);
        Ok(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlowParams {
    pub based_on: GlowBasedOn,
    pub threshold: f32,
    pub radius: f32,
    pub intensity: f32,
}

impl GlowParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            based_on: GlowBasedOn::from_params(params),
            threshold: param_f32_at_any(params, &["threshold", "Threshold", "0002"], time, 0.0),
            radius: param_f32_at_any(params, &["radius", "Radius", "0003"], time, 16.0),
            intensity: param_f32_at_any(params, &["intensity", "Intensity", "0004"], time, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlowBasedOn {
    Combined,
    ColorChannels,
    AlphaChannel,
}

impl GlowBasedOn {
    fn from_params(params: &Value) -> Self {
        Self::resolve(params).based_on
    }

    fn resolve(params: &Value) -> GlowBasedOnResolution {
        for name in ["based_on", "basedOn", "Glow Based On", "0001"] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if text.contains("alpha") {
                    return GlowBasedOnResolution {
                        based_on: Self::AlphaChannel,
                        param_source: name,
                        raw_number: None,
                    };
                }
                if text.contains("color")
                    || text.contains("rgb")
                    || text.contains("luma")
                    || text.contains("luminance")
                {
                    return GlowBasedOnResolution {
                        based_on: Self::ColorChannels,
                        param_source: name,
                        raw_number: None,
                    };
                }
                if text.contains("both") || text.contains("combined") {
                    return GlowBasedOnResolution {
                        based_on: Self::Combined,
                        param_source: name,
                        raw_number: None,
                    };
                }
            }
            if let Some(number) = value
                .as_i64()
                .or_else(|| value.as_f64().map(|value| value.round() as i64))
            {
                let based_on = match number {
                    1 => Self::ColorChannels,
                    2 => Self::AlphaChannel,
                    _ => Self::Combined,
                };
                return GlowBasedOnResolution {
                    based_on,
                    param_source: name,
                    raw_number: Some(number),
                };
            }
        }
        GlowBasedOnResolution {
            based_on: Self::Combined,
            param_source: "default_absent",
            raw_number: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GlowBasedOnResolution {
    based_on: GlowBasedOn,
    param_source: &'static str,
    raw_number: Option<i64>,
}

fn based_on_label(based_on: GlowBasedOn) -> &'static str {
    match based_on {
        GlowBasedOn::Combined => "combined",
        GlowBasedOn::ColorChannels => "color_channels",
        GlowBasedOn::AlphaChannel => "alpha_channel",
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowDebugParams {
    pub based_on: &'static str,
    pub based_on_param_source: &'static str,
    pub based_on_raw_number: Option<i64>,
    pub threshold: f32,
    pub radius: f32,
    pub intensity: f32,
    pub kernel_radius: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlowIntermediateHashes {
    pub input_rgba: u64,
    pub threshold_source_rgba: u64,
    pub blurred_glow_rgba: u64,
    pub intensity_scaled_glow_rgba: u64,
    pub final_rgba: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowIntermediateAlphaStats {
    pub input: CanvasAlphaStats,
    pub threshold_source: CanvasAlphaStats,
    pub blurred_glow: CanvasAlphaStats,
    pub intensity_scaled_glow: CanvasAlphaStats,
    pub final_output: CanvasAlphaStats,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowDebugTrace {
    pub params: GlowDebugParams,
    pub hashes: GlowIntermediateHashes,
    pub alpha: GlowIntermediateAlphaStats,
}

pub fn glow_debug_trace(input: &Canvas, params: &Value, time: f64) -> GlowDebugTrace {
    let based_on_resolution = GlowBasedOn::resolve(params);
    let params = GlowParams::from_json(params, time);
    let threshold = params.threshold.clamp(0.0, 255.0);
    let kernel_radius = blur_radius(params.radius / 2.0);
    let intensity = params.intensity.max(0.0);
    let source = glow_source(input, threshold, params.based_on);
    let blurred = blur_canvas(&source, kernel_radius);
    let mut scaled = blurred.clone();
    scale_canvas(&mut scaled, intensity);

    let mut output = scaled.clone();
    composite_normal(&mut output, input, 100.0);

    GlowDebugTrace {
        params: GlowDebugParams {
            based_on: based_on_label(params.based_on),
            based_on_param_source: based_on_resolution.param_source,
            based_on_raw_number: based_on_resolution.raw_number,
            threshold: params.threshold,
            radius: params.radius,
            intensity: params.intensity,
            kernel_radius,
        },
        hashes: GlowIntermediateHashes {
            input_rgba: canvas_debug_hash(input),
            threshold_source_rgba: canvas_debug_hash(&source),
            blurred_glow_rgba: canvas_debug_hash(&blurred),
            intensity_scaled_glow_rgba: canvas_debug_hash(&scaled),
            final_rgba: canvas_debug_hash(&output),
        },
        alpha: GlowIntermediateAlphaStats {
            input: canvas_alpha_stats(input),
            threshold_source: canvas_alpha_stats(&source),
            blurred_glow: canvas_alpha_stats(&blurred),
            intensity_scaled_glow: canvas_alpha_stats(&scaled),
            final_output: canvas_alpha_stats(&output),
        },
    }
}

fn glow_source(input: &Canvas, threshold: f32, based_on: GlowBasedOn) -> Canvas {
    let mut source = Canvas::transparent(input.width, input.height);
    for y in 0..input.height {
        for x in 0..input.width {
            let pixel = input.pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            let luminance =
                0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
            let passes = match based_on {
                GlowBasedOn::Combined => luminance >= threshold || pixel[3] as f32 >= threshold,
                GlowBasedOn::ColorChannels => luminance >= threshold,
                GlowBasedOn::AlphaChannel => pixel[3] as f32 >= threshold,
            };
            if passes {
                source.set_pixel(x, y, pixel);
            }
        }
    }
    source
}

fn scale_canvas(canvas: &mut Canvas, intensity: f32) {
    for chunk in canvas.data.chunks_exact_mut(4) {
        let alpha_scale = intensity.clamp(0.0, 8.0);
        chunk[0] = (chunk[0] as f32 * intensity).round().clamp(0.0, 255.0) as u8;
        chunk[1] = (chunk[1] as f32 * intensity).round().clamp(0.0, 255.0) as u8;
        chunk[2] = (chunk[2] as f32 * intensity).round().clamp(0.0, 255.0) as u8;
        chunk[3] = (chunk[3] as f32 * alpha_scale).round().clamp(0.0, 255.0) as u8;
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
        let params = GlowParams::from_json(
            &json!({
            "0001": { "value": 2 },
            "0002": { "value": 42 },
            "0003": { "value": 20 },
            "0004": { "value": 1.5 }
            }),
            0.0,
        );

        assert_eq!(params.based_on, GlowBasedOn::AlphaChannel);
        assert_eq!(params.threshold, 42.0);
        assert_eq!(params.radius, 20.0);
        assert_eq!(params.intensity, 1.5);
    }

    #[test]
    fn color_channels_based_on_ignores_high_alpha_dark_pixels() {
        let mut input = Canvas::transparent(2, 1);
        input.set_pixel(0, 0, [32, 32, 32, 255]);
        input.set_pixel(1, 0, [240, 240, 240, 64]);

        let source = glow_source(&input, 120.0, GlowBasedOn::ColorChannels);

        assert_eq!(source.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(source.pixel(1, 0), [240, 240, 240, 64]);
    }

    #[test]
    fn alpha_channel_based_on_ignores_low_alpha_bright_pixels() {
        let mut input = Canvas::transparent(2, 1);
        input.set_pixel(0, 0, [240, 240, 240, 64]);
        input.set_pixel(1, 0, [32, 32, 32, 255]);

        let source = glow_source(&input, 120.0, GlowBasedOn::AlphaChannel);

        assert_eq!(source.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(source.pixel(1, 0), [32, 32, 32, 255]);
    }

    #[test]
    fn debug_trace_reports_distinct_threshold_sources_for_based_on_modes() {
        let mut input = Canvas::transparent(2, 1);
        input.set_pixel(0, 0, [32, 32, 32, 255]);
        input.set_pixel(1, 0, [240, 240, 240, 64]);

        let combined = glow_debug_trace(
            &input,
            &json!({ "0001": "combined", "0002": 120, "0003": 0, "0004": 1.0 }),
            0.0,
        );
        let color = glow_debug_trace(
            &input,
            &json!({ "0001": { "value": 1 }, "0002": 120, "0003": 0, "0004": 1.0 }),
            0.0,
        );
        let alpha = glow_debug_trace(
            &input,
            &json!({ "0001": { "value": 2 }, "0002": 120, "0003": 0, "0004": 1.0 }),
            0.0,
        );
        let combined_source = glow_source(&input, 120.0, GlowBasedOn::Combined);
        let color_source = glow_source(&input, 120.0, GlowBasedOn::ColorChannels);
        let alpha_source = glow_source(&input, 120.0, GlowBasedOn::AlphaChannel);

        assert_eq!(combined.params.based_on, "combined");
        assert_eq!(color.params.based_on, "color_channels");
        assert_eq!(alpha.params.based_on, "alpha_channel");
        assert_eq!(combined.params.based_on_param_source, "0001");
        assert_eq!(color.params.based_on_param_source, "0001");
        assert_eq!(color.params.based_on_raw_number, Some(1));
        assert_eq!(alpha.params.based_on_raw_number, Some(2));
        assert_eq!(
            combined.hashes.threshold_source_rgba,
            canvas_debug_hash(&combined_source)
        );
        assert_eq!(
            color.hashes.threshold_source_rgba,
            canvas_debug_hash(&color_source)
        );
        assert_eq!(
            alpha.hashes.threshold_source_rgba,
            canvas_debug_hash(&alpha_source)
        );
        assert_ne!(
            combined.hashes.threshold_source_rgba,
            color.hashes.threshold_source_rgba
        );
        assert_ne!(
            color.hashes.threshold_source_rgba,
            alpha.hashes.threshold_source_rgba
        );
        assert_eq!(combined_source.pixel(0, 0), [32, 32, 32, 255]);
        assert_eq!(combined_source.pixel(1, 0), [240, 240, 240, 64]);
        assert_eq!(color_source.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(color_source.pixel(1, 0), [240, 240, 240, 64]);
        assert_eq!(alpha_source.pixel(0, 0), [32, 32, 32, 255]);
        assert_eq!(alpha_source.pixel(1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn params_sample_time_varying_threshold_radius_and_intensity() {
        let params = json!({
            "0002": {
                "keyframes": [
                    { "t": 0.0, "v": 120.0 },
                    { "t": 1.0, "v": 180.0 }
                ]
            },
            "0003": {
                "keyframes": [
                    { "t": 0.0, "v": 10.0 },
                    { "t": 1.0, "v": 50.0 }
                ]
            },
            "0004": {
                "keyframes": [
                    { "t": 0.0, "v": 0.5 },
                    { "t": 1.0, "v": 2.5 }
                ]
            }
        });

        let early = GlowParams::from_json(&params, 0.0);
        let mid = GlowParams::from_json(&params, 0.5);
        let late = GlowParams::from_json(&params, 1.0);

        assert_eq!(early.threshold, 120.0);
        assert_eq!(mid.threshold, 150.0);
        assert_eq!(late.threshold, 180.0);
        assert_eq!(early.radius, 10.0);
        assert_eq!(mid.radius, 30.0);
        assert_eq!(late.radius, 50.0);
        assert_eq!(early.intensity, 0.5);
        assert_eq!(mid.intensity, 1.5);
        assert_eq!(late.intensity, 2.5);
    }

    #[test]
    fn render_samples_animated_radius_at_context_time() {
        let mut input = Canvas::transparent(5, 1);
        input.set_pixel(2, 0, [255, 255, 255, 255]);
        let params = json!({
            "0002": 0,
            "0003": {
                "keyframes": [
                    { "t": 0.0, "v": 0.0 },
                    { "t": 1.0, "v": 4.0 }
                ]
            },
            "0004": 1.0
        });

        let early = Glow::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &params,
            )
            .unwrap();
        let late = Glow::default()
            .render(
                &input,
                &EffectContext {
                    time: 1.0,
                    fps: 30.0,
                },
                &params,
            )
            .unwrap();

        assert_eq!(early.pixel(0, 0)[3], 0);
        assert!(late.pixel(0, 0)[3] > 0);
        assert_ne!(canvas_debug_hash(&early), canvas_debug_hash(&late));
    }

    #[test]
    fn glow_radius_uses_shared_ceil_quantization_after_half_mapping() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [255, 255, 255, 255]);

        let trace = glow_debug_trace(&input, &json!({ "0002": 0, "0003": 0.5, "0004": 1.0 }), 0.0);

        assert_eq!(trace.params.radius, 0.5);
        assert_eq!(trace.params.kernel_radius, 1);
    }

    #[test]
    fn debug_trace_reports_threshold_and_glow_alpha_intermediates() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(0, 0, [32, 32, 32, 255]);
        input.set_pixel(1, 0, [240, 240, 240, 64]);

        let trace = glow_debug_trace(
            &input,
            &json!({ "0001": "color channels", "0002": 120, "0003": 2, "0004": 1.0 }),
            0.0,
        );

        assert_eq!(trace.params.based_on, "color_channels");
        assert_eq!(trace.alpha.input.nonzero_pixels, 2);
        assert_eq!(trace.alpha.threshold_source.nonzero_pixels, 1);
        assert!(
            trace.alpha.blurred_glow.nonzero_pixels >= trace.alpha.threshold_source.nonzero_pixels
        );
        assert!(trace.alpha.final_output.nonzero_pixels >= trace.alpha.input.nonzero_pixels);
    }

    #[test]
    fn debug_trace_marks_absent_based_on_as_default_source() {
        let input = Canvas::transparent(1, 1);

        let trace = glow_debug_trace(&input, &json!({ "0002": 120, "0003": 0 }), 0.0);

        assert_eq!(trace.params.based_on, "combined");
        assert_eq!(trace.params.based_on_param_source, "default_absent");
        assert_eq!(trace.params.based_on_raw_number, None);
    }
}
