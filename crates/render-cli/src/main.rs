use clap::{Parser, Subcommand, ValueEnum};
use media_gst::VideoSource;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "render-cli")]
#[command(about = "AE-subset native renderer skeleton", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Doctor,
    Probe {
        path: PathBuf,
    },
    Validate {
        #[arg(long)]
        scene: PathBuf,
    },
    ValidatePayload {
        #[arg(long)]
        payload: PathBuf,
        #[arg(long)]
        strict: bool,
    },
    ImportPayload {
        #[arg(long)]
        payload: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        diagnostics: Option<PathBuf>,
    },
    ResolveAssets {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long)]
        assets_root: Option<PathBuf>,
        #[arg(long)]
        job_archive: Option<PathBuf>,
        #[arg(long)]
        strict: bool,
    },
    DumpFrames {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 5)]
        count: u32,
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        #[arg(long)]
        step: Option<f64>,
    },
    Render {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        assets_root: Option<PathBuf>,
        #[arg(long)]
        job_archive: Option<PathBuf>,
        #[arg(long)]
        mp4: Option<PathBuf>,
    },
    Mux {
        #[arg(long)]
        frames: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        fps: Option<f64>,
    },
    Compare {
        #[arg(long)]
        native: PathBuf,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        fps: Option<f64>,
        #[arg(long)]
        frames: Option<u32>,
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        #[arg(long)]
        threshold_mean: Option<f64>,
        #[arg(long)]
        threshold_max: Option<u8>,
    },
    Job {
        #[arg(long)]
        job_dir: PathBuf,
        #[arg(long)]
        payload: Option<PathBuf>,
        #[arg(long)]
        scene: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        assets_root: Option<PathBuf>,
        #[arg(long)]
        job_archive: Option<PathBuf>,
        #[arg(long)]
        mp4: Option<PathBuf>,
        #[arg(long)]
        ae_reference: Option<PathBuf>,
        #[arg(long)]
        strict: bool,
        #[arg(long, value_enum, default_value_t = RouteMode::Auto)]
        route: RouteMode,
        #[arg(long)]
        no_mp4: bool,
    },
    ListEffects,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum RouteMode {
    Auto,
    Native,
    Fallback,
}

struct CliExit {
    code: i32,
    error: anyhow::Error,
}

impl CliExit {
    fn config(error: impl Into<anyhow::Error>) -> Self {
        Self {
            code: 1,
            error: error.into(),
        }
    }

    fn render(error: impl Into<anyhow::Error>) -> Self {
        Self {
            code: 2,
            error: error.into(),
        }
    }

    fn unsupported(error: impl Into<anyhow::Error>) -> Self {
        Self {
            code: 3,
            error: error.into(),
        }
    }
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    if let Err(err) = run() {
        eprintln!("render-cli.error code={} error={:#}", err.code, err.error);
        std::process::exit(err.code);
    }
}

