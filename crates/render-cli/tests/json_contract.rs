use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ae-native-renderer-{label}-{}-{nonce}",
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

fn fixture_request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/json_api/minimal_solid.request.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn run_json(cwd: &Path, request: &[u8]) -> Output {
    run_json_with_env(cwd, request, &[])
}

fn run_json_with_env(cwd: &Path, request: &[u8], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_render-cli"));
    command
        .args(["json", "--request", "-"])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(request).unwrap();
    child.wait_with_output().unwrap()
}

fn response(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must contain exactly one JSON value: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn add_required_audio_layer(
    request: &mut Value,
    file_name: &str,
    in_point: f64,
    out_point: f64,
    start_time: f64,
) {
    request["footage_layers"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "name": "required-audio",
            "type": "footage",
            "in_point": in_point,
            "out_point": out_point,
            "z_index": 2,
            "props": {},
            "effects": {},
            "text_data": {
                "layer_meta": {
                    "comp_name_target": "Comp 1",
                    "audioEnabled": true,
                    "startTime": start_time
                },
                "source_footage": {
                    "file_name": file_name,
                    "file_path": ""
                }
            }
        }));
}

fn has_capability(response: &Value, bucket: &str, code: &str, subject: &str) -> bool {
    response["capabilities"][bucket]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["code"] == code && item["subject"] == subject)
}

fn json_string_f64(value: &Value) -> f64 {
    value.as_str().unwrap().parse().unwrap()
}

