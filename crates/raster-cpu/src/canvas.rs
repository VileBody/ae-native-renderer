use image::{ColorType, ImageFormat};
use rayon::prelude::*;
use std::path::Path;

const PARALLEL_FILL_MIN_PIXELS: usize = 8_000_000;

#[derive(Debug, Clone)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    /// v0 storage: straight RGBA8. Target: RGBA f32 premultiplied.
    pub data: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32, color: [u8; 4]) -> Self {
        let mut data = vec![0; (width * height * 4) as usize];
        if color != [0, 0, 0, 0] {
            fill_rgba(&mut data, color);
        }
        Self {
            width,
            height,
            data,
        }
    }

    pub fn transparent(width: u32, height: u32) -> Self {
        Self::new(width, height, [0, 0, 0, 0])
    }

    pub fn resize_and_clear(&mut self, width: u32, height: u32, color: [u8; 4]) {
        let len = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        self.width = width;
        self.height = height;
        self.data.resize(len, 0);
        fill_rgba(&mut self.data, color);
    }

    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> anyhow::Result<Self> {
        let expected = (width * height * 4) as usize;
        if data.len() != expected {
            anyhow::bail!(
                "invalid RGBA buffer size: got {}, expected {}",
                data.len(),
                expected
            );
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = ((y * self.width + x) * 4) as usize;
        self.data[idx..idx + 4].copy_from_slice(&rgba);
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let idx = ((y * self.width + x) * 4) as usize;
        [
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]
    }

    pub fn save_png(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        image::save_buffer_with_format(
            path,
            &self.data,
            self.width,
            self.height,
            ColorType::Rgba8,
            ImageFormat::Png,
        )?;
        Ok(())
    }
}

fn fill_rgba(data: &mut [u8], color: [u8; 4]) {
    if data.len() / 4 >= PARALLEL_FILL_MIN_PIXELS {
        data.par_chunks_exact_mut(4)
            .for_each(|pixel| pixel.copy_from_slice(&color));
    } else if color == [0, 0, 0, 0] {
        data.fill(0);
    } else {
        for pixel in data.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_and_clear_reuses_storage_and_sets_dimensions() {
        let mut canvas = Canvas::transparent(8, 8);
        let capacity = canvas.data.capacity();

        canvas.resize_and_clear(4, 4, [1, 2, 3, 4]);

        assert_eq!((canvas.width, canvas.height), (4, 4));
        assert_eq!(canvas.data.len(), 4 * 4 * 4);
        assert!(canvas.data.capacity() >= capacity);
        assert!(canvas
            .data
            .chunks_exact(4)
            .all(|pixel| pixel == [1, 2, 3, 4]));
    }

    #[test]
    fn large_canvas_fill_is_pixel_exact() {
        let color = [17, 91, 203, 147];
        let mut canvas = Canvas::new(384, 192, color);

        assert_eq!(canvas.data.len(), 384 * 192 * 4);
        assert!(canvas.data.chunks_exact(4).all(|pixel| pixel == color));

        canvas.resize_and_clear(512, 160, [0, 0, 0, 0]);
        assert_eq!(canvas.data.len(), 512 * 160 * 4);
        assert!(canvas.data.iter().all(|&channel| channel == 0));
    }

    #[test]
    fn fill_parallelism_starts_at_4k_class_sizes() {
        assert!(1920 * 1080 < PARALLEL_FILL_MIN_PIXELS);
        assert!(3840 * 2160 >= PARALLEL_FILL_MIN_PIXELS);
    }
}
