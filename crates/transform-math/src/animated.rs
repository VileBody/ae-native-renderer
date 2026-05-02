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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interpolation {
    Hold,
    Linear,
    Bezier,
    CubicBezier(CubicBezier),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub const LINEAR: Self = Self {
        x1: 0.0,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
    };

    pub const EASE: Self = Self {
        x1: 0.25,
        y1: 0.1,
        x2: 0.25,
        y2: 1.0,
    };

    pub const EASE_IN_OUT: Self = Self {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    };

    pub fn ease(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= 0.0 || progress >= 1.0 {
            return progress;
        }

        let mut t = progress;
        for _ in 0..6 {
            let x = cubic_sample(t, self.x1, self.x2) - progress;
            let dx = cubic_derivative(t, self.x1, self.x2);
            if dx.abs() < 1.0e-6 {
                break;
            }
            let next = t - x / dx;
            if !(0.0..=1.0).contains(&next) {
                break;
            }
            t = next;
        }

        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..12 {
            let x = cubic_sample(t, self.x1, self.x2);
            if (x - progress).abs() < 1.0e-5 {
                break;
            }
            if x < progress {
                low = t;
            } else {
                high = t;
            }
            t = (low + high) * 0.5;
        }

        cubic_sample(t, self.y1, self.y2).clamp(0.0, 1.0)
    }
}

impl Default for CubicBezier {
    fn default() -> Self {
        Self::EASE_IN_OUT
    }
}

pub trait AnimatedValue: Clone {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self;
}

impl AnimatedValue for f32 {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        *from + (*to - *from) * progress
    }
}

impl AnimatedValue for [f32; 2] {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        [
            <f32 as AnimatedValue>::interpolate(&from[0], &to[0], progress),
            <f32 as AnimatedValue>::interpolate(&from[1], &to[1], progress),
        ]
    }
}

impl<T: AnimatedValue> Animated<T> {
    pub fn value_at(&self, time: f64) -> T {
        match self {
            Animated::Static(v) => v.clone(),
            Animated::Keyframes(keys) => value_at_keyframes(keys, time),
        }
    }
}

fn value_at_keyframes<T: AnimatedValue>(keys: &[Keyframe<T>], time: f64) -> T {
    let first = keys.first().expect("Animated::Keyframes must not be empty");
    if time <= first.time {
        return first.value.clone();
    }

    for pair in keys.windows(2) {
        let from = &pair[0];
        let to = &pair[1];
        if time <= to.time {
            if matches!(from.interpolation, Interpolation::Hold) || to.time <= from.time {
                return from.value.clone();
            }

            let progress = ((time - from.time) / (to.time - from.time)).clamp(0.0, 1.0) as f32;
            let eased = match from.interpolation {
                Interpolation::Hold => 0.0,
                Interpolation::Linear => progress,
                Interpolation::Bezier => CubicBezier::EASE_IN_OUT.ease(progress),
                Interpolation::CubicBezier(bezier) => bezier.ease(progress),
            };
            return T::interpolate(&from.value, &to.value, eased);
        }
    }

    keys.last()
        .expect("Animated::Keyframes must not be empty")
        .value
        .clone()
}

fn cubic_sample(t: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - t;
    3.0 * inv * inv * t * p1 + 3.0 * inv * t * t * p2 + t * t * t
}

fn cubic_derivative(t: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - t;
    3.0 * inv * inv * p1 + 6.0 * inv * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_keeps_previous_key_value_until_next_key() {
        let animated = Animated::Keyframes(vec![
            Keyframe {
                time: 0.0,
                value: 10.0,
                interpolation: Interpolation::Hold,
            },
            Keyframe {
                time: 1.0,
                value: 20.0,
                interpolation: Interpolation::Linear,
            },
        ]);

        assert_eq!(animated.value_at(0.5), 10.0);
        assert_eq!(animated.value_at(1.0), 10.0);
        assert_eq!(animated.value_at(1.1), 20.0);
    }

    #[test]
    fn linear_interpolates_scalar_and_vec2() {
        let scalar = Animated::Keyframes(vec![
            Keyframe {
                time: 0.0,
                value: 0.0,
                interpolation: Interpolation::Linear,
            },
            Keyframe {
                time: 2.0,
                value: 10.0,
                interpolation: Interpolation::Linear,
            },
        ]);
        let vec2 = Animated::Keyframes(vec![
            Keyframe {
                time: 0.0,
                value: [0.0, 10.0],
                interpolation: Interpolation::Linear,
            },
            Keyframe {
                time: 2.0,
                value: [10.0, 20.0],
                interpolation: Interpolation::Linear,
            },
        ]);

        assert_eq!(scalar.value_at(1.0), 5.0);
        assert_eq!(vec2.value_at(1.0), [5.0, 15.0]);
    }

    #[test]
    fn cubic_bezier_ease_is_monotonic() {
        let bezier = CubicBezier::EASE_IN_OUT;
        let mut previous = bezier.ease(0.0);
        for step in 1..=100 {
            let progress = step as f32 / 100.0;
            let current = bezier.ease(progress);
            assert!(current >= previous, "{current} regressed below {previous}");
            previous = current;
        }
    }

    #[test]
    fn cubic_bezier_interpolates_with_ease_shape() {
        let animated = Animated::Keyframes(vec![
            Keyframe {
                time: 0.0,
                value: 0.0,
                interpolation: Interpolation::CubicBezier(CubicBezier::EASE_IN_OUT),
            },
            Keyframe {
                time: 1.0,
                value: 100.0,
                interpolation: Interpolation::Linear,
            },
        ]);

        assert!(animated.value_at(0.25) < 25.0);
        assert!((animated.value_at(0.5) - 50.0).abs() < 0.01);
        assert!(animated.value_at(0.75) > 75.0);
    }

    #[test]
    fn legacy_bezier_variant_uses_default_ease_in_out() {
        let animated = Animated::Keyframes(vec![
            Keyframe {
                time: 0.0,
                value: 0.0,
                interpolation: Interpolation::Bezier,
            },
            Keyframe {
                time: 1.0,
                value: 100.0,
                interpolation: Interpolation::Linear,
            },
        ]);

        assert!((animated.value_at(0.5) - 50.0).abs() < 0.01);
    }
}
