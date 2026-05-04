use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;

pub fn save_rgba8(image: &RgbaImage, path: impl AsRef<Path>) -> anyhow::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    Ok(())
}

pub fn solid_rgba8(width: u32, height: u32, rgba: [u8; 4]) -> RgbaImage {
    ImageBuffer::from_pixel(width, height, Rgba(rgba))
}

pub fn transparent_rgba8(width: u32, height: u32) -> RgbaImage {
    solid_rgba8(width, height, [0, 0, 0, 0])
}

pub fn impulse_rgba8(width: u32, height: u32, x: u32, y: u32, rgba: [u8; 4]) -> RgbaImage {
    let mut image = transparent_rgba8(width, height);
    if x < width && y < height {
        image.put_pixel(x, y, Rgba(rgba));
    }
    image
}

pub fn centered_impulse_rgba8(width: u32, height: u32) -> RgbaImage {
    impulse_rgba8(width, height, width / 2, height / 2, [255, 255, 255, 255])
}

pub fn horizontal_ramp_rgba8(width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let denominator = width.saturating_sub(1).max(1) as f32;
    for y in 0..height {
        for x in 0..width {
            let value = ((x as f32 / denominator) * 255.0).round() as u8;
            image.put_pixel(x, y, Rgba([value, value, value, 255]));
        }
    }
    image
}

pub fn vertical_ramp_rgba8(width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let denominator = height.saturating_sub(1).max(1) as f32;
    for y in 0..height {
        let value = ((y as f32 / denominator) * 255.0).round() as u8;
        for x in 0..width {
            image.put_pixel(x, y, Rgba([value, value, value, 255]));
        }
    }
    image
}

pub fn ramp_rgba8(width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let x_denominator = width.saturating_sub(1).max(1) as f32;
    let y_denominator = height.saturating_sub(1).max(1) as f32;
    for y in 0..height {
        for x in 0..width {
            let r = ((x as f32 / x_denominator) * 255.0).round() as u8;
            let g = ((y as f32 / y_denominator) * 255.0).round() as u8;
            image.put_pixel(x, y, Rgba([r, g, 128, 255]));
        }
    }
    image
}

pub fn alpha_ramp_rgba8(width: u32, height: u32, rgb: [u8; 3]) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let denominator = width.saturating_sub(1).max(1) as f32;
    for y in 0..height {
        for x in 0..width {
            let alpha = ((x as f32 / denominator) * 255.0).round() as u8;
            image.put_pixel(x, y, Rgba([rgb[0], rgb[1], rgb[2], alpha]));
        }
    }
    image
}

pub fn checkerboard_rgba8(width: u32, height: u32, cell: u32) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let cell = cell.max(1);
    for y in 0..height {
        for x in 0..width {
            let on = ((x / cell) + (y / cell)) % 2 == 0;
            let value = if on { 255 } else { 0 };
            image.put_pixel(x, y, Rgba([value, value, value, 255]));
        }
    }
    image
}

/// R encodes normalized x and G encodes normalized y for coordinate-warp probes.
pub fn coordinate_field_rgba8(width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let x_denominator = width.saturating_sub(1).max(1) as f32;
    let y_denominator = height.saturating_sub(1).max(1) as f32;
    for y in 0..height {
        for x in 0..width {
            let r = ((x as f32 / x_denominator) * 255.0).round() as u8;
            let g = ((y as f32 / y_denominator) * 255.0).round() as u8;
            image.put_pixel(x, y, Rgba([r, g, 0, 255]));
        }
    }
    image
}

pub fn coordinate_field_uv_at_pixel(image: &RgbaImage, x: u32, y: u32) -> Option<(f32, f32)> {
    if x >= image.width() || y >= image.height() {
        return None;
    }
    let pixel = image.get_pixel(x, y).0;
    Some((pixel[0] as f32 / 255.0, pixel[1] as f32 / 255.0))
}

pub fn grid_rgba8(
    width: u32,
    height: u32,
    step: u32,
    line: [u8; 4],
    background: [u8; 4],
) -> RgbaImage {
    let mut image = solid_rgba8(width, height, background);
    let step = step.max(1);
    for y in 0..height {
        for x in 0..width {
            if x % step == 0 || y % step == 0 {
                image.put_pixel(x, y, Rgba(line));
            }
        }
    }
    image
}

pub fn near_edge_square_rgba8(width: u32, height: u32, size: u32, rgba: [u8; 4]) -> RgbaImage {
    let mut image = transparent_rgba8(width, height);
    let size = size.min(width).min(height);
    for y in 0..size {
        for x in 0..size {
            image.put_pixel(x, y, Rgba(rgba));
        }
    }
    image
}

pub fn numbered_frame_rgba8(width: u32, height: u32, frame_index: u32) -> RgbaImage {
    let r = (frame_index & 0xff) as u8;
    let g = ((frame_index >> 8) & 0xff) as u8;
    let b = ((frame_index >> 16) & 0xff) as u8;
    let mut image = solid_rgba8(width, height, [r, g, b, 255]);
    for bit in 0..16u32 {
        if ((frame_index >> bit) & 1) == 1 {
            let x0 = bit * width / 16;
            let x1 = ((bit + 1) * width / 16).min(width);
            for y in 0..height.min(12) {
                for x in x0..x1 {
                    image.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                }
            }
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_field_can_be_read_as_uv() {
        let image = coordinate_field_rgba8(3, 5);

        assert_eq!(coordinate_field_uv_at_pixel(&image, 0, 0), Some((0.0, 0.0)));
        assert_eq!(coordinate_field_uv_at_pixel(&image, 2, 4), Some((1.0, 1.0)));
    }
}
