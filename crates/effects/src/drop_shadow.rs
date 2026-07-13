use crate::{
    box_blur::{blur_canvas, blur_radius, canvas_alpha_stats, canvas_debug_hash, CanvasAlphaStats},
    param_bool_any, param_f32_any, param_rgba_any, Effect, EffectContext,
};
use raster_cpu::{composite_normal, Canvas};
use rayon::prelude::*;
use serde_json::Value;

const DROP_SHADOW_SOFTNESS_DIVISOR: f32 = 2.71;
const DROP_SHADOW_FLT_SOFTNESS_SCALE: f32 = 0.5;
const DROP_SHADOW_SOFTNESS_ITERATIONS: u32 = 1;
// Full-frame Rayon passes regress memory-bound 1080p renders on Apple Silicon.
// Keep UHD and larger canvases eligible while leaving production 1080p serial.
const PARALLEL_SHADOW_PIXEL_THRESHOLD: usize = 8_000_000;

#[derive(Debug, Default)]
pub struct DropShadow;

impl Effect for DropShadow {
    fn match_name(&self) -> &'static str {
        "ADBE Drop Shadow"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = DropShadowParams::from_json(params);
        let resolved = resolve_drop_shadow_debug_params(params);
        let mut shadow = raw_offset_shadow_canvas(input, params, resolved);

        if resolved.blur_radius > 0 {
            shadow = blur_shadow_alpha_channel_only(
                &shadow,
                resolved.blur_radius,
                resolved.blur_iterations,
                params.color,
            );
        }

        if params.shadow_only {
            return Ok(shadow);
        }

