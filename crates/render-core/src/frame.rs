use raster_cpu::Canvas;

#[derive(Debug)]
pub struct RenderedFrame {
    pub index: u32,
    pub time: f64,
    pub canvas: Canvas,
}
