use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fps(pub f64);

impl Fps {
    pub fn frame_duration(self) -> f64 {
        1.0 / self.0
    }

    pub fn frame_time(self, frame_index: u64) -> f64 {
        frame_index as f64 / self.0
    }

    pub fn floor_frame_index(self, time_seconds: f64) -> u64 {
        (time_seconds * self.0).floor().max(0.0) as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShutterSpec {
    pub fps: Fps,
    pub shutter_angle_degrees: f64,
    pub shutter_phase_degrees: f64,
    pub samples: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShutterInterval {
    pub open_seconds: f64,
    pub close_seconds: f64,
    pub exposure_seconds: f64,
}

impl ShutterSpec {
    pub fn exposure_duration_seconds(&self) -> f64 {
        self.fps.frame_duration() * self.shutter_angle_degrees / 360.0
    }

    pub fn interval_for_frame_time(&self, time_seconds: f64) -> ShutterInterval {
        let frame_duration = self.fps.frame_duration();
        let exposure_seconds = self.exposure_duration_seconds();
        let open_seconds = time_seconds + frame_duration * self.shutter_phase_degrees / 360.0;
        ShutterInterval {
            open_seconds,
            close_seconds: open_seconds + exposure_seconds,
            exposure_seconds,
        }
    }

    pub fn sample_times_for_frame_time(&self, time_seconds: f64) -> Vec<f64> {
        sample_times(*self, time_seconds)
    }

    pub fn sample_weights(&self) -> Vec<f64> {
        if self.samples == 0 {
            return Vec::new();
        }
        vec![1.0 / self.samples as f64; self.samples]
    }
}

pub fn sample_times(spec: ShutterSpec, frame_time_seconds: f64) -> Vec<f64> {
    if spec.samples == 0 {
        return Vec::new();
    }
    if spec.samples == 1 || spec.shutter_angle_degrees == 0.0 {
        return vec![frame_time_seconds];
    }

    let interval = spec.interval_for_frame_time(frame_time_seconds);
    (0..spec.samples)
        .map(|sample| {
            interval.open_seconds
                + interval.exposure_seconds * ((sample as f64 + 0.5) / spec.samples as f64)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosterizeMode {
    Floor,
    Nearest,
}

pub fn posterize_time(time_seconds: f64, fps: Fps, mode: PosterizeMode) -> f64 {
    let quantized = time_seconds * fps.0;
    match mode {
        PosterizeMode::Floor => quantized.floor() / fps.0,
        PosterizeMode::Nearest => quantized.round() / fps.0,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeProbe {
    pub label: String,
    pub time_seconds: f64,
}

pub fn time_boundary_probes(label: &str, time_seconds: f64, epsilon: f64) -> Vec<TimeProbe> {
    vec![
        TimeProbe {
            label: format!("{label}_minus_epsilon"),
            time_seconds: time_seconds - epsilon,
        },
        TimeProbe {
            label: format!("{label}_exact"),
            time_seconds,
        },
        TimeProbe {
            label: format!("{label}_plus_epsilon"),
            time_seconds: time_seconds + epsilon,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_blur_sample_times_match_180_minus_90_four_samples() {
        let spec = ShutterSpec {
            fps: Fps(24.0),
            shutter_angle_degrees: 180.0,
            shutter_phase_degrees: -90.0,
            samples: 4,
        };

        let samples = sample_times(spec, 1.0);

        assert_close(
            &samples,
            &[0.9921875, 0.9973958333333334, 1.0026041666666667, 1.0078125],
        );
    }

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-12,
                "expected {expected}, got {actual}"
            );
        }
    }
}
