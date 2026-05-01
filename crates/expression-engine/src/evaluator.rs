use crate::{ExprContext, ExprValue};

pub trait ExpressionEvaluator: Send + Sync {
    fn eval(&self, source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue>;
}

#[derive(Debug, Default)]
pub struct StubExpressionEvaluator;

impl ExpressionEvaluator for StubExpressionEvaluator {
    fn eval(&self, _source: &str, ctx: &ExprContext) -> anyhow::Result<ExprValue> {
        // TODO: integrate QuickJS/Boa and expose AE-like variables.
        Ok(ctx.value.clone().unwrap_or(ExprValue::Number(0.0)))
    }
}
