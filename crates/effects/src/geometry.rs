use crate::{param_f32_at, Effect};
use raster_cpu::Canvas;
use serde_json::Value;

const NATIVE_LAYER_SPACE_ORIGIN_PARAM: &str = "__native_layer_space_origin";

#[derive(Debug, Default)]
pub struct Geometry2;

impl Effect for Geometry2 {
    fn match_name(&self) -> &'static str {
        "ADBE Geometry2"
    }

    fn render(
        &self,
        input: &Canvas,
        _ctx: &crate::EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let layer_origin = geometry2_layer_space_origin(input, params);
        Ok(transform_canvas(
            input,
            Geometry2Params::from_json(input, params, _ctx.time),
            layer_origin,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Geometry2Params {
    anchor: (f32, f32),
    position: (f32, f32),
    scale: (f32, f32),
    rotation: f32,
    skew: f32,
    skew_axis: f32,
    pixel_aspect: f32,
    sampler_mode: Geometry2SamplerMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Geometry2SamplerMode {
    Bilinear,
    Bicubic,
}

impl Geometry2SamplerMode {
    fn from_sampling_value(value: f32) -> Self {
        if !value.is_finite() {
            return Self::Bilinear;
        }
        match value.round() as i32 {
            2 => Self::Bicubic,
            _ => Self::Bilinear,
        }
    }

    fn sampling_value(self) -> i32 {
        match self {
            Self::Bilinear => 1,
            Self::Bicubic => 2,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Bilinear => GEOMETRY2_BILINEAR_SAMPLER_MODE,
            Self::Bicubic => GEOMETRY2_BICUBIC_SAMPLER_MODE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry2DebugData {
    pub raw_params: Value,
    pub property_mapping: Geometry2PropertyMapping,
    pub resolved: Geometry2ResolvedParams,
    pub forward_matrix: [[f32; 3]; 3],
    pub inverse_matrix: [[f32; 3]; 3],
    pub samples: Vec<Geometry2Sample>,
    pub sampler_mode: &'static str,
    pub edge_policy: &'static str,
    pub out_of_bounds_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry2PropertyMapping {
    pub payload_0003: Geometry2PropertyMappingEntry,
    pub payload_0004: Geometry2PropertyMappingEntry,
    pub payload_0005: Geometry2PropertyMappingEntry,
    pub payload_0008: Geometry2PropertyMappingEntry,
    pub payload_0009: Geometry2PropertyMappingEntry,
    pub payload_0012: Geometry2PropertyMappingEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry2PropertyMappingEntry {
    pub payload_key: &'static str,
    pub match_name: &'static str,
    pub ui_label: &'static str,
    pub native_role: &'static str,
    pub present: bool,
    pub raw_value: Option<Value>,
    pub note: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry2ResolvedParams {
    pub anchor: [f32; 2],
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub skew: f32,
    pub skew_axis: f32,
    pub pixel_aspect: f32,
    pub sampling: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry2Sample {
    pub output_xy: [u32; 2],
    pub source_uv: [f32; 2],
    pub sample_xy: Option<[u32; 2]>,
    pub sample_rgba: Option<[u8; 4]>,
    pub out_of_bounds: bool,
}

impl Geometry2Params {
    fn identity(input: &Canvas) -> Self {
        let center = (input.width as f32 * 0.5, input.height as f32 * 0.5);
        Self {
            anchor: center,
            position: center,
            scale: (100.0, 100.0),
            rotation: 0.0,
            skew: 0.0,
            skew_axis: 0.0,
            pixel_aspect: 1.0,
            sampler_mode: Geometry2SamplerMode::Bilinear,
        }
    }

    pub(crate) fn from_json(input: &Canvas, params: &Value, time: f64) -> Self {
        let mut transform = Self::identity(input);
        transform.anchor = point_param(params, &["anchor", "anchorPoint", "Anchor Point", "0001"])
            .unwrap_or(transform.anchor);
        transform.position =
            point_param(params, &["position", "Position", "0002"]).unwrap_or(transform.position);
        transform.scale = scale_param(params, time).unwrap_or(transform.scale);
        transform.rotation = scalar_param(
            params,
            &["rotation", "Rotation", "0008", "ADBE Geometry2-0007"],
            time,
            0.0,
        );
        transform.skew = scalar_param(
            params,
            &["skew", "Skew", "0006", "ADBE Geometry2-0005"],
            time,
            0.0,
        );
        transform.skew_axis = scalar_param(
            params,
            &[
                "skewAxis",
                "skew_axis",
                "Skew Axis",
                "0007",
                "ADBE Geometry2-0006",
            ],
            time,
            0.0,
        );
        transform.pixel_aspect = pixel_aspect_param(params, time);
        transform.sampler_mode = Geometry2SamplerMode::from_sampling_value(scalar_param(
            params,
            &["sampling", "Sampling", "0012", "ADBE Geometry2-0012"],
            time,
            1.0,
        ));
        transform
    }

    fn is_identity(self) -> bool {
        nearly_eq(self.anchor.0, self.position.0)
            && nearly_eq(self.anchor.1, self.position.1)
            && nearly_eq(self.scale.0, 100.0)
            && nearly_eq(self.scale.1, 100.0)
            && nearly_eq(self.rotation, 0.0)
            && nearly_eq(self.skew, 0.0)
    }

    fn resolved(self) -> Geometry2ResolvedParams {
        Geometry2ResolvedParams {
            anchor: [self.anchor.0, self.anchor.1],
            position: [self.position.0, self.position.1],
            scale: [self.scale.0, self.scale.1],
            rotation: self.rotation,
            skew: self.skew,
            skew_axis: self.skew_axis,
            pixel_aspect: self.pixel_aspect,
            sampling: self.sampler_mode.sampling_value(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Geometry2Mapping {
    inverse_matrix: [[f32; 3]; 3],
    forward_matrix: [[f32; 3]; 3],
}

impl Geometry2Mapping {
    fn from_params(transform: Geometry2Params, layer_origin: (f32, f32)) -> Self {
        let sx = effective_scale(transform.scale.0);
        let sy = effective_scale(transform.scale.1);
        let pixel_aspect = effective_pixel_aspect(transform.pixel_aspect);
        let mut local_forward_matrix = identity_matrix();
        local_forward_matrix = mat_mul(
            translation_matrix(-transform.anchor.0, -transform.anchor.1),
            local_forward_matrix,
        );
        local_forward_matrix = mat_mul(scale_matrix(pixel_aspect, 1.0), local_forward_matrix);
        local_forward_matrix = mat_mul(scale_matrix(sx, sy), local_forward_matrix);
        local_forward_matrix = mat_mul(rotation_matrix(transform.rotation), local_forward_matrix);
        if !nearly_eq(transform.skew, 0.0) {
            local_forward_matrix =
                mat_mul(rotation_matrix(transform.skew_axis), local_forward_matrix);
            local_forward_matrix = mat_mul(
                skew_x_matrix(-transform.skew.to_radians().tan()),
                local_forward_matrix,
            );
            local_forward_matrix =
                mat_mul(rotation_matrix(-transform.skew_axis), local_forward_matrix);
        }
        local_forward_matrix = mat_mul(scale_matrix(1.0 / pixel_aspect, 1.0), local_forward_matrix);
        local_forward_matrix = mat_mul(
            translation_matrix(transform.position.0, transform.position.1),
            local_forward_matrix,
        );

        let forward_matrix = offset_matrix(local_forward_matrix, layer_origin);
        let inverse_matrix = invert_affine(forward_matrix).unwrap_or_else(identity_matrix);

        Self {
            inverse_matrix,
            forward_matrix,
        }
    }

    fn source_uv(self, output_x: f32, output_y: f32) -> (f32, f32) {
        (
            self.inverse_matrix[0][0] * output_x
                + self.inverse_matrix[0][1] * output_y
                + self.inverse_matrix[0][2],
            self.inverse_matrix[1][0] * output_x
                + self.inverse_matrix[1][1] * output_y
                + self.inverse_matrix[1][2],
        )
    }
}

pub fn geometry2_debug_data(input: &Canvas, params: &Value, time: f64) -> Geometry2DebugData {
    let transform = Geometry2Params::from_json(input, params, time);
    let layer_origin = geometry2_layer_space_origin(input, params);
    let mapping = Geometry2Mapping::from_params(transform, layer_origin);
    let samples = geometry_probe_points(input)
        .into_iter()
        .map(|[x, y]| {
            let source_uv = mapping.source_uv(x as f32, y as f32);
            let sample_xy = rounded_sample(input, source_uv);
            let out_of_bounds = geometry2_out_of_bounds(input, source_uv, transform.sampler_mode);
            let sample_rgba = if out_of_bounds {
                None
            } else {
                Some(sample_geometry2(input, source_uv, transform.sampler_mode))
            };
            Geometry2Sample {
                output_xy: [x, y],
                source_uv: [source_uv.0, source_uv.1],
                sample_xy,
                sample_rgba,
                out_of_bounds,
            }
        })
        .collect();
    let out_of_bounds_count = count_oob(input, mapping, transform.sampler_mode);

    Geometry2DebugData {
        raw_params: params.clone(),
        property_mapping: geometry2_property_mapping(params),
        resolved: transform.resolved(),
        forward_matrix: mapping.forward_matrix,
        inverse_matrix: mapping.inverse_matrix,
        samples,
        sampler_mode: transform.sampler_mode.label(),
        edge_policy: "partial_footprint_transparent_out_of_bounds",
        out_of_bounds_count,
    }
}

fn geometry2_property_mapping(params: &Value) -> Geometry2PropertyMapping {
    Geometry2PropertyMapping {
        payload_0003: geometry2_property_mapping_entry(
            params,
            "0003",
            "ADBE Geometry2-0011",
            "Uniform Scale",
            "uniform_scale_fallback",
            "Payload key 0003 is the uniform scale checkbox/value namespace; axis controls take priority when present.",
        ),
        payload_0004: geometry2_property_mapping_entry(
            params,
            "0004",
            "ADBE Geometry2-0003",
            "Scale Height",
            "scale_height",
            "Payload key 0004 is Scale Height, not Scale Width.",
        ),
        payload_0005: geometry2_property_mapping_entry(
            params,
            "0005",
            "ADBE Geometry2-0004",
            "Scale Width",
            "scale_width",
            "Payload key 0005 is Scale Width.",
        ),
        payload_0008: geometry2_property_mapping_entry(
            params,
            "0008",
            "ADBE Geometry2-0007",
            "Rotation",
            "rotation_degrees",
            "Payload key 0008 is Rotation; matchName ADBE Geometry2-0008 is the separate opacity slot.",
        ),
        payload_0009: geometry2_property_mapping_entry(
            params,
            "0009",
            "ADBE Geometry2-0008",
            "Opacity",
            "opacity_not_applied_by_current_native_geometry2",
            "Recorded so payload 0008 is not confused with matchName ADBE Geometry2-0008 opacity.",
        ),
        payload_0012: geometry2_property_mapping_entry(
            params,
            "0012",
            "ADBE Geometry2-0012",
            "Sampling",
            "sampler_mode",
            "Frida CPU-wrapper dump confirmed PF_ParamDef index 12; low s32 value maps 1 to Bilinear and 2 to Bicubic.",
        ),
    }
}

fn geometry2_property_mapping_entry(
    params: &Value,
    payload_key: &'static str,
    match_name: &'static str,
    ui_label: &'static str,
    native_role: &'static str,
    note: &'static str,
) -> Geometry2PropertyMappingEntry {
    let raw_value = params.get(payload_key).cloned();
    Geometry2PropertyMappingEntry {
        payload_key,
        match_name,
        ui_label,
        native_role,
        present: raw_value.is_some(),
        raw_value,
        note,
    }
}

fn transform_canvas(
    input: &Canvas,
    transform: Geometry2Params,
    layer_origin: (f32, f32),
) -> Canvas {
    if input.width == 0 || input.height == 0 || transform.is_identity() {
        return input.clone();
    }

    let mut output = Canvas::transparent(input.width, input.height);
    let mapping = Geometry2Mapping::from_params(transform, layer_origin);

    for y in 0..input.height {
        for x in 0..input.width {
            let source_uv = mapping.source_uv(x as f32, y as f32);
            if geometry2_out_of_bounds(input, source_uv, transform.sampler_mode) {
                continue;
            }
            output.set_pixel(
                x,
                y,
                sample_geometry2(input, source_uv, transform.sampler_mode),
            );
        }
    }

    output
}

fn scale_param(params: &Value, time: f64) -> Option<(f32, f32)> {
    if let Some(point) = point_param(params, &["scale", "Scale"]) {
        return Some(point);
    }
    let width_scale = scalar_param_opt(
        params,
        &["scaleX", "Scale Width", "0005", "ADBE Geometry2-0004"],
        time,
    );
    let height_scale = scalar_param_opt(
        params,
        &["scaleY", "Scale Height", "0004", "ADBE Geometry2-0003"],
        time,
    );
    if width_scale.is_some() || height_scale.is_some() {
        let uniform_scale_enabled = scalar_param_opt(
            params,
            &[
                "uniformScale",
                "Uniform Scale",
                "0003",
                "ADBE Geometry2-0011",
            ],
            time,
        )
        .map(|value| value != 0.0)
        .unwrap_or(false);
        return match (width_scale, height_scale, uniform_scale_enabled) {
            (Some(x), Some(y), _) => Some((x, y)),
            (Some(x), None, _) => Some((x, x)),
            (None, Some(y), _) => Some((y, y)),
            (None, None, _) => None,
        };
    }
    if has_param(params, "0003") {
        let scale = param_f32_at(params, "0003", time, 100.0);
        return Some((scale, scale));
    }
    None
}

fn pixel_aspect_param(params: &Value, time: f64) -> f32 {
    let value = scalar_param(
        params,
        &[
            "pixelAspect",
            "pixel_aspect",
            "pixelAspectRatio",
            "pixel_aspect_ratio",
        ],
        time,
        1.0,
    );
    effective_pixel_aspect(value)
}

fn effective_scale(scale: f32) -> f32 {
    if scale.abs() < f32::EPSILON {
        1.0
    } else {
        scale / 100.0
    }
}

fn effective_pixel_aspect(pixel_aspect: f32) -> f32 {
    if pixel_aspect.is_finite() && pixel_aspect.abs() >= f32::EPSILON {
        pixel_aspect
    } else {
        1.0
    }
}

fn identity_matrix() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

fn translation_matrix(x: f32, y: f32) -> [[f32; 3]; 3] {
    [[1.0, 0.0, x], [0.0, 1.0, y], [0.0, 0.0, 1.0]]
}

fn scale_matrix(x: f32, y: f32) -> [[f32; 3]; 3] {
    [[x, 0.0, 0.0], [0.0, y, 0.0], [0.0, 0.0, 1.0]]
}

fn rotation_matrix(degrees: f32) -> [[f32; 3]; 3] {
    let radians = degrees.rem_euclid(360.0).to_radians();
    if radians.abs() < f32::EPSILON {
        return identity_matrix();
    }
    let (sin, cos) = radians.sin_cos();
    [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]]
}

fn skew_x_matrix(amount: f32) -> [[f32; 3]; 3] {
    [[1.0, amount, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

fn mat_mul(left: [[f32; 3]; 3], right: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for row in 0..3 {
        for col in 0..3 {
            out[row][col] = left[row][0] * right[0][col]
                + left[row][1] * right[1][col]
                + left[row][2] * right[2][col];
        }
    }
    out
}

fn invert_affine(matrix: [[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let a = matrix[0][0];
    let b = matrix[0][1];
    let tx = matrix[0][2];
    let c = matrix[1][0];
    let d = matrix[1][1];
    let ty = matrix[1][2];
    let det = a * d - b * c;
    if det.abs() < f32::EPSILON {
        return None;
    }
    let inv_det = 1.0 / det;
    Some([
        [d * inv_det, -b * inv_det, (b * ty - d * tx) * inv_det],
        [-c * inv_det, a * inv_det, (c * tx - a * ty) * inv_det],
        [0.0, 0.0, 1.0],
    ])
}

fn offset_matrix(local_matrix: [[f32; 3]; 3], origin: (f32, f32)) -> [[f32; 3]; 3] {
    let origin_x = origin.0;
    let origin_y = origin.1;
    let mut matrix = local_matrix;
    matrix[0][2] = local_matrix[0][2] + origin_x
        - local_matrix[0][0] * origin_x
        - local_matrix[0][1] * origin_y;
    matrix[1][2] = local_matrix[1][2] + origin_y
        - local_matrix[1][0] * origin_x
        - local_matrix[1][1] * origin_y;
    matrix
}

fn layer_space_origin(input: &Canvas) -> (f32, f32) {
    let mut min_x = input.width;
    let mut min_y = input.height;
    for y in 0..input.height {
        for x in 0..input.width {
            if input.pixel(x, y)[3] == 0 {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
        }
    }
    if min_x == input.width || min_y == input.height {
        (0.0, 0.0)
    } else {
        (min_x as f32, min_y as f32)
    }
}

fn geometry2_layer_space_origin(input: &Canvas, params: &Value) -> (f32, f32) {
    if let Some(origin) = params
        .get(NATIVE_LAYER_SPACE_ORIGIN_PARAM)
        .and_then(Value::as_array)
        .and_then(|values| {
            let x = values.first()?.as_f64()? as f32;
            let y = values.get(1)?.as_f64()? as f32;
            Some((x, y))
        })
    {
        return origin;
    }
    layer_space_origin(input)
}

fn rounded_sample(input: &Canvas, source_uv: (f32, f32)) -> Option<[u32; 2]> {
    let sx = source_uv.0.round() as i32;
    let sy = source_uv.1.round() as i32;
    if sx < 0 || sy < 0 || sx >= input.width as i32 || sy >= input.height as i32 {
        return None;
    }
    Some([sx as u32, sy as u32])
}

fn sample_geometry2_bilinear(input: &Canvas, source_uv: (f32, f32)) -> [u8; 4] {
    if input.width == 0 || input.height == 0 {
        return [0, 0, 0, 0];
    }
    let x0 = source_uv.0.floor() as i32;
    let y0 = source_uv.1.floor() as i32;
    let tx = source_uv.0 - x0 as f32;
    let ty = source_uv.1 - y0 as f32;
    let weights_x = [(x0, 1.0 - tx), (x0 + 1, tx)];
    let weights_y = [(y0, 1.0 - ty), (y0 + 1, ty)];
    sample_geometry2_premult_unpremultiply(input, &weights_x, &weights_y)
}

fn sample_geometry2(input: &Canvas, source_uv: (f32, f32), mode: Geometry2SamplerMode) -> [u8; 4] {
    match mode {
        Geometry2SamplerMode::Bilinear => sample_geometry2_bilinear(input, source_uv),
        Geometry2SamplerMode::Bicubic => sample_geometry2_bicubic(input, source_uv),
    }
}

fn sample_geometry2_bicubic(input: &Canvas, source_uv: (f32, f32)) -> [u8; 4] {
    if input.width == 0 || input.height == 0 {
        return [0, 0, 0, 0];
    }
    let x0 = source_uv.0.floor() as i32;
    let y0 = source_uv.1.floor() as i32;
    let tx = source_uv.0 - x0 as f32;
    let ty = source_uv.1 - y0 as f32;
    let wx = cubic_keys_weights(tx, GEOMETRY2_BICUBIC_KEYS_A);
    let wy = cubic_keys_weights(ty, GEOMETRY2_BICUBIC_KEYS_A);
    let weights_x = [
        (x0 - 1, wx[0]),
        (x0, wx[1]),
        (x0 + 1, wx[2]),
        (x0 + 2, wx[3]),
    ];
    let weights_y = [
        (y0 - 1, wy[0]),
        (y0, wy[1]),
        (y0 + 1, wy[2]),
        (y0 + 2, wy[3]),
    ];
    sample_geometry2_premult_unpremultiply(input, &weights_x, &weights_y)
}

fn sample_geometry2_premult_unpremultiply(
    input: &Canvas,
    weights_x: &[(i32, f32)],
    weights_y: &[(i32, f32)],
) -> [u8; 4] {
    let mut premult = [0.0_f32; 3];
    let mut alpha = 0.0_f32;
    for (sy, wy) in weights_y {
        for (sx, wx) in weights_x {
            if *sx < 0 || *sy < 0 || *sx >= input.width as i32 || *sy >= input.height as i32 {
                continue;
            }
            let weight = wx * wy;
            let sample = input.pixel(*sx as u32, *sy as u32);
            let sample_alpha = sample[3] as f32 / 255.0;
            for channel in 0..3 {
                premult[channel] += sample[channel] as f32 * sample_alpha * weight;
            }
            alpha += sample[3] as f32 * weight;
        }
    }

    let alpha_norm = alpha / 255.0;
    let mut out = [0_u8; 4];
    if alpha_norm > 1e-9 {
        for channel in 0..3 {
            out[channel] = (premult[channel] / alpha_norm).round().clamp(0.0, 255.0) as u8;
        }
    }
    out[3] = alpha.round().clamp(0.0, 255.0) as u8;
    out
}

fn cubic_keys_weights(t: f32, a: f32) -> [f32; 4] {
    [
        cubic_keys_weight(1.0 + t, a),
        cubic_keys_weight(t, a),
        cubic_keys_weight(1.0 - t, a),
        cubic_keys_weight(2.0 - t, a),
    ]
}

fn cubic_keys_weight(x: f32, a: f32) -> f32 {
    let x = x.abs();
    if x < 1.0 {
        return (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0;
    }
    if x < 2.0 {
        return a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a;
    }
    0.0
}

fn geometry2_out_of_bounds(
    input: &Canvas,
    source_uv: (f32, f32),
    mode: Geometry2SamplerMode,
) -> bool {
    if input.width == 0 || input.height == 0 {
        return true;
    }
    match mode {
        Geometry2SamplerMode::Bilinear => {
            source_uv.0 <= -1.0
                || source_uv.1 <= -1.0
                || source_uv.0 >= input.width as f32
                || source_uv.1 >= input.height as f32
        }
        Geometry2SamplerMode::Bicubic => {
            source_uv.0 <= -2.0
                || source_uv.1 <= -2.0
                || source_uv.0 >= input.width as f32 + 1.0
                || source_uv.1 >= input.height as f32 + 1.0
        }
    }
}

fn count_oob(input: &Canvas, mapping: Geometry2Mapping, mode: Geometry2SamplerMode) -> u32 {
    let mut count = 0;
    for y in 0..input.height {
        for x in 0..input.width {
            let source_uv = mapping.source_uv(x as f32, y as f32);
            if geometry2_out_of_bounds(input, source_uv, mode) {
                count += 1;
            }
        }
    }
    count
}

fn geometry_probe_points(input: &Canvas) -> Vec<[u32; 2]> {
    if input.width == 0 || input.height == 0 {
        return Vec::new();
    }
    let xs = [0, input.width / 2, input.width - 1];
    let ys = [0, input.height / 2, input.height - 1];
    let mut points = Vec::with_capacity(9);
    for y in ys {
        for x in xs {
            if !points.contains(&[x, y]) {
                points.push([x, y]);
            }
        }
    }
    points
}

fn point_param(params: &Value, names: &[&str]) -> Option<(f32, f32)> {
    for name in names {
        let Some(raw_value) = params.get(*name) else {
            continue;
        };
        let Some(value) = nested_value(raw_value) else {
            continue;
        };
        if let Some(values) = value.as_array() {
            let x = values.first().and_then(Value::as_f64)? as f32;
            let y = values.get(1).and_then(Value::as_f64)? as f32;
            return Some((x, y));
        }
        if let (Some(x), Some(y)) = (
            value.get("x").and_then(Value::as_f64),
            value.get("y").and_then(Value::as_f64),
        ) {
            return Some((x as f32, y as f32));
        }
    }
    None
}

fn scalar_param(params: &Value, names: &[&str], time: f64, default: f32) -> f32 {
    scalar_param_opt(params, names, time).unwrap_or(default)
}

fn scalar_param_opt(params: &Value, names: &[&str], time: f64) -> Option<f32> {
    for name in names {
        if !has_param(params, name) {
            continue;
        };
        return Some(param_f32_at(params, name, time, 0.0));
    }
    None
}

fn nested_value(value: &Value) -> Option<&Value> {
    Some(value.get("value").unwrap_or(value))
}

fn has_param(params: &Value, name: &str) -> bool {
    params.get(name).is_some()
}

fn nearly_eq(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.001
}

const GEOMETRY2_BILINEAR_SAMPLER_MODE: &str =
    "bilinear_premult_unpremultiply_partial_footprint_transparent";
const GEOMETRY2_BICUBIC_SAMPLER_MODE: &str =
    "bicubic_keys_a_-0.7_premult_unpremultiply_partial_footprint_transparent";
const GEOMETRY2_BICUBIC_KEYS_A: f32 = -0.7;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EffectContext;
    use serde_json::json;

    #[test]
    fn geometry_offsets_position_around_anchor() {
        let mut input = Canvas::transparent(3, 1);
        input.set_pixel(1, 0, [10, 20, 30, 255]);

        let output = Geometry2::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({
                    "anchor": [1, 0],
                    "position": [2, 0]
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(2, 0), [10, 20, 30, 255]);
        assert_eq!(output.pixel(1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn geometry_applies_transform_in_layer_space_origin() {
        let mut input = Canvas::transparent(4, 4);
        input.set_pixel(1, 1, [10, 20, 30, 255]);
        input.set_pixel(2, 1, [40, 50, 60, 255]);

        let output = Geometry2::default()
            .render(
                &input,
                &EffectContext {
                    time: 0.0,
                    fps: 30.0,
                },
                &json!({
                    "anchor": [0, 0],
                    "position": [0, 0],
                    "rotation": 90
                }),
            )
            .unwrap();

        assert_eq!(output.pixel(1, 2), [40, 50, 60, 255]);
    }

    #[test]
    fn geometry_layer_origin_override_uses_comp_space_for_adjustments() {
        let mut input = Canvas::transparent(512, 512);
        input.set_pixel(128, 128, [10, 20, 30, 255]);
        input.set_pixel(256, 256, [40, 50, 60, 255]);

        let params = json!({
            "0001": [256, 256],
            "0002": [256, 256],
            "0004": 120,
            "0008": 72,
            "0012": 2,
            "rotation": 17
        });
        let native_layer_debug = geometry2_debug_data(&input, &params, 0.0);
        let adjustment_debug = geometry2_debug_data(
            &input,
            &json!({
                "0001": [256, 256],
                "0002": [256, 256],
                "0004": 120,
                "0008": 72,
                "0012": 2,
                "rotation": 17,
                "__native_layer_space_origin": [0, 0]
            }),
            0.0,
        );

        let native_center = native_layer_debug
            .samples
            .iter()
            .find(|sample| sample.output_xy == [256, 256])
            .unwrap();
        let adjustment_center = adjustment_debug
            .samples
            .iter()
            .find(|sample| sample.output_xy == [256, 256])
            .unwrap();

        assert!((native_center.source_uv[0] - 256.0).abs() > 1.0);
        assert!((adjustment_center.source_uv[0] - 256.0).abs() < 0.001);
        assert!((adjustment_center.source_uv[1] - 256.0).abs() < 0.001);
    }

    #[test]
    fn params_accept_ae_numbered_transform_values() {
        let input = Canvas::transparent(10, 8);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0001": { "value": [1, 2] },
                "0002": { "value": { "x": 3, "y": 4 } },
                "0003": { "value": 82 },
                "0004": { "value": 125 },
                "0008": { "value": 80 },
                "0012": { "value": 2 },
                "rotation": { "value": 15 }
            }),
            0.0,
        );

        assert_eq!(params.anchor, (1.0, 2.0));
        assert_eq!(params.position, (3.0, 4.0));
        assert_eq!(params.scale, (125.0, 125.0));
        assert_eq!(params.rotation, 15.0);
        assert_eq!(params.sampler_mode, Geometry2SamplerMode::Bicubic);
    }

    #[test]
    fn numbered_uniform_scale_uses_0004_and_ignores_0008_height_interpretation() {
        let input = Canvas::transparent(512, 512);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0001": [128, 128],
                "0002": [256, 256],
                "0003": 82,
                "0004": 120,
                "0008": 72,
                "rotation": 17
            }),
            0.0,
        );

        assert_eq!(params.scale, (120.0, 120.0));
        assert_eq!(params.rotation, 17.0);
    }

    #[test]
    fn numbered_0008_is_rotation_fallback() {
        let input = Canvas::transparent(512, 512);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0001": [128, 128],
                "0002": [256, 256],
                "0003": 1,
                "0004": 120,
                "0008": 72
            }),
            0.0,
        );

        assert_eq!(params.scale, (120.0, 120.0));
        assert_eq!(params.rotation, 72.0);
    }

    #[test]
    fn ae_property_dump_indices_map_skew_axis_rotation_and_ignore_opacity_slot() {
        let input = Canvas::transparent(512, 512);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0001": [128, 128],
                "0002": [256, 256],
                "0003": 1,
                "0004": 120,
                "0005": 90,
                "0006": 14,
                "0007": 35,
                "0008": 72,
                "0009": 18
            }),
            0.0,
        );

        assert_eq!(params.scale, (90.0, 120.0));
        assert_eq!(params.skew, 14.0);
        assert_eq!(params.skew_axis, 35.0);
        assert_eq!(params.rotation, 72.0);
    }

    #[test]
    fn named_axis_scale_remains_non_uniform() {
        let input = Canvas::transparent(512, 512);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "scaleX": 120,
                "scaleY": 82
            }),
            0.0,
        );

        assert_eq!(params.scale, (120.0, 82.0));
    }

    #[test]
    fn params_accept_time_varying_numbered_scale() {
        let input = Canvas::transparent(4, 4);
        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0003": {
                    "keyframes": [
                        { "t": 0.0, "v": 100.0 },
                        { "t": 1.0, "v": 200.0 }
                    ]
                }
            }),
            0.5,
        );

        assert_eq!(params.scale, (150.0, 150.0));
    }

    #[test]
    fn debug_data_reports_resolved_params_mapping_samples_and_oob() {
        let input = Canvas::transparent(3, 3);
        let params = json!({
            "anchor": [1, 1],
            "position": [2, 1]
        });

        let debug = geometry2_debug_data(&input, &params, 0.0);

        assert_eq!(debug.raw_params, params);
        assert!(!debug.property_mapping.payload_0003.present);
        assert_eq!(
            debug.property_mapping.payload_0008.native_role,
            "rotation_degrees"
        );
        assert_eq!(
            debug.resolved,
            Geometry2ResolvedParams {
                anchor: [1.0, 1.0],
                position: [2.0, 1.0],
                scale: [100.0, 100.0],
                rotation: 0.0,
                skew: 0.0,
                skew_axis: 0.0,
                pixel_aspect: 1.0,
                sampling: 1,
            }
        );
        assert_eq!(
            debug.forward_matrix,
            [[1.0, -0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        );
        assert_eq!(
            debug.inverse_matrix,
            [[1.0, 0.0, -1.0], [-0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        );
        assert_eq!(debug.sampler_mode, GEOMETRY2_BILINEAR_SAMPLER_MODE);
        assert_eq!(
            debug.edge_policy,
            "partial_footprint_transparent_out_of_bounds"
        );
        assert_eq!(debug.samples.len(), 9);
        assert_eq!(debug.out_of_bounds_count, 3);

        let left_middle = debug
            .samples
            .iter()
            .find(|sample| sample.output_xy == [0, 1])
            .unwrap();
        assert_eq!(left_middle.source_uv, [-1.0, 1.0]);
        assert_eq!(left_middle.sample_xy, None);
        assert_eq!(left_middle.sample_rgba, None);
        assert!(left_middle.out_of_bounds);

        let center = debug
            .samples
            .iter()
            .find(|sample| sample.output_xy == [1, 1])
            .unwrap();
        assert_eq!(center.source_uv, [0.0, 1.0]);
        assert_eq!(center.sample_xy, Some([0, 1]));
        assert_eq!(center.sample_rgba, Some([0, 0, 0, 0]));
        assert!(!center.out_of_bounds);
    }

    #[test]
    fn debug_data_reports_high_risk_numbered_property_mapping() {
        let input = Canvas::transparent(512, 512);
        let params = json!({
            "0003": 82,
            "0004": 120,
            "0008": 72,
            "0012": 2,
            "rotation": 17
        });

        let debug = geometry2_debug_data(&input, &params, 0.0);

        assert_eq!(debug.property_mapping.payload_0003.payload_key, "0003");
        assert_eq!(
            debug.property_mapping.payload_0003.match_name,
            "ADBE Geometry2-0011"
        );
        assert_eq!(
            debug.property_mapping.payload_0003.native_role,
            "uniform_scale_fallback"
        );
        assert_eq!(
            debug.property_mapping.payload_0003.raw_value,
            Some(json!(82))
        );
        assert_eq!(
            debug.property_mapping.payload_0004.match_name,
            "ADBE Geometry2-0003"
        );
        assert_eq!(
            debug.property_mapping.payload_0004.native_role,
            "scale_height"
        );
        assert_eq!(
            debug.property_mapping.payload_0008.match_name,
            "ADBE Geometry2-0007"
        );
        assert_eq!(
            debug.property_mapping.payload_0008.native_role,
            "rotation_degrees"
        );
        assert!(debug
            .property_mapping
            .payload_0009
            .note
            .contains("not confused"));
        assert_eq!(
            debug.property_mapping.payload_0009.match_name,
            "ADBE Geometry2-0008"
        );
        assert!(!debug.property_mapping.payload_0009.present);
        assert_eq!(
            debug.property_mapping.payload_0012.match_name,
            "ADBE Geometry2-0012"
        );
        assert_eq!(
            debug.property_mapping.payload_0012.native_role,
            "sampler_mode"
        );
        assert_eq!(
            debug.property_mapping.payload_0012.raw_value,
            Some(json!(2))
        );
        assert_eq!(debug.resolved.sampling, 2);
        assert_eq!(debug.sampler_mode, GEOMETRY2_BICUBIC_SAMPLER_MODE);
    }

    #[test]
    fn geometry_uses_bilinear_partial_footprint_transparent_sampling_for_subpixel_uv() {
        let mut input = Canvas::transparent(2, 1);
        input.set_pixel(0, 0, [0, 0, 0, 255]);
        input.set_pixel(1, 0, [100, 20, 0, 255]);

        assert_eq!(
            sample_geometry2_bilinear(&input, (0.5, 0.0)),
            [50, 10, 0, 255]
        );
        assert_eq!(
            sample_geometry2_bilinear(&input, (-0.5, 0.0)),
            [0, 0, 0, 128]
        );
        assert_eq!(
            sample_geometry2_bilinear(&input, (1.5, 0.0)),
            [100, 20, 0, 128]
        );
        assert!(geometry2_out_of_bounds(
            &input,
            (-1.0, 0.0),
            Geometry2SamplerMode::Bilinear
        ));
        assert!(!geometry2_out_of_bounds(
            &input,
            (-0.5, 0.0),
            Geometry2SamplerMode::Bilinear
        ));
        assert!(!geometry2_out_of_bounds(
            &input,
            (1.5, 0.0),
            Geometry2SamplerMode::Bilinear
        ));
        assert!(geometry2_out_of_bounds(
            &input,
            (2.0, 0.0),
            Geometry2SamplerMode::Bilinear
        ));
    }

    #[test]
    fn geometry_sampler_interpolates_premultiplied_color_and_returns_straight_rgba() {
        let mut input = Canvas::transparent(2, 1);
        input.set_pixel(0, 0, [200, 100, 0, 128]);
        input.set_pixel(1, 0, [0, 0, 200, 255]);

        assert_eq!(
            sample_geometry2_bilinear(&input, (0.5, 0.0)),
            [67, 33, 133, 192]
        );
    }

    #[test]
    fn geometry_sampling_0012_selects_bicubic_branch() {
        let mut input = Canvas::transparent(4, 1);
        input.set_pixel(0, 0, [0, 0, 0, 255]);
        input.set_pixel(1, 0, [255, 0, 0, 255]);
        input.set_pixel(2, 0, [0, 0, 0, 255]);
        input.set_pixel(3, 0, [0, 0, 0, 255]);

        assert_eq!(
            sample_geometry2(&input, (1.5, 0.0), Geometry2SamplerMode::Bilinear),
            [128, 0, 0, 255]
        );
        assert_eq!(
            sample_geometry2(&input, (1.5, 0.0), Geometry2SamplerMode::Bicubic),
            [150, 0, 0, 255]
        );

        let params = Geometry2Params::from_json(
            &input,
            &json!({
                "0012": {
                    "keyframes": [
                        { "t": 0.0, "v": 1.0 },
                        { "t": 1.0, "v": 2.0 }
                    ]
                }
            }),
            1.0,
        );
        assert_eq!(params.sampler_mode, Geometry2SamplerMode::Bicubic);
    }

    #[test]
    fn geometry_matrix_matches_reverse_engineered_skew_order() {
        let input = Canvas::transparent(16, 16);
        let transform = Geometry2Params::from_json(
            &input,
            &json!({
                "anchor": [3.0, 4.0],
                "position": [8.0, 7.0],
                "scale": [120.0, 80.0],
                "rotation": 15.0,
                "skew": 20.0,
                "skewAxis": 35.0,
                "pixelAspect": 1.5
            }),
            0.0,
        );

        let mapping = Geometry2Mapping::from_params(transform, (0.0, 0.0));
        let pa = 1.5_f32;
        let mut expected = identity_matrix();
        expected = mat_mul(translation_matrix(-3.0, -4.0), expected);
        expected = mat_mul(scale_matrix(pa, 1.0), expected);
        expected = mat_mul(scale_matrix(1.2, 0.8), expected);
        expected = mat_mul(rotation_matrix(15.0), expected);
        expected = mat_mul(rotation_matrix(35.0), expected);
        expected = mat_mul(skew_x_matrix(-20.0_f32.to_radians().tan()), expected);
        expected = mat_mul(rotation_matrix(-35.0), expected);
        expected = mat_mul(scale_matrix(1.0 / pa, 1.0), expected);
        expected = mat_mul(translation_matrix(8.0, 7.0), expected);

        assert_matrix_near(mapping.forward_matrix, expected, 0.0001);
        assert_matrix_near(
            mat_mul(mapping.forward_matrix, mapping.inverse_matrix),
            identity_matrix(),
            0.0001,
        );
    }

    #[test]
    fn geometry_skew_moves_y_into_x_like_ae_hx_shear() {
        let input = Canvas::transparent(20, 20);
        let transform = Geometry2Params::from_json(
            &input,
            &json!({
                "anchor": [0.0, 0.0],
                "position": [0.0, 0.0],
                "skew": 45.0,
                "skewAxis": 0.0
            }),
            0.0,
        );

        let mapping = Geometry2Mapping::from_params(transform, (0.0, 0.0));
        let mapped = apply_matrix(mapping.forward_matrix, (4.0, 3.0));

        assert_near(mapped.0, 1.0, 0.0001);
        assert_near(mapped.1, 3.0, 0.0001);
    }

    #[test]
    fn geometry_pixel_aspect_changes_rotation_basis() {
        let input = Canvas::transparent(20, 20);
        let square = Geometry2Mapping::from_params(
            Geometry2Params::from_json(
                &input,
                &json!({
                    "anchor": [0.0, 0.0],
                    "position": [0.0, 0.0],
                    "rotation": 90.0
                }),
                0.0,
            ),
            (0.0, 0.0),
        );
        let wide = Geometry2Mapping::from_params(
            Geometry2Params::from_json(
                &input,
                &json!({
                    "anchor": [0.0, 0.0],
                    "position": [0.0, 0.0],
                    "rotation": 90.0,
                    "pixelAspect": 2.0
                }),
                0.0,
            ),
            (0.0, 0.0),
        );

        assert_matrix_near(
            square.forward_matrix,
            [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            0.0001,
        );
        assert_matrix_near(
            wide.forward_matrix,
            [[0.0, -0.5, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            0.0001,
        );
    }

    fn apply_matrix(matrix: [[f32; 3]; 3], point: (f32, f32)) -> (f32, f32) {
        (
            matrix[0][0] * point.0 + matrix[0][1] * point.1 + matrix[0][2],
            matrix[1][0] * point.0 + matrix[1][1] * point.1 + matrix[1][2],
        )
    }

    fn assert_matrix_near(left: [[f32; 3]; 3], right: [[f32; 3]; 3], epsilon: f32) {
        for row in 0..3 {
            for col in 0..3 {
                assert_near(left[row][col], right[row][col], epsilon);
            }
        }
    }

    fn assert_near(left: f32, right: f32, epsilon: f32) {
        assert!(
            (left - right).abs() <= epsilon,
            "expected {left} ~= {right} within {epsilon}"
        );
    }
}
