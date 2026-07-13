use crate::{param_bool_any, param_f32_at_any, param_value, Effect, EffectContext};
use raster_cpu::Canvas;
use rayon::prelude::*;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct F3Stylize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StylizeMode {
    Extract,
    Xerox,
    NeonExtract,
    OldCamera,
}

impl StylizeMode {
    fn from_params(params: &Value) -> Self {
        let value = ["mode", "style", "operation"]
            .into_iter()
            .find_map(|name| param_value(params, name))
            .and_then(Value::as_str)
            .unwrap_or("extract")
            .to_ascii_lowercase();
        match value.as_str() {
            "xerox" => Self::Xerox,
            "neon" | "neon_extract" => Self::NeonExtract,
            "old_camera" | "oldcamera" | "film" => Self::OldCamera,
            _ => Self::Extract,
        }
    }
}

impl Effect for F3Stylize {
    fn match_name(&self) -> &'static str {
        "ANR F3 Stylize"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let mode = StylizeMode::from_params(params);
        let threshold = param_f32_at_any(
            params,
            &["threshold", "Threshold", "black_point"],
            ctx.time,
            128.0,
        )
        .clamp(0.0, 255.0);
        let softness =
            param_f32_at_any(params, &["softness", "Softness", "feather"], ctx.time, 24.0)
                .max(0.001);
        let amount = param_f32_at_any(params, &["amount", "Amount", "intensity"], ctx.time, 1.0)
            .clamp(0.0, 4.0);
        let composite_original = param_bool_any(
            params,
            &["composite_original", "compositeOriginal", "keep_original"],
            false,
        );
        let mut output = Canvas::transparent(input.width, input.height);
        let width = input.width as usize;
        let height = input.height as usize;
        output
            .data
            .par_chunks_exact_mut(4)
            .enumerate()
            .for_each(|(index, out)| {
                let x = index % width.max(1);
                let y = index / width.max(1);
                let source = rgba_at(input, x as i32, y as i32);
                let styled = match mode {
                    StylizeMode::Extract => extract_pixel(source, threshold, softness, amount),
                    StylizeMode::Xerox => {
                        xerox_pixel(input, x, y, source, threshold, softness, amount)
                    }
                    StylizeMode::NeonExtract => neon_pixel(input, x, y, source, amount),
                    StylizeMode::OldCamera => {
                        old_camera_pixel(source, x, y, width, height, ctx.time, amount)
                    }
                };
                let pixel = if composite_original {
                    add_over_original(source, styled)
                } else {
                    styled
                };
                out.copy_from_slice(&pixel);
            });
        Ok(output)
    }
}

fn add_over_original(original: [u8; 4], styled: [u8; 4]) -> [u8; 4] {
    [
        original[0].saturating_add(styled[0]),
        original[1].saturating_add(styled[1]),
        original[2].saturating_add(styled[2]),
        original[3].max(styled[3]),
    ]
}

fn extract_pixel(pixel: [u8; 4], threshold: f32, softness: f32, amount: f32) -> [u8; 4] {
    let mask = smoothstep(
        threshold - softness * 0.5,
        threshold + softness * 0.5,
        luminance(pixel),
    );
    let gain = (mask * amount).clamp(0.0, 1.0);
    [
        (pixel[0] as f32 * gain).round() as u8,
        (pixel[1] as f32 * gain).round() as u8,
        (pixel[2] as f32 * gain).round() as u8,
        (pixel[3] as f32 * gain).round() as u8,
    ]
}

fn xerox_pixel(
    input: &Canvas,
    x: usize,
    y: usize,
    source: [u8; 4],
    threshold: f32,
    softness: f32,
    amount: f32,
) -> [u8; 4] {
    let edge = sobel_luma(input, x, y);
    let tone = 1.0
        - smoothstep(
            threshold - softness * 0.5,
            threshold + softness * 0.5,
            luminance(source),
        );
    let ink = (tone.max(edge / 255.0) * amount).clamp(0.0, 1.0);
    let value = ((1.0 - ink) * 255.0).round() as u8;
    [value, value, value, source[3]]
}

fn neon_pixel(input: &Canvas, x: usize, y: usize, source: [u8; 4], amount: f32) -> [u8; 4] {
    let edge = (sobel_luma(input, x, y) / 255.0 * amount).clamp(0.0, 1.0);
    let max_rgb = source[0].max(source[1]).max(source[2]).max(1) as f32;
    [
        (source[0] as f32 / max_rgb * edge * 255.0).round() as u8,
        (source[1] as f32 / max_rgb * edge * 255.0).round() as u8,
        (source[2] as f32 / max_rgb * edge * 255.0).round() as u8,
        (source[3] as f32 * edge).round() as u8,
    ]
}