        let mut output = shadow;
        composite_normal(&mut output, input, 100.0);
        Ok(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DropShadowParams {
    pub color: [u8; 4],
    pub opacity: f32,
    pub direction_degrees: f32,
    pub distance: f32,
    pub softness: f32,
    pub shadow_only: bool,
}

impl DropShadowParams {
    pub(crate) fn from_json(params: &Value) -> Self {
        Self {
            color: param_rgba_any(params, &["color", "Color", "0001", "0050"], [0, 0, 0, 255]),
            opacity: param_f32_any(params, &["opacity", "Opacity", "0002", "0052"], 255.0),
            direction_degrees: param_f32_any(
                params,
                &["direction", "Direction", "0003", "0054"],
                135.0,
            ),
            distance: param_f32_any(params, &["distance", "Distance", "0004", "0053"], 5.0),
            softness: param_f32_any(params, &["softness", "Softness", "0005", "0051"], 0.0),
            shadow_only: param_bool_any(params, &["shadowOnly", "Shadow Only", "0006"], false),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropShadowDebugParams {
    pub color: [u8; 4],
    pub opacity: f32,
    pub opacity_normalized: f32,
    pub direction_degrees: f32,
    pub distance: f32,
    pub softness: f32,
    pub shadow_only: bool,
    pub dx: i32,
    pub dy: i32,
    pub blur_radius: u32,
    pub blur_iterations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropShadowIntermediateHashes {
    pub input_rgba: u64,
    pub source_alpha_rgba: u64,
    pub raw_offset_shadow_rgba: u64,
    pub blurred_shadow_rgba: u64,
    pub final_rgba: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropShadowIntermediateAlphaStats {
    pub input: CanvasAlphaStats,
    pub source_alpha: CanvasAlphaStats,
    pub raw_offset_shadow: CanvasAlphaStats,
    pub blurred_shadow: CanvasAlphaStats,
    pub final_output: CanvasAlphaStats,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropShadowDebugTrace {
    pub params: DropShadowDebugParams,
    pub hashes: DropShadowIntermediateHashes,
    pub alpha: DropShadowIntermediateAlphaStats,
}

pub fn drop_shadow_debug_trace(input: &Canvas, params: &Value) -> DropShadowDebugTrace {
    let params = DropShadowParams::from_json(params);
    let resolved = resolve_drop_shadow_debug_params(params);
    let source_alpha = alpha_mask_canvas(input);
    let raw_shadow = raw_offset_shadow_canvas(input, params, resolved);
    let mut blurred_shadow = raw_shadow.clone();
    if resolved.blur_radius > 0 {
        blurred_shadow = blur_shadow_alpha_channel_only(
            &blurred_shadow,
            resolved.blur_radius,
            resolved.blur_iterations,
            params.color,
        );
    }

    let mut output = blurred_shadow.clone();
    if !params.shadow_only {
        composite_normal(&mut output, input, 100.0);
    }

    DropShadowDebugTrace {
        params: resolved,
        hashes: DropShadowIntermediateHashes {
            input_rgba: canvas_debug_hash(input),
            source_alpha_rgba: canvas_debug_hash(&source_alpha),
            raw_offset_shadow_rgba: canvas_debug_hash(&raw_shadow),
            blurred_shadow_rgba: canvas_debug_hash(&blurred_shadow),
            final_rgba: canvas_debug_hash(&output),
        },
        alpha: DropShadowIntermediateAlphaStats {
            input: canvas_alpha_stats(input),
            source_alpha: canvas_alpha_stats(&source_alpha),
            raw_offset_shadow: canvas_alpha_stats(&raw_shadow),
            blurred_shadow: canvas_alpha_stats(&blurred_shadow),
            final_output: canvas_alpha_stats(&output),
        },
    }
}

fn resolve_drop_shadow_debug_params(params: DropShadowParams) -> DropShadowDebugParams {
    let (dx, dy) = drop_shadow_offset(params.direction_degrees, params.distance);
    DropShadowDebugParams {
        color: params.color,
        opacity: params.opacity,
        opacity_normalized: normalize_opacity(params.opacity),
        direction_degrees: params.direction_degrees,
        distance: params.distance,
        softness: params.softness,
        shadow_only: params.shadow_only,
        dx,
        dy,
        blur_radius: drop_shadow_blur_radius(params.softness),
        blur_iterations: drop_shadow_blur_iterations(params.softness),
    }
}

fn drop_shadow_offset(direction_degrees: f32, distance: f32) -> (i32, i32) {
    let direction = direction_degrees.to_radians();
    (
        (direction.sin() * distance).trunc() as i32,
        (-direction.cos() * distance).trunc() as i32,
    )
}

fn drop_shadow_blur_radius(softness: f32) -> u32 {
    if softness <= 0.0 {
        0
    } else {
        blur_radius((softness * DROP_SHADOW_FLT_SOFTNESS_SCALE) / DROP_SHADOW_SOFTNESS_DIVISOR)
    }
}

fn drop_shadow_blur_iterations(softness: f32) -> u32 {
    if softness <= 0.0 {
        0
    } else {
        DROP_SHADOW_SOFTNESS_ITERATIONS
    }
}

fn alpha_mask_canvas(input: &Canvas) -> Canvas {
    let mut mask = Canvas::transparent(input.width, input.height);
    if canvas_uses_parallel_pixels(input) {
        mask.data
            .par_chunks_mut(4)
            .zip(input.data.par_chunks(4))
            .for_each(|(output, source)| output.fill(source[3]));
    } else {
        for (output, source) in mask.data.chunks_mut(4).zip(input.data.chunks(4)) {
            output.fill(source[3]);
        }
    }
    mask
}

fn blur_shadow_alpha_channel_only(
    shadow: &Canvas,
    radius: u32,
    iterations: u32,
    color: [u8; 4],
) -> Canvas {
    let mut alpha_only = Canvas::transparent(shadow.width, shadow.height);
    if canvas_uses_parallel_pixels(shadow) {
        alpha_only
            .data
            .par_chunks_mut(4)
            .zip(shadow.data.par_chunks(4))
            .for_each(|(output, source)| output[3] = source[3]);
    } else {
        for (output, source) in alpha_only.data.chunks_mut(4).zip(shadow.data.chunks(4)) {
            output[3] = source[3];
        }
    }

    let mut blurred_alpha = alpha_only;
    for _ in 0..iterations.max(1) {
        blurred_alpha = blur_canvas(&blurred_alpha, radius);
    }
    let mut output = Canvas::transparent(shadow.width, shadow.height);
    if canvas_uses_parallel_pixels(shadow) {
        output
            .data
            .par_chunks_mut(4)
            .zip(blurred_alpha.data.par_chunks(4))
            .for_each(|(output, source)| colorize_shadow_pixel(output, source[3], color));
    } else {
        for (output, source) in output.data.chunks_mut(4).zip(blurred_alpha.data.chunks(4)) {
            colorize_shadow_pixel(output, source[3], color);
        }
    }
    output
}

fn raw_offset_shadow_canvas(
    input: &Canvas,
    params: DropShadowParams,
    resolved: DropShadowDebugParams,
) -> Canvas {
    let mut shadow = Canvas::transparent(input.width, input.height);
    if canvas_uses_parallel_pixels(input) {
        shadow
            .data
            .par_chunks_mut(4)
            .enumerate()
            .for_each(|(index, output)| {
                write_offset_shadow_pixel(output, index, input, params, resolved)
            });
    } else {
        for (index, output) in shadow.data.chunks_mut(4).enumerate() {
            write_offset_shadow_pixel(output, index, input, params, resolved);
        }
    }
    shadow
}

fn write_offset_shadow_pixel(
    output: &mut [u8],
    destination_index: usize,
    input: &Canvas,
    params: DropShadowParams,
    resolved: DropShadowDebugParams,
) {
    let width = input.width as usize;
    let destination_x = (destination_index % width) as i32;
    let destination_y = (destination_index / width) as i32;
    let source_x = destination_x - resolved.dx;
    let source_y = destination_y - resolved.dy;
    if source_x < 0
        || source_y < 0
        || source_x >= input.width as i32
        || source_y >= input.height as i32
    {
        return;
    }
    let source = input.pixel(source_x as u32, source_y as u32);
    if source[3] == 0 {
        return;
    }
    let alpha = (source[3] as f32 * resolved.opacity_normalized)
        .round()
        .clamp(0.0, 255.0) as u8;
    if alpha == 0 {
        return;
    }
    output.copy_from_slice(&[params.color[0], params.color[1], params.color[2], alpha]);
}

fn colorize_shadow_pixel(output: &mut [u8], alpha: u8, color: [u8; 4]) {
    if alpha != 0 {
        output.copy_from_slice(&[color[0], color[1], color[2], alpha]);
    }
}

fn canvas_uses_parallel_pixels(canvas: &Canvas) -> bool {
    canvas.width as usize * canvas.height as usize >= PARALLEL_SHADOW_PIXEL_THRESHOLD
}

fn normalize_opacity(value: f32) -> f32 {
    (value / 255.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::ThreadPoolBuilder;
    use serde_json::json;

    #[test]
    fn automatic_parallelism_starts_above_full_hd() {
        assert!(1920usize * 1080 < PARALLEL_SHADOW_PIXEL_THRESHOLD);
        assert!(3840usize * 2160 >= PARALLEL_SHADOW_PIXEL_THRESHOLD);
    }

    fn patterned_canvas(width: u32, height: u32) -> Canvas {
        let mut canvas = Canvas::transparent(width, height);
        for (index, pixel) in canvas.data.chunks_mut(4).enumerate() {
            let value = index as u32;
            pixel.copy_from_slice(&[
                value.wrapping_mul(11) as u8,
                value.wrapping_mul(23).wrapping_add(7) as u8,
                value.wrapping_mul(41).wrapping_add(13) as u8,
                value.wrapping_mul(67).wrapping_add(29) as u8,
            ]);
        }
        canvas
    }

    fn raw_offset_shadow_sequential_reference(
        input: &Canvas,
        params: DropShadowParams,
        resolved: DropShadowDebugParams,
    ) -> Canvas {
        let mut shadow = Canvas::transparent(input.width, input.height);
        for y in 0..input.height {
            for x in 0..input.width {
                let source = input.pixel(x, y);
                if source[3] == 0 {
                    continue;
                }
                let sx = x as i32 + resolved.dx;
                let sy = y as i32 + resolved.dy;
                if sx < 0 || sy < 0 || sx >= input.width as i32 || sy >= input.height as i32 {
                    continue;
                }
                let alpha = (source[3] as f32 * resolved.opacity_normalized)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let existing = shadow.pixel(sx as u32, sy as u32);
                if alpha > existing[3] {
                    shadow.set_pixel(
                        sx as u32,
                        sy as u32,
                        [params.color[0], params.color[1], params.color[2], alpha],
                    );
                }
            }
        }
        shadow
    }

    #[test]
    fn drop_shadow_offsets_input_alpha() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(0, 0, [255, 255, 255, 255]);

        let output = DropShadow::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({
                    "0001": [0, 0, 0, 1],
                    "0002": 255,
                    "0003": 90,
                    "0004": 1,
                    "0005": 0,
                    "0006": true
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(1, 0)[3], 255);
    }

    #[test]
    fn params_accept_ae_numbered_wrapped_values() {
        let params = DropShadowParams::from_json(&json!({
            "0001": { "value": [1.0, 0.5, 0.0, 1.0] },
            "0002": { "value": 50 },
            "0003": { "value": 90 },
            "0004": { "value": 12 },
            "0005": { "value": 4 },
            "0006": { "value": 1 }
        }));

        assert_eq!(params.color, [255, 128, 0, 255]);
        assert_eq!(params.opacity, 50.0);
        assert_eq!(params.direction_degrees, 90.0);
        assert_eq!(params.distance, 12.0);
        assert_eq!(params.softness, 4.0);
        assert!(params.shadow_only);
    }

    #[test]
    fn ae_probe_direction_135_offsets_down_and_right() {
        assert_eq!(drop_shadow_offset(135.0, 28.0), (19, 19));
    }

    #[test]
    fn ae_probe_cardinal_directions_start_at_up_and_rotate_clockwise() {
        assert_eq!(drop_shadow_offset(0.0, 23.0), (0, -23));
        assert_eq!(drop_shadow_offset(90.0, 23.0), (23, 0));
        assert_eq!(drop_shadow_offset(180.0, 23.0), (0, 23));
        assert_eq!(drop_shadow_offset(270.0, 23.0), (-23, 0));
    }

    #[test]
    fn offset_uses_truncation_for_fractional_distances() {
        assert_eq!(drop_shadow_offset(30.0, 23.0), (11, -19));
        assert_eq!(drop_shadow_offset(315.0, 23.0), (-16, -16));
    }

    #[test]
    fn softness_blurs_alpha_without_diluting_shadow_rgb() {
        let color = [200, 100, 50, 255];
        let mut shadow = Canvas::transparent(3, 1);
        shadow.set_pixel(1, 0, [color[0], color[1], color[2], 255]);

        let blurred = blur_shadow_alpha_channel_only(&shadow, 1, 1, color);

        assert_eq!(blurred.pixel(0, 0), [200, 100, 50, 127]);
        assert_eq!(blurred.pixel(1, 0), [200, 100, 50, 85]);
        assert_eq!(blurred.pixel(2, 0), [200, 100, 50, 127]);
    }

    #[test]
    fn ae_cpu_probe_softness_18_uses_flt_blur_radius() {
        assert_eq!(drop_shadow_blur_radius(18.0), 4);
        assert_eq!(drop_shadow_blur_iterations(18.0), 1);
        assert_eq!(drop_shadow_blur_radius(1.0), 1);
        assert_eq!(drop_shadow_blur_radius(8.0), 2);
        assert_eq!(drop_shadow_blur_radius(32.0), 6);
        assert_eq!(drop_shadow_blur_radius(0.0), 0);
        assert_eq!(drop_shadow_blur_iterations(0.0), 0);
    }

    #[test]
    fn debug_trace_reports_shadow_alpha_intermediates() {
        let mut input = Canvas::transparent(5, 3);
        input.set_pixel(1, 1, [255, 255, 255, 255]);

        let trace = drop_shadow_debug_trace(
            &input,
            &json!({
                "0001": [0, 0, 0, 1],
                "0002": 50,
                "0003": 90,
                "0004": 1,
                "0005": 2,
                "0006": false
            }),
        );

        assert!((trace.params.opacity_normalized - (50.0 / 255.0)).abs() < f32::EPSILON);
        assert_eq!(trace.params.blur_radius, 1);
        assert_eq!(trace.params.blur_iterations, 1);
        assert_eq!(trace.alpha.input.nonzero_pixels, 1);
        assert_eq!(trace.alpha.raw_offset_shadow.nonzero_pixels, 1);
        assert!(
            trace.alpha.blurred_shadow.nonzero_pixels
                > trace.alpha.raw_offset_shadow.nonzero_pixels
        );
        assert!(
            trace.alpha.final_output.nonzero_pixels >= trace.alpha.blurred_shadow.nonzero_pixels
        );
    }

    #[test]
    fn ae_composite_probe_opacity_uses_raw_0_to_255_scale() {
        assert_eq!(normalize_opacity(0.0), 0.0);
        assert!((normalize_opacity(50.0) - (50.0 / 255.0)).abs() < f32::EPSILON);
        assert!((normalize_opacity(100.0) - (100.0 / 255.0)).abs() < f32::EPSILON);
        assert!((normalize_opacity(180.0) - (180.0 / 255.0)).abs() < f32::EPSILON);
        assert_eq!(normalize_opacity(255.0), 1.0);
    }

    #[test]
    fn color_alpha_is_not_part_of_ae_drop_shadow_color_property() {
        let mut input = Canvas::transparent(1, 1);
        input.set_pixel(0, 0, [255, 255, 255, 255]);

        let opaque_color_alpha = raw_offset_shadow_canvas(
            &input,
            DropShadowParams {
                color: [255, 0, 0, 255],
                opacity: 100.0,
                direction_degrees: 0.0,
                distance: 0.0,
                softness: 0.0,
                shadow_only: true,
            },
            DropShadowDebugParams {
                color: [255, 0, 0, 255],
                opacity: 100.0,
                opacity_normalized: normalize_opacity(100.0),
                direction_degrees: 0.0,
                distance: 0.0,
                softness: 0.0,
                shadow_only: true,
                dx: 0,
                dy: 0,
                blur_radius: 0,
                blur_iterations: 0,
            },
        );
        let half_color_alpha = raw_offset_shadow_canvas(
            &input,
            DropShadowParams {
                color: [255, 0, 0, 128],
                opacity: 100.0,
                direction_degrees: 0.0,
                distance: 0.0,
                softness: 0.0,
                shadow_only: true,
            },
            DropShadowDebugParams {
                color: [255, 0, 0, 128],
                opacity: 100.0,
                opacity_normalized: normalize_opacity(100.0),
                direction_degrees: 0.0,
                distance: 0.0,
                softness: 0.0,
                shadow_only: true,
                dx: 0,
                dy: 0,
                blur_radius: 0,
                blur_iterations: 0,
            },
        );

        assert_eq!(opaque_color_alpha.pixel(0, 0), [255, 0, 0, 100]);
        assert_eq!(half_color_alpha.pixel(0, 0), [255, 0, 0, 100]);
    }

    #[test]
    fn sapphire_drop_shadow_params_lower_to_native_shadow_fields() {
        let params = DropShadowParams::from_json(&json!({
            "0050": [1.0, 0.5, 0.0, 1.0],
            "0051": 8.0,
            "0052": 60.0,
            "0053": 0.0,
            "0054": 0.0
        }));
        assert_eq!(params.color, [255, 128, 0, 255]);
        assert_eq!(params.softness, 8.0);
        assert_eq!(params.opacity, 60.0);
        assert_eq!(params.distance, 0.0);
        assert_eq!(params.direction_degrees, 0.0);
    }

    #[test]
    fn parallel_offset_is_byte_exact_against_forward_reference() {
        let input = patterned_canvas(257, 131);
        let params = DropShadowParams {
            color: [201, 103, 47, 128],
            opacity: 113.0,
            direction_degrees: 135.0,
            distance: 19.0,
            softness: 0.0,
            shadow_only: true,
        };
        let resolved = resolve_drop_shadow_debug_params(params);
        let expected = raw_offset_shadow_sequential_reference(&input, params, resolved);
        let one_thread = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let four_threads = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
        let actual_one = one_thread.install(|| raw_offset_shadow_canvas(&input, params, resolved));
        let actual_four =
            four_threads.install(|| raw_offset_shadow_canvas(&input, params, resolved));

        assert_eq!(actual_one.data, expected.data);
        assert_eq!(actual_four.data, expected.data);
    }

    #[test]
    fn full_shadow_is_deterministic_across_thread_counts() {
        let input = patterned_canvas(257, 131);
        let params = json!({
            "color": [17, 29, 43, 255],
            "opacity": 181,
            "direction": 217,
            "distance": 13,
            "softness": 32,
            "shadowOnly": false
        });
        let context = EffectContext {
            time: 0.0,
            fps: 24.0,
        };
        let one_thread = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let four_threads = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
        let actual_one = one_thread.install(|| {
            DropShadow::default()
                .render(&input, &context, &params)
                .unwrap()
        });
        let actual_four = four_threads.install(|| {
            DropShadow::default()
                .render(&input, &context, &params)
                .unwrap()
        });

        assert_eq!(actual_one.data, actual_four.data);
    }
}
