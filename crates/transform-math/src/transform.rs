use crate::{Mat3, Vec2};

#[derive(Debug, Clone, Copy)]
pub struct Transform2D {
    pub anchor: Vec2,
    pub position: Vec2,
    pub scale_percent: Vec2,
    pub rotation_deg: f32,
    pub opacity_percent: f32,
}

impl Transform2D {
    pub fn matrix(self) -> Mat3 {
        let scale = Vec2::new(self.scale_percent.x / 100.0, self.scale_percent.y / 100.0);
        Mat3::translate(self.position)
            .mul(Mat3::rotate_degrees(self.rotation_deg))
            .mul(Mat3::scale(scale))
            .mul(Mat3::translate(Vec2::new(-self.anchor.x, -self.anchor.y)))
    }
}
