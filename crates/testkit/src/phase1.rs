use crate::{load_rgba_png, RgbaImageData};
use anyhow::{ensure, Context};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const PHASE1_CASE_IDS: &[&str] = &["CMP_010", "INT_010", "INT_020", "EFF_040"];

#[derive(Debug, Clone, Deserialize)]
pub struct AeConformancePackManifest {
    pub schema: String,
    pub pack_id: String,
    pub composition: AePackComposition,
    #[serde(default)]
    pub cases: Vec<AePackCase>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AePackComposition {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub case_duration_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AePackCase {
    pub id: String,
    pub title: String,
    pub ladder: String,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub frames_to_compare: Vec<u32>,
    #[serde(default)]
    pub assets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AeConformancePack {
    root: PathBuf,
    manifest: AeConformancePackManifest,
}

#[derive(Debug, Clone)]
pub struct Phase1GoldenAudit {
    pub cases_checked: usize,
    pub frames_checked: usize,
}

impl AeConformancePack {
    pub fn load(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let manifest_path = root.join("manifest.json");
        let manifest: AeConformancePackManifest = serde_json::from_slice(
            &std::fs::read(&manifest_path)
                .with_context(|| format!("failed to read {}", manifest_path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
        manifest.validate()?;
        Ok(Self { root, manifest })
    }

    pub fn manifest(&self) -> &AeConformancePackManifest {
        &self.manifest
    }

    pub fn case(&self, id: &str) -> Option<&AePackCase> {
        self.manifest.cases.iter().find(|case| case.id == id)
    }

    pub fn phase1_cases(&self) -> anyhow::Result<Vec<&AePackCase>> {
        PHASE1_CASE_IDS
            .iter()
            .map(|id| {
                self.case(id)
                    .with_context(|| format!("phase 1 case {id} is missing from AE pack manifest"))
            })
            .collect()
    }

    pub fn golden_png_path(&self, case_id: &str, frame: u32) -> PathBuf {
        self.root
            .join("ae_goldens")
            .join("png")
            .join(case_id)
            .join(format!("{case_id}_{frame:05}.png"))
    }

    pub fn load_golden_png(&self, case_id: &str, frame: u32) -> anyhow::Result<RgbaImageData> {
        let path = self.golden_png_path(case_id, frame);
        load_rgba_png(&path).with_context(|| format!("failed to load {}", path.display()))
    }

    pub fn audit_phase1_goldens(&self) -> anyhow::Result<Phase1GoldenAudit> {
        let mut frames_checked = 0;
        let cases = self.phase1_cases()?;
        for case in &cases {
            ensure!(
                !case.frames_to_compare.is_empty(),
                "phase 1 case {} has no frames_to_compare",
                case.id
            );
            for &frame in &case.frames_to_compare {
                let image = self.load_golden_png(&case.id, frame)?;
                ensure!(
                    image.width == self.manifest.composition.width
                        && image.height == self.manifest.composition.height,
                    "golden {} frame {} has {}x{}, expected {}x{}",
                    case.id,
                    frame,
                    image.width,
                    image.height,
                    self.manifest.composition.width,
                    self.manifest.composition.height
                );
                frames_checked += 1;
            }
        }
        Ok(Phase1GoldenAudit {
            cases_checked: cases.len(),
            frames_checked,
        })
    }

    pub fn missing_phase1_golden_paths(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut missing = Vec::new();
        for case in self.phase1_cases()? {
            for &frame in &case.frames_to_compare {
                let path = self.golden_png_path(&case.id, frame);
                if !path.is_file() {
                    missing.push(path);
                }
            }
        }
        Ok(missing)
    }
}

impl AeConformancePackManifest {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.schema == "ae-native-renderer.conformance-pack.v1",
            "unexpected AE conformance pack schema {:?}",
            self.schema
        );
        ensure!(
            !self.pack_id.trim().is_empty(),
            "AE conformance pack id must not be empty"
        );
        ensure!(
            self.composition.width > 0 && self.composition.height > 0,
            "AE conformance composition dimensions must be positive"
        );
        ensure!(
            self.composition.fps.is_finite() && self.composition.fps > 0.0,
            "AE conformance composition fps must be positive"
        );
        ensure!(
            self.composition.case_duration_seconds.is_finite()
                && self.composition.case_duration_seconds > 0.0,
            "AE conformance case duration must be positive"
        );
        ensure!(!self.cases.is_empty(), "AE conformance pack has no cases");

        for case in &self.cases {
            ensure!(!case.id.trim().is_empty(), "case id must not be empty");
            ensure!(
                !case.frames_to_compare.is_empty(),
                "case {} must list frames_to_compare",
                case.id
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> AeConformancePack {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/ae_conformance_pack");
        AeConformancePack::load(root).expect("AE conformance pack should load")
    }

    #[test]
    fn phase1_cases_are_declared_with_expected_scope() {
        let pack = pack();
        let cases = pack.phase1_cases().unwrap();
        let ids: Vec<_> = cases.iter().map(|case| case.id.as_str()).collect();
        assert_eq!(ids, PHASE1_CASE_IDS);

        assert_eq!(pack.case("CMP_010").unwrap().modules, ["M19"]);
        assert_eq!(pack.case("INT_010").unwrap().modules, ["M04"]);
        assert_eq!(pack.case("INT_020").unwrap().modules, ["M04"]);
        assert_eq!(pack.case("EFF_040").unwrap().modules, ["M12"]);
    }

    #[test]
    fn phase1_selected_ae_goldens_are_loadable_rgba_pngs() {
        let pack = pack();
        let missing = pack.missing_phase1_golden_paths().unwrap();
        if !missing.is_empty() && !crate::ae_png_goldens_required() {
            eprintln!(
                "skipping phase1 AE PNG golden load test: {} files missing; set {}=1 to require checked-in goldens",
                missing.len(),
                crate::REQUIRE_AE_PNG_GOLDENS_ENV
            );
            return;
        }
        assert!(
            missing.is_empty(),
            "missing phase1 AE PNG goldens: {:?}",
            missing
        );

        let audit = pack.audit_phase1_goldens().unwrap();
        assert_eq!(audit.cases_checked, 4);
        assert_eq!(audit.frames_checked, 15);
    }
}
