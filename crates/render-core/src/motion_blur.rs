//! Native layer motion blur skeleton.
//!
//! Target algorithm: temporal supersampling across shutter interval.

use render_ir::MotionBlurSettings;

pub fn shutter_interval(
    frame_time: f64,
    frame_duration: f64,
    shutter_angle: f64,
    shutter_phase: f64,
) -> (f64, f64) {
    let exposure = frame_duration * shutter_angle / 360.0;
    let open = frame_time + frame_duration * shutter_phase / 360.0;
    (open, open + exposure)
}

pub fn sample_times(
    frame_time: f64,
    frame_duration: f64,
    settings: MotionBlurSettings,
) -> Vec<f64> {
    let samples = settings.samples.clamp(1, 64);
    if !settings.enabled || samples <= 1 || settings.shutter_angle <= 0.0 {
        return vec![frame_time];
    }
    let (open, close) = shutter_interval(
        frame_time,
        frame_duration,
        settings.shutter_angle,
        settings.shutter_phase,
    );
    let exposure = close - open;
    (0..samples)
        .map(|sample| open + exposure * ((sample as f64 + 0.5) / samples as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutter_interval_uses_angle_and_phase() {
        let (open, close) = shutter_interval(1.0, 0.1, 180.0, -90.0);

        assert!((open - 0.975).abs() < 1.0e-9);
        assert!((close - 1.025).abs() < 1.0e-9);
    }

    #[test]
    fn sample_times_are_midpoints_inside_shutter() {
        let times = sample_times(
            1.0,
            0.1,
            MotionBlurSettings {
                enabled: true,
                samples: 2,
                shutter_angle: 180.0,
                shutter_phase: -90.0,
            },
        );

        assert_eq!(times.len(), 2);
        assert!((times[0] - 0.9875).abs() < 1.0e-9);
        assert!((times[1] - 1.0125).abs() < 1.0e-9);
    }

    #[test]
    fn default_enabled_motion_blur_uses_ae_transform_sample_ladder() {
        let times = sample_times(
            1.0,
            0.1,
            MotionBlurSettings {
                enabled: true,
                ..MotionBlurSettings::default()
            },
        );

        assert_eq!(times.len(), render_ir::DEFAULT_MOTION_BLUR_SAMPLES as usize);
    }
}
