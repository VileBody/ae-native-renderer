use crate::{param_f32_at, param_f32_at_any, Effect, EffectContext};
use raster_cpu::Canvas;
use rayon::prelude::*;
use serde_json::Value;

const EDGE_POLICY_CLIP_TO_LAYER_BOUNDS: &str = "clip_to_layer_bounds";
// Full-frame Rayon passes regress memory-bound 1080p renders on Apple Silicon.
// Keep UHD and larger canvases eligible while leaving production 1080p serial.
const PARALLEL_BLUR_PIXEL_THRESHOLD: usize = 8_000_000;

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

    blur_pass_canvases_with_mode(
        input,
        radius,
        input.width as usize * input.height as usize >= PARALLEL_BLUR_PIXEL_THRESHOLD,
    )
}

fn blur_pass_canvases_with_mode(input: &Canvas, radius: u32, parallel: bool) -> (Canvas, Canvas) {
    let horizontal = horizontal_blur_pass(input, radius, parallel);
    let output = vertical_blur_pass(&horizontal, radius, parallel);
    (horizontal, output)
}

fn horizontal_blur_pass(input: &Canvas, radius: u32, parallel: bool) -> Canvas {
    let mut horizontal = Canvas::transparent(input.width, input.height);
    let row_bytes = input.width as usize * 4;
    if parallel {
        horizontal
            .data
            .par_chunks_mut(row_bytes)
            .zip(input.data.par_chunks(row_bytes))
            .for_each(|(output_row, input_row)| {
                fill_horizontal_blur_row(output_row, input_row, input.width, radius)
            });
    } else {
        for (output_row, input_row) in horizontal
            .data
            .chunks_mut(row_bytes)
            .zip(input.data.chunks(row_bytes))
        {
            fill_horizontal_blur_row(output_row, input_row, input.width, radius);
        }
    }
    horizontal
}

fn fill_horizontal_blur_row(output: &mut [u8], input: &[u8], width: u32, radius: u32) {
    let mut sum = PremultipliedSum::default();
    let mut count = 0_u32;
    let right = radius.min(width - 1);
    for x in 0..=right {
        add_pixel(&mut sum, row_pixel(input, x));
        count += 1;
    }
    for x in 0..width {
        let offset = x as usize * 4;
        output[offset..offset + 4].copy_from_slice(&average_pixel(sum, count));
        if x >= radius {
            subtract_pixel(&mut sum, row_pixel(input, x - radius));
            count -= 1;
        }
        let add_x = x + radius + 1;
        if add_x < width {
            add_pixel(&mut sum, row_pixel(input, add_x));
            count += 1;
        }
    }
}

fn vertical_blur_pass(input: &Canvas, radius: u32, parallel: bool) -> Canvas {
    let mut output = Canvas::transparent(input.width, input.height);
    if !parallel {
        for x in 0..input.width {
            fill_vertical_blur_column(&mut output, input, x, radius);
        }
        return output;
    }

    let columns = (0..input.width)
        .into_par_iter()
        .map(|x| blurred_vertical_column(input, x, radius))
        .collect::<Vec<_>>();
    let row_bytes = input.width as usize * 4;
    output
        .data
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, column) in columns.iter().enumerate() {
                row[x * 4..x * 4 + 4].copy_from_slice(&column[y * 4..y * 4 + 4]);
            }
        });
    output
}

fn fill_vertical_blur_column(output: &mut Canvas, input: &Canvas, x: u32, radius: u32) {
    let mut sum = PremultipliedSum::default();
    let mut count = 0_u32;
    let bottom = radius.min(input.height - 1);
    for y in 0..=bottom {
        add_pixel(&mut sum, input.pixel(x, y));
        count += 1;
    }
    for y in 0..input.height {
        output.set_pixel(x, y, average_pixel(sum, count));
        if y >= radius {
            subtract_pixel(&mut sum, input.pixel(x, y - radius));
            count -= 1;
        }
        let add_y = y + radius + 1;
        if add_y < input.height {
            add_pixel(&mut sum, input.pixel(x, add_y));
            count += 1;
        }
    }
}