fn run() -> Result<(), CliExit> {
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor => doctor().map_err(CliExit::config),
        Command::Probe { path } => probe(path).map_err(CliExit::config),
        Command::Validate { scene } => validate(scene).map_err(CliExit::config),
        Command::ValidatePayload { payload, strict } => {
            validate_payload(payload, strict).map_err(CliExit::config)
        }
        Command::ImportPayload {
            payload,
            out,
            diagnostics,
        } => import_payload(payload, out, diagnostics).map_err(CliExit::config),
        Command::ResolveAssets {
            scene,
            assets_root,
            job_archive,
            strict,
        } => resolve_assets(scene, assets_root, job_archive, strict).map_err(CliExit::config),
        Command::DumpFrames {
            input,
            out,
            count,
            start,
            step,
        } => dump_frames(input, out, count, start, step).map_err(CliExit::render),
        Command::Render {
            scene,
            out,
            assets_root,
            job_archive,
            mp4,
        } => render(scene, out, assets_root, job_archive, mp4).map_err(CliExit::render),
        Command::Mux { frames, out, fps } => mux(frames, out, fps).map_err(CliExit::render),
        Command::Compare {
            native,
            reference,
            out,
            fps,
            frames,
            start,
            threshold_mean,
            threshold_max,
        } => {
            let ok = compare_outputs(
                native,
                reference,
                out,
                fps,
                frames,
                start,
                threshold_mean,
                threshold_max,
            )
            .map_err(CliExit::render)?;
            if ok {
                Ok(())
            } else {
                Err(CliExit::render(anyhow::anyhow!(
                    "conformance thresholds failed"
                )))
            }
        }
        Command::Job {
            job_dir,
            payload,
            scene,
            out,
            assets_root,
            job_archive,
            mp4,
            ae_reference,
            strict,
            route,
            no_mp4,
        } => run_job(JobOptions {
            job_dir,
            payload,
            scene,
            out,
            assets_root,
            job_archive,
            mp4,
            ae_reference,
            strict,
            route,
            no_mp4,
        }),
        Command::ListEffects => {
            for name in effects::EffectRegistry::known_match_names() {
                println!("{name}");
            }
            Ok(())
        }
    }
}

fn doctor() -> anyhow::Result<()> {
    println!("renderer.version={}", env!("CARGO_PKG_VERSION"));
    match media_gst::doctor::doctor() {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
        }
        Err(err) => {
            println!("gstreamer=error: {err:#}");
        }
    }
    println!(
        "effects.known={}",
        effects::EffectRegistry::known_match_names().len()
    );
    Ok(())
}

fn probe(path: PathBuf) -> anyhow::Result<()> {
    let info = media_gst::probe::probe(path)?;
    println!("{}", serde_json::to_string_pretty(&info)?);
    Ok(())
}

fn validate(scene_path: PathBuf) -> anyhow::Result<()> {
    let scene = render_ir::load_scene(scene_path)?;
    println!("scene.version={}", scene.version);
    println!("composition.id={}", scene.composition.id);
    println!("layers={}", scene.layers.len());
    Ok(())
}

fn validate_payload(payload_path: PathBuf, strict: bool) -> anyhow::Result<()> {
    let payload = ae_bridge::load_payload(&payload_path)?;
    let report = ae_bridge::validate_payload(&payload, strict);
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.ok {
        anyhow::bail!("payload validation failed: {}", payload_path.display());
    }
    Ok(())
}

fn import_payload(
    payload_path: PathBuf,
    out: PathBuf,
    diagnostics: Option<PathBuf>,
) -> anyhow::Result<()> {
    let payload = ae_bridge::load_payload(&payload_path)?;
    let result = ae_bridge::import_payload_to_scene(&payload)?;

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out, serde_json::to_string_pretty(&result.scene)?)?;

    if let Some(path) = diagnostics {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&result.diagnostics)?)?;
        println!("import.diagnostics={}", path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&result.diagnostics)?);
    }

    println!(
        "import.done payload={} out={}",
        payload_path.display(),
        out.display()
    );
    Ok(())
}

fn resolve_assets(
    scene_path: PathBuf,
    assets_root: Option<PathBuf>,
    job_archive: Option<PathBuf>,
    strict: bool,
) -> anyhow::Result<()> {
    let scene = render_ir::load_scene(&scene_path)?;
    let resolver = build_resolver(&scene_path, assets_root, job_archive)?;
    let mut ok = true;
    let mut assets = Vec::new();

    for asset in &scene.assets {
        let kind = asset_kind_name(&asset.kind);
        let resolution = resolver.resolve(&asset.path, Some(kind));
        if !resolution.exists {
            ok = false;
        }
        let (probe, error) = if let Some(path) = &resolution.resolved_path {
            match kind {
                "video" | "audio" => match media_gst::probe::probe(path) {
                    Ok(probe) => (Some(probe), None),
                    Err(err) => {
                        ok = false;
                        (None, Some(format!("{err:#}")))
                    }
                },
                _ => (None, None),
            }
        } else {
            (None, Some("asset was not found".to_string()))
        };
        assets.push(serde_json::json!({
            "id": asset.id.clone(),
            "type": kind,
            "path": asset.path.clone(),
            "resolution": resolution,
            "probe": probe,
            "error": error
        }));
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": ok,
            "root": resolver.root().display().to_string(),
            "assets": assets
        }))?
    );

    if strict && !ok {
        anyhow::bail!("asset resolution failed for {}", scene_path.display());
    }
    Ok(())
}

