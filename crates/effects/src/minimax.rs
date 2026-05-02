use crate::{param_f32, param_f32_at, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct Minimax;

impl Effect for Minimax {
    fn match_name(&self) -> &'static str {
        "ADBE Minimax"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let radius = param_f32_at(params, "0002", _ctx.time, param_f32(params, "radius", 0.0))
            .round()
            .clamp(0.0, 32.0) as u32;
        if radius == 0 || input.width == 0 || input.height == 0 {
            return Ok(input.clone());
        }

        let operation = Operation::from_params(params);
        let channels = Channels::from_params(params);
        Ok(minimax_canvas(input, radius, operation, channels))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Maximum,
    Minimum,
}

impl Operation {
    fn from_params(params: &Value) -> Self {
        for name in ["operation", "Operation", "mode", "0001"] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if text.contains("min") || text.contains("erode") {
                    return Self::Minimum;
                }
                if text.contains("max") || text.contains("dilate") {
                    return Self::Maximum;
                }
            }
            if let Some(number) = value.as_i64() {
                return if number == 2 {
                    Self::Minimum
                } else {
                    Self::Maximum
                };
            }
        }
        Self::Maximum
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channels {
    Alpha,
    Rgba,
}

impl Channels {
    fn from_params(params: &Value) -> Self {
        for name in ["channels", "Channels", "channel", "0003"] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if text.contains("rgba") || text.contains("color") {
                    return Self::Rgba;
                }
                if text.contains("alpha") {
                    return Self::Alpha;
                }
            }
            if let Some(number) = value.as_i64() {
                return if number == 2 { Self::Rgba } else { Self::Alpha };
            }
            if value.as_bool() == Some(true) {
                return Self::Rgba;
            }
        }
        Self::Alpha
    }
}

fn minimax_canvas(input: &Canvas, radius: u32, operation: Operation, channels: Channels) -> Canvas {
    let mut output = Canvas::transparent(input.width, input.height);
    for y in 0..input.height {
        for x in 0..input.width {
            output.set_pixel(
                x,
                y,
                extremum_pixel(input, x, y, radius, operation, channels),
            );
        }
    }
    output
}

fn extremum_pixel(
    input: &Canvas,
    x: u32,
    y: u32,
    radius: u32,
    operation: Operation,
    channels: Channels,
) -> [u8; 4] {
    let mut output = input.pixel(x, y);
    let mut values = match operation {
        Operation::Maximum => [0_u8; 4],
        Operation::Minimum => [255_u8; 4],
    };
    let min_x = x.saturating_sub(radius);
    let min_y = y.saturating_sub(radius);
    let max_x = (x + radius).min(input.width - 1);
    let max_y = (y + radius).min(input.height - 1);

    for sy in min_y..=max_y {
        for sx in min_x..=max_x {
            let pixel = input.pixel(sx, sy);
            for channel in 0..4 {
                values[channel] = match operation {
                    Operation::Maximum => values[channel].max(pixel[channel]),
                    Operation::Minimum => values[channel].min(pixel[channel]),
                };
            }
        }
    }

    match channels {
        Channels::Alpha => output[3] = values[3],
        Channels::Rgba => output = values,
    }
    output
}

fn param_value<'a>(params: &'a Value, name: &str) -> Option<&'a Value> {
    let value = params.get(name)?;
    Some(value.get("value").unwrap_or(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maximum_expands_alpha() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [20, 40, 60, 255]);

        let output = Minimax::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({ "radius": 1, "operation": "maximum", "channels": "alpha" }),
            )
            .unwrap();

        assert_eq!(output.pixel(0, 0)[3], 255);
        assert_eq!(output.pixel(2, 0)[3], 255);
        assert_eq!(output.pixel(0, 0)[0], 0);
    }

    #[test]
    fn minimum_erodes_rgba() {
        let mut input = Canvas::new(3, 1, [200, 180, 160, 255]);
        input.set_pixel(1, 0, [10, 20, 30, 0]);

        let output = Minimax::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({ "radius": 1, "operation": "minimum", "channels": "rgba" }),
            )
            .unwrap();

        assert_eq!(output.pixel(0, 0), [10, 20, 30, 0]);
        assert_eq!(output.pixel(2, 0), [10, 20, 30, 0]);
    }
}
