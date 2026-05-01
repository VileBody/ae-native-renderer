#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Time(pub f64);

impl Time {
    pub fn seconds(self) -> f64 {
        self.0
    }
}
