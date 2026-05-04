use crate::{ExprContext, ExprValue, ExpressionEvaluator, NamedPatternExpressionEvaluator};
use anyhow::{bail, Context};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyValueType {
    Any,
    Scalar,
    Vector2,
    Vector3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyExpressionMode {
    ParsedSubset,
    ValueAtTimeHost,
    NamedFingerprintFallback,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyExpressionResult {
    pub value: ExprValue,
    pub value_type: PropertyValueType,
    pub mode: PropertyExpressionMode,
}

#[derive(Debug, Clone, Default)]
pub struct CompExpressionContext {
    pub id: String,
    pub width: f64,
    pub height: f64,
    pub duration: f64,
    pub frame_duration: f64,
}

#[derive(Debug, Clone, Default)]
pub struct LayerExpressionContext {
    pub id: String,
    pub index: Option<usize>,
    pub in_point: f64,
    pub out_point: f64,
    pub start_time: f64,
}

#[derive(Debug, Clone)]
pub struct PropertyExpressionContext {
    pub time: f64,
    pub value: ExprValue,
    pub comp: CompExpressionContext,
    pub layer: LayerExpressionContext,
    pub vars: HashMap<String, ExprValue>,
}

impl PropertyExpressionContext {
    pub fn to_expr_context(&self) -> ExprContext {
        let mut vars = self.vars.clone();
        vars.entry("thisComp.width".to_string())
            .or_insert(ExprValue::Number(self.comp.width));
        vars.entry("thisComp.height".to_string())
            .or_insert(ExprValue::Number(self.comp.height));
        vars.entry("thisComp.duration".to_string())
            .or_insert(ExprValue::Number(self.comp.duration));
        vars.entry("thisComp.frameDuration".to_string())
            .or_insert(ExprValue::Number(self.comp.frame_duration));
        vars.entry("frameDuration".to_string())
            .or_insert(ExprValue::Number(self.comp.frame_duration));
        vars.entry("thisLayer.inPoint".to_string())
            .or_insert(ExprValue::Number(self.layer.in_point));
        vars.entry("thisLayer.outPoint".to_string())
            .or_insert(ExprValue::Number(self.layer.out_point));
        vars.entry("thisLayer.startTime".to_string())
            .or_insert(ExprValue::Number(self.layer.start_time));
        if let Some(index) = self.layer.index {
            vars.entry("thisLayer.index".to_string())
                .or_insert(ExprValue::Number(index as f64));
        }

        ExprContext {
            time: self.time,
            in_point: self.layer.in_point,
            out_point: self.layer.out_point,
            value: Some(self.value.clone()),
            vars,
            ..ExprContext::default()
        }
    }
}

impl Default for PropertyExpressionContext {
    fn default() -> Self {
        Self {
            time: 0.0,
            value: ExprValue::Number(0.0),
            comp: CompExpressionContext::default(),
            layer: LayerExpressionContext::default(),
            vars: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PropertyExpressionRequest<'a> {
    pub source: &'a str,
    pub property_path: &'a str,
    pub target_type: PropertyValueType,
    pub fingerprint: Option<&'a str>,
    pub context: PropertyExpressionContext,
}

pub trait PropertyExpressionHost {
    fn value_at_time(
        &self,
        _property_path: &str,
        _time: f64,
        _ctx: &PropertyExpressionContext,
    ) -> anyhow::Result<Option<ExprValue>> {
        Ok(None)
    }

    fn named_fingerprint_value(
        &self,
        _fingerprint: &str,
        _property_path: &str,
        _ctx: &PropertyExpressionContext,
    ) -> anyhow::Result<Option<ExprValue>> {
        Ok(None)
    }
}

#[derive(Debug, Default)]
pub struct NoopPropertyExpressionHost;

impl PropertyExpressionHost for NoopPropertyExpressionHost {}

pub trait PropertyExpressionEvaluator: Send + Sync {
    fn eval_property(
        &self,
        request: &PropertyExpressionRequest<'_>,
        host: &dyn PropertyExpressionHost,
    ) -> anyhow::Result<PropertyExpressionResult>;
}

#[derive(Debug, Default)]
pub struct BoundaryPropertyExpressionEvaluator<E = NamedPatternExpressionEvaluator> {
    fallback: E,
}

impl<E> BoundaryPropertyExpressionEvaluator<E> {
    pub fn new(fallback: E) -> Self {
        Self { fallback }
    }
}

impl<E> PropertyExpressionEvaluator for BoundaryPropertyExpressionEvaluator<E>
where
    E: ExpressionEvaluator,
{
    fn eval_property(
        &self,
        request: &PropertyExpressionRequest<'_>,
        host: &dyn PropertyExpressionHost,
    ) -> anyhow::Result<PropertyExpressionResult> {
        let expr_ctx = request.context.to_expr_context();

        if let Some(arg_source) = standalone_call_arg(request.source, "valueAtTime") {
            let sample_time = self
                .fallback
                .eval(arg_source, &expr_ctx)
                .and_then(coerce_number)
                .context("valueAtTime argument must evaluate to a scalar time")?;
            let Some(value) =
                host.value_at_time(request.property_path, sample_time, &request.context)?
            else {
                bail!(
                    "BLOCKER: valueAtTime requires a property host for `{}` at time {}",
                    request.property_path,
                    sample_time
                );
            };
            return Ok(PropertyExpressionResult {
                value: coerce_value(value, request.target_type)?,
                value_type: request.target_type,
                mode: PropertyExpressionMode::ValueAtTimeHost,
            });
        }

        match self.fallback.eval(request.source, &expr_ctx) {
            Ok(value) => Ok(PropertyExpressionResult {
                value: coerce_value(value, request.target_type)?,
                value_type: request.target_type,
                mode: PropertyExpressionMode::ParsedSubset,
            }),
            Err(err) => {
                if let Some(fingerprint) = request.fingerprint {
                    if let Some(value) = host.named_fingerprint_value(
                        fingerprint,
                        request.property_path,
                        &request.context,
                    )? {
                        return Ok(PropertyExpressionResult {
                            value: coerce_value(value, request.target_type)?,
                            value_type: request.target_type,
                            mode: PropertyExpressionMode::NamedFingerprintFallback,
                        });
                    }
                }
                Err(err).with_context(|| {
                    format!(
                        "property expression `{}` is outside the native subset",
                        request.property_path
                    )
                })
            }
        }
    }
}

pub fn coerce_value(value: ExprValue, target_type: PropertyValueType) -> anyhow::Result<ExprValue> {
    match target_type {
        PropertyValueType::Any => Ok(value),
        PropertyValueType::Scalar => match value {
            ExprValue::Number(_) => Ok(value),
            ExprValue::Array(values) if values.len() == 1 => Ok(ExprValue::Number(values[0])),
            other => bail!("cannot coerce {other:?} to scalar without property-specific AE rules"),
        },
        PropertyValueType::Vector2 => {
            let vector = match value {
                ExprValue::Number(value) => [value, value],
                ExprValue::Vec2(value) => value,
                ExprValue::Vec3(value) => [value[0], value[1]],
                ExprValue::Array(values) if values.len() >= 2 => [values[0], values[1]],
                other => bail!("cannot coerce {other:?} to Vec2"),
            };
            Ok(ExprValue::Vec2(vector))
        }
        PropertyValueType::Vector3 => {
            let vector = match value {
                ExprValue::Number(value) => [value, value, value],
                ExprValue::Vec2(value) => [value[0], value[1], 0.0],
                ExprValue::Vec3(value) => value,
                ExprValue::Array(values) if values.len() >= 3 => [values[0], values[1], values[2]],
                other => bail!("cannot coerce {other:?} to Vec3"),
            };
            Ok(ExprValue::Vec3(vector))
        }
    }
}

fn coerce_number(value: ExprValue) -> anyhow::Result<f64> {
    match coerce_value(value, PropertyValueType::Scalar)? {
        ExprValue::Number(value) => Ok(value),
        _ => unreachable!("scalar coercion only returns numbers"),
    }
}

fn standalone_call_arg<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let source = source.trim().trim_end_matches(';').trim();
    let rest = source.strip_prefix(name)?.trim_start();
    let rest = rest.strip_prefix('(')?;
    let mut depth = 1_i32;
    for (index, ch) in rest.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let tail = rest[index + ch.len_utf8()..].trim();
                    return tail.is_empty().then_some(rest[..index].trim());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct TestHost;

    impl PropertyExpressionHost for TestHost {
        fn value_at_time(
            &self,
            property_path: &str,
            time: f64,
            _ctx: &PropertyExpressionContext,
        ) -> anyhow::Result<Option<ExprValue>> {
            if property_path == "transform.position" && (time - 0.25).abs() < 1.0e-9 {
                return Ok(Some(ExprValue::Vec2([3.0, 4.0])));
            }
            Ok(None)
        }

        fn named_fingerprint_value(
            &self,
            fingerprint: &str,
            property_path: &str,
            _ctx: &PropertyExpressionContext,
        ) -> anyhow::Result<Option<ExprValue>> {
            if fingerprint == "edge_wobble:test" && property_path == "transform.position" {
                return Ok(Some(ExprValue::Vec2([7.0, 8.0])));
            }
            Ok(None)
        }
    }

    fn request<'a>(source: &'a str) -> PropertyExpressionRequest<'a> {
        PropertyExpressionRequest {
            source,
            property_path: "transform.position",
            target_type: PropertyValueType::Vector2,
            fingerprint: None,
            context: PropertyExpressionContext {
                time: 0.5,
                value: ExprValue::Vec2([1.0, 2.0]),
                comp: CompExpressionContext {
                    width: 10.0,
                    height: 20.0,
                    duration: 2.0,
                    frame_duration: 0.25,
                    ..CompExpressionContext::default()
                },
                layer: LayerExpressionContext {
                    index: Some(4),
                    in_point: 3.0,
                    out_point: 5.0,
                    start_time: 2.5,
                    ..LayerExpressionContext::default()
                },
                vars: HashMap::new(),
            },
        }
    }

    fn evaluator() -> BoundaryPropertyExpressionEvaluator<NamedPatternExpressionEvaluator> {
        BoundaryPropertyExpressionEvaluator::default()
    }

    #[test]
    fn property_subset_seeds_this_comp_this_layer_and_coerces_value() {
        let evaluator = evaluator();
        let request = request("value + [thisComp.width / 2, thisLayer.inPoint]");

        let result = evaluator
            .eval_property(&request, &NoopPropertyExpressionHost)
            .unwrap();

        assert_eq!(result.mode, PropertyExpressionMode::ParsedSubset);
        assert_eq!(result.value, ExprValue::Vec2([6.0, 5.0]));
    }

    #[test]
    fn value_at_time_uses_property_host_boundary() {
        let evaluator = evaluator();
        let request = request("valueAtTime(time - thisComp.frameDuration)");

        let result = evaluator.eval_property(&request, &TestHost).unwrap();

        assert_eq!(result.mode, PropertyExpressionMode::ValueAtTimeHost);
        assert_eq!(result.value, ExprValue::Vec2([3.0, 4.0]));
    }

    #[test]
    fn value_at_time_without_host_is_explicit_blocker() {
        let evaluator = evaluator();
        let request = request("valueAtTime(0.25)");

        let err = evaluator
            .eval_property(&request, &NoopPropertyExpressionHost)
            .unwrap_err();

        assert!(err.to_string().contains("BLOCKER: valueAtTime requires"));
    }

    #[test]
    fn named_fingerprint_fallback_is_host_scoped() {
        let evaluator = evaluator();
        let mut request = request("edge_wobble()");
        request.fingerprint = Some("edge_wobble:test");

        let result = evaluator.eval_property(&request, &TestHost).unwrap();

        assert_eq!(
            result.mode,
            PropertyExpressionMode::NamedFingerprintFallback
        );
        assert_eq!(result.value, ExprValue::Vec2([7.0, 8.0]));
    }

    #[test]
    fn scalar_output_rejects_implicit_vector_pick() {
        let err = coerce_value(ExprValue::Vec2([1.0, 2.0]), PropertyValueType::Scalar).unwrap_err();

        assert!(err.to_string().contains("property-specific AE rules"));
    }
}
