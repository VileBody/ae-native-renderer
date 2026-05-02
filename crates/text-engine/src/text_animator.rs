#[derive(Debug, Clone)]
pub struct TextAnimatorSpec {
    pub name: String,
    pub selector: TextSelector,
    pub properties: TextAnimatorProperties,
}

#[derive(Debug, Clone)]
pub enum TextSelector {
    Range(RangeSelector),
    Expression(ExpressionSelector),
}

#[derive(Debug, Clone)]
pub struct RangeSelector {
    pub start_percent: f32,
    pub end_percent: f32,
    pub based_on: BasedOn,
    pub smoothness: f32,
}

#[derive(Debug, Clone)]
pub struct ExpressionSelector {
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasedOn {
    Characters,
    Words,
    Lines,
}

#[derive(Debug, Clone, Default)]
pub struct TextAnimatorProperties {
    pub opacity: Option<f32>,
    pub position_3d: Option<[f32; 3]>,
    pub scale_3d: Option<[f32; 3]>,
    pub rotation_z: Option<f32>,
    pub blur: Option<[f32; 2]>,
}

pub fn range_selector_weight_stub(
    index: usize,
    total: usize,
    start_percent: f32,
    end_percent: f32,
) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let pos = ((index + 1) as f32 / total as f32) * 100.0;
    if pos >= start_percent && pos <= end_percent {
        1.0
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorShape {
    Square,
    RampUp,
    RampDown,
    Triangle,
    Round,
    Smooth,
}

impl Default for SelectorShape {
    fn default() -> Self {
        Self::Square
    }
}

#[derive(Debug, Clone)]
pub struct RangeSelectorV2 {
    pub start_percent: f32,
    pub end_percent: f32,
    pub based_on: BasedOn,
    pub shape: SelectorShape,
    pub smoothness: f32,
    pub randomize_order: bool,
    pub random_seed: u64,
    pub wiggly: Option<WigglySelector>,
}

impl Default for RangeSelectorV2 {
    fn default() -> Self {
        Self {
            start_percent: 0.0,
            end_percent: 100.0,
            based_on: BasedOn::Characters,
            shape: SelectorShape::Square,
            smoothness: 100.0,
            randomize_order: false,
            random_seed: 0,
            wiggly: None,
        }
    }
}

impl From<RangeSelector> for RangeSelectorV2 {
    fn from(selector: RangeSelector) -> Self {
        Self {
            start_percent: selector.start_percent,
            end_percent: selector.end_percent,
            based_on: selector.based_on,
            smoothness: selector.smoothness,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WigglySelector {
    pub amplitude_percent: f32,
    pub frequency_hz: f32,
    pub seed: u64,
    pub phase: f32,
}

impl Default for WigglySelector {
    fn default() -> Self {
        Self {
            amplitude_percent: 0.0,
            frequency_hz: 1.0,
            seed: 0,
            phase: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextUnit {
    pub based_on: BasedOn,
    pub index: usize,
    pub line_index: usize,
    pub word_index: Option<usize>,
    pub char_start: usize,
    pub char_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Debug, Clone)]
pub struct TextUnitWeight {
    pub unit: TextUnit,
    pub selector_index: usize,
    pub total: usize,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct TextUnitBlur {
    pub unit_index: usize,
    pub blur: [f32; 2],
}

#[derive(Debug, Clone)]
pub struct TextBlurPlan {
    pub per_unit_blur: Vec<TextUnitBlur>,
    pub max_blur: [f32; 2],
    pub layer_fallback_blur: [f32; 2],
    pub requires_per_unit_filter: bool,
}

pub fn text_units(text: &str, based_on: BasedOn) -> Vec<TextUnit> {
    match based_on {
        BasedOn::Characters => character_units(text),
        BasedOn::Words => word_units(text),
        BasedOn::Lines => line_units(text),
    }
}

pub fn evaluate_range_selector_v2(
    text: &str,
    selector: &RangeSelectorV2,
    time_seconds: f32,
) -> Vec<TextUnitWeight> {
    let units = text_units(text, selector.based_on);
    range_selector_weights(&units, selector, time_seconds)
}

pub fn range_selector_weights(
    units: &[TextUnit],
    selector: &RangeSelectorV2,
    time_seconds: f32,
) -> Vec<TextUnitWeight> {
    let total = units.len();
    if total == 0 {
        return Vec::new();
    }

    let selector_indices = if selector.randomize_order {
        deterministic_selector_order(total, selector.random_seed)
    } else {
        (0..total).collect()
    };

    units
        .iter()
        .cloned()
        .zip(selector_indices)
        .map(|(unit, selector_index)| {
            let mut weight = range_selector_weight_v2(selector_index, total, selector);
            if let Some(wiggly) = selector.wiggly {
                weight = apply_wiggly_weight(weight, &unit, wiggly, time_seconds);
            }
            TextUnitWeight {
                unit,
                selector_index,
                total,
                weight,
            }
        })
        .collect()
}

pub fn range_selector_weight_v2(
    selector_index: usize,
    total: usize,
    selector: &RangeSelectorV2,
) -> f32 {
    if total == 0 {
        return 0.0;
    }

    let start = selector
        .start_percent
        .min(selector.end_percent)
        .clamp(0.0, 100.0);
    let end = selector
        .start_percent
        .max(selector.end_percent)
        .clamp(0.0, 100.0);
    let pos = unit_center_percent(selector_index, total);
    let span = end - start;
    if span <= f32::EPSILON {
        return 0.0;
    }

    let raw = match selector.shape {
        SelectorShape::Square => {
            if pos >= start && pos <= end {
                1.0
            } else {
                0.0
            }
        }
        SelectorShape::RampUp => ((pos - start) / span).clamp(0.0, 1.0),
        SelectorShape::RampDown => (1.0 - ((pos - start) / span)).clamp(0.0, 1.0),
        SelectorShape::Triangle => {
            let t = ((pos - start) / span).clamp(0.0, 1.0);
            if pos < start || pos > end {
                0.0
            } else {
                1.0 - (2.0 * t - 1.0).abs()
            }
        }
        SelectorShape::Round => {
            let t = ((pos - start) / span).clamp(0.0, 1.0);
            if pos < start || pos > end {
                0.0
            } else {
                (std::f32::consts::PI * t).sin().clamp(0.0, 1.0)
            }
        }
        SelectorShape::Smooth => {
            let t = ((pos - start) / span).clamp(0.0, 1.0);
            if pos < start || pos > end {
                0.0
            } else if t <= 0.5 {
                smoothstep(t * 2.0)
            } else {
                smoothstep((1.0 - t) * 2.0)
            }
        }
    };

    smooth_selector_weight(raw, selector.smoothness)
}

pub fn deterministic_selector_order(total: usize, seed: u64) -> Vec<usize> {
    let mut keyed: Vec<(u64, usize)> = (0..total)
        .map(|index| (stable_hash(seed, index as u64, 0), index))
        .collect();
    keyed.sort_by_key(|(hash, index)| (*hash, *index));

    let mut order = vec![0; total];
    for (rank, (_, source_index)) in keyed.into_iter().enumerate() {
        order[source_index] = rank;
    }
    order
}

pub fn plan_blur_animator(weights: &[TextUnitWeight], blur: [f32; 2]) -> TextBlurPlan {
    let mut max_blur = [0.0_f32, 0.0_f32];
    let mut blur_sum = [0.0_f32, 0.0_f32];
    let mut per_unit_blur = Vec::with_capacity(weights.len());

    for unit_weight in weights {
        let unit_blur = [blur[0] * unit_weight.weight, blur[1] * unit_weight.weight];
        max_blur[0] = max_blur[0].max(unit_blur[0].abs());
        max_blur[1] = max_blur[1].max(unit_blur[1].abs());
        blur_sum[0] += unit_blur[0];
        blur_sum[1] += unit_blur[1];
        per_unit_blur.push(TextUnitBlur {
            unit_index: unit_weight.unit.index,
            blur: unit_blur,
        });
    }

    let layer_fallback_blur = if weights.is_empty() {
        [0.0, 0.0]
    } else {
        [
            blur_sum[0] / weights.len() as f32,
            blur_sum[1] / weights.len() as f32,
        ]
    };

    let requires_per_unit_filter = match weights.first() {
        Some(first) => weights
            .iter()
            .any(|unit_weight| (unit_weight.weight - first.weight).abs() > 0.0001),
        None => false,
    };

    TextBlurPlan {
        per_unit_blur,
        max_blur,
        layer_fallback_blur,
        requires_per_unit_filter,
    }
}

fn character_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let mut line_index = 0;
    for (char_index, (byte_start, ch)) in text.char_indices().enumerate() {
        if ch == '\n' {
            line_index += 1;
            continue;
        }
        if ch == '\r' {
            continue;
        }
        units.push(TextUnit {
            based_on: BasedOn::Characters,
            index: units.len(),
            line_index,
            word_index: None,
            char_start: char_index,
            char_end: char_index + 1,
            byte_start,
            byte_end: byte_start + ch.len_utf8(),
        });
    }
    units
}

fn word_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let mut line_index = 0;
    let mut char_index = 0;
    let mut word_start: Option<(usize, usize)> = None;
    let mut previous_was_cr = false;

    for (byte_start, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some((start_byte, start_char)) = word_start.take() {
                push_word_unit(
                    &mut units,
                    line_index,
                    start_char,
                    char_index,
                    start_byte,
                    byte_start,
                );
            }
            if ch == '\r' {
                line_index += 1;
                previous_was_cr = true;
            } else if ch == '\n' {
                if !previous_was_cr {
                    line_index += 1;
                }
                previous_was_cr = false;
            } else {
                previous_was_cr = false;
            }
        } else if word_start.is_none() {
            previous_was_cr = false;
            word_start = Some((byte_start, char_index));
        } else {
            previous_was_cr = false;
        }
        char_index += 1;
    }

    if let Some((start_byte, start_char)) = word_start {
        push_word_unit(
            &mut units,
            line_index,
            start_char,
            char_index,
            start_byte,
            text.len(),
        );
    }

    units
}

fn push_word_unit(
    units: &mut Vec<TextUnit>,
    line_index: usize,
    char_start: usize,
    char_end: usize,
    byte_start: usize,
    byte_end: usize,
) {
    let index = units.len();
    units.push(TextUnit {
        based_on: BasedOn::Words,
        index,
        line_index,
        word_index: Some(index),
        char_start,
        char_end,
        byte_start,
        byte_end,
    });
}

fn line_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let mut line_start_byte = 0;
    let mut line_start_char = 0;
    let mut char_index = 0;
    let mut chars = text.char_indices().peekable();

    while let Some((byte_start, ch)) = chars.next() {
        match ch {
            '\n' => {
                push_line_unit(
                    &mut units,
                    line_start_char,
                    char_index,
                    line_start_byte,
                    byte_start,
                );
                char_index += 1;
                line_start_byte = byte_start + ch.len_utf8();
                line_start_char = char_index;
            }
            '\r' => {
                push_line_unit(
                    &mut units,
                    line_start_char,
                    char_index,
                    line_start_byte,
                    byte_start,
                );
                char_index += 1;
                let mut next_line_byte = byte_start + ch.len_utf8();
                if let Some((next_byte, '\n')) = chars.peek().copied() {
                    chars.next();
                    char_index += 1;
                    next_line_byte = next_byte + '\n'.len_utf8();
                }
                line_start_byte = next_line_byte;
                line_start_char = char_index;
            }
            _ => {
                char_index += 1;
            }
        }
    }

    if text.is_empty() || line_start_byte <= text.len() {
        push_line_unit(
            &mut units,
            line_start_char,
            char_index,
            line_start_byte,
            text.len(),
        );
    }

    units
}

fn push_line_unit(
    units: &mut Vec<TextUnit>,
    char_start: usize,
    char_end: usize,
    byte_start: usize,
    byte_end: usize,
) {
    let index = units.len();
    units.push(TextUnit {
        based_on: BasedOn::Lines,
        index,
        line_index: index,
        word_index: None,
        char_start,
        char_end,
        byte_start,
        byte_end,
    });
}

fn unit_center_percent(index: usize, total: usize) -> f32 {
    ((index as f32 + 0.5) / total as f32) * 100.0
}

fn smooth_selector_weight(weight: f32, smoothness: f32) -> f32 {
    let smooth_amount = (smoothness / 100.0).clamp(0.0, 1.0);
    let smoothed = smoothstep(weight.clamp(0.0, 1.0));
    weight * (1.0 - smooth_amount) + smoothed * smooth_amount
}

fn apply_wiggly_weight(
    base_weight: f32,
    unit: &TextUnit,
    wiggly: WigglySelector,
    time_seconds: f32,
) -> f32 {
    let amplitude = (wiggly.amplitude_percent / 100.0).max(0.0);
    if amplitude <= f32::EPSILON {
        return base_weight.clamp(0.0, 1.0);
    }

    let frequency = wiggly.frequency_hz.max(0.0);
    let sample = time_seconds * frequency + wiggly.phase;
    let step = sample.floor();
    let t = sample - step;
    let step_a = step as i64;
    let step_b = step_a + 1;
    let a = signed_noise(wiggly.seed, unit.index as u64, step_a);
    let b = signed_noise(wiggly.seed, unit.index as u64, step_b);
    let noise = a + (b - a) * smoothstep(t);

    (base_weight + noise * amplitude).clamp(0.0, 1.0)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn signed_noise(seed: u64, index: u64, step: i64) -> f32 {
    hash_unit_float(stable_hash(seed, index, step as u64)) * 2.0 - 1.0
}

fn hash_unit_float(hash: u64) -> f32 {
    let mantissa = (hash >> 40) as u32;
    mantissa as f32 / 0x00ff_ffff as f32
}

fn stable_hash(seed: u64, index: u64, salt: u64) -> u64 {
    let mut value = seed
        ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ salt.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn builds_character_word_and_line_units_with_stable_indices() {
        let text = "Hi all\nRust now";

        let chars = text_units(text, BasedOn::Characters);
        assert_eq!(chars.len(), 14);
        assert_eq!(chars[0].index, 0);
        assert_eq!(chars[0].char_start, 0);
        assert_eq!(chars[5].line_index, 0);
        assert_eq!(chars[6].line_index, 1);
        assert_eq!(&text[chars[6].byte_start..chars[6].byte_end], "R");

        let words = text_units(text, BasedOn::Words);
        let word_text: Vec<&str> = words
            .iter()
            .map(|unit| &text[unit.byte_start..unit.byte_end])
            .collect();
        assert_eq!(word_text, vec!["Hi", "all", "Rust", "now"]);
        assert_eq!(words[2].index, 2);
        assert_eq!(words[2].line_index, 1);

        let lines = text_units(text, BasedOn::Lines);
        let line_text: Vec<&str> = lines
            .iter()
            .map(|unit| &text[unit.byte_start..unit.byte_end])
            .collect();
        assert_eq!(line_text, vec!["Hi all", "Rust now"]);
        assert_eq!(lines[1].char_start, 7);
    }

    #[test]
    fn range_weights_cover_characters_words_and_lines() {
        let mut selector = RangeSelectorV2 {
            start_percent: 0.0,
            end_percent: 50.0,
            smoothness: 0.0,
            ..RangeSelectorV2::default()
        };

        let char_weights: Vec<f32> = evaluate_range_selector_v2("abcd", &selector, 0.0)
            .into_iter()
            .map(|unit| unit.weight)
            .collect();
        assert_eq!(char_weights, vec![1.0, 1.0, 0.0, 0.0]);

        selector.based_on = BasedOn::Words;
        let word_weights: Vec<f32> =
            evaluate_range_selector_v2("one two three four", &selector, 0.0)
                .into_iter()
                .map(|unit| unit.weight)
                .collect();
        assert_eq!(word_weights, vec![1.0, 1.0, 0.0, 0.0]);

        selector.based_on = BasedOn::Lines;
        let line_weights: Vec<f32> = evaluate_range_selector_v2("a\nb\nc\nd", &selector, 0.0)
            .into_iter()
            .map(|unit| unit.weight)
            .collect();
        assert_eq!(line_weights, vec![1.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn range_shapes_and_smoothness_are_evaluable() {
        let mut selector = RangeSelectorV2 {
            start_percent: 0.0,
            end_percent: 100.0,
            shape: SelectorShape::RampUp,
            smoothness: 0.0,
            ..RangeSelectorV2::default()
        };

        assert_approx(range_selector_weight_v2(0, 4, &selector), 0.125);
        assert_approx(range_selector_weight_v2(3, 4, &selector), 0.875);

        selector.smoothness = 100.0;
        assert_approx(range_selector_weight_v2(0, 4, &selector), 0.04296875);
        assert_approx(range_selector_weight_v2(3, 4, &selector), 0.95703125);

        selector.shape = SelectorShape::Triangle;
        selector.smoothness = 0.0;
        let triangle: Vec<f32> = (0..4)
            .map(|index| range_selector_weight_v2(index, 4, &selector))
            .collect();
        assert_eq!(triangle, vec![0.25, 0.75, 0.75, 0.25]);

        selector.shape = SelectorShape::Round;
        assert!(
            range_selector_weight_v2(1, 4, &selector) > range_selector_weight_v2(0, 4, &selector)
        );
    }

    #[test]
    fn randomize_order_is_deterministic_and_seeded() {
        let a = deterministic_selector_order(12, 42);
        let b = deterministic_selector_order(12, 42);
        let c = deterministic_selector_order(12, 43);

        assert_eq!(a, b);
        assert_ne!(a, c);

        let mut sorted = a.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn randomize_order_changes_selector_weights_without_changing_units() {
        let selector = RangeSelectorV2 {
            start_percent: 0.0,
            end_percent: 25.0,
            smoothness: 0.0,
            randomize_order: true,
            random_seed: 7,
            ..RangeSelectorV2::default()
        };

        let first = evaluate_range_selector_v2("abcdefgh", &selector, 0.0);
        let second = evaluate_range_selector_v2("abcdefgh", &selector, 0.0);
        assert_eq!(
            first.iter().map(|unit| unit.unit.index).collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        assert_eq!(
            first.iter().map(|unit| unit.selector_index).collect::<Vec<_>>(),
            second.iter().map(|unit| unit.selector_index).collect::<Vec<_>>()
        );
        assert_eq!(
            first.iter().map(|unit| unit.weight).collect::<Vec<_>>(),
            second.iter().map(|unit| unit.weight).collect::<Vec<_>>()
        );
    }

    #[test]
    fn wiggly_selector_is_deterministic_for_same_seed_and_time() {
        let selector = RangeSelectorV2 {
            start_percent: 0.0,
            end_percent: 100.0,
            smoothness: 0.0,
            wiggly: Some(WigglySelector {
                amplitude_percent: 30.0,
                frequency_hz: 2.0,
                seed: 99,
                phase: 0.25,
            }),
            ..RangeSelectorV2::default()
        };

        let first = evaluate_range_selector_v2("abcd", &selector, 0.5);
        let second = evaluate_range_selector_v2("abcd", &selector, 0.5);
        let later = evaluate_range_selector_v2("abcd", &selector, 0.75);

        assert_eq!(
            first.iter().map(|unit| unit.weight).collect::<Vec<_>>(),
            second.iter().map(|unit| unit.weight).collect::<Vec<_>>()
        );
        assert_ne!(
            first.iter().map(|unit| unit.weight).collect::<Vec<_>>(),
            later.iter().map(|unit| unit.weight).collect::<Vec<_>>()
        );
    }

    #[test]
    fn blur_plan_keeps_per_unit_data_and_layer_fallback() {
        let selector = RangeSelectorV2 {
            start_percent: 0.0,
            end_percent: 100.0,
            shape: SelectorShape::RampUp,
            smoothness: 0.0,
            ..RangeSelectorV2::default()
        };
        let weights = evaluate_range_selector_v2("abcd", &selector, 0.0);
        let plan = plan_blur_animator(&weights, [8.0, 2.0]);

        assert_eq!(plan.per_unit_blur.len(), 4);
        assert_approx(plan.per_unit_blur[0].blur[0], 1.0);
        assert_approx(plan.max_blur[0], 7.0);
        assert!(plan.requires_per_unit_filter);
        assert!(plan.layer_fallback_blur[0] > 0.0);
    }
}
