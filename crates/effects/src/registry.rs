use crate::{
    box_blur::BoxBlur2, drop_shadow::DropShadow, geometry::Geometry2, glow::Glow,
    minimax::Minimax, posterize_time::PosterizeTime, turbulent_displace::TurbulentDisplace, Effect,
};

pub struct EffectRegistry;

impl EffectRegistry {
    pub fn create(match_name: &str) -> Option<Box<dyn Effect>> {
        match match_name {
            "ADBE Box Blur2" => Some(Box::new(BoxBlur2::default())),
            "ADBE Drop Shadow" => Some(Box::new(DropShadow::default())),
            "ADBE Glo2" => Some(Box::new(Glow::default())),
            "ADBE Minimax" => Some(Box::new(Minimax::default())),
            "ADBE Posterize Time" => Some(Box::new(PosterizeTime::default())),
            "ADBE Geometry2" => Some(Box::new(Geometry2::default())),
            "ADBE Turbulent Displace" => Some(Box::new(TurbulentDisplace::default())),
            _ => None,
        }
    }

    pub fn known_match_names() -> &'static [&'static str] {
        &[
            "ADBE Box Blur2",
            "ADBE Drop Shadow",
            "ADBE Glo2",
            "ADBE Minimax",
            "ADBE Posterize Time",
            "ADBE Geometry2",
            "ADBE Turbulent Displace",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_accepts_supported_ae_match_names() {
        for match_name in EffectRegistry::known_match_names() {
            let effect = EffectRegistry::create(match_name)
                .unwrap_or_else(|| panic!("missing effect for {match_name}"));
            assert_eq!(effect.match_name(), *match_name);
        }
    }

    #[test]
    fn registry_rejects_unknown_match_name() {
        assert!(EffectRegistry::create("ADBE Definitely Not Real").is_none());
    }
}
