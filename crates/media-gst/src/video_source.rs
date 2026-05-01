#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pts: f64,
    pub rgba: Vec<u8>,
}

pub trait VideoSource {
    fn info(&self) -> VideoInfo;
    fn frame_at(&mut self, time: f64) -> anyhow::Result<VideoFrame>;
}

pub struct GstVideoSourceTodo;
