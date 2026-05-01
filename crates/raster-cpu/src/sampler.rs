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
        [image.data[idx], image.data[idx + 1], image.data[idx + 2], image.data[idx + 3]]
    }
}

pub struct BilinearSamplerTodo;