fn generate_sine_audio(path: &Path, duration: f64) {
    let generated = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency=997:sample_rate=48000:duration={duration}"
        ))
        .args(["-c:a", "pcm_s16le"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "synthetic audio generation failed: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
}

fn generate_two_tone_mp3(path: &Path) {
    let generated = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "lavfi", "-i"])
        .arg("sine=frequency=220:sample_rate=48000:duration=53")
        .args(["-f", "lavfi", "-i"])
        .arg("sine=frequency=997:sample_rate=48000:duration=16")
        .args(["-filter_complex", "[0:a][1:a]concat=n=2:v=0:a=1[a]"])
        .args(["-map", "[a]", "-c:a", "libmp3lame", "-b:a", "192k"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "two-tone MP3 generation failed: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
}

fn decode_audio_f32(path: &Path) -> Vec<f32> {
    let decoded = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error"])
        .arg("-i")
        .arg(path)
        .args(["-map", "0:a:0", "-ac", "1", "-ar", "48000"])
        .args(["-c:a", "pcm_f32le", "-f", "f32le", "pipe:1"])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "audio decode failed: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    assert_eq!(decoded.stdout.len() % 4, 0);
    decoded
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

fn rms_between(samples: &[f32], start: f64, end: f64) -> f64 {
    let start = (start * 48_000.0).round() as usize;
    let end = ((end * 48_000.0).round() as usize).min(samples.len());
    let window = &samples[start.min(end)..end];
    assert!(!window.is_empty());
    (window
        .iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum::<f64>()
        / window.len() as f64)
        .sqrt()
}

fn frequency_between(samples: &[f32], start: f64, end: f64) -> f64 {
    let start = (start * 48_000.0).round() as usize;
    let end = ((end * 48_000.0).round() as usize).min(samples.len());
    let window = &samples[start.min(end)..end];
    assert!(window.len() > 1);
    let upward_crossings = window
        .windows(2)
        .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
        .count();
    upward_crossings as f64 * 48_000.0 / window.len() as f64
}

#[test]
fn validate_accepts_stdin_and_keeps_stdout_machine_readable() {
    let scratch = Scratch::new("validate-stdin");
    let mut request = fixture_request();
    request["action"] = json!("validate");
    request["outputSpec"]["directory"] = json!("out");

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert!(output.status.success());
    let response = response(&output);
    assert_eq!(response["schema"], "ae-native-renderer.render-response.v1");
    assert_eq!(response["status"], "validated");
    assert_eq!(response["ok"], true);
    assert!(response.get("render").is_none());
}

#[test]
fn invalid_json_returns_protocol_response_and_exit_one() {
    let scratch = Scratch::new("invalid-json");
    let output = run_json(&scratch.0, b"{");
    assert_eq!(output.status.code(), Some(1));
    let response = response(&output);
    assert_eq!(response["status"], "invalid");
    assert_eq!(response["ok"], false);
}

#[test]
fn unsupported_policy_error_returns_exit_three_without_rendering() {
    let scratch = Scratch::new("unsupported");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["visualOps"] = json!([{
        "id": "f3",
        "type": "hook.f3.effect.v1",
        "params": {"detected_effect_ids": ["not_a_real_effect"]},
        "required": true
    }]);
    request["policy"]["onUnsupported"] = json!("error");
    request["outputSpec"]["directory"] = json!("out");

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert_eq!(output.status.code(), Some(3));
    let response = response(&output);
    assert_eq!(response["status"], "unsupported");
    assert_eq!(response["ok"], false);
    assert!(!scratch.0.join("out").exists());
}

#[test]
fn unknown_effect_param_policy_error_returns_exit_three_without_rendering() {
    let scratch = Scratch::new("unsupported-effect-param");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["footage_layers"][0]["effects"] = json!({
        "ADBE Drop Shadow": {
            "mystery": {"value": 42}
        }
    });
    request["policy"]["onUnsupported"] = json!("error");
    request["outputSpec"]["directory"] = json!("out");

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert_eq!(output.status.code(), Some(3));
    let response = response(&output);
    assert_eq!(response["status"], "unsupported");
    assert_eq!(response["ok"], false);
    assert!(has_capability(
        &response,
        "unsupported",
        "effect_param.ADBE Drop Shadow.mystery",
        "red-solid"
    ));
    assert!(!scratch.0.join("out").exists());
}

#[test]
fn approximate_native_operation_is_allowed_by_unsupported_error_policy() {
    let scratch = Scratch::new("approximate-native");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["visualOps"] = json!([{
        "id": "f3",
        "type": "hook.f3.effect.v1",
        "params": {"detected_effect_ids": ["analog_glitch"]},
        "required": true
    }]);
    request["policy"]["onUnsupported"] = json!("error");
    request["outputSpec"]["directory"] = json!("out");

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert!(output.status.success());
    let response = response(&output);
    assert_eq!(response["status"], "rendered");
    assert_eq!(response["ok"], true);
    assert!(scratch.0.join("out/render/frames").is_dir());
}

#[test]
fn debug_spec_can_force_full_effect_stage_telemetry_for_video_output() {
    let scratch = Scratch::new("debug-spec-stage-telemetry");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["video"] = json!("result.mp4");
    request["debugSpec"] = json!({
        "captureEffectStages": true,
        "sourceLayers": true,
        "preEffects": true,
        "textMasks": true,
        "adjustmentResults": true,
        "precompResults": true,
        "finalComposite": true
    });

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert!(output.status.success());
    let response = response(&output);
    assert_eq!(response["status"], "rendered");

    let normalized: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/request.normalized.json")).unwrap())
            .unwrap();
    assert_eq!(normalized["debugSpec"]["captureEffectStages"], true);
    assert_eq!(normalized["debugSpec"]["finalComposite"], true);

    let render_log = fs::read_to_string(scratch.0.join("out/render/render-log.jsonl")).unwrap();
    let start_event: Value = serde_json::from_str(render_log.lines().next().unwrap()).unwrap();
    assert_eq!(start_event["output"]["telemetry"], "full");
    assert_eq!(start_event["output"]["stage_debug"]["effect_stages"], true);

    let frame_event: Value = render_log
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|event| event["event"] == "frame.rendered")
        .unwrap();
    let stage_paths = frame_event["stage_debug"].as_array().unwrap();
    assert!(stage_paths.iter().any(|path| path
        .as_str()
        .is_some_and(|path| path.contains("/source_layers/"))));
    assert!(stage_paths.iter().any(|path| path
        .as_str()
        .is_some_and(|path| path.contains("/pre_effects/"))));
    assert!(stage_paths.iter().any(|path| path
        .as_str()
        .is_some_and(|path| path.contains("/final_composite/"))));
    for path in stage_paths {
        let path = path.as_str().unwrap();
        assert!(
            scratch.0.join("out/render").join(path).is_file(),
            "missing stage debug image {path}"
        );
    }
}

#[test]
fn runtime_tuning_profile_changes_lowered_scene_and_is_manifested() {
    let scratch = Scratch::new("runtime-tuning");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["footage_layers"][0]["effects"] = json!({
        "ADBE Box Blur2": {
            "radius": {"value": 4.0}
        }
    });
    request["tuningSpec"] = json!({
        "profile": "builtin:p0p1-readiness",
        "overrides": {
            "effects": {
                "blur": {
                    "box_radius_multiplier": 2.5
                }
            }
        }
    });

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response = response(&output);
    assert_eq!(response["status"], "rendered");
    assert!(response["artifacts"]["tuning_profile"]
        .as_str()
        .is_some_and(|path| path.ends_with("tuning-profile.resolved.json")));

    let scene: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/scene.json")).unwrap()).unwrap();
    assert_eq!(
        scene["layers"][0]["effects"][0]["params"]["radius"]["value"],
        10.0
    );

    let tuning: Value = serde_json::from_slice(
        &fs::read(scratch.0.join("out/tuning-profile.resolved.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        tuning["profile"]["effects"]["blur"]["box_radius_multiplier"],
        2.5
    );
    assert!(tuning["report"]["applied"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "effects.blur.box_radius_multiplier"));

    let manifest: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/output-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["artifacts"]["tuning_profile"]["path"],
        "tuning-profile.resolved.json"
    );
}

#[test]
fn unknown_runtime_tuning_key_fails_before_output() {
    let scratch = Scratch::new("runtime-tuning-unknown");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["tuningSpec"] = json!({
        "profile": "builtin:p0p1-readiness",
        "overrides": {
            "f3": {
                "analog_glitch": {
                    "mystery_knob": 42.0
                }
            }
        }
    });

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert_eq!(output.status.code(), Some(1));
    let response = response(&output);
    assert_eq!(response["status"], "invalid");
    assert_eq!(response["ok"], false);
    assert!(response["errors"][0]
        .as_str()
        .unwrap()
        .contains("unsupported tuning key f3.analog_glitch.mystery_knob"));
    assert!(!scratch.0.join("out").exists());
}

#[test]
fn identical_request_produces_identical_manifest_in_different_roots() {
    let first = Scratch::new("determinism-a");
    let second = Scratch::new("determinism-b");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]
        .as_object_mut()
        .unwrap()
        .remove("video");
    let bytes = serde_json::to_vec(&request).unwrap();

    let output_a = run_json(&first.0, &bytes);
    let output_b = run_json(&second.0, &bytes);
    assert!(output_a.status.success());
    assert!(output_b.status.success());
    assert_eq!(response(&output_a)["status"], "rendered");
    assert_eq!(response(&output_b)["status"], "rendered");

    let manifest_a = fs::read(first.0.join("out/output-manifest.json")).unwrap();
    let manifest_b = fs::read(second.0.join("out/output-manifest.json")).unwrap();
    assert_eq!(manifest_a, manifest_b);
    let manifest: Value = serde_json::from_slice(&manifest_a).unwrap();
    assert_eq!(manifest["artifacts"]["frames"]["path"], "render/frames");
    assert_eq!(manifest["artifacts"]["frames"]["count"], 3);
}

#[test]
fn exact_frame_selection_keeps_full_duration_and_renders_source_frame_number() {
    let scratch = Scratch::new("exact-frame");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["compsSpec"][0]["dur"] = json!(2.0);
    request["footage_layers"][0]["out_point"] = json!(2.0);
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["frames"] = json!([41]);

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response = response(&output);
    assert_eq!(response["status"], "rendered");
    assert_eq!(response["scene"]["duration"], 2.0);
    assert_eq!(response["scene"]["frame_count"], 48);

    let frames_dir = scratch.0.join("out/render/frames");
    let mut frame_names = fs::read_dir(&frames_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    frame_names.sort();
    assert_eq!(frame_names, ["frame_000041.png"]);

    let output_manifest: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/output-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(output_manifest["composition"]["duration"], 2.0);
    assert_eq!(output_manifest["composition"]["frame_count"], 48);
    assert_eq!(output_manifest["artifacts"]["frames"]["count"], 1);

    let render_manifest: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/render/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(render_manifest["duration"], 2.0);
    assert_eq!(render_manifest["frames"], 48);
    assert_eq!(render_manifest["rendered_frames"], 1);
    assert_eq!(render_manifest["selected_frames"], json!([41]));
    assert_eq!(render_manifest["timing"]["frames"][0]["frame"], 41);

    let events = fs::read_to_string(scratch.0.join("out/render/render-log.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(events[0]["event"], "render.start");
    assert_eq!(events[0]["frames"], 48);
    assert_eq!(events[0]["rendered_frames"], 1);
    assert_eq!(events[0]["selected_frames"], json!([41]));
    assert_eq!(events[1]["event"], "frame.rendered");
    assert_eq!(events[1]["frame"], 41);
    assert_eq!(events[2]["event"], "render.done");
    assert_eq!(events[2]["frames"], 1);
    assert_eq!(events[2]["composition_frames"], 48);
    assert_eq!(events[2]["selected_frames"], json!([41]));
}

#[test]
fn exact_frame_selection_is_sorted_and_deduplicated() {
    let scratch = Scratch::new("canonical-frames");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["frames"] = json!([2, 0, 2]);

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let normalized: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/request.normalized.json")).unwrap())
            .unwrap();
    assert_eq!(normalized["outputSpec"]["frames"], json!([0, 2]));

    let mut frame_names = fs::read_dir(scratch.0.join("out/render/frames"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    frame_names.sort();
    assert_eq!(frame_names, ["frame_000000.png", "frame_000002.png"]);
}

#[test]
fn out_of_range_frame_is_rejected_before_render() {
    let scratch = Scratch::new("out-of-range-frame");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["frames"] = json!([3]);

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert_eq!(output.status.code(), Some(1));
    let response = response(&output);
    assert_eq!(response["status"], "invalid");
    assert_eq!(response["ok"], false);
    assert!(response["errors"][0]
        .as_str()
        .unwrap()
        .contains("out-of-range frame 3"));
    assert!(!scratch.0.join("out").exists());
}

#[test]
fn video_with_exact_frames_is_rejected_before_render() {
    let scratch = Scratch::new("video-with-frames");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["video"] = json!("result.mp4");
    request["outputSpec"]["frames"] = json!([1]);

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert_eq!(output.status.code(), Some(1));
    let response = response(&output);
    assert_eq!(response["status"], "invalid");
    assert_eq!(response["ok"], false);
    assert!(response["errors"][0]
        .as_str()
        .unwrap()
        .contains("outputSpec.frames cannot be combined with outputSpec.video"));
    assert!(!scratch.0.join("out").exists());
}

#[test]
fn render_request_can_return_mp4_without_polluting_stdout() {
    if !command_available("ffmpeg") {
        return;
    }

    let scratch = Scratch::new("mp4");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["video"] = json!("result.mp4");
    let output = run_json_with_env(
        &scratch.0,
        &serde_json::to_vec(&request).unwrap(),
        &[("AE_RENDER_MUX_BACKEND", "ffmpeg")],
    );

    assert!(output.status.success());
    let response = response(&output);
    assert_eq!(response["status"], "rendered");
    let video = scratch.0.join("out/result.mp4");
    assert!(fs::metadata(video).unwrap().len() > 0);
}

#[test]
fn required_audio_stays_not_implemented_without_video_output() {
    let scratch = Scratch::new("audio-no-video");
    let mut request = fixture_request();
    request["action"] = json!("render");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]
        .as_object_mut()
        .unwrap()
        .remove("video");
    add_required_audio_layer(&mut request, "missing.wav", 0.0, 0.125, 0.0);

    let output = run_json(&scratch.0, &serde_json::to_vec(&request).unwrap());
    assert!(output.status.success());
    let response = response(&output);
    assert_eq!(response["status"], "partial");
    assert_eq!(response["complete"], false);
    assert!(has_capability(
        &response,
        "not_implemented",
        "layer.audio",
        "required-audio"
    ));
    assert!(has_capability(
        &response,
        "not_implemented",
        "import.skip_audio",
        "required-audio"
    ));
    assert!(!has_capability(
        &response,
        "supported",
        "layer.audio",
        "required-audio"
    ));
}

#[test]
fn render_muxes_trimmed_and_offset_audio_into_mp4() {
    if !command_available("ffmpeg") || !command_available("ffprobe") {
        return;
    }

    let scratch = Scratch::new("audio-mp4");
    let media_dir = scratch.0.join("media/audio");
    fs::create_dir_all(&media_dir).unwrap();
    let audio_path = media_dir.join("tone.wav");
    generate_sine_audio(&audio_path, 1.0);

    let mut request = fixture_request();
    request["action"] = json!("render");
    request["compsSpec"][0]["dur"] = json!(1.5);
    request["footage_layers"][0]["out_point"] = json!(1.5);
    request["assetsSpec"]["root"] = json!(".");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["video"] = json!("result.mp4");
    // The layer starts at 0.5s and seeks to 0.75s in the one-second source.
    // Only the final 0.25s of source audio should therefore reach the output.
    add_required_audio_layer(&mut request, "tone.wav", 0.5, 1.25, -0.25);

    let output = run_json_with_env(
        &scratch.0,
        &serde_json::to_vec(&request).unwrap(),
        &[("AE_RENDER_MUX_BACKEND", "ffmpeg")],
    );
    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response = response(&output);
    assert_eq!(response["status"], "rendered");
    assert_eq!(response["complete"], true);
    assert!(has_capability(
        &response,
        "supported",
        "layer.audio",
        "required-audio"
    ));
    assert!(has_capability(
        &response,
        "supported",
        "import.skip_audio",
        "required-audio"
    ));
    assert!(!has_capability(
        &response,
        "not_implemented",
        "layer.audio",
        "required-audio"
    ));

    let video_path = scratch.0.join("out/result.mp4");
    let probe_output = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries"])
        .arg("stream=codec_name,codec_type,start_time,duration:format=duration")
        .args(["-of", "json"])
        .arg(&video_path)
        .output()
        .unwrap();
    assert!(
        probe_output.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&probe_output.stderr)
    );
    let probe: Value = serde_json::from_slice(&probe_output.stdout).unwrap();
    let streams = probe["streams"].as_array().unwrap();
    let video_stream = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .unwrap();
    let audio_stream = streams
        .iter()
        .find(|stream| stream["codec_type"] == "audio")
        .unwrap();
    assert_eq!(audio_stream["codec_name"], "aac");
    assert!((json_string_f64(&video_stream["duration"]) - 1.5).abs() < 0.05);

    let audio_start = json_string_f64(&audio_stream["start_time"]);
    let audio_duration = json_string_f64(&audio_stream["duration"]);
    assert!(
        (audio_start - 0.5).abs() < 0.06,
        "audio timeline offset was {audio_start}"
    );
    assert!(
        (audio_start + audio_duration - 0.75).abs() < 0.08,
        "audio source trim ended at {}",
        audio_start + audio_duration
    );
    assert!((json_string_f64(&probe["format"]["duration"]) - 1.5).abs() < 0.05);

    let capabilities: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/capabilities.json")).unwrap())
            .unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("out/output-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(capabilities["complete"], true);
    assert_eq!(manifest["capabilities"]["complete"], true);
    assert_eq!(manifest["artifacts"]["video"]["path"], "result.mp4");
    assert_eq!(manifest["output"]["video"]["path"], "result.mp4");
    assert_eq!(manifest["output"]["frame_count"], 36);
    assert_eq!(manifest["output"]["audio_probe"]["codec"], "aac");
    assert_eq!(manifest["output"]["audio_probe"]["channels"], 1);
}

#[test]
fn brat_audio_uses_source_time_53_local_fallback_and_envelope() {
    if !command_available("ffmpeg") || !command_available("ffprobe") {
        return;
    }

    let scratch = Scratch::new("brat-audio-53");
    let media_dir = scratch.0.join("media/audio");
    fs::create_dir_all(&media_dir).unwrap();
    generate_two_tone_mp3(&media_dir.join("audio_source.mp3"));

    let mut request = fixture_request();
    request["action"] = json!("render");
    request["compsSpec"][0]["dur"] = json!(15.0);
    request["footage_layers"][0]["out_point"] = json!(15.0);
    request["assetsSpec"]["root"] = json!(".");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["video"] = json!("result.mp4");
    request["policy"]["onUnsupported"] = json!("error");
    add_required_audio_layer(&mut request, "audio_source.mp3", 0.0, 15.0, -53.0);
    let audio_layer = request["footage_layers"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    audio_layer["text_data"]["source_footage"]["file_path"] =
        json!("/app/work/jobs/job/data/inputs/audio/original-production-name.mp3");
    audio_layer["text_data"]["audio_envelope"] = json!({
        "fade_in_s": 0.5,
        "fade_out_s": 0.5,
        "min_db": -48.0
    });

    let output = run_json_with_env(
        &scratch.0,
        &serde_json::to_vec(&request).unwrap(),
        &[("AE_RENDER_MUX_BACKEND", "ffmpeg")],
    );
    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response = response(&output);
    assert_eq!(response["status"], "rendered");
    assert_eq!(response["complete"], true);
    let audio_capability = response["capabilities"]["supported"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "layer.audio" && item["subject"] == "required-audio")
        .unwrap();
    let detail = audio_capability["detail"].as_str().unwrap();
    assert!(detail.contains("source_start=53.000000"));
    assert!(detail.contains("fade_in=0.500000"));
    assert!(detail.contains("fade_out=0.500000"));

    let video_path = scratch.0.join("out/result.mp4");
    let probe_output = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries"])
        .arg("stream=codec_type,start_time,duration:format=duration")
        .args(["-of", "json"])
        .arg(&video_path)
        .output()
        .unwrap();
    assert!(probe_output.status.success());
    let probe: Value = serde_json::from_slice(&probe_output.stdout).unwrap();
    let audio_stream = probe["streams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stream| stream["codec_type"] == "audio")
        .unwrap();
    assert!(json_string_f64(&audio_stream["start_time"]).abs() < 0.05);
    assert!((json_string_f64(&audio_stream["duration"]) - 15.0).abs() < 0.08);
    assert!((json_string_f64(&probe["format"]["duration"]) - 15.0).abs() < 0.05);

    let samples = decode_audio_f32(&video_path);
    assert!(samples.len() >= 48_000 * 14);
    let frequency = frequency_between(&samples, 1.0, 2.0);
    assert!(
        (frequency - 997.0).abs() < 25.0,
        "expected the source fragment at 53s (997Hz), got {frequency}Hz"
    );
    let middle_rms = rms_between(&samples, 7.0, 7.1);
    let head_rms = rms_between(&samples, 0.02, 0.1);
    let tail_rms = rms_between(&samples, 14.9, 14.98);
    assert!(
        head_rms < middle_rms * 0.4,
        "fade-in RMS {head_rms} was not below middle RMS {middle_rms}"
    );
    assert!(
        tail_rms < middle_rms * 0.4,
        "fade-out RMS {tail_rms} was not below middle RMS {middle_rms}"
    );
}

#[test]
fn empty_audio_seek_does_not_promote_mux_capability() {
    if !command_available("ffmpeg") || !command_available("ffprobe") {
        return;
    }

    let scratch = Scratch::new("audio-empty-seek");
    let media_dir = scratch.0.join("media/audio");
    fs::create_dir_all(&media_dir).unwrap();
    generate_sine_audio(&media_dir.join("short.wav"), 0.1);

    let mut request = fixture_request();
    request["action"] = json!("render");
    request["compsSpec"][0]["dur"] = json!(0.5);
    request["footage_layers"][0]["out_point"] = json!(0.5);
    request["assetsSpec"]["root"] = json!(".");
    request["outputSpec"]["directory"] = json!("out");
    request["outputSpec"]["video"] = json!("result.mp4");
    add_required_audio_layer(&mut request, "short.wav", 0.0, 0.5, -2.0);

    let output = run_json_with_env(
        &scratch.0,
        &serde_json::to_vec(&request).unwrap(),
        &[("AE_RENDER_MUX_BACKEND", "ffmpeg")],
    );
    assert!(output.status.success());
    let response = response(&output);
    assert_eq!(response["status"], "partial");
    assert_eq!(response["complete"], false);
    assert!(has_capability(
        &response,
        "not_implemented",
        "layer.audio",
        "required-audio"
    ));
    assert!(!has_capability(
        &response,
        "supported",
        "layer.audio",
        "required-audio"
    ));
    assert!(response["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning
            .as_str()
            .is_some_and(|warning| warning.contains("without an audio stream"))));

    let probe_output = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "stream=codec_type"])
        .args(["-of", "json"])
        .arg(scratch.0.join("out/result.mp4"))
        .output()
        .unwrap();
    assert!(probe_output.status.success());
    let probe: Value = serde_json::from_slice(&probe_output.stdout).unwrap();
    assert!(probe["streams"]
        .as_array()
        .unwrap()
        .iter()
        .all(|stream| stream["codec_type"] != "audio"));
}
