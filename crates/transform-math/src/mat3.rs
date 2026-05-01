use crate::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat3 {
    pub m: [[f32; 3]; 3],
}

impl Mat3 {
    pub fn identity() -> Self {
        Self { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] }
    }

    pub fn translate(v: Vec2) -> Self {
        Self { m: [[1.0, 0.0, v.x], [0.0, 1.0, v.y], [0.0, 0.0, 1.0]] }
    }

    pub fn scale(v: Vec2) -> Self {
        Self { m: [[v.x, 0.0, 0.0], [0.0, v.y, 0.0], [0.0, 0.0, 1.0]] }
    }

    pub fn rotate_degrees(deg: f32) -> Self {
        let r = deg.to_radians();
        let c = r.cos();
        let s = r.sin();
        Self { m: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]] }
    }

    pub fn mul(self, rhs: Self) -> Self {
        let mut out = [[0.0; 3]; 3];
        for y in 0..3 {
            for x in 0..3 {
                out[y][x] = self.m[y][0] * rhs.m[0][x]
                    + self.m[y][1] * rhs.m[1][x]
                    + self.m[y][2] * rhs.m[2][x];
            }
        }
        Self { m: out }
    }

    pub fn transform_point(self, p: Vec2) -> Vec2 {
        Vec2 {
            x: self.m[0][0] * p.x + self.m[0][1] * p.y + self.m[0][2],
            y: self.m[1][0] * p.x + self.m[1][1] * p.y + self.m[1][2],
        }
    }
}
