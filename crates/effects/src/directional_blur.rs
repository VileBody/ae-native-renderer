use crate::{param_f32_at_any, Effect, EffectContext};
use raster_cpu::Canvas;
use rayon::prelude::*;
use serde_json::Value;

const DIRECTION_NAMES: &[&str] = &["direction", "Direction", "0001", "ADBE Motion Blur-0001"];
const BLUR_LENGTH_NAMES: &[&str] = &[
    "blur_length",
    "blurLength",
    "Blur Length",
    "length",
    "0002",
    "ADBE Motion Blur-0002",
];

#[derive(Debug, Default)]
pub struct DirectionalBlur;

impl Effect for DirectionalBlur {
    fn match_name(&self) -> &'static str {
        "ADBE Motion Blur"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = DirectionalBlurParams::from_json(params, ctx.time);
        if params.blur_length <= f32::EPSILON || input.width == 0 || input.height == 0 {
            return Ok(input.clone());
        }

        let direction = params.direction_degrees.rem_euclid(180.0);
        if direction.min(180.0 - direction) <= 0.0001 {
            return Ok(axis_blur(input, params.blur_length, Axis::Horizontal));
        }
        if (direction - 90.0).abs() <= 0.0001 {
            return Ok(axis_blur(input, params.blur_length, Axis::Vertical));
        }

