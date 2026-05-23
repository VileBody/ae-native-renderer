use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use media_gst::{VideoSink, VideoSource};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

mod conformance_pack;
mod media_plan;

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
    ConformancePack {
        #[arg(long, default_value = "fixtures/ae_conformance_pack")]
        pack: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "case")]
        cases: Vec<String>,
        #[arg(long)]
        threshold_mean: Option<f64>,
        #[arg(long)]
        threshold_max: Option<u8>,
        #[arg(long)]
        fail_on_diff: bool,
    },
    P2TextJournal {
        #[arg(long, default_value = "fixtures/ae_conformance_pack")]
        pack: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "case")]
        cases: Vec<String>,
        #[arg(long)]
        full_events: bool,
        #[arg(long)]
        ae_ref_root: Option<PathBuf>,
    },
    HypothesisPack {
        #[arg(long)]
        module: String,
        #[arg(long)]
        candidate: String,
        #[arg(long, value_enum, default_value_t = HypothesisStatusArg::Instrumented)]
        status: HypothesisStatusArg,
        #[arg(long, value_enum, default_value_t = HypothesisGateKindArg::Isolated)]
        gate: HypothesisGateKindArg,
        #[arg(long)]
        question: Option<String>,
        #[arg(long)]
        hypothesis: Option<String>,
        #[arg(long = "evidence")]
        evidence_sources: Vec<String>,
        #[arg(long = "note")]
        notes: Vec<String>,
        #[arg(long)]
        candidate_config: Option<PathBuf>,
        #[arg(long, default_value = "fixtures/ae_conformance_pack")]
        pack: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "case")]
        cases: Vec<String>,
        #[arg(long)]
        threshold_mean: Option<f64>,
        #[arg(long)]
        threshold_max: Option<u8>,
        #[arg(long)]
        fail_on_diff: bool,
    },
    TurbulentSamples {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
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

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum HypothesisStatusArg {
    CandidateReady,
    Instrumented,
    IsolatedImproved,
    CompositionImproved,
    StackSafe,
    NeedsNewProbe,
    Rejected,
    ReverseImplemented,
    ParityLocked,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum HypothesisGateKindArg {
    Isolated,
    Composition,
    Stack,
    Template,
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
        Command::ConformancePack {
            pack,
            out,
            cases,
            threshold_mean,
            threshold_max,
            fail_on_diff,
        } => {
            let ok = conformance_pack::run_pack(conformance_pack::RunOptions {
                pack,
                out,
                cases,
                threshold_mean,
                threshold_max,
            })
            .map_err(CliExit::render)?;
            if fail_on_diff && !ok {
                Err(CliExit::render(anyhow::anyhow!(
                    "conformance pack thresholds failed"
                )))
            } else {
                Ok(())
            }
        }
        Command::P2TextJournal {
            pack,
            out,
            cases,
            full_events,
            ae_ref_root,
        } => {
            conformance_pack::run_p2_text_journal(conformance_pack::P2TextJournalOptions {
                pack,
                out,
                cases,
                full_events,
                ae_ref_root,
            })
            .map_err(CliExit::render)?;
            Ok(())
        }
        Command::HypothesisPack {
            module,
            candidate,
            status,
            gate,
            question,
            hypothesis,
            evidence_sources,
            notes,
            candidate_config,
            pack,
            out,
            cases,
            threshold_mean,
            threshold_max,
            fail_on_diff,
        } => {
            let ok = run_hypothesis_pack(HypothesisPackOptions {
                module,
                candidate,
                status,
                gate,
                question,
                hypothesis,
                evidence_sources,
                notes,
                candidate_config,
                pack,
                out,
                cases,
                threshold_mean,
                threshold_max,
            })
            .map_err(CliExit::render)?;
            if fail_on_diff && !ok {
                Err(CliExit::render(anyhow::anyhow!(
                    "hypothesis conformance thresholds failed"
                )))
            } else {
                Ok(())
            }
        }
        Command::TurbulentSamples { request, out } => {
            turbulent_samples(request, out).map_err(CliExit::render)
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

#[derive(Debug, Deserialize)]
struct TurbulentSamplesRequest {
    #[serde(default)]
    schema: Option<String>,
    width: u32,
    height: u32,
    frames: Vec<TurbulentSamplesFrameRequest>,
}

#[derive(Debug, Deserialize)]
struct TurbulentSamplesFrameRequest {
    #[serde(default)]
    case_id: Option<String>,
    frame: u32,
    time: f64,
    #[serde(default)]
    params: Value,
    points: Vec<[u32; 2]>,
}

fn turbulent_samples(request: PathBuf, out: Option<PathBuf>) -> anyhow::Result<()> {
    let raw = fs::read_to_string(&request)
        .with_context(|| format!("reading turbulent sample request {}", request.display()))?;
    let parsed: TurbulentSamplesRequest =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", request.display()))?;
    anyhow::ensure!(parsed.width > 0, "turbulent sample width must be positive");
    anyhow::ensure!(
        parsed.height > 0,
        "turbulent sample height must be positive"
    );

    let input = raster_cpu::Canvas::transparent(parsed.width, parsed.height);
    let mut frames = Vec::with_capacity(parsed.frames.len());
    for frame in parsed.frames {
        let resolved = effects::turbulent_displace::turbulent_displace_resolved_params_for_input(
            &input,
            &frame.params,
            frame.time,
        );
        let samples = effects::turbulent_displace::turbulent_displace_field_samples(
            &input,
            &frame.params,
            frame.time,
            &frame.points,
        );
        let telemetry = effects::turbulent_displace::turbulent_displace_field_telemetry(
            &input,
            &frame.params,
            frame.time,
        );
        frames.push(json!({
            "case_id": frame.case_id,
            "frame": frame.frame,
            "time": frame.time,
            "params": frame.params,
            "resolved": turbulent_resolved_params_json(resolved),
            "field_hash": format!("{:016x}", telemetry.field_hash),
            "field_hash_u64": telemetry.field_hash,
            "out_of_bounds_count": telemetry.out_of_bounds_count,
            "sampler_mode": "nearest_round",
            "sample_count": samples.len(),
            "samples": samples.into_iter().map(turbulent_sample_json).collect::<Vec<_>>()
        }));
    }

    let response = json!({
        "schema": "ae-native-renderer.turbulent-native-samples.v1",
        "request_schema": parsed.schema,
        "width": parsed.width,
        "height": parsed.height,
        "frames": frames
    });
    let text = serde_json::to_string_pretty(&response)? + "\n";
    if let Some(out) = out {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out, text)?;
        println!(
            "turbulent-samples.done frames={} out={}",
            response["frames"].as_array().map_or(0, Vec::len),
            out.display()
        );
    } else {
        print!("{text}");
    }
    Ok(())
}

fn turbulent_resolved_params_json(
    resolved: effects::turbulent_displace::TurbulentDisplaceResolvedParams,
) -> Value {
    json!({
        "displacement_type": resolved.displacement_type,
        "amount": resolved.amount,
        "size": resolved.size,
        "offset": resolved.offset,
        "complexity": resolved.complexity,
        "complexity_fraction": resolved.complexity_fraction,
        "evolution": resolved.evolution,
        "cycle_evolution": resolved.cycle_evolution,
        "cycle_revolutions": resolved.cycle_revolutions,
        "random_seed": resolved.random_seed,
        "antialiasing_best_quality": resolved.antialiasing_best_quality,
        "pinning": resolved.pinning,
        "resize_layer": resolved.resize_layer,
        "amplitude": resolved.amplitude,
        "phase_radians": resolved.phase_radians
    })
}

fn turbulent_sample_json(
    sample: effects::turbulent_displace::TurbulentDisplaceFieldSample,
) -> Value {
    let sampled_displacement = sample.sample_xy.map(|[sx, sy]| {
        [
            sx as i64 - sample.output_xy[0] as i64,
            sy as i64 - sample.output_xy[1] as i64,
        ]
    });
    json!({
        "output_xy": sample.output_xy,
        "noise": sample.noise,
        "displacement": sample.displacement,
        "source_uv": sample.source_uv,
        "sample_xy": sample.sample_xy,
        "sampled_displacement": sampled_displacement,
        "out_of_bounds": sample.out_of_bounds
    })
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

struct HypothesisPackOptions {
    module: String,
    candidate: String,
    status: HypothesisStatusArg,
    gate: HypothesisGateKindArg,
    question: Option<String>,
    hypothesis: Option<String>,
    evidence_sources: Vec<String>,
    notes: Vec<String>,
    candidate_config: Option<PathBuf>,
    pack: PathBuf,
    out: PathBuf,
    cases: Vec<String>,
    threshold_mean: Option<f64>,
    threshold_max: Option<u8>,
}

fn run_hypothesis_pack(options: HypothesisPackOptions) -> anyhow::Result<bool> {
    fs::create_dir_all(&options.out)?;
    let ok = conformance_pack::run_pack(conformance_pack::RunOptions {
        pack: options.pack,
        out: options.out.clone(),
        cases: options.cases,
        threshold_mean: options.threshold_mean,
        threshold_max: options.threshold_max,
    })?;

    let conformance_report_path = options.out.join("report.json");
    let conformance_report: Value =
        serde_json::from_str(&fs::read_to_string(&conformance_report_path)?)?;
    let candidate_config = match options.candidate_config {
        Some(path) => serde_json::from_str(
            &fs::read_to_string(&path)
                .with_context(|| format!("reading candidate config {}", path.display()))?,
        )
        .with_context(|| format!("parsing candidate config {}", path.display()))?,
        None => Value::Null,
    };

    let case_summaries = conformance_report
        .get("cases")
        .and_then(Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .map(|case| {
                    json!({
                        "case": case.get("case"),
                        "status": case.get("status"),
                        "ok": case.get("ok"),
                        "modules": case.get("modules"),
                        "summary": case.get("summary")
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let case_ids = conformance_report
        .get("cases")
        .and_then(Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .filter_map(|case| case.get("case").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut gate_artifacts = BTreeMap::new();
    gate_artifacts.insert(
        "conformance_report".to_string(),
        conformance_report_path.display().to_string(),
    );
    let gate = testkit::HypothesisGate {
        kind: hypothesis_gate_kind(options.gate),
        name: "native_conformance_pack".to_string(),
        cases: case_ids,
        ok,
        metrics: json!({
            "summary": conformance_report.get("summary"),
            "m19_alpha_composite_gate": conformance_report.get("m19_alpha_composite_gate"),
            "cases": case_summaries
        }),
        artifacts: gate_artifacts,
        notes: options.notes.clone(),
    };

    let mut artifacts = BTreeMap::new();
    artifacts.insert(
        "conformance_report".to_string(),
        conformance_report_path.display().to_string(),
    );
    let hypothesis_report_path = options.out.join("hypothesis_report.json");
    let mut report = testkit::HypothesisRunReport::new(options.module, options.candidate)
        .with_status(hypothesis_status(options.status))
        .with_candidate_config(candidate_config)?;
    report.question = options.question;
    report.hypothesis = options.hypothesis;
    report.evidence_sources = options.evidence_sources;
    report.notes = options.notes;
    report.gates.push(gate);
    report.artifacts = artifacts;
    testkit::write_hypothesis_report(&hypothesis_report_path, &report)?;
    println!(
        "hypothesis-pack.done ok={} report={}",
        ok,
        hypothesis_report_path.display()
    );
    Ok(ok)
}

fn hypothesis_status(status: HypothesisStatusArg) -> testkit::HypothesisStatus {
    match status {
        HypothesisStatusArg::CandidateReady => testkit::HypothesisStatus::CandidateReady,
        HypothesisStatusArg::Instrumented => testkit::HypothesisStatus::Instrumented,
        HypothesisStatusArg::IsolatedImproved => testkit::HypothesisStatus::IsolatedImproved,
        HypothesisStatusArg::CompositionImproved => testkit::HypothesisStatus::CompositionImproved,
        HypothesisStatusArg::StackSafe => testkit::HypothesisStatus::StackSafe,
        HypothesisStatusArg::NeedsNewProbe => testkit::HypothesisStatus::NeedsNewProbe,
        HypothesisStatusArg::Rejected => testkit::HypothesisStatus::Rejected,
        HypothesisStatusArg::ReverseImplemented => testkit::HypothesisStatus::ReverseImplemented,
        HypothesisStatusArg::ParityLocked => testkit::HypothesisStatus::ParityLocked,
    }
}

fn hypothesis_gate_kind(gate: HypothesisGateKindArg) -> testkit::HypothesisGateKind {
    match gate {
        HypothesisGateKindArg::Isolated => testkit::HypothesisGateKind::Isolated,
        HypothesisGateKindArg::Composition => testkit::HypothesisGateKind::Composition,
        HypothesisGateKindArg::Stack => testkit::HypothesisGateKind::Stack,
        HypothesisGateKindArg::Template => testkit::HypothesisGateKind::Template,
    }
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
            "output": "frame_%06d.png",
            "media": {
                "backend": source.backend_name(),
                "stats": source.stats()
            }
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
    if let Some(mp4_path) = mp4.as_ref().filter(|_| direct_mp4_enabled()) {
        match render_direct_mp4(
            &scene_path,
            &scene,
            &out,
            assets_root.clone(),
            job_archive.clone(),
            mp4_path,
        )? {
            DirectMp4Attempt::Rendered => {
                println!(
                    "render.done scene={} out={}",
                    scene_path.display(),
                    out.display()
                );
                return Ok(());
            }
            DirectMp4Attempt::Fallback { reason } => {
                eprintln!(
                    "render.warn direct MP4 path unavailable; falling back to PNG sequence path: {reason}"
                );
            }
        }
    }

    render_png_output(
        &scene_path,
        &scene,
        &out,
        assets_root,
        job_archive,
        mp4.as_deref(),
    )?;
    println!(
        "render.done scene={} out={}",
        scene_path.display(),
        out.display()
    );
    Ok(())
}

fn render_png_output(
    scene_path: &Path,
    scene: &render_ir::Scene,
    out: &Path,
    assets_root: Option<PathBuf>,
    job_archive: Option<PathBuf>,
    mp4: Option<&Path>,
) -> anyhow::Result<()> {
    let fps = scene.composition.fps;
    let strict_media = assets_root.is_some() || job_archive.is_some();
    let resolver = build_resolver(scene_path, assets_root, job_archive)?;
    let mut footage = CliFootageProvider::new(scene.assets.clone(), resolver, strict_media);
    std::fs::create_dir_all(out)?;
    let media_plan = media_plan::build_timeline_media_plan(scene)?;
    let prepare_report = footage.prepare(&media_plan)?;
    write_media_plan(out, &media_plan, prepare_report)?;
    render_core::render_png_sequence_with_footage(scene, out, &mut footage)?;
    write_media_report(out, &footage)?;
    if let Some(mp4) = mp4 {
        let frames_dir = out.join("frames");
        let mux_report = encode_png_sequence_to_mp4(&frames_dir, fps, mp4)?;
        write_mux_report(out, Some(&frames_dir), fps, mp4, "render", &mux_report)?;
        println!("render.mp4={}", mp4.display());
    }
    Ok(())
}

enum DirectMp4Attempt {
    Rendered,
    Fallback { reason: String },
}

fn render_direct_mp4(
    scene_path: &Path,
    scene: &render_ir::Scene,
    out: &Path,
    assets_root: Option<PathBuf>,
    job_archive: Option<PathBuf>,
    mp4: &Path,
) -> anyhow::Result<DirectMp4Attempt> {
    let requested = mux_backend();
    if requested == MuxBackend::Ffmpeg {
        return Ok(DirectMp4Attempt::Fallback {
            reason: "AE_RENDER_MUX_BACKEND=ffmpeg has no direct VideoSink implementation"
                .to_string(),
        });
    }

    let fps = scene.composition.fps;
    let direct_started = Instant::now();
    let mut sink = match media_gst::GstMp4VideoSink::open(
        mp4,
        scene.composition.width,
        scene.composition.height,
        fps,
    ) {
        Ok(sink) => sink,
        Err(err) if requested == MuxBackend::Auto => {
            return Ok(DirectMp4Attempt::Fallback {
                reason: format!("GStreamer MP4 sink failed to open: {err:#}"),
            });
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!("failed to open GStreamer MP4 sink for {}", mp4.display())
            });
        }
    };

    let strict_media = assets_root.is_some() || job_archive.is_some();
    let resolver = build_resolver(scene_path, assets_root, job_archive)?;
    let mut footage = CliFootageProvider::new(scene.assets.clone(), resolver, strict_media);
    std::fs::create_dir_all(out)?;
    let media_plan = media_plan::build_timeline_media_plan(scene)?;
    let prepare_report = footage.prepare(&media_plan)?;
    write_media_plan(out, &media_plan, prepare_report)?;

    render_core::render_sequence_with_footage_callback(
        scene,
        out,
        &mut footage,
        render_core::RenderSequenceOptions::video_sink(
            mp4.display().to_string(),
            "gstreamer-appsrc-mp4",
        ),
        |frame, time, canvas| {
            sink.write_frame(
                &media_gst::VideoFrame {
                    width: canvas.width,
                    height: canvas.height,
                    pts: time,
                    rgba: canvas.data.clone(),
                },
                time,
            )
            .with_context(|| format!("failed to write frame {frame} to MP4 sink"))?;
            Ok(render_core::RenderFrameOutput::default())
        },
    )?;

    let sink_manifest = sink.finish()?;
    write_media_report(out, &footage)?;
    let mux_report = MuxEncodeReport {
        requested_backend: requested.name(),
        backend: sink_manifest.backend.clone(),
        elapsed_ms: elapsed_ms(direct_started),
        sink_manifest: Some(sink_manifest),
    };
    write_mux_report(out, None, fps, mp4, "render-direct", &mux_report)?;
    println!("render.mp4={}", mp4.display());
    Ok(DirectMp4Attempt::Rendered)
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
    let mux_report = encode_png_sequence_to_mp4(&frames_dir, fps, &out)?;
    let report_dir = out
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    write_mux_report(
        &report_dir,
        Some(&frames_dir),
        fps,
        &out,
        "mux",
        &mux_report,
    )?;
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
    let mut weighted_sum_sq = 0.0_f64;
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
        let components = (metrics.total_pixels * 4) as f64;
        weighted_sum += metrics.mean_abs_diff * components;
        weighted_sum_sq += metrics.rmse_abs_diff * metrics.rmse_abs_diff * components;
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
            "rmse_abs_diff": metrics.rmse_abs_diff,
            "changed_pixels": metrics.changed_pixels,
            "total_pixels": metrics.total_pixels
        }));
    }

    let total_components = (total_pixels * 4).max(1) as f64;
    let mean_abs_diff = weighted_sum / total_components;
    let rmse_abs_diff = (weighted_sum_sq / total_components).sqrt();
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
        "reference_media": reference_video.as_ref().map(|source| json!({
            "backend": source.backend_name(),
            "path": source.path().display().to_string(),
            "stats": source.stats()
        })),
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
            "rmse_abs_diff": rmse_abs_diff,
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
    fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
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

    let native_out = options
        .out
        .clone()
        .unwrap_or_else(|| job_out.join("native"));
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

fn build_job_capability_report(input: &PreparedJobInput, strict: bool, route: RouteMode) -> Value {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaBackend {
    Auto,
    Gstreamer,
    Ffmpeg,
}

impl MediaBackend {
    fn name(self) -> &'static str {
        match self {
            MediaBackend::Auto => "auto",
            MediaBackend::Gstreamer => "gstreamer",
            MediaBackend::Ffmpeg => "ffmpeg",
        }
    }

    fn policy_name(self) -> &'static str {
        match self {
            MediaBackend::Auto => "GStreamer appsink preferred, FFmpeg fallback",
            MediaBackend::Gstreamer => "GStreamer appsink",
            MediaBackend::Ffmpeg => "persistent FFmpeg rawvideo pipe",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MuxBackend {
    Auto,
    Gstreamer,
    Ffmpeg,
}

impl MuxBackend {
    fn name(self) -> &'static str {
        match self {
            MuxBackend::Auto => "auto",
            MuxBackend::Gstreamer => "gstreamer",
            MuxBackend::Ffmpeg => "ffmpeg",
        }
    }
}

enum CliVideoSource {
    Gstreamer(media_gst::GstVideoSource),
    Ffmpeg(media_gst::FfmpegVideoSource),
}

impl CliVideoSource {
    fn open(path: &str, backend: MediaBackend) -> anyhow::Result<Self> {
        match backend {
            MediaBackend::Gstreamer => Ok(Self::Gstreamer(media_gst::GstVideoSource::open(path)?)),
            MediaBackend::Ffmpeg => Ok(Self::Ffmpeg(media_gst::FfmpegVideoSource::open(path)?)),
            MediaBackend::Auto => match media_gst::GstVideoSource::open(path) {
                Ok(source) => Ok(Self::Gstreamer(source)),
                Err(gst_err) => {
                    eprintln!(
                        "render.warn gstreamer backend failed for '{}'; falling back to ffmpeg: {gst_err:#}",
                        path
                    );
                    Ok(Self::Ffmpeg(media_gst::FfmpegVideoSource::open(path)?))
                }
            },
        }
    }

    fn backend_name(&self) -> &'static str {
        match self {
            Self::Gstreamer(source) => source.backend_name(),
            Self::Ffmpeg(source) => source.backend_name(),
        }
    }

    fn path(&self) -> &Path {
        match self {
            Self::Gstreamer(source) => source.path(),
            Self::Ffmpeg(source) => source.path(),
        }
    }

    fn decoder_is_running(&self) -> bool {
        match self {
            Self::Gstreamer(source) => source.decoder_is_running(),
            Self::Ffmpeg(source) => source.decoder_is_running(),
        }
    }

    fn park_decoder(&mut self) {
        match self {
            Self::Gstreamer(source) => source.park_decoder(),
            Self::Ffmpeg(source) => source.park_decoder(),
        }
    }
}

impl media_gst::VideoSource for CliVideoSource {
    fn info(&self) -> media_gst::VideoInfo {
        match self {
            Self::Gstreamer(source) => source.info(),
            Self::Ffmpeg(source) => source.info(),
        }
    }

    fn prepare(&mut self, plan: &media_gst::SourcePlan) -> anyhow::Result<()> {
        match self {
            Self::Gstreamer(source) => source.prepare(plan),
            Self::Ffmpeg(source) => source.prepare(plan),
        }
    }

    fn frame_at(&mut self, time: f64) -> anyhow::Result<media_gst::VideoFrame> {
        match self {
            Self::Gstreamer(source) => source.frame_at(time),
            Self::Ffmpeg(source) => source.frame_at(time),
        }
    }

    fn stats(&self) -> media_gst::VideoSourceStats {
        match self {
            Self::Gstreamer(source) => source.stats(),
            Self::Ffmpeg(source) => source.stats(),
        }
    }
}

struct CliFootageProvider {
    assets: HashMap<String, render_ir::Asset>,
    resolver: media_gst::JobAssetResolver,
    sources: HashMap<String, CliVideoSource>,
    source_paths: HashMap<String, String>,
    decoder_lru: VecDeque<String>,
    max_open_decoders: usize,
    backend: MediaBackend,
    missing_sources: HashSet<String>,
    strict_media: bool,
    prepare_report: Option<Value>,
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
            source_paths: HashMap::new(),
            decoder_lru: VecDeque::new(),
            max_open_decoders: max_open_decoders(),
            backend: media_backend(),
            missing_sources: HashSet::new(),
            strict_media,
            prepare_report: None,
        }
    }

    fn prepare(&mut self, plan: &media_gst::TimelineMediaPlan) -> anyhow::Result<Value> {
        let prepare_started = Instant::now();
        let prewarm_frames = media_prewarm_frames();
        let mut prepared_sources = Vec::new();
        let mut skipped_sources = Vec::new();
        let mut missing_sources = Vec::new();

        for source_plan in &plan.sources {
            let first_render_frame = source_plan.first_render_frame;
            let should_prewarm = prewarm_frames > 0
                && first_render_frame
                    .map(|frame| frame < prewarm_frames)
                    .unwrap_or(false);

            if !should_prewarm {
                skipped_sources.push(json!({
                    "asset_id": source_plan.asset_id.clone(),
                    "reason": if prewarm_frames == 0 {
                        "prewarm_disabled"
                    } else {
                        "outside_prewarm_window"
                    },
                    "first_render_frame": first_render_frame
                }));
                continue;
            }

            let source_started = Instant::now();
            if !self.open_source(&source_plan.asset_id)? {
                missing_sources.push(json!({
                    "asset_id": source_plan.asset_id.clone(),
                    "first_render_frame": first_render_frame,
                    "elapsed_ms": elapsed_ms(source_started)
                }));
                continue;
            }

            self.sources
                .get_mut(&source_plan.asset_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("video source '{}' was not opened", source_plan.asset_id)
                })?
                .prepare(source_plan)?;
            self.mark_decoder_used(&source_plan.asset_id);
            prepared_sources.push(json!({
                "asset_id": source_plan.asset_id.clone(),
                "first_render_frame": first_render_frame,
                "first_source_time": source_plan.first_source_time,
                "last_source_time": source_plan.last_source_time,
                "requests": source_plan.requests.len(),
                "elapsed_ms": elapsed_ms(source_started)
            }));
        }

        let report = json!({
            "policy": "open and start decoder for planned sources needed in the first render frames",
            "elapsed_ms": elapsed_ms(prepare_started),
            "prewarm_frames": prewarm_frames,
            "planned_sources": plan.sources.len(),
            "planned_requests": plan.total_requests,
            "prepared_sources": prepared_sources,
            "skipped_sources": skipped_sources,
            "missing_sources": missing_sources
        });
        self.prepare_report = Some(report.clone());
        Ok(report)
    }

    fn media_report(&self) -> Value {
        let mut source_ids = self.sources.keys().cloned().collect::<Vec<_>>();
        source_ids.sort();

        let mut totals = media_gst::VideoSourceStats::default();
        let sources = source_ids
            .into_iter()
            .filter_map(|source_id| {
                let source = self.sources.get(&source_id)?;
                let stats = source.stats();
                totals.requests += stats.requests;
                totals.cache_hits += stats.cache_hits;
                totals.cache_misses += stats.cache_misses;
                totals.frames_decoded += stats.frames_decoded;
                totals.decoder_spawns += stats.decoder_spawns;
                totals.decoder_restarts += stats.decoder_restarts;
                totals.decoder_parks += stats.decoder_parks;
                totals.sequential_frames_skipped += stats.sequential_frames_skipped;
                totals.max_cache_entries = totals.max_cache_entries.max(stats.max_cache_entries);
                totals.max_sequential_decode_gap = totals
                    .max_sequential_decode_gap
                    .max(stats.max_sequential_decode_gap);
                totals.request_ms += stats.request_ms;
                totals.cache_hit_ms += stats.cache_hit_ms;
                totals.cache_miss_ms += stats.cache_miss_ms;
                totals.decoder_spawn_ms += stats.decoder_spawn_ms;
                totals.frame_read_ms += stats.frame_read_ms;
                totals.max_request_ms = totals.max_request_ms.max(stats.max_request_ms);
                totals.max_frame_read_ms = totals.max_frame_read_ms.max(stats.max_frame_read_ms);
                Some(json!({
                    "asset_id": source_id.clone(),
                    "backend": source.backend_name(),
                    "path": self.source_paths
                        .get(&source_id)
                        .cloned()
                        .unwrap_or_else(|| source.path().display().to_string()),
                    "stats": stats,
                    "derived": media_stats_derived(&stats)
                }))
            })
            .collect::<Vec<_>>();

        let hit_rate = if totals.requests == 0 {
            0.0
        } else {
            totals.cache_hits as f64 / totals.requests as f64
        };
        let mut missing_sources = self.missing_sources.iter().cloned().collect::<Vec<_>>();
        missing_sources.sort();

        json!({
            "backend_policy": format!(
                "{} + per-source LRU frame cache",
                self.backend.policy_name()
            ),
            "requested_backend": self.backend.name(),
            "opened_sources": sources.len(),
            "max_open_decoders": self.max_open_decoders,
            "missing_sources": missing_sources,
            "prepare": self.prepare_report.clone(),
            "cache_hit_rate": hit_rate,
            "totals": totals,
            "derived": media_stats_derived(&totals),
            "sources": sources
        })
    }

    fn mark_decoder_used(&mut self, source: &str) {
        self.decoder_lru.retain(|existing| existing != source);
        self.decoder_lru.push_back(source.to_string());

        while self.running_decoder_count() > self.max_open_decoders {
            let Some(candidate) = self.decoder_lru.pop_front() else {
                break;
            };
            if candidate == source {
                self.decoder_lru.push_back(candidate);
                break;
            }
            if let Some(decoder) = self.sources.get_mut(&candidate) {
                if decoder.decoder_is_running() {
                    decoder.park_decoder();
                }
            }
        }
    }

    fn running_decoder_count(&self) -> usize {
        self.sources
            .values()
            .filter(|source| source.decoder_is_running())
            .count()
    }

    fn open_source(&mut self, source: &str) -> anyhow::Result<bool> {
        if self.missing_sources.contains(source) {
            return Ok(false);
        }
        if self.sources.contains_key(source) {
            return Ok(true);
        }

        let asset = self.assets.get(source).cloned().ok_or_else(|| {
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
            return Ok(false);
        };

        let resolved_path = path.clone();
        self.sources.insert(
            source.to_string(),
            CliVideoSource::open(&resolved_path, self.backend)?,
        );
        self.source_paths.insert(source.to_string(), resolved_path);
        Ok(true)
    }
}

impl render_core::FootageProvider for CliFootageProvider {
    fn frame_at(&mut self, source: &str, time: f64) -> anyhow::Result<Option<raster_cpu::Canvas>> {
        if !self.open_source(source)? {
            return Ok(None);
        }

        let decoded = self
            .sources
            .get_mut(source)
            .ok_or_else(|| anyhow::anyhow!("video source '{source}' was not opened"))?
            .frame_at(time)?;
        self.mark_decoder_used(source);
        Ok(Some(raster_cpu::Canvas::from_rgba(
            decoded.width,
            decoded.height,
            decoded.rgba,
        )?))
    }
}

fn max_open_decoders() -> usize {
    std::env::var("AE_RENDER_MAX_OPEN_DECODERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(6)
}

fn media_backend() -> MediaBackend {
    match std::env::var("AE_RENDER_MEDIA_BACKEND")
        .unwrap_or_else(|_| "auto".to_string())
        .to_lowercase()
        .as_str()
    {
        "gstreamer" | "gst" | "appsink" => MediaBackend::Gstreamer,
        "ffmpeg" | "ffmpeg-pipe" => MediaBackend::Ffmpeg,
        _ => MediaBackend::Auto,
    }
}

fn mux_backend() -> MuxBackend {
    match std::env::var("AE_RENDER_MUX_BACKEND")
        .unwrap_or_else(|_| "auto".to_string())
        .to_lowercase()
        .as_str()
    {
        "gstreamer" | "gst" | "appsrc" => MuxBackend::Gstreamer,
        "ffmpeg" | "ffmpeg-cli" => MuxBackend::Ffmpeg,
        _ => MuxBackend::Auto,
    }
}

fn direct_mp4_enabled() -> bool {
    let output_mode = std::env::var("AE_RENDER_OUTPUT_MODE")
        .unwrap_or_default()
        .to_lowercase();
    if matches!(
        output_mode.as_str(),
        "direct_mp4" | "direct-mp4" | "video_sink" | "videosink"
    ) {
        return true;
    }

    let direct = std::env::var("AE_RENDER_DIRECT_MP4")
        .unwrap_or_default()
        .to_lowercase();
    matches!(direct.as_str(), "1" | "true" | "yes" | "on")
}

fn media_prewarm_frames() -> u32 {
    std::env::var("AE_RENDER_PREWARM_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
}

fn write_media_plan(
    out: &Path,
    plan: &media_gst::TimelineMediaPlan,
    prepare_report: Value,
) -> anyhow::Result<()> {
    let path = out.join("media-plan.json");
    fs::write(
        &path,
        serde_json::to_string_pretty(&json!({
            "plan": plan,
            "prepare": prepare_report
        }))?,
    )?;
    println!("render.media_plan={}", path.display());
    Ok(())
}

fn write_media_report(out: &Path, footage: &CliFootageProvider) -> anyhow::Result<()> {
    let path = out.join("media-report.json");
    fs::write(
        &path,
        serde_json::to_string_pretty(&footage.media_report())?,
    )?;
    println!("render.media_report={}", path.display());
    Ok(())
}

struct MuxEncodeReport {
    requested_backend: &'static str,
    backend: String,
    elapsed_ms: f64,
    sink_manifest: Option<media_gst::VideoSinkManifest>,
}

fn encode_png_sequence_to_mp4(
    frames_dir: &Path,
    fps: f64,
    out: &Path,
) -> anyhow::Result<MuxEncodeReport> {
    let requested = mux_backend();
    match requested {
        MuxBackend::Gstreamer => encode_png_sequence_gstreamer(frames_dir, fps, out, requested),
        MuxBackend::Ffmpeg => encode_png_sequence_ffmpeg(frames_dir, fps, out, requested),
        MuxBackend::Auto => match encode_png_sequence_gstreamer(frames_dir, fps, out, requested) {
            Ok(report) => Ok(report),
            Err(gst_err) => {
                eprintln!(
                    "mux.warn gstreamer sink failed for '{}'; falling back to ffmpeg: {gst_err:#}",
                    out.display()
                );
                encode_png_sequence_ffmpeg(frames_dir, fps, out, requested)
            }
        },
    }
}

fn encode_png_sequence_ffmpeg(
    frames_dir: &Path,
    fps: f64,
    out: &Path,
    requested: MuxBackend,
) -> anyhow::Result<MuxEncodeReport> {
    let started = Instant::now();
    render_core::mux_png_sequence_to_mp4(frames_dir, fps, out)?;
    Ok(MuxEncodeReport {
        requested_backend: requested.name(),
        backend: "ffmpeg-cli".to_string(),
        elapsed_ms: elapsed_ms(started),
        sink_manifest: None,
    })
}

fn encode_png_sequence_gstreamer(
    frames_dir: &Path,
    fps: f64,
    out: &Path,
    requested: MuxBackend,
) -> anyhow::Result<MuxEncodeReport> {
    let started = Instant::now();
    let frame_count = count_native_frames(frames_dir)?;
    anyhow::ensure!(
        frame_count > 0,
        "no PNG frames found in {}",
        frames_dir.display()
    );
    let first = testkit::load_rgba_png(frames_dir.join("frame_000000.png"))?;
    let mut sink = media_gst::GstMp4VideoSink::open(out, first.width, first.height, fps)?;

    for frame in 0..frame_count {
        let path = frames_dir.join(format!("frame_{frame:06}.png"));
        let image = if frame == 0 {
            first.clone()
        } else {
            testkit::load_rgba_png(&path)?
        };
        anyhow::ensure!(
            image.width == first.width && image.height == first.height,
            "frame {} dimensions differ: expected {}x{}, got {}x{}",
            path.display(),
            first.width,
            first.height,
            image.width,
            image.height
        );
        sink.write_frame(
            &media_gst::VideoFrame {
                width: image.width,
                height: image.height,
                pts: frame as f64 / fps,
                rgba: image.data,
            },
            frame as f64 / fps,
        )?;
    }

    let manifest = sink.finish()?;
    Ok(MuxEncodeReport {
        requested_backend: requested.name(),
        backend: manifest.backend.clone(),
        elapsed_ms: elapsed_ms(started),
        sink_manifest: Some(manifest),
    })
}

fn write_mux_report(
    report_dir: &Path,
    frames_dir: Option<&Path>,
    fps: f64,
    out: &Path,
    mode: &str,
    report: &MuxEncodeReport,
) -> anyhow::Result<()> {
    fs::create_dir_all(report_dir)?;
    let path = report_dir.join("mux-report.json");
    let frames = frames_dir.map(|path| path.display().to_string());
    let input = match &frames {
        Some(frames) => json!({
            "kind": "png_sequence",
            "frames": frames
        }),
        None => json!({
            "kind": "render_core_callback"
        }),
    };
    fs::write(
        &path,
        serde_json::to_string_pretty(&json!({
            "mode": mode,
            "requested_backend": report.requested_backend,
            "backend": report.backend,
            "frames": frames,
            "input": input,
            "fps": fps,
            "out": out.display().to_string(),
            "elapsed_ms": report.elapsed_ms,
            "sink_manifest": report.sink_manifest
        }))?,
    )?;
    println!("mux.report={}", path.display());
    Ok(())
}

fn media_stats_derived(stats: &media_gst::VideoSourceStats) -> Value {
    json!({
        "avg_request_ms": ratio(stats.request_ms, stats.requests),
        "avg_cache_hit_ms": ratio(stats.cache_hit_ms, stats.cache_hits),
        "avg_cache_miss_ms": ratio(stats.cache_miss_ms, stats.cache_misses),
        "avg_decoder_spawn_ms": ratio(stats.decoder_spawn_ms, stats.decoder_spawns),
        "avg_frame_read_ms": ratio(stats.frame_read_ms, stats.frames_decoded),
        "decoded_frames_per_request": ratio(stats.frames_decoded as f64, stats.requests),
        "skipped_frames_per_request": ratio(stats.sequential_frames_skipped as f64, stats.requests)
    })
}

fn ratio(total: f64, count: u64) -> f64 {
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}
