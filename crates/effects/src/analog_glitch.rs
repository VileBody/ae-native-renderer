use crate::{param_f32, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;
use std::f32::consts::TAU;

#[derive(Debug, Default)]
pub struct AnalogGlitch;

impl Effect for AnalogGlitch {
    fn match_name(&self) -> &'static str {
        "ANR Analog Glitch"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let contrast = param_f32(params, "contrast", 1.8).max(0.0);
        let red_gain = param_f32(params, "red_gain", 1.25).max(0.0);
        let green_gain = param_f32(params, "green_gain", 0.046).max(0.0);
        let blue_gain = param_f32(params, "blue_gain", 0.092).max(0.0);
        let tone_gain = param_f32(params, "tone_gain", 1.10).max(0.0);
        let scanline_strength = param_f32(params, "scanline_strength", 0.20).clamp(0.0, 1.0);
        let dot_strength = param_f32(params, "dot_strength", 0.23).clamp(0.0, 1.0);
        let highlight_grid_strength =
            param_f32(params, "highlight_grid_strength", 0.0).clamp(0.0, 1.0);
        let scanline_period = param_f32(params, "scanline_period", 4.0).round().max(1.0) as u32;
        let scanline_on = param_f32(params, "scanline_on", 2.0)
            .round()
            .clamp(1.0, scanline_period as f32) as u32;
        let dot_period = param_f32(params, "dot_period", 3.0).round().max(1.0) as u32;
        let wave_amplitude = param_f32(params, "wave_amplitude", 6.0);
        let wave_width = param_f32(params, "wave_width", 1200.0).max(1.0);
        let phase = ctx.time as f32 * TAU * 0.5;
        let mut output = Canvas::transparent(input.width, input.height);
        let line_on_fraction = scanline_on as f32 / scanline_period as f32;
        let line_mean = line_on_fraction + (1.0 - line_on_fraction) * 0.25;
        let dot_mean = if dot_period == 1 {
            0.45
        } else {
            ((dot_period - 1) as f32 + 0.45) / dot_period as f32
        };

        for y in 0..input.height {
            let wave = ((y as f32 / wave_width) * TAU + phase).sin() * wave_amplitude;
            let hard_line_gain = if y % scanline_period < scanline_on {
                1.0
            } else {
                0.25
            };
            let line_gain = line_mean + (hard_line_gain - line_mean) * scanline_strength;
            for x in 0..input.width {
                let source_x = (x as f32 + wave)
                    .round()
                    .clamp(0.0, input.width.saturating_sub(1) as f32)
                    as u32;
                let source = input.pixel(source_x, y);
                let luma = (source[0] as f32 * 0.2126
                    + source[1] as f32 * 0.7152
                    + source[2] as f32 * 0.0722)
                    / 255.0;
                let contrasted = ((luma - 0.20) * contrast + 0.20).clamp(0.0, 1.0);
                let hard_dot_gain = if x % dot_period == dot_period - 1 {
                    0.45
                } else {
                    1.0
                };
                let dot_gain = dot_mean + (hard_dot_gain - dot_mean) * dot_strength;
                let soft_gain = line_gain * dot_gain;
                let highlight_weight = smoothstep(0.65, 0.95, contrasted) * highlight_grid_strength;
                let grid_gain = highlight_grid_gain(x, y);
                let modulation = soft_gain + (grid_gain - soft_gain) * highlight_weight;
                let level = contrasted * modulation * tone_gain * 255.0;
                output.set_pixel(
                    x,
                    y,
                    [
                        scaled_channel(level, red_gain),
                        scaled_channel(level, green_gain),
                        scaled_channel(level, blue_gain),
                        source[3],
                    ],
                );
            }
        }
        Ok(output)
    }
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn highlight_grid_gain(x: u32, y: u32) -> f32 {
    // The production analog stack resolves to a 4x8 CRT-like lattice on clipped
    // shutter frames. Keep it highlight-selective so ordinary footage retains
    // the softer modulation used by the rest of the shot.
    const ROW: [f32; 4] = [1.0, 0.20, 0.15, 0.85];
    const COLUMN: [f32; 8] = [0.10, 0.0, 0.0, 0.10, 0.40, 0.90, 1.0, 0.55];
    0.23 + 0.29 * ROW[(y % 4) as usize] * COLUMN[(x % 8) as usize]
}

fn scaled_channel(level: f32, gain: f32) -> u8 {
    (level * gain).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glow::Glow;
    use serde_json::json;

    #[test]
    fn output_keeps_subtle_rgb_texture_and_soft_modulation() {
        let input = Canvas::new(6, 4, [180, 150, 120, 255]);
        let output = AnalogGlitch
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 24.0,
                },
                &json!({"wave_amplitude": 0}),
            )
            .unwrap();

        let bright = output.pixel(1, 0);
        let scanline = output.pixel(1, 2);
        let dot = output.pixel(2, 0);

        assert_eq!(bright, [196, 7, 14, 255]);
        assert_eq!(scanline, [154, 6, 11, 255]);
        assert_eq!(dot, [167, 6, 12, 255]);
        assert!(scanline[0] < bright[0]);
        assert!(dot[0] < bright[0]);
        assert!(scanline[0] as f32 >= bright[0] as f32 * 0.75);
        assert!(dot[0] as f32 >= bright[0] as f32 * 0.80);
    }

    #[test]
    fn clipped_highlights_use_the_production_four_by_eight_grid_without_clamping() {
        let input = Canvas::new(16, 4, [255, 255, 255, 255]);
        let output = AnalogGlitch
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 24.0,
                },
                &json!({
                    "contrast": 1.3,
                    "red_gain": 1.25,
                    "tone_gain": 1.1,
                    "wave_amplitude": 0,
                    "highlight_grid_strength": 1.0
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(6, 0), output.pixel(14, 0));
        assert!(output.pixel(6, 0)[0] > output.pixel(1, 0)[0]);
        assert!(output.pixel(6, 0)[0] > output.pixel(6, 1)[0]);
        assert!(output.pixel(6, 0)[0] < 250);
    }

    #[test]
    fn clipped_highlight_grid_survives_the_production_glow_stack() {
        let ctx = EffectContext {
            time: 1.0,
            fps: 23.976,
        };
        let input = Canvas::new(128, 128, [255, 255, 255, 255]);
        let analog = AnalogGlitch
            .render(
                &input,
                &ctx,
                &json!({
                    "contrast": 1.3,
                    "red_gain": 1.25,
                    "scanline_period": 4,
                    "scanline_on": 2,
                    "dot_period": 8,
                    "highlight_grid_strength": 1.0,
                    "wave_amplitude": 0,
                    "wave_width": 1200
                }),
            )
            .unwrap();
        let low_glow = Glow
            .render(
                &analog,
                &ctx,
                &json!({
                    "based_on": "color channels",
                    "threshold": 12.75,
                    "radius": 17.0,
                    "intensity": 0.55,
                    "operation": "add",
                    "composite_original": "on top"
                }),
            )
            .unwrap();
        let output = Glow
            .render(
                &low_glow,
                &ctx,
                &json!({
                    "based_on": "color channels",
                    "threshold": 125.0,
                    "radius": 17.0,
                    "intensity": 0.35,
                    "operation": "add",
                    "composite_original": "on top"
                }),
            )
            .unwrap();

        let trough = output.pixel(65, 64)[0];
        let peak = output.pixel(70, 64)[0];
        assert!(
            (220..=245).contains(&peak),
            "highlight peak should retain texture, got peak={peak}, floor={trough}"
        );
        assert!(
            (130..=150).contains(&trough),
            "highlight floor should remain red, got floor={trough}, peak={peak}"
        );
    }
}
