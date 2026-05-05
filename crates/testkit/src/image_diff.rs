use image::{ImageBuffer, Rgba};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DiffMetrics {
    pub max_abs_diff: u8,
    pub mean_abs_diff: f64,
    pub rmse_abs_diff: f64,
    pub changed_pixels: u64,
    pub total_pixels: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SplitDiffMetrics {
    pub rgba: DiffMetrics,
    pub rgb: DiffMetrics,
    pub alpha: DiffMetrics,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RgbAlphaMetricFlags {
    pub raw_rgb_ignores_alpha: bool,
    pub alpha_reported_separately: bool,
    pub rgb_under_alpha_policy_uses_source_over: bool,
    pub rgb_under_alpha_policy_uses_reference_background: bool,
    pub premult_unpremultiply_applied: bool,
    pub premult_contract_locked: bool,
    pub diagnostic_only: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RgbAlphaMetricPolicy {
    pub schema: &'static str,
    pub raw_rgba: &'static str,
    pub rgb_error: &'static str,
    pub rgb_error_under_alpha_policy: &'static str,
    pub alpha_error: &'static str,
    pub background_alpha_normalization: &'static str,
    pub premult_handling: &'static str,
    pub flags: RgbAlphaMetricFlags,
}

pub const RGB_ALPHA_METRIC_POLICY: RgbAlphaMetricPolicy = RgbAlphaMetricPolicy {
    schema: "m19.rgb_alpha_metric_policy.v1",
    raw_rgba: "absolute RGBA8 channel diff; compatibility gate only",
    rgb_error: "raw RGB channel diff with alpha ignored",
    rgb_error_under_alpha_policy:
        "straight RGBA8 source-over projection onto the AE/reference background RGB",
    alpha_error: "raw alpha channel diff reported separately from RGB",
    background_alpha_normalization:
        "ignore alpha only where native and AE pixels both match their detected background RGB",
    premult_handling:
        "locked RGBA8 export contract: straight pixels at native boundaries; normal source-over premultiplies internally and unpremultiplies before RGBA8 quantization",
    flags: RgbAlphaMetricFlags {
        raw_rgb_ignores_alpha: true,
        alpha_reported_separately: true,
        rgb_under_alpha_policy_uses_source_over: true,
        rgb_under_alpha_policy_uses_reference_background: true,
        premult_unpremultiply_applied: false,
        premult_contract_locked: true,
        diagnostic_only: false,
    },
};

pub const M19_ALPHA_COMPOSITE_GATE_CASES: &[&str] = &["PRI_010", "CMP_010", "STK_010", "STK_020"];

#[derive(Debug, Clone, Copy, Serialize)]
pub struct M19AlphaCompositeGatePolicy {
    pub schema: &'static str,
    pub required_cases: &'static [&'static str],
    pub primary_visible_metric: &'static str,
    pub alpha_metric: &'static str,
    pub compatibility_metric: &'static str,
    pub background_metric: &'static str,
    pub raw_rgba_tuning_status: &'static str,
    pub premult_status: &'static str,
    pub reverse_readiness_scope: &'static str,
}

pub const M19_ALPHA_COMPOSITE_GATE_POLICY: M19AlphaCompositeGatePolicy =
    M19AlphaCompositeGatePolicy {
        schema: "m19.alpha_composite_gate.v1",
        required_cases: M19_ALPHA_COMPOSITE_GATE_CASES,
        primary_visible_metric: "rgb_straight_source_over_ae_background",
        alpha_metric: "alpha",
        compatibility_metric: "rgba",
        background_metric: "background_alpha_normalized",
        raw_rgba_tuning_status: "compatibility_only_not_effect_tuning",
        premult_status: "locked_rgba8_normal_source_over_contract",
        reverse_readiness_scope: "M19 is reverse implemented for RGBA8 normal layer source-over, transparent background RGB, layer opacity as source alpha gain, and PNG/TIFF golden comparison metrics; effect-local premultiply wrappers remain owned by their effect modules.",
    };

pub fn is_m19_alpha_composite_gate_case(case_id: &str) -> bool {
    M19_ALPHA_COMPOSITE_GATE_CASES.contains(&case_id)
}

pub fn m19_alpha_composite_gate_role(case_id: &str) -> Option<&'static str> {
    match case_id {
        "PRI_010" => Some("primitive_source_alpha_layer_opacity_background_rgb"),
        "CMP_010" => Some("premult_probe_source_over_projection_sampling_audit"),
        "STK_010" => Some("drop_shadow_stack_alpha_composite_regression"),
        "STK_020" => Some("blur_minimax_stack_alpha_order_regression"),
        _ => None,
    }
}

pub fn diff_rgba8(a: &[u8], b: &[u8]) -> DiffMetrics {
    diff_rgba8_channels(a, b, &[0, 1, 2, 3])
}

pub fn diff_rgb8(a: &[u8], b: &[u8]) -> DiffMetrics {
    diff_rgba8_channels(a, b, &[0, 1, 2])
}

pub fn diff_alpha8(a: &[u8], b: &[u8]) -> DiffMetrics {
    diff_rgba8_channels(a, b, &[3])
}

pub fn diff_rgba8_split(a: &[u8], b: &[u8]) -> SplitDiffMetrics {
    SplitDiffMetrics {
        rgba: diff_rgba8(a, b),
        rgb: diff_rgb8(a, b),
        alpha: diff_alpha8(a, b),
    }
}

pub fn diff_rgba8_background_alpha_normalized(
    a: &[u8],
    b: &[u8],
    a_background_rgb: [u8; 3],
    b_background_rgb: [u8; 3],
) -> DiffMetrics {
    assert_eq!(a.len(), b.len(), "image buffers must have same length");
    assert_eq!(a.len() % 4, 0, "RGBA buffers must be 4-byte aligned");
    let mut max_abs_diff = 0u8;
    let mut sum = 0u64;
    let mut sum_sq = 0u64;
    let mut changed_pixels = 0u64;
    for (a_pixel, b_pixel) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let is_background_pair =
            a_pixel[..3] == a_background_rgb && b_pixel[..3] == b_background_rgb;
        let mut pixel_changed = false;
        for channel in 0..4 {
            let d = if channel == 3 && is_background_pair {
                0
            } else {
                a_pixel[channel].abs_diff(b_pixel[channel])
            };
            max_abs_diff = max_abs_diff.max(d);
            sum += d as u64;
            sum_sq += (d as u64) * (d as u64);
            pixel_changed |= d > 0;
        }
        if pixel_changed {
            changed_pixels += 1;
        }
    }
    let components = a.len().max(1) as f64;
    DiffMetrics {
        max_abs_diff,
        mean_abs_diff: sum as f64 / components,
        rmse_abs_diff: (sum_sq as f64 / components).sqrt(),
        changed_pixels,
        total_pixels: (a.len() / 4) as u64,
    }
}

