pub mod analog_glitch;
pub mod box_blur;
pub mod directional_blur;
pub mod drop_shadow;
pub mod f3_stylize;
pub mod gaussian_blur;
pub mod geometry;
pub mod glow;
pub mod image_wipe;
pub mod invert;
pub mod layer_masks;
pub mod minimax;
pub mod optics_compensation;
pub mod posterize_time;
pub mod registry;
pub mod shape_overlay;
pub mod turbulent_displace;
pub mod vertical_gradient;

pub use registry::*;

use raster_cpu::Canvas;
use serde_json::Value;
use transform_math::CubicBezier;

#[derive(Debug, Clone)]
pub struct EffectContext {
    pub time: f64,
    pub fps: f64,
}

pub trait Effect: Send + Sync {
    fn match_name(&self) -> &'static str;

    fn render(&self, input: &Canvas, ctx: &EffectContext, params: &Value)
        -> anyhow::Result<Canvas>;

    fn render_into(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
        output: &mut Canvas,
    ) -> anyhow::Result<()> {
        *output = self.render(input, ctx, params)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EffectStatus {
    Stub,
    Approximate,
    Production,
}

pub(crate) fn param_f32(params: &Value, name: &str, default: f32) -> f32 {
    param_value(params, name)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .unwrap_or(default)
}

pub(crate) fn param_f32_any(params: &Value, names: &[&str], default: f32) -> f32 {
    for name in names {
        if params.get(*name).is_some() {
            return param_f32(params, name, default);
        }
    }
    default
}

pub(crate) fn param_f32_at(params: &Value, name: &str, time: f64, default: f32) -> f32 {
    let Some(raw) = params.get(name) else {
        return default;
    };
    if let Some(expression) = raw.get("expression").and_then(Value::as_str) {
        if expression.trim() == "time*500" {
            return (time * 500.0) as f32;
        }
    }
    if let Some(value) = raw.get("value").or(Some(raw)).and_then(Value::as_f64) {
        return value as f32;
    }
    let Some(keyframes) = raw.get("keyframes").and_then(Value::as_array) else {
        return default;
    };
    evaluate_scalar_keyframes(keyframes, time).unwrap_or(default)
}

pub(crate) fn param_f32_at_any(params: &Value, names: &[&str], time: f64, default: f32) -> f32 {
    for name in names {
        if params.get(*name).is_some() {
            return param_f32_at(params, name, time, default);
        }
    }
    default
}

pub(crate) fn param_bool(params: &Value, name: &str, default: bool) -> bool {
    param_value(params, name)
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_f64().map(|number| number.abs() > f64::EPSILON))
        })
        .unwrap_or(default)
}

pub(crate) fn param_bool_any(params: &Value, names: &[&str], default: bool) -> bool {
    for name in names {
        if params.get(*name).is_some() {
            return param_bool(params, name, default);
        }
    }
    default
}

pub(crate) fn param_rgba(params: &Value, name: &str, default: [u8; 4]) -> [u8; 4] {
    let Some(value) = param_value(params, name) else {
        return default;
    };
    let Some(values) = value.as_array() else {
        return default;
    };
    let channel = |idx: usize, fallback: u8| -> u8 {
        values
            .get(idx)
            .and_then(Value::as_f64)
            .map(|value| {
                let scaled = if value <= 1.0 { value * 255.0 } else { value };
                scaled.round().clamp(0.0, 255.0) as u8
            })
            .unwrap_or(fallback)
    };
    [
        channel(0, default[0]),
        channel(1, default[1]),
        channel(2, default[2]),
        channel(3, default[3]),
    ]
}

pub(crate) fn param_rgba_any(params: &Value, names: &[&str], default: [u8; 4]) -> [u8; 4] {
    for name in names {
        if param_value(params, name)
            .and_then(Value::as_array)
            .is_some()
        {
            return param_rgba(params, name, default);
        }
    }
    default
}

pub(crate) fn param_value<'a>(params: &'a Value, name: &str) -> Option<&'a Value> {
    let value = params.get(name)?;
    value.get("value").or(Some(value))
}

fn evaluate_scalar_keyframes(keyframes: &[Value], time: f64) -> Option<f32> {
    if keyframes.is_empty() {
        return None;
    }
    let value_at = |key: &Value| {
        key.get("v")
            .or_else(|| key.get("value"))
            .and_then(Value::as_f64)
            .map(|value| value as f32)
    };
    let time_at = |key: &Value| {
        key.get("t")
            .or_else(|| key.get("time"))
            .and_then(Value::as_f64)
    };
    let ease_at = |key: &Value| {
        let ease = key.get("ease")?;
        Some(CubicBezier::new(
            ease.get("x1")?.as_f64()? as f32,
            ease.get("y1")?.as_f64()? as f32,
            ease.get("x2")?.as_f64()? as f32,
            ease.get("y2")?.as_f64()? as f32,
        ))
    };
    if time <= time_at(&keyframes[0])? {
        return value_at(&keyframes[0]);
    }
    for pair in keyframes.windows(2) {
        let a_time = time_at(&pair[0])?;
        let b_time = time_at(&pair[1])?;
        if time <= b_time {
            let a_value = value_at(&pair[0])?;
            let b_value = value_at(&pair[1])?;
            if b_time <= a_time {
                return Some(a_value);
            }
            let linear_t = ((time - a_time) / (b_time - a_time)).clamp(0.0, 1.0) as f32;
            let t = ease_at(&pair[0])
                .map(|ease| ease.ease(linear_t))
                .unwrap_or(linear_t);
            return Some(a_value + (b_value - a_value) * t);
        }
    }
    keyframes.last().and_then(value_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scalar_keyframes_accept_short_and_long_names_with_linear_interpolation() {
        let short = json!({
            "amount": {"keyframes": [{"t": 0.0, "v": 0.0}, {"t": 1.0, "v": 1.0}]}
        });
        let long = json!({
            "amount": {
                "keyframes": [
                    {"time": 0.0, "value": 0.0},
                    {"time": 1.0, "value": 1.0}
                ]
            }
        });

        assert_eq!(param_f32_at(&short, "amount", 0.25, -1.0), 0.25);
        assert_eq!(param_f32_at(&long, "amount", 0.25, -1.0), 0.25);
    }

    #[test]
    fn scalar_keyframes_apply_optional_cubic_ease() {
        let params = json!({
            "amount": {
                "keyframes": [
                    {
                        "time": 0.0,
                        "value": 0.0,
                        "ease": {"x1": 1.0 / 3.0, "y1": 0.0, "x2": 2.0 / 3.0, "y2": 1.0}
                    },
                    {"time": 1.0, "value": 1.0}
                ]
            }
        });

        let value = param_f32_at(&params, "amount", 0.25, -1.0);
        assert!((value - 0.15625).abs() < 1.0e-5);
    }

    #[test]
    fn default_render_into_writes_the_supplied_canvas() {
        let effect = crate::registry::EffectRegistry::create("ADBE Invert").unwrap();
        let input = Canvas::new(1, 1, [10, 20, 30, 40]);
        let mut output = Canvas::transparent(0, 0);

        effect
            .render_into(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 24.0,
                },
                &json!({}),
                &mut output,
            )
            .unwrap();

        assert_eq!(output.pixel(0, 0), [245, 235, 225, 40]);
    }
}
