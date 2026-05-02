use crate::{param_f32_at, Effect};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct Geometry2;

impl Effect for Geometry2 {
    fn match_name(&self) -> &'static str {
        "ADBE Geometry2"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &crate::EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        Ok(transform_canvas(
            input,
            Geometry2Params::from_json(input, params, _ctx.time),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Geometry2Params {
    anchor: (f32, f32),
    position: (f32, f32),
    scale: (f32, f32),
    rotation: f32,
}

impl Geometry2Params {
    fn identity(input: &Canvas) -> Self {
        let center = (input.width as f32 * 0.5, input.height as f32 * 0.5);
        Self {
            anchor: center,
            position: center,
            scale: (100.0, 100.0),
            rotation: 0.0,
        }
    }

    pub(crate) fn from_json(input: &Canvas, params: &Value, time: f64) -> Self {
        let mut transform = Self::identity(input);
        transform.anchor = point_param(params, &["anchor", "anchorPoint", "Anchor Point", "0001"])
            .unwrap_or(transform.anchor);
        transform.position =
            point_param(params, &["position", "Position", "0002"]).unwrap_or(transform.position);
        transform.scale = scale_param(params, time).unwrap_or(transform.scale);
        transform.rotation = scalar_param(params, &["rotation", "Rotation"], time, 0.0);
        transform
    }

    fn is_identity(self) -> bool {
        nearly_eq(self.anchor.0, self.position.0)
            && nearly_eq(self.anchor.1, self.position.1)
            && nearly_eq(self.scale.0, 100.0)
            && nearly_eq(self.scale.1, 100.0)
            && nearly_eq(self.rotation, 0.0)
    }
}

fn transform_canvas(input: &Canvas, transform: Geometry2Params) -> Canvas {
    if input.width == 0 || input.height == 0 || transform.is_identity() {
        return input.clone();
    }

    let mut output = Canvas::transparent(input.width, input.height);
    let radians = (-transform.rotation).to_radians();
    let (sin, cos) = radians.sin_cos();
    let sx = if transform.scale.0.abs() < f32::EPSILON {
        1.0
    } else {
        transform.scale.0 / 100.0
    };
    let sy = if transform.scale.1.abs() < f32::EPSILON {
        1.0
    } else {
        transform.scale.1 / 100.0
    };

    for y in 0..input.height {
        for x in 0..input.width {
            let dx = x as f32 - transform.position.0;
            let dy = y as f32 - transform.position.1;
            let rx = dx * cos - dy * sin;
            let ry = dx * sin + dy * cos;
            let source_x = transform.anchor.0 + rx / sx;
            let source_y = transform.anchor.1 + ry / sy;
            let sx = source_x.round() as i32;
            let sy = source_y.round() as i32;
            if sx < 0 || sy < 0 || sx >= input.width as i32 || sy >= input.height as i32 {
                continue;
            }
            output.set_pixel(x, y, input.pixel(sx as u32, sy as u32));
        }
    }

    output
}

fn scale_param(params: &Value, time: f64) -> Option<(f32, f32)> {
    if let Some(point) = point_param(params, &["scale", "Scale"]) {
        return Some(point);
    }
    if has_param(params, "0003") {
        let scale = param_f32_at(params, "0003", time, 100.0);
        return Some((scale, scale));
    }
    match (
        scalar_param_opt(params, &["scaleX", "Scale Width", "0004"], time),
        scalar_param_opt(params, &["scaleY", "Scale Height", "0008"], time),
    ) {
        (Some(x), Some(y)) => Some((x, y)),
        (Some(x), None) => Some((x, x)),
        (None, Some(y)) => Some((y, y)),
        (None, None) => None,
    }
}

fn point_param(params: &Value, names: &[&str]) -> Option<(f32, f32)> {
    for name in names {
        let Some(raw_value) = params.get(*name) else {
            continue;
        };
        let Some(value) = nested_value(raw_value) else {
            continue;
        };
        if let Some(values) = value.as_array() {
            let x = values.first().and_then(Value::as_f64)? as f32;
            let y = values.get(1).and_then(Value::as_f64)? as f32;
            return Some((x, y));
        }
        if let (Some(x), Some(y)) = (
            value.get("x").and_then(Value::as_f64),
            value.get("y").and_then(Value::as_f64),
        ) {
            return Some((x as f32, y as f32));
        }
    }
    None
}

fn scalar_param(params: &Value, names: &[&str], time: f64, default: f32) -> f32 {
    scalar_param_opt(params, names, time).unwrap_or(default)
}

fn scalar_param_opt(params: &Value, names: &[&str], time: f64) -> Option<f32> {
    for name in names {
        if !has_param(params, name) {
            continue;
        };
        return Some(param_f32_at(params, name, time, 0.0));
    }
    None
}

fn nested_value(value: &Value) -> Option<&Value> {
    Some(value.get("value").unwrap_or(value))
}

fn has_param(params: &Value, name: &str) -> bool {
    params.get(name).is_some()
}

fn nearly_eq(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.001
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EffectContext;
    use serde_json::json;

    #[test]
    fn geometry_offsets_position_around_anchor() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [10, 20, 30, 255]);

        let output = Geometry2::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({
                    "anchor": [1, 0],
                    "position": [2, 0]
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(2, 0), [10, 20, 30, 255]);
        assert_eq!(output.pixel(1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn params_accept_ae_numbered_transform_values() {
        let input = Canvas::transparent(10, 8);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0001": { "value": [1, 2] },
                "0002": { "value": { "x": 3, "y": 4 } },
                "0004": { "value": 125 },
                "0008": { "value": 80 },
                "rotation": { "value": 15 }
            }),
            0.0,
        );

        assert_eq!(params.anchor, (1.0, 2.0));
        assert_eq!(params.position, (3.0, 4.0));
        assert_eq!(params.scale, (125.0, 80.0));
        assert_eq!(params.rotation, 15.0);
    }

    #[test]
    fn params_accept_time_varying_numbered_scale() {
        let input = Canvas::transparent(4, 4);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0003": {
                    "keyframes": [
                        { "t": 0.0, "v": 100.0 },
                        { "t": 1.0, "v": 200.0 }
                    ]
                }
            }),
            0.5,
        );

        assert_eq!(params.scale, (150.0, 150.0));
    }
}
