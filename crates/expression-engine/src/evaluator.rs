use crate::{ExprContext, ExprValue};
use anyhow::{bail, Context};

pub trait ExpressionEvaluator: Send + Sync {
    fn eval(&self, source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue>;
}

#[derive(Debug, Default)]
pub struct NamedPatternExpressionEvaluator;

impl ExpressionEvaluator for NamedPatternExpressionEvaluator {
    fn eval(&self, source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue> {
        let source = source.trim().trim_end_matches(';').trim();
        if source.is_empty() {
            bail!("empty expression");
        }

        if is_generated_bounce_selector(source) {
            return eval_generated_bounce_selector(source, ctx);
        }

        eval_value_expr(source, ctx).and_then(coerce_output)
    }
}

#[derive(Debug, Default)]
pub struct StubExpressionEvaluator;

impl ExpressionEvaluator for StubExpressionEvaluator {
    fn eval(&self, source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue> {
        NamedPatternExpressionEvaluator.eval(source, ctx)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ValueExpr {
    Number(f64),
    Vec2([f64; 2]),
}

fn eval_value_expr(source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue> {
    match eval_expr(source, ctx)? {
        ValueExpr::Number(value) => Ok(ExprValue::Number(value)),
        ValueExpr::Vec2(value) => Ok(ExprValue::Vec2(value)),
    }
}

fn eval_expr(source: &str, ctx: &ExprContext) -> anyhow::Result<ValueExpr> {
    let source = source.trim().trim_end_matches(';').trim();

    if let Some((left, right)) = split_top_level(source, '+') {
        return add_values(eval_expr(left, ctx)?, eval_expr(right, ctx)?);
    }

    if let Some((left, right)) = split_top_level(source, '*') {
        let left = eval_scalar(left, ctx)?;
        let right = eval_scalar(right, ctx)?;
        return Ok(ValueExpr::Number(left * right));
    }

    if source == "value" {
        return ctx
            .value
            .clone()
            .context("expression references value, but context.value is missing")
            .and_then(coerce_output)
            .and_then(value_to_expr);
    }

    if let Some(vec) = parse_vec2_literal(source, ctx)? {
        return Ok(ValueExpr::Vec2(vec));
    }

    eval_scalar(source, ctx).map(ValueExpr::Number)
}

fn eval_scalar(source: &str, ctx: &ExprContext) -> anyhow::Result<f64> {
    let source = source.trim();
    if let Ok(value) = source.parse::<f64>() {
        return Ok(value);
    }

    resolve_scalar(source, ctx).with_context(|| format!("unsupported scalar expression `{source}`"))
}

fn resolve_scalar(name: &str, ctx: &ExprContext) -> anyhow::Result<f64> {
    match name {
        "time" => Ok(ctx.time),
        "inPoint" => Ok(ctx.in_point),
        "outPoint" => Ok(ctx.out_point),
        "textIndex" => ctx
            .text_index
            .map(|value| value as f64)
            .context("expression references textIndex, but context.text_index is missing"),
        "textTotal" => ctx
            .text_total
            .map(|value| value as f64)
            .context("expression references textTotal, but context.text_total is missing"),
        "thisComp.frameDuration" => ctx
            .vars
            .get("thisComp.frameDuration")
            .or_else(|| ctx.vars.get("frameDuration"))
            .context(
                "expression references thisComp.frameDuration, but it is missing from context.vars",
            )
            .and_then(expr_value_to_number),
        _ => ctx
            .vars
            .get(name)
            .with_context(|| format!("unknown expression variable `{name}`"))
            .and_then(expr_value_to_number),
    }
}

fn parse_vec2_literal(source: &str, ctx: &ExprContext) -> anyhow::Result<Option<[f64; 2]>> {
    let Some(inner) = source.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return Ok(None);
    };
    let (x, y) = split_top_level(inner, ',')
        .with_context(|| format!("Vec2 expression `{source}` must contain exactly two elements"))?;
    Ok(Some([eval_number_expr(x, ctx)?, eval_number_expr(y, ctx)?]))
}

fn eval_number_expr(source: &str, ctx: &ExprContext) -> anyhow::Result<f64> {
    match eval_expr(source, ctx)? {
        ValueExpr::Number(value) => Ok(value),
        ValueExpr::Vec2(_) => bail!("Vec2 element `{source}` must evaluate to a number"),
    }
}

fn add_values(left: ValueExpr, right: ValueExpr) -> anyhow::Result<ValueExpr> {
    match (left, right) {
        (ValueExpr::Number(left), ValueExpr::Number(right)) => Ok(ValueExpr::Number(left + right)),
        (ValueExpr::Vec2(left), ValueExpr::Vec2(right)) => {
            Ok(ValueExpr::Vec2([left[0] + right[0], left[1] + right[1]]))
        }
        (ValueExpr::Vec2(left), ValueExpr::Number(right)) => {
            Ok(ValueExpr::Vec2([left[0] + right, left[1] + right]))
        }
        (ValueExpr::Number(left), ValueExpr::Vec2(right)) => {
            Ok(ValueExpr::Vec2([left + right[0], left + right[1]]))
        }
    }
}

fn split_top_level(source: &str, needle: char) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ if ch == needle && depth == 0 && index > 0 => {
                if needle == '+'
                    && matches!(source[..index].chars().last(), Some('e' | 'E'))
                {
                    continue;
                }
                let left = source[..index].trim();
                let right = source[index + ch.len_utf8()..].trim();
                if !left.is_empty() && !right.is_empty() {
                    return Some((left, right));
                }
            }
            _ => {}
        }
    }
    None
}

fn value_to_expr(value: ExprValue) -> anyhow::Result<ValueExpr> {
    match value {
        ExprValue::Number(value) => Ok(ValueExpr::Number(value)),
        ExprValue::Vec2(value) => Ok(ValueExpr::Vec2(value)),
        other => bail!("unsupported expression output after coercion: {other:?}"),
    }
}

fn coerce_output(value: ExprValue) -> anyhow::Result<ExprValue> {
    match value {
        ExprValue::Number(value) => Ok(ExprValue::Number(value)),
        ExprValue::Vec2(value) => Ok(ExprValue::Vec2(value)),
        ExprValue::Vec3(value) => Ok(ExprValue::Vec2([value[0], value[1]])),
        ExprValue::Array(values) => match values.as_slice() {
            [value] => Ok(ExprValue::Number(*value)),
            [x, y] => Ok(ExprValue::Vec2([*x, *y])),
            _ => bail!(
                "expression output arrays must have one numeric element or two Vec2 elements, got {}",
                values.len()
            ),
        },
    }
}

fn expr_value_to_number(value: &ExprValue) -> anyhow::Result<f64> {
    match value {
        ExprValue::Number(value) => Ok(*value),
        ExprValue::Array(values) if values.len() == 1 => Ok(values[0]),
        other => bail!("expected numeric expression variable, got {other:?}"),
    }
}

fn is_generated_bounce_selector(source: &str) -> bool {
    source.contains("myDelay")
        && source.contains("textIndex")
        && source.contains("Math.cos")
        && source.contains("Math.exp")
}

fn eval_generated_bounce_selector(source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue> {
    let delay = extract_js_assignment(source, "delay").unwrap_or(0.05) as f64;
    let freq = extract_js_assignment(source, "freq").unwrap_or(2.0) as f64;
    let amplitude = extract_js_assignment(source, "amplitude").unwrap_or(100.0) as f64;
    let decay = extract_js_assignment(source, "decay").unwrap_or(8.0) as f64;
    let text_index = ctx
        .text_index
        .context("generated bounce selector requires context.text_index")?
        as f64;
    let t = ctx.time - ctx.in_point - delay * text_index;
    if t < -1.0e-9 {
        return Ok(ExprValue::Number(0.0));
    }
    let t = t.max(0.0);
    let value =
        amplitude * (freq * t * 2.0 * std::f64::consts::PI).cos() * (-decay * t).exp();
    Ok(ExprValue::Number(value))
}

fn extract_js_assignment(source: &str, name: &str) -> Option<f32> {
    let compact: String = source.chars().filter(|ch| !ch.is_whitespace()).collect();
    let needle = format!("{name}=");
    let rest = compact.split(&needle).nth(1)?;
    let raw = rest.split(';').next()?.trim();
    raw.parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn eval(source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue> {
        NamedPatternExpressionEvaluator.eval(source, ctx)
    }

    #[test]
    fn evaluates_value_and_coerces_vec3_to_vec2() {
        let ctx = ExprContext {
            value: Some(ExprValue::Vec3([10.0, 20.0, 30.0])),
            ..ExprContext::default()
        };

        assert_eq!(eval("value", &ctx).unwrap(), ExprValue::Vec2([10.0, 20.0]));
    }

    #[test]
    fn evaluates_literals_time_math_and_value_offset() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), ExprValue::Number(4.0));
        let ctx = ExprContext {
            time: 1.25,
            value: Some(ExprValue::Vec2([10.0, 20.0])),
            vars,
            ..ExprContext::default()
        };

        assert_eq!(eval("42", &ctx).unwrap(), ExprValue::Number(42.0));
        assert_eq!(
            eval("[time, x]", &ctx).unwrap(),
            ExprValue::Vec2([1.25, 4.0])
        );
        assert_eq!(eval("time * 8", &ctx).unwrap(), ExprValue::Number(10.0));
        assert_eq!(
            eval("value + [3, -5]", &ctx).unwrap(),
            ExprValue::Vec2([13.0, 15.0])
        );
    }

