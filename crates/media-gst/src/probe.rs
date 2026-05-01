use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaProbe {
    pub path: String,
    pub kind: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub duration: Option<f64>,
    pub pixel_format: Option<String>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u16>,
    pub audio_duration: Option<f64>,
}

pub fn probe(path: impl AsRef<Path>) -> anyhow::Result<MediaProbe> {
    let path = path.as_ref();
    if !path.exists() {
        anyhow::bail!("media path does not exist: {}", path.display());
    }

    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_streams")
        .arg("-show_format")
        .arg(path)
        .output()
        .map_err(|err| anyhow::anyhow!("failed to run ffprobe: {err}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "ffprobe failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let raw: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let streams = raw["streams"].as_array().cloned().unwrap_or_default();
    let format_duration = raw["format"]["duration"].as_str().and_then(parse_f64);

    let video = streams
        .iter()
        .find(|stream| stream["codec_type"].as_str() == Some("video"));
    let audio = streams
        .iter()
        .find(|stream| stream["codec_type"].as_str() == Some("audio"));

    let width = video
        .and_then(|stream| stream["width"].as_u64())
        .map(|value| value as u32);
    let height = video
        .and_then(|stream| stream["height"].as_u64())
        .map(|value| value as u32);
    let fps = video
        .and_then(|stream| {
            stream["r_frame_rate"]
                .as_str()
                .or_else(|| stream["avg_frame_rate"].as_str())
        })
        .and_then(parse_ratio);
    let video_duration = video
        .and_then(|stream| stream["duration"].as_str())
        .and_then(parse_f64)
        .or(format_duration);
    let audio_duration = audio
        .and_then(|stream| stream["duration"].as_str())
        .and_then(parse_f64)
        .or(format_duration);

    Ok(MediaProbe {
        path: path.display().to_string(),
        kind: match (video.is_some(), audio.is_some()) {
            (true, true) => "video+audio",
            (true, false) => "video",
            (false, true) => "audio",
            (false, false) => "unknown",
        }
        .to_string(),
        width,
        height,
        fps,
        duration: video_duration.or(audio_duration).or(format_duration),
        pixel_format: video
            .and_then(|stream| stream["pix_fmt"].as_str())
            .map(str::to_string),
        audio_sample_rate: audio
            .and_then(|stream| stream["sample_rate"].as_str())
            .and_then(|value| value.parse::<u32>().ok()),
        audio_channels: audio
            .and_then(|stream| stream["channels"].as_u64())
            .map(|value| value as u16),
        audio_duration,
    })
}

fn parse_f64(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn parse_ratio(value: &str) -> Option<f64> {
    if value == "0/0" {
        return None;
    }
    if let Some((num, den)) = value.split_once('/') {
        let num = parse_f64(num)?;
        let den = parse_f64(den)?;
        if den == 0.0 {
            None
        } else {
            Some(num / den)
        }
    } else {
        parse_f64(value)
    }
}
