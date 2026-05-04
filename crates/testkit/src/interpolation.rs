use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpolationKind {
    Hold,
    Linear,
    CubicBezier,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Keyframe<T> {
    pub time_seconds: f64,
    pub value: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CubicBezier1D {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl CubicBezier1D {
    pub fn ease_in_out() -> Self {
        Self {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        }
    }

    pub fn sample_y_for_x(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        let mut t = x;
        for _ in 0..8 {
            let fx = cubic(0.0, self.x1, self.x2, 1.0, t) - x;
            let derivative = cubic_derivative(0.0, self.x1, self.x2, 1.0, t);
            if derivative.abs() < 1e-12 {
                break;
            }
            let next = t - fx / derivative;
            if !(0.0..=1.0).contains(&next) {
                break;
            }
            t = next;
        }
        cubic(0.0, self.y1, self.y2, 1.0, t).clamp(0.0, 1.0)
    }
}

fn cubic(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let mt = 1.0 - t;
    mt * mt * mt * a + 3.0 * mt * mt * t * b + 3.0 * mt * t * t * c + t * t * t * d
}

fn cubic_derivative(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let mt = 1.0 - t;
    3.0 * mt * mt * (b - a) + 6.0 * mt * t * (c - b) + 3.0 * t * t * (d - c)
}

pub fn normalized_segment_t(time_seconds: f64, start_seconds: f64, end_seconds: f64) -> f64 {
    if (end_seconds - start_seconds).abs() < f64::EPSILON {
        1.0
    } else {
        ((time_seconds - start_seconds) / (end_seconds - start_seconds)).clamp(0.0, 1.0)
    }
}

pub fn interpolate_scalar(
    time_seconds: f64,
    start: Keyframe<f64>,
    end: Keyframe<f64>,
    kind: InterpolationKind,
    bezier: Option<CubicBezier1D>,
) -> f64 {
    match kind {
        InterpolationKind::Hold => start.value,
        InterpolationKind::Linear => {
            let t = normalized_segment_t(time_seconds, start.time_seconds, end.time_seconds);
            start.value + (end.value - start.value) * t
        }
        InterpolationKind::CubicBezier => {
            let t = normalized_segment_t(time_seconds, start.time_seconds, end.time_seconds);
            let eased = bezier
                .unwrap_or_else(CubicBezier1D::ease_in_out)
                .sample_y_for_x(t);
            start.value + (end.value - start.value) * eased
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpolationProbe {
    pub label: String,
    pub time_seconds: f64,
    pub expected_value: f64,
}

pub fn standard_interpolation_probes(
    start: Keyframe<f64>,
    end: Keyframe<f64>,
    kind: InterpolationKind,
) -> Vec<InterpolationProbe> {
    let epsilon = ((end.time_seconds - start.time_seconds).abs() * 1e-6).max(1e-9);
    let midpoint = (start.time_seconds + end.time_seconds) * 0.5;
    [
        ("before_start_epsilon", start.time_seconds - epsilon),
        ("at_start", start.time_seconds),
        ("midpoint", midpoint),
        ("at_end", end.time_seconds),
        ("after_end_epsilon", end.time_seconds + epsilon),
    ]
    .into_iter()
    .map(|(label, time_seconds)| InterpolationProbe {
        label: label.to_string(),
        time_seconds,
        expected_value: interpolate_scalar(time_seconds, start, end, kind, None),
    })
    .collect()
}
