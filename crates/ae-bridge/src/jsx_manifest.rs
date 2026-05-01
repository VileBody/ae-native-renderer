use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsxManifest {
    pub template_id: String,
    pub required_effects: Vec<String>,
    pub required_fonts: Vec<String>,
    pub notes: Vec<String>,
}
