pub mod box_blur;
pub mod drop_shadow;
pub mod geometry;
pub mod glow;
pub mod minimax;
pub mod posterize_time;
pub mod registry;
pub mod turbulent_displace;

pub use registry::*;

use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct EffectContext {
    pub time: f64,
    pub fps: f64,
}

pub trait Effect: Send + Sync {
    fn match_name(&self) -> &'static str;

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas>;
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

pub(crate) fn param_bool(params: &Value, name: &str, default: bool) -> bool {
    param_value(params, name)
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_f64().map(|number| number.abs() > f64::EPSILON))
        })
        .unwrap_or(default)
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

fn param_value<'a>(params: &'a Value, name: &str) -> Option<&'a Value> {
    let value = params.get(name)?;
    value.get("value").or(Some(value))
}

fn evaluate_scalar_keyframes(keyframes: &[Value], time: f64) -> Option<f32> {
    if keyframes.is_empty() {
        return None;
    }
    let value_at = |key: &Value| key.get("v").and_then(Value::as_f64).map(|value| value as f32);
    let time_at = |key: &Value| key.get("t").and_then(Value::as_f64);
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
            let t = ((time - a_time) / (b_time - a_time)).clamp(0.0, 1.0) as f32;
            return Some(a_value + (b_value - a_value) * t);
        }
    }
    keyframes.last().and_then(value_at)
}
