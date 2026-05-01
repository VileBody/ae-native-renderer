use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    Probe { path: PathBuf },
    Validate { #[arg(long)] scene: PathBuf },
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
    Render { #[arg(long)] scene: PathBuf, #[arg(long)] out: PathBuf },
    Job { #[arg(long)] job_dir: PathBuf },
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
        Command::ImportPayload { payload, out, diagnostics } => import_payload(payload, out, diagnostics),
        Command::Render { scene, out } => render(scene, out),
        Command::Job { job_dir } => {
            let scene = job_dir.join("scene.json");
            let out = job_dir.join("out");
            render(scene, out)
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
    println!("effects.known={}", effects::EffectRegistry::known_match_names().len());
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

fn import_payload(payload_path: PathBuf, out: PathBuf, diagnostics: Option<PathBuf>) -> anyhow::Result<()> {
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

    println!("import.done payload={} out={}", payload_path.display(), out.display());
    Ok(())
}

fn render(scene_path: PathBuf, out: PathBuf) -> anyhow::Result<()> {
    let scene = render_ir::load_scene(&scene_path)?;
    std::fs::create_dir_all(&out)?;
    render_core::render_png_sequence(&scene, &out)?;
    println!("render.done scene={} out={}", scene_path.display(), out.display());
    Ok(())
}
