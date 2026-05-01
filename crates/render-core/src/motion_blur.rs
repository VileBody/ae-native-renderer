//! Native layer motion blur skeleton.
//!
//! Target algorithm: temporal supersampling across shutter interval.

#[derive(Debug, Clone, Copy)]
pub struct MotionBlurSettings {
    pub enabled: bool,
    pub samples: u32,
    pub shutter_angle: f64,
    pub shutter_phase: f64,
}

impl Default for MotionBlurSettings {
    fn default() -> Self {
        Self { enabled: false, samples: 8, shutter_angle: 180.0, shutter_phase: -90.0 }
    }
}

pub fn shutter_interval(frame_time: f64, frame_duration: f64, shutter_angle: f64, shutter_phase: f64) -> (f64, f64) {
    let exposure = frame_duration * shutter_angle / 360.0;
    let open = frame_time + frame_duration * shutter_phase / 360.0;
    (open, open + exposure)
}