fn dump_frames(
    input: PathBuf,
    out: PathBuf,
    count: u32,
    start: f64,
    step: Option<f64>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&out)?;
    let mut source = media_gst::FfmpegVideoSource::open(&input)?;
    let info = source.info();
    let step = step.unwrap_or_else(|| {
        if info.fps > 0.0 {
            1.0 / info.fps
        } else {
            1.0 / 30.0
        }
    });

    for frame in 0..count {
        let time = start + frame as f64 * step;
        let decoded = source.frame_at(time)?;
        let canvas = raster_cpu::Canvas::from_rgba(decoded.width, decoded.height, decoded.rgba)?;
        canvas.save_png(out.join(format!("frame_{frame:06}.png")))?;
    }

    std::fs::write(
        out.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "input": input.display().to_string(),
            "width": info.width,
            "height": info.height,
            "fps": info.fps,
            "duration": info.duration,
            "frames": count,
            "start": start,
            "step": step,
            "output": "frame_%06d.png"
        }))?,
    )?;
    println!(
        "dump-frames.done input={} out={}",
        input.display(),
        out.display()
    );
    Ok(())
}

fn render(
    scene_path: PathBuf,
    out: PathBuf,
    assets_root: Option<PathBuf>,
    job_archive: Option<PathBuf>,
    mp4: Option<PathBuf>,
) -> anyhow::Result<()> {
    let scene = render_ir::load_scene(&scene_path)?;
    let fps = scene.composition.fps;
    let strict_media = assets_root.is_some() || job_archive.is_some();
    let resolver = build_resolver(&scene_path, assets_root, job_archive)?;
    let mut footage = CliFootageProvider::new(scene.assets.clone(), resolver, strict_media);
    std::fs::create_dir_all(&out)?;
    render_core::render_png_sequence_with_footage(&scene, &out, &mut footage)?;
    if let Some(mp4) = mp4 {
        render_core::mux_png_sequence_to_mp4(out.join("frames"), fps, &mp4)?;
        println!("render.mp4={}", mp4.display());
    }
    println!(
        "render.done scene={} out={}",
        scene_path.display(),
        out.display()
    );
    Ok(())
}

fn mux(frames: PathBuf, out: PathBuf, fps: Option<f64>) -> anyhow::Result<()> {
    let frames_dir = if frames.join("frames").is_dir() {
        frames.join("frames")
    } else {
        frames.clone()
    };
    let fps = fps
        .or_else(|| read_manifest_fps(&frames))
        .or_else(|| frames.parent().and_then(read_manifest_fps))
        .unwrap_or(30.0);
    render_core::mux_png_sequence_to_mp4(&frames_dir, fps, &out)?;
    println!(
        "mux.done frames={} fps={} out={}",
        frames_dir.display(),
        fps,
        out.display()
    );
    Ok(())
}

