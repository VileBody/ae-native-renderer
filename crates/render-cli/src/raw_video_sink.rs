use anyhow::Context;
use media_gst::VideoSinkManifest;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

pub(crate) struct FfmpegRawVideoSink {
    output: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    width: u32,
    height: u32,
    fps: f64,
    frames: u32,
    last_time: Option<f64>,
    finished: bool,
}

impl FfmpegRawVideoSink {
    pub(crate) fn open(
        output: impl AsRef<Path>,
        width: u32,
        height: u32,
        fps: f64,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(width > 0 && height > 0, "video dimensions must be positive");
        anyhow::ensure!(fps.is_finite() && fps > 0.0, "video fps must be positive");

        let output = output.as_ref().to_path_buf();
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating video output directory {}", parent.display()))?;
        }

        let pixel_format = if width % 2 == 0 && height % 2 == 0 {
            "yuv420p"
        } else {
            "yuv444p"
        };
        let mut command = Command::new("ffmpeg");
        command
            .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "rawvideo", "-pixel_format", "rgba"])
            .arg("-video_size")
            .arg(format!("{width}x{height}"))
            .arg("-framerate")
            .arg(format!("{fps:.12}"))
            .args(["-i", "pipe:0", "-an", "-c:v", "libx264"])
            .arg("-preset")
            .arg(ffmpeg_preset())
            .arg("-crf")
            .arg(ffmpeg_crf())
            .arg("-pix_fmt")
            .arg(pixel_format)
            .args(["-movflags", "+faststart"])
            .arg(&output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .with_context(|| "spawning ffmpeg raw RGBA video encoder")?;
        let stdin = child
            .stdin
            .take()
            .context("ffmpeg raw RGBA encoder did not expose stdin")?;
        Ok(Self {
            output,
            child: Some(child),
            stdin: Some(stdin),
            width,
            height,
            fps,
            frames: 0,
            last_time: None,
            finished: false,
        })
    }

    pub(crate) fn write_rgba(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        time: f64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.finished,
            "cannot write frame after ffmpeg sink finish"
        );
        anyhow::ensure!(
            width == self.width && height == self.height,
            "raw frame dimensions differ: expected {}x{}, got {}x{}",
            self.width,
            self.height,
            width,
            height
        );
        let expected = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .context("raw RGBA frame dimensions overflow")?;
        anyhow::ensure!(
            rgba.len() == expected,
            "invalid raw RGBA frame size: got {}, expected {expected}",
            rgba.len()
        );
        anyhow::ensure!(time.is_finite() && time >= 0.0, "frame time must be finite");
        if let Some(last_time) = self.last_time {
            anyhow::ensure!(
                time > last_time,
                "raw video frames must be strictly ordered: {time:.9} followed {last_time:.9}"
            );
        }

        self.stdin
            .as_mut()
            .context("ffmpeg raw RGBA stdin is closed")?
            .write_all(rgba)
            .with_context(|| format!("writing raw RGBA frame {} to ffmpeg", self.frames))?;
        self.frames += 1;
        self.last_time = Some(time);
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> anyhow::Result<VideoSinkManifest> {
        if self.finished {
            return Ok(self.manifest());
        }
        anyhow::ensure!(self.frames > 0, "cannot finish an empty raw video stream");
        self.stdin.take();
        let child = self
            .child
            .take()
            .context("ffmpeg encoder process is missing")?;
        let output = child
            .wait_with_output()
            .with_context(|| "waiting for ffmpeg raw RGBA encoder")?;
        if !output.status.success() {
            anyhow::bail!(
                "ffmpeg raw RGBA encoder failed for {}: {}",
                self.output.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let metadata = std::fs::metadata(&self.output)
            .with_context(|| format!("reading encoded MP4 {}", self.output.display()))?;
        anyhow::ensure!(
            metadata.len() > 0,
            "ffmpeg produced an empty MP4 at {}",
            self.output.display()
        );
        self.finished = true;
        Ok(self.manifest())
    }

    fn manifest(&self) -> VideoSinkManifest {
        VideoSinkManifest {
            backend: "ffmpeg-rawvideo-pipe".to_string(),
            output: self.output.display().to_string(),
            frames: self.frames,
            duration: self.frames as f64 / self.fps,
        }
    }
}

impl Drop for FfmpegRawVideoSink {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn ffmpeg_preset() -> String {
    std::env::var("AE_RENDER_FFMPEG_PRESET").unwrap_or_else(|_| "veryfast".to_string())
}

fn ffmpeg_crf() -> String {
    std::env::var("AE_RENDER_FFMPEG_CRF").unwrap_or_else(|_| "18".to_string())
}
