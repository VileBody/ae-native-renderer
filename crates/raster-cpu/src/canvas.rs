use image::{ImageBuffer, Rgba};
use std::path::Path;

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
        for px in data.chunks_exact_mut(4) {
            px.copy_from_slice(&color);
        }
        Self { width, height, data }
    }

    pub fn transparent(width: u32, height: u32) -> Self {
        Self::new(width, height, [0, 0, 0, 0])
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
        let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(self.width, self.height, self.data.clone())
            .ok_or_else(|| anyhow::anyhow!("invalid canvas buffer"))?;
        img.save(path)?;
        Ok(())
    }
}
