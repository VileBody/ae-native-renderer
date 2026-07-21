use crate::{
    analog_glitch::AnalogGlitch, box_blur::BoxBlur2, directional_blur::DirectionalBlur,
    drop_shadow::DropShadow, f3_stylize::F3Stylize, gaussian_blur::GaussianBlur2,
    geometry::Geometry2, glow::Glow, image_wipe::ImageWipe, invert::Invert,
    layer_masks::LayerMasks, minimax::Minimax, optics_compensation::OpticsCompensation,
    posterize_time::PosterizeTime, shape_overlay::ShapeOverlay,
    turbulent_displace::TurbulentDisplace, vertical_gradient::VerticalGradient, Effect,
};

pub struct EffectRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectBackend {
    Native,
    NativeApproximation,
    ExternalPlugin,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityStatus {
    Approximate,
    ProductionSubset,
}

#[derive(Debug, Clone, Copy)]
pub struct EffectMetadata {
    pub stable_id: &'static str,
    pub ae_match_name: &'static str,
    pub backend: EffectBackend,
    pub known_params: &'static [&'static str],
    pub keyframes: bool,
    pub alpha: &'static str,
    pub color: &'static str,
    pub fallback_policy: &'static str,
    pub parity: ParityStatus,
}

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
            "ANR Layer Masks" => Some(Box::new(LayerMasks)),
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
            "ANR Layer Masks",
            "ADBE Minimax",
            "ADBE Optics Compensation",
            "ADBE Posterize Time",
            "ADBE Geometry2",
            "ADBE Turbulent Displace",
        ]
    }

    pub fn metadata(match_name: &str) -> Option<&'static EffectMetadata> {
        EFFECT_METADATA
            .iter()
            .find(|metadata| metadata.ae_match_name == match_name)
    }

    pub fn known_metadata() -> &'static [EffectMetadata] {
        EFFECT_METADATA
    }

    pub fn is_known_param(match_name: &str, param: &str) -> bool {
        let Some(metadata) = Self::metadata(match_name) else {
            return false;
        };
        metadata.known_params.contains(&param) || is_ae_numeric_param(param)
    }
}

fn is_ae_numeric_param(param: &str) -> bool {
    let bytes = param.as_bytes();
    (4..=8).contains(&bytes.len()) && bytes.iter().all(u8::is_ascii_digit)
}

const EFFECT_METADATA: &[EffectMetadata] = &[
    metadata(
        "anr.analog_glitch",
        "ANR Analog Glitch",
        &[
            "contrast",
            "red_gain",
            "wave_amplitude",
            "wave_width",
            "grid_w",
            "grid_h",
            "glow_radius",
        ],
    ),
    metadata(
        "anr.f3_stylize",
        "ANR F3 Stylize",
        &[
            "mode",
            "amount",
            "threshold",
            "softness",
            "composite_original",
            "magentas",
            "tint",
            "tint_black",
            "height",
            "width",
            "speed",
        ],
    ),
    metadata(
        "anr.shape_overlay",
        "ANR Shape Overlay",
        &[
            "shape",
            "opacity",
            "thickness",
            "size",
            "fill",
            "stroke",
            "seed",
        ],
    ),
    metadata(
        "anr.vertical_gradient",
        "ANR Vertical Gradient",
        &[
            "text_paint",
            "top",
            "bottom",
            "brightness",
            "start_xy",
            "end_xy",
            "source_match_name",
            "sapphire_params",
        ],
    ),
    metadata(
        "adbe.box_blur2",
        "ADBE Box Blur2",
        &[
            "radius",
            "iterations",
            "repeat_edge_pixels",
            "horizontal",
            "vertical",
        ],
    ),
    metadata(
        "adbe.drop_shadow",
        "ADBE Drop Shadow",
        &[
            "color",
            "opacity",
            "direction",
            "distance",
            "softness",
            "source_match_name",
            "sapphire_params",
        ],
    ),
    metadata(
        "adbe.motion_blur",
        "ADBE Motion Blur",
        &["direction", "blur_length", "0051", "S_BlurMotion-0051"],
    ),
    metadata(
        "adbe.gaussian_blur2",
        "ADBE Gaussian Blur 2",
        &[
            "blurriness",
            "repeat_edge_pixels",
            "9961714",
            "BCC6LensBlur-9961714",
        ],
    ),
    metadata(
        "adbe.glo2",
        "ADBE Glo2",
        &[
            "threshold",
            "radius",
            "intensity",
            "operation",
            "color",
            "based_on",
            "composite_original",
        ],
    ),
    metadata(
        "cc.image_wipe",
        "CC Image Wipe",
        &["completion", "border_softness"],
    ),
    metadata(
        "adbe.invert",
        "ADBE Invert",
        &["channel", "blend_with_original"],
    ),
    metadata("anr.layer_masks", "ANR Layer Masks", &["masks"]),
    metadata(
        "adbe.minimax",
        "ADBE Minimax",
        &["operation", "channels", "direction", "radius"],
    ),
    metadata(
        "adbe.optics_compensation",
        "ADBE Optics Compensation",
        &["field_of_view", "reverse", "center"],
    ),
    metadata(
        "adbe.posterize_time",
        "ADBE Posterize Time",
        &["frameRate", "frame_rate"],
    ),
    metadata(
        "adbe.geometry2",
        "ADBE Geometry2",
        &[
            "anchor",
            "position",
            "scale",
            "scale_width",
            "scale_height",
            "rotation",
            "opacity",
        ],
    ),
    metadata(
        "adbe.turbulent_displace",
        "ADBE Turbulent Displace",
        &[
            "amount",
            "size",
            "offset",
            "complexity",
            "evolution",
            "seed",
            "pinning",
            "resize_layer",
        ],
    ),
];

const fn metadata(
    stable_id: &'static str,
    ae_match_name: &'static str,
    known_params: &'static [&'static str],
) -> EffectMetadata {
    EffectMetadata {
        stable_id,
        ae_match_name,
        backend: EffectBackend::NativeApproximation,
        known_params,
        keyframes: true,
        alpha: "straight-rgba8-boundary/premultiplied-float-sampling",
        color: "unmanaged-srgb-approximation",
        fallback_policy: "unsupported_unknown_params",
        parity: ParityStatus::Approximate,
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

    #[test]
    fn registry_exposes_metadata_and_param_contract() {
        let metadata = EffectRegistry::metadata("ADBE Drop Shadow").unwrap();
        assert_eq!(metadata.stable_id, "adbe.drop_shadow");
        assert!(metadata.keyframes);
        assert!(EffectRegistry::is_known_param(
            "ADBE Drop Shadow",
            "softness"
        ));
        assert!(EffectRegistry::is_known_param("ADBE Drop Shadow", "0052"));
        assert!(!EffectRegistry::is_known_param(
            "ADBE Drop Shadow",
            "mystery"
        ));
    }
}
