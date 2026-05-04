#[cfg(test)]
mod tests {
    use crate::{builtin_passport, load_rgba_png, Dimension, Discontinuity, OperatorKind};
    use anyhow::{Context, Result};
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    #[derive(Debug, Clone, Copy)]
    struct Phase3Case {
        id: &'static str,
        ladder: &'static str,
        modules: &'static [&'static str],
        selected_frames: &'static [u64],
    }

    const PHASE3_CASES: &[Phase3Case] = &[
        Phase3Case {
            id: "EFF_010",
            ladder: "operator_static",
            modules: &["M10"],
            selected_frames: &[0],
        },
        Phase3Case {
            id: "EFF_020",
            ladder: "operator_static",
            modules: &["M11"],
            selected_frames: &[0],
        },
        Phase3Case {
            id: "EFF_030",
            ladder: "operator_static",
            modules: &["M10"],
            selected_frames: &[0],
        },
        Phase3Case {
            id: "EFF_050",
            ladder: "operator_static",
            modules: &["M13"],
            selected_frames: &[0],
        },
        Phase3Case {
            id: "STK_010",
            ladder: "stack",
            modules: &["M10", "M19"],
            selected_frames: &[0],
        },
        Phase3Case {
            id: "STK_020",
            ladder: "stack",
            modules: &["M10", "M13", "M19"],
            selected_frames: &[0],
        },
    ];

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("testkit crate should live under crates/testkit")
            .to_path_buf()
    }

    fn conformance_manifest() -> Result<Value> {
        let path = repo_root().join("fixtures/ae_conformance_pack/manifest.json");
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&data).with_context(|| format!("parsing {}", path.display()))
    }

    fn case_by_id<'a>(manifest: &'a Value, id: &str) -> Result<&'a Value> {
        manifest["cases"]
            .as_array()
            .and_then(|cases| cases.iter().find(|case| case["id"].as_str() == Some(id)))
            .with_context(|| format!("case {id} must be present in AE conformance manifest"))
    }

    #[test]
    fn phase3_manifest_selects_static_and_stack_frame_zero() -> Result<()> {
        let manifest = conformance_manifest()?;

        for expected in PHASE3_CASES {
            let case = case_by_id(&manifest, expected.id)?;
            assert_eq!(
                case["ladder"].as_str(),
                Some(expected.ladder),
                "{}",
                expected.id
            );

            let frames = case["frames_to_compare"]
                .as_array()
                .with_context(|| format!("{} frames_to_compare must be an array", expected.id))?;
            let frames = frames
                .iter()
                .map(|frame| frame.as_u64().unwrap_or(u64::MAX))
                .collect::<Vec<_>>();
            assert_eq!(
                frames.as_slice(),
                expected.selected_frames,
                "{}",
                expected.id
            );

            let modules = case["modules"]
                .as_array()
                .with_context(|| format!("{} modules must be an array", expected.id))?;
            let modules = modules
                .iter()
                .map(|module| module.as_str().unwrap_or(""))
                .collect::<Vec<_>>();
            assert_eq!(modules.as_slice(), expected.modules, "{}", expected.id);
        }

        Ok(())
    }

    #[test]
    fn phase3_selected_ae_goldens_exist_and_are_512_rgba() -> Result<()> {
        let root = repo_root();

        for case in PHASE3_CASES {
            for frame in case.selected_frames {
                let path = root.join(format!(
                    "fixtures/ae_conformance_pack/ae_goldens/png/{}/{}_{:05}.png",
                    case.id, case.id, frame
                ));
                let image = load_rgba_png(&path)
                    .with_context(|| format!("loading selected AE golden {}", path.display()))?;
                assert_eq!(
                    (image.width, image.height),
                    (512, 512),
                    "{}",
                    path.display()
                );
                assert_eq!(
                    image.data.len(),
                    (512 * 512 * 4) as usize,
                    "{}",
                    path.display()
                );
            }
        }

        Ok(())
    }

    #[test]
    fn phase3_effect_passports_decompose_required_operator_risks() {
        let box_blur = builtin_passport("ADBE Box Blur2").expect("Box Blur passport");
        assert_eq!(box_blur.kind, OperatorKind::SpatialNeighborhood);
        assert!(box_blur.has_dimension(Dimension::Spatial));
        assert!(box_blur.has_dimension(Dimension::AnimatedParams));
        assert!(box_blur.has_discontinuity(Discontinuity::IntegerRadius));
        assert!(box_blur.has_discontinuity(Discontinuity::EdgeSampling));

        let drop_shadow = builtin_passport("ADBE Drop Shadow").expect("Drop Shadow passport");
        assert_eq!(drop_shadow.kind, OperatorKind::SpatialNeighborhood);
        assert!(drop_shadow.has_dimension(Dimension::Coordinate));
        assert!(drop_shadow.has_dimension(Dimension::Alpha));
        assert!(drop_shadow.has_discontinuity(Discontinuity::PremultAlphaBoundary));
        assert!(drop_shadow
            .debug_outputs
            .iter()
            .any(|name| name == "shadow_mask"));

        let glow = builtin_passport("ADBE Glo2").expect("Glow passport");
        assert_eq!(glow.kind, OperatorKind::SpatialNeighborhood);
        assert!(glow.has_dimension(Dimension::Pointwise));
        assert!(glow.has_discontinuity(Discontinuity::Threshold));
        assert!(glow
            .debug_outputs
            .iter()
            .any(|name| name == "threshold_mask"));

        let minimax = builtin_passport("ADBE Minimax").expect("Minimax passport");
        assert_eq!(minimax.kind, OperatorKind::SpatialNeighborhood);
        assert!(minimax.has_dimension(Dimension::Spatial));
        assert!(minimax.has_dimension(Dimension::Alpha));
        assert!(minimax.has_discontinuity(Discontinuity::IntegerRadius));
        assert!(minimax
            .debug_outputs
            .iter()
            .any(|name| name == "channel_minmax"));
    }

    #[test]
    fn phase3_stack_cases_cover_non_commutative_ordering() -> Result<()> {
        let manifest = conformance_manifest()?;

        let shadow_stack = case_by_id(&manifest, "STK_010")?;
        assert_eq!(shadow_stack["title"].as_str(), Some("Drop Shadow x2 stack"));
        assert_eq!(shadow_stack["ladder"].as_str(), Some("stack"));

        let blur_minimax_stack = case_by_id(&manifest, "STK_020")?;
        assert_eq!(
            blur_minimax_stack["title"].as_str(),
            Some("Blur then Minimax non-commutative stack")
        );
        assert_eq!(blur_minimax_stack["ladder"].as_str(), Some("stack"));
        let assets = blur_minimax_stack["assets"]
            .as_array()
            .expect("STK_020 assets");
        assert!(assets
            .iter()
            .any(|asset| asset.as_str() == Some("hard_edge")));

        Ok(())
    }
}
