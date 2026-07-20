use crate::temporal::{posterize_time, Fps, PosterizeMode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberedFrameExpectation {
    pub comp_frame: u64,
    pub comp_time_seconds: f64,
    pub layer_time_seconds: f64,
    pub source_time_seconds: f64,
    pub posterize_bucket: u64,
    pub source_frame: u64,
}

pub fn numbered_frame_expectation(
    comp_frame: u64,
    comp_fps: Fps,
    layer_start_seconds: f64,
    source_start_seconds: f64,
    source_fps: Fps,
    posterize_fps: Fps,
) -> NumberedFrameExpectation {
    let comp_time_seconds = comp_fps.frame_time(comp_frame);
    let layer_time_seconds = posterize_time(comp_time_seconds, posterize_fps, PosterizeMode::Floor);
    let source_time_seconds =
        (source_start_seconds + layer_time_seconds - layer_start_seconds).max(0.0);

    NumberedFrameExpectation {
        comp_frame,
        comp_time_seconds,
        layer_time_seconds,
        source_time_seconds,
        posterize_bucket: posterize_fps.floor_frame_index(comp_time_seconds),
        source_frame: source_fps.floor_frame_index(source_time_seconds),
    }
}

pub fn numbered_frame_expectations(
    comp_frames: impl IntoIterator<Item = u64>,
    comp_fps: Fps,
    layer_start_seconds: f64,
    source_start_seconds: f64,
    source_fps: Fps,
    posterize_fps: Fps,
) -> Vec<NumberedFrameExpectation> {
    comp_frames
        .into_iter()
        .map(|comp_frame| {
            numbered_frame_expectation(
                comp_frame,
                comp_fps,
                layer_start_seconds,
                source_start_seconds,
                source_fps,
                posterize_fps,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, bail, ensure};
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    #[test]
    fn posterize_time_10fps_buckets_hold_three_30fps_frames() {
        let expectations =
            numbered_frame_expectations(0..12, Fps(30.0), 0.0, 0.0, Fps(30.0), Fps(10.0));

        let buckets: Vec<u64> = expectations
            .iter()
            .map(|expectation| expectation.posterize_bucket)
            .collect();
        let source_frames: Vec<u64> = expectations
            .iter()
            .map(|expectation| expectation.source_frame)
            .collect();

        assert_eq!(buckets, vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3]);
        assert_eq!(source_frames, vec![0, 0, 0, 3, 3, 3, 6, 6, 6, 9, 9, 9]);
    }

    #[test]
    fn numbered_frame_source_time_respects_layer_and_source_offsets() {
        let expectation =
            numbered_frame_expectation(15, Fps(30.0), 0.25, 2.0, Fps(30.0), Fps(10.0));

        assert_close(expectation.comp_time_seconds, 0.5);
        assert_close(expectation.layer_time_seconds, 0.5);
        assert_close(expectation.source_time_seconds, 2.25);
        assert_eq!(expectation.posterize_bucket, 5);
        assert_eq!(expectation.source_frame, 67);
    }

    #[test]
    fn phase2_ae_goldens_exist_and_decode_for_manifest_frames() -> anyhow::Result<()> {
        let root = repo_root();
        let manifest_path = root.join("fixtures/ae_conformance_pack/manifest.json");
        let manifest: Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
        let unavailable = unavailable_phase2_goldens(&root, &manifest)?;
        if !unavailable.is_empty() {
            if !crate::ae_png_goldens_required() {
                eprintln!(
                    "skipping phase2 AE PNG golden load test: {} assets unavailable; set {}=1 to require goldens",
                    unavailable.len(),
                    crate::REQUIRE_AE_PNG_GOLDENS_ENV
                );
                return Ok(());
            }
            bail!("phase2 AE PNG golden assets unavailable: {unavailable:?}");
        }

        for case_id in ["TMP_010", "TMP_020", "STK_030"] {
            let case = manifest_case(&manifest, case_id)?;
            let frames = case
                .get("frames_to_compare")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("{case_id} has no frames_to_compare"))?;
            ensure!(!frames.is_empty(), "{case_id} has no compare frames");

            let png_dir = root.join(format!(
                "fixtures/ae_conformance_pack/ae_goldens/png/{case_id}"
            ));
            ensure!(png_dir.is_dir(), "{} is missing", png_dir.display());
            ensure!(
                count_pngs(&png_dir)? == 60,
                "{case_id} should have a complete 60-frame clean PNG sequence"
            );

            for frame in frames {
                let frame = frame
                    .as_u64()
                    .ok_or_else(|| anyhow!("{case_id} frame index is not an integer"))?;
                let png = png_dir.join(format!("{case_id}_{frame:05}.png"));
                ensure!(png.is_file(), "{} is missing", png.display());

                let image = image::open(&png)?.to_rgba8();
                ensure!(
                    image.width() == 512 && image.height() == 512,
                    "{} has unexpected dimensions {}x{}",
                    png.display(),
                    image.width(),
                    image.height()
                );
            }
        }

        Ok(())
    }

    fn unavailable_phase2_goldens(root: &Path, manifest: &Value) -> anyhow::Result<Vec<PathBuf>> {
        let mut unavailable = Vec::new();
        for case_id in ["TMP_010", "TMP_020", "STK_030"] {
            let case = manifest_case(manifest, case_id)?;
            let frames = case
                .get("frames_to_compare")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("{case_id} has no frames_to_compare"))?;
            let png_dir = root.join(format!(
                "fixtures/ae_conformance_pack/ae_goldens/png/{case_id}"
            ));
            if !png_dir.is_dir() || count_pngs(&png_dir)? != 60 {
                unavailable.push(png_dir);
                continue;
            }
            for frame in frames {
                let frame = frame
                    .as_u64()
                    .ok_or_else(|| anyhow!("{case_id} frame index is not an integer"))?;
                let png = png_dir.join(format!("{case_id}_{frame:05}.png"));
                if !png.is_file() {
                    unavailable.push(png);
                }
            }
        }
        Ok(unavailable)
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("testkit crate should live two levels below repo root")
            .to_path_buf()
    }

    fn manifest_case<'a>(manifest: &'a Value, case_id: &str) -> anyhow::Result<&'a Value> {
        manifest
            .get("cases")
            .and_then(Value::as_array)
            .and_then(|cases| {
                cases
                    .iter()
                    .find(|case| case.get("id").and_then(Value::as_str) == Some(case_id))
            })
            .ok_or_else(|| anyhow!("{case_id} is missing from conformance manifest"))
    }

    fn count_pngs(dir: &Path) -> anyhow::Result<usize> {
        Ok(std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("png"))
            .count())
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-12,
            "expected {expected}, got {actual}"
        );
    }
}
