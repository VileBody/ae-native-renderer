use crate::{
    box_blur::{canvas_alpha_stats, canvas_debug_hash, CanvasAlphaStats},
    param_bool_any, param_f32_at_any, param_value, Effect, EffectContext,
};
use raster_cpu::Canvas;
use serde_json::Value;
use std::collections::VecDeque;

const EDGE_POLICY_CLIP_TO_IMAGE_BOUNDS: &str = "clip_to_image_bounds";
const EDGE_POLICY_TRANSPARENT_BLACK_OUTSIDE_BOUNDS: &str = "transparent_black_outside_bounds";

#[derive(Debug, Default)]
pub struct Minimax;

impl Effect for Minimax {
    fn match_name(&self) -> &'static str {
        "ADBE Minimax"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = MinimaxParams::from_json(params, _ctx.time);
        let radius = kernel_radius(params.radius);
        if radius == 0 || input.width == 0 || input.height == 0 {
            return Ok(input.clone());
        }

        Ok(minimax_canvas(input, radius, params))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MinimaxParams {
    pub operation: Operation,
    pub radius: f32,
    pub channels: Channels,
    pub direction: Direction,
    pub dont_shrink_edges: bool,
}

impl MinimaxParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            operation: Operation::from_params(params),
            radius: param_f32_at_any(params, &["radius", "Radius", "0002"], time, 0.0),
            channels: Channels::from_params(params),
            direction: Direction::from_params(params),
            dont_shrink_edges: param_bool_any(
                params,
                &[
                    "dont_shrink_edges",
                    "Don't Shrink Edges",
                    "dontShrinkEdges",
                    "0005",
                ],
                false,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimaxDebugParams {
    pub operation: &'static str,
    pub channels: &'static str,
    pub direction: &'static str,
    pub radius: f32,
    pub kernel_radius: u32,
    pub dont_shrink_edges: bool,
    pub stage_count: u8,
    pub edge_policy: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimaxIntermediateHashes {
    pub input_rgba: u64,
    pub first_pass_rgba: u64,
    pub first_stage_rgba: u64,
    pub second_stage_rgba: u64,
    pub output_rgba: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimaxIntermediateAlphaStats {
    pub input: CanvasAlphaStats,
    pub first_pass: CanvasAlphaStats,
    pub first_stage: CanvasAlphaStats,
    pub second_stage: CanvasAlphaStats,
    pub output: CanvasAlphaStats,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimaxDebugTrace {
    pub params: MinimaxDebugParams,
    pub hashes: MinimaxIntermediateHashes,
    pub alpha: MinimaxIntermediateAlphaStats,
}

pub fn minimax_debug_trace(input: &Canvas, params: &Value, time: f64) -> MinimaxDebugTrace {
    let params = MinimaxParams::from_json(params, time);
    let radius = kernel_radius(params.radius);
    let canvases = minimax_debug_canvases(input, radius, params);

    MinimaxDebugTrace {
        params: MinimaxDebugParams {
            operation: operation_label(params.operation),
            channels: channels_label(params.channels),
            direction: direction_label(params.direction),
            radius: params.radius,
            kernel_radius: radius,
            dont_shrink_edges: params.dont_shrink_edges,
            stage_count: operation_stage_count(params.operation),
            edge_policy: edge_policy_label(params.dont_shrink_edges),
        },
        hashes: MinimaxIntermediateHashes {
            input_rgba: canvas_debug_hash(input),
            first_pass_rgba: canvas_debug_hash(&canvases.first_pass),
            first_stage_rgba: canvas_debug_hash(&canvases.first_stage),
            second_stage_rgba: canvas_debug_hash(&canvases.second_stage),
            output_rgba: canvas_debug_hash(&canvases.output),
        },
        alpha: MinimaxIntermediateAlphaStats {
            input: canvas_alpha_stats(input),
            first_pass: canvas_alpha_stats(&canvases.first_pass),
            first_stage: canvas_alpha_stats(&canvases.first_stage),
            second_stage: canvas_alpha_stats(&canvases.second_stage),
            output: canvas_alpha_stats(&canvases.output),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    Maximum,
    Minimum,
    MinimumThenMaximum,
    MaximumThenMinimum,
}

impl Operation {
    fn from_params(params: &Value) -> Self {
        for name in ["operation", "Operation", "mode", "0001"] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if (text.contains("minimum") || text.contains("min"))
                    && (text.contains("then maximum") || text.contains("then max"))
                {
                    return Self::MinimumThenMaximum;
                }
                if (text.contains("maximum") || text.contains("max"))
                    && (text.contains("then minimum") || text.contains("then min"))
                {
                    return Self::MaximumThenMinimum;
                }
                if text.contains("min") || text.contains("erode") {
                    return Self::Minimum;
                }
                if text.contains("max") || text.contains("dilate") {
                    return Self::Maximum;
                }
            }
            if let Some(number) = number_as_i64(value) {
                return match number {
                    1 => Self::Minimum,
                    2 => Self::Maximum,
                    3 => Self::MinimumThenMaximum,
                    4 => Self::MaximumThenMinimum,
                    _ => Self::Maximum,
                };
            }
        }
        Self::Maximum
    }
}

fn operation_label(operation: Operation) -> &'static str {
    match operation {
        Operation::Maximum => "maximum",
        Operation::Minimum => "minimum",
        Operation::MinimumThenMaximum => "minimum_then_maximum",
        Operation::MaximumThenMinimum => "maximum_then_minimum",
    }
}

fn operation_stage_count(operation: Operation) -> u8 {
    match operation {
        Operation::Minimum | Operation::Maximum => 1,
        Operation::MinimumThenMaximum | Operation::MaximumThenMinimum => 2,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Channels {
    Color,
    AlphaAndColor,
    Red,
    Green,
    Blue,
    Alpha,
}

impl Channels {
    fn from_params(params: &Value) -> Self {
        for name in ["channels", "Channels", "channel", "0003"] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if text.contains("alpha") && (text.contains("color") || text.contains("rgb")) {
                    return Self::AlphaAndColor;
                }
                if text == "r" || text.contains("red") {
                    return Self::Red;
                }
                if text == "g" || text.contains("green") {
                    return Self::Green;
                }
                if text == "b" || text.contains("blue") {
                    return Self::Blue;
                }
                if text.contains("rgb") || text.contains("color") {
                    return Self::Color;
                }
                if text.contains("alpha") {
                    return Self::Alpha;
                }
            }
            if let Some(number) = number_as_i64(value) {
                return match number {
                    1 => Self::Color,
                    2 => Self::AlphaAndColor,
                    3 => Self::Red,
                    4 => Self::Green,
                    5 => Self::Blue,
                    6 => Self::Alpha,
                    _ => Self::Color,
                };
            }
            if value.as_bool() == Some(true) {
                return Self::Color;
            }
        }
        Self::Color
    }
}

fn channels_label(channels: Channels) -> &'static str {
    match channels {
        Channels::Color => "color",
        Channels::AlphaAndColor => "alpha_and_color",
        Channels::Red => "red",
        Channels::Green => "green",
        Channels::Blue => "blue",
        Channels::Alpha => "alpha",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    HorizontalAndVertical,
    Horizontal,
    Vertical,
}

impl Direction {
    fn from_params(params: &Value) -> Self {
        for name in ["direction", "Direction", "0004"] {
            let Some(value) = param_value(params, name) else {
                continue;
            };
            if let Some(text) = value.as_str() {
                let text = text.to_ascii_lowercase();
                if text.contains("vertical") && !text.contains("horizontal") {
                    return Self::Vertical;
                }
                if text.contains("horizontal") && !text.contains("vertical") {
                    return Self::Horizontal;
                }
                if text.contains("horiz") || text.contains("vert") || text.contains("both") {
                    return Self::HorizontalAndVertical;
                }
            }
            if let Some(number) = number_as_i64(value) {
                return match number {
                    2 => Self::Horizontal,
                    3 => Self::Vertical,
                    _ => Self::HorizontalAndVertical,
                };
            }
        }
        Self::HorizontalAndVertical
    }
}

fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::HorizontalAndVertical => "horizontal_and_vertical",
        Direction::Horizontal => "horizontal",
        Direction::Vertical => "vertical",
    }
}

fn edge_policy_label(dont_shrink_edges: bool) -> &'static str {
    if dont_shrink_edges {
        EDGE_POLICY_CLIP_TO_IMAGE_BOUNDS
    } else {
        EDGE_POLICY_TRANSPARENT_BLACK_OUTSIDE_BOUNDS
    }
}

fn kernel_radius(radius: f32) -> u32 {
    radius.round().max(0.0) as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extremum {
    Maximum,
    Minimum,
}

fn minimax_canvas(input: &Canvas, radius: u32, params: MinimaxParams) -> Canvas {
    match params.operation {
        Operation::Minimum => apply_extremum_operation(
            input,
            radius,
            Extremum::Minimum,
            params.channels,
            params.direction,
            params.dont_shrink_edges,
        ),
        Operation::Maximum => apply_extremum_operation(
            input,
            radius,
            Extremum::Maximum,
            params.channels,
            params.direction,
            params.dont_shrink_edges,
        ),
        Operation::MinimumThenMaximum => {
            let eroded = apply_extremum_operation(
                input,
                radius,
                Extremum::Minimum,
                params.channels,
                params.direction,
                params.dont_shrink_edges,
            );
            apply_extremum_operation(
                &eroded,
                radius,
                Extremum::Maximum,
                params.channels,
                params.direction,
                params.dont_shrink_edges,
            )
        }
        Operation::MaximumThenMinimum => {
            let expanded = apply_extremum_operation(
                input,
                radius,
                Extremum::Maximum,
                params.channels,
                params.direction,
                params.dont_shrink_edges,
            );
            apply_extremum_operation(
                &expanded,
                radius,
                Extremum::Minimum,
                params.channels,
                params.direction,
                params.dont_shrink_edges,
            )
        }
    }
}

#[derive(Debug, Clone)]
struct MinimaxDebugCanvases {
    first_pass: Canvas,
    first_stage: Canvas,
    second_stage: Canvas,
    output: Canvas,
}

#[derive(Debug, Clone)]
struct ExtremumStageDebug {
    first_pass: Canvas,
    output: Canvas,
}

fn minimax_debug_canvases(
    input: &Canvas,
    radius: u32,
    params: MinimaxParams,
) -> MinimaxDebugCanvases {
    if radius == 0 || input.width == 0 || input.height == 0 {
        let output = input.clone();
        return MinimaxDebugCanvases {
            first_pass: output.clone(),
            first_stage: output.clone(),
            second_stage: output.clone(),
            output,
        };
    }

    match params.operation {
        Operation::Minimum => single_stage_debug(input, radius, Extremum::Minimum, params),
        Operation::Maximum => single_stage_debug(input, radius, Extremum::Maximum, params),
        Operation::MinimumThenMaximum => {
            two_stage_debug(input, radius, Extremum::Minimum, Extremum::Maximum, params)
        }
        Operation::MaximumThenMinimum => {
            two_stage_debug(input, radius, Extremum::Maximum, Extremum::Minimum, params)
        }
    }
}

fn single_stage_debug(
    input: &Canvas,
    radius: u32,
    extremum: Extremum,
    params: MinimaxParams,
) -> MinimaxDebugCanvases {
    let stage = apply_extremum_operation_debug(
        input,
        radius,
        extremum,
        params.channels,
        params.direction,
        params.dont_shrink_edges,
    );
    MinimaxDebugCanvases {
        first_pass: stage.first_pass,
        first_stage: stage.output.clone(),
        second_stage: stage.output.clone(),
        output: stage.output,
    }
}

fn two_stage_debug(
    input: &Canvas,
    radius: u32,
    first_extremum: Extremum,
    second_extremum: Extremum,
    params: MinimaxParams,
) -> MinimaxDebugCanvases {
    let first_stage = apply_extremum_operation_debug(
        input,
        radius,
        first_extremum,
        params.channels,
        params.direction,
        params.dont_shrink_edges,
    );
    let second_stage = apply_extremum_operation_debug(
        &first_stage.output,
        radius,
        second_extremum,
        params.channels,
        params.direction,
        params.dont_shrink_edges,
    );
    MinimaxDebugCanvases {
        first_pass: first_stage.first_pass,
        first_stage: first_stage.output,
        second_stage: second_stage.output.clone(),
        output: second_stage.output,
    }
}

fn apply_extremum_operation(
    input: &Canvas,
    radius: u32,
    extremum: Extremum,
    channels: Channels,
    direction: Direction,
    dont_shrink_edges: bool,
) -> Canvas {
    match direction {
        Direction::HorizontalAndVertical => {
            let horizontal = minimax_pass(
                input,
                radius,
                extremum,
                channels,
                Axis::Horizontal,
                dont_shrink_edges,
            );
            minimax_pass(
                &horizontal,
                radius,
                extremum,
                channels,
                Axis::Vertical,
                dont_shrink_edges,
            )
        }
        Direction::Horizontal => minimax_pass(
            input,
            radius,
            extremum,
            channels,
            Axis::Horizontal,
            dont_shrink_edges,
        ),
        Direction::Vertical => minimax_pass(
            input,
            radius,
            extremum,
            channels,
            Axis::Vertical,
            dont_shrink_edges,
        ),
    }
}

fn apply_extremum_operation_debug(
    input: &Canvas,
    radius: u32,
    extremum: Extremum,
    channels: Channels,
    direction: Direction,
    dont_shrink_edges: bool,
) -> ExtremumStageDebug {
    match direction {
        Direction::HorizontalAndVertical => {
            let horizontal = minimax_pass(
                input,
                radius,
                extremum,
                channels,
                Axis::Horizontal,
                dont_shrink_edges,
            );
            let output = minimax_pass(
                &horizontal,
                radius,
                extremum,
                channels,
                Axis::Vertical,
                dont_shrink_edges,
            );
            ExtremumStageDebug {
                first_pass: horizontal,
                output,
            }
        }
        Direction::Horizontal => {
            let output = minimax_pass(
                input,
                radius,
                extremum,
                channels,
                Axis::Horizontal,
                dont_shrink_edges,
            );
            ExtremumStageDebug {
                first_pass: output.clone(),
                output,
            }
        }
        Direction::Vertical => {
            let output = minimax_pass(
                input,
                radius,
                extremum,
                channels,
                Axis::Vertical,
                dont_shrink_edges,
            );
            ExtremumStageDebug {
                first_pass: output.clone(),
                output,
            }
        }
    }
}

fn minimax_pass(
    input: &Canvas,
    radius: u32,
    extremum: Extremum,
    channels: Channels,
    axis: Axis,
    dont_shrink_edges: bool,
) -> Canvas {
    let mut output = input.clone();
    let (line_count, line_len) = match axis {
        Axis::Horizontal => (input.height as usize, input.width as usize),
        Axis::Vertical => (input.width as usize, input.height as usize),
    };
    let radius = radius as usize;
    let mut deque = VecDeque::with_capacity(line_len);

    for channel in 0..4 {
        if !channel_is_selected(channels, channel) {
            continue;
        }
        for line in 0..line_count {
            deque.clear();
            minimax_line(
                input,
                &mut output,
                line,
                line_len,
                radius,
                channel,
                axis,
                extremum,
                dont_shrink_edges,
                &mut deque,
            );
        }
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn minimax_line(
    input: &Canvas,
    output: &mut Canvas,
    line: usize,
    line_len: usize,
    radius: usize,
    channel: usize,
    axis: Axis,
    extremum: Extremum,
    dont_shrink_edges: bool,
    deque: &mut VecDeque<(usize, u8)>,
) {
    let mut next = 0_usize;

    for center in 0..line_len {
        let window_end = center.saturating_add(radius).min(line_len - 1);
        while next <= window_end {
            let value = input.data[canvas_index(input.width as usize, axis, line, next, channel)];
            while let Some(&(_, previous)) = deque.back() {
                let is_dominated = match extremum {
                    Extremum::Maximum => previous <= value,
                    Extremum::Minimum => previous >= value,
                };
                if !is_dominated {
                    break;
                }
                deque.pop_back();
            }
            deque.push_back((next, value));
            next += 1;
        }

        let window_start = center.saturating_sub(radius);
        while deque
            .front()
            .is_some_and(|&(index, _)| index < window_start)
        {
            deque.pop_front();
        }

        let samples_transparent_black = !dont_shrink_edges
            && extremum == Extremum::Minimum
            && (radius > center || radius > line_len - 1 - center);
        let value = if samples_transparent_black {
            0
        } else {
            deque.front().map_or(0, |&(_, value)| value)
        };
        let output_index = canvas_index(output.width as usize, axis, line, center, channel);
        output.data[output_index] = value;
    }
}

fn canvas_index(width: usize, axis: Axis, line: usize, position: usize, channel: usize) -> usize {
    let (x, y) = match axis {
        Axis::Horizontal => (position, line),
        Axis::Vertical => (line, position),
    };
    (y * width + x) * 4 + channel
}

fn channel_is_selected(channels: Channels, channel: usize) -> bool {
    match channels {
        Channels::Color => channel < 3,
        Channels::AlphaAndColor => channel < 4,
        Channels::Red => channel == 0,
        Channels::Green => channel == 1,
        Channels::Blue => channel == 2,
        Channels::Alpha => channel == 3,
    }
}

fn number_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|number| number.round() as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[derive(Debug, Clone, Copy)]
    struct TestRng(u64);

    impl TestRng {
        fn next_u8(&mut self) -> u8 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 32) as u8
        }

        fn dimension(&mut self) -> u32 {
            u32::from(self.next_u8() % 8 + 1)
        }
    }

    fn ctx() -> EffectContext {
        EffectContext {
            time: 0.0,
            fps: 30.0,
        }
    }

    fn render(input: &Canvas, params: Value) -> Canvas {
        Minimax::default().render(input, &ctx(), &params).unwrap()
    }

    fn random_canvas(rng: &mut TestRng, width: u32, height: u32) -> Canvas {
        let mut input = Canvas::transparent(width, height);
        for value in &mut input.data {
            *value = rng.next_u8();
        }
        input
    }

    fn naive_minimax_canvas(input: &Canvas, radius: u32, params: MinimaxParams) -> Canvas {
        match params.operation {
            Operation::Minimum => naive_apply_extremum_operation(
                input,
                radius,
                Extremum::Minimum,
                params.channels,
                params.direction,
                params.dont_shrink_edges,
            ),
            Operation::Maximum => naive_apply_extremum_operation(
                input,
                radius,
                Extremum::Maximum,
                params.channels,
                params.direction,
                params.dont_shrink_edges,
            ),
            Operation::MinimumThenMaximum => {
                let first = naive_apply_extremum_operation(
                    input,
                    radius,
                    Extremum::Minimum,
                    params.channels,
                    params.direction,
                    params.dont_shrink_edges,
                );
                naive_apply_extremum_operation(
                    &first,
                    radius,
                    Extremum::Maximum,
                    params.channels,
                    params.direction,
                    params.dont_shrink_edges,
                )
            }
            Operation::MaximumThenMinimum => {
                let first = naive_apply_extremum_operation(
                    input,
                    radius,
                    Extremum::Maximum,
                    params.channels,
                    params.direction,
                    params.dont_shrink_edges,
                );
                naive_apply_extremum_operation(
                    &first,
                    radius,
                    Extremum::Minimum,
                    params.channels,
                    params.direction,
                    params.dont_shrink_edges,
                )
            }
        }
    }

    fn naive_apply_extremum_operation(
        input: &Canvas,
        radius: u32,
        extremum: Extremum,
        channels: Channels,
        direction: Direction,
        dont_shrink_edges: bool,
    ) -> Canvas {
        match direction {
            Direction::HorizontalAndVertical => {
                let horizontal = naive_minimax_pass(
                    input,
                    radius,
                    extremum,
                    channels,
                    Axis::Horizontal,
                    dont_shrink_edges,
                );
                naive_minimax_pass(
                    &horizontal,
                    radius,
                    extremum,
                    channels,
                    Axis::Vertical,
                    dont_shrink_edges,
                )
            }
            Direction::Horizontal => naive_minimax_pass(
                input,
                radius,
                extremum,
                channels,
                Axis::Horizontal,
                dont_shrink_edges,
            ),
            Direction::Vertical => naive_minimax_pass(
                input,
                radius,
                extremum,
                channels,
                Axis::Vertical,
                dont_shrink_edges,
            ),
        }
    }

    fn naive_minimax_pass(
        input: &Canvas,
        radius: u32,
        extremum: Extremum,
        channels: Channels,
        axis: Axis,
        dont_shrink_edges: bool,
    ) -> Canvas {
        let mut output = input.clone();
        let (line_count, line_len) = match axis {
            Axis::Horizontal => (input.height as usize, input.width as usize),
            Axis::Vertical => (input.width as usize, input.height as usize),
        };
        let radius = radius as usize;

        for channel in 0..4 {
            if !channel_is_selected(channels, channel) {
                continue;
            }
            for line in 0..line_count {
                for center in 0..line_len {
                    let samples_transparent_black = !dont_shrink_edges
                        && extremum == Extremum::Minimum
                        && (radius > center || radius > line_len - 1 - center);
                    let value = if samples_transparent_black {
                        0
                    } else {
                        let start = center.saturating_sub(radius);
                        let end = center.saturating_add(radius).min(line_len - 1);
                        let initial = match extremum {
                            Extremum::Maximum => 0,
                            Extremum::Minimum => 255,
                        };
                        (start..=end).fold(initial, |value, position| {
                            let sample = input.data
                                [canvas_index(input.width as usize, axis, line, position, channel)];
                            match extremum {
                                Extremum::Maximum => value.max(sample),
                                Extremum::Minimum => value.min(sample),
                            }
                        })
                    };
                    let index = canvas_index(output.width as usize, axis, line, center, channel);
                    output.data[index] = value;
                }
            }
        }
        output
    }

    #[test]
    fn maximum_expands_alpha() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [20, 40, 60, 255]);

