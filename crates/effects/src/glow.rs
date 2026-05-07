use crate::{
    box_blur::{canvas_alpha_stats, canvas_debug_hash, CanvasAlphaStats},
    param_f32_at_any, param_value, Effect, EffectContext,
};
use raster_cpu::{composite_normal, Canvas};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct Glow;

const AE_GLOW_IR_GAUSSIAN_RADIUS_SCALE: f32 = 0.4;
const IR_GAUSSIAN_MIN_RADIUS: f32 = 0.1;
const IR_GAUSSIAN_THETA: f64 = 0.844_799_995_422_363_3;
const IR_GAUSSIAN_EXP1: f64 = -1.259_999_990_463_256_8;
const IR_GAUSSIAN_EXP2: f64 = -2.519_999_980_926_513_7;
const IR_GAUSSIAN_K0: f64 = 0.962_899_982_929_229_7;
const IR_GAUSSIAN_K1: f64 = 1.942_000_031_471_252_4;

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
        let intensity = params.intensity.max(0.0);

        let source = glow_source(input, threshold, params.based_on);

        let mut glow = glow_blur_canvas(&source, glow_ir_gaussian_radius(params.radius));
        scale_canvas(&mut glow, intensity);

        let output = composite_glow(input, &glow, params.operation, params.composite_original);
        Ok(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlowParams {
    pub based_on: GlowBasedOn,
    pub threshold: f32,
    pub radius: f32,
    pub intensity: f32,
    pub composite_original: GlowCompositeOriginal,
    pub operation: GlowOperation,
}

impl GlowParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            based_on: GlowBasedOn::from_params(params),
            threshold: param_f32_at_any(params, &["threshold", "Threshold", "0002"], time, 0.0),
            radius: param_f32_at_any(params, &["radius", "Radius", "0003"], time, 16.0),
            intensity: param_f32_at_any(params, &["intensity", "Intensity", "0004"], time, 1.0),
            composite_original: GlowCompositeOriginal::from_params(params),
            operation: GlowOperation::from_params(params),
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
                    1 => Self::AlphaChannel,
                    2 => Self::ColorChannels,
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
            based_on: Self::ColorChannels,
            param_source: "default_absent",
            raw_number: Some(2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlowCompositeOriginal {
    None,
    Behind,
    OnTop,
}

impl GlowCompositeOriginal {
    fn from_params(params: &Value) -> Self {
        for name in [
            "composite_original",
            "compositeOriginal",
            "Composite Original",
            "0005",
        ] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if text.contains("none") {
                    return Self::None;
                }
                if text.contains("behind") {
                    return Self::Behind;
                }
                if text.contains("top") || text.contains("front") {
                    return Self::OnTop;
                }
            }
            if let Some(number) = value
                .as_i64()
                .or_else(|| value.as_f64().map(|value| value.round() as i64))
            {
                return match number {
                    1 => Self::Behind,
                    2 => Self::OnTop,
                    _ => Self::None,
                };
            }
        }
        Self::OnTop
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlowOperation {
    None,
    Normal,
    Add,
    Screen,
}

impl GlowOperation {
    fn from_params(params: &Value) -> Self {
        for name in ["operation", "Glow Operation", "0006"] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if text.contains("none") {
                    return Self::None;
                }
                if text.contains("normal") {
                    return Self::Normal;
                }
                if text.contains("screen") {
                    return Self::Screen;
                }
                if text.contains("add") {
                    return Self::Add;
                }
            }
            if let Some(number) = value
                .as_i64()
                .or_else(|| value.as_f64().map(|value| value.round() as i64))
            {
                return match number {
                    1 => Self::None,
                    2 => Self::Normal,
                    3 => Self::Add,
                    6 => Self::Screen,
                    _ => Self::Add,
                };
            }
        }
        Self::Add
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

fn composite_original_label(composite_original: GlowCompositeOriginal) -> &'static str {
    match composite_original {
        GlowCompositeOriginal::None => "none",
        GlowCompositeOriginal::Behind => "behind",
        GlowCompositeOriginal::OnTop => "on_top",
    }
}

fn operation_label(operation: GlowOperation) -> &'static str {
    match operation {
        GlowOperation::None => "none",
        GlowOperation::Normal => "normal",
        GlowOperation::Add => "add",
        GlowOperation::Screen => "screen",
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowDebugParams {
    pub based_on: &'static str,
    pub based_on_param_source: &'static str,
    pub based_on_raw_number: Option<i64>,
    pub threshold: f32,
    pub radius: f32,
    pub ir_gaussian_radius: f32,
    pub intensity: f32,
    pub composite_original: &'static str,
    pub operation: &'static str,
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
    let ir_gaussian_radius = glow_ir_gaussian_radius(params.radius);
    let kernel_radius = glow_kernel_radius(params.radius);
    let intensity = params.intensity.max(0.0);
    let source = glow_source(input, threshold, params.based_on);
    let blurred = glow_blur_canvas(&source, ir_gaussian_radius);
    let mut scaled = blurred.clone();
    scale_canvas(&mut scaled, intensity);
    let output = composite_glow(input, &scaled, params.operation, params.composite_original);

    GlowDebugTrace {
        params: GlowDebugParams {
            based_on: based_on_label(params.based_on),
            based_on_param_source: based_on_resolution.param_source,
            based_on_raw_number: based_on_resolution.raw_number,
            threshold: params.threshold,
            radius: params.radius,
            ir_gaussian_radius,
            intensity: params.intensity,
            composite_original: composite_original_label(params.composite_original),
            operation: operation_label(params.operation),
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

fn glow_ir_gaussian_radius(radius: f32) -> f32 {
    radius * AE_GLOW_IR_GAUSSIAN_RADIUS_SCALE
}

fn glow_kernel_radius(radius: f32) -> u32 {
    glow_gaussian_support_radius(glow_ir_gaussian_radius(radius))
}

fn glow_gaussian_support_radius(sigma: f32) -> u32 {
    if sigma.is_nan() || sigma <= 0.0 {
        return 0;
    }
    (sigma * 2.0).ceil().clamp(1.0, 128.0) as u32
}

fn glow_blur_canvas(input: &Canvas, sigma: f32) -> Canvas {
    if sigma.is_nan() || sigma <= 0.0 || input.width == 0 || input.height == 0 {
        return input.clone();
    }

    let coeffs = ir_recursive_gaussian_coefficients(sigma.max(IR_GAUSSIAN_MIN_RADIUS));
    let horizontal = ir_recursive_gaussian_pass_horizontal(input, coeffs);
    ir_recursive_gaussian_pass_vertical(&horizontal, coeffs)
}

#[derive(Debug, Clone, Copy)]
struct IrRecursiveGaussianCoefficients {
    causal_current: f32,
    causal_previous_source: f32,
    feedback_previous: f32,
    feedback_previous2: f32,
    anticausal_next_source: f32,
    anticausal_next2_source: f32,
}

fn ir_recursive_gaussian_coefficients(radius: f32) -> IrRecursiveGaussianCoefficients {
    let radius = radius.max(IR_GAUSSIAN_MIN_RADIUS) as f64;
    let theta = IR_GAUSSIAN_THETA / radius;
    let exp1 = (IR_GAUSSIAN_EXP1 / radius).exp();
    let exp2 = (IR_GAUSSIAN_EXP2 / radius).exp();
    let sin_theta = theta.sin();
    let cos_theta = theta.cos();
    let denominator = 1.0 - 2.0 * exp1 * cos_theta + exp2;
    let gain = (((1.0 - exp1 * cos_theta) / denominator) * IR_GAUSSIAN_K0
        + ((exp1 * sin_theta) / denominator) * IR_GAUSSIAN_K1)
        * 2.0
        - IR_GAUSSIAN_K0;
    let norm = 1.0 / gain;
    let causal_current = norm * IR_GAUSSIAN_K0;
    let causal_previous_source =
        (sin_theta * (norm * IR_GAUSSIAN_K1) - cos_theta * causal_current) * exp1;
    let feedback_previous = -2.0 * exp1 * cos_theta;
    let feedback_previous2 = exp2;
    let anticausal_next_source = causal_previous_source - feedback_previous * causal_current;
    let anticausal_next2_source = -feedback_previous2 * causal_current;

    IrRecursiveGaussianCoefficients {
        causal_current: causal_current as f32,
        causal_previous_source: causal_previous_source as f32,
        feedback_previous: feedback_previous as f32,
        feedback_previous2: feedback_previous2 as f32,
        anticausal_next_source: anticausal_next_source as f32,
        anticausal_next2_source: anticausal_next2_source as f32,
    }
}

fn ir_recursive_gaussian_pass_horizontal(
    input: &Canvas,
    coeffs: IrRecursiveGaussianCoefficients,
) -> Canvas {
    let mut output = Canvas::transparent(input.width, input.height);
    for y in 0..input.height {
        let mut line = Vec::with_capacity(input.width as usize);
        for x in 0..input.width {
            line.push(pixel_to_float(input.pixel(x, y)));
        }
        let filtered = ir_recursive_gaussian_filter_line(&line, coeffs);
        for (x, pixel) in filtered.into_iter().enumerate() {
            output.set_pixel(x as u32, y, float_pixel_to_u8(pixel));
        }
    }
    output
}

fn ir_recursive_gaussian_pass_vertical(
    input: &Canvas,
    coeffs: IrRecursiveGaussianCoefficients,
) -> Canvas {
    let mut output = Canvas::transparent(input.width, input.height);
    for x in 0..input.width {
        let mut line = Vec::with_capacity(input.height as usize);
        for y in 0..input.height {
            line.push(pixel_to_float(input.pixel(x, y)));
        }
        let filtered = ir_recursive_gaussian_filter_line(&line, coeffs);
        for (y, pixel) in filtered.into_iter().enumerate() {
            output.set_pixel(x, y as u32, float_pixel_to_u8(pixel));
        }
    }
    output
}

fn ir_recursive_gaussian_filter_line(
    source: &[[f32; 4]],
    coeffs: IrRecursiveGaussianCoefficients,
) -> Vec<[f32; 4]> {
    let len = source.len();
    if len == 0 {
        return Vec::new();
    }

    let mut causal = vec![[0.0_f32; 4]; len];
    let mut prev_source = [0.0_f32; 4];
    let mut prev = [0.0_f32; 4];
    let mut prev2 = [0.0_f32; 4];
    for (index, pixel) in source.iter().enumerate() {
        let mut next = [0.0_f32; 4];
        for channel in 0..4 {
            next[channel] = pixel[channel] * coeffs.causal_current
                + prev_source[channel] * coeffs.causal_previous_source
                - prev[channel] * coeffs.feedback_previous
                - prev2[channel] * coeffs.feedback_previous2;
        }
        causal[index] = next;
        prev_source = *pixel;
        prev2 = prev;
        prev = next;
    }

    let mut output = vec![[0.0_f32; 4]; len];
    let mut reverse_next = [0.0_f32; 4];
    let mut reverse_next2 = [0.0_f32; 4];
    for index in (0..len).rev() {
        let source_next = source.get(index + 1).copied().unwrap_or([0.0; 4]);
        let source_next2 = source.get(index + 2).copied().unwrap_or([0.0; 4]);
        let mut reverse = [0.0_f32; 4];
        for channel in 0..4 {
            reverse[channel] = source_next[channel] * coeffs.anticausal_next_source
                + source_next2[channel] * coeffs.anticausal_next2_source
                - reverse_next[channel] * coeffs.feedback_previous
                - reverse_next2[channel] * coeffs.feedback_previous2;
            output[index][channel] = causal[index][channel] + reverse[channel];
        }
        reverse_next2 = reverse_next;
        reverse_next = reverse;
    }

    output
}

fn pixel_to_float(pixel: [u8; 4]) -> [f32; 4] {
    [
        pixel[0] as f32,
        pixel[1] as f32,
        pixel[2] as f32,
        pixel[3] as f32,
    ]
}

fn float_pixel_to_u8(pixel: [f32; 4]) -> [u8; 4] {
    [
        pixel[0].round().clamp(0.0, 255.0) as u8,
        pixel[1].round().clamp(0.0, 255.0) as u8,
        pixel[2].round().clamp(0.0, 255.0) as u8,
        pixel[3].round().clamp(0.0, 255.0) as u8,
    ]
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

fn composite_glow(
    input: &Canvas,
    glow: &Canvas,
    operation: GlowOperation,
    composite_original: GlowCompositeOriginal,
) -> Canvas {
    let mut operated = match operation {
        GlowOperation::None => glow.clone(),
        GlowOperation::Normal => {
            let mut output = input.clone();
            composite_normal(&mut output, glow, 100.0);
            output
        }
        GlowOperation::Add => blend_glow_with_input(input, glow, add_channel),
        GlowOperation::Screen => blend_glow_with_input(input, glow, screen_channel),
    };

    match composite_original {
        GlowCompositeOriginal::None => operated,
        GlowCompositeOriginal::Behind => {
            composite_normal(&mut operated, input, 100.0);
            operated
        }
        GlowCompositeOriginal::OnTop => operated,
    }
}

fn blend_glow_with_input(input: &Canvas, glow: &Canvas, channel_blend: fn(u8, u8) -> u8) -> Canvas {
    let mut output = Canvas::transparent(input.width, input.height);
    for y in 0..input.height {
        for x in 0..input.width {
            let base = input.pixel(x, y);
            let top = glow.pixel(x, y);
            let alpha = source_over_alpha(base[3], top[3]);
            output.set_pixel(
                x,
                y,
                [
                    channel_blend(base[0], top[0]),
                    channel_blend(base[1], top[1]),
                    channel_blend(base[2], top[2]),
                    alpha,
                ],
            );
        }
    }
    output
}

fn source_over_alpha(base: u8, top: u8) -> u8 {
    let base = base as f32 / 255.0;
    let top = top as f32 / 255.0;
    ((top + base * (1.0 - top)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn add_channel(base: u8, glow: u8) -> u8 {
    base.saturating_add(glow)
}

fn screen_channel(base: u8, glow: u8) -> u8 {
    let inv = (255_u16 - base as u16) * (255_u16 - glow as u16);
    (255_u16 - ((inv + 127) / 255)).min(255) as u8
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

        assert_eq!(params.based_on, GlowBasedOn::ColorChannels);
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
            &json!({ "0001": { "value": 2 }, "0002": 120, "0003": 0, "0004": 1.0 }),
            0.0,
        );
        let alpha = glow_debug_trace(
            &input,
            &json!({ "0001": { "value": 1 }, "0002": 120, "0003": 0, "0004": 1.0 }),
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
        assert_eq!(color.params.based_on_raw_number, Some(2));
        assert_eq!(alpha.params.based_on_raw_number, Some(1));
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
    fn operation_numeric_values_follow_glow_aex_blend_table() {
        assert_eq!(
            GlowOperation::from_params(&json!({ "0006": { "value": 1 } })),
            GlowOperation::None
        );
        assert_eq!(
            GlowOperation::from_params(&json!({ "0006": { "value": 2 } })),
            GlowOperation::Normal
        );
        assert_eq!(
            GlowOperation::from_params(&json!({ "0006": { "value": 3 } })),
            GlowOperation::Add
        );
        assert_eq!(
            GlowOperation::from_params(&json!({ "0006": { "value": 6 } })),
            GlowOperation::Screen
        );
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
    fn glow_radius_uses_ae_ir_gaussian_radius_scale_before_quantization() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [255, 255, 255, 255]);

        let small = glow_debug_trace(&input, &json!({ "0002": 0, "0003": 0.5, "0004": 1.0 }), 0.0);
        let large = glow_debug_trace(
            &input,
            &json!({ "0002": 0, "0003": 10.0, "0004": 1.0 }),
            0.0,
        );

        assert_eq!(small.params.radius, 0.5);
        assert!((small.params.ir_gaussian_radius - 0.2).abs() < 0.000_001);
        assert_eq!(small.params.kernel_radius, 1);
        assert_eq!(large.params.radius, 10.0);
        assert_eq!(large.params.ir_gaussian_radius, 4.0);
        assert_eq!(large.params.kernel_radius, 8);
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

        assert_eq!(trace.params.based_on, "color_channels");
        assert_eq!(trace.params.based_on_param_source, "default_absent");
        assert_eq!(trace.params.based_on_raw_number, Some(2));
        assert_eq!(trace.params.composite_original, "on_top");
        assert_eq!(trace.params.operation, "add");
    }
}
