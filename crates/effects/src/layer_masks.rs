use crate::{Effect, EffectContext};
use raster_cpu::Canvas;
use rayon::prelude::*;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct LayerMasks;

impl Effect for LayerMasks {
    fn match_name(&self) -> &'static str {
        "ANR Layer Masks"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let masks = params
            .get("masks")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("ANR Layer Masks requires params.masks[]"))?;
        let parsed = masks
            .iter()
            .map(parse_mask)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut output = input.clone();
        let width = input.width.max(1) as usize;
        output
            .data
            .par_chunks_exact_mut(4)
            .enumerate()
            .for_each(|(index, pixel)| {
                let point = [(index % width) as f32 + 0.5, (index / width) as f32 + 0.5];
                let mut coverage: f32 = if parsed.first().is_some_and(|mask| {
                    matches!(mask.mode, MaskMode::Subtract | MaskMode::Intersect)
                }) {
                    1.0
                } else {
                    0.0
                };
                for mask in &parsed {
                    let mut value = mask.shape.coverage(point);
                    if mask.inverted {
                        value = 1.0 - value;
                    }
                    value *= mask.opacity;
                    coverage = match mask.mode {
                        MaskMode::Add => coverage.max(value),
                        MaskMode::Subtract => coverage * (1.0 - value),
                        MaskMode::Intersect => coverage.min(value),
                        MaskMode::Difference => (coverage - value).abs(),
                    };
                }
                pixel[3] = (pixel[3] as f32 * coverage.clamp(0.0, 1.0))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            });
        Ok(output)
    }
}

#[derive(Debug, Clone, Copy)]
enum MaskMode {
    Add,
    Subtract,
    Intersect,
    Difference,
}

#[derive(Debug)]
struct Mask {
    mode: MaskMode,
    shape: MaskShape,
    opacity: f32,
    inverted: bool,
}

#[derive(Debug)]
enum MaskShape {
    Rect([f32; 4]),
    Ellipse([f32; 4]),
    Polygon(Vec<[f32; 2]>),
}

impl MaskShape {
    fn coverage(&self, point: [f32; 2]) -> f32 {
        let inside = match self {
            MaskShape::Rect([x, y, w, h]) => {
                point[0] >= *x && point[0] <= *x + *w && point[1] >= *y && point[1] <= *y + *h
            }
            MaskShape::Ellipse([x, y, w, h]) => {
                let rx = (*w * 0.5).max(f32::EPSILON);
                let ry = (*h * 0.5).max(f32::EPSILON);
                let dx = (point[0] - (*x + rx)) / rx;
                let dy = (point[1] - (*y + ry)) / ry;
                dx * dx + dy * dy <= 1.0
            }
            MaskShape::Polygon(vertices) => point_in_polygon(point, vertices),
        };
        if inside {
            1.0
        } else {
            0.0
        }
    }
}

fn parse_mask(value: &Value) -> anyhow::Result<Mask> {
    let mode = match value
        .get("mode")
        .or_else(|| value.get("maskMode"))
        .and_then(Value::as_str)
        .unwrap_or("add")
        .to_ascii_lowercase()
        .as_str()
    {
        "add" => MaskMode::Add,
        "subtract" => MaskMode::Subtract,
        "intersect" => MaskMode::Intersect,
        "difference" => MaskMode::Difference,
        other => anyhow::bail!("unsupported mask mode {other}"),
    };
    let shape_value = value
        .get("shape")
        .or_else(|| value.get("maskShape"))
        .unwrap_or(value);
    let shape_value = shape_value.get("value").unwrap_or(shape_value);
    let shape = if let Some(rect) = four_numbers(shape_value.get("rect").unwrap_or(shape_value)) {
        let kind = value
            .get("shapeType")
            .or_else(|| shape_value.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("rect");
        if kind.eq_ignore_ascii_case("ellipse") {
            MaskShape::Ellipse(rect)
        } else {
            MaskShape::Rect(rect)
        }
    } else if let Some(vertices) = shape_value.get("vertices").and_then(Value::as_array) {
        let vertices = vertices.iter().filter_map(two_numbers).collect::<Vec<_>>();
        if vertices.len() < 3 {
            anyhow::bail!("polygon mask requires at least three vertices");
        }
        MaskShape::Polygon(vertices)
    } else {
        anyhow::bail!("mask requires rect [x,y,w,h] or shape.vertices");
    };
    let raw_opacity = value.get("opacity").and_then(number_value).unwrap_or(100.0);
    Ok(Mask {
        mode,
        shape,
        opacity: if raw_opacity <= 1.0 {
            raw_opacity
        } else {
            raw_opacity / 100.0
        }
        .clamp(0.0, 1.0),
        inverted: value
            .get("inverted")
            .or_else(|| value.get("invert"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn number_value(value: &Value) -> Option<f32> {
    value
        .as_f64()
        .or_else(|| value.get("value").and_then(Value::as_f64))
        .map(|number| number as f32)
}

fn four_numbers(value: &Value) -> Option<[f32; 4]> {
    let values = value.as_array()?;
    Some([
        number_value(values.first()?)?,
        number_value(values.get(1)?)?,
        number_value(values.get(2)?)?,
        number_value(values.get(3)?)?,
    ])
}

fn two_numbers(value: &Value) -> Option<[f32; 2]> {
    let values = value.as_array()?;
    Some([
        number_value(values.first()?)?,
        number_value(values.get(1)?)?,
    ])
}

fn point_in_polygon(point: [f32; 2], vertices: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut previous = vertices.len() - 1;
    for current in 0..vertices.len() {
        let a = vertices[current];
        let b = vertices[previous];
        if ((a[1] > point[1]) != (b[1] > point[1]))
            && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]).max(f32::EPSILON) + a[0]
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rectangle_and_subtract_masks_modify_alpha_in_order() {
        let input = Canvas::new(8, 8, [255, 0, 0, 255]);
        let output = LayerMasks
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 24.0,
                },
                &json!({"masks": [
                    {"mode": "add", "rect": [1, 1, 6, 6]},
                    {"mode": "subtract", "rect": [3, 3, 2, 2]}
                ]}),
            )
            .unwrap();
        assert_eq!(output.pixel(0, 0)[3], 0);
        assert_eq!(output.pixel(2, 2)[3], 255);
        assert_eq!(output.pixel(3, 3)[3], 0);
    }
}