pub fn diff_rgb8_foreground_masked(
    a: &[u8],
    b: &[u8],
    a_background_rgb: [u8; 3],
    b_background_rgb: [u8; 3],
) -> DiffMetrics {
    assert_eq!(a.len(), b.len(), "image buffers must have same length");
    assert_eq!(a.len() % 4, 0, "RGBA buffers must be 4-byte aligned");
    let mut max_abs_diff = 0u8;
    let mut sum = 0u64;
    let mut sum_sq = 0u64;
    let mut changed_pixels = 0u64;
    let mut compared_pixels = 0u64;
    for (a_pixel, b_pixel) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        if !is_foreground_pixel(a_pixel, a_background_rgb)
            && !is_foreground_pixel(b_pixel, b_background_rgb)
        {
            continue;
        }

        compared_pixels += 1;
        let mut pixel_changed = false;
        for channel in 0..3 {
            let d = a_pixel[channel].abs_diff(b_pixel[channel]);
            max_abs_diff = max_abs_diff.max(d);
            sum += d as u64;
            sum_sq += (d as u64) * (d as u64);
            pixel_changed |= d > 0;
        }
        if pixel_changed {
            changed_pixels += 1;
        }
    }
    let components = (compared_pixels * 3).max(1) as f64;
    DiffMetrics {
        max_abs_diff,
        mean_abs_diff: sum as f64 / components,
        rmse_abs_diff: (sum_sq as f64 / components).sqrt(),
        changed_pixels,
        total_pixels: compared_pixels,
    }
}

