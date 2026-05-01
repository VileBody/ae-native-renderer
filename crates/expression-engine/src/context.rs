use crate::ExprValue;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ExprContext {
    pub time: f64,
    pub in_point: f64,
    pub out_point: f64,
    pub text_index: Option<usize>,
    pub text_total: Option<usize>,
    pub value: Option<ExprValue>,
    pub vars: HashMap<String, ExprValue>,
}
