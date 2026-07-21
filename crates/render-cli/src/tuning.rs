use anyhow::{bail, Context};
use render_ir::{EffectSpec, Layer, Scene, Transform2D};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const TUNING_SCHEMA: &str = "ae-native-renderer.native-tunables.v1";
const BUILTIN_P0P1: &str = include_str!("../../../configs/native_tunables.p0p1.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuningSpec {
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default = "empty_object")]
    pub overrides: Value,
}

impl Default for TuningSpec {
    fn default() -> Self {
        Self {
            profile: None,
            overrides: empty_object(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTunables {
    pub schema: String,
    pub profile: String,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub text: TextTunables,
    #[serde(default)]
    pub effects: EffectTunables,
    #[serde(default)]
    pub styles: BTreeMap<String, BTreeMap<String, f32>>,
    #[serde(default)]
    pub f3: BTreeMap<String, BTreeMap<String, f32>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextTunables {
    #[serde(default)]
    pub baseline_offsets_px: BTreeMap<String, f32>,
    #[serde(default)]
    pub source_rect: SourceRectTunables,
    #[serde(default)]
    pub reveal: RevealTunables,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRectTunables {
    #[serde(default)]
    pub edge_tolerance_px: f32,
    #[serde(default)]
    pub alpha_threshold: u8,
    #[serde(default)]
    pub wrap_epsilon_px: f32,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RevealTunables {
    #[serde(default)]
    pub min_visible_scale: f32,
    #[serde(default)]
    pub min_visible_alpha: f32,
    #[serde(default)]
    pub word_timing_frame_tolerance: u32,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectTunables {
    #[serde(default)]
    pub blur: BlurTunables,
    #[serde(default)]
    pub glow: GlowTunables,
    #[serde(default)]
    pub shadow: ShadowTunables,
    #[serde(default)]
    pub sampling: SamplingTunables,
    #[serde(default)]
    pub alpha: AlphaTunables,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlurTunables {
    #[serde(default = "one")]
    pub box_radius_multiplier: f32,
    #[serde(default = "one")]
    pub gaussian_sigma_multiplier: f32,
    #[serde(default)]
    pub repeat_edge_pixels_default: bool,
}

impl Default for BlurTunables {
    fn default() -> Self {
        Self {
            box_radius_multiplier: 1.0,
            gaussian_sigma_multiplier: 1.0,
            repeat_edge_pixels_default: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlowTunables {
    #[serde(default = "one")]
    pub radius_multiplier: f32,
    #[serde(default = "one")]
    pub intensity_multiplier: f32,
    #[serde(default)]
    pub threshold_offset: f32,
    #[serde(default = "default_true")]
    pub composite_original_default: bool,
}

impl Default for GlowTunables {
    fn default() -> Self {
        Self {
            radius_multiplier: 1.0,
            intensity_multiplier: 1.0,
            threshold_offset: 0.0,
            composite_original_default: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowTunables {
    #[serde(default = "one")]
    pub softness_multiplier: f32,
    #[serde(default = "one")]
    pub opacity_multiplier: f32,
    #[serde(default = "one")]
    pub distance_multiplier: f32,
}

impl Default for ShadowTunables {
    fn default() -> Self {
        Self {
            softness_multiplier: 1.0,
            opacity_multiplier: 1.0,
            distance_multiplier: 1.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingTunables {
    #[serde(default = "default_resampling")]
    pub resampling: String,
    #[serde(default = "default_transparent")]
    pub optics_edge: String,
    #[serde(default = "default_transparent")]
    pub motion_blur_edge: String,
}

impl Default for SamplingTunables {
    fn default() -> Self {
        Self {
            resampling: default_resampling(),
            optics_edge: default_transparent(),
            motion_blur_edge: default_transparent(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaTunables {
    #[serde(default = "default_boundary")]
    pub boundary: String,
    #[serde(default = "default_internal")]
    pub internal: String,
    #[serde(default = "default_matte_threshold")]
    pub matte_threshold: u8,
}

impl Default for AlphaTunables {
    fn default() -> Self {
        Self {
            boundary: default_boundary(),
            internal: default_internal(),
            matte_threshold: default_matte_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TuningReport {
    pub profile: String,
    pub source: String,
    pub sha256: String,
    pub applied: Vec<String>,
    pub contract_only: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedTuning {
    pub profile: NativeTunables,
    pub report: TuningReport,
    pub normalized: Value,
}

pub fn resolve(spec: &TuningSpec, base_dir: &Path) -> anyhow::Result<Option<ResolvedTuning>> {
    let overrides = spec
        .overrides
        .as_object()
        .context("tuningSpec.overrides must be an object")?;
    let has_overrides = !overrides.is_empty();
    if spec.profile.as_deref().is_none_or(str::is_empty) && !has_overrides {
        return Ok(None);
    }

    let (mut raw, source) = match spec.profile.as_deref().filter(|value| !value.is_empty()) {
        Some("builtin:p0p1-readiness") => (
            serde_json::from_str(BUILTIN_P0P1).context("parsing built-in P0/P1 tuning profile")?,
            "builtin:p0p1-readiness".to_string(),
        ),
        Some(profile) => {
            let path = PathBuf::from(profile);
            let path = if path.is_absolute() {
                path
            } else {
                base_dir.join(path)
            };
            let bytes = fs::read(&path)
                .with_context(|| format!("reading tuning profile {}", path.display()))?;
            (
                serde_json::from_slice(&bytes)
                    .with_context(|| format!("parsing tuning profile {}", path.display()))?,
                profile.to_string(),
            )
        }
        None => (
            serde_json::from_str(BUILTIN_P0P1).context("parsing built-in P0/P1 tuning profile")?,
            "builtin:p0p1-readiness+overrides".to_string(),
        ),
    };
    merge_json(&mut raw, &spec.overrides)?;
    let profile: NativeTunables = serde_json::from_value(raw.clone())
        .context("validating native tuning profile (unknown fields are rejected)")?;
    validate(&profile)?;
    let normalized_bytes = serde_json::to_vec(&profile)?;
    let sha256 = format!("{:x}", Sha256::digest(&normalized_bytes));
    Ok(Some(ResolvedTuning {
        report: TuningReport {
            profile: profile.profile.clone(),
            source,
            sha256,
            applied: Vec::new(),
            contract_only: vec![
                "text.source_rect.* (differential acceptance thresholds)".to_string(),
                "text.reveal.* (visual acceptance thresholds)".to_string(),
                "effects.alpha.matte_threshold (reserved for full matte compositor)".to_string(),
            ],
        },
        normalized: serde_json::to_value(&profile)?,
        profile,
    }))
}

pub fn apply_to_scene(scene: &mut Scene, resolved: &mut ResolvedTuning) -> anyhow::Result<()> {
    let mut applied = BTreeSet::new();
    let fps = scene.composition.fps;
    for layer in &mut scene.layers {
        apply_layer(layer, fps, &resolved.profile, &mut applied)?;
    }
    for composition in &mut scene.compositions {
        for layer in &mut composition.layers {
            apply_layer(
                layer,
                composition.composition.fps,
                &resolved.profile,
                &mut applied,
            )?;
        }
    }
    resolved.report.applied = applied.into_iter().collect();
    Ok(())
}

fn apply_layer(
    layer: &mut Layer,
    fps: f64,
    profile: &NativeTunables,
    applied: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    let id = layer.id().to_string();
    if let Layer::Text {
        text,
        font,
        char_styles,
        transform,
        ..
    } = layer
    {
        let key = baseline_key(font, text, char_styles);
        let offset = profile
            .text
            .baseline_offsets_px
            .get(key)
            .or_else(|| profile.text.baseline_offsets_px.get("default"))
            .copied()
            .unwrap_or(0.0);
        if offset.abs() > f32::EPSILON {
            offset_transform_y(transform, offset);
            applied.insert(format!("text.baseline_offsets_px.{key}"));
        }
    }

    if id.starts_with("visual_f3_flash_on_cut_") {
        if let Some(values) = profile.f3.get("flash_on_cuts") {
            apply_flash_tuning(layer, values, fps, applied)?;
        }
    }

    let effects = effects_mut(layer);
    apply_global_effect_tunings(effects, &profile.effects, applied)?;
    if let Some(style_id) = id.strip_prefix("bot_style_") {
        if let Some(values) = profile.styles.get(style_id) {
            apply_style_tuning(style_id, effects, values, applied)?;
        }
    }
    if id == "visual_f3_analog_glitch" {
        if let Some(values) = profile.f3.get("analog_glitch") {
            apply_analog_tuning(effects, values, applied)?;
        }
    }
    if id.starts_with("visual_f3_crystal_glow_") {
        if let Some(values) = profile.f3.get("crystal_glow") {
            apply_crystal_glow_tuning(effects, values, applied)?;
        }
    }
    Ok(())
}

fn apply_global_effect_tunings(
    effects: &mut [EffectSpec],
    tuning: &EffectTunables,
    applied: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    for effect in effects {
        match effect.match_name.as_str() {
            "ADBE Box Blur2" => {
                if multiply_first(
                    &mut effect.params,
                    &["radius", "Radius", "0001"],
                    tuning.blur.box_radius_multiplier,
                )? {
                    applied.insert("effects.blur.box_radius_multiplier".to_string());
                }
                if ensure_bool_default(
                    &mut effect.params,
                    "repeat_edge_pixels",
                    tuning.blur.repeat_edge_pixels_default,
                )? {
                    applied.insert("effects.blur.repeat_edge_pixels_default".to_string());
                }
            }
            "ADBE Gaussian Blur 2" => {
                if multiply_first(
                    &mut effect.params,
                    &[
                        "blurriness",
                        "Blurriness",
                        "blur_radius",
                        "blurRadius",
                        "radius",
                        "0001",
                        "ADBE Gaussian Blur 2-0001",
                    ],
                    tuning.blur.gaussian_sigma_multiplier,
                )? {
                    applied.insert("effects.blur.gaussian_sigma_multiplier".to_string());
                }
                if ensure_bool_default(
                    &mut effect.params,
                    "repeat_edge_pixels",
                    tuning.blur.repeat_edge_pixels_default,
                )? {
                    applied.insert("effects.blur.repeat_edge_pixels_default".to_string());
                }
            }
            "ADBE Glo2" => {
                if multiply_first(
                    &mut effect.params,
                    &["radius", "Radius", "0003"],
                    tuning.glow.radius_multiplier,
                )? {
                    applied.insert("effects.glow.radius_multiplier".to_string());
                }
                if multiply_first(
                    &mut effect.params,
                    &["intensity", "Intensity", "0004"],
                    tuning.glow.intensity_multiplier,
                )? {
                    applied.insert("effects.glow.intensity_multiplier".to_string());
                }
                if offset_first(
                    &mut effect.params,
                    &["threshold", "Threshold", "0002"],
                    tuning.glow.threshold_offset,
                )? {
                    applied.insert("effects.glow.threshold_offset".to_string());
                }
                if ensure_bool_default(
                    &mut effect.params,
                    "composite_original",
                    tuning.glow.composite_original_default,
                )? {
                    applied.insert("effects.glow.composite_original_default".to_string());
                }
            }
            "ADBE Drop Shadow" => {
                if multiply_first(
                    &mut effect.params,
                    &["softness", "Softness", "0005", "0051"],
                    tuning.shadow.softness_multiplier,
                )? {
                    applied.insert("effects.shadow.softness_multiplier".to_string());
                }
                if multiply_first(
                    &mut effect.params,
                    &["opacity", "Opacity", "0002", "0052"],
                    tuning.shadow.opacity_multiplier,
                )? {
                    applied.insert("effects.shadow.opacity_multiplier".to_string());
                }
                if multiply_first(
                    &mut effect.params,
                    &["distance", "Distance", "0004", "0053"],
                    tuning.shadow.distance_multiplier,
                )? {
                    applied.insert("effects.shadow.distance_multiplier".to_string());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_style_tuning(
    style_id: &str,
    effects: &mut [EffectSpec],
    values: &BTreeMap<String, f32>,
    applied: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    for (name, value) in values {
        let target = match name.as_str() {
            "gaussian_blur" => ("ADBE Gaussian Blur 2", "blurriness"),
            "glow_radius" => ("ADBE Glo2", "radius"),
            "glow_intensity" => ("ADBE Glo2", "intensity"),
            "geometry_rotation" => ("ADBE Geometry2", "rotation"),
            "scale_width" => ("ADBE Geometry2", "scale_width"),
            "scale_height" => ("ADBE Geometry2", "scale_height"),
            "motion_blur_length" => ("ADBE Motion Blur", "blur_length"),
            _ => bail!("unsupported tuning key styles.{style_id}.{name}"),
        };
        set_effect_number(effects, target.0, target.1, *value)?;
        applied.insert(format!("styles.{style_id}.{name}"));
    }
    Ok(())
}

fn apply_analog_tuning(
    effects: &mut [EffectSpec],
    values: &BTreeMap<String, f32>,
    applied: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    for (name, value) in values {
        let effect_key = match name.as_str() {
            "contrast" | "red_gain" | "wave_amplitude" | "wave_width" => name.as_str(),
            "grid_w" => "scanline_period",
            "grid_h" => "dot_period",
            _ => bail!("unsupported tuning key f3.analog_glitch.{name}"),
        };
        set_effect_number(effects, "ANR Analog Glitch", effect_key, *value)?;
        applied.insert(format!("f3.analog_glitch.{name}"));
    }
    Ok(())
}

fn apply_crystal_glow_tuning(
    effects: &mut [EffectSpec],
    values: &BTreeMap<String, f32>,
    applied: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    for (name, value) in values {
        match name.as_str() {
            "pre_blur" => set_effect_number(effects, "ADBE Gaussian Blur 2", "blurriness", *value)?,
            "glow_radius_a" => set_nth_effect_number(effects, "ADBE Glo2", 0, "radius", *value)?,
            "glow_radius_b" => set_nth_effect_number(effects, "ADBE Glo2", 1, "radius", *value)?,
            _ => bail!("unsupported tuning key f3.crystal_glow.{name}"),
        }
        applied.insert(format!("f3.crystal_glow.{name}"));
    }
    Ok(())
}

fn apply_flash_tuning(
    layer: &mut Layer,
    values: &BTreeMap<String, f32>,
    fps: f64,
    applied: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    let Layer::Solid {
        start,
        duration,
        transform,
        ..
    } = layer
    else {
        return Ok(());
    };
    for (name, value) in values {
        match name.as_str() {
            "opacity" => {
                let opacity = if *value <= 1.0 {
                    *value * 100.0
                } else {
                    *value
                };
                transform.opacity = opacity;
                if let Some(first) = transform.animation.opacity.first_mut() {
                    first.value = opacity;
                }
            }
            "fade_frames" => {
                let fade = f64::from(*value) / fps.max(1.0);
                if let Some(last) = transform.animation.opacity.last_mut() {
                    last.time = (*start + fade).min(*start + *duration);
                }
            }
            _ => bail!("unsupported tuning key f3.flash_on_cuts.{name}"),
        }
        applied.insert(format!("f3.flash_on_cuts.{name}"));
    }
    Ok(())
}

fn validate(profile: &NativeTunables) -> anyhow::Result<()> {
    if profile.schema != TUNING_SCHEMA {
        bail!(
            "unsupported tuning schema '{}'; expected '{}'",
            profile.schema,
            TUNING_SCHEMA
        );
    }
    for (name, value) in [
        (
            "effects.blur.box_radius_multiplier",
            profile.effects.blur.box_radius_multiplier,
        ),
        (
            "effects.blur.gaussian_sigma_multiplier",
            profile.effects.blur.gaussian_sigma_multiplier,
        ),
        (
            "effects.glow.radius_multiplier",
            profile.effects.glow.radius_multiplier,
        ),
        (
            "effects.glow.intensity_multiplier",
            profile.effects.glow.intensity_multiplier,
        ),
        (
            "effects.shadow.softness_multiplier",
            profile.effects.shadow.softness_multiplier,
        ),
        (
            "effects.shadow.opacity_multiplier",
            profile.effects.shadow.opacity_multiplier,
        ),
        (
            "effects.shadow.distance_multiplier",
            profile.effects.shadow.distance_multiplier,
        ),
    ] {
        if !value.is_finite() || value < 0.0 {
            bail!("{name} must be finite and non-negative, got {value}");
        }
    }
    validate_finite(
        "effects.glow.threshold_offset",
        profile.effects.glow.threshold_offset,
    )?;
    for (key, value) in &profile.text.baseline_offsets_px {
        validate_finite(&format!("text.baseline_offsets_px.{key}"), *value)?;
    }
    validate_non_negative(
        "text.source_rect.edge_tolerance_px",
        profile.text.source_rect.edge_tolerance_px,
    )?;
    validate_non_negative(
        "text.source_rect.wrap_epsilon_px",
        profile.text.source_rect.wrap_epsilon_px,
    )?;
    validate_unit_interval(
        "text.reveal.min_visible_scale",
        profile.text.reveal.min_visible_scale,
    )?;
    validate_unit_interval(
        "text.reveal.min_visible_alpha",
        profile.text.reveal.min_visible_alpha,
    )?;
    if profile.effects.sampling.resampling != "bilinear_premultiplied" {
        bail!("effects.sampling.resampling currently supports only bilinear_premultiplied");
    }
    if profile.effects.sampling.optics_edge != "transparent"
        || profile.effects.sampling.motion_blur_edge != "transparent"
    {
        bail!("optics_edge and motion_blur_edge currently support only transparent");
    }
    if profile.effects.alpha.boundary != "straight_rgba8"
        || profile.effects.alpha.internal != "premultiplied_float"
    {
        bail!("alpha model must remain straight_rgba8/premultiplied_float");
    }
    for key in profile.text.baseline_offsets_px.keys() {
        if !matches!(
            key.as_str(),
            "default" | "montserrat_bold_cyrillic" | "point_semibold_cyrillic"
        ) {
            bail!("unsupported tuning key text.baseline_offsets_px.{key}");
        }
    }
    for (style, values) in &profile.styles {
        if !matches!(
            style.as_str(),
            "txt_soft_v1" | "txt_punch_v1" | "txt_drop_v1" | "ftg_al16_default_v1"
        ) {
            bail!("unsupported tuning style id {style}");
        }
        for (key, value) in values {
            if !matches!(
                key.as_str(),
                "gaussian_blur"
                    | "glow_radius"
                    | "glow_intensity"
                    | "geometry_rotation"
                    | "scale_width"
                    | "scale_height"
                    | "motion_blur_length"
            ) {
                bail!("unsupported tuning key styles.{style}.{key}");
            }
            validate_finite(&format!("styles.{style}.{key}"), *value)?;
            if key != "geometry_rotation" {
                validate_non_negative(&format!("styles.{style}.{key}"), *value)?;
            }
        }
    }
    for (id, values) in &profile.f3 {
        if !matches!(
            id.as_str(),
            "flash_on_cuts" | "analog_glitch" | "crystal_glow"
        ) {
            bail!("unsupported tuning F3 id {id}");
        }
        for (key, value) in values {
            let supported = match id.as_str() {
                "flash_on_cuts" => matches!(key.as_str(), "opacity" | "fade_frames"),
                "analog_glitch" => matches!(
                    key.as_str(),
                    "contrast" | "red_gain" | "wave_amplitude" | "wave_width" | "grid_w" | "grid_h"
                ),
                "crystal_glow" => {
                    matches!(key.as_str(), "pre_blur" | "glow_radius_a" | "glow_radius_b")
                }
                _ => false,
            };
            if !supported {
                bail!("unsupported tuning key f3.{id}.{key}");
            }
            validate_finite(&format!("f3.{id}.{key}"), *value)?;
            if key != "wave_amplitude" {
                validate_non_negative(&format!("f3.{id}.{key}"), *value)?;
            }
        }
    }
    Ok(())
}

fn validate_finite(name: &str, value: f32) -> anyhow::Result<()> {
    if !value.is_finite() {
        bail!("{name} must be finite, got {value}");
    }
    Ok(())
}

fn validate_non_negative(name: &str, value: f32) -> anyhow::Result<()> {
    validate_finite(name, value)?;
    if value < 0.0 {
        bail!("{name} must be non-negative, got {value}");
    }
    Ok(())
}

fn validate_unit_interval(name: &str, value: f32) -> anyhow::Result<()> {
    validate_finite(name, value)?;
    if !(0.0..=1.0).contains(&value) {
        bail!("{name} must be in 0..=1, got {value}");
    }
    Ok(())
}

fn merge_json(base: &mut Value, overrides: &Value) -> anyhow::Result<()> {
    if overrides.is_null() {
        return Ok(());
    }
    let Some(overrides) = overrides.as_object() else {
        bail!("tuningSpec.overrides must be an object");
    };
    let Some(base) = base.as_object_mut() else {
        bail!("tuning profile root must be an object");
    };
    for (key, value) in overrides {
        if let (Some(existing), Some(_)) = (base.get_mut(key), value.as_object()) {
            merge_json(existing, value)?;
        } else {
            base.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

fn effects_mut(layer: &mut Layer) -> &mut Vec<EffectSpec> {
    match layer {
        Layer::Solid { effects, .. }
        | Layer::Footage { effects, .. }
        | Layer::Text { effects, .. }
        | Layer::Precomp { effects, .. }
        | Layer::Adjustment { effects, .. } => effects,
    }
}

fn baseline_key<'a>(
    font: &'a str,
    text: &str,
    char_styles: &[render_ir::TextCharStyle],
) -> &'a str {
    let has_cyrillic = text
        .chars()
        .any(|character| ('\u{0400}'..='\u{052f}').contains(&character));
    if !has_cyrillic {
        return "default";
    }
    let normalized = font.to_ascii_lowercase().replace(['-', ' ', '_'], "");
    let has_point = normalized.contains("pointsemibold")
        || char_styles.iter().any(|style| {
            style.font.as_deref().is_some_and(|font| {
                font.to_ascii_lowercase()
                    .replace(['-', ' ', '_'], "")
                    .contains("pointsemibold")
            })
        });
    if has_point {
        "point_semibold_cyrillic"
    } else if normalized.contains("montserratbold") {
        "montserrat_bold_cyrillic"
    } else {
        "default"
    }
}

fn offset_transform_y(transform: &mut Transform2D, offset: f32) {
    transform.position[1] += offset;
    for keyframe in &mut transform.animation.position {
        keyframe.value[1] += offset;
    }
}

fn set_effect_number(
    effects: &mut [EffectSpec],
    match_name: &str,
    key: &str,
    value: f32,
) -> anyhow::Result<()> {
    let effect = effects
        .iter_mut()
        .find(|effect| effect.match_name == match_name)
        .with_context(|| format!("tuning target effect {match_name} is missing"))?;
    object_mut(&mut effect.params)?.insert(key.to_string(), Value::from(value));
    Ok(())
}

fn set_nth_effect_number(
    effects: &mut [EffectSpec],
    match_name: &str,
    index: usize,
    key: &str,
    value: f32,
) -> anyhow::Result<()> {
    let effect = effects
        .iter_mut()
        .filter(|effect| effect.match_name == match_name)
        .nth(index)
        .with_context(|| format!("tuning target effect {match_name}[{index}] is missing"))?;
    object_mut(&mut effect.params)?.insert(key.to_string(), Value::from(value));
    Ok(())
}

fn multiply_first(params: &mut Value, names: &[&str], factor: f32) -> anyhow::Result<bool> {
    mutate_first_number(params, names, |value| value * f64::from(factor))
}

fn offset_first(params: &mut Value, names: &[&str], offset: f32) -> anyhow::Result<bool> {
    if offset.abs() <= f32::EPSILON {
        return Ok(false);
    }
    mutate_first_number(params, names, |value| value + f64::from(offset))
}

fn mutate_first_number(
    params: &mut Value,
    names: &[&str],
    mutate: impl Fn(f64) -> f64 + Copy,
) -> anyhow::Result<bool> {
    let object = object_mut(params)?;
    let Some(name) = names.iter().find(|name| object.contains_key(**name)) else {
        return Ok(false);
    };
    mutate_numeric_value(object.get_mut(*name).expect("key checked above"), mutate)?;
    Ok(true)
}

fn mutate_numeric_value(
    value: &mut Value,
    mutate: impl Fn(f64) -> f64 + Copy,
) -> anyhow::Result<()> {
    if let Some(number) = value.as_f64() {
        *value = Value::from(mutate(number));
        return Ok(());
    }
    let Some(object) = value.as_object_mut() else {
        bail!("tunable effect parameter must be numeric or an animated numeric object");
    };
    let mut changed = false;
    let direct_key = if object.contains_key("value") {
        Some("value")
    } else if object.contains_key("v") {
        Some("v")
    } else {
        None
    };
    if let Some(key) = direct_key {
        let raw = object.get_mut(key).expect("key checked above");
        let number = raw
            .as_f64()
            .context("animated tunable value must be numeric")?;
        *raw = Value::from(mutate(number));
        changed = true;
    }
    if let Some(keyframes) = object.get_mut("keyframes").and_then(Value::as_array_mut) {
        for keyframe in keyframes {
            let Some(keyframe) = keyframe.as_object_mut() else {
                bail!("tunable keyframe must be an object");
            };
            let key = if keyframe.contains_key("value") {
                "value"
            } else if keyframe.contains_key("v") {
                "v"
            } else {
                bail!("tunable keyframe is missing value/v");
            };
            let raw = keyframe.get_mut(key).expect("key checked above");
            let number = raw
                .as_f64()
                .context("tunable keyframe value must be numeric")?;
            *raw = Value::from(mutate(number));
            changed = true;
        }
    }
    if !changed {
        bail!("animated tunable value has neither a numeric value nor keyframes");
    }
    Ok(())
}

fn ensure_bool_default(params: &mut Value, key: &str, value: bool) -> anyhow::Result<bool> {
    let object = object_mut(params)?;
    if object.contains_key(key) {
        return Ok(false);
    }
    object.insert(key.to_string(), Value::Bool(value));
    Ok(true)
}

fn object_mut(value: &mut Value) -> anyhow::Result<&mut Map<String, Value>> {
    value
        .as_object_mut()
        .context("effect tuning requires object parameters")
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn one() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_resampling() -> String {
    "bilinear_premultiplied".to_string()
}

fn default_transparent() -> String {
    "transparent".to_string()
}

fn default_boundary() -> String {
    "straight_rgba8".to_string()
}

fn default_internal() -> String {
    "premultiplied_float".to_string()
}

fn default_matte_threshold() -> u8 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_ir::{BlendMode, Composition, MotionBlurSettings, Rect};
    use serde_json::json;

    fn scene_with_effects() -> Scene {
        Scene {
            version: "1".to_string(),
            composition: Composition {
                id: "main".to_string(),
                width: 64,
                height: 64,
                fps: 24.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers: vec![Layer::Solid {
                id: "probe".to_string(),
                start: 0.0,
                duration: 1.0,
                blend_mode: BlendMode::Normal,
                color: [255, 255, 255, 255],
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 64.0,
                    h: 64.0,
                },
                transform: Transform2D::default(),
                effects: vec![
                    EffectSpec {
                        match_name: "ADBE Box Blur2".to_string(),
                        params: json!({"radius": {"keyframes":[{"time":0.0,"value":2.0},{"time":1.0,"value":4.0}]}}),
                    },
                    EffectSpec {
                        match_name: "ADBE Glo2".to_string(),
                        params: json!({"threshold":100.0,"radius":10.0,"intensity":0.5}),
                    },
                ],
            }],
        }
    }

    #[test]
    fn built_in_profile_and_overrides_apply_without_recompile() {
        let spec = TuningSpec {
            profile: Some("builtin:p0p1-readiness".to_string()),
            overrides: json!({"effects":{"blur":{"box_radius_multiplier":2.0},"glow":{"radius_multiplier":3.0,"threshold_offset":5.0}}}),
        };
        let mut resolved = resolve(&spec, Path::new(".")).unwrap().unwrap();
        let mut scene = scene_with_effects();
        apply_to_scene(&mut scene, &mut resolved).unwrap();
        let Layer::Solid { effects, .. } = &scene.layers[0] else {
            panic!()
        };
        assert_eq!(
            effects[0].params["radius"]["keyframes"][0]["value"],
            json!(4.0)
        );
        assert_eq!(
            effects[0].params["radius"]["keyframes"][1]["value"],
            json!(8.0)
        );
        assert_eq!(effects[1].params["radius"], json!(30.0));
        assert_eq!(effects[1].params["threshold"], json!(105.0));
        assert!(resolved
            .report
            .applied
            .contains(&"effects.blur.box_radius_multiplier".to_string()));
    }

    #[test]
    fn unknown_override_is_rejected() {
        let spec = TuningSpec {
            profile: Some("builtin:p0p1-readiness".to_string()),
            overrides: json!({"effects":{"glow":{"mystery":1.0}}}),
        };
        let error = resolve(&spec, Path::new(".")).unwrap_err().to_string();
        assert!(error.contains("unknown field"), "{error}");
    }

    #[test]
    fn non_object_overrides_are_rejected_even_without_profile() {
        let error = resolve(
            &TuningSpec {
                profile: None,
                overrides: json!([]),
            },
            Path::new("."),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("tuningSpec.overrides must be an object"));
    }

    #[test]
    fn absent_tuning_spec_resolves_to_none() {
        assert!(resolve(&TuningSpec::default(), Path::new("."))
            .unwrap()
            .is_none());
    }
}
