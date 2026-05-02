use crate::{param_f32_any, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct BoxBlur2;

impl Effect for BoxBlur2 {
    fn match_name(&self) -> &'static str {
        "ADBE Box Blur2"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = BoxBlurParams::from_json(params);
        Ok(blur_canvas(input, blur_radius(params.radius)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoxBlurParams {
    pub radius: f32,
    pub iterations: f32,
}

impl BoxBlurParams {
    pub(crate) fn from_json(params: &Value) -> Self {
        Self {
            radius: param_f32_any(params, &["radius", "Radius", "0001", "0002"], 0.0),
            iterations: param_f32_any(params, &["iterations", "Iterations", "0002"], 1.0),
        }
    }
}

pub(crate) fn blur_canvas(input: &Canvas, radius: u32) -> Canvas {
    if radius == 0 || input.width == 0 || input.height == 0 {
        return input.clone();
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

    output
}

pub(crate) fn blur_radius(value: f32) -> u32 {
    value.round().clamp(0.0, 64.0) as u32
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
            BoxBlurParams::from_json(&serde_json::json!({ "radius": 2.4 })).radius,
            2.4
        );
        assert_eq!(
            BoxBlurParams::from_json(&serde_json::json!({ "0001": { "value": 7 } })).radius,
            7.0
        );
        assert_eq!(
            BoxBlurParams::from_json(&serde_json::json!({ "0002": 3 })).radius,
            3.0
        );
    }
}