        Ok(line_blur(
            input,
            params.direction_degrees,
            params.blur_length,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DirectionalBlurParams {
    pub direction_degrees: f32,
    pub blur_length: f32,
}

impl DirectionalBlurParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        let direction_degrees = param_f32_at_any(params, DIRECTION_NAMES, time, 0.0);
        let blur_length = param_f32_at_any(params, BLUR_LENGTH_NAMES, time, 0.0);
        Self {
            direction_degrees: finite_or(direction_degrees, 0.0),
            blur_length: finite_or(blur_length, 0.0).max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

fn axis_blur(input: &Canvas, blur_length: f32, axis: Axis) -> Canvas {
    let radius = (blur_length * 0.5).ceil().max(1.0) as i32;
    let tap_count = (radius * 2 + 1) as f64;
    let mut output = Canvas::transparent(input.width, input.height);

    match axis {
        Axis::Horizontal => {
            let row_bytes = input.width as usize * 4;
            input
                .data
                .par_chunks_exact(row_bytes)
                .zip(output.data.par_chunks_exact_mut(row_bytes))
                .for_each(|(source, destination)| {
                    sliding_axis_line(
                        input.width as usize,
                        radius as usize,
                        tap_count,
                        |position| rgba_from_bytes(source, position),
                        |position, pixel| write_rgba(destination, position, pixel),
                    );
                });
        }
        Axis::Vertical => {
            let columns = (0..input.width as usize)
                .into_par_iter()
                .map(|x| {
                    let mut column = vec![[0_u8; 4]; input.height as usize];
                    sliding_axis_line(
                        input.height as usize,
                        radius as usize,
                        tap_count,
                        |y| rgba_at_index(input, y * input.width as usize + x),
                        |y, pixel| column[y] = pixel,
                    );
                    column
                })
                .collect::<Vec<_>>();
            for (x, column) in columns.into_iter().enumerate() {
                for (y, pixel) in column.into_iter().enumerate() {
                    let index = (y * input.width as usize + x) * 4;
                    output.data[index..index + 4].copy_from_slice(&pixel);
                }
            }
        }
    }

    output
}

fn sliding_axis_line(
    length: usize,
    radius: usize,
    tap_count: f64,
    pixel_at: impl Fn(usize) -> [u8; 4],
    mut write: impl FnMut(usize, [u8; 4]),
) {
    if length == 0 {
        return;
    }
    let mut sums = [0.0_f64; 4];
    for position in 0..=radius.min(length - 1) {
        add_premultiplied(&mut sums, pixel_at(position), 1.0);
    }
    for center in 0..length {
        write(center, straight_rgba_from_sums(sums, tap_count));
        if center >= radius {
            add_premultiplied(&mut sums, pixel_at(center - radius), -1.0);
        }
        let incoming = center.saturating_add(radius).saturating_add(1);
        if incoming < length {
            add_premultiplied(&mut sums, pixel_at(incoming), 1.0);
        }
    }
}

fn add_premultiplied(sums: &mut [f64; 4], pixel: [u8; 4], sign: f64) {
    let alpha = pixel[3] as f64 / 255.0;
    sums[0] += pixel[0] as f64 * alpha * sign;
    sums[1] += pixel[1] as f64 * alpha * sign;
    sums[2] += pixel[2] as f64 * alpha * sign;
    sums[3] += pixel[3] as f64 * sign;
}

fn straight_rgba_from_sums(sums: [f64; 4], tap_count: f64) -> [u8; 4] {
    let mut output = [0_u8; 4];
    if sums[3] > f64::EPSILON {
        for channel in 0..3 {
            output[channel] = (sums[channel] * 255.0 / sums[3]).round().clamp(0.0, 255.0) as u8;
        }
    }
    output[3] = (sums[3] / tap_count).round().clamp(0.0, 255.0) as u8;
    output
}

fn line_blur(input: &Canvas, direction_degrees: f32, blur_length: f32) -> Canvas {
    let radians = direction_degrees.to_radians();
    let direction = (radians.cos(), radians.sin());
    let sample_count = (blur_length.ceil() as usize + 1).max(3) | 1;
    let denominator = (sample_count - 1) as f32;
    let mut output = Canvas::transparent(input.width, input.height);
    output
        .data
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(index, destination)| {
            let x = index % input.width as usize;
            let y = index / input.width as usize;
            let mut accumulated = [0.0_f32; 4];
            for sample_index in 0..sample_count {
                let progress = sample_index as f32 / denominator - 0.5;
                let distance = progress * blur_length;
                let sample = sample_premultiplied_bilinear(
                    input,
                    x as f32 + direction.0 * distance,
                    y as f32 + direction.1 * distance,
                );
                for channel in 0..4 {
                    accumulated[channel] += sample[channel];
                }
            }
            let scale = 1.0 / sample_count as f32;
            for channel in &mut accumulated {
                *channel *= scale;
            }
            destination.copy_from_slice(&straight_rgba(accumulated));
        });

    output
}

fn rgba_from_bytes(bytes: &[u8], position: usize) -> [u8; 4] {
    let index = position * 4;
    [
        bytes[index],
        bytes[index + 1],
        bytes[index + 2],
        bytes[index + 3],
    ]
}

fn write_rgba(bytes: &mut [u8], position: usize, pixel: [u8; 4]) {
    let index = position * 4;
    bytes[index..index + 4].copy_from_slice(&pixel);
}

fn rgba_at_index(input: &Canvas, pixel_index: usize) -> [u8; 4] {
    let index = pixel_index * 4;
    [
        input.data[index],
        input.data[index + 1],
        input.data[index + 2],
        input.data[index + 3],
    ]
}

pub(crate) fn sample_premultiplied_bilinear(input: &Canvas, x: f32, y: f32) -> [f32; 4] {
    if !x.is_finite() || !y.is_finite() {
        return [0.0; 4];
    }
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let mut output = [0.0_f32; 4];

    for (sample_y, weight_y) in [(y0, 1.0 - ty), (y0 + 1, ty)] {
        for (sample_x, weight_x) in [(x0, 1.0 - tx), (x0 + 1, tx)] {
            if sample_x < 0
                || sample_y < 0
                || sample_x >= input.width as i32
                || sample_y >= input.height as i32
            {
                continue;
            }
            let weight = weight_x * weight_y;
            if weight <= 0.0 {
                continue;
            }
            let pixel = input.pixel(sample_x as u32, sample_y as u32);
            let alpha = pixel[3] as f32 / 255.0;
            output[0] += pixel[0] as f32 / 255.0 * alpha * weight;
            output[1] += pixel[1] as f32 / 255.0 * alpha * weight;
            output[2] += pixel[2] as f32 / 255.0 * alpha * weight;
            output[3] += alpha * weight;
        }
    }

    output
}

pub(crate) fn straight_rgba(premultiplied: [f32; 4]) -> [u8; 4] {
    let alpha = premultiplied[3].clamp(0.0, 1.0);
    let mut output = [0_u8; 4];
    if alpha > 1.0e-7 {
        for channel in 0..3 {
            output[channel] = (premultiplied[channel] / alpha * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
    output[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    output
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
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
    fn animated_direction_and_length_are_sampled_at_context_time() {
        let params = json!({
            "ADBE Motion Blur-0001": {
                "keyframes": [{"t": 0.0, "v": 0.0}, {"t": 1.0, "v": 90.0}]
            },
            "ADBE Motion Blur-0002": {
                "keyframes": [{"t": 0.0, "v": 0.0}, {"t": 1.0, "v": 8.0}]
            }
        });
        let parsed = DirectionalBlurParams::from_json(&params, 0.5);
        assert!((parsed.direction_degrees - 45.0).abs() < 0.001);
        assert!((parsed.blur_length - 4.0).abs() < 0.001);
    }

    #[test]
    fn horizontal_impulse_blurs_only_along_the_requested_axis() {
        let mut input = Canvas::transparent(7, 5);
        input.set_pixel(3, 2, [255, 255, 255, 255]);
        let output = DirectionalBlur
            .render(
                &input,
                &context(0.0),
                &json!({"direction": 0.0, "blur_length": 4.0}),
            )
            .unwrap();

        assert!(output.pixel(1, 2)[3] > 0);
        assert!(output.pixel(5, 2)[3] > 0);
        assert_eq!(output.pixel(3, 1), [0, 0, 0, 0]);
        assert_eq!(output.pixel(3, 3), [0, 0, 0, 0]);
    }

    #[test]
    fn checkerboard_edges_are_transparently_extended() {
        let mut input = Canvas::transparent(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                let value = if (x + y) % 2 == 0 { 255 } else { 0 };
                input.set_pixel(x, y, [value, value, value, 255]);
            }
        }
        let output = DirectionalBlur
            .render(
                &input,
                &context(0.0),
                &json!({"direction": 90.0, "blur_length": 4.0}),
            )
            .unwrap();

        assert!(output.pixel(2, 0)[3] < 255);
        assert_eq!(output.pixel(2, 2)[3], 255);
        assert_ne!(output.pixel(2, 2)[0], input.pixel(2, 2)[0]);
    }

    #[test]
    fn bilinear_sampling_does_not_leak_rgb_from_transparent_pixels() {
        let input = Canvas::from_rgba(2, 1, vec![255, 0, 0, 0, 0, 0, 255, 128]).unwrap();
        let sampled = straight_rgba(sample_premultiplied_bilinear(&input, 0.5, 0.0));
        assert_eq!(sampled, [0, 0, 255, 64]);
    }

    #[test]
    fn zero_length_is_an_exact_identity() {
        let input = Canvas::new(3, 2, [17, 33, 65, 129]);
        let output = DirectionalBlur
            .render(
                &input,
                &context(1.0),
                &json!({"direction": 73.0, "blur_length": 0.0}),
            )
            .unwrap();
        assert_eq!(output.data, input.data);
    }
}
