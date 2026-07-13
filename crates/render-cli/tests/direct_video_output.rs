use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ae-native-renderer-direct-video-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn production_json_streams_ordered_rgba_directly_to_mp4_without_png_frames() {
    if !command_available("ffmpeg") {
        return;
    }

    let scratch = Scratch::new();
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["video"] = json!("result.mp4");

    let mut child = Command::new(env!("CARGO_BIN_EXE_render-cli"))
        .args(["json", "--request", "-"])
        .current_dir(&scratch.0)
        .env("AE_RENDER_MUX_BACKEND", "ffmpeg")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&request).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "direct render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "rendered");
    assert_eq!(response["ok"], true);

    let output_dir = scratch.0.join("out");
    let video = output_dir.join("result.mp4");
    assert!(fs::metadata(&video).unwrap().len() > 0);
    assert!(
        !output_dir.join("render/frames").exists(),
        "direct video output must not materialize PNG frames"
    );

    let mux_report: Value =
        serde_json::from_slice(&fs::read(output_dir.join("render/mux-report.json")).unwrap())
            .unwrap();
    assert_eq!(mux_report["mode"], "render-direct");
    assert_eq!(mux_report["backend"], "ffmpeg-rawvideo-pipe");
    assert_eq!(mux_report["input"]["kind"], "render_core_callback");
    assert_eq!(mux_report["sink_manifest"]["frames"], 3);

    let render_manifest: Value =
        serde_json::from_slice(&fs::read(output_dir.join("render/manifest.json")).unwrap())
            .unwrap();
    let rendered_frames = render_manifest["timing"]["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| frame["frame"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rendered_frames, [0, 1, 2]);

    let output_manifest: Value =
        serde_json::from_slice(&fs::read(output_dir.join("output-manifest.json")).unwrap())
            .unwrap();
    assert!(output_manifest["artifacts"].get("frames").is_none());
    assert_eq!(output_manifest["artifacts"]["video"]["path"], "result.mp4");
    assert_eq!(output_manifest["output"]["frame_count"], 3);
}

fn fixture_request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/json_api/minimal_solid.request.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
