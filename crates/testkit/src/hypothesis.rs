use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub const HYPOTHESIS_RUN_SCHEMA: &str = "ae-native-renderer.hypothesis-run.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisStatus {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisGateKind {
    Isolated,
    Composition,
    Stack,
    Template,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypothesisGate {
    pub kind: HypothesisGateKind,
    pub name: String,
    #[serde(default)]
    pub cases: Vec<String>,
    pub ok: bool,
    #[serde(default)]
    pub metrics: Value,
    #[serde(default)]
    pub artifacts: BTreeMap<String, String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypothesisRunReport {
    pub schema: String,
    pub module: String,
    pub candidate_id: String,
    pub status: HypothesisStatus,
    pub accepted: bool,
    pub rejected: bool,
    pub needs_new_probe: bool,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub evidence_sources: Vec<String>,
    #[serde(default)]
    pub candidate_config: Value,
    pub candidate_config_hash: String,
    #[serde(default)]
    pub gates: Vec<HypothesisGate>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl HypothesisRunReport {
    pub fn new(module: impl Into<String>, candidate_id: impl Into<String>) -> Self {
        let candidate_config = Value::Null;
        let candidate_config_hash =
            crate::telemetry::hash_json(&candidate_config).unwrap_or_else(|_| String::new());
        let mut report = Self {
            schema: HYPOTHESIS_RUN_SCHEMA.to_string(),
            module: module.into(),
            candidate_id: candidate_id.into(),
            status: HypothesisStatus::CandidateReady,
            accepted: false,
            rejected: false,
            needs_new_probe: false,
            question: None,
            hypothesis: None,
            evidence_sources: Vec::new(),
            candidate_config,
            candidate_config_hash,
            gates: Vec::new(),
            artifacts: BTreeMap::new(),
            notes: Vec::new(),
        };
        report.refresh_decision_flags();
        report
    }

    pub fn with_status(mut self, status: HypothesisStatus) -> Self {
        self.status = status;
        self.refresh_decision_flags();
        self
    }

    pub fn with_candidate_config(mut self, candidate_config: Value) -> anyhow::Result<Self> {
        self.candidate_config_hash = crate::telemetry::hash_json(&candidate_config)?;
        self.candidate_config = candidate_config;
        Ok(self)
    }

    pub fn refresh_decision_flags(&mut self) {
        self.accepted = matches!(
            self.status,
            HypothesisStatus::IsolatedImproved
                | HypothesisStatus::CompositionImproved
                | HypothesisStatus::StackSafe
                | HypothesisStatus::ReverseImplemented
                | HypothesisStatus::ParityLocked
        );
        self.rejected = self.status == HypothesisStatus::Rejected;
        self.needs_new_probe = self.status == HypothesisStatus::NeedsNewProbe;
    }
}

pub fn write_hypothesis_report(
    path: impl AsRef<Path>,
    report: &HypothesisRunReport,
) -> anyhow::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(report)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn report_hashes_candidate_config_stably() {
        let report = HypothesisRunReport::new("M10", "shadow_blur_floor")
            .with_candidate_config(json!({
                "blur_radius_mode": "floor",
                "edge_policy": "transparent"
            }))
            .unwrap();

        assert_eq!(report.schema, HYPOTHESIS_RUN_SCHEMA);
        assert_eq!(report.module, "M10");
        assert_eq!(report.candidate_id, "shadow_blur_floor");
        assert_eq!(report.candidate_config_hash.len(), 64);
    }

    #[test]
    fn status_sets_decision_flags() {
        let improved = HypothesisRunReport::new("M13", "fractional_radius")
            .with_status(HypothesisStatus::IsolatedImproved);
        assert!(improved.accepted);
        assert!(!improved.rejected);
        assert!(!improved.needs_new_probe);

        let rejected =
            HypothesisRunReport::new("M13", "radius_floor").with_status(HypothesisStatus::Rejected);
        assert!(!rejected.accepted);
        assert!(rejected.rejected);

        let blocked = HypothesisRunReport::new("M14", "field_path_unknown")
            .with_status(HypothesisStatus::NeedsNewProbe);
        assert!(blocked.needs_new_probe);
    }

    #[test]
    fn gate_serializes_cases_metrics_and_artifacts() {
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            "conformance_report".to_string(),
            "target/ae_agents/m10/report.json".to_string(),
        );
        let gate = HypothesisGate {
            kind: HypothesisGateKind::Isolated,
            name: "native_conformance_pack".to_string(),
            cases: vec!["EFF_010".to_string()],
            ok: true,
            metrics: json!({ "mean_abs_diff": 0.25 }),
            artifacts,
            notes: vec!["shadow mask improved".to_string()],
        };

        let encoded = serde_json::to_value(&gate).unwrap();
        assert_eq!(encoded["kind"], "isolated");
        assert_eq!(encoded["cases"][0], "EFF_010");
        assert_eq!(
            encoded["artifacts"]["conformance_report"],
            "target/ae_agents/m10/report.json"
        );
    }
}
