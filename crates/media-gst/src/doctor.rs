use std::process::Command;

pub fn doctor() -> anyhow::Result<Vec<String>> {
    let required = ["decodebin", "appsink", "appsrc", "videoconvert"];
    let mut lines = Vec::new();
    lines.push(format!("gstreamer.tools.gst-inspect={}", command_available("gst-inspect-1.0")));
    for name in required {
        let ok = Command::new("gst-inspect-1.0")
            .arg(name)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false);
        lines.push(format!("gstreamer.plugin.{name}={ok}"));
    }
    lines.push(format!("ffmpeg.binary={}", command_available("ffmpeg")));
    Ok(lines)
}

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}
