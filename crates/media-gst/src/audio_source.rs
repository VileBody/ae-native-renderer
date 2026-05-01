#[derive(Debug, Clone)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: f64,
}

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub pts: f64,
    pub samples_f32: Vec<f32>,
}

pub trait AudioSource {
    fn info(&self) -> AudioInfo;
    fn samples_at(&mut self, time: f64, duration: f64) -> anyhow::Result<AudioBuffer>;
}
