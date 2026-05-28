use crate::ArePathPoint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreCurveRowXTable {
    pub rounded_start_y: i32,
    pub rounded_end_y: i32,
    pub crossing_x: Vec<f32>,
}

impl AreCurveRowXTable {
    pub fn row_count(&self) -> usize {
        self.crossing_x.len()
    }
}

pub fn cubic_scalar_table_125b8(p0: f32, p1: f32, p2: f32, p3: f32, step_count: usize) -> Vec<f32> {
    if step_count < 2 {
        return vec![p0, p3];
    }

    let dt = 1.0 / step_count as f32;
    let d10 = (p1 - p0) * 3.0;
    let d21 = (p2 - p1) * 3.0;
    let second = (d21 - d10) * dt * dt;
    let third = (p3 - d21 - p0) * dt * dt * dt;
    let third_six = third * 6.0;
    let mut second_twice = second + second;
    let mut delta = third + second + dt * d10;
    let mut value = p0;

    let mut table = Vec::with_capacity(step_count + 1);
    for _ in 0..step_count {
        second_twice += third_six;
        table.push(value);
        value += delta;
        delta += second_twice;
    }
    table.push(p3);
    table
}

pub fn row_x_table_1268c(x_table: &[f32], y_table: &[f32]) -> AreCurveRowXTable {
    assert_eq!(
        x_table.len(),
        y_table.len(),
        "native row x table requires paired x/y scalar tables",
    );
    assert!(
        !x_table.is_empty(),
        "native row x table requires at least one sample",
    );

    let rounded_start_y = native_round_i32(y_table[0]);
    let rounded_end_y = native_round_i32(*y_table.last().unwrap());
    let mut previous_rounded_y = rounded_start_y;
    let mut crossing_x = Vec::new();

    for i in 0..x_table.len().saturating_sub(1) {
        let y0 = y_table[i];
        let y1 = y_table[i + 1];
        let rounded_y1 = native_round_i32(y1);
        if previous_rounded_y != rounded_y1 {
            let x0 = x_table[i];
            let x1 = x_table[i + 1];
            let denominator = y1 - y0;
            let x = if denominator.abs() <= f32::EPSILON {
                x1
            } else {
                ((y1 - rounded_y1 as f32) * (x0 - x1)) / denominator + x1
            };
            crossing_x.push(x);
            previous_rounded_y = rounded_y1;
        }
    }

    let expected = (rounded_end_y - rounded_start_y).max(0) as usize;
    if crossing_x.len() < expected {
        let fill = crossing_x
            .last()
            .copied()
            .unwrap_or_else(|| *x_table.last().unwrap());
        crossing_x.resize(expected, fill);
    }

    AreCurveRowXTable {
        rounded_start_y,
        rounded_end_y,
        crossing_x,
    }
}

pub fn cubic_row_x_table_fc04(
    p0: ArePathPoint,
    p1: ArePathPoint,
    p2: ArePathPoint,
    p3: ArePathPoint,
    step_count: usize,
) -> AreCurveRowXTable {
    let x_table = cubic_scalar_table_125b8(p0.x, p1.x, p2.x, p3.x, step_count);
    let y_table = cubic_scalar_table_125b8(p0.y, p1.y, p2.y, p3.y, step_count);
    row_x_table_1268c(&x_table, &y_table)
}

fn native_round_i32(value: f32) -> i32 {
    let rounded = value.round();
    if rounded.is_nan() {
        0
    } else if rounded > i32::MAX as f32 {
        i32::MAX
    } else if rounded < i32::MIN as f32 {
        i32::MIN
    } else {
        rounded as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.0001,
            "actual {actual} expected {expected}",
        );
    }

    #[test]
    fn p6_curve_row_x_table_125b8_linear_cubic_scalar_table() {
        let table = cubic_scalar_table_125b8(0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 4);

        assert_eq!(table.len(), 5);
        assert_close(table[0], 0.0);
        assert_close(table[1], 0.25);
        assert_close(table[2], 0.5);
        assert_close(table[3], 0.75);
        assert_close(table[4], 1.0);
    }

    #[test]
    fn p6_curve_row_x_table_1268c_interpolates_at_rounded_y_transition() {
        let table = row_x_table_1268c(&[0.0, 10.0], &[0.2, 1.2]);

        assert_eq!(table.rounded_start_y, 0);
        assert_eq!(table.rounded_end_y, 1);
        assert_eq!(table.row_count(), 1);
        assert_close(table.crossing_x[0], 8.0);
    }

    #[test]
    fn p6_curve_row_x_table_fc04_builds_monotone_linearized_curve_rows() {
        let table = cubic_row_x_table_fc04(
            ArePathPoint::new(0.2, 0.2),
            ArePathPoint::new(1.2, 1.2),
            ArePathPoint::new(2.2, 2.2),
            ArePathPoint::new(3.2, 3.2),
            8,
        );

        assert_eq!(table.rounded_start_y, 0);
        assert_eq!(table.rounded_end_y, 3);
        assert_eq!(table.row_count(), 3);
        assert_close(table.crossing_x[0], 1.0);
        assert_close(table.crossing_x[1], 2.0);
        assert_close(table.crossing_x[2], 3.0);
    }
}