fn blurred_vertical_column(input: &Canvas, x: u32, radius: u32) -> Vec<u8> {
    let mut column = vec![0u8; input.height as usize * 4];
    let mut sum = PremultipliedSum::default();
    let mut count = 0_u32;
    let bottom = radius.min(input.height - 1);
    for y in 0..=bottom {
        add_pixel(&mut sum, input.pixel(x, y));
        count += 1;
    }
    for y in 0..input.height {
        let offset = y as usize * 4;
        column[offset..offset + 4].copy_from_slice(&average_pixel(sum, count));
        if y >= radius {
            subtract_pixel(&mut sum, input.pixel(x, y - radius));
            count -= 1;
        }
        let add_y = y + radius + 1;
        if add_y < input.height {
            add_pixel(&mut sum, input.pixel(x, add_y));
            count += 1;
        }
    }
    column
}

fn row_pixel(row: &[u8], x: u32) -> [u8; 4] {
    let offset = x as usize * 4;
    [
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ]
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

#[derive(Clone, Copy, Default)]
struct PremultipliedSum {
    rgb: [u64; 3],
    alpha: u64,
}

fn add_pixel(sum: &mut PremultipliedSum, pixel: [u8; 4]) {
    let alpha = u64::from(pixel[3]);
    for channel in 0..3 {
        sum.rgb[channel] += u64::from(pixel[channel]) * alpha;
    }
    sum.alpha += alpha;
}

fn subtract_pixel(sum: &mut PremultipliedSum, pixel: [u8; 4]) {
    let alpha = u64::from(pixel[3]);
    for channel in 0..3 {
        sum.rgb[channel] -= u64::from(pixel[channel]) * alpha;
    }
    sum.alpha -= alpha;
}

fn average_pixel(sum: PremultipliedSum, count: u32) -> [u8; 4] {
    let alpha = sum.alpha / u64::from(count);
    let mut output = [0_u8; 4];
    if sum.alpha > 0 {
        for channel in 0..3 {
            output[channel] = (sum.rgb[channel] / sum.alpha).min(255) as u8;
        }
    }
    output[3] = alpha.min(255) as u8;
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::ThreadPoolBuilder;

    #[test]
    fn automatic_parallelism_starts_above_full_hd() {
        assert!(1920usize * 1080 < PARALLEL_BLUR_PIXEL_THRESHOLD);
        assert!(3840usize * 2160 >= PARALLEL_BLUR_PIXEL_THRESHOLD);
    }

    fn patterned_canvas(width: u32, height: u32) -> Canvas {
        let mut canvas = Canvas::transparent(width, height);
        for (index, pixel) in canvas.data.chunks_mut(4).enumerate() {
            let value = index as u32;
            pixel.copy_from_slice(&[
                value.wrapping_mul(17) as u8,
                value.wrapping_mul(29).wrapping_add(3) as u8,
                value.wrapping_mul(43).wrapping_add(11) as u8,
                value.wrapping_mul(61).wrapping_add(19) as u8,
            ]);
        }
        canvas
    }

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
    fn blur_uses_premultiplied_rgb_for_transparent_neighbors() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(0, 0, [255, 0, 0, 0]);
        input.set_pixel(1, 0, [0, 255, 0, 255]);

        let output = blur_canvas(&input, 1);

        assert_eq!(output.pixel(0, 0), [0, 255, 0, 127]);
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

        assert_eq!(output.pixel(0, 0), [150, 150, 150, 135]);
        assert_eq!(output.pixel(1, 0), [150, 150, 150, 90]);
        assert_eq!(output.pixel(2, 0), [180, 180, 180, 90]);
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

    #[test]
    fn parallel_passes_are_byte_exact_against_sequential_and_thread_count() {
        let input = patterned_canvas(257, 131);
        let one_thread = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let four_threads = ThreadPoolBuilder::new().num_threads(4).build().unwrap();

        for radius in [1, 17, 64] {
            let expected = blur_pass_canvases_with_mode(&input, radius, false);
            let actual_one =
                one_thread.install(|| blur_pass_canvases_with_mode(&input, radius, true));
            let actual_four =
                four_threads.install(|| blur_pass_canvases_with_mode(&input, radius, true));

            assert_eq!(
                actual_one.0.data, expected.0.data,
                "horizontal radius {radius}"
            );
            assert_eq!(
                actual_one.1.data, expected.1.data,
                "vertical radius {radius}"
            );
            assert_eq!(
                actual_four.0.data, expected.0.data,
                "horizontal radius {radius}"
            );
            assert_eq!(
                actual_four.1.data, expected.1.data,
                "vertical radius {radius}"
            );
            assert_eq!(
                actual_one.1.data, actual_four.1.data,
                "thread-count radius {radius}"
            );
        }
    }
}
