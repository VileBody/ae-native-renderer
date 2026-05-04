use crate::Canvas;

pub trait Sampler {
    fn sample(&self, image: &Canvas, x: f32, y: f32) -> [u8; 4];
}

pub struct NearestSampler;

impl Sampler for NearestSampler {
    fn sample(&self, image: &Canvas, x: f32, y: f32) -> [u8; 4] {
        let xi = x.round() as i32;
        let yi = y.round() as i32;
        if xi < 0 || yi < 0 || xi >= image.width as i32 || yi >= image.height as i32 {
            return [0, 0, 0, 0];
        }
        let idx = (((yi as u32) * image.width + xi as u32) * 4) as usize;
        [
            image.data[idx],
            image.data[idx + 1],
            image.data[idx + 2],
            image.data[idx + 3],
        ]
    }
}

pub struct BilinearSamplerTodo;

pub struct BilinearSampler;

impl Sampler for BilinearSampler {
    fn sample(&self, image: &Canvas, x: f32, y: f32) -> [u8; 4] {
        if image.width == 0 || image.height == 0 {
            return [0, 0, 0, 0];
        }
        if x < 0.0 || y < 0.0 || x > (image.width - 1) as f32 || y > (image.height - 1) as f32 {
            return [0, 0, 0, 0];
        }
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(image.width - 1);
        let y1 = (y0 + 1).min(image.height - 1);
        let tx = x - x0 as f32;
        let ty = y - y0 as f32;
        let p00 = image.pixel(x0, y0);
        let p10 = image.pixel(x1, y0);
        let p01 = image.pixel(x0, y1);
        let p11 = image.pixel(x1, y1);
        let mut out = [0_u8; 4];
        for channel in 0..4 {
            let top = lerp(p00[channel] as f32, p10[channel] as f32, tx);
            let bottom = lerp(p01[channel] as f32, p11[channel] as f32, tx);
            out[channel] = lerp(top, bottom, ty).round().clamp(0.0, 255.0) as u8;
        }
        out
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_sampler_interpolates_channels() {
        let mut canvas = Canvas::transparent(2, 2);
        canvas.set_pixel(0, 0, [0, 0, 0, 255]);
        canvas.set_pixel(1, 0, [100, 0, 0, 255]);
        canvas.set_pixel(0, 1, [0, 100, 0, 255]);
        canvas.set_pixel(1, 1, [100, 100, 0, 255]);

        let sampled = BilinearSampler.sample(&canvas, 0.5, 0.5);

        assert_eq!(sampled, [50, 50, 0, 255]);
    }
}