fn old_camera_pixel(
    source: [u8; 4],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    time: f64,
    amount: f32,
) -> [u8; 4] {
    let nx = (x as f32 + 0.5) / width.max(1) as f32 * 2.0 - 1.0;
    let ny = (y as f32 + 0.5) / height.max(1) as f32 * 2.0 - 1.0;
    let vignette = (1.0 - (nx * nx + ny * ny) * 0.42 * amount).clamp(0.25, 1.0);
    let frame = (time * 24.0).floor().max(0.0) as u32;
    let noise = hash_noise(x as u32, y as u32, frame) * 2.0 - 1.0;
    let flicker = 0.96 + hash_noise(frame, frame.rotate_left(7), 0) * 0.08;
    let grain = noise * 14.0 * amount;
    let luma = luminance(source);
    let value = |tint: f32| {
        (luma * tint * vignette * flicker + grain)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [value(1.08), value(1.0), value(0.82), source[3]]
}

fn sobel_luma(input: &Canvas, x: usize, y: usize) -> f32 {
    let x = x as i32;
    let y = y as i32;
    let sample = |dx, dy| luminance(rgba_at(input, x + dx, y + dy));
    let gx = -sample(-1, -1) + sample(1, -1) - 2.0 * sample(-1, 0) + 2.0 * sample(1, 0)
        - sample(-1, 1)
        + sample(1, 1);
    let gy = -sample(-1, -1) - 2.0 * sample(0, -1) - sample(1, -1)
        + sample(-1, 1)
        + 2.0 * sample(0, 1)
        + sample(1, 1);
    (gx * gx + gy * gy).sqrt().clamp(0.0, 255.0)
}

fn rgba_at(input: &Canvas, x: i32, y: i32) -> [u8; 4] {
    if x < 0 || y < 0 || x >= input.width as i32 || y >= input.height as i32 {
        return [0, 0, 0, 0];
    }
    let index = (y as usize * input.width as usize + x as usize) * 4;
    [
        input.data[index],
        input.data[index + 1],
        input.data[index + 2],
        input.data[index + 3],
    ]
}

fn luminance(pixel: [u8; 4]) -> f32 {
    0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0).max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash_noise(x: u32, y: u32, seed: u32) -> f32 {
    let mut value = x
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(y.rotate_left(16))
        .wrapping_add(seed.wrapping_mul(0x85eb_ca6b));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    (value ^ (value >> 16)) as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn render(input: &Canvas, params: Value, time: f64) -> Canvas {
        F3Stylize
            .render(input, &EffectContext { time, fps: 24.0 }, &params)
            .unwrap()
    }

    #[test]
    fn extract_keeps_only_highlights() {
        let mut input = Canvas::transparent(2, 1);
        input.set_pixel(0, 0, [20, 20, 20, 255]);
        input.set_pixel(1, 0, [240, 240, 240, 255]);
        let output = render(
            &input,
            json!({"mode":"extract", "threshold":128, "softness":1}),
            0.0,
        );
        assert_eq!(output.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(output.pixel(1, 0), [240, 240, 240, 255]);
    }

    #[test]
    fn neon_extract_marks_a_hard_edge() {
        let mut input = Canvas::new(5, 3, [0, 0, 0, 255]);
        for y in 0..3 {
            for x in 3..5 {
                input.set_pixel(x, y, [255, 20, 20, 255]);
            }
        }
        let output = render(&input, json!({"mode":"neon_extract"}), 0.0);
        assert!(output.pixel(2, 1)[3] > 0 || output.pixel(3, 1)[3] > 0);
        assert_eq!(output.pixel(0, 1)[3], 0);
    }

    #[test]
    fn old_camera_is_deterministic_per_frame() {
        let input = Canvas::new(8, 8, [120, 140, 160, 255]);
        let first = render(&input, json!({"mode":"old_camera"}), 1.0);
        let second = render(&input, json!({"mode":"old_camera"}), 1.0);
        let next = render(&input, json!({"mode":"old_camera"}), 1.1);
        assert_eq!(first.data, second.data);
        assert_ne!(first.data, next.data);
    }

    #[test]
    fn extract_can_add_highlights_over_original() {
        let input = Canvas::new(1, 1, [100, 100, 100, 255]);
        let output = render(
            &input,
            json!({
                "mode":"extract",
                "threshold":50,
                "softness":1,
                "composite_original":true
            }),
            0.0,
        );
        assert_eq!(output.pixel(0, 0), [200, 200, 200, 255]);
    }
}
