#[derive(Debug, Clone, Copy)]
pub struct DiffMetrics {
    pub max_abs_diff: u8,
    pub mean_abs_diff: f64,
    pub changed_pixels: u64,
}

pub fn diff_rgba8(a: &[u8], b: &[u8]) -> DiffMetrics {
    assert_eq!(a.len(), b.len(), "image buffers must have same length");
    let mut max_abs_diff = 0u8;
    let mut sum = 0u64;
    let mut changed_pixels = 0u64;
    for (x, y) in a.iter().zip(b.iter()) {
        let d = x.abs_diff(*y);
        max_abs_diff = max_abs_diff.max(d);
        sum += d as u64;
        if d > 0 {
            changed_pixels += 1;
        }
    }
    DiffMetrics {
        max_abs_diff,
        mean_abs_diff: sum as f64 / a.len().max(1) as f64,
        changed_pixels,
    }
}