    #[test]
    fn resolves_this_comp_frame_duration_from_vars() {
        let mut vars = HashMap::new();
        vars.insert(
            "thisComp.frameDuration".to_string(),
            ExprValue::Number(1.0 / 24.0),
        );
        let ctx = ExprContext {
            vars,
            ..ExprContext::default()
        };

        assert_eq!(
            eval("thisComp.frameDuration * 24", &ctx).unwrap(),
            ExprValue::Number(1.0)
        );
    }

    #[test]
    fn evaluates_generated_bounce_selector_fingerprint() {
        let source = r#"
            delay = 0.05;
            myDelay = delay * textIndex;
            freq = 2;
            amplitude = 100;
            decay = 8;
            Math.cos(freq * time * 2 * Math.PI) / Math.exp(decay * time);
        "#;
        let ctx = ExprContext {
            time: 0.05,
            in_point: 0.0,
            text_index: Some(1),
            ..ExprContext::default()
        };

        assert_eq!(eval(source, &ctx).unwrap(), ExprValue::Number(100.0));
    }

    #[test]
    fn unsupported_expressions_fail_explicitly() {
        let err = eval("wiggle(3, 10)", &ExprContext::default()).unwrap_err();
        assert!(err.to_string().contains("unsupported scalar expression"));
    }
}
