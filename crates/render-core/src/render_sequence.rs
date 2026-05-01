use crate::layer_eval::render_frame;
use render_ir::Scene;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn render_png_sequence(scene: &Scene, out_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let out_dir = out_dir.as_ref();
    let frames_dir = out_dir.join("frames");
    fs::create_dir_all(&frames_dir)?;

    let frame_count = (scene.composition.duration * scene.composition.fps).ceil() as u32;
    for frame in 0..frame_count {
        let canvas = render_frame(scene, frame)?;
        let path = frames_dir.join(format!("frame_{frame:06}.png"));
        canvas.save_png(path)?;
    }

    let manifest = json!({
        "width": scene.composition.width,
        "height": scene.composition.height,
        "fps": scene.composition.fps,
        "duration": scene.composition.duration,
        "frames": frame_count,
        "output": "frames/frame_%06d.png"
    });
    fs::write(out_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}