        let output = render(
            &input,
            json!({ "radius": 1, "operation": "maximum", "channels": "alpha" }),
        );

        assert_eq!(output.pixel(0, 0)[3], 255);
        assert_eq!(output.pixel(2, 0)[3], 255);
        assert_eq!(output.pixel(0, 0)[0], 0);
    }

    #[test]
    fn minimum_erodes_alpha() {
        let mut input = Canvas::new(3, 1, [10, 20, 30, 255]);
        input.set_pixel(1, 0, [10, 20, 30, 0]);

        let output = render(
            &input,
            json!({ "radius": 1, "operation": "minimum", "channels": "alpha" }),
        );

        assert_eq!(output.pixel(0, 0), [10, 20, 30, 0]);
        assert_eq!(output.pixel(1, 0), [10, 20, 30, 0]);
        assert_eq!(output.pixel(2, 0), [10, 20, 30, 0]);
    }

    #[test]
    fn minimum_erodes_rgb_without_eroding_alpha() {
        let mut input = Canvas::new(3, 1, [200, 180, 160, 255]);
        input.set_pixel(1, 0, [10, 20, 30, 0]);

        let output = render(
            &input,
            json!({ "radius": 1, "operation": "minimum", "channels": "rgb", "dont_shrink_edges": true }),
        );

        assert_eq!(output.pixel(0, 0), [10, 20, 30, 255]);
        assert_eq!(output.pixel(2, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn params_accept_ae_numbered_wrapped_values_for_full_surface() {
        let params = MinimaxParams::from_json(
            &json!({
                "0001": { "value": 3 },
                "0002": { "value": 9 },
                "0003": { "value": 2 },
                "0004": { "value": 3 },
                "0005": { "value": 1 }
            }),
            0.0,
        );

        assert_eq!(params.operation, Operation::MinimumThenMaximum);
        assert_eq!(params.radius, 9.0);
        assert_eq!(params.channels, Channels::AlphaAndColor);
        assert_eq!(params.direction, Direction::Vertical);
        assert!(params.dont_shrink_edges);
    }

    #[test]
    fn ae_probe_enum_mapping_uses_minimum_for_one_and_color_for_channel_one() {
        let params = MinimaxParams::from_json(
            &json!({
                "0001": { "value": 1 },
                "0002": { "value": 12 },
                "0003": { "value": 1 }
            }),
            0.0,
        );

        assert_eq!(params.operation, Operation::Minimum);
        assert_eq!(params.channels, Channels::Color);
        assert_eq!(params.direction, Direction::HorizontalAndVertical);
        assert!(!params.dont_shrink_edges);
    }

    #[test]
    fn channel_modes_select_expected_lanes() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(0, 0, [100, 50, 60, 70]);
        input.set_pixel(1, 0, [10, 200, 20, 30]);
        input.set_pixel(2, 0, [5, 6, 250, 255]);

        let base = json!({ "0001": 2, "0002": 1, "0004": 2 });

        let mut color = base.clone();
        color["0003"] = json!(1);
        assert_eq!(render(&input, color).pixel(1, 0), [100, 200, 250, 30]);

        let mut alpha_and_color = base.clone();
        alpha_and_color["0003"] = json!(2);
        assert_eq!(
            render(&input, alpha_and_color).pixel(1, 0),
            [100, 200, 250, 255]
        );

        let mut red = base.clone();
        red["0003"] = json!(3);
        assert_eq!(render(&input, red).pixel(1, 0), [100, 200, 20, 30]);

        let mut alpha = base;
        alpha["0003"] = json!(6);
        assert_eq!(render(&input, alpha).pixel(1, 0), [10, 200, 20, 255]);
    }

    #[test]
    fn direction_modes_expand_alpha_on_expected_axes() {
        let mut input = Canvas::transparent(3, 3);
        input.set_pixel(1, 1, [0, 0, 0, 255]);

        let horizontal = render(
            &input,
            json!({ "0001": 2, "0002": 1, "0003": 6, "0004": 2 }),
        );
        assert_eq!(horizontal.pixel(0, 1)[3], 255);
        assert_eq!(horizontal.pixel(2, 1)[3], 255);
        assert_eq!(horizontal.pixel(1, 0)[3], 0);
        assert_eq!(horizontal.pixel(1, 2)[3], 0);

        let vertical = render(
            &input,
            json!({ "0001": 2, "0002": 1, "0003": 6, "0004": 3 }),
        );
        assert_eq!(vertical.pixel(1, 0)[3], 255);
        assert_eq!(vertical.pixel(1, 2)[3], 255);
        assert_eq!(vertical.pixel(0, 1)[3], 0);
        assert_eq!(vertical.pixel(2, 1)[3], 0);

        let both = render(
            &input,
            json!({ "0001": 2, "0002": 1, "0003": 6, "0004": 1 }),
        );
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(both.pixel(x, y)[3], 255);
            }
        }
    }

    #[test]
    fn compound_operations_apply_minimax_stage_order() {
        let mut spike = Canvas::transparent(3, 1);
        spike.set_pixel(1, 0, [0, 0, 0, 255]);
        let opened = render(
            &spike,
            json!({ "0001": 3, "0002": 1, "0003": 6, "0004": 2 }),
        );
        assert_eq!(opened.pixel(0, 0)[3], 0);
        assert_eq!(opened.pixel(1, 0)[3], 0);
        assert_eq!(opened.pixel(2, 0)[3], 0);

        let mut hole = Canvas::new(3, 1, [0, 0, 0, 255]);
        hole.set_pixel(1, 0, [0, 0, 0, 0]);
        let closed = render(
            &hole,
            json!({ "0001": 4, "0002": 1, "0003": 6, "0004": 2, "0005": 1 }),
        );
        assert_eq!(closed.pixel(0, 0)[3], 255);
        assert_eq!(closed.pixel(1, 0)[3], 255);
        assert_eq!(closed.pixel(2, 0)[3], 255);
    }

    #[test]
    fn minimum_uses_transparent_black_outside_bounds_unless_dont_shrink_edges_is_set() {
        let input = Canvas::new(5, 1, [255, 255, 255, 255]);

        let shrink_edges = render(
            &input,
            json!({ "0001": 1, "0002": 1, "0003": 2, "0004": 2, "0005": 0 }),
        );
        assert_eq!(shrink_edges.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(shrink_edges.pixel(1, 0), [255, 255, 255, 255]);
        assert_eq!(shrink_edges.pixel(3, 0), [255, 255, 255, 255]);
        assert_eq!(shrink_edges.pixel(4, 0), [0, 0, 0, 0]);

        let preserve_edges = render(
            &input,
            json!({ "0001": 1, "0002": 1, "0003": 2, "0004": 2, "0005": 1 }),
        );
        for x in 0..5 {
            assert_eq!(preserve_edges.pixel(x, 0), [255, 255, 255, 255]);
        }
    }

    #[test]
    fn params_accept_time_varying_numbered_radius() {
        let params = MinimaxParams::from_json(
            &json!({
                "0002": {
                    "keyframes": [
                        { "t": 0.0, "v": 2.0 },
                        { "t": 1.0, "v": 6.0 }
                    ]
                }
            }),
            0.5,
        );

        assert_eq!(params.radius, 4.0);
    }

    #[test]
    fn fractional_radius_matches_ae_round_half_up_probe() {
        let input = Canvas::transparent(3, 1);

        let below_half = minimax_debug_trace(&input, &json!({ "0002": 0.49 }), 0.0);
        let at_half = minimax_debug_trace(&input, &json!({ "0002": 0.5 }), 0.0);
        let above_one_and_half = minimax_debug_trace(&input, &json!({ "0002": 1.6 }), 0.0);

        assert_eq!(below_half.params.kernel_radius, 0);
        assert_eq!(at_half.params.kernel_radius, 1);
        assert_eq!(above_one_and_half.params.kernel_radius, 2);
    }

    #[test]
    fn kernel_radius_is_not_capped_at_32() {
        assert_eq!(kernel_radius(32.0), 32);
        assert_eq!(kernel_radius(33.0), 33);
        assert_eq!(kernel_radius(165.0), 165);
        assert_eq!(kernel_radius(-1.0), 0);
    }

    #[test]
    fn monotonic_deque_matches_naive_reference_for_all_modes() {
        const OPERATIONS: [Operation; 4] = [
            Operation::Maximum,
            Operation::Minimum,
            Operation::MinimumThenMaximum,
            Operation::MaximumThenMinimum,
        ];
        const CHANNELS: [Channels; 6] = [
            Channels::Color,
            Channels::AlphaAndColor,
            Channels::Red,
            Channels::Green,
            Channels::Blue,
            Channels::Alpha,
        ];
        const DIRECTIONS: [Direction; 3] = [
            Direction::HorizontalAndVertical,
            Direction::Horizontal,
            Direction::Vertical,
        ];
        const RADII: [u32; 7] = [0, 1, 2, 7, 32, 33, 165];

        let mut rng = TestRng(0x6d69_6e69_6d61_7821);
        for case in 0..3 {
            let width = rng.dimension();
            let height = rng.dimension();
            let input = random_canvas(&mut rng, width, height);

            for operation in OPERATIONS {
                for channels in CHANNELS {
                    for direction in DIRECTIONS {
                        for dont_shrink_edges in [false, true] {
                            for radius in RADII {
                                let params = MinimaxParams {
                                    operation,
                                    radius: radius as f32,
                                    channels,
                                    direction,
                                    dont_shrink_edges,
                                };
                                let actual = minimax_canvas(&input, radius, params);
                                let expected = naive_minimax_canvas(&input, radius, params);

                                assert_eq!(
                                    actual.data, expected.data,
                                    "case={case} size={width}x{height} operation={operation:?} channels={channels:?} direction={direction:?} dont_shrink_edges={dont_shrink_edges} radius={radius}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn debug_trace_reports_resolved_params_and_render_hashes() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [20, 40, 60, 255]);

        let trace = minimax_debug_trace(
            &input,
            &json!({
                "0001": { "value": 2 },
                "0002": { "value": 1.6 },
                "0003": { "value": 6 },
                "0004": { "value": 2 },
                "0005": { "value": 1 }
            }),
            0.0,
        );
        let expected_output = minimax_canvas(
            &input,
            2,
            MinimaxParams {
                operation: Operation::Maximum,
                radius: 1.6,
                channels: Channels::Alpha,
                direction: Direction::Horizontal,
                dont_shrink_edges: true,
            },
        );

        assert_eq!(trace.params.operation, "maximum");
        assert_eq!(trace.params.channels, "alpha");
        assert_eq!(trace.params.direction, "horizontal");
        assert_eq!(trace.params.radius, 1.6);
        assert_eq!(trace.params.kernel_radius, 2);
        assert!(trace.params.dont_shrink_edges);
        assert_eq!(trace.params.stage_count, 1);
        assert_eq!(trace.params.edge_policy, EDGE_POLICY_CLIP_TO_IMAGE_BOUNDS);
        assert_eq!(trace.hashes.input_rgba, canvas_debug_hash(&input));
        assert_eq!(
            trace.hashes.output_rgba,
            canvas_debug_hash(&expected_output)
        );
        assert_eq!(
            trace.hashes.first_stage_rgba,
            canvas_debug_hash(&expected_output)
        );
        assert_eq!(
            trace.hashes.second_stage_rgba,
            canvas_debug_hash(&expected_output)
        );
        assert_eq!(trace.alpha.input.nonzero_pixels, 1);
        assert_eq!(trace.alpha.output.nonzero_pixels, 3);
    }

    #[test]
    fn debug_trace_exposes_first_pass_for_two_axis_morphology() {
        let mut input = Canvas::transparent(3, 3);
        input.set_pixel(1, 1, [20, 40, 60, 255]);

        let trace = minimax_debug_trace(
            &input,
            &json!({ "0001": 2, "0002": 1, "0003": 6, "0004": 1 }),
            0.0,
        );

        assert_eq!(trace.params.stage_count, 1);
        assert_eq!(trace.alpha.input.nonzero_pixels, 1);
        assert_eq!(trace.alpha.first_pass.nonzero_pixels, 3);
        assert_eq!(trace.alpha.first_stage.nonzero_pixels, 9);
        assert_eq!(trace.alpha.output.nonzero_pixels, 9);
        assert_ne!(trace.hashes.first_pass_rgba, trace.hashes.first_stage_rgba);
    }

    #[test]
    fn debug_trace_reports_compound_stage_count_and_second_stage() {
        let mut hole = Canvas::new(3, 1, [0, 0, 0, 255]);
        hole.set_pixel(1, 0, [0, 0, 0, 0]);

        let trace = minimax_debug_trace(
            &hole,
            &json!({ "0001": 4, "0002": 1, "0003": 6, "0004": 2, "0005": 1 }),
            0.0,
        );

        assert_eq!(trace.params.operation, "maximum_then_minimum");
        assert_eq!(trace.params.stage_count, 2);
        assert_eq!(trace.alpha.first_stage.nonzero_pixels, 3);
        assert_eq!(trace.alpha.second_stage.nonzero_pixels, 3);
        assert_eq!(trace.alpha.output.nonzero_pixels, 3);
    }
}