pub fn diff_rgb8_over_background(a: &[u8], b: &[u8], background_rgb: [u8; 3]) -> DiffMetrics {
    assert_eq!(a.len(), b.len(), "image buffers must have same length");
    assert_eq!(a.len() % 4, 0, "RGBA buffers must be 4-byte aligned");
    let mut max_abs_diff = 0u8;
    let mut sum = 0u64;
    let mut sum_sq = 0u64;
    let mut changed_pixels = 0u64;
    for (a_pixel, b_pixel) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut pixel_changed = false;
        for channel in 0..3 {
            let a_composited = composite_channel_over_background(
                a_pixel[channel],
                a_pixel[3],
                background_rgb[channel],
            );
            let b_composited = composite_channel_over_background(
                b_pixel[channel],
                b_pixel[3],
                background_rgb[channel],
            );
            let d = a_composited.abs_diff(b_composited);
            max_abs_diff = max_abs_diff.max(d);
            sum += d as u64;
            sum_sq += (d as u64) * (d as u64);
            pixel_changed |= d > 0;
        }
        if pixel_changed {
            changed_pixels += 1;
        }
    }
    let components = ((a.len() / 4) * 3).max(1) as f64;
    DiffMetrics {
        max_abs_diff,
        mean_abs_diff: sum as f64 / components,
        rmse_abs_diff: (sum_sq as f64 / components).sqrt(),
        changed_pixels,
        total_pixels: (a.len() / 4) as u64,
    }
}

pub fn diff_rgb8_under_alpha_policy(a: &[u8], b: &[u8], background_rgb: [u8; 3]) -> DiffMetrics {
    diff_rgb8_over_background(a, b, background_rgb)
}

fn diff_rgba8_channels(a: &[u8], b: &[u8], channels: &[usize]) -> DiffMetrics {
    assert_eq!(a.len(), b.len(), "image buffers must have same length");
    assert_eq!(a.len() % 4, 0, "RGBA buffers must be 4-byte aligned");
    let mut max_abs_diff = 0u8;
    let mut sum = 0u64;
    let mut sum_sq = 0u64;
    let mut changed_pixels = 0u64;
    for (a_pixel, b_pixel) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut pixel_changed = false;
        for channel in channels {
            let d = a_pixel[*channel].abs_diff(b_pixel[*channel]);
            max_abs_diff = max_abs_diff.max(d);
            sum += d as u64;
            sum_sq += (d as u64) * (d as u64);
            pixel_changed |= d > 0;
        }
        if pixel_changed {
            changed_pixels += 1;
        }
    }
    let components = ((a.len() / 4) * channels.len()).max(1) as f64;
    DiffMetrics {
        max_abs_diff,
        mean_abs_diff: sum as f64 / components,
        rmse_abs_diff: (sum_sq as f64 / components).sqrt(),
        changed_pixels,
        total_pixels: (a.len() / 4) as u64,
    }
}

fn is_foreground_pixel(pixel: &[u8], background_rgb: [u8; 3]) -> bool {
    pixel[3] > 0 && [pixel[0], pixel[1], pixel[2]] != background_rgb
}

fn composite_channel_over_background(src: u8, alpha: u8, background: u8) -> u8 {
    let alpha = alpha as u32;
    let inv_alpha = 255 - alpha;
    (((src as u32 * alpha) + (background as u32 * inv_alpha) + 127) / 255) as u8
}

#[derive(Debug, Clone)]
pub struct RgbaImageData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub fn load_rgba_png(path: impl AsRef<Path>) -> anyhow::Result<RgbaImageData> {
    let image = image::open(path.as_ref())?.to_rgba8();
    Ok(RgbaImageData {
        width: image.width(),
        height: image.height(),
        data: image.into_raw(),
    })
}

