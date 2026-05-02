use crate::probe::probe;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Instant;

const DEFAULT_CACHE_CAPACITY: usize = 4;
const MAX_SEQUENTIAL_DECODE_GAP: u64 = 180;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoSourceStats {
    pub requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub frames_decoded: u64,
    pub decoder_spawns: u64,
    pub decoder_restarts: u64,
    pub decoder_parks: u64,
    pub sequential_frames_skipped: u64,
    pub max_cache_entries: usize,
    pub max_sequential_decode_gap: u64,
    pub request_ms: f64,
    pub cache_hit_ms: f64,
    pub cache_miss_ms: f64,
    pub decoder_spawn_ms: f64,
    pub frame_read_ms: f64,
    pub max_request_ms: f64,
    pub max_frame_read_ms: f64,
}

pub trait VideoSource {
    fn info(&self) -> VideoInfo;
    fn frame_at(&mut self, time: f64) -> anyhow::Result<VideoFrame>;

    fn stats(&self) -> VideoSourceStats {
        VideoSourceStats::default()
    }
}

pub struct GstVideoSourceTodo;

pub struct FfmpegVideoSource {
    path: PathBuf,
    info: VideoInfo,
    max_sequential_decode_gap: u64,
    decoder: Option<FfmpegPipe>,
    cache: FrameCache,
    stats: VideoSourceStats,
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
            max_sequential_decode_gap: media_sequential_decode_gap(),
            decoder: None,
            cache: FrameCache::new(media_frame_cache_capacity()),
            stats: VideoSourceStats {
                max_cache_entries: media_frame_cache_capacity(),
                max_sequential_decode_gap: media_sequential_decode_gap(),
                ..VideoSourceStats::default()
            },
        })
    }

    pub fn backend_name(&self) -> &'static str {
        "ffmpeg-persistent-pipe"
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn decoder_is_running(&self) -> bool {
        self.decoder.is_some()
    }

    pub fn park_decoder(&mut self) {
        if self.decoder.is_some() {
            self.stats.decoder_parks += 1;
        }
        self.stop_decoder();
    }

    fn frame_index_for_time(&self, time: f64) -> u64 {
        let fps = self.info.fps.max(0.000_001);
        let frame_duration = 1.0 / fps;
        let max_time = (self.info.duration - frame_duration).max(0.0);
        let clamped = time.clamp(0.0, max_time);
        let frame = (clamped * fps).round().max(0.0) as u64;
        let max_frame = if self.info.duration > 0.0 {
            (self.info.duration * fps).ceil().max(1.0) as u64 - 1
        } else {
            frame
        };
        frame.min(max_frame)
    }

    fn time_for_frame_index(&self, frame_index: u64) -> f64 {
        frame_index as f64 / self.info.fps.max(0.000_001)
    }

    fn ensure_decoder_at(&mut self, frame_index: u64) -> anyhow::Result<()> {
        let restart = match self.decoder.as_ref() {
            Some(decoder) => {
                frame_index < decoder.next_frame
                    || frame_index.saturating_sub(decoder.next_frame)
                        > self.max_sequential_decode_gap
            }
            None => true,
        };

        if !restart {
            return Ok(());
        }

        if self.decoder.is_some() {
            self.stats.decoder_restarts += 1;
        }
        self.stop_decoder();

        let time = self.time_for_frame_index(frame_index);
        let spawn_started = Instant::now();
        let mut child = Command::new("ffmpeg")
            .arg("-v")
            .arg("error")
            .arg("-ss")
            .arg(format!("{time:.6}"))
            .arg("-i")
            .arg(&self.path)
            .arg("-an")
            .arg("-sn")
            .arg("-dn")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("rgba")
            .arg("pipe:1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| anyhow::anyhow!("failed to spawn ffmpeg decoder: {err}"))?;
        self.stats.decoder_spawn_ms += elapsed_ms(spawn_started);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg decoder stdout was not captured"))?;
        self.decoder = Some(FfmpegPipe {
            child,
            stdout,
            next_frame: frame_index,
        });
        self.stats.decoder_spawns += 1;
        Ok(())
    }

    fn read_next_frame(&mut self) -> anyhow::Result<(u64, VideoFrame)> {
        let expected = (self.info.width * self.info.height * 4) as usize;
        let decoder = self
            .decoder
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg decoder was not started"))?;
        let frame_index = decoder.next_frame;
        let mut rgba = vec![0_u8; expected];
        let read_started = Instant::now();
        decoder.stdout.read_exact(&mut rgba).map_err(|err| {
            anyhow::anyhow!(
                "ffmpeg decoder ended before frame {} for {}: {err}",
                frame_index,
                self.path.display()
            )
        })?;
        let read_ms = elapsed_ms(read_started);
        self.stats.frame_read_ms += read_ms;
        self.stats.max_frame_read_ms = self.stats.max_frame_read_ms.max(read_ms);
        decoder.next_frame += 1;
        self.stats.frames_decoded += 1;
        Ok((
            frame_index,
            VideoFrame {
                width: self.info.width,
                height: self.info.height,
                pts: self.time_for_frame_index(frame_index),
                rgba,
            },
        ))
    }

    fn stop_decoder(&mut self) {
        if let Some(mut decoder) = self.decoder.take() {
            let _ = decoder.child.kill();
            let _ = decoder.child.wait();
        }
    }
}

