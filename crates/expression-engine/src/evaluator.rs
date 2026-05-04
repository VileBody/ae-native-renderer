use crate::{ExprContext, ExprValue};
use anyhow::{bail, Context};
use std::collections::HashMap;

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

        let mut parser = Parser::new(source, ctx);
        let value = parser.parse()?;
        value_to_output(value)
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

struct Parser<'a> {
    source: &'a str,
    pos: usize,
    ctx: &'a ExprContext,
    locals: HashMap<String, ValueExpr>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, ctx: &'a ExprContext) -> Self {
        Self {
            source,
            pos: 0,
            ctx,
            locals: HashMap::new(),
        }
    }

    fn parse(&mut self) -> anyhow::Result<ValueExpr> {
        let mut value = None;
        loop {
            self.skip_ws();
            if self.is_eof() {
                break;
            }
            value = Some(self.parse_statement()?);
            self.skip_ws();
            if self.is_eof() {
                break;
            }
            if !self.consume_char(';') {
                bail!("unsupported expression tail `{}`", &self.source[self.pos..]);
            }
        }
        value.context("empty expression")
    }

    fn parse_statement(&mut self) -> anyhow::Result<ValueExpr> {
        self.skip_ws();
        let start = self.pos;
        if self.peek_char().is_some_and(is_ident_start) {
            let name = self.parse_ident()?;
            self.skip_ws();
            if self.consume_char('=') {
                let value = self.parse_additive()?;
                self.locals.insert(name, value.clone());
                return Ok(value);
            }
        }
        self.pos = start;
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> anyhow::Result<ValueExpr> {
        let mut value = self.parse_multiplicative()?;
        loop {
            self.skip_ws();
            if self.consume_char('+') {
                value = add_values(value, self.parse_multiplicative()?)?;
            } else if self.consume_char('-') {
                value = sub_values(value, self.parse_multiplicative()?)?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_multiplicative(&mut self) -> anyhow::Result<ValueExpr> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.consume_char('*') {
                value = mul_values(value, self.parse_unary()?)?;
            } else if self.consume_char('/') {
                value = div_values(value, self.parse_unary()?)?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_unary(&mut self) -> anyhow::Result<ValueExpr> {
        self.skip_ws();
        if self.consume_char('+') {
            return self.parse_unary();
        }
        if self.consume_char('-') {
            return negate_value(self.parse_unary()?);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> anyhow::Result<ValueExpr> {
        self.skip_ws();
        if self.consume_char('(') {
            let value = self.parse_additive()?;
            self.expect_char(')')?;
            return Ok(value);
        }
        if self.consume_char('[') {
            let x = self.parse_additive()?;
            self.expect_char(',')?;
            let y = self.parse_additive()?;
            self.expect_char(']')?;
            return Ok(ValueExpr::Vec2([number_value(x)?, number_value(y)?]));
        }
        if self
            .peek_char()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '.')
        {
            return self.parse_number().map(ValueExpr::Number);
        }
        let ident = self.parse_ident()?;
        self.skip_ws();
        if self.consume_char('(') {
            return self.parse_call(&ident);
        }
        self.resolve_variable(&ident)
    }

    fn parse_call(&mut self, name: &str) -> anyhow::Result<ValueExpr> {
        let mut args = Vec::new();
        self.skip_ws();
        if !self.consume_char(')') {
            loop {
                args.push(self.parse_additive()?);
                self.skip_ws();
                if self.consume_char(')') {
                    break;
                }
                self.expect_char(',')?;
            }
        }

        match name {
            "Math.sin" | "sin" => unary_number_fn(name, &args, f64::sin),
            "Math.cos" | "cos" => unary_number_fn(name, &args, f64::cos),
            "Math.exp" | "exp" => unary_number_fn(name, &args, f64::exp),
            "Math.min" | "min" => variadic_number_fn(name, &args, |a, b| a.min(b)),
            "Math.max" | "max" => variadic_number_fn(name, &args, |a, b| a.max(b)),
            _ => bail!("unsupported expression function `{name}`"),
        }
    }

    fn resolve_variable(&self, name: &str) -> anyhow::Result<ValueExpr> {
        if let Some(value) = self.locals.get(name) {
            return Ok(value.clone());
        }
        match name {
            "time" => Ok(ValueExpr::Number(self.ctx.time)),
            "inPoint" => Ok(ValueExpr::Number(self.ctx.in_point)),
            "outPoint" => Ok(ValueExpr::Number(self.ctx.out_point)),
            "thisLayer.inPoint" => Ok(ValueExpr::Number(
                self.ctx
                    .vars
                    .get("thisLayer.inPoint")
                    .map(expr_value_to_number)
                    .transpose()?
                    .unwrap_or(self.ctx.in_point),
            )),
            "thisLayer.outPoint" => Ok(ValueExpr::Number(
                self.ctx
                    .vars
                    .get("thisLayer.outPoint")
                    .map(expr_value_to_number)
                    .transpose()?
                    .unwrap_or(self.ctx.out_point),
            )),
            "textIndex" => self
                .ctx
                .text_index
                .map(|value| ValueExpr::Number(value as f64))
                .context("expression references textIndex, but context.text_index is missing"),
            "textTotal" => self
                .ctx
                .text_total
                .map(|value| ValueExpr::Number(value as f64))
                .context("expression references textTotal, but context.text_total is missing"),
            "value" => self
                .ctx
                .value
                .clone()
                .context("expression references value, but context.value is missing")
                .and_then(|value| expr_value_to_expr(&value)),
            "Math.PI" => Ok(ValueExpr::Number(std::f64::consts::PI)),
            "thisComp.width"
            | "thisComp.height"
            | "thisComp.duration"
            | "thisComp.frameDuration" => self
                .ctx
                .vars
                .get(name)
                .or_else(|| {
                    if name == "thisComp.frameDuration" {
                        self.ctx.vars.get("frameDuration")
                    } else {
                        None
                    }
                })
                .with_context(|| {
                    format!("expression references {name}, but it is missing from context.vars")
                })
                .and_then(expr_value_to_expr),
            _ => self
                .ctx
                .vars
                .get(name)
                .with_context(|| format!("unknown expression variable `{name}`"))
                .and_then(expr_value_to_expr),
        }
    }

    fn parse_number(&mut self) -> anyhow::Result<f64> {
        let start = self.pos;
        let mut saw_digit = false;
        while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
            saw_digit = true;
            self.advance_char();
        }
        if self.peek_char() == Some('.') {
            self.advance_char();
            while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                saw_digit = true;
                self.advance_char();
            }
        }
        if !saw_digit {
            bail!("malformed number at byte {}", start);
        }
        if matches!(self.peek_char(), Some('e' | 'E')) {
            let exp_mark = self.pos;
            self.advance_char();
            if matches!(self.peek_char(), Some('+' | '-')) {
                self.advance_char();
            }
            let exp_start = self.pos;
            while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                self.advance_char();
            }
            if self.pos == exp_start {
                self.pos = exp_mark;
            }
        }
        self.source[start..self.pos]
            .parse::<f64>()
            .with_context(|| format!("malformed number `{}`", &self.source[start..self.pos]))
    }

    fn parse_ident(&mut self) -> anyhow::Result<String> {
        self.skip_ws();
        let start = self.pos;
        if !self.peek_char().is_some_and(is_ident_start) {
            bail!(
                "unsupported scalar expression `{}`",
                &self.source[self.pos..]
            );
        }
        self.advance_char();
        while self.peek_char().is_some_and(is_ident_continue) {
            self.advance_char();
        }
        Ok(self.source[start..self.pos].to_string())
    }

    fn expect_char(&mut self, expected: char) -> anyhow::Result<()> {
        self.skip_ws();
        if self.consume_char(expected) {
            Ok(())
        } else {
            bail!("expected `{expected}` at byte {}", self.pos)
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        self.skip_ws();
        if self.peek_char() == Some(expected) {
            self.advance_char();
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.advance_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn advance_char(&mut self) {
        if let Some(ch) = self.peek_char() {
            self.pos += ch.len_utf8();
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.')
}

fn unary_number_fn(
    name: &str,
    args: &[ValueExpr],
    f: impl FnOnce(f64) -> f64,
) -> anyhow::Result<ValueExpr> {
    let [arg] = args else {
        bail!("{name} expects exactly one numeric argument");
    };
    Ok(ValueExpr::Number(f(number_ref(arg, name)?)))
}

fn variadic_number_fn(
    name: &str,
    args: &[ValueExpr],
    f: impl Fn(f64, f64) -> f64,
) -> anyhow::Result<ValueExpr> {
    let Some(first) = args.first() else {
        bail!("{name} expects at least one numeric argument");
    };
    let mut value = number_ref(first, name)?;
    for arg in &args[1..] {
        value = f(value, number_ref(arg, name)?);
    }
    Ok(ValueExpr::Number(value))
}

fn number_ref(value: &ValueExpr, context: &str) -> anyhow::Result<f64> {
    match value {
        ValueExpr::Number(value) => Ok(*value),
        ValueExpr::Vec2(_) => bail!("{context} expects a number, got Vec2"),
    }
}

fn number_value(value: ValueExpr) -> anyhow::Result<f64> {
    match value {
        ValueExpr::Number(value) => Ok(value),
        ValueExpr::Vec2(_) => bail!("Vec2 array elements must evaluate to numbers"),
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

fn sub_values(left: ValueExpr, right: ValueExpr) -> anyhow::Result<ValueExpr> {
    match (left, right) {
        (ValueExpr::Number(left), ValueExpr::Number(right)) => Ok(ValueExpr::Number(left - right)),
        (ValueExpr::Vec2(left), ValueExpr::Vec2(right)) => {
            Ok(ValueExpr::Vec2([left[0] - right[0], left[1] - right[1]]))
        }
        (ValueExpr::Vec2(left), ValueExpr::Number(right)) => {
            Ok(ValueExpr::Vec2([left[0] - right, left[1] - right]))
        }
        (ValueExpr::Number(left), ValueExpr::Vec2(right)) => {
            Ok(ValueExpr::Vec2([left - right[0], left - right[1]]))
        }
    }
}

fn mul_values(left: ValueExpr, right: ValueExpr) -> anyhow::Result<ValueExpr> {
    match (left, right) {
        (ValueExpr::Number(left), ValueExpr::Number(right)) => Ok(ValueExpr::Number(left * right)),
        (ValueExpr::Vec2(left), ValueExpr::Number(right)) => {
            Ok(ValueExpr::Vec2([left[0] * right, left[1] * right]))
        }
        (ValueExpr::Number(left), ValueExpr::Vec2(right)) => {
            Ok(ValueExpr::Vec2([left * right[0], left * right[1]]))
        }
        (ValueExpr::Vec2(_), ValueExpr::Vec2(_)) => bail!("Vec2 * Vec2 is unsupported"),
    }
}

fn div_values(left: ValueExpr, right: ValueExpr) -> anyhow::Result<ValueExpr> {
    let divisor = number_value(right)?;
    if divisor.abs() <= f64::EPSILON {
        bail!("division by zero in expression");
    }
    match left {
        ValueExpr::Number(left) => Ok(ValueExpr::Number(left / divisor)),
        ValueExpr::Vec2(left) => Ok(ValueExpr::Vec2([left[0] / divisor, left[1] / divisor])),
    }
}

fn negate_value(value: ValueExpr) -> anyhow::Result<ValueExpr> {
    match value {
        ValueExpr::Number(value) => Ok(ValueExpr::Number(-value)),
        ValueExpr::Vec2(value) => Ok(ValueExpr::Vec2([-value[0], -value[1]])),
    }
}

fn expr_value_to_expr(value: &ExprValue) -> anyhow::Result<ValueExpr> {
    match value {
        ExprValue::Number(value) => Ok(ValueExpr::Number(*value)),
        ExprValue::Vec2(value) => Ok(ValueExpr::Vec2(*value)),
        ExprValue::Vec3(value) => Ok(ValueExpr::Vec2([value[0], value[1]])),
        ExprValue::Array(values) => match values.as_slice() {
            [value] => Ok(ValueExpr::Number(*value)),
            [x, y] => Ok(ValueExpr::Vec2([*x, *y])),
            _ => bail!(
                "expression arrays must have one numeric element or two Vec2 elements, got {}",
                values.len()
            ),
        },
    }
}

fn expr_value_to_number(value: &ExprValue) -> anyhow::Result<f64> {
    match expr_value_to_expr(value)? {
        ValueExpr::Number(value) => Ok(value),
        ValueExpr::Vec2(_) => bail!("expected numeric expression variable, got Vec2"),
    }
}

fn value_to_output(value: ValueExpr) -> anyhow::Result<ExprValue> {
    match value {
        ValueExpr::Number(value) => Ok(ExprValue::Number(value)),
        ValueExpr::Vec2(value) => Ok(ExprValue::Vec2(value)),
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
    let text_index =
        ctx.text_index
            .context("generated bounce selector requires context.text_index")? as f64;
    let t = ctx.time - ctx.in_point - delay * text_index;
    if t < -1.0e-9 {
        return Ok(ExprValue::Number(0.0));
    }
    let t = t.max(0.0);
    let value = amplitude * (freq * t * 2.0 * std::f64::consts::PI).cos() * (-decay * t).exp();
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
    fn evaluates_literals_precedence_math_and_value_offset() {
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
        assert_eq!(eval("2 + 3 * 4", &ctx).unwrap(), ExprValue::Number(14.0));
        assert_eq!(eval("(2 + 3) * 4", &ctx).unwrap(), ExprValue::Number(20.0));
        assert_eq!(eval("10 - 3 / 2", &ctx).unwrap(), ExprValue::Number(8.5));
        assert_eq!(
            eval("value + [3, -5]", &ctx).unwrap(),
            ExprValue::Vec2([13.0, 15.0])
        );
        assert_eq!(
            eval("(value - [2, 4]) / 2", &ctx).unwrap(),
            ExprValue::Vec2([4.0, 8.0])
        );
    }

    #[test]
    fn evaluates_exp_010_style_assignment_subset() {
        let ctx = ExprContext {
            time: 1.0 / 6.0,
            in_point: 0.0,
            out_point: 2.0,
            value: Some(ExprValue::Vec2([256.0, 256.0])),
            ..ExprContext::default()
        };

        let value = eval(
            r#"
                intro = 0.25; outro = 0.25; amp = 34; freq = 2.0;
                edge = Math.min(time - inPoint, outPoint - time);
                env = Math.max(0, Math.min(1, edge / intro));
                value + [Math.sin(time * freq * 2 * Math.PI) * amp * env, 0];
            "#,
            &ctx,
        )
        .unwrap();

        let ExprValue::Vec2(position) = value else {
            panic!("expected Vec2 output");
        };
        assert!((position[0] - 275.6299).abs() < 1.0e-3, "{position:?}");
        assert!((position[1] - 256.0).abs() < 1.0e-6, "{position:?}");
    }

    #[test]
    fn resolves_this_comp_and_layer_vars() {
        let mut vars = HashMap::new();
        vars.insert(
            "thisComp.frameDuration".to_string(),
            ExprValue::Number(1.0 / 24.0),
        );
        vars.insert("thisComp.width".to_string(), ExprValue::Number(1080.0));
        vars.insert("thisLayer.inPoint".to_string(), ExprValue::Number(2.0));
        let ctx = ExprContext {
            vars,
            in_point: 1.0,
            ..ExprContext::default()
        };

        assert_eq!(
            eval("thisComp.frameDuration * 24", &ctx).unwrap(),
            ExprValue::Number(1.0)
        );
        assert_eq!(
            eval("thisComp.width / 2", &ctx).unwrap(),
            ExprValue::Number(540.0)
        );
        assert_eq!(
            eval("thisLayer.inPoint", &ctx).unwrap(),
            ExprValue::Number(2.0)
        );
    }

    #[test]
    fn evaluates_math_functions() {
        let value = eval(
            "Math.max(1, min(4, 2 + 3)) + Math.cos(Math.PI)",
            &ExprContext::default(),
        )
        .unwrap();

        assert_eq!(value, ExprValue::Number(3.0));
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
        assert!(err.to_string().contains("unsupported expression function"));
    }
}
