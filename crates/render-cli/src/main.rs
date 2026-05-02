use clap::{Parser, Subcommand};
use media_gst::VideoSource;
use std::collections::{HashMap, HashSet};
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
    Job {
        #[arg(long)]
        job_dir: PathBuf,
    },
    ListEffects,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor => doctor(),
        Command::Probe { path } => probe(path),
        Command::Validate { scene } => validate(scene),
        Command::ValidatePayload { payload, strict } => validate_payload(payload, strict),
        Command::ImportPayload {
            payload,
            out,
            diagnostics,
        } => import_payload(payload, out, diagnostics),
        Command::ResolveAssets {
            scene,
            assets_root,
            job_archive,
            strict,
        } => resolve_assets(scene, assets_root, job_archive, strict),
        Command::DumpFrames {
            input,
            out,
            count,
            start,
            step,
        } => dump_frames(input, out, count, start, step),
        Command::Render {
            scene,
            out,
            assets_root,
            job_archive,
            mp4,
        } => render(scene, out, assets_root, job_archive, mp4),
        Command::Mux { frames, out, fps } => mux(frames, out, fps),
        Command::Job { job_dir } => {
            let scene = job_dir.join("scene.json");
            let out = job_dir.join("out");
            render(scene, out, Some(job_dir), None, None)
        }
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

fn read_manifest_fps(path: &Path) -> Option<f64> {
    let manifest_path = path.join("manifest.json");
    let raw = std::fs::read_to_string(manifest_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value.get("fps").and_then(serde_json::Value::as_f64)
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
