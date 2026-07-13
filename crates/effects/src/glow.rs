use crate::{
    box_blur::{canvas_alpha_stats, canvas_debug_hash, CanvasAlphaStats},
    param_f32_at_any, param_value, Effect, EffectContext,
};
use raster_cpu::{composite_normal, composite_normal_pixel, Canvas};
use rayon::prelude::*;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct Glow;

pub(crate) const AE_IR_GAUSSIAN_RADIUS_SCALE: f32 = 0.4;
const IR_GAUSSIAN_MIN_RADIUS: f32 = 0.1;
const IR_GAUSSIAN_THETA: f64 = 0.844_799_995_422_363_3;
const IR_GAUSSIAN_EXP1: f64 = -1.259_999_990_463_256_8;
const IR_GAUSSIAN_EXP2: f64 = -2.519_999_980_926_513_7;
const IR_GAUSSIAN_K0: f64 = 0.962_899_982_929_229_7;
const IR_GAUSSIAN_K1: f64 = 1.942_000_031_471_252_4;
const IR_GAUSSIAN_TRANSPOSE_TILE: usize = 32;

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

        let source = glow_source_float(input, threshold, params.based_on);
        let blurred = ir_recursive_gaussian_blur_float_buffer(
            input.width,
            input.height,
            source,
            glow_ir_gaussian_radius(params.radius),
            IrGaussianBlurOptions {
                horizontal: true,
                vertical: true,
                repeat_edge_pixels: false,
            },
        );

        Ok(composite_scaled_float_glow(
            input,
            &blurred,
            intensity,
            params.operation,
            params.composite_original,
        ))
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
    radius * AE_IR_GAUSSIAN_RADIUS_SCALE
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
    ir_gaussian_blur_canvas(
        input,
        sigma,
        IrGaussianBlurOptions {
            horizontal: true,
            vertical: true,
            repeat_edge_pixels: false,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IrGaussianBlurOptions {
    pub horizontal: bool,
    pub vertical: bool,
    pub repeat_edge_pixels: bool,
}

pub(crate) fn ir_gaussian_blur_canvas(
    input: &Canvas,
    sigma: f32,
    options: IrGaussianBlurOptions,
) -> Canvas {
    if sigma.is_nan() || sigma <= 0.0 || input.width == 0 || input.height == 0 {
        return input.clone();
    }
    if !options.horizontal && !options.vertical {
        return input.clone();
    }

    let filtered = ir_recursive_gaussian_blur_float_buffer(
        input.width,
        input.height,
        float_canvas_from_canvas(input),
        sigma,
        options,
    );
    float_canvas_to_canvas(input.width, input.height, &filtered)
}

fn ir_recursive_gaussian_blur_float_buffer(
    width: u32,
    height: u32,
    mut filtered: Vec<[f32; 4]>,
    sigma: f32,
    options: IrGaussianBlurOptions,
) -> Vec<[f32; 4]> {
    if sigma.is_nan()
        || sigma <= 0.0
        || width == 0
        || height == 0
        || (!options.horizontal && !options.vertical)
    {
        return filtered;
    }

    let coeffs = ir_recursive_gaussian_coefficients(sigma.max(IR_GAUSSIAN_MIN_RADIUS));
    let mut scratch = vec![[0.0_f32; 4]; filtered.len()];
    if options.horizontal {
        ir_recursive_gaussian_pass_horizontal_float_into(
            width,
            height,
            &filtered,
            &mut scratch,
            coeffs,
            options.repeat_edge_pixels,
        );
        std::mem::swap(&mut filtered, &mut scratch);
    }
    if options.vertical {
        ir_recursive_gaussian_pass_vertical_float_into(
            width,
            height,
            &filtered,
            &mut scratch,
            coeffs,
            options.repeat_edge_pixels,
        );
        std::mem::swap(&mut filtered, &mut scratch);
    }
    filtered
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

#[cfg(test)]
fn ir_recursive_gaussian_pass_horizontal_float(
    width: u32,
    height: u32,
    input: &[[f32; 4]],
    coeffs: IrRecursiveGaussianCoefficients,
    repeat_edge_pixels: bool,
) -> Vec<[f32; 4]> {
    let mut output = vec![[0.0_f32; 4]; (width * height) as usize];
    ir_recursive_gaussian_pass_horizontal_float_into(
        width,
        height,
        input,
        &mut output,
        coeffs,
        repeat_edge_pixels,
    );
    output
}

fn ir_recursive_gaussian_pass_horizontal_float_into(
    width: u32,
    height: u32,
    input: &[[f32; 4]],
    output: &mut [[f32; 4]],
    coeffs: IrRecursiveGaussianCoefficients,
    repeat_edge_pixels: bool,
) {
    let width = width as usize;
    debug_assert_eq!(input.len(), width * height as usize);
    debug_assert_eq!(output.len(), input.len());
    input
        .par_chunks_exact(width)
        .zip(output.par_chunks_exact_mut(width))
        .for_each(|(source, destination)| {
            ir_recursive_gaussian_filter_line_into(source, destination, coeffs, repeat_edge_pixels);
        });
}

#[cfg(test)]
fn ir_recursive_gaussian_pass_vertical_float(
    width: u32,
    height: u32,
    input: &[[f32; 4]],
    coeffs: IrRecursiveGaussianCoefficients,
    repeat_edge_pixels: bool,
) -> Vec<[f32; 4]> {
    let mut output = vec![[0.0_f32; 4]; (width * height) as usize];
    ir_recursive_gaussian_pass_vertical_float_into(
        width,
        height,
        input,
        &mut output,
        coeffs,
        repeat_edge_pixels,
    );
    output
}

fn ir_recursive_gaussian_pass_vertical_float_into(
    width: u32,
    height: u32,
    input: &[[f32; 4]],
    output: &mut [[f32; 4]],
    coeffs: IrRecursiveGaussianCoefficients,
    repeat_edge_pixels: bool,
) {
    let width = width as usize;
    let height = height as usize;
    debug_assert_eq!(input.len(), width * height);
    debug_assert_eq!(output.len(), input.len());

    // Transpose once so each recurrent column becomes a cache-local row. The
    // passed output is scratch until the final transpose restores row-major order.
    transpose_float_buffer_blocked_into(input, width, height, output);
    let mut filtered_transposed = vec![[0.0_f32; 4]; input.len()];
    output
        .par_chunks_exact(height)
        .zip(filtered_transposed.par_chunks_exact_mut(height))
        .for_each(|(source_column, destination_column)| {
            ir_recursive_gaussian_filter_line_into(
                source_column,
                destination_column,
                coeffs,
                repeat_edge_pixels,
            );
        });
    transpose_float_buffer_blocked_into(&filtered_transposed, height, width, output);
}

fn transpose_float_buffer_blocked_into(
    input: &[[f32; 4]],
    source_width: usize,
    source_height: usize,
    output: &mut [[f32; 4]],
) {
    debug_assert!(source_width > 0);
    debug_assert!(source_height > 0);
    debug_assert_eq!(input.len(), source_width * source_height);
    debug_assert_eq!(output.len(), input.len());

    let destination_block_len = source_height * IR_GAUSSIAN_TRANSPOSE_TILE;
    output
        .par_chunks_mut(destination_block_len)
        .enumerate()
        .for_each(|(block_index, destination_rows)| {
            let source_x = block_index * IR_GAUSSIAN_TRANSPOSE_TILE;
            let block_width = (source_width - source_x).min(IR_GAUSSIAN_TRANSPOSE_TILE);
            debug_assert_eq!(destination_rows.len(), block_width * source_height);

            for source_y in (0..source_height).step_by(IR_GAUSSIAN_TRANSPOSE_TILE) {
                let block_height = (source_height - source_y).min(IR_GAUSSIAN_TRANSPOSE_TILE);
                for local_y in 0..block_height {
                    let y = source_y + local_y;
                    let source_start = y * source_width + source_x;
                    let source_row = &input[source_start..source_start + block_width];
                    for (local_x, &pixel) in source_row.iter().enumerate() {
                        destination_rows[local_x * source_height + y] = pixel;
                    }
                }
            }
        });
}

fn float_canvas_from_canvas(input: &Canvas) -> Vec<[f32; 4]> {
    let mut output = Vec::with_capacity((input.width * input.height) as usize);
    for chunk in input.data.chunks_exact(4) {
        output.push([
            chunk[0] as f32,
            chunk[1] as f32,
            chunk[2] as f32,
            chunk[3] as f32,
        ]);
    }
    output
}

fn float_canvas_to_canvas(width: u32, height: u32, input: &[[f32; 4]]) -> Canvas {
    let mut data = Vec::with_capacity(input.len() * 4);
    for &pixel in input {
        data.extend_from_slice(&float_pixel_to_u8(pixel));
    }
    Canvas {
        width,
        height,
        data,
    }
}

fn ir_recursive_gaussian_filter_line_into(
    source: &[[f32; 4]],
    output: &mut [[f32; 4]],
    coeffs: IrRecursiveGaussianCoefficients,
    repeat_edge_pixels: bool,
) {
    debug_assert_eq!(source.len(), output.len());
    let Some(&first) = source.first() else {
        return;
    };
    let len = source.len();
    let (mut prev_source, mut prev, mut prev2) = if repeat_edge_pixels {
        let state = recursive_boundary_state(
            first,
            coeffs.causal_current + coeffs.causal_previous_source,
            coeffs,
        );
        (first, state, state)
    } else {
        ([0.0; 4], [0.0; 4], [0.0; 4])
    };
    for (index, pixel) in source.iter().enumerate() {
        let next = recursive_gaussian_causal(*pixel, prev_source, prev, prev2, coeffs);
        output[index] = next;
        prev_source = *pixel;
        prev2 = prev;
        prev = next;
    }

    let repeated_right_source = if repeat_edge_pixels {
        source[len - 1]
    } else {
        [0.0; 4]
    };
    let reverse_state = if repeat_edge_pixels {
        recursive_boundary_state(
            repeated_right_source,
            coeffs.anticausal_next_source + coeffs.anticausal_next2_source,
            coeffs,
        )
    } else {
        [0.0; 4]
    };
    let mut reverse_next = reverse_state;
    let mut reverse_next2 = reverse_state;
    for index in (0..len).rev() {
        let source_next = source
            .get(index + 1)
            .copied()
            .unwrap_or(repeated_right_source);
        let source_next2 = source
            .get(index + 2)
            .copied()
            .unwrap_or(repeated_right_source);
        let reverse = recursive_gaussian_anticausal(
            source_next,
            source_next2,
            reverse_next,
            reverse_next2,
            coeffs,
        );
        for channel in 0..4 {
            output[index][channel] += reverse[channel];
        }
        reverse_next2 = reverse_next;
        reverse_next = reverse;
    }
}

fn recursive_gaussian_causal(
    pixel: [f32; 4],
    previous_source: [f32; 4],
    previous: [f32; 4],
    previous2: [f32; 4],
    coeffs: IrRecursiveGaussianCoefficients,
) -> [f32; 4] {
    let mut output = [0.0_f32; 4];
    for channel in 0..4 {
        output[channel] = pixel[channel] * coeffs.causal_current
            + previous_source[channel] * coeffs.causal_previous_source
            - previous[channel] * coeffs.feedback_previous
            - previous2[channel] * coeffs.feedback_previous2;
    }
    output
}

fn recursive_gaussian_anticausal(
    source_next: [f32; 4],
    source_next2: [f32; 4],
    reverse_next: [f32; 4],
    reverse_next2: [f32; 4],
    coeffs: IrRecursiveGaussianCoefficients,
) -> [f32; 4] {
    let mut output = [0.0_f32; 4];
    for channel in 0..4 {
        output[channel] = source_next[channel] * coeffs.anticausal_next_source
            + source_next2[channel] * coeffs.anticausal_next2_source
            - reverse_next[channel] * coeffs.feedback_previous
            - reverse_next2[channel] * coeffs.feedback_previous2;
    }
    output
}

fn recursive_boundary_state(
    source: [f32; 4],
    source_gain: f32,
    coeffs: IrRecursiveGaussianCoefficients,
) -> [f32; 4] {
    let denominator = 1.0 + coeffs.feedback_previous + coeffs.feedback_previous2;
    let scale = if denominator.abs() > f32::EPSILON {
        source_gain / denominator
    } else {
        0.0
    };
    source.map(|channel| channel * scale)
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
    let mut data = Vec::with_capacity(input.data.len());
    for chunk in input.data.chunks_exact(4) {
        let pixel = [chunk[0], chunk[1], chunk[2], chunk[3]];
        if glow_pixel_passes_threshold(pixel, threshold, based_on) {
            data.extend_from_slice(&pixel);
        } else {
            data.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    Canvas {
        width: input.width,
        height: input.height,
        data,
    }
}

fn glow_source_float(input: &Canvas, threshold: f32, based_on: GlowBasedOn) -> Vec<[f32; 4]> {
    let mut source = Vec::with_capacity((input.width * input.height) as usize);
    for chunk in input.data.chunks_exact(4) {
        let pixel = [chunk[0], chunk[1], chunk[2], chunk[3]];
        if glow_pixel_passes_threshold(pixel, threshold, based_on) {
            source.push([
                pixel[0] as f32,
                pixel[1] as f32,
                pixel[2] as f32,
                pixel[3] as f32,
            ]);
        } else {
            source.push([0.0; 4]);
        }
    }
    source
}

fn glow_pixel_passes_threshold(pixel: [u8; 4], threshold: f32, based_on: GlowBasedOn) -> bool {
    if pixel[3] == 0 {
        return false;
    }
    let luminance = 0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
    match based_on {
        GlowBasedOn::Combined => luminance >= threshold || pixel[3] as f32 >= threshold,
        GlowBasedOn::ColorChannels => luminance >= threshold,
        GlowBasedOn::AlphaChannel => pixel[3] as f32 >= threshold,
    }
}

fn scale_canvas(canvas: &mut Canvas, intensity: f32) {
    for chunk in canvas.data.chunks_exact_mut(4) {
        let scaled = scale_glow_pixel([chunk[0], chunk[1], chunk[2], chunk[3]], intensity);
        chunk.copy_from_slice(&scaled);
    }
}

fn scale_glow_pixel(mut pixel: [u8; 4], intensity: f32) -> [u8; 4] {
    let alpha_scale = intensity.clamp(0.0, 8.0);
    pixel[0] = (pixel[0] as f32 * intensity).round().clamp(0.0, 255.0) as u8;
    pixel[1] = (pixel[1] as f32 * intensity).round().clamp(0.0, 255.0) as u8;
    pixel[2] = (pixel[2] as f32 * intensity).round().clamp(0.0, 255.0) as u8;
    pixel[3] = (pixel[3] as f32 * alpha_scale).round().clamp(0.0, 255.0) as u8;
    pixel
}

fn composite_scaled_float_glow(
    input: &Canvas,
    blurred: &[[f32; 4]],
    intensity: f32,
    operation: GlowOperation,
    composite_original: GlowCompositeOriginal,
) -> Canvas {
    debug_assert_eq!(blurred.len() * 4, input.data.len());
    let mut data = Vec::with_capacity(input.data.len());
    for (base, &blurred_pixel) in input.data.chunks_exact(4).zip(blurred) {
        let base = [base[0], base[1], base[2], base[3]];
        let glow = scale_glow_pixel(float_pixel_to_u8(blurred_pixel), intensity);
        let operated = match operation {
            GlowOperation::None => glow,
            GlowOperation::Normal => composite_normal_pixel(base, glow, 100.0),
            GlowOperation::Add => blend_glow_pixel(base, glow, add_channel),
            GlowOperation::Screen => blend_glow_pixel(base, glow, screen_channel),
        };
        let output = match composite_original {
            GlowCompositeOriginal::None | GlowCompositeOriginal::OnTop => operated,
            GlowCompositeOriginal::Behind => composite_normal_pixel(operated, base, 100.0),
        };
        data.extend_from_slice(&output);
    }
    Canvas {
        width: input.width,
        height: input.height,
        data,
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
    let mut data = Vec::with_capacity(input.data.len());
    for (base, top) in input.data.chunks_exact(4).zip(glow.data.chunks_exact(4)) {
        let base = [base[0], base[1], base[2], base[3]];
        let top = [top[0], top[1], top[2], top[3]];
        data.extend_from_slice(&blend_glow_pixel(base, top, channel_blend));
    }
    Canvas {
        width: input.width,
        height: input.height,
        data,
    }
}

fn blend_glow_pixel(base: [u8; 4], glow: [u8; 4], channel_blend: fn(u8, u8) -> u8) -> [u8; 4] {
    [
        channel_blend(base[0], glow[0]),
        channel_blend(base[1], glow[1]),
        channel_blend(base[2], glow[2]),
        source_over_alpha(base[3], glow[3]),
    ]
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
    use rayon::ThreadPoolBuilder;
    use serde_json::json;

    fn patterned_canvas(width: u32, height: u32) -> Canvas {
        let mut input = Canvas::transparent(width, height);
        for y in 0..height {
            for x in 0..width {
                input.set_pixel(
                    x,
                    y,
                    [
                        (x.wrapping_mul(29) + y.wrapping_mul(13)) as u8,
                        (x.wrapping_mul(7) + y.wrapping_mul(37)) as u8,
                        (x.wrapping_mul(41) + y.wrapping_mul(19)) as u8,
                        (x.wrapping_mul(17) + y.wrapping_mul(23) + 31) as u8,
                    ],
                );
            }
        }
        input
    }

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
    fn recursive_gaussian_quantizes_once_after_both_axes() {
        let mut input = Canvas::transparent(3, 3);
        input.set_pixel(1, 1, [191, 113, 47, 173]);

        let sigma = 0.8;
        let coeffs = ir_recursive_gaussian_coefficients(sigma);
        let source = float_canvas_from_canvas(&input);
        let horizontal = ir_recursive_gaussian_pass_horizontal_float(
            input.width,
            input.height,
            &source,
            coeffs,
            false,
        );
        let final_float = ir_recursive_gaussian_pass_vertical_float(
            input.width,
            input.height,
            &horizontal,
            coeffs,
            false,
        );
        let final_once = float_canvas_to_canvas(input.width, input.height, &final_float);

        let mut quantized_horizontal = Canvas::transparent(input.width, input.height);
        for y in 0..input.height {
            for x in 0..input.width {
                quantized_horizontal.set_pixel(
                    x,
                    y,
                    float_pixel_to_u8(horizontal[(y * input.width + x) as usize]),
                );
            }
        }
        let staged_source = float_canvas_from_canvas(&quantized_horizontal);
        let staged_float = ir_recursive_gaussian_pass_vertical_float(
            input.width,
            input.height,
            &staged_source,
            coeffs,
            false,
        );
        let staged_u8 = float_canvas_to_canvas(input.width, input.height, &staged_float);

        assert_eq!(
            canvas_debug_hash(&glow_blur_canvas(&input, sigma)),
            canvas_debug_hash(&final_once)
        );
        assert_ne!(
            canvas_debug_hash(&final_once),
            canvas_debug_hash(&staged_u8)
        );
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

    #[test]
    fn fused_threshold_intensity_and_composite_matches_reference_pixels() {
        let mut input = Canvas::transparent(7, 5);
        for y in 0..input.height {
            for x in 0..input.width {
                input.set_pixel(
                    x,
                    y,
                    [
                        (x * 31 + y * 17) as u8,
                        (x * 11 + y * 43) as u8,
                        (x * 53 + y * 7) as u8,
                        ((x + y * 2) * 23) as u8,
                    ],
                );
            }
        }

        for operation in [
            GlowOperation::None,
            GlowOperation::Normal,
            GlowOperation::Add,
            GlowOperation::Screen,
        ] {
            for composite_original in [
                GlowCompositeOriginal::None,
                GlowCompositeOriginal::Behind,
                GlowCompositeOriginal::OnTop,
            ] {
                let source = glow_source(&input, 73.0, GlowBasedOn::Combined);
                let mut glow = glow_blur_canvas(&source, glow_ir_gaussian_radius(9.5));
                scale_canvas(&mut glow, 1.37);
                let reference = composite_glow(&input, &glow, operation, composite_original);

                let blurred = ir_recursive_gaussian_blur_float_buffer(
                    input.width,
                    input.height,
                    glow_source_float(&input, 73.0, GlowBasedOn::Combined),
                    glow_ir_gaussian_radius(9.5),
                    IrGaussianBlurOptions {
                        horizontal: true,
                        vertical: true,
                        repeat_edge_pixels: false,
                    },
                );
                let fused = composite_scaled_float_glow(
                    &input,
                    &blurred,
                    1.37,
                    operation,
                    composite_original,
                );

                assert_eq!(
                    fused.data, reference.data,
                    "operation={operation:?}, composite_original={composite_original:?}"
                );
            }
        }
    }

    #[test]
    fn recursive_gaussian_is_bit_exact_with_one_or_many_threads() {
        let input = patterned_canvas(137, 91);
        let source = float_canvas_from_canvas(&input);
        let single_thread = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let four_threads = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
        let options = [
            IrGaussianBlurOptions {
                horizontal: true,
                vertical: false,
                repeat_edge_pixels: false,
            },
            IrGaussianBlurOptions {
                horizontal: false,
                vertical: true,
                repeat_edge_pixels: false,
            },
            IrGaussianBlurOptions {
                horizontal: true,
                vertical: true,
                repeat_edge_pixels: false,
            },
            IrGaussianBlurOptions {
                horizontal: true,
                vertical: true,
                repeat_edge_pixels: true,
            },
        ];

        for sigma in [0.4, 2.75, 17.0] {
            for options in options {
                let expected = single_thread.install(|| {
                    ir_recursive_gaussian_blur_float_buffer(
                        input.width,
                        input.height,
                        source.clone(),
                        sigma,
                        options,
                    )
                });
                for attempt in 0..3 {
                    let actual = four_threads.install(|| {
                        ir_recursive_gaussian_blur_float_buffer(
                            input.width,
                            input.height,
                            source.clone(),
                            sigma,
                            options,
                        )
                    });
                    assert_eq!(
                        actual, expected,
                        "sigma={sigma}, options={options:?}, attempt={attempt}"
                    );
                }
            }
        }
    }

    #[test]
    fn two_glow_stack_is_byte_exact_with_one_or_many_threads() {
        let input = patterned_canvas(127, 83);
        let context = EffectContext {
            time: 0.0,
            fps: 30.0,
        };
        let tight = json!({
            "based_on": "color channels",
            "threshold": 92,
            "radius": 3.5,
            "intensity": 0.85,
            "operation": "screen",
            "composite_original": "on top"
        });
        let broad = json!({
            "based_on": "combined",
            "threshold": 126,
            "radius": 11.0,
            "intensity": 1.45,
            "operation": "add",
            "composite_original": "on top"
        });
        let render_stack = || {
            let tight_output = Glow::default().render(&input, &context, &tight).unwrap();
            Glow::default()
                .render(&tight_output, &context, &broad)
                .unwrap()
        };
        let single_thread = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let four_threads = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
        let expected = single_thread.install(render_stack);

        for attempt in 0..4 {
            let actual = four_threads.install(render_stack);
            assert_eq!(
                actual.data, expected.data,
                "two-Glow output changed on attempt {attempt}"
            );
        }
    }

    #[test]
    fn two_glow_stack_is_order_sensitive_and_pixel_stable() {
        let mut input = Canvas::transparent(9, 7);
        for y in 1..6 {
            for x in 1..8 {
                let alpha = if (x + y) % 3 == 0 { 96 } else { 224 };
                input.set_pixel(
                    x,
                    y,
                    [
                        (x * 29 + y * 13) as u8,
                        (x * 7 + y * 37) as u8,
                        (x * 41 + y * 19) as u8,
                        alpha,
                    ],
                );
            }
        }
        let context = EffectContext {
            time: 0.0,
            fps: 30.0,
        };
        let tight = json!({
            "based_on": "color channels",
            "threshold": 92,
            "radius": 3.5,
            "intensity": 0.85,
            "operation": "screen",
            "composite_original": "on top"
        });
        let broad = json!({
            "based_on": "combined",
            "threshold": 126,
            "radius": 11.0,
            "intensity": 1.45,
            "operation": "add",
            "composite_original": "on top"
        });

        let tight_then_broad = Glow
            .render(
                &Glow.render(&input, &context, &tight).unwrap(),
                &context,
                &broad,
            )
            .unwrap();
        let broad_then_tight = Glow
            .render(
                &Glow.render(&input, &context, &broad).unwrap(),
                &context,
                &tight,
            )
            .unwrap();

        assert_ne!(tight_then_broad.data, broad_then_tight.data);
        assert_eq!(
            canvas_debug_hash(&tight_then_broad),
            9_524_823_262_784_283_956
        );
        assert_eq!(
            canvas_debug_hash(&broad_then_tight),
            735_625_038_440_722_515
        );
    }
}
