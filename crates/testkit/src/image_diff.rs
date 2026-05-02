use image::{ImageBuffer, Rgba};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct DiffMetrics {
    pub max_abs_diff: u8,
    pub mean_abs_diff: f64,
    pub changed_pixels: u64,
    pub total_pixels: u64,
}

pub fn diff_rgba8(a: &[u8], b: &[u8]) -> DiffMetrics {
    assert_eq!(a.len(), b.len(), "image buffers must have same length");
    assert_eq!(a.len() % 4, 0, "RGBA buffers must be 4-byte aligned");
    let mut max_abs_diff = 0u8;
    let mut sum = 0u64;
    let mut changed_pixels = 0u64;
    for (a_pixel, b_pixel) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut pixel_changed = false;
        for (x, y) in a_pixel.iter().zip(b_pixel.iter()) {
            let d = x.abs_diff(*y);
            max_abs_diff = max_abs_diff.max(d);
            sum += d as u64;
            pixel_changed |= d > 0;
        }
        if pixel_changed {
            changed_pixels += 1;
        }
    }
    DiffMetrics {
        max_abs_diff,
        mean_abs_diff: sum as f64 / a.len().max(1) as f64,
        changed_pixels,
        total_pixels: (a.len() / 4) as u64,
    }
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
    let image: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(width, height, diff).ok_or_else(|| {
            anyhow::anyhow!("invalid diff image buffer for {width}x{height}")
        })?;
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
    }
}