pub fn write_diff_png(
    a: &[u8],
    b: &[u8],
    width: u32,
    height: u32,
    path: impl AsRef<Path>,
) -> anyhow::Result<DiffMetrics> {
    let expected = (width * height * 4) as usize;
    anyhow::ensure!(
        a.len() == expected && b.len() == expected,
        "RGBA buffer dimensions do not match {width}x{height}"
    );
    let metrics = diff_rgba8(a, b);
    let mut diff = vec![0_u8; expected];
    for ((out, a_pixel), b_pixel) in diff
        .chunks_exact_mut(4)
        .zip(a.chunks_exact(4))
        .zip(b.chunks_exact(4))
    {
        let r = amplified_abs_diff(a_pixel[0], b_pixel[0]);
        let g = amplified_abs_diff(a_pixel[1], b_pixel[1]);
        let b = amplified_abs_diff(a_pixel[2], b_pixel[2]);
        let alpha = amplified_abs_diff(a_pixel[3], b_pixel[3]);
        out.copy_from_slice(&[r.max(alpha), g, b.max(alpha), 255]);
    }
    let image: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, diff)
        .ok_or_else(|| anyhow::anyhow!("invalid diff image buffer for {width}x{height}"))?;
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    Ok(metrics)
}

fn amplified_abs_diff(a: u8, b: u8) -> u8 {
    a.abs_diff(b).saturating_mul(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_pixels_are_counted_per_pixel() {
        let a = [0, 0, 0, 255, 10, 10, 10, 255];
        let b = [0, 0, 0, 255, 20, 10, 30, 255];

        let metrics = diff_rgba8(&a, &b);

        assert_eq!(metrics.total_pixels, 2);
        assert_eq!(metrics.changed_pixels, 1);
        assert_eq!(metrics.max_abs_diff, 20);
        assert!(metrics.rmse_abs_diff > metrics.mean_abs_diff);
    }

    #[test]
    fn rgb_and_alpha_metrics_are_split() {
        let native = [5, 5, 6, 255, 20, 30, 40, 128];
        let ae = [5, 5, 6, 0, 20, 31, 40, 128];

        let metrics = diff_rgba8_split(&native, &ae);

        assert_eq!(metrics.rgba.max_abs_diff, 255);
        assert_eq!(metrics.rgb.max_abs_diff, 1);
        assert_eq!(metrics.alpha.max_abs_diff, 255);
        assert_eq!(metrics.rgb.changed_pixels, 1);
        assert_eq!(metrics.alpha.changed_pixels, 1);
    }

    #[test]
    fn background_alpha_normalization_ignores_matching_corner_rgb_alpha_only_diff() {
        let native = [5, 5, 6, 255, 20, 30, 40, 128];
        let ae = [5, 5, 6, 0, 20, 30, 40, 128];

        let raw = diff_rgba8(&native, &ae);
        let normalized = diff_rgba8_background_alpha_normalized(&native, &ae, [5, 5, 6], [5, 5, 6]);

        assert_eq!(raw.max_abs_diff, 255);
        assert_eq!(normalized.max_abs_diff, 0);
        assert_eq!(normalized.changed_pixels, 0);
    }

    #[test]
    fn foreground_masked_rgb_ignores_matching_background_rgb_alpha_only_diff() {
        let native = [5, 5, 6, 255, 5, 5, 6, 255, 5, 5, 6, 255];
        let ae = [5, 5, 6, 0, 5, 5, 6, 0, 5, 5, 6, 0];

        let raw = diff_rgba8(&native, &ae);
        let foreground = diff_rgb8_foreground_masked(&native, &ae, [5, 5, 6], [5, 5, 6]);

        assert_eq!(raw.max_abs_diff, 255);
        assert_eq!(foreground.max_abs_diff, 0);
        assert_eq!(foreground.mean_abs_diff, 0.0);
        assert_eq!(foreground.changed_pixels, 0);
        assert_eq!(foreground.total_pixels, 0);
    }

    #[test]
    fn foreground_masked_rgb_detects_foreground_rgb_diff() {
        let native = [5, 5, 6, 255, 100, 20, 30, 255, 5, 5, 6, 255];
        let ae = [5, 5, 6, 0, 150, 20, 30, 255, 5, 5, 6, 0];

        let foreground = diff_rgb8_foreground_masked(&native, &ae, [5, 5, 6], [5, 5, 6]);

        assert_eq!(foreground.max_abs_diff, 50);
        assert_eq!(foreground.changed_pixels, 1);
        assert_eq!(foreground.total_pixels, 1);
        assert!((foreground.mean_abs_diff - (50.0 / 3.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn rgb_over_background_ignores_invisible_rgb_and_background_alpha_diff() {
        let native = [255, 0, 0, 0, 5, 5, 6, 255];
        let ae = [0, 0, 255, 0, 5, 5, 6, 0];

        let raw_rgb = diff_rgb8(&native, &ae);
        let raw_rgba = diff_rgba8(&native, &ae);
        let over_background = diff_rgb8_over_background(&native, &ae, [5, 5, 6]);

        assert_eq!(raw_rgb.max_abs_diff, 255);
        assert_eq!(raw_rgba.changed_pixels, 2);
        assert_eq!(over_background.max_abs_diff, 0);
        assert_eq!(over_background.mean_abs_diff, 0.0);
        assert_eq!(over_background.rmse_abs_diff, 0.0);
        assert_eq!(over_background.changed_pixels, 0);
        assert_eq!(over_background.total_pixels, 2);
    }

    #[test]
    fn rgb_over_background_catches_premultiplied_color_at_partial_alpha() {
        let straight = [255, 0, 0, 128];
        let premultiplied_looking = [128, 0, 0, 128];

        let over_black = diff_rgb8_over_background(&straight, &premultiplied_looking, [0, 0, 0]);

        assert_eq!(over_black.max_abs_diff, 64);
        assert_eq!(over_black.changed_pixels, 1);
        assert_eq!(over_black.total_pixels, 1);
        assert!((over_black.mean_abs_diff - (64.0 / 3.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn rgb_under_alpha_policy_uses_reference_background_projection() {
        let native = [255, 0, 0, 0, 255, 0, 0, 128];
        let ae = [0, 0, 255, 0, 128, 0, 0, 128];

        let metrics = diff_rgb8_under_alpha_policy(&native, &ae, [0, 0, 0]);

        assert_eq!(metrics.max_abs_diff, 64);
        assert_eq!(metrics.changed_pixels, 1);
        assert_eq!(metrics.total_pixels, 2);
    }

    #[test]
    fn rgb_alpha_metric_policy_flags_are_explicit() {
        assert!(RGB_ALPHA_METRIC_POLICY.flags.alpha_reported_separately);
        assert!(
            RGB_ALPHA_METRIC_POLICY
                .flags
                .rgb_under_alpha_policy_uses_reference_background
        );
        assert!(!RGB_ALPHA_METRIC_POLICY.flags.premult_unpremultiply_applied);
        assert!(RGB_ALPHA_METRIC_POLICY.flags.premult_contract_locked);
        assert!(!RGB_ALPHA_METRIC_POLICY.flags.diagnostic_only);
    }

    #[test]
    fn m19_alpha_composite_gate_policy_names_required_cases_and_metrics() {
        assert_eq!(
            M19_ALPHA_COMPOSITE_GATE_POLICY.required_cases,
            ["PRI_010", "CMP_010", "STK_010", "STK_020"]
        );
        assert_eq!(
            M19_ALPHA_COMPOSITE_GATE_POLICY.primary_visible_metric,
            "rgb_straight_source_over_ae_background"
        );
        assert_eq!(
            M19_ALPHA_COMPOSITE_GATE_POLICY.raw_rgba_tuning_status,
            "compatibility_only_not_effect_tuning"
        );
        assert_eq!(
            M19_ALPHA_COMPOSITE_GATE_POLICY.premult_status,
            "locked_rgba8_normal_source_over_contract"
        );
        assert!(is_m19_alpha_composite_gate_case("CMP_010"));
        assert_eq!(
            m19_alpha_composite_gate_role("STK_020"),
            Some("blur_minimax_stack_alpha_order_regression")
        );
        assert_eq!(m19_alpha_composite_gate_role("EFF_010"), None);
    }
}
