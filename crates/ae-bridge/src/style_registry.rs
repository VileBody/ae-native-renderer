#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleBackend {
    NativeApproximation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleParityStatus {
    Approximate,
}

#[derive(Debug, Clone, Copy)]
pub struct StyleMetadata {
    pub stable_id: &'static str,
    pub source: &'static str,
    pub backend: StyleBackend,
    pub effect_ids: &'static [&'static str],
    pub alpha: &'static str,
    pub color: &'static str,
    pub fallback_policy: &'static str,
    pub parity: StyleParityStatus,
}

pub struct StyleRegistry;

impl StyleRegistry {
    pub fn metadata(stable_id: &str) -> Option<&'static StyleMetadata> {
        STYLE_METADATA
            .iter()
            .find(|metadata| metadata.stable_id == stable_id)
    }

    pub fn known_metadata() -> &'static [StyleMetadata] {
        STYLE_METADATA
    }

    pub fn effects(stable_id: &str) -> Option<Vec<render_ir::EffectSpec>> {
        let recipes: serde_json::Value =
            serde_json::from_str(include_str!("../config/style_recipes.v1.json"))
                .expect("embedded style recipe catalog must be valid JSON");
        serde_json::from_value(recipes.get(stable_id)?.clone()).ok()
    }
}

const STYLE_METADATA: &[StyleMetadata] = &[
    metadata(
        "ftg_al16_default_v1",
        &["ADBE Gaussian Blur 2", "ADBE Geometry2", "ADBE Motion Blur"],
    ),
    metadata("txt_soft_v1", &["ADBE Glo2", "ADBE Geometry2"]),
    metadata("txt_punch_v1", &["ADBE Geometry2", "ADBE Motion Blur"]),
    metadata(
        "txt_drop_v1",
        &["ADBE Glo2", "ADBE Geometry2", "ADBE Motion Blur"],
    ),
];

const fn metadata(stable_id: &'static str, effect_ids: &'static [&'static str]) -> StyleMetadata {
    StyleMetadata {
        stable_id,
        source: "blast.bot.semantic_style",
        backend: StyleBackend::NativeApproximation,
        effect_ids,
        alpha: "straight-rgba8-boundary/premultiplied-float-effects",
        color: "unmanaged-srgb-approximation",
        fallback_policy: "not_implemented_unknown_style",
        parity: StyleParityStatus::Approximate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_exposes_frozen_bot_style_metadata() {
        let style = StyleRegistry::metadata("txt_soft_v1").unwrap();
        assert_eq!(style.source, "blast.bot.semantic_style");
        assert_eq!(style.effect_ids, &["ADBE Glo2", "ADBE Geometry2"]);
        assert!(StyleRegistry::metadata("txt_unknown_v1").is_none());
        assert_eq!(StyleRegistry::effects("txt_soft_v1").unwrap().len(), 2);
        assert!(StyleRegistry::effects("txt_unknown_v1").is_none());
    }
}
