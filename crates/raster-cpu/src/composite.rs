use crate::Canvas;

/// AE GPUFoundation fast-path threshold observed in `GF::Composite`.
pub const AE_ALPHA_GAIN_EPSILON: f32 = 8.0e-6;

pub fn composite_normal_pixel(dst: [u8; 4], src: [u8; 4], opacity_percent: f32) -> [u8; 4] {
    let opacity = (opacity_percent / 100.0).clamp(0.0, 1.0);
    if opacity <= AE_ALPHA_GAIN_EPSILON {
        return dst;
    }

    let sa = (src[3] as f32 / 255.0) * opacity;
    if sa <= 0.0 {
        return dst;
    }

    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    let mut out = dst;
    for c in 0..3 {
        let s = src[c] as f32 / 255.0;
        let d = dst[c] as f32 / 255.0;
        let out_c = if out_a <= 0.0 {
            d
        } else {
            (s * sa + d * da * (1.0 - sa)) / out_a
        };
        out[c] = quantize_unit_float(out_c);
    }
    out[3] = quantize_unit_float(out_a);
    out
}

pub fn composite_normal(dst: &mut Canvas, src: &Canvas, opacity_percent: f32) {
    let w = dst.width.min(src.width);
    let h = dst.height.min(src.height);

    for y in 0..h {
        for x in 0..w {
            let i = ((y * dst.width + x) * 4) as usize;
            let j = ((y * src.width + x) * 4) as usize;
            let dst_pixel = [
                dst.data[i],
                dst.data[i + 1],
                dst.data[i + 2],
                dst.data[i + 3],
            ];
            let src_pixel = [
                src.data[j],
                src.data[j + 1],
                src.data[j + 2],
                src.data[j + 3],
            ];
            let out = composite_normal_pixel(dst_pixel, src_pixel, opacity_percent);
            dst.data[i..i + 4].copy_from_slice(&out);
        }
    }
}

fn quantize_unit_float(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_source_preserves_destination_rgb_under_zero_alpha() {
        let mut dst = Canvas::new(1, 1, [5, 5, 6, 0]);
        let src = Canvas::new(1, 1, [0, 0, 0, 0]);

        composite_normal(&mut dst, &src, 100.0);

        assert_eq!(dst.pixel(0, 0), [5, 5, 6, 0]);
    }

    #[test]
    fn zero_layer_opacity_preserves_destination() {
        let mut dst = Canvas::new(1, 1, [5, 5, 6, 0]);
        let src = Canvas::new(1, 1, [200, 100, 50, 255]);

        composite_normal(&mut dst, &src, 0.0);

        assert_eq!(dst.pixel(0, 0), [5, 5, 6, 0]);
    }

    #[test]
    fn epsilon_layer_opacity_preserves_destination() {
        let mut dst = Canvas::new(1, 1, [5, 5, 6, 0]);
        let src = Canvas::new(1, 1, [200, 100, 50, 255]);

        composite_normal(&mut dst, &src, AE_ALPHA_GAIN_EPSILON * 100.0);

        assert_eq!(dst.pixel(0, 0), [5, 5, 6, 0]);
    }

    #[test]
    fn partial_alpha_matches_reversed_source_over_formula() {
        let out = composite_normal_pixel([10, 20, 30, 128], [200, 40, 80, 64], 50.0);

        assert_eq!(out, [52, 24, 41, 144]);
    }

    #[test]
    fn layer_opacity_scales_source_alpha_before_source_over() {
        let out = composite_normal_pixel([20, 40, 80, 128], [220, 80, 20, 128], 35.0);

        assert_eq!(out, [80, 52, 62, 150]);
    }

    #[test]
    fn transparent_destination_keeps_source_rgb_and_scaled_alpha() {
        let out = composite_normal_pixel([5, 5, 6, 0], [250, 0, 0, 128], 50.0);

        assert_eq!(out, [250, 0, 0, 64]);
    }
}
