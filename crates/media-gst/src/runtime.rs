use crate::video_source::VideoFrame;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameRequest {
    pub render_frame: u32,
    pub render_time: f64,
    pub source_time: f64,
    pub composition: String,
    pub layer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePlan {
    pub asset_id: String,
    pub requests: Vec<FrameRequest>,
    pub first_render_frame: Option<u32>,
    pub last_render_frame: Option<u32>,
    pub first_source_time: Option<f64>,
    pub last_source_time: Option<f64>,
    pub unique_source_times: usize,
}

impl SourcePlan {
    pub fn new(asset_id: impl Into<String>, mut requests: Vec<FrameRequest>) -> Self {
        requests.sort_by(|a, b| {
            a.render_frame
                .cmp(&b.render_frame)
                .then_with(|| a.source_time.total_cmp(&b.source_time))
                .then_with(|| a.layer_id.cmp(&b.layer_id))
        });
        let first_render_frame = requests.first().map(|request| request.render_frame);
        let last_render_frame = requests.last().map(|request| request.render_frame);
        let first_source_time = requests
            .iter()
            .map(|request| request.source_time)
            .min_by(|a, b| a.total_cmp(b));
        let last_source_time = requests
            .iter()
            .map(|request| request.source_time)
            .max_by(|a, b| a.total_cmp(b));
        let mut unique_times = requests
            .iter()
            .map(|request| quantize_time(request.source_time))
            .collect::<Vec<_>>();
        unique_times.sort_unstable();
        unique_times.dedup();

        Self {
            asset_id: asset_id.into(),
            requests,
            first_render_frame,
            last_render_frame,
            first_source_time,
            last_source_time,
            unique_source_times: unique_times.len(),
        }
    }

    pub fn first_source_time(&self) -> Option<f64> {
        self.first_source_time
    }

    pub fn first_request_source_time(&self) -> Option<f64> {
        self.requests.first().map(|request| request.source_time)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineMediaPlan {
    pub fps: f64,
    pub duration: f64,
    pub frames: u32,
    pub total_requests: usize,
    pub frames_with_requests: usize,
    pub max_requests_per_frame: usize,
    pub sources: Vec<SourcePlan>,
}

impl TimelineMediaPlan {
    pub fn new(
        fps: f64,
        duration: f64,
        frames: u32,
        frames_with_requests: usize,
        max_requests_per_frame: usize,
        mut sources: Vec<SourcePlan>,
    ) -> Self {
        sources.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        let total_requests = sources
            .iter()
            .map(|source| source.requests.len())
            .sum::<usize>();
        Self {
            fps,
            duration,
            frames,
            total_requests,
            frames_with_requests,
            max_requests_per_frame,
            sources,
        }
    }

    pub fn source_plan(&self, asset_id: &str) -> Option<&SourcePlan> {
        self.sources.iter().find(|source| source.asset_id == asset_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSinkManifest {
    pub backend: String,
    pub output: String,
    pub frames: u32,
    pub duration: f64,
}

pub trait VideoSink {
    fn write_frame(&mut self, frame: &VideoFrame, time: f64) -> anyhow::Result<()>;
    fn finish(&mut self) -> anyhow::Result<VideoSinkManifest>;
}

fn quantize_time(time: f64) -> i64 {
    (time * 1_000_000.0).round() as i64
}
