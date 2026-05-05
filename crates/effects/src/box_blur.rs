use crate::{param_f32_at, param_f32_at_any, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

const EDGE_POLICY_CLIP_TO_LAYER_BOUNDS: &str = "clip_to_layer_bounds";

#[derive(Debug, Default)]
pub struct BoxBlur2;

impl Effect for BoxBlur2 {
    fn match_name(&self) -> &'static str {
        "ADBE Box Blur2"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = BoxBlurParams::from_json(params, ctx.time);
        Ok(blur_canvas_iterations(
            input,
            blur_radius(params.radius),
            blur_iterations(params.iterations),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoxBlurParams {
    pub radius: f32,
    pub iterations: f32,
}

impl BoxBlurParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            radius: box_blur_radius_param(params, time),
            iterations: box_blur_iterations_param(params, time),
        }
    }
}

fn box_blur_radius_param(params: &Value, time: f64) -> f32 {
    if has_param(params, &["radius", "Radius", "0001"]) {
        return param_f32_at_any(params, &["radius", "Radius", "0001"], time, 0.0);
    }

    if params.get("0002").is_some() && !has_param(params, &["iterations", "Iterations"]) {
        return param_f32_at(params, "0002", time, 0.0);
    }

    0.0
}

fn box_blur_iterations_param(params: &Value, time: f64) -> f32 {
    if has_param(params, &["iterations", "Iterations"]) {
        return param_f32_at_any(params, &["iterations", "Iterations"], time, 1.0);
    }

    if params.get("0002").is_some() && has_param(params, &["radius", "Radius", "0001"]) {
        return param_f32_at(params, "0002", time, 1.0);
    }

    1.0
}

fn has_param(params: &Value, names: &[&str]) -> bool {
    names.iter().any(|name| params.get(*name).is_some())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxBlurDebugParams {
    pub radius: f32,
    pub iterations: f32,
    pub iterations_applied: u32,
    pub kernel_radius: u32,
    pub edge_policy: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasAlphaStats {
    pub total_pixels: u64,
    pub nonzero_pixels: u64,
    pub full_pixels: u64,
    pub alpha_sum: u64,
    pub alpha_min_nonzero: u8,
    pub alpha_max: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxBlurIntermediateHashes {
    pub input_rgba: u64,
    pub horizontal_pass_rgba: u64,
    pub first_iteration_rgba: u64,
    pub output_rgba: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxBlurIntermediateAlphaStats {
    pub input: CanvasAlphaStats,
    pub horizontal_pass: CanvasAlphaStats,
    pub first_iteration: CanvasAlphaStats,
    pub output: CanvasAlphaStats,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxBlurDebugTrace {
    pub params: BoxBlurDebugParams,
    pub hashes: BoxBlurIntermediateHashes,
    pub alpha: BoxBlurIntermediateAlphaStats,
}

pub fn box_blur_debug_trace(input: &Canvas, params: &Value, time: f64) -> BoxBlurDebugTrace {
    let params = BoxBlurParams::from_json(params, time);
    let kernel_radius = blur_radius(params.radius);
    let iterations_applied = blur_iterations(params.iterations);
    let (horizontal, first_iteration) = blur_pass_canvases(input, kernel_radius);
    let output = apply_remaining_iterations(&first_iteration, kernel_radius, iterations_applied);
    BoxBlurDebugTrace {
        params: BoxBlurDebugParams {
            radius: params.radius,
            iterations: params.iterations,
            iterations_applied,
            kernel_radius,
            edge_policy: EDGE_POLICY_CLIP_TO_LAYER_BOUNDS,
        },
        hashes: BoxBlurIntermediateHashes {
            input_rgba: canvas_debug_hash(input),
            horizontal_pass_rgba: canvas_debug_hash(&horizontal),
            first_iteration_rgba: canvas_debug_hash(&first_iteration),
            output_rgba: canvas_debug_hash(&output),
        },
        alpha: BoxBlurIntermediateAlphaStats {
            input: canvas_alpha_stats(input),
            horizontal_pass: canvas_alpha_stats(&horizontal),
            first_iteration: canvas_alpha_stats(&first_iteration),
            output: canvas_alpha_stats(&output),
        },
    }
}

pub(crate) fn blur_canvas(input: &Canvas, radius: u32) -> Canvas {
    blur_pass_canvases(input, radius).1
}

fn blur_canvas_iterations(input: &Canvas, radius: u32, iterations: u32) -> Canvas {
    if iterations <= 1 {
        return blur_canvas(input, radius);
    }

    let first_iteration = blur_canvas(input, radius);
    apply_remaining_iterations(&first_iteration, radius, iterations)
}

fn apply_remaining_iterations(input: &Canvas, radius: u32, iterations: u32) -> Canvas {
    let mut output = input.clone();
    for _ in 1..iterations {
        output = blur_canvas(&output, radius);
    }
    output
}

fn blur_pass_canvases(input: &Canvas, radius: u32) -> (Canvas, Canvas) {
    if radius == 0 || input.width == 0 || input.height == 0 {
        let output = input.clone();
        return (output.clone(), output);
    }

    let mut horizontal = Canvas::transparent(input.width, input.height);
    for y in 0..input.height {
        let mut sum = [0_u32; 4];
        let mut count = 0_u32;
        let right = radius.min(input.width - 1);
        for x in 0..=right {
            add_pixel(&mut sum, input.pixel(x, y));
            count += 1;
        }
        for x in 0..input.width {
            horizontal.set_pixel(x, y, average_pixel(sum, count));
            if x >= radius {
                subtract_pixel(&mut sum, input.pixel(x - radius, y));
                count -= 1;
            }
            let add_x = x + radius + 1;
            if add_x < input.width {
                add_pixel(&mut sum, input.pixel(add_x, y));
                count += 1;
            }
        }
    }

    let mut output = Canvas::transparent(input.width, input.height);
    for x in 0..input.width {
        let mut sum = [0_u32; 4];
        let mut count = 0_u32;
        let bottom = radius.min(input.height - 1);
        for y in 0..=bottom {
            add_pixel(&mut sum, horizontal.pixel(x, y));
            count += 1;
        }
        for y in 0..input.height {
            output.set_pixel(x, y, average_pixel(sum, count));
            if y >= radius {
                subtract_pixel(&mut sum, horizontal.pixel(x, y - radius));
                count -= 1;
            }
            let add_y = y + radius + 1;
            if add_y < input.height {
                add_pixel(&mut sum, horizontal.pixel(x, add_y));
                count += 1;
            }
        }
    }

    (horizontal, output)
}

pub(crate) fn blur_radius(value: f32) -> u32 {
    if value.is_nan() {
        return 0;
    }
    value.ceil().clamp(0.0, 64.0) as u32
}

fn blur_iterations(value: f32) -> u32 {
    value.round().clamp(1.0, 64.0) as u32
}

pub fn canvas_debug_hash(canvas: &Canvas) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in canvas
        .width
        .to_le_bytes()
        .into_iter()
        .chain(canvas.height.to_le_bytes())
        .chain(canvas.data.iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn canvas_alpha_stats(canvas: &Canvas) -> CanvasAlphaStats {
    let mut nonzero_pixels = 0_u64;
    let mut full_pixels = 0_u64;
    let mut alpha_sum = 0_u64;
    let mut alpha_min_nonzero = u8::MAX;
    let mut alpha_max = 0_u8;

    for pixel in canvas.data.chunks_exact(4) {
        let alpha = pixel[3];
        alpha_sum += u64::from(alpha);
        alpha_max = alpha_max.max(alpha);
        if alpha > 0 {
            nonzero_pixels += 1;
            alpha_min_nonzero = alpha_min_nonzero.min(alpha);
        }
        if alpha == 255 {
            full_pixels += 1;
        }
    }

    if nonzero_pixels == 0 {
        alpha_min_nonzero = 0;
    }

    CanvasAlphaStats {
        total_pixels: u64::from(canvas.width) * u64::from(canvas.height),
        nonzero_pixels,
        full_pixels,
        alpha_sum,
        alpha_min_nonzero,
        alpha_max,
    }
}

fn add_pixel(sum: &mut [u32; 4], pixel: [u8; 4]) {
    for channel in 0..4 {
        sum[channel] += u32::from(pixel[channel]);
    }
}

fn subtract_pixel(sum: &mut [u32; 4], pixel: [u8; 4]) {
    for channel in 0..4 {
        sum[channel] -= u32::from(pixel[channel]);
    }
}

fn average_pixel(sum: [u32; 4], count: u32) -> [u8; 4] {
    [
        (sum[0] / count) as u8,
        (sum[1] / count) as u8,
        (sum[2] / count) as u8,
        (sum[3] / count) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_spreads_alpha_to_neighbors() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [255, 255, 255, 255]);

        let output = blur_canvas(&input, 1);

        assert!(output.pixel(0, 0)[3] > 0);
        assert!(output.pixel(1, 0)[3] > 0);
        assert!(output.pixel(2, 0)[3] > 0);
    }

    #[test]
    fn params_accept_direct_named_and_ae_numbered_values() {
        assert_eq!(
            BoxBlurParams::from_json(&serde_json::json!({ "radius": 2.4 }), 0.0).radius,
            2.4
        );
        assert_eq!(
            BoxBlurParams::from_json(&serde_json::json!({ "0001": { "value": 7 } }), 0.0).radius,
            7.0
        );
        assert_eq!(
            BoxBlurParams::from_json(&serde_json::json!({ "0002": 3 }), 0.0).radius,
            3.0
        );
        assert_eq!(
            BoxBlurParams::from_json(&serde_json::json!({ "0002": 3 }), 0.0).iterations,
            1.0
        );
        let ae_numbered = BoxBlurParams::from_json(
            &serde_json::json!({
                "0001": { "value": 7 },
                "0002": { "value": 3 }
            }),
            0.0,
        );
        assert_eq!(ae_numbered.radius, 7.0);
        assert_eq!(ae_numbered.iterations, 3.0);
    }

    #[test]
    fn params_sample_time_varying_radius_and_iterations() {
        let params = serde_json::json!({
            "radius": {
                "keyframes": [
                    { "t": 0.0, "v": 2.0 },
                    { "t": 1.0, "v": 10.0 }
                ]
            },
            "iterations": {
                "keyframes": [
                    { "t": 0.0, "v": 1.0 },
                    { "t": 1.0, "v": 5.0 }
                ]
            }
        });

        let early = BoxBlurParams::from_json(&params, 0.0);
        let mid = BoxBlurParams::from_json(&params, 0.5);
        let late = BoxBlurParams::from_json(&params, 1.0);

        assert_eq!(early.radius, 2.0);
        assert_eq!(mid.radius, 6.0);
        assert_eq!(late.radius, 10.0);
        assert_eq!(early.iterations, 1.0);
        assert_eq!(mid.iterations, 3.0);
        assert_eq!(late.iterations, 5.0);
    }

    #[test]
    fn render_samples_animated_radius_at_context_time() {
        let mut input = Canvas::transparent(5, 1);
        input.set_pixel(2, 0, [255, 255, 255, 255]);
        let params = serde_json::json!({
            "radius": {
                "keyframes": [
                    { "t": 0.0, "v": 0.0 },
                    { "t": 1.0, "v": 2.0 }
                ]
            },
            "iterations": 1
        });

        let early = BoxBlur2::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &params,
            )
            .unwrap();
        let late = BoxBlur2::default()
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
    fn radius_quantization_uses_gpu_foundation_ceil_semantics() {
        assert_eq!(blur_radius(-0.25), 0);
        assert_eq!(blur_radius(0.0), 0);
        assert_eq!(blur_radius(0.01), 1);
        assert_eq!(blur_radius(0.5), 1);
        assert_eq!(blur_radius(0.99), 1);
        assert_eq!(blur_radius(1.0), 1);
        assert_eq!(blur_radius(1.01), 2);
        assert_eq!(blur_radius(1.5), 2);
        assert_eq!(blur_radius(64.1), 64);
    }

    #[test]
    fn edge_pixels_use_clipped_sample_windows() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(0, 0, [90, 90, 90, 90]);
        input.set_pixel(1, 0, [180, 180, 180, 180]);

        let output = blur_canvas(&input, 1);

        assert_eq!(output.pixel(0, 0), [135, 135, 135, 135]);
        assert_eq!(output.pixel(1, 0), [90, 90, 90, 90]);
        assert_eq!(output.pixel(2, 0), [90, 90, 90, 90]);
    }

    #[test]
    fn render_applies_iterations_after_first_pass() {
        let mut input = Canvas::transparent(5, 1);
        input.set_pixel(2, 0, [255, 255, 255, 255]);
        let ctx = EffectContext {
            time: 0.0,
            fps: 30.0,
        };

        let one_iteration = BoxBlur2::default()
            .render(
                &input,
                &ctx,
                &serde_json::json!({ "radius": 1, "iterations": 1 }),
            )
            .unwrap();
        let two_iterations = BoxBlur2::default()
            .render(
                &input,
                &ctx,
                &serde_json::json!({ "radius": 1, "iterations": 2 }),
            )
            .unwrap();

        assert_eq!(one_iteration.pixel(0, 0)[3], 0);
        assert!(two_iterations.pixel(0, 0)[3] > 0);
        assert_ne!(
            canvas_debug_hash(&one_iteration),
            canvas_debug_hash(&two_iterations)
        );
    }

    #[test]
    fn debug_trace_reports_iteration_application_and_edge_policy() {
        let mut input = Canvas::transparent(5, 1);
        input.set_pixel(2, 0, [255, 255, 255, 255]);

        let trace = box_blur_debug_trace(
            &input,
            &serde_json::json!({ "radius": 1, "iterations": 2.4 }),
            0.0,
        );
        let first_iteration = blur_canvas(&input, 1);
        let final_output = blur_canvas_iterations(&input, 1, 2);

        assert_eq!(trace.params.iterations, 2.4);
        assert_eq!(trace.params.iterations_applied, 2);
        assert_eq!(trace.params.edge_policy, EDGE_POLICY_CLIP_TO_LAYER_BOUNDS);
        assert_eq!(
            trace.hashes.first_iteration_rgba,
            canvas_debug_hash(&first_iteration)
        );
        assert_eq!(trace.hashes.output_rgba, canvas_debug_hash(&final_output));
        assert_eq!(trace.alpha.input.nonzero_pixels, 1);
        assert!(trace.alpha.output.nonzero_pixels > trace.alpha.input.nonzero_pixels);
    }

    #[test]
    fn iterations_clamp_to_at_least_one_pass() {
        assert_eq!(blur_iterations(-3.0), 1);
        assert_eq!(blur_iterations(0.49), 1);
        assert_eq!(blur_iterations(1.5), 2);
    }

    #[test]
    fn alpha_stats_count_coverage_and_extrema() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(0, 0, [10, 20, 30, 64]);
        input.set_pixel(1, 0, [10, 20, 30, 255]);

        let stats = canvas_alpha_stats(&input);

        assert_eq!(stats.total_pixels, 3);
        assert_eq!(stats.nonzero_pixels, 2);
        assert_eq!(stats.full_pixels, 1);
        assert_eq!(stats.alpha_sum, 319);
        assert_eq!(stats.alpha_min_nonzero, 64);
        assert_eq!(stats.alpha_max, 255);
    }
}