impl Drop for FfmpegVideoSource {
    fn drop(&mut self) {
        self.stop_decoder();
    }
}

impl VideoSource for FfmpegVideoSource {
    fn info(&self) -> VideoInfo {
        self.info.clone()
    }

    fn frame_at(&mut self, time: f64) -> anyhow::Result<VideoFrame> {
        let request_started = Instant::now();
        self.stats.requests += 1;
        let frame_index = self.frame_index_for_time(time);

        if let Some(frame) = self.cache.get(frame_index) {
            self.stats.cache_hits += 1;
            let elapsed = elapsed_ms(request_started);
            self.stats.cache_hit_ms += elapsed;
            self.stats.request_ms += elapsed;
            self.stats.max_request_ms = self.stats.max_request_ms.max(elapsed);
            return Ok(frame);
        }

        self.stats.cache_misses += 1;
        self.ensure_decoder_at(frame_index)?;

        loop {
            let (decoded_index, frame) = self.read_next_frame()?;
            self.cache.insert(decoded_index, frame.clone());
            if decoded_index == frame_index {
                let elapsed = elapsed_ms(request_started);
                self.stats.cache_miss_ms += elapsed;
                self.stats.request_ms += elapsed;
                self.stats.max_request_ms = self.stats.max_request_ms.max(elapsed);
                return Ok(frame);
            }
            self.stats.sequential_frames_skipped += 1;
        }
    }

    fn stats(&self) -> VideoSourceStats {
        self.stats.clone()
    }
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn media_frame_cache_capacity() -> usize {
    std::env::var("AE_RENDER_MEDIA_FRAME_CACHE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CACHE_CAPACITY)
}

fn media_sequential_decode_gap() -> u64 {
    std::env::var("AE_RENDER_MAX_SEQUENTIAL_DECODE_GAP")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(MAX_SEQUENTIAL_DECODE_GAP)
}

struct FfmpegPipe {
    child: Child,
    stdout: ChildStdout,
    next_frame: u64,
}

struct FrameCache {
    capacity: usize,
    order: VecDeque<u64>,
    frames: HashMap<u64, VideoFrame>,
}

impl FrameCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::new(),
            frames: HashMap::new(),
        }
    }

    fn get(&mut self, frame_index: u64) -> Option<VideoFrame> {
        let frame = self.frames.get(&frame_index).cloned()?;
        self.order.retain(|existing| *existing != frame_index);
        self.order.push_back(frame_index);
        Some(frame)
    }

    fn insert(&mut self, frame_index: u64, frame: VideoFrame) {
        if self.capacity == 0 {
            return;
        }
        if self.frames.contains_key(&frame_index) {
            self.order.retain(|existing| *existing != frame_index);
            self.order.push_back(frame_index);
            self.frames.insert(frame_index, frame);
            return;
        }
        while self.frames.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.frames.remove(&oldest);
            } else {
                break;
            }
        }
        self.order.push_back(frame_index);
        self.frames.insert(frame_index, frame);
    }
}
