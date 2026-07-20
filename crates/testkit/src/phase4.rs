#[derive(Debug, Clone, Copy)]
pub struct Phase4Case {
    pub id: &'static str,
    pub frames: &'static [u32],
    pub checkpoints: &'static [&'static str],
}

pub const GEOMETRY2_CASE: Phase4Case = Phase4Case {
    id: "EFF_040",
    frames: &[0],
    checkpoints: &[
        "params",
        "property_mapping",
        "matrix",
        "inverse_matrix",
        "sample_uv",
        "final_pixels",
    ],
};

pub const TURBULENT_DISPLACE_CASE: Phase4Case = Phase4Case {
    id: "EFF_060",
    frames: &[0, 1, 15, 30, 45, 59],
    checkpoints: &[
        "params",
        "noise",
        "dx",
        "dy",
        "displacement_uv",
        "field_state",
        "field_hash",
        "sampled_source",
        "final_pixels",
    ],
};

pub const STACK_030_CASE: Phase4Case = Phase4Case {
    id: "STK_030",
    frames: &[0, 1, 5, 10, 15, 20, 30, 45, 59],
    checkpoints: &[
        "geometry2_final",
        "posterize_quantized_time",
        "minimax_output_mask",
        "turbulent_displacement_uv",
        "final_pixels",
    ],
};

pub fn phase4_cases() -> [Phase4Case; 3] {
    [GEOMETRY2_CASE, TURBULENT_DISPLACE_CASE, STACK_030_CASE]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator::{
        builtin_passport, Dimension, Discontinuity, OperatorKind, OperatorPassport,
    };
    use image::GenericImageView;
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    const PACK_ROOT: &str = "../../fixtures/ae_conformance_pack";

    fn repo_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    fn conformance_manifest() -> Value {
        let path = repo_path(&format!("{PACK_ROOT}/manifest.json"));
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        serde_json::from_str(&raw).expect("AE conformance manifest should be valid JSON")
    }

    fn case<'a>(manifest: &'a Value, id: &str) -> &'a Value {
        manifest["cases"]
            .as_array()
            .expect("manifest cases should be an array")
            .iter()
            .find(|case| case["id"] == id)
            .unwrap_or_else(|| panic!("manifest should contain case {id}"))
    }

    fn frames_to_compare(case: &Value) -> Vec<u32> {
        case["frames_to_compare"]
            .as_array()
            .expect("case should declare frames_to_compare")
            .iter()
            .map(|frame| {
                frame
                    .as_u64()
                    .expect("frame index should be an unsigned integer") as u32
            })
            .collect()
    }

    fn assert_passport_debugs(passport: &OperatorPassport, expected: &[&str]) {
        for expected_debug in expected {
            assert!(
                passport
                    .debug_outputs
                    .iter()
                    .any(|debug| debug == expected_debug),
                "{} should expose debug output {expected_debug}",
                passport.match_name
            );
        }
    }

    #[test]
    fn manifest_selects_phase4_frames() {
        let manifest = conformance_manifest();

        for phase_case in phase4_cases() {
            let manifest_case = case(&manifest, phase_case.id);
            assert_eq!(frames_to_compare(manifest_case), phase_case.frames);
        }
    }

    #[test]
    fn selected_ae_goldens_exist_and_match_pack_dimensions() {
        let manifest = conformance_manifest();
        let width = manifest["composition"]["width"].as_u64().unwrap() as u32;
        let height = manifest["composition"]["height"].as_u64().unwrap() as u32;
        let missing = missing_phase4_goldens();
        if !missing.is_empty() {
            if !crate::ae_png_goldens_required() {
                eprintln!(
                    "skipping phase4 AE PNG golden load test: {} files missing; set {}=1 to require goldens",
                    missing.len(),
                    crate::REQUIRE_AE_PNG_GOLDENS_ENV
                );
                return;
            }
            panic!("missing phase4 AE PNG goldens: {missing:?}");
        }

        for phase_case in phase4_cases() {
            for frame in phase_case.frames {
                let path = repo_path(&format!(
                    "{PACK_ROOT}/ae_goldens/png/{0}/{0}_{1:05}.png",
                    phase_case.id, frame
                ));
                let image = image::open(&path)
                    .unwrap_or_else(|err| panic!("failed to open {}: {err}", path.display()));
                assert_eq!(
                    image.dimensions(),
                    (width, height),
                    "{} frame {} should match manifest dimensions",
                    phase_case.id,
                    frame
                );
            }
        }
    }

    fn missing_phase4_goldens() -> Vec<PathBuf> {
        phase4_cases()
            .into_iter()
            .flat_map(|phase_case| {
                phase_case.frames.iter().map(move |frame| {
                    repo_path(&format!(
                        "{PACK_ROOT}/ae_goldens/png/{0}/{0}_{1:05}.png",
                        phase_case.id, frame
                    ))
                })
            })
            .filter(|path| !path.is_file())
            .collect()
    }

    #[test]
    fn geometry2_passport_covers_matrix_to_sampling_chain() {
        let passport = builtin_passport("ADBE Geometry2").unwrap();

        assert_eq!(passport.kind, OperatorKind::Geometry);
        assert!(passport.has_dimension(Dimension::Coordinate));
        assert!(passport.has_dimension(Dimension::AnimatedParams));
        assert!(passport.has_discontinuity(Discontinuity::SamplerRounding));
        assert!(passport.has_discontinuity(Discontinuity::EdgeSampling));
        assert_passport_debugs(
            &passport,
            &["property_mapping", "matrix", "inverse_matrix", "sample_uv"],
        );
    }

    #[test]
    fn turbulent_displace_passport_prioritizes_displacement_field_parity() {
        let passport = builtin_passport("ADBE Turbulent Displace").unwrap();

        assert_eq!(passport.kind, OperatorKind::CoordinateWarp);
        assert!(passport.has_dimension(Dimension::Coordinate));
        assert!(passport.has_dimension(Dimension::Procedural));
        assert!(passport.has_dimension(Dimension::Temporal));
        assert!(passport.has_discontinuity(Discontinuity::ComplexityOctaveBoundary));
        assert!(passport.has_discontinuity(Discontinuity::SeedChange));
        assert_passport_debugs(
            &passport,
            &[
                "noise",
                "dx",
                "dy",
                "uv",
                "field_state",
                "field_hash",
                "sampled_source",
            ],
        );
    }

    #[test]
    fn stk030_manifest_keeps_geometry_posterize_minimax_turbulent_context() {
        let manifest = conformance_manifest();
        let stk030 = case(&manifest, STACK_030_CASE.id);
        let modules = stk030["modules"]
            .as_array()
            .expect("STK_030 should declare module IDs")
            .iter()
            .map(|module| module.as_str().unwrap())
            .collect::<Vec<_>>();

        assert!(modules.contains(&"M12"));
        assert!(modules.contains(&"M13"));
        assert!(modules.contains(&"M14"));
        assert!(modules.contains(&"M15"));
        assert!(modules.contains(&"M16"));
        assert!(modules.contains(&"M19"));
        assert!(stk030["title"]
            .as_str()
            .unwrap()
            .contains("Geometry2 -> Posterize Time -> Minimax -> Turbulent Displace"));
    }
}
