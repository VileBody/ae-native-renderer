use crate::probe::probe;
use std::path::{Path, PathBuf};
use std::process::Command;

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

#[derive(Debug, Clone)]
pub struct FfmpegVideoSource {
    path: PathBuf,
    info: VideoInfo,
}

impl FfmpegVideoSource {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let media = probe(&path)?;
        let width = media.width.ok_or_else(|| {
            anyhow::anyhow!(
                "video width was not reported by ffprobe: {}",
                path.display()
            )
        })?;
        let height = media.height.ok_or_else(|| {
            anyhow::anyhow!(
                "video height was not reported by ffprobe: {}",
                path.display()
            )
        })?;
        Ok(Self {
            path,
            info: VideoInfo {
                width,
                height,
                fps: media.fps.unwrap_or(30.0),
                duration: media.duration.unwrap_or(0.0),
            },
        })
    }
}

impl VideoSource for FfmpegVideoSource {
    fn info(&self) -> VideoInfo {
        self.info.clone()
    }

    fn frame_at(&mut self, time: f64) -> anyhow::Result<VideoFrame> {
        let frame_duration = if self.info.fps > 0.0 {
            1.0 / self.info.fps
        } else {
            0.0
        };
        let max_time = (self.info.duration - frame_duration).max(0.0);
        let time = time.clamp(0.0, max_time);

        let output = Command::new("ffmpeg")
            .arg("-v")
            .arg("error")
            .arg("-ss")
            .arg(format!("{time:.6}"))
            .arg("-i")
            .arg(&self.path)
            .arg("-frames:v")
            .arg("1")
            .arg("-an")
            .arg("-sn")
            .arg("-dn")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("rgba")
            .arg("pipe:1")
            .output()
            .map_err(|err| anyhow::anyhow!("failed to run ffmpeg: {err}"))?;

        if !output.status.success() {
            anyhow::bail!(
                "ffmpeg failed to decode frame at {time:.3}s from {}: {}",
                self.path.display(),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let expected = (self.info.width * self.info.height * 4) as usize;
        if output.stdout.len() != expected {
            anyhow::bail!(
                "ffmpeg decoded {} bytes for {}, expected {} bytes",
                output.stdout.len(),
                self.path.display(),
                expected
            );
        }

        Ok(VideoFrame {
            width: self.info.width,
            height: self.info.height,
            pts: time,
            rgba: output.stdout,
        })
    }
}