fn compare_outputs(
    native: PathBuf,
    reference: PathBuf,
    out: PathBuf,
    fps: Option<f64>,
    frames: Option<u32>,
    start: f64,
    threshold_mean: Option<f64>,
    threshold_max: Option<u8>,
) -> anyhow::Result<bool> {
    let native_frames = frames_dir_for(&native);
    anyhow::ensure!(
        native_frames.is_dir(),
        "native frames directory does not exist: {}",
        native_frames.display()
    );
    let reference_frames = reference.is_dir().then(|| frames_dir_for(&reference));
    let mut reference_video = if reference_frames.is_none() {
        Some(media_gst::FfmpegVideoSource::open(&reference)?)
    } else {
        None
    };

    let fps = fps
        .or_else(|| read_manifest_fps(&native))
        .or_else(|| native_frames.parent().and_then(read_manifest_fps))
        .or_else(|| reference_video.as_ref().map(|source| source.info().fps))
        .unwrap_or(30.0);
    let frame_count = frames.unwrap_or(count_native_frames(&native_frames)?);
    anyhow::ensure!(frame_count > 0, "no native frames found to compare");

    let diff_dir = out.join("diff");
    fs::create_dir_all(&diff_dir)?;

    let mut frame_reports = Vec::new();
    let mut max_abs_diff = 0_u8;
    let mut changed_pixels = 0_u64;
    let mut total_pixels = 0_u64;
    let mut weighted_sum = 0.0_f64;
    let mut worst_mean = (0_u32, 0.0_f64);
    let mut worst_max = (0_u32, 0_u8);

    for frame in 0..frame_count {
        let time = start + frame as f64 / fps;
        let native_path = native_frames.join(format!("frame_{frame:06}.png"));
        let native_image = testkit::load_rgba_png(&native_path)?;
        let reference_image = if let Some(reference_frames) = &reference_frames {
            testkit::load_rgba_png(reference_frames.join(format!("frame_{frame:06}.png")))?
        } else {
            let decoded = reference_video
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("reference video was not opened"))?
                .frame_at(time)?;
            testkit::RgbaImageData {
                width: decoded.width,
                height: decoded.height,
                data: decoded.rgba,
            }
        };
        anyhow::ensure!(
            native_image.width == reference_image.width
                && native_image.height == reference_image.height,
            "frame {frame} dimensions differ: native={}x{} reference={}x{}",
            native_image.width,
            native_image.height,
            reference_image.width,
            reference_image.height
        );

        let diff_rel = format!("diff/frame_{frame:06}.png");
        let diff_path = out.join(&diff_rel);
        let metrics = testkit::write_diff_png(
            &native_image.data,
            &reference_image.data,
            native_image.width,
            native_image.height,
            &diff_path,
        )?;
        max_abs_diff = max_abs_diff.max(metrics.max_abs_diff);
        changed_pixels += metrics.changed_pixels;
        total_pixels += metrics.total_pixels;
        weighted_sum += metrics.mean_abs_diff * (metrics.total_pixels * 4) as f64;
        if metrics.mean_abs_diff > worst_mean.1 {
            worst_mean = (frame, metrics.mean_abs_diff);
        }
        if metrics.max_abs_diff > worst_max.1 {
            worst_max = (frame, metrics.max_abs_diff);
        }
        frame_reports.push(json!({
            "frame": frame,
            "time": time,
            "native": format!("frames/frame_{frame:06}.png"),
            "reference": if reference_frames.is_some() {
                json!(format!("frames/frame_{frame:06}.png"))
            } else {
                json!(reference.display().to_string())
            },
            "diff": diff_rel,
            "width": native_image.width,
            "height": native_image.height,
            "max_abs_diff": metrics.max_abs_diff,
            "mean_abs_diff": metrics.mean_abs_diff,
            "changed_pixels": metrics.changed_pixels,
            "total_pixels": metrics.total_pixels
        }));
    }

    let total_components = (total_pixels * 4).max(1) as f64;
    let mean_abs_diff = weighted_sum / total_components;
    let threshold_ok = threshold_mean.map_or(true, |limit| mean_abs_diff <= limit)
        && threshold_max.map_or(true, |limit| max_abs_diff <= limit);
    let native_manifest = read_manifest_value(&native)
        .or_else(|| native_frames.parent().and_then(read_manifest_value));
    let native_features = native_manifest
        .as_ref()
        .map(manifest_features)
        .unwrap_or_else(|| json!({ "approximate": [], "unsupported": [] }));

    let report = json!({
        "ok": threshold_ok,
        "native": native.display().to_string(),
        "reference": reference.display().to_string(),
        "fps": fps,
        "start": start,
        "frames": frame_count,
        "thresholds": {
            "mean_abs_diff": threshold_mean,
            "max_abs_diff": threshold_max
        },
        "summary": {
            "max_abs_diff": max_abs_diff,
            "mean_abs_diff": mean_abs_diff,
            "changed_pixels": changed_pixels,
            "total_pixels": total_pixels,
            "changed_pixel_ratio": changed_pixels as f64 / total_pixels.max(1) as f64,
            "worst_mean_frame": worst_mean.0,
            "worst_max_frame": worst_max.0
        },
        "native_features": native_features,
        "frames_detail": frame_reports
    });
    fs::create_dir_all(&out)?;
    fs::write(out.join("report.json"), serde_json::to_string_pretty(&report)?)?;
    println!(
        "compare.done ok={} native={} reference={} report={}",
        threshold_ok,
        native.display(),
        reference.display(),
        out.join("report.json").display()
    );
    Ok(threshold_ok)
}

