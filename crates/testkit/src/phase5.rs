#[cfg(test)]
mod tests {
    use crate::load_rgba_png;
    use anyhow::{Context, Result};
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    #[derive(Debug, Clone, Copy)]
    struct Phase5Case {
        id: &'static str,
        ladder: &'static str,
        modules: &'static [&'static str],
        selected_frames: &'static [u64],
    }

    const PHASE5_CASES: &[Phase5Case] = &[
        Phase5Case {
            id: "TXT_010",
            ladder: "animated_operator",
            modules: &["M05", "M06"],
            selected_frames: &[0, 8, 16, 24, 32, 45, 59],
        },
        Phase5Case {
            id: "TXT_020",
            ladder: "animated_operator",
            modules: &["M05", "M06"],
            selected_frames: &[0, 8, 16, 24, 32, 45, 59],
        },
        Phase5Case {
            id: "TXT_030",
            ladder: "animated_operator",
            modules: &["M05", "M07"],
            selected_frames: &[0, 8, 16, 24, 32, 45, 59],
        },
        Phase5Case {
            id: "TXT_040",
            ladder: "animated_operator",
            modules: &["M08", "M09"],
            selected_frames: &[0, 5, 10, 15, 20, 30, 45, 59],
        },
        Phase5Case {
            id: "EXP_010",
            ladder: "animated_operator",
            modules: &["M09"],
            selected_frames: &[0, 5, 10, 15, 20, 30, 45, 59],
        },
        Phase5Case {
            id: "GPH_010",
            ladder: "template_slice",
            modules: &["M17", "M05", "M19"],
            selected_frames: &[0, 15, 30, 45],
        },
    ];

    const PHASE5_TELEMETRY_CHECKPOINTS: &[(&str, &[&str])] = &[
        ("M05", &["font_resolution", "glyph_layout"]),
        ("M06", &["selector_units", "selector_weights"]),
        ("M07", &["glyph_transform", "glyph_opacity", "glyph_blur"]),
        ("M08", &["expression_selector_amount"]),
        (
            "M09",
            &["property_expression_value", "edge_wobble_position"],
        ),
        (
            "M17",
            &[
                "collapse_mode",
                "deferred_text_raster_scale",
                "sharpness_probe",
            ],
        ),
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
    fn phase5_manifest_declares_text_expression_and_collapse_cases() -> Result<()> {
        let manifest = conformance_manifest()?;

        for expected in PHASE5_CASES {
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
            assert_eq!(frames, expected.selected_frames, "{}", expected.id);

            let modules = case["modules"]
                .as_array()
                .with_context(|| format!("{} modules must be an array", expected.id))?;
            let modules = modules
                .iter()
                .map(|module| module.as_str().unwrap_or(""))
                .collect::<Vec<_>>();
            assert_eq!(modules, expected.modules, "{}", expected.id);
        }

        Ok(())
    }

    #[test]
    fn phase5_selected_ae_goldens_exist_and_are_512_rgba() -> Result<()> {
        let root = repo_root();

        for case in PHASE5_CASES {
            for frame in case.selected_frames {
                let path = root.join(format!(
                    "fixtures/ae_conformance_pack/ae_goldens/png/{}/{:}_{:05}.png",
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
    fn phase5_font_assumptions_match_ae_manifest() -> Result<()> {
        let manifest = conformance_manifest()?;
        let fonts = manifest["required_fonts"]
            .as_array()
            .context("required_fonts must be an array")?;

        let montserrat = fonts
            .iter()
            .find(|font| font["family_or_postscript"].as_str() == Some("Montserrat-BoldItalic"))
            .context("Montserrat-BoldItalic required font")?;
        let point = fonts
            .iter()
            .find(|font| font["family_or_postscript"].as_str() == Some("Point-Light"))
            .context("Point-Light required font")?;

        assert_eq!(used_by(montserrat), vec!["TXT_010", "TXT_020"]);
        assert_eq!(used_by(point), vec!["TXT_030", "TXT_040"]);

        let root = repo_root();
        assert!(
            root.join("fixtures/ae_conformance_pack/assets/fonts/Montserrat-BoldItalic.ttf")
                .exists(),
            "Montserrat-BoldItalic.ttf must stay checked into the conformance pack for exact native text layout"
        );
        assert!(
            root.join("fixtures/ae_conformance_pack/assets/fonts/Montserrat-Italic[wght].ttf")
                .exists(),
            "Montserrat variable italic TTF must stay checked into the conformance pack as fallback/source asset"
        );
        let point_font_path =
            root.join("fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf");
        let point_archive_path = root.join("youworkforthem-T9068-point.zip");
        assert!(
            point_font_path.exists() || point_archive_path.exists(),
            "Point-Light must be available either as assets/fonts/Point-Light.ttf or as the local root archive"
        );

        let font_files = manifest["assets"]["font_files"]
            .as_array()
            .context("assets.font_files must be an array")?;
        assert!(
            font_files.iter().any(|path| path
                .as_str()
                .is_some_and(|path| path.contains("Point-Light"))),
            "manifest should document the local Point-Light native font asset when it is used"
        );

        Ok(())
    }

    #[test]
    fn phase5_telemetry_plan_covers_first_divergence_points() {
        let checkpoints = PHASE5_TELEMETRY_CHECKPOINTS;

        assert!(has_checkpoint(checkpoints, "M05", "font_resolution"));
        assert!(has_checkpoint(checkpoints, "M05", "glyph_layout"));
        assert!(has_checkpoint(checkpoints, "M06", "selector_weights"));
        assert!(has_checkpoint(checkpoints, "M07", "glyph_transform"));
        assert!(has_checkpoint(checkpoints, "M07", "glyph_blur"));
        assert!(has_checkpoint(
            checkpoints,
            "M08",
            "expression_selector_amount"
        ));
        assert!(has_checkpoint(checkpoints, "M09", "edge_wobble_position"));
        assert!(has_checkpoint(
            checkpoints,
            "M17",
            "deferred_text_raster_scale"
        ));
        assert!(has_checkpoint(checkpoints, "M17", "sharpness_probe"));
    }

    fn used_by(font: &Value) -> Vec<&str> {
        font["used_by"]
            .as_array()
            .expect("font used_by")
            .iter()
            .map(|case| case.as_str().expect("used_by case id"))
            .collect()
    }

    fn has_checkpoint(checkpoints: &[(&str, &[&str])], module: &str, checkpoint: &str) -> bool {
        checkpoints
            .iter()
            .any(|(id, names)| *id == module && names.iter().any(|name| *name == checkpoint))
    }
}
