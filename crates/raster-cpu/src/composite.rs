use crate::Canvas;

pub fn composite_normal(dst: &mut Canvas, src: &Canvas, opacity_percent: f32) {
    let opacity = (opacity_percent / 100.0).clamp(0.0, 1.0);
    let w = dst.width.min(src.width);
    let h = dst.height.min(src.height);

    for y in 0..h {
        for x in 0..w {
            let i = ((y * dst.width + x) * 4) as usize;
            let j = ((y * src.width + x) * 4) as usize;
            let sa = (src.data[j + 3] as f32 / 255.0) * opacity;
            if sa <= 0.0 {
                continue;
            }
            let da = dst.data[i + 3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);

            for c in 0..3 {
                let s = src.data[j + c] as f32 / 255.0;
                let d = dst.data[i + c] as f32 / 255.0;
                let out = if out_a <= 0.0 {
                    d
                } else {
                    (s * sa + d * da * (1.0 - sa)) / out_a
                };
                dst.data[i + c] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            dst.data[i + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
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
}
