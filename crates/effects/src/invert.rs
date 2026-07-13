use crate::{param_f32_at_any, Effect, EffectContext};
use raster_cpu::Canvas;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct Invert;

impl Effect for Invert {
    fn match_name(&self) -> &'static str {
        "ADBE Invert"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = InvertParams::from_json(params, ctx.time);
        let original_mix = normalized_percent(params.blend_with_original);
        if original_mix >= 1.0 {
            return Ok(input.clone());
        }

        let mut output = input.clone();
        for (source, destination) in input
            .data
            .chunks_exact(4)
            .zip(output.data.chunks_exact_mut(4))
        {
            let source = [source[0], source[1], source[2], source[3]];
            let inverted = invert_pixel(source, params.channel);
            for channel in 0..4 {
                destination[channel] = mix_u8(inverted[channel], source[channel], original_mix);
            }
        }
        Ok(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InvertParams {
    pub channel: InvertChannel,
    pub blend_with_original: f32,
}

impl InvertParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        Self {
            channel: InvertChannel::from_params(params, time),
            blend_with_original: param_f32_at_any(
                params,
                &[
                    "blend_with_original",
                    "blendWithOriginal",
                    "Blend With Original",
                    "0002",
                    "ADBE Invert-0002",
                ],
                time,
                0.0,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InvertChannel {
    Rgb,
    Red,
    Green,
    Blue,
    Hls,
    Hue,
    Lightness,
    Saturation,
    Yiq,
    Luminance,
    InPhaseChrominance,
    QuadratureChrominance,
    Alpha,
}

impl InvertChannel {
    fn from_params(params: &Value, time: f64) -> Self {
        let names = ["channel", "Channel", "channels", "0001", "ADBE Invert-0001"];
        let Some(value) = discrete_param_value_at(params, &names, time) else {
            return Self::Rgb;
        };

        if let Some(text) = value.as_str() {
            return Self::from_text(text).unwrap_or(Self::Rgb);
        }
        value_as_i64(value)
            .map(Self::from_ae_number)
            .unwrap_or(Self::Rgb)
    }

    fn from_text(text: &str) -> Option<Self> {
        let normalized = normalize_label(text);
        let channel = match normalized.as_str() {
            "rgb" | "color" | "color channels" => Self::Rgb,
            "red" | "red channel" | "r" => Self::Red,
            "green" | "green channel" | "g" => Self::Green,
            "blue" | "blue channel" | "b" => Self::Blue,
            "hls" => Self::Hls,
            "hue" | "h" => Self::Hue,
            "lightness" | "light" | "l" => Self::Lightness,
            "saturation" | "sat" | "s" => Self::Saturation,
            "yiq" => Self::Yiq,
            "luminance" | "luma" | "y" => Self::Luminance,
            "in phase chrominance" | "inphase chrominance" | "in phase" | "i" => {
                Self::InPhaseChrominance
            }
            "quadrature chrominance" | "quadrature" | "q" => Self::QuadratureChrominance,
            "alpha" | "alpha channel" | "a" => Self::Alpha,
            _ => return text.trim().parse::<i64>().ok().map(Self::from_ae_number),
        };
        Some(channel)
    }

    fn from_ae_number(number: i64) -> Self {
        // AE's popup values include separators between the RGB, HLS, YIQ, and Alpha groups.
        match number {
            1 => Self::Rgb,
            2 => Self::Red,
            3 => Self::Green,
            4 => Self::Blue,
            6 => Self::Hls,
            7 => Self::Hue,
            8 => Self::Lightness,
            9 => Self::Saturation,
            11 => Self::Yiq,
            12 => Self::Luminance,
            13 => Self::InPhaseChrominance,
            14 => Self::QuadratureChrominance,
            16 => Self::Alpha,
            _ => Self::Rgb,
        }
    }
}

fn discrete_param_value_at<'a>(params: &'a Value, names: &[&str], time: f64) -> Option<&'a Value> {
    for name in names {
        let Some(raw) = params.get(*name) else {
            continue;
        };
        if let Some(keyframes) = raw.get("keyframes").and_then(Value::as_array) {
            if let Some(keyframe) = discrete_keyframe_at(keyframes, time) {
                if let Some(value) = keyframe.get("v").or_else(|| keyframe.get("value")) {
                    return Some(value);
                }
            }
        }
        return raw.get("value").or(Some(raw));
    }
    None
}

fn discrete_keyframe_at(keyframes: &[Value], time: f64) -> Option<&Value> {
    let first = keyframes.first()?;
    let mut selected = first;
    for keyframe in keyframes {
        let Some(key_time) = keyframe.get("t").and_then(Value::as_f64) else {
            continue;
        };
        if key_time > time {
            break;
        }
        selected = keyframe;
    }
    Some(selected)
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_f64().map(|number| number.round() as i64))
}

fn normalize_label(text: &str) -> String {
    text.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_percent(percent: f32) -> f32 {
    if percent.is_finite() {
        (percent / 100.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn mix_u8(processed: u8, original: u8, original_mix: f32) -> u8 {
    (processed as f32 + (original as f32 - processed as f32) * original_mix)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn invert_pixel(pixel: [u8; 4], channel: InvertChannel) -> [u8; 4] {
    match channel {
        InvertChannel::Rgb | InvertChannel::Yiq => {
            [255 - pixel[0], 255 - pixel[1], 255 - pixel[2], pixel[3]]
        }
        InvertChannel::Red => [255 - pixel[0], pixel[1], pixel[2], pixel[3]],
        InvertChannel::Green => [pixel[0], 255 - pixel[1], pixel[2], pixel[3]],
        InvertChannel::Blue => [pixel[0], pixel[1], 255 - pixel[2], pixel[3]],
        InvertChannel::Alpha => [pixel[0], pixel[1], pixel[2], 255 - pixel[3]],
        InvertChannel::Hls
        | InvertChannel::Hue
        | InvertChannel::Lightness
        | InvertChannel::Saturation => invert_hls(pixel, channel),
        InvertChannel::Luminance
        | InvertChannel::InPhaseChrominance
        | InvertChannel::QuadratureChrominance => invert_yiq(pixel, channel),
    }
}

fn invert_hls(pixel: [u8; 4], channel: InvertChannel) -> [u8; 4] {
    let rgb = rgb_u8_to_unit(pixel);
    let [mut hue, mut lightness, mut saturation] = rgb_to_hls(rgb);
    match channel {
        InvertChannel::Hls => {
            hue = 1.0 - hue;
            lightness = 1.0 - lightness;
            saturation = 1.0 - saturation;
        }
        InvertChannel::Hue => hue = 1.0 - hue,
        InvertChannel::Lightness => lightness = 1.0 - lightness,
        InvertChannel::Saturation => saturation = 1.0 - saturation,
        _ => unreachable!("invert_hls requires an HLS channel"),
    }
    rgb_unit_to_u8(hls_to_rgb([hue, lightness, saturation]), pixel[3])
}

fn invert_yiq(pixel: [u8; 4], channel: InvertChannel) -> [u8; 4] {
    let [mut luminance, mut in_phase, mut quadrature] = rgb_to_yiq(rgb_u8_to_unit(pixel));
    match channel {
        InvertChannel::Luminance => luminance = 1.0 - luminance,
        InvertChannel::InPhaseChrominance => in_phase = -in_phase,
        InvertChannel::QuadratureChrominance => quadrature = -quadrature,
        _ => unreachable!("invert_yiq requires a YIQ channel"),
    }
    rgb_unit_to_u8(yiq_to_rgb([luminance, in_phase, quadrature]), pixel[3])
}

fn rgb_u8_to_unit(pixel: [u8; 4]) -> [f32; 3] {
    [
        pixel[0] as f32 / 255.0,
        pixel[1] as f32 / 255.0,
        pixel[2] as f32 / 255.0,
    ]
}

fn rgb_unit_to_u8(rgb: [f32; 3], alpha: u8) -> [u8; 4] {
    [
        unit_to_u8(rgb[0]),
        unit_to_u8(rgb[1]),
        unit_to_u8(rgb[2]),
        alpha,
    ]
}

fn unit_to_u8(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

fn rgb_to_hls([red, green, blue]: [f32; 3]) -> [f32; 3] {
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let lightness = (maximum + minimum) * 0.5;
    let delta = maximum - minimum;
    if delta <= f32::EPSILON {
        return [0.0, lightness, 0.0];
    }

    let saturation = if lightness <= 0.5 {
        delta / (maximum + minimum)
    } else {
        delta / (2.0 - maximum - minimum)
    };
    let hue_sector = if maximum == red {
        (green - blue) / delta
    } else if maximum == green {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    };
    [(hue_sector / 6.0).rem_euclid(1.0), lightness, saturation]
}

fn hls_to_rgb([hue, lightness, saturation]: [f32; 3]) -> [f32; 3] {
    if saturation <= f32::EPSILON {
        return [lightness; 3];
    }
    let maximum = if lightness <= 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let minimum = 2.0 * lightness - maximum;
    [
        hue_to_rgb(minimum, maximum, hue + 1.0 / 3.0),
        hue_to_rgb(minimum, maximum, hue),
        hue_to_rgb(minimum, maximum, hue - 1.0 / 3.0),
    ]
}

fn hue_to_rgb(minimum: f32, maximum: f32, hue: f32) -> f32 {
    let hue = hue.rem_euclid(1.0);
    if hue < 1.0 / 6.0 {
        minimum + (maximum - minimum) * 6.0 * hue
    } else if hue < 0.5 {
        maximum
    } else if hue < 2.0 / 3.0 {
        minimum + (maximum - minimum) * (2.0 / 3.0 - hue) * 6.0
    } else {
        minimum
    }
}

fn rgb_to_yiq([red, green, blue]: [f32; 3]) -> [f32; 3] {
    [
        0.299 * red + 0.587 * green + 0.114 * blue,
        0.596 * red - 0.275 * green - 0.321 * blue,
        0.212 * red - 0.523 * green + 0.311 * blue,
    ]
}

fn yiq_to_rgb([luminance, in_phase, quadrature]: [f32; 3]) -> [f32; 3] {
    [
        luminance + 0.956 * in_phase + 0.621 * quadrature,
        luminance - 0.272 * in_phase - 0.647 * quadrature,
        luminance - 1.106 * in_phase + 1.703 * quadrature,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn render(pixel: [u8; 4], params: Value) -> [u8; 4] {
        Invert
            .render(
                &Canvas::new(1, 1, pixel),
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &params,
            )
            .unwrap()
            .pixel(0, 0)
    }

    #[test]
    fn default_inverts_rgb_and_preserves_alpha() {
        assert_eq!(render([10, 20, 30, 40], json!({})), [245, 235, 225, 40]);
    }

    #[test]
    fn ae_numbered_popup_values_include_group_separators() {
        let cases = [
            (1, InvertChannel::Rgb),
            (2, InvertChannel::Red),
            (3, InvertChannel::Green),
            (4, InvertChannel::Blue),
            (6, InvertChannel::Hls),
            (7, InvertChannel::Hue),
            (8, InvertChannel::Lightness),
            (9, InvertChannel::Saturation),
            (11, InvertChannel::Yiq),
            (12, InvertChannel::Luminance),
            (13, InvertChannel::InPhaseChrominance),
            (14, InvertChannel::QuadratureChrominance),
            (16, InvertChannel::Alpha),
        ];
        for (number, expected) in cases {
            let params = InvertParams::from_json(&json!({ "0001": { "value": number } }), 0.0);
            assert_eq!(params.channel, expected, "popup value {number}");
        }
    }

    #[test]
    fn named_and_full_match_name_params_are_accepted() {
        let params = InvertParams::from_json(
            &json!({
                "ADBE Invert-0001": { "value": "Quadrature Chrominance" },
                "ADBE Invert-0002": { "value": 35 }
            }),
            0.0,
        );
        assert_eq!(params.channel, InvertChannel::QuadratureChrominance);
        assert_eq!(params.blend_with_original, 35.0);

        let params = InvertParams::from_json(
            &json!({ "channel": "alpha channel", "blendWithOriginal": 25 }),
            0.0,
        );
        assert_eq!(params.channel, InvertChannel::Alpha);
        assert_eq!(params.blend_with_original, 25.0);
    }

    #[test]
    fn individual_rgb_and_alpha_modes_change_only_selected_lane() {
        let source = [10, 20, 30, 40];
        assert_eq!(render(source, json!({ "0001": 2 })), [245, 20, 30, 40]);
        assert_eq!(render(source, json!({ "0001": 3 })), [10, 235, 30, 40]);
        assert_eq!(render(source, json!({ "0001": 4 })), [10, 20, 225, 40]);
        assert_eq!(render(source, json!({ "0001": 16 })), [10, 20, 30, 215]);
    }

    #[test]
    fn hls_component_modes_use_calculated_channels() {
        assert_eq!(
            render([255, 0, 0, 77], json!({ "0001": "saturation" })),
            [128, 128, 128, 77]
        );
        assert_eq!(
            render([64, 64, 64, 77], json!({ "0001": "lightness" })),
            [191, 191, 191, 77]
        );
        assert_ne!(
            render([210, 80, 30, 77], json!({ "0001": "hue" }))[0..3],
            [210, 80, 30]
        );
    }

    #[test]
    fn yiq_modes_invert_signed_chrominance_and_luminance() {
        let source = [210, 80, 30, 77];
        assert_eq!(render(source, json!({ "0001": 11 })), [45, 175, 225, 77]);

        let luminance = render(source, json!({ "0001": 12 }));
        let in_phase = render(source, json!({ "0001": 13 }));
        let quadrature = render(source, json!({ "0001": 14 }));
        assert_eq!(luminance[3], 77);
        assert_eq!(in_phase[3], 77);
        assert_eq!(quadrature[3], 77);
        assert_ne!(luminance[0..3], source[0..3]);
        assert_ne!(in_phase[0..3], source[0..3]);
        assert_ne!(quadrature[0..3], source[0..3]);
        assert_ne!(in_phase[0..3], quadrature[0..3]);
    }

    #[test]
    fn blend_with_original_is_clamped_and_animated() {
        let source = [10, 20, 30, 40];
        assert_eq!(
            render(source, json!({ "Blend With Original": 50 })),
            [128, 128, 128, 40]
        );
        assert_eq!(render(source, json!({ "0002": 100 })), source);
        assert_eq!(render(source, json!({ "0002": 150 })), source);

        let params = json!({
            "0002": {
                "keyframes": [
                    { "t": 0.0, "v": 0.0 },
                    { "t": 1.0, "v": 100.0 }
                ]
            }
        });
        let sampled = InvertParams::from_json(&params, 0.5);
        assert_eq!(sampled.blend_with_original, 50.0);
    }

    #[test]
    fn popup_keyframes_are_sampled_discretely() {
        let params = json!({
            "0001": {
                "value": 1,
                "keyframes": [
                    { "t": 0.0, "v": 2 },
                    { "t": 1.0, "v": 16 }
                ]
            }
        });
        assert_eq!(
            InvertParams::from_json(&params, 0.5).channel,
            InvertChannel::Red
        );
        assert_eq!(
            InvertParams::from_json(&params, 1.0).channel,
            InvertChannel::Alpha
        );
    }
}
