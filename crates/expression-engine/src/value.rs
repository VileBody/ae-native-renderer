#[derive(Debug, Clone, PartialEq)]
pub enum ExprValue {
    Number(f64),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
    Array(Vec<f64>),
}
