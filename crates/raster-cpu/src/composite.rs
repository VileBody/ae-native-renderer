use crate::Canvas;
use rayon::prelude::*;

/// AE GPUFoundation fast-path threshold observed in `GF::Composite`.
pub const AE_ALPHA_GAIN_EPSILON: f32 = 8.0e-6;
const PARALLEL_COMPOSITE_MIN_PIXELS: usize = 8_000_000;

pub fn composite_normal_pixel(dst: [u8; 4], src: [u8; 4], opacity_percent: f32) -> [u8; 4] {
    let opacity = (opacity_percent / 100.0).clamp(0.0, 1.0);
    composite_normal_pixel_with_opacity(dst, src, opacity)
}

fn composite_normal_pixel_with_opacity(dst: [u8; 4], src: [u8; 4], opacity: f32) -> [u8; 4] {
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
    let width = dst.width.min(src.width) as usize;
    let height = dst.height.min(src.height) as usize;
    if width == 0 || height == 0 {
        return;
    }

    let opacity = (opacity_percent / 100.0).clamp(0.0, 1.0);
    if opacity <= AE_ALPHA_GAIN_EPSILON {
        return;
    }

    let dst_stride = dst.width as usize * 4;
    let src_stride = src.width as usize * 4;
    let blend_bytes = width * 4;
    let dst_rows = &mut dst.data[..dst_stride * height];
    let src_rows = &src.data[..src_stride * height];

    if width * height >= PARALLEL_COMPOSITE_MIN_PIXELS {
        dst_rows
            .par_chunks_exact_mut(dst_stride)
            .zip(src_rows.par_chunks_exact(src_stride))
            .for_each(|(dst_row, src_row)| {
                composite_normal_row(dst_row, src_row, blend_bytes, opacity)
            });
    } else {
        for (dst_row, src_row) in dst_rows
            .chunks_exact_mut(dst_stride)
            .zip(src_rows.chunks_exact(src_stride))
        {
            composite_normal_row(dst_row, src_row, blend_bytes, opacity);
        }
    }
}

fn composite_normal_row(dst: &mut [u8], src: &[u8], blend_bytes: usize, opacity: f32) {
    for (dst_pixel, src_pixel) in dst[..blend_bytes]
        .chunks_exact_mut(4)
        .zip(src[..blend_bytes].chunks_exact(4))
    {
        let dst_rgba = [dst_pixel[0], dst_pixel[1], dst_pixel[2], dst_pixel[3]];
        let src_rgba = [src_pixel[0], src_pixel[1], src_pixel[2], src_pixel[3]];
        dst_pixel.copy_from_slice(&composite_normal_pixel_with_opacity(
            dst_rgba, src_rgba, opacity,
        ));
    }
}

fn quantize_unit_float(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterned_canvas(width: u32, height: u32, seed: u8) -> Canvas {
        let mut canvas = Canvas::transparent(width, height);
        for (index, pixel) in canvas.data.chunks_exact_mut(4).enumerate() {
            let index = index as u32;
            pixel.copy_from_slice(&[
                seed.wrapping_add((index * 17) as u8),
                seed.wrapping_mul(3).wrapping_add((index * 29) as u8),
                seed.wrapping_mul(7).wrapping_add((index * 43) as u8),
                seed.wrapping_add((index * 61) as u8),
            ]);
        }
        canvas
    }

    fn composite_normal_serial(dst: &mut Canvas, src: &Canvas, opacity_percent: f32) {
        let width = dst.width.min(src.width);
        let height = dst.height.min(src.height);
        for y in 0..height {
            for x in 0..width {
                let dst_pixel = dst.pixel(x, y);
                let src_pixel = src.pixel(x, y);
                dst.set_pixel(
                    x,
                    y,
                    composite_normal_pixel(dst_pixel, src_pixel, opacity_percent),
                );
            }
        }
    }

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

    #[test]
    fn full_frame_composite_matches_serial_reference_and_preserves_uncovered_pixels() {
        let mut expected = patterned_canvas(401, 263, 11);
        let mut actual = expected.clone();
        let source = patterned_canvas(389, 251, 173);
        let uncovered_pixel = expected.pixel(400, 262);

        composite_normal_serial(&mut expected, &source, 37.5);
        composite_normal(&mut actual, &source, 37.5);

        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.pixel(400, 262), uncovered_pixel);
    }

    #[test]
    fn composite_parallelism_starts_at_4k_class_sizes() {
        assert!(1920 * 1080 < PARALLEL_COMPOSITE_MIN_PIXELS);
        assert!(3840 * 2160 >= PARALLEL_COMPOSITE_MIN_PIXELS);
    }
}
