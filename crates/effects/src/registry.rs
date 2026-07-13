use crate::{
    analog_glitch::AnalogGlitch, box_blur::BoxBlur2, directional_blur::DirectionalBlur,
    drop_shadow::DropShadow, f3_stylize::F3Stylize, gaussian_blur::GaussianBlur2,
    geometry::Geometry2, glow::Glow, image_wipe::ImageWipe, invert::Invert, minimax::Minimax,
    optics_compensation::OpticsCompensation, posterize_time::PosterizeTime,
    shape_overlay::ShapeOverlay, turbulent_displace::TurbulentDisplace,
    vertical_gradient::VerticalGradient, Effect,
};

pub struct EffectRegistry;

impl EffectRegistry {
    pub fn create(match_name: &str) -> Option<Box<dyn Effect>> {
        match match_name {
            "ANR Analog Glitch" => Some(Box::new(AnalogGlitch)),
            "ANR F3 Stylize" => Some(Box::new(F3Stylize)),
            "ANR Shape Overlay" => Some(Box::new(ShapeOverlay)),
            "ANR Vertical Gradient" => Some(Box::new(VerticalGradient)),
            "ADBE Box Blur2" => Some(Box::new(BoxBlur2::default())),
            "ADBE Drop Shadow" => Some(Box::new(DropShadow::default())),
            "ADBE Motion Blur" => Some(Box::new(DirectionalBlur)),
            "ADBE Gaussian Blur 2" => Some(Box::new(GaussianBlur2)),
            "ADBE Glo2" => Some(Box::new(Glow::default())),
            "CC Image Wipe" => Some(Box::new(ImageWipe)),
            "ADBE Invert" => Some(Box::new(Invert)),
            "ADBE Minimax" => Some(Box::new(Minimax::default())),
            "ADBE Optics Compensation" => Some(Box::new(OpticsCompensation)),
            "ADBE Posterize Time" => Some(Box::new(PosterizeTime::default())),
            "ADBE Geometry2" => Some(Box::new(Geometry2::default())),
            "ADBE Turbulent Displace" => Some(Box::new(TurbulentDisplace::default())),
            _ => None,
        }
    }

    pub fn known_match_names() -> &'static [&'static str] {
        &[
            "ANR Analog Glitch",
            "ANR F3 Stylize",
            "ANR Shape Overlay",
            "ANR Vertical Gradient",
            "ADBE Box Blur2",
            "ADBE Drop Shadow",
            "ADBE Motion Blur",
            "ADBE Gaussian Blur 2",
            "ADBE Glo2",
            "CC Image Wipe",
            "ADBE Invert",
            "ADBE Minimax",
            "ADBE Optics Compensation",
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
