use crate::{param_f32_any, param_f32_at_any, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct TurbulentDisplace;

impl Effect for TurbulentDisplace {
    fn match_name(&self) -> &'static str {
        "ADBE Turbulent Displace"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = TurbulentDisplaceParams::from_json(params, ctx.time);
        let amount = params.amount.clamp(0.0, 200.0);
        if amount <= f32::EPSILON || input.width == 0 || input.height == 0 {
            return Ok(input.clone());
        }

        let size = params.size.max(1.0);
        let complexity = params.complexity.round().clamp(1.0, 6.0) as u32;
        let evolution = params.evolution;

        Ok(displace_canvas(input, amount, size, complexity, evolution))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TurbulentDisplaceParams {
    pub amount: f32,
    pub size: f32,
    pub complexity: f32,
    pub evolution: f32,
}

impl TurbulentDisplaceParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            amount: param_f32_any(params, &["amount", "Amount", "0002"], 0.0),
            size: param_f32_any(params, &["size", "Size", "0003"], 100.0),
            complexity: param_f32_any(params, &["complexity", "Complexity", "0005"], 2.0),
            evolution: param_f32_at_any(
                params,
                &["evolution", "Evolution", "0006"],
                time,
                time as f32 * 45.0,
            ),
        }
    }
}

fn displace_canvas(
    input: &Canvas,
    amount: f32,
    size: f32,
    complexity: u32,
    evolution: f32,
) -> Canvas {
    let mut output = Canvas::transparent(input.width, input.height);
    let amplitude = amount * 0.25;
    let phase = evolution.to_radians();

    for y in 0..input.height {
        for x in 0..input.width {
            let nx = x as f32 / size;
            let ny = y as f32 / size;
            let dx = turbulence(nx + 17.0, ny, phase, complexity) * amplitude;
            let dy = turbulence(nx, ny + 29.0, phase + 1.7, complexity) * amplitude;
            let sx = (x as f32 + dx).round();
            let sy = (y as f32 + dy).round();
            if sx < 0.0 || sy < 0.0 || sx >= input.width as f32 || sy >= input.height as f32 {
                continue;
            }
            output.set_pixel(x, y, input.pixel(sx as u32, sy as u32));
        }
    }

    output
}

fn turbulence(x: f32, y: f32, phase: f32, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut total = 0.0;
    for octave in 0..octaves {
        let angle =
            x * 12.9898 * frequency + y * 78.233 * frequency + phase + octave as f32 * 4.123;
        value += angle.sin() * amplitude;
        total += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    (value / total).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn zero_amount_is_pass_through() {
        let input = Canvas::new(2, 1, [1, 2, 3, 4]);

        let output = TurbulentDisplace::default()
            .render(
                &input,
                &EffectContext {
                    time: 1.0,
                    fps: 30.0,
                },
                &json!({ "amount": 0 }),
            )
            .unwrap();

        assert_eq!(output.data, input.data);
    }

    #[test]
    fn displacement_is_deterministic_and_moves_pixels() {
        let mut input = Canvas::transparent(5, 1);
        for x in 0..5 {
            input.set_pixel(x, 0, [x as u8 * 50, 0, 0, 255]);
        }
        let ctx = EffectContext {
            time: 0.0,
            fps: 30.0,
        };
        let params = json!({ "amount": 16, "size": 3, "complexity": 2, "evolution": 0 });

        let first = TurbulentDisplace::default()
            .render(&input, &ctx, &params)
            .unwrap();
        let second = TurbulentDisplace::default()
            .render(&input, &ctx, &params)
            .unwrap();

        assert_eq!(first.data, second.data);
        assert_ne!(first.data, input.data);
    }

    #[test]
    fn params_accept_ae_numbered_values() {
        let params = TurbulentDisplaceParams::from_json(
            &json!({
                "0002": { "value": 18 },
                "0003": { "value": 32 },
                "0005": { "value": 4 },
                "0006": { "value": 90 }
            }),
            1.0,
        );

        assert_eq!(params.amount, 18.0);
        assert_eq!(params.size, 32.0);
        assert_eq!(params.complexity, 4.0);
        assert_eq!(params.evolution, 90.0);
    }

    #[test]
    fn params_accept_time_varying_numbered_evolution() {
        let params = TurbulentDisplaceParams::from_json(
            &json!({
                "0006": {
                    "keyframes": [
                        { "t": 0.0, "v": 0.0 },
                        { "t": 2.0, "v": 180.0 }
                    ]
                }
            }),
            1.0,
        );

        assert_eq!(params.evolution, 90.0);
    }
}
