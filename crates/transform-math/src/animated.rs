#[derive(Debug, Clone)]
pub enum Animated<T> {
    Static(T),
    Keyframes(Vec<Keyframe<T>>),
}

#[derive(Debug, Clone)]
pub struct Keyframe<T> {
    pub time: f64,
    pub value: T,
    pub interpolation: Interpolation,
}

#[derive(Debug, Clone, Copy)]
pub enum Interpolation {
    Hold,
    Linear,
    Bezier,
}

impl<T: Clone> Animated<T> {
    pub fn value_at(&self, _time: f64) -> T {
        match self {
            Animated::Static(v) => v.clone(),
            Animated::Keyframes(keys) => keys
                .first()
                .expect("Animated::Keyframes must not be empty")
                .value
                .clone(),
        }
    }
}