fn read_manifest_fps(path: &Path) -> Option<f64> {
    read_manifest_value(path).and_then(|value| value.get("fps").and_then(Value::as_f64))
}

fn read_manifest_value(path: &Path) -> Option<Value> {
    let raw = fs::read_to_string(path.join("manifest.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

fn manifest_features(manifest: &Value) -> Value {
    json!({
        "approximate": manifest.get("approximate").cloned().unwrap_or_else(|| json!([])),
        "unsupported": manifest.get("unsupported").cloned().unwrap_or_else(|| json!([]))
    })
}

fn frames_dir_for(path: &Path) -> PathBuf {
    if path.join("frames").is_dir() {
        path.join("frames")
    } else {
        path.to_path_buf()
    }
}

fn count_native_frames(frames_dir: &Path) -> anyhow::Result<u32> {
    let mut count = 0_u32;
    loop {
        let path = frames_dir.join(format!("frame_{count:06}.png"));
        if !path.exists() {
            break;
        }
        count += 1;
    }
    Ok(count)
}

struct JobOptions {
    job_dir: PathBuf,
    payload: Option<PathBuf>,
    scene: Option<PathBuf>,
    out: Option<PathBuf>,
    assets_root: Option<PathBuf>,
    job_archive: Option<PathBuf>,
    mp4: Option<PathBuf>,
    ae_reference: Option<PathBuf>,
    strict: bool,
    route: RouteMode,
    no_mp4: bool,
}

fn run_job(options: JobOptions) -> Result<(), CliExit> {
    let job_out = options.job_dir.join("out");
    fs::create_dir_all(&job_out).map_err(CliExit::config)?;
    let mut log = File::create(job_out.join("job-log.jsonl")).map_err(CliExit::config)?;
    write_job_log(
        &mut log,
        &json!({
            "event": "job.start",
            "job_dir": options.job_dir.display().to_string(),
            "route": route_name(options.route),
            "strict": options.strict
        }),
    )
    .map_err(CliExit::config)?;

    let input = prepare_job_input(&options, &job_out, &mut log)?;
    let capability_report = build_job_capability_report(&input, options.strict, options.route);
    fs::write(
        job_out.join("capabilities.json"),
        serde_json::to_string_pretty(&capability_report).map_err(CliExit::config)?,
    )
    .map_err(CliExit::config)?;

    let can_render_natively = capability_report["ok"].as_bool().unwrap_or(false);
    let unsupported = capability_report["unsupported"]
        .as_array()
        .map(|items| !items.is_empty())
        .unwrap_or(false);
    let ignored = capability_report["ignored"]
        .as_array()
        .map(|items| !items.is_empty())
        .unwrap_or(false);
    if options.route == RouteMode::Fallback || !can_render_natively {
        let reason = if unsupported {
            "unsupported native features"
        } else if options.strict && ignored {
            "ignored features in strict mode"
        } else {
            "fallback route requested"
        };
        let report = json!({
            "status": "fallback",
            "route": "fallback",
            "reason": reason,
            "capabilities": "capabilities.json"
        });
        fs::write(
            job_out.join("job-report.json"),
            serde_json::to_string_pretty(&report).map_err(CliExit::config)?,
        )
        .map_err(CliExit::config)?;
        write_job_log(
            &mut log,
            &json!({
                "event": "job.route",
                "route": "fallback",
                "reason": reason
            }),
        )
        .map_err(CliExit::config)?;
        return Err(CliExit::unsupported(anyhow::anyhow!(
            "job should be routed to AE fallback: {reason}"
        )));
    }

    let native_out = options.out.clone().unwrap_or_else(|| job_out.join("native"));
    let mp4 = if options.no_mp4 {
        None
    } else {
        Some(
            options
                .mp4
                .clone()
                .unwrap_or_else(|| job_out.join("native.mp4")),
        )
    };
    let assets_root = options
        .assets_root
        .clone()
        .or_else(|| default_assets_root(&options.job_dir));

    write_job_log(
        &mut log,
        &json!({
            "event": "job.route",
            "route": "native",
            "scene": input.scene_path.display().to_string(),
            "out": native_out.display().to_string()
        }),
    )
    .map_err(CliExit::config)?;

    render(
        input.scene_path.clone(),
        native_out.clone(),
        assets_root,
        options.job_archive.clone(),
        mp4.clone(),
    )
    .map_err(CliExit::render)?;

    let reference = options
        .ae_reference
        .clone()
        .or_else(|| default_ae_reference(&options.job_dir));
    let conformance = if let Some(reference) = reference {
        let compare_out = job_out.join("conformance");
        let ok = compare_outputs(
            native_out.clone(),
            reference.clone(),
            compare_out.clone(),
            None,
            None,
            0.0,
            None,
            None,
        )
        .map_err(CliExit::render)?;
        Some(json!({
            "ok": ok,
            "reference": reference.display().to_string(),
            "report": compare_out.join("report.json").display().to_string()
        }))
    } else {
        None
    };

    let report = json!({
        "status": "success",
        "route": "native",
        "scene": input.scene_path.display().to_string(),
        "out": native_out.display().to_string(),
        "mp4": mp4.as_ref().map(|path| path.display().to_string()),
        "capabilities": "capabilities.json",
        "conformance": conformance
    });
    fs::write(
        job_out.join("job-report.json"),
        serde_json::to_string_pretty(&report).map_err(CliExit::config)?,
    )
    .map_err(CliExit::config)?;
    write_job_log(
        &mut log,
        &json!({
            "event": "job.done",
            "status": "success",
            "report": "job-report.json"
        }),
    )
    .map_err(CliExit::config)?;
    println!(
        "job.done route=native report={}",
        job_out.join("job-report.json").display()
    );
    Ok(())
}

struct PreparedJobInput {
    scene: render_ir::Scene,
    scene_path: PathBuf,
    payload_report: Option<ae_bridge::PayloadValidationReport>,
    import_diagnostics: Option<ae_bridge::PayloadImportDiagnostics>,
    scene_features: render_core::FeatureSummary,
}

fn prepare_job_input(
    options: &JobOptions,
    job_out: &Path,
    log: &mut File,
) -> Result<PreparedJobInput, CliExit> {
    let payload_path = options
        .payload
        .clone()
        .or_else(|| existing_path(options.job_dir.join("payload.json")));
    if let Some(payload_path) = payload_path {
        let payload = ae_bridge::load_payload(&payload_path).map_err(CliExit::config)?;
        let payload_report = ae_bridge::validate_payload(&payload, false);
        if !payload_report.errors.is_empty() {
            return Err(CliExit::config(anyhow::anyhow!(
                "payload validation failed: {}",
                payload_report.errors.join("; ")
            )));
        }
        let import = ae_bridge::import_payload_to_scene(&payload).map_err(CliExit::config)?;
        let scene_path = job_out.join("scene.json");
        fs::write(
            &scene_path,
            serde_json::to_string_pretty(&import.scene).map_err(CliExit::config)?,
        )
        .map_err(CliExit::config)?;
        fs::write(
            job_out.join("import-diagnostics.json"),
            serde_json::to_string_pretty(&import.diagnostics).map_err(CliExit::config)?,
        )
        .map_err(CliExit::config)?;
        write_job_log(
            log,
            &json!({
                "event": "job.input",
                "kind": "payload",
                "payload": payload_path.display().to_string(),
                "scene": scene_path.display().to_string()
            }),
        )
        .map_err(CliExit::config)?;
        let scene_features = render_core::scene_feature_summary(&import.scene);
        return Ok(PreparedJobInput {
            scene: import.scene,
            scene_path,
            payload_report: Some(payload_report),
            import_diagnostics: Some(import.diagnostics),
            scene_features,
        });
    }

    let scene_path = options
        .scene
        .clone()
        .or_else(|| existing_path(options.job_dir.join("scene.json")))
        .ok_or_else(|| {
            CliExit::config(anyhow::anyhow!(
                "job requires --payload, --scene, payload.json, or scene.json"
            ))
        })?;
    let scene = render_ir::load_scene(&scene_path).map_err(CliExit::config)?;
    write_job_log(
        log,
        &json!({
            "event": "job.input",
            "kind": "scene",
            "scene": scene_path.display().to_string()
        }),
    )
    .map_err(CliExit::config)?;
    let scene_features = render_core::scene_feature_summary(&scene);
    Ok(PreparedJobInput {
        scene,
        scene_path,
        payload_report: None,
        import_diagnostics: None,
        scene_features,
    })
}

fn build_job_capability_report(
    input: &PreparedJobInput,
    strict: bool,
    route: RouteMode,
) -> Value {
    let mut approximate = input.scene_features.approximate.clone();
    let mut ignored = Vec::new();
    let mut unsupported = input.scene_features.unsupported.clone();

    if let Some(report) = &input.payload_report {
        collect_findings(
            &report.findings,
            &mut approximate,
            &mut ignored,
            &mut unsupported,
        );
    }
    if let Some(diagnostics) = &input.import_diagnostics {
        collect_findings(
            &diagnostics.findings,
            &mut approximate,
            &mut ignored,
            &mut unsupported,
        );
    }
    approximate.sort();
    approximate.dedup();
    ignored.sort();
    ignored.dedup();
    unsupported.sort();
    unsupported.dedup();

    json!({
        "ok": unsupported.is_empty() && (!strict || ignored.is_empty()),
        "strict": strict,
        "requested_route": route_name(route),
        "selected_route": if unsupported.is_empty()
            && (!strict || ignored.is_empty())
            && route != RouteMode::Fallback {
            "native"
        } else {
            "fallback"
        },
        "scene": {
            "path": input.scene_path.display().to_string(),
            "composition": input.scene.composition.id,
            "width": input.scene.composition.width,
            "height": input.scene.composition.height,
            "fps": input.scene.composition.fps,
            "duration": input.scene.composition.duration,
            "layers": input.scene.layers.len(),
            "assets": input.scene.assets.len()
        },
        "approximate": approximate,
        "ignored": ignored,
        "unsupported": unsupported,
        "payload_report": input.payload_report,
        "import_diagnostics": input.import_diagnostics
    })
}

fn collect_findings(
    findings: &[ae_bridge::CapabilityFinding],
    approximate: &mut Vec<String>,
    ignored: &mut Vec<String>,
    unsupported: &mut Vec<String>,
) {
    for finding in findings {
        let item = match &finding.layer {
            Some(layer) => format!("{}: {}", finding.feature, layer),
            None => finding.feature.clone(),
        };
        match finding.status {
            ae_bridge::CapabilityStatus::Approximate => approximate.push(item),
            ae_bridge::CapabilityStatus::Ignored => ignored.push(item),
            ae_bridge::CapabilityStatus::Unsupported => unsupported.push(item),
            ae_bridge::CapabilityStatus::Supported => {}
        }
    }
}

fn write_job_log(log: &mut File, value: &Value) -> anyhow::Result<()> {
    writeln!(log, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

fn route_name(route: RouteMode) -> &'static str {
    match route {
        RouteMode::Auto => "auto",
        RouteMode::Native => "native",
        RouteMode::Fallback => "fallback",
    }
}

fn existing_path(path: PathBuf) -> Option<PathBuf> {
    path.exists().then_some(path)
}

fn default_assets_root(job_dir: &Path) -> Option<PathBuf> {
    let app = job_dir.join("app");
    if app.exists() {
        Some(app)
    } else {
        Some(job_dir.to_path_buf())
    }
}

fn default_ae_reference(job_dir: &Path) -> Option<PathBuf> {
    existing_path(job_dir.join("work/output.mp4"))
}

fn build_resolver(
    scene_path: &Path,
    assets_root: Option<PathBuf>,
    job_archive: Option<PathBuf>,
) -> anyhow::Result<media_gst::JobAssetResolver> {
    if let Some(archive) = job_archive {
        return media_gst::JobAssetResolver::from_archive(archive);
    }

    let root = assets_root
        .or_else(|| scene_path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(media_gst::JobAssetResolver::from_root(root))
}

fn asset_kind_name(kind: &render_ir::AssetKind) -> &'static str {
    match kind {
        render_ir::AssetKind::Video => "video",
        render_ir::AssetKind::Audio => "audio",
        render_ir::AssetKind::Image => "image",
        render_ir::AssetKind::Font => "font",
    }
}

struct CliFootageProvider {
    assets: HashMap<String, render_ir::Asset>,
    resolver: media_gst::JobAssetResolver,
    sources: HashMap<String, media_gst::FfmpegVideoSource>,
    missing_sources: HashSet<String>,
    strict_media: bool,
}

impl CliFootageProvider {
    fn new(
        assets: Vec<render_ir::Asset>,
        resolver: media_gst::JobAssetResolver,
        strict_media: bool,
    ) -> Self {
        Self {
            assets: assets
                .into_iter()
                .map(|asset| (asset.id.clone(), asset))
                .collect(),
            resolver,
            sources: HashMap::new(),
            missing_sources: HashSet::new(),
            strict_media,
        }
    }
}

impl render_core::FootageProvider for CliFootageProvider {
    fn frame_at(&mut self, source: &str, time: f64) -> anyhow::Result<Option<raster_cpu::Canvas>> {
        if self.missing_sources.contains(source) {
            return Ok(None);
        }

        if !self.sources.contains_key(source) {
            let asset = self.assets.get(source).ok_or_else(|| {
                anyhow::anyhow!("footage source '{source}' was not found in scene assets")
            })?;
            if !matches!(asset.kind, render_ir::AssetKind::Video) {
                anyhow::bail!("footage source '{source}' points to a non-video asset");
            }
            let resolution = self.resolver.resolve(&asset.path, Some("video"));
            let Some(path) = resolution.resolved_path.clone() else {
                if self.strict_media {
                    anyhow::bail!(
                        "video asset '{}' was not found; tried: {}",
                        asset.path,
                        resolution.candidates.join(", ")
                    );
                }
                eprintln!(
                    "render.warn missing video asset '{}'; using checkerboard placeholder",
                    asset.path
                );
                self.missing_sources.insert(source.to_string());
                return Ok(None);
            };
            self.sources.insert(
                source.to_string(),
                media_gst::FfmpegVideoSource::open(path)?,
            );
        }

        let decoded = self
            .sources
            .get_mut(source)
            .ok_or_else(|| anyhow::anyhow!("video source '{source}' was not opened"))?
            .frame_at(time)?;
        Ok(Some(raster_cpu::Canvas::from_rgba(
            decoded.width,
            decoded.height,
            decoded.rgba,
        )?))
    }
}
