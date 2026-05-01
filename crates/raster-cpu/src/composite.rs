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
            let da = dst.data[i + 3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);

            for c in 0..3 {
                let s = src.data[j + c] as f32 / 255.0;
                let d = dst.data[i + c] as f32 / 255.0;
                let out = if out_a <= 0.0 {
                    0.0
                } else {
                    (s * sa + d * da * (1.0 - sa)) / out_a
                };
                dst.data[i + c] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            dst.data[i + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}
