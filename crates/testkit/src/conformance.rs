use anyhow::{bail, ensure, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub const CONFORMANCE_SCHEMA_VERSION: &str = "conformance.v0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConformanceManifest {
    pub schema_version: String,
    #[serde(default)]
    pub cases: Vec<ConformanceCase>,
}

impl ConformanceManifest {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.schema_version == CONFORMANCE_SCHEMA_VERSION,
            "unsupported conformance schema_version {:?}, expected {:?}",
            self.schema_version,
            CONFORMANCE_SCHEMA_VERSION
        );
        ensure!(!self.cases.is_empty(), "conformance manifest has no cases");

        let mut names = BTreeSet::new();
        for case in &self.cases {
            ensure!(
                names.insert(case.name.as_str()),
                "duplicate conformance case {:?}",
                case.name
            );
            case.validate()
                .with_context(|| format!("invalid conformance case {:?}", case.name))?;
        }

        Ok(())
    }

    pub fn case(&self, name: &str) -> Option<&ConformanceCase> {
        self.cases.iter().find(|case| case.name == name)
    }

    pub fn cases_for_feature(
        &self,
        feature: ConformanceFeature,
    ) -> impl Iterator<Item = &ConformanceCase> {
        self.cases
            .iter()
            .filter(move |case| case.feature == feature)
    }

    pub fn pending_ae_references(&self) -> impl Iterator<Item = &ConformanceCase> {
        self.cases
            .iter()
            .filter(|case| case.ae_reference.status == AeReferenceStatus::PendingAeExport)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConformanceCase {
    pub name: String,
    pub feature: ConformanceFeature,
    pub scene: String,
    pub ae_reference: AeReference,
    pub thresholds: ConformanceThresholds,
    #[serde(default)]
    pub frames: Vec<ConformanceFrame>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

impl ConformanceCase {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.name.trim().is_empty(), "case name must not be empty");
        validate_relative_path(&self.scene, "scene")?;
        self.ae_reference.validate()?;
        self.thresholds.validate()?;
        ensure!(
            !self.frames.is_empty(),
            "case {:?} must declare at least one frame",
            self.name
        );

        let mut frame_indexes = BTreeSet::new();
        for frame in &self.frames {
            ensure!(
                frame_indexes.insert(frame.index),
                "duplicate frame {} in case {:?}",
                frame.index,
                self.name
            );
            frame.validate().with_context(|| {
                format!("invalid frame {} in case {:?}", frame.index, self.name)
            })?;
        }

        Ok(())
    }

    pub fn frame_count_hint(&self) -> u32 {
        self.frames
            .iter()
            .map(|frame| frame.index + 1)
            .max()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceFeature {
    BezierEase,
    Effects,
    Collapse,
    MotionBlur,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AeReference {
    #[serde(default)]
    pub status: AeReferenceStatus,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub exported_from: String,
}

impl AeReference {
    pub fn validate(&self) -> anyhow::Result<()> {
        match self.status {
            AeReferenceStatus::PendingAeExport => ensure!(
                !self.placeholder.trim().is_empty(),
                "pending AE references must explain the placeholder"
            ),
            AeReferenceStatus::Ready => ensure!(
                !self.exported_from.trim().is_empty(),
                "ready AE references must record exported_from"
            ),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AeReferenceStatus {
    PendingAeExport,
    Ready,
}

impl Default for AeReferenceStatus {
    fn default() -> Self {
        Self::PendingAeExport
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ConformanceThresholds {
    pub max_abs_diff: u8,
    pub mean_abs_diff: f64,
    #[serde(default)]
    pub changed_pixel_ratio: Option<f64>,
}

impl ConformanceThresholds {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.mean_abs_diff.is_finite() && self.mean_abs_diff >= 0.0,
            "mean_abs_diff must be a finite non-negative number"
        );
        if let Some(ratio) = self.changed_pixel_ratio {
            ensure!(
                ratio.is_finite() && (0.0..=1.0).contains(&ratio),
                "changed_pixel_ratio must be between 0.0 and 1.0"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConformanceFrame {
    pub index: u32,
    pub time: f64,
    pub ae_reference_png: String,
    #[serde(default)]
    pub notes: String,
}

impl ConformanceFrame {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.time.is_finite() && self.time >= 0.0,
            "frame time must be finite and non-negative"
        );
        validate_relative_path(&self.ae_reference_png, "ae_reference_png")
    }
}

fn validate_relative_path(path: &str, field: &str) -> anyhow::Result<()> {
    let path = Path::new(path);
    ensure!(
        !path.as_os_str().is_empty(),
        "{field} path must not be empty"
    );
    ensure!(
        !path.is_absolute(),
        "{field} path must be repository-relative"
    );
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            bail!("{field} path must not contain parent-directory components");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_manifest_accepts_pending_ae_references() {
        let manifest = ConformanceManifest {
            schema_version: CONFORMANCE_SCHEMA_VERSION.to_string(),
            cases: vec![ConformanceCase {
                name: "bezier_ease_probe".to_string(),
                feature: ConformanceFeature::BezierEase,
                scene: "fixtures/conformance/scenes/bezier_ease_probe_scene.json".to_string(),
                ae_reference: AeReference {
                    status: AeReferenceStatus::PendingAeExport,
                    placeholder: "export frame_000006.png from AE".to_string(),
                    exported_from: String::new(),
                },
                thresholds: ConformanceThresholds {
                    max_abs_diff: 8,
                    mean_abs_diff: 0.75,
                    changed_pixel_ratio: Some(0.02),
                },
                frames: vec![ConformanceFrame {
                    index: 6,
                    time: 0.5,
                    ae_reference_png:
                        "fixtures/conformance/ae-reference/bezier_ease/frame_000006.png".to_string(),
                    notes: String::new(),
                }],
                tags: vec!["transform".to_string()],
                notes: String::new(),
            }],
        };

        manifest.validate().unwrap();
        assert_eq!(
            manifest
                .cases_for_feature(ConformanceFeature::BezierEase)
                .count(),
            1
        );
        assert_eq!(manifest.pending_ae_references().count(), 1);
        assert_eq!(
            manifest
                .case("bezier_ease_probe")
                .unwrap()
                .frame_count_hint(),
            7
        );
    }

    #[test]
    fn fixture_manifest_validates() {
        let manifest: ConformanceManifest =
            serde_json::from_str(include_str!("../../../fixtures/conformance/manifest.json"))
                .unwrap();

        manifest.validate().unwrap();
        assert!(manifest.case("motion_blur_velocity_probe").is_some());
        assert_eq!(manifest.pending_ae_references().count(), 4);
    }

    #[test]
    fn manifest_rejects_duplicate_case_names() {
        let case = valid_case("duplicate");
        let manifest = ConformanceManifest {
            schema_version: CONFORMANCE_SCHEMA_VERSION.to_string(),
            cases: vec![case.clone(), case],
        };

        let err = manifest.validate().unwrap_err().to_string();

        assert!(err.contains("duplicate conformance case"));
    }

    #[test]
    fn manifest_rejects_unsafe_paths() {
        let mut case = valid_case("unsafe_path");
        case.scene = "../outside.json".to_string();
        let manifest = ConformanceManifest {
            schema_version: CONFORMANCE_SCHEMA_VERSION.to_string(),
            cases: vec![case],
        };

        let err = format!("{:#}", manifest.validate().unwrap_err());

        assert!(err.contains("parent-directory"));
    }

    #[test]
    fn case_rejects_duplicate_frame_indexes() {
        let mut case = valid_case("duplicate_frames");
        case.frames.push(case.frames[0].clone());

        let err = case.validate().unwrap_err().to_string();

        assert!(err.contains("duplicate frame"));
    }

    fn valid_case(name: &str) -> ConformanceCase {
        ConformanceCase {
            name: name.to_string(),
            feature: ConformanceFeature::Effects,
            scene: "fixtures/conformance/scenes/effects_stack_probe_scene.json".to_string(),
            ae_reference: AeReference {
                status: AeReferenceStatus::PendingAeExport,
                placeholder: "AE reference PNG intentionally not checked in yet".to_string(),
                exported_from: String::new(),
            },
            thresholds: ConformanceThresholds {
                max_abs_diff: 12,
                mean_abs_diff: 1.0,
                changed_pixel_ratio: None,
            },
            frames: vec![ConformanceFrame {
                index: 0,
                time: 0.0,
                ae_reference_png:
                    "fixtures/conformance/ae-reference/effects_stack/frame_000000.png".to_string(),
                notes: String::new(),
            }],
            tags: Vec::new(),
            notes: String::new(),
        }
    }
}
