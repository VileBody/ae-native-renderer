use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    SourceDecode,
    TextLayout,
    SelectorWeights,
    LayerTransform,
    EffectInput,
    EffectOutput,
    PrecompOutput,
    MotionBlurSample,
    MotionBlurAccumulation,
    FinalFrame,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub kind: CheckpointKind,
    pub name: String,
    pub frame_index: u64,
    pub time_seconds: f64,
    pub hash: String,
    #[serde(default)]
    pub metrics: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TelemetryTrace {
    pub checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointDiff {
    pub index: usize,
    pub expected_name: String,
    pub actual_name: String,
    pub expected_hash: String,
    pub actual_hash: String,
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    encode_hex(&hasher.finalize())
}

pub fn hash_json(value: &Value) -> anyhow::Result<String> {
    Ok(hash_bytes(&serde_json::to_vec(value)?))
}

impl TelemetryTrace {
    pub fn first_hash_divergence(&self, actual: &TelemetryTrace) -> Option<CheckpointDiff> {
        for (index, (expected, actual)) in
            self.checkpoints.iter().zip(&actual.checkpoints).enumerate()
        {
            if expected.hash != actual.hash || expected.name != actual.name {
                return Some(CheckpointDiff {
                    index,
                    expected_name: expected.name.clone(),
                    actual_name: actual.name.clone(),
                    expected_hash: expected.hash.clone(),
                    actual_hash: actual.hash.clone(),
                });
            }
        }

        if self.checkpoints.len() != actual.checkpoints.len() {
            let index = self.checkpoints.len().min(actual.checkpoints.len());
            return Some(CheckpointDiff {
                index,
                expected_name: self
                    .checkpoints
                    .get(index)
                    .map(|checkpoint| checkpoint.name.clone())
                    .unwrap_or_else(|| "<missing>".to_string()),
                actual_name: actual
                    .checkpoints
                    .get(index)
                    .map(|checkpoint| checkpoint.name.clone())
                    .unwrap_or_else(|| "<missing>".to_string()),
                expected_hash: self
                    .checkpoints
                    .get(index)
                    .map(|checkpoint| checkpoint.hash.clone())
                    .unwrap_or_default(),
                actual_hash: actual
                    .checkpoints
                    .get(index)
                    .map(|checkpoint| checkpoint.hash.clone())
                    .unwrap_or_default(),
            });
        }

        None
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
