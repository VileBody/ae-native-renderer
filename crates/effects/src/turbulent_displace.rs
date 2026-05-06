use crate::{param_bool_any, param_f32_any, param_f32_at_any, param_value, Effect, EffectContext};
use raster_cpu::{BilinearSampler, Canvas, Sampler};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct TurbulentDisplace;

impl Effect for TurbulentDisplace {
    fn match_name(&self) -> &'static str {
        "ADBE Turbulent Displace"
    }

    fn render(
        &self,
        input: &Canvas,
        ctx: &EffectContext,
        params: &Value,
    ) -> anyhow::Result<Canvas> {
        let params = TurbulentDisplaceParams::from_json(params, ctx.time);
        let resolved = params.resolved_for_input(input);
        if resolved.amount <= f32::EPSILON || input.width == 0 || input.height == 0 {
            return Ok(input.clone());
        }

        Ok(displace_canvas(input, resolved))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TurbulentDisplaceParams {
    pub displacement_type: f32,
    pub amount: f32,
    pub size: f32,
    pub offset: [f32; 2],
    pub offset_is_default: bool,
    pub complexity: f32,
    pub evolution: f32,
    pub cycle_evolution: bool,
    pub cycle_revolutions: f32,
    pub random_seed: f32,
    pub antialiasing_best_quality: bool,
    pub pinning: f32,
    pub resize_layer: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurbulentDisplaceFieldTelemetry {
    pub raw_params: Value,
    pub resolved: TurbulentDisplaceResolvedParams,
    pub ae_wrapper: TurbulentDisplaceAeWrapperTelemetry,
    pub field_state: TurbulentDisplaceFieldStateTelemetry,
    pub samples: Vec<TurbulentDisplaceFieldSample>,
    pub field_hash: u64,
    pub sampler_mode: &'static str,
    pub edge_policy: &'static str,
    pub out_of_bounds_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbulentDisplaceAeWrapperTelemetry {
    pub state_block_bytes: usize,
    pub gpu_param_block_bytes: usize,
    pub noise_table_rows: u32,
    pub inferred_internal_displacement_mode: u32,
    pub kernel_path: &'static str,
    pub uses_h_lookup: bool,
    pub uses_v_lookup: bool,
    pub h_lookup_len: u32,
    pub v_lookup_len: u32,
    pub amount_fixed16: i32,
    pub size_fixed16: i32,
    pub offset_fixed16: [i32; 2],
    pub evolution_fixed16: i32,
    pub cycle_evolution: bool,
    pub cycle_revolutions_fixed16: i32,
    pub random_seed_fixed16: i32,
    pub antialiasing_best_quality: bool,
    pub complexity_octaves: u32,
    pub complexity_fraction: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbulentDisplaceFieldStateTelemetry {
    pub model: &'static str,
    pub coordinate_space: &'static str,
    pub dispatch_path: &'static str,
    pub complexity_octaves: u32,
    pub complexity_fraction: f32,
    pub evolution_degrees: f32,
    pub cycle_evolution: bool,
    pub cycle_revolutions: f32,
    pub antialiasing_best_quality: bool,
    pub phase_radians: f32,
    pub amplitude: f32,
    pub source_uv_convention: &'static str,
    pub hash_coverage: &'static str,
    pub property_mapping_status: &'static str,
    pub tuning_guardrail: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbulentDisplaceResolvedParams {
    pub displacement_type: u32,
    pub amount: f32,
    pub size: f32,
    pub offset: [f32; 2],
    pub complexity: u32,
    pub complexity_fraction: f32,
    pub evolution: f32,
    pub cycle_evolution: bool,
    pub cycle_revolutions: f32,
    pub random_seed: u32,
    pub antialiasing_best_quality: bool,
    pub pinning: u32,
    pub resize_layer: bool,
    pub amplitude: f32,
    pub phase_radians: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbulentDisplaceFieldSample {
    pub output_xy: [u32; 2],
    pub noise: [f32; 2],
    pub displacement: [f32; 2],
    pub source_uv: [f32; 2],
    pub sample_xy: Option<[u32; 2]>,
    pub out_of_bounds: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TurbulentDisplaceFieldVector {
    noise: [f32; 2],
    displacement: [f32; 2],
    source_uv: [f32; 2],
}

impl TurbulentDisplaceParams {
    pub(crate) fn from_json(params: &Value, time: f64) -> Self {
        let offset = param_point_any_optional(
            params,
            &[
                "offset",
                "Offset",
                "Offset (Turbulence)",
                "offset_turbulence",
                "0004",
            ],
        );
        Self {
            displacement_type: param_f32_any(
                params,
                &["displacement", "Displacement", "displacement_type", "0001"],
                1.0,
            ),
            amount: param_f32_any(params, &["amount", "Amount", "0002"], 0.0),
            size: param_f32_any(params, &["size", "Size", "0003"], 100.0),
            offset: offset.unwrap_or([0.0, 0.0]),
            offset_is_default: offset.is_none(),
            complexity: param_f32_any(params, &["complexity", "Complexity", "0005"], 2.0),
            evolution: param_f32_at_any(
                params,
                &["evolution", "Evolution", "0006"],
                time,
                time as f32 * 45.0,
            ),
            cycle_evolution: param_bool_any(
                params,
                &[
                    "cycle_evolution",
                    "cycleEvolution",
                    "Cycle Evolution",
                    "0008",
                ],
                false,
            ),
            cycle_revolutions: param_f32_any(
                params,
                &[
                    "cycle_revolutions",
                    "cycleRevolutions",
                    "Cycle (in Revolutions)",
                    "Cycle Revolutions",
                    "0009",
                ],
                1.0,
            ),
            random_seed: param_f32_any(
                params,
                &["random_seed", "randomSeed", "seed", "Random Seed", "0010"],
                0.0,
            ),
            antialiasing_best_quality: param_bool_any(
                params,
                &[
                    "antialiasing_best_quality",
                    "antialiasingBestQuality",
                    "Antialiasing for Best Quality",
                    "0014",
                ],
                true,
            ),
            pinning: param_f32_any(params, &["pinning", "Pinning", "0012"], 3.0),
            resize_layer: param_bool_any(params, &["resize_layer", "Resize Layer", "0013"], false),
        }
    }

    fn resolved(self) -> TurbulentDisplaceResolvedParams {
        map_params_to_field_model(self, None)
    }

    fn resolved_for_input(self, input: &Canvas) -> TurbulentDisplaceResolvedParams {
        map_params_to_field_model(
            self,
            Some([input.width as f32 * 0.5, input.height as f32 * 0.5]),
        )
    }
}

fn map_params_to_field_model(
    params: TurbulentDisplaceParams,
    default_offset: Option<[f32; 2]>,
) -> TurbulentDisplaceResolvedParams {
    let amount = params.amount.clamp(0.0, 200.0);
    let size = params.size.clamp(2.0, 1000.0);
    let complexity = params.complexity.clamp(1.0, 6.0);
    let complexity_octaves = complexity.floor() as u32;
    let complexity_fraction = complexity - complexity_octaves as f32;
    let random_seed = params.random_seed.round().clamp(0.0, u32::MAX as f32) as u32;
    let phase_radians = params.evolution.to_radians() + seed_phase(random_seed);
    let offset = if params.offset_is_default {
        default_offset.unwrap_or(params.offset)
    } else {
        params.offset
    };
    TurbulentDisplaceResolvedParams {
        displacement_type: params.displacement_type.round().clamp(1.0, 9.0) as u32,
        amount,
        size,
        offset,
        complexity: complexity_octaves,
        complexity_fraction,
        evolution: params.evolution,
        cycle_evolution: params.cycle_evolution,
        cycle_revolutions: params.cycle_revolutions.max(0.0),
        random_seed,
        antialiasing_best_quality: params.antialiasing_best_quality,
        pinning: params.pinning.round().clamp(0.0, 17.0) as u32,
        resize_layer: params.resize_layer,
        amplitude: amount * size * AE_TURBULENT_AMOUNT_SIZE_SCALE,
        phase_radians,
    }
}

fn param_point_any_optional(params: &Value, names: &[&str]) -> Option<[f32; 2]> {
    for name in names {
        let Some(value) = param_value(params, name) else {
            continue;
        };
        if let Some(values) = value.as_array() {
            let x = values
                .first()
                .and_then(Value::as_f64)
                .map(|value| value as f32)
                .unwrap_or(0.0);
            let y = values
                .get(1)
                .and_then(Value::as_f64)
                .map(|value| value as f32)
                .unwrap_or(0.0);
            return Some([x, y]);
        }
        if let Some(text) = value.as_str() {
            let mut parts = text.split(',').map(str::trim);
            if let (Some(x), Some(y)) = (parts.next(), parts.next()) {
                if let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>()) {
                    return Some([x, y]);
                }
            }
        }
    }
    None
}

pub fn turbulent_displace_field_telemetry(
    input: &Canvas,
    params: &Value,
    time: f64,
) -> TurbulentDisplaceFieldTelemetry {
    let raw = TurbulentDisplaceParams::from_json(params, time);
    let resolved = raw.resolved_for_input(input);
    let ae_wrapper = ae_wrapper_telemetry(input, raw);
    let field_state = field_state_telemetry(resolved, ae_wrapper);
    let model = AeTurbulentFieldModel::new(resolved);
    let samples = turbulent_probe_points(input)
        .into_iter()
        .map(|[x, y]| field_sample(input, &model, x, y))
        .collect();
    let (field_hash, out_of_bounds_count) = field_hash_and_oob(input, &model);

    TurbulentDisplaceFieldTelemetry {
        raw_params: params.clone(),
        resolved,
        ae_wrapper,
        field_state,
        samples,
        field_hash,
        sampler_mode: TURBULENT_SAMPLER_MODE,
        edge_policy: edge_policy_label(resolved),
        out_of_bounds_count,
    }
}

pub fn turbulent_displace_resolved_params(
    params: &Value,
    time: f64,
) -> TurbulentDisplaceResolvedParams {
    TurbulentDisplaceParams::from_json(params, time).resolved()
}

pub fn turbulent_displace_resolved_params_for_input(
    input: &Canvas,
    params: &Value,
    time: f64,
) -> TurbulentDisplaceResolvedParams {
    TurbulentDisplaceParams::from_json(params, time).resolved_for_input(input)
}

pub fn turbulent_displace_field_samples(
    input: &Canvas,
    params: &Value,
    time: f64,
    points: &[[u32; 2]],
) -> Vec<TurbulentDisplaceFieldSample> {
    let resolved = TurbulentDisplaceParams::from_json(params, time).resolved_for_input(input);
    let model = AeTurbulentFieldModel::new(resolved);
    points
        .iter()
        .map(|&[x, y]| field_sample(input, &model, x, y))
        .collect()
}

fn field_state_telemetry(
    resolved: TurbulentDisplaceResolvedParams,
    ae_wrapper: TurbulentDisplaceAeWrapperTelemetry,
) -> TurbulentDisplaceFieldStateTelemetry {
    TurbulentDisplaceFieldStateTelemetry {
        model: "ae_lcg_table_noise_v1",
        coordinate_space: "output_pixel_to_source_uv",
        dispatch_path: ae_wrapper.kernel_path,
        complexity_octaves: ae_wrapper.complexity_octaves,
        complexity_fraction: ae_wrapper.complexity_fraction,
        evolution_degrees: resolved.evolution,
        cycle_evolution: resolved.cycle_evolution,
        cycle_revolutions: resolved.cycle_revolutions,
        antialiasing_best_quality: resolved.antialiasing_best_quality,
        phase_radians: resolved.phase_radians,
        amplitude: resolved.amplitude,
        source_uv_convention: "source_uv = output_xy + displacement",
        hash_coverage: "full_frame_ae_lcg_displacement_source_uv_sample_xy_oob",
        property_mapping_status: "0008_cycle_evolution_0009_cycle_revolutions_0010_random_seed_0014_antialiasing_recorded",
        tuning_guardrail: "do_not_tune_from_final_png_only",
    }
}

fn ae_wrapper_telemetry(
    input: &Canvas,
    params: TurbulentDisplaceParams,
) -> TurbulentDisplaceAeWrapperTelemetry {
    let displacement_type = params.displacement_type.round().clamp(1.0, 9.0) as u32;
    let internal_mode = inferred_ae_internal_displacement_mode(displacement_type);
    let uses_h_lookup = matches!(internal_mode, 9 | 11);
    let uses_v_lookup = matches!(internal_mode, 10 | 11);
    let complexity = params.complexity.clamp(1.0, 6.0);
    let complexity_octaves = complexity.floor() as u32;

    TurbulentDisplaceAeWrapperTelemetry {
        state_block_bytes: 0x8130,
        gpu_param_block_bytes: 0x405c,
        noise_table_rows: 64,
        inferred_internal_displacement_mode: internal_mode,
        kernel_path: if (9..=11).contains(&internal_mode) {
            "TurbulentDisplaceFrac1DKernel"
        } else {
            "TurbulentDisplaceFracAllKernel"
        },
        uses_h_lookup,
        uses_v_lookup,
        h_lookup_len: if uses_h_lookup { input.width + 2 } else { 0 },
        v_lookup_len: if uses_v_lookup { input.height + 2 } else { 0 },
        amount_fixed16: to_fixed16(params.amount),
        size_fixed16: to_fixed16(params.size),
        offset_fixed16: [to_fixed16(params.offset[0]), to_fixed16(params.offset[1])],
        evolution_fixed16: to_fixed16(params.evolution),
        cycle_evolution: params.cycle_evolution,
        cycle_revolutions_fixed16: to_fixed16(params.cycle_revolutions),
        random_seed_fixed16: to_fixed16(params.random_seed),
        antialiasing_best_quality: params.antialiasing_best_quality,
        complexity_octaves,
        complexity_fraction: complexity - complexity_octaves as f32,
    }
}

fn inferred_ae_internal_displacement_mode(displacement_type: u32) -> u32 {
    match displacement_type {
        7 => 9,
        8 => 10,
        9 => 11,
        other => other,
    }
}

fn to_fixed16(value: f32) -> i32 {
    (value * 65_536.0).round() as i32
}

fn displace_canvas(input: &Canvas, resolved: TurbulentDisplaceResolvedParams) -> Canvas {
    let mut output = Canvas::transparent(input.width, input.height);
    let model = AeTurbulentFieldModel::new(resolved);

    for y in 0..input.height {
        for x in 0..input.width {
            let sample = field_sample(input, &model, x, y);
            let Some(pixel) = sample_source_pixel(input, sample.source_uv, model.resolved) else {
                continue;
            };
            output.set_pixel(x, y, pixel);
        }
    }

    output
}

fn field_sample(
    input: &Canvas,
    model: &AeTurbulentFieldModel,
    x: u32,
    y: u32,
) -> TurbulentDisplaceFieldSample {
    let vector = model.field_vector(x, y);
    let (sample_xy, out_of_bounds) = sample_nearest_round(input, vector.source_uv, model.resolved);

    TurbulentDisplaceFieldSample {
        output_xy: [x, y],
        noise: vector.noise,
        displacement: vector.displacement,
        source_uv: vector.source_uv,
        sample_xy,
        out_of_bounds,
    }
}

#[derive(Debug, Clone)]
struct AeTurbulentFieldModel {
    resolved: TurbulentDisplaceResolvedParams,
    amount_pixels: f64,
    coordinate_scale: f64,
    table: Vec<f64>,
}

impl AeTurbulentFieldModel {
    fn new(resolved: TurbulentDisplaceResolvedParams) -> Self {
        let coordinate_scale = ae_coordinate_scale(resolved);
        let table = ae_turbulent_table(resolved);
        Self {
            amount_pixels: resolved.amplitude as f64,
            resolved,
            coordinate_scale,
            table,
        }
    }

    fn field_vector(&self, x: u32, y: u32) -> TurbulentDisplaceFieldVector {
        let noise = self.field_noise(x, y);
        let displacement = self.displacement_from_noise(noise);
        TurbulentDisplaceFieldVector {
            noise,
            displacement,
            source_uv: [x as f32 + displacement[0], y as f32 + displacement[1]],
        }
    }

    fn field_noise(&self, x: u32, y: u32) -> [f32; 2] {
        let x_coord =
            (x as f64 - self.resolved.offset[0] as f64) * self.coordinate_scale + AE_NOISE_X_BIAS;
        let y_coord =
            (y as f64 - self.resolved.offset[1] as f64) * self.coordinate_scale + AE_NOISE_Y_BIAS;
        let [first, second] = self.field_pair(x_coord, y_coord);
        [first as f32, second as f32]
    }

    fn field_pair(&self, x: f64, y: f64) -> [f64; 2] {
        let mode = self.resolved.displacement_type;
        let bicubic = matches!(mode, 5..=7);
        let first = self.noise2d(x, y, bicubic);
        if matches!(mode, 1 | 5) {
            [
                first,
                self.noise2d(x + AE_PAIR_X_OFFSET, y + AE_PAIR_Y_OFFSET, bicubic),
            ]
        } else {
            [
                first - self.noise2d(x + AE_DERIVATIVE_STEP, y, bicubic),
                first - self.noise2d(x, y + AE_DERIVATIVE_STEP, bicubic),
            ]
        }
    }

    fn displacement_from_noise(&self, noise: [f32; 2]) -> [f32; 2] {
        if self.amount_pixels <= f64::EPSILON {
            return [0.0, 0.0];
        }
        let mut dx = self.amount_pixels * noise[0] as f64;
        let mut dy = self.amount_pixels * noise[1] as f64;
        if matches!(self.resolved.displacement_type, 3 | 7) {
            std::mem::swap(&mut dx, &mut dy);
            dx = -dx;
        }
        [dx as f32, dy as f32]
    }

    fn noise2d(&self, mut x: f64, mut y: f64, bicubic: bool) -> f64 {
        if !bicubic {
            let rotated_x = x - y;
            y += x;
            x = rotated_x;
        }

        let mut value = 0.0;
        let mut amplitude = AE_OCTAVE_INITIAL_AMPLITUDE;
        for _ in 0..self.resolved.complexity {
            x *= AE_LACUNARITY;
            y *= AE_LACUNARITY;
            value += self.noise2d_once(x, y, bicubic) * amplitude;
            amplitude *= AE_OCTAVE_PERSISTENCE;
        }
        if self.resolved.complexity_fraction > 0.0 {
            value += self.noise2d_once(x * AE_LACUNARITY, y * AE_LACUNARITY, bicubic)
                * amplitude
                * self.resolved.complexity_fraction as f64;
        }
        value
    }

    fn noise2d_once(&self, x: f64, y: f64, bicubic: bool) -> f64 {
        if bicubic {
            ae_table_lookup_bicubic(&self.table, x, y)
        } else {
            ae_table_lookup_bilinear(&self.table, x, y)
        }
    }
}

fn sample_nearest_round(
    input: &Canvas,
    source_uv: [f32; 2],
    resolved: TurbulentDisplaceResolvedParams,
) -> (Option<[u32; 2]>, bool) {
    let sx = source_uv[0].round();
    let sy = source_uv[1].round();
    if sx < 0.0 || sy < 0.0 || sx >= input.width as f32 || sy >= input.height as f32 {
        if uses_clamped_edges(resolved) && input.width > 0 && input.height > 0 {
            return (
                Some([
                    sx.clamp(0.0, input.width.saturating_sub(1) as f32) as u32,
                    sy.clamp(0.0, input.height.saturating_sub(1) as f32) as u32,
                ]),
                true,
            );
        }
        return (None, true);
    }
    (Some([sx as u32, sy as u32]), false)
}

fn sample_source_pixel(
    input: &Canvas,
    source_uv: [f32; 2],
    resolved: TurbulentDisplaceResolvedParams,
) -> Option<[u8; 4]> {
    if input.width == 0 || input.height == 0 {
        return None;
    }
    let max_x = input.width.saturating_sub(1) as f32;
    let max_y = input.height.saturating_sub(1) as f32;
    let mut sx = source_uv[0];
    let mut sy = source_uv[1];
    if sx < 0.0 || sy < 0.0 || sx > max_x || sy > max_y {
        if !uses_clamped_edges(resolved) {
            return None;
        }
        sx = sx.clamp(0.0, max_x);
        sy = sy.clamp(0.0, max_y);
    }
    Some(BilinearSampler.sample(input, sx, sy))
}

fn uses_clamped_edges(resolved: TurbulentDisplaceResolvedParams) -> bool {
    resolved.pinning > 0 || resolved.resize_layer
}

fn edge_policy_label(resolved: TurbulentDisplaceResolvedParams) -> &'static str {
    if resolved.resize_layer {
        "clamp_edges_resize_layer"
    } else if resolved.pinning > 0 {
        "clamp_edges_pinning"
    } else {
        "transparent_out_of_bounds"
    }
}

fn turbulent_probe_points(input: &Canvas) -> Vec<[u32; 2]> {
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

fn field_hash_and_oob(input: &Canvas, model: &AeTurbulentFieldModel) -> (u64, u32) {
    let mut hash = FNV_OFFSET_BASIS;
    let mut out_of_bounds_count = 0;
    hash = fnv_hash_u32(hash, input.width);
    hash = fnv_hash_u32(hash, input.height);
    for y in 0..input.height {
        for x in 0..input.width {
            let sample = field_sample(input, model, x, y);
            hash = fnv_hash_u32(hash, sample.displacement[0].to_bits());
            hash = fnv_hash_u32(hash, sample.displacement[1].to_bits());
            hash = fnv_hash_u32(hash, sample.source_uv[0].to_bits());
            hash = fnv_hash_u32(hash, sample.source_uv[1].to_bits());
            if let Some([sx, sy]) = sample.sample_xy {
                hash = fnv_hash_u32(hash, sx);
                hash = fnv_hash_u32(hash, sy);
            } else {
                hash = fnv_hash_u32(hash, u32::MAX);
                hash = fnv_hash_u32(hash, u32::MAX);
            }
            hash = fnv_hash_u32(hash, if sample.out_of_bounds { 1 } else { 0 });
            if sample.out_of_bounds {
                out_of_bounds_count += 1;
            }
        }
    }
    (hash, out_of_bounds_count)
}

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const TURBULENT_SAMPLER_MODE: &str = "pf_subpixel_sample_straight_bilinear";
const AE_TURBULENT_AMOUNT_SIZE_SCALE: f32 = 0.05;
const AE_SIZE_TO_COORD_SCALE: f64 = 0.5;
const AE_RADIAL_COORD_SCALE: f64 = 0.353_553_39;
const AE_NOISE_X_BIAS: f64 = 7913.17;
const AE_NOISE_Y_BIAS: f64 = 9711.73;
const AE_PAIR_X_OFFSET: f64 = 37.0;
const AE_PAIR_Y_OFFSET: f64 = 17.0;
const AE_DERIVATIVE_STEP: f64 = 0.25;
const AE_LACUNARITY: f64 = 1.77;
const AE_OCTAVE_INITIAL_AMPLITUDE: f64 = 0.25;
const AE_OCTAVE_PERSISTENCE: f64 = 0.7;
const AE_TABLE_SIZE: usize = 64;
const AE_TABLE_LEN: usize = AE_TABLE_SIZE * AE_TABLE_SIZE;
const AE_EVOLUTION_STEP_FIXED16: i32 = 0x5a0000;
const AE_EVOLUTION_PERIOD_FIXED16: i32 = 0x7e900000;
const AE_TABLE_BASE_SEED: u32 = 0x00bc8cb5;
const AE_TABLE_HASH_MUL: i32 = -0x145e_b7c3;
const AE_TABLE_HASH_ADD: i32 = 0x0daa_96f5;
const AE_LCG_MUL: u32 = 0x41c6_4e6d;
const AE_LCG_ADD: u32 = 0x3039;
const AE_FIXED16: f64 = 65_536.0;
const AE_EVOLUTION_CYCLE_FIXED16: f64 = 5_898_240.0;
const AE_RAND_SCALE: f64 = 1.0 / 16_384.0;

fn fnv_hash_u32(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn seed_phase(seed: u32) -> f32 {
    seed as f32 * 0.618_034
}

fn ae_coordinate_scale(resolved: TurbulentDisplaceResolvedParams) -> f64 {
    let mut scale = AE_SIZE_TO_COORD_SCALE / resolved.size as f64;
    if matches!(resolved.displacement_type, 1..=3) {
        scale *= AE_RADIAL_COORD_SCALE;
    }
    scale
}

fn ae_turbulent_table(resolved: TurbulentDisplaceResolvedParams) -> Vec<f64> {
    let evolution_fixed16 = (resolved.evolution as f64 * AE_FIXED16).round() as i32;
    let cycle = evolution_fixed16 as f64 / AE_EVOLUTION_CYCLE_FIXED16;
    let cycle_floor = ae_floor_i32(cycle);
    let cycle_base = (cycle_floor as f64 * AE_EVOLUTION_CYCLE_FIXED16) as i32;
    let evolution_fraction =
        (evolution_fixed16.wrapping_sub(cycle_base)) as f64 / AE_EVOLUTION_CYCLE_FIXED16;
    let seed = (resolved.random_seed as i32)
        .wrapping_mul(AE_FIXED16 as i32)
        .wrapping_add(cycle_base);
    let period = if resolved.cycle_evolution && resolved.cycle_revolutions > 0.0 {
        (resolved.cycle_revolutions as f64 * 360.0 * AE_FIXED16).round() as i32
    } else {
        AE_EVOLUTION_PERIOD_FIXED16
    };
    ae_build_table(seed, AE_EVOLUTION_STEP_FIXED16, period, evolution_fraction)
}

fn ae_build_table(seed: i32, step: i32, period: i32, fraction: f64) -> Vec<f64> {
    let period = period.max(1);
    let mut base_seed = AE_TABLE_BASE_SEED;
    let mut s0 = ae_seed_hash(ae_period_mod(
        seed.wrapping_sub(step.wrapping_mul(2)),
        period,
    ));
    let mut s1 = ae_seed_hash(ae_period_mod(seed.wrapping_sub(step), period));
    let mut s2 = ae_seed_hash(ae_period_mod(seed, period));
    let mut s3 = ae_seed_hash(ae_period_mod(seed.wrapping_add(step), period));
    let mut s4 = ae_seed_hash(ae_period_mod(
        seed.wrapping_add(step.wrapping_mul(2)),
        period,
    ));
    let mut s5 = ae_seed_hash(ae_period_mod(
        seed.wrapping_add(step.wrapping_mul(3)),
        period,
    ));

    let mut table = Vec::with_capacity(AE_TABLE_LEN);
    for _ in 0..AE_TABLE_SIZE {
        for _ in 0..AE_TABLE_SIZE {
            base_seed = ae_lcg(base_seed);
            s0 = ae_lcg(s0);
            s1 = ae_lcg(s1);
            s2 = ae_lcg(s2);
            s3 = ae_lcg(s3);
            s4 = ae_lcg(s4);
            s5 = ae_lcg(s5);

            let mut t = ae_rand_unit(base_seed) + fraction;
            let p0;
            let p1;
            let p2;
            let p3;
            if t >= 0.0 {
                if t > 1.0 {
                    t -= 1.0;
                    p0 = ae_rand_unit(s2);
                    p1 = ae_rand_unit(s3);
                    p2 = ae_rand_unit(s4);
                    p3 = ae_rand_unit(s5);
                } else {
                    p0 = ae_rand_unit(s1);
                    p1 = ae_rand_unit(s2);
                    p2 = ae_rand_unit(s3);
                    p3 = ae_rand_unit(s4);
                }
            } else {
                t += 1.0;
                p0 = ae_rand_unit(s0);
                p1 = ae_rand_unit(s1);
                p2 = ae_rand_unit(s2);
                p3 = ae_rand_unit(s3);
            }
            table.push(ae_bspline(p0, p1, p2, p3, t));
        }
    }
    table
}

fn ae_seed_hash(value: i32) -> u32 {
    value
        .wrapping_mul(AE_TABLE_HASH_MUL)
        .wrapping_add(AE_TABLE_HASH_ADD) as u32
}

fn ae_period_mod(value: i32, period: i32) -> i32 {
    let value = value as f64;
    let period_f = period as f64;
    let mut rem = value - (value / period_f).trunc() * period_f;
    if rem < 0.0 {
        rem += period_f;
    }
    rem as i32
}

fn ae_lcg(value: u32) -> u32 {
    value.wrapping_mul(AE_LCG_MUL).wrapping_add(AE_LCG_ADD)
}

fn ae_rand_unit(value: u32) -> f64 {
    ((value >> 16) & 0x7fff) as f64 * AE_RAND_SCALE - 1.0
}

fn ae_bspline(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    let one_minus_t = 1.0 - t;
    (((3.0 * t3 - 6.0 * t2) + 4.0) * p1
        + one_minus_t * one_minus_t * one_minus_t * p0
        + ((3.0 * t2 - 3.0 * t3) + 3.0 * t + 1.0) * p2
        + t3 * p3)
        * 0.2
}

fn ae_floor_i32(value: f64) -> i32 {
    let truncated = value as i32;
    if value < 0.0 && value != truncated as f64 {
        truncated - 1
    } else {
        truncated
    }
}

fn ae_smoothstep(value: f64) -> f64 {
    if value < 0.0 {
        0.0
    } else if value <= 1.0 {
        (3.0 - value * 2.0) * value * value
    } else {
        1.0
    }
}

fn ae_table_lookup_bilinear(table: &[f64], x: f64, y: f64) -> f64 {
    let ix = ae_floor_i32(x);
    let iy = ae_floor_i32(y);
    let fx = ae_smoothstep(x - ix as f64);
    let fy = ae_smoothstep(y - iy as f64);

    let p00 = table[ae_table_index_2d(ix, iy)];
    let p10 = table[ae_table_index_2d(ix.wrapping_add(1), iy)];
    let p01 = table[ae_table_index_2d(ix, iy.wrapping_add(1))];
    let p11 = table[ae_table_index_2d(ix.wrapping_add(1), iy.wrapping_add(1))];
    ((1.0 - fx) * p00 + fx * p10) * (1.0 - fy) + ((1.0 - fx) * p01 + fx * p11) * fy
}

fn ae_table_lookup_bicubic(table: &[f64], x: f64, y: f64) -> f64 {
    let ix = ae_floor_i32(x);
    let iy = ae_floor_i32(y);
    let fx = x - ix as f64;
    let fy = y - iy as f64;
    let row_y2 = ae_bicubic_row(table, ix, iy.wrapping_add(2), fx);
    let row_y1 = ae_bicubic_row(table, ix, iy.wrapping_add(1), fx);
    let row_y0 = ae_bicubic_row(table, ix, iy, fx);
    let row_ym1 = ae_bicubic_row(table, ix, iy.wrapping_sub(1), fx);
    ae_bspline(row_ym1, row_y0, row_y1, row_y2, fy)
}

fn ae_bicubic_row(table: &[f64], ix: i32, iy: i32, fx: f64) -> f64 {
    ae_bspline(
        table[ae_table_index_2d(ix.wrapping_sub(1), iy)],
        table[ae_table_index_2d(ix, iy)],
        table[ae_table_index_2d(ix.wrapping_add(1), iy)],
        table[ae_table_index_2d(ix.wrapping_add(2), iy)],
        fx,
    )
}

fn ae_table_index_2d(ix: i32, iy: i32) -> usize {
    let row = (((iy >> 6) as u32 ^ ix as u32) & 0x3f) as usize;
    let col = (((ix >> 6) as u32 ^ iy as u32) & 0x3f) as usize;
    row * AE_TABLE_SIZE + col
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assert_close(left: f32, right: f32) {
        assert!((left - right).abs() < 0.0001, "left={left}, right={right}");
    }

    struct Eff060TraceSidecarFrame {
        time: f64,
        evolution: f32,
        field_hash: u64,
        out_of_bounds_count: u32,
        center_displacement: [f32; 2],
        center_source_uv: [f32; 2],
        center_sample_xy: [u32; 2],
    }

    #[test]
    fn zero_amount_is_pass_through() {
        let input = Canvas::new(2, 1, [1, 2, 3, 4]);

        let output = TurbulentDisplace::default()
            .render(
                &input,
                &EffectContext {
                    time: 1.0,
                    fps: 30.0,
                },
                &json!({ "amount": 0 }),
            )
            .unwrap();

        assert_eq!(output.data, input.data);
    }

    #[test]
    fn displacement_is_deterministic_and_moves_pixels() {
        let mut input = Canvas::transparent(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                input.set_pixel(x, y, [(x * 13) as u8, (y * 17) as u8, 0, 255]);
            }
        }
        let ctx = EffectContext {
            time: 0.0,
            fps: 30.0,
        };
        let params = json!({ "amount": 45, "size": 65, "complexity": 2, "evolution": 0 });

        let first = TurbulentDisplace::default()
            .render(&input, &ctx, &params)
            .unwrap();
        let second = TurbulentDisplace::default()
            .render(&input, &ctx, &params)
            .unwrap();

        assert_eq!(first.data, second.data);
        assert_ne!(first.data, input.data);
    }

    #[test]
    fn params_accept_ae_numbered_values() {
        let params = TurbulentDisplaceParams::from_json(
            &json!({
                "0001": { "value": 1 },
                "0002": { "value": 18 },
                "0003": { "value": 32 },
                "0004": { "value": [12, 24] },
                "0005": { "value": 4 },
                "0006": { "value": 90 },
                "0008": { "value": 1 },
                "0009": { "value": 2 },
                "0010": { "value": 7 },
                "0014": { "value": 0 },
                "0012": { "value": 3 },
                "0013": { "value": 1 }
            }),
            1.0,
        );

        assert_eq!(params.displacement_type, 1.0);
        assert_eq!(params.amount, 18.0);
        assert_eq!(params.size, 32.0);
        assert_eq!(params.offset, [12.0, 24.0]);
        assert_eq!(params.complexity, 4.0);
        assert_eq!(params.evolution, 90.0);
        assert!(params.cycle_evolution);
        assert_eq!(params.cycle_revolutions, 2.0);
        assert_eq!(params.random_seed, 7.0);
        assert!(!params.antialiasing_best_quality);
        assert_eq!(params.pinning, 3.0);
        assert!(params.resize_layer);
    }

    #[test]
    fn params_accept_time_varying_numbered_evolution() {
        let params = TurbulentDisplaceParams::from_json(
            &json!({
                "0006": {
                    "keyframes": [
                        { "t": 0.0, "v": 0.0 },
                        { "t": 2.0, "v": 180.0 }
                    ]
                }
            }),
            1.0,
        );

        assert_eq!(params.evolution, 90.0);
    }

    #[test]
    fn field_telemetry_reports_resolved_params_samples_and_hash() {
        let input = Canvas::transparent(5, 5);
        let params = json!({
            "amount": 16,
            "size": 3,
            "complexity": 2,
            "evolution": 90
        });

        let telemetry = turbulent_displace_field_telemetry(&input, &params, 0.0);
        let same = turbulent_displace_field_telemetry(&input, &params, 0.0);

        assert_eq!(telemetry.raw_params, params);
        assert_eq!(telemetry.resolved.amount, 16.0);
        assert_eq!(telemetry.resolved.size, 3.0);
        assert_eq!(telemetry.resolved.complexity, 2);
        assert_eq!(telemetry.resolved.evolution, 90.0);
        assert!(!telemetry.resolved.cycle_evolution);
        assert_eq!(telemetry.resolved.cycle_revolutions, 1.0);
        assert!(telemetry.resolved.antialiasing_best_quality);
        assert_close(
            telemetry.resolved.amplitude,
            16.0 * 3.0 * AE_TURBULENT_AMOUNT_SIZE_SCALE,
        );
        assert_close(
            telemetry.resolved.phase_radians,
            std::f32::consts::FRAC_PI_2,
        );
        assert_eq!(telemetry.ae_wrapper.state_block_bytes, 0x8130);
        assert_eq!(telemetry.ae_wrapper.gpu_param_block_bytes, 0x405c);
        assert_eq!(telemetry.ae_wrapper.noise_table_rows, 64);
        assert_eq!(
            telemetry.ae_wrapper.kernel_path,
            "TurbulentDisplaceFracAllKernel"
        );
        assert_eq!(telemetry.ae_wrapper.inferred_internal_displacement_mode, 1);
        assert_eq!(telemetry.ae_wrapper.amount_fixed16, 16 * 65_536);
        assert_eq!(telemetry.ae_wrapper.size_fixed16, 3 * 65_536);
        assert_eq!(telemetry.ae_wrapper.evolution_fixed16, 90 * 65_536);
        assert!(!telemetry.ae_wrapper.cycle_evolution);
        assert_eq!(telemetry.ae_wrapper.cycle_revolutions_fixed16, 65_536);
        assert_eq!(telemetry.ae_wrapper.random_seed_fixed16, 0);
        assert!(telemetry.ae_wrapper.antialiasing_best_quality);
        assert_eq!(telemetry.ae_wrapper.complexity_octaves, 2);
        assert_close(telemetry.ae_wrapper.complexity_fraction, 0.0);
        assert_eq!(telemetry.field_state.model, "ae_lcg_table_noise_v1");
        assert_eq!(
            telemetry.field_state.coordinate_space,
            "output_pixel_to_source_uv"
        );
        assert_eq!(
            telemetry.field_state.dispatch_path,
            "TurbulentDisplaceFracAllKernel"
        );
        assert_eq!(telemetry.field_state.complexity_octaves, 2);
        assert_close(telemetry.field_state.complexity_fraction, 0.0);
        assert_eq!(telemetry.field_state.evolution_degrees, 90.0);
        assert!(!telemetry.field_state.cycle_evolution);
        assert_eq!(telemetry.field_state.cycle_revolutions, 1.0);
        assert!(telemetry.field_state.antialiasing_best_quality);
        assert_eq!(
            telemetry.field_state.property_mapping_status,
            "0008_cycle_evolution_0009_cycle_revolutions_0010_random_seed_0014_antialiasing_recorded"
        );
        assert_eq!(
            telemetry.field_state.tuning_guardrail,
            "do_not_tune_from_final_png_only"
        );
        assert_eq!(
            telemetry.sampler_mode,
            "pf_subpixel_sample_straight_bilinear"
        );
        assert_eq!(telemetry.edge_policy, "clamp_edges_pinning");
        assert_eq!(telemetry.samples.len(), 9);
        assert_eq!(telemetry.field_hash, same.field_hash);
        assert_eq!(telemetry.samples, same.samples);

        let center = telemetry
            .samples
            .iter()
            .find(|sample| sample.output_xy == [2, 2])
            .unwrap();
        assert_close(
            center.displacement[0],
            center.noise[0] * telemetry.resolved.amplitude,
        );
        assert_close(
            center.displacement[1],
            center.noise[1] * telemetry.resolved.amplitude,
        );
        assert_close(center.source_uv[0], 2.0 + center.displacement[0]);
        assert_close(center.source_uv[1], 2.0 + center.displacement[1]);
    }

    #[test]
    fn public_field_sample_export_matches_telemetry_samples() {
        let input = Canvas::transparent(5, 5);
        let params = json!({
            "amount": 16,
            "size": 3,
            "complexity": 2,
            "evolution": 90
        });
        let points = [[0, 0], [2, 2], [4, 4]];

        let exported = turbulent_displace_field_samples(&input, &params, 0.0, &points);
        let telemetry = turbulent_displace_field_telemetry(&input, &params, 0.0);

        assert_eq!(exported.len(), points.len());
        for sample in exported {
            let expected = telemetry
                .samples
                .iter()
                .find(|candidate| candidate.output_xy == sample.output_xy)
                .unwrap();
            assert_eq!(&sample, expected);
        }
    }

    #[test]
    fn zero_amount_field_telemetry_has_zero_displacement() {
        let input = Canvas::transparent(3, 3);
        let params = json!({
            "amount": 0,
            "size": 4,
            "complexity": 3,
            "evolution": 90
        });
        let changed_evolution = json!({
            "amount": 0,
            "size": 4,
            "complexity": 3,
            "evolution": 180
        });

        let telemetry = turbulent_displace_field_telemetry(&input, &params, 0.0);
        let other = turbulent_displace_field_telemetry(&input, &changed_evolution, 0.0);

        assert_eq!(telemetry.resolved.amount, 0.0);
        assert_eq!(telemetry.resolved.amplitude, 0.0);
        assert_eq!(telemetry.out_of_bounds_count, 0);
        assert_eq!(telemetry.field_hash, other.field_hash);
        for sample in telemetry.samples {
            assert_eq!(sample.displacement, [0.0, 0.0]);
            assert_eq!(
                sample.source_uv,
                [sample.output_xy[0] as f32, sample.output_xy[1] as f32]
            );
            assert_eq!(sample.sample_xy, Some(sample.output_xy));
            assert!(!sample.out_of_bounds);
        }
    }

    #[test]
    fn evolution_changes_field_hash_and_probe_displacement() {
        let input = Canvas::transparent(5, 5);
        let params = json!({
            "amount": 16,
            "size": 3,
            "complexity": 2,
            "0006": {
                "keyframes": [
                    { "t": 0.0, "v": 0.0 },
                    { "t": 1.0, "v": 180.0 }
                ]
            }
        });

        let start = turbulent_displace_field_telemetry(&input, &params, 0.0);
        let end = turbulent_displace_field_telemetry(&input, &params, 1.0);

        assert_eq!(start.resolved.evolution, 0.0);
        assert_eq!(end.resolved.evolution, 180.0);
        assert_ne!(start.field_hash, end.field_hash);

        let start_center = start
            .samples
            .iter()
            .find(|sample| sample.output_xy == [2, 2])
            .unwrap();
        let end_center = end
            .samples
            .iter()
            .find(|sample| sample.output_xy == [2, 2])
            .unwrap();
        assert_ne!(start_center.displacement, end_center.displacement);
    }

    #[test]
    fn resolved_params_capture_current_field_model_bounds() {
        let params = TurbulentDisplaceParams {
            displacement_type: 99.0,
            amount: 999.0,
            size: -12.0,
            offset: [10.0, 20.0],
            offset_is_default: false,
            complexity: 9.2,
            evolution: 450.0,
            cycle_evolution: true,
            cycle_revolutions: -2.0,
            random_seed: 7.0,
            antialiasing_best_quality: false,
            pinning: 99.0,
            resize_layer: true,
        };

        let resolved = params.resolved();

        assert_eq!(resolved.displacement_type, 9);
        assert_eq!(resolved.amount, 200.0);
        assert_eq!(resolved.size, 2.0);
        assert_eq!(resolved.offset, [10.0, 20.0]);
        assert_eq!(resolved.complexity, 6);
        assert_eq!(resolved.evolution, 450.0);
        assert!(resolved.cycle_evolution);
        assert_eq!(resolved.cycle_revolutions, 0.0);
        assert_eq!(resolved.random_seed, 7);
        assert!(!resolved.antialiasing_best_quality);
        assert_eq!(resolved.pinning, 17);
        assert!(resolved.resize_layer);
        assert_close(
            resolved.amplitude,
            200.0 * 2.0 * AE_TURBULENT_AMOUNT_SIZE_SCALE,
        );
        assert_close(
            resolved.phase_radians,
            450.0_f32.to_radians() + seed_phase(7),
        );
    }

    #[test]
    fn ae_probe_controls_affect_field_and_are_preserved_in_raw_params() {
        let input = Canvas::transparent(7, 7);
        let base = json!({
            "0001": 1,
            "0002": 16,
            "0003": 3,
            "0004": [0, 0],
            "0005": 2,
            "0006": 90,
            "0010": 0,
            "0012": 3,
            "0013": 0
        });
        let with_modeled_controls = json!({
            "0001": 2,
            "0002": 16,
            "0003": 3,
            "0004": [4, 5],
            "0005": 2,
            "0006": 90,
            "0010": 42,
            "0012": 0,
            "0013": true
        });

        let baseline = turbulent_displace_field_telemetry(&input, &base, 0.0);
        let current = turbulent_displace_field_telemetry(&input, &with_modeled_controls, 0.0);

        assert_ne!(current.resolved, baseline.resolved);
        assert_ne!(current.samples, baseline.samples);
        assert_ne!(current.field_hash, baseline.field_hash);
        assert_eq!(current.resolved.displacement_type, 2);
        assert_eq!(current.resolved.offset, [4.0, 5.0]);
        assert_eq!(current.resolved.random_seed, 42);
        assert_eq!(current.resolved.pinning, 0);
        assert!(current.resolved.resize_layer);
        assert_eq!(current.edge_policy, "clamp_edges_resize_layer");
        assert_eq!(current.raw_params, with_modeled_controls);
    }

    #[test]
    fn ae_wrapper_telemetry_reports_inferred_frac1d_lookup_shape() {
        let input = Canvas::transparent(11, 13);
        let params = json!({
            "0001": 9,
            "0002": 45,
            "0003": 65,
            "0004": [2.5, -3.0],
            "0005": 2.75,
            "0006": 180
        });

        let telemetry = turbulent_displace_field_telemetry(&input, &params, 0.0);

        assert_eq!(
            telemetry.ae_wrapper.kernel_path,
            "TurbulentDisplaceFrac1DKernel"
        );
        assert_eq!(
            telemetry.field_state.dispatch_path,
            "TurbulentDisplaceFrac1DKernel"
        );
        assert_eq!(telemetry.ae_wrapper.inferred_internal_displacement_mode, 11);
        assert!(telemetry.ae_wrapper.uses_h_lookup);
        assert!(telemetry.ae_wrapper.uses_v_lookup);
        assert_eq!(telemetry.ae_wrapper.h_lookup_len, input.width + 2);
        assert_eq!(telemetry.ae_wrapper.v_lookup_len, input.height + 2);
        assert_eq!(telemetry.ae_wrapper.offset_fixed16, [163_840, -196_608]);
        assert_eq!(telemetry.ae_wrapper.complexity_octaves, 2);
        assert_close(telemetry.ae_wrapper.complexity_fraction, 0.75);
    }

    #[test]
    fn eff_060_round2_integrated_trace_sidecars_match_field_model() {
        let input = Canvas::transparent(512, 512);
        let params = json!({
            "0002": 45,
            "0003": 65,
            "0005": 2,
            "0006": {
                "keyframes": [
                    { "t": 0.0, "v": 0.0 },
                    { "t": 2.0, "v": 180.0 }
                ]
            }
        });
        let expected = [
            Eff060TraceSidecarFrame {
                time: 0.0,
                evolution: 0.0,
                field_hash: 2683613110696052001,
                out_of_bounds_count: 11351,
                center_displacement: [7.687488, -6.037524],
                center_source_uv: [263.6875, 249.96248],
                center_sample_xy: [264, 250],
            },
            Eff060TraceSidecarFrame {
                time: 0.5,
                evolution: 45.0,
                field_hash: 15425239775886312303,
                out_of_bounds_count: 7082,
                center_displacement: [-1.7932884, -5.3806043],
                center_source_uv: [254.20671, 250.6194],
                center_sample_xy: [254, 251],
            },
            Eff060TraceSidecarFrame {
                time: 1.0,
                evolution: 90.0,
                field_hash: 1218132080864211637,
                out_of_bounds_count: 3103,
                center_displacement: [-4.21859, -3.4364386],
                center_source_uv: [251.7814, 252.56357],
                center_sample_xy: [252, 253],
            },
            Eff060TraceSidecarFrame {
                time: 1.5,
                evolution: 135.0,
                field_hash: 4697150789677612090,
                out_of_bounds_count: 4222,
                center_displacement: [1.5609949, -4.2884946],
                center_source_uv: [257.561, 251.7115],
                center_sample_xy: [258, 252],
            },
        ];

        for expected in expected {
            let telemetry = turbulent_displace_field_telemetry(&input, &params, expected.time);

            assert_eq!(telemetry.resolved.amount, 45.0);
            assert_eq!(telemetry.resolved.size, 65.0);
            assert_eq!(telemetry.resolved.complexity, 2);
            assert_close(
                telemetry.resolved.amplitude,
                45.0 * 65.0 * AE_TURBULENT_AMOUNT_SIZE_SCALE,
            );
            assert_eq!(telemetry.resolved.evolution, expected.evolution);
            assert_eq!(telemetry.sampler_mode, TURBULENT_SAMPLER_MODE);
            assert_eq!(telemetry.edge_policy, "clamp_edges_pinning");
            assert_eq!(telemetry.field_hash, expected.field_hash);
            assert_eq!(telemetry.out_of_bounds_count, expected.out_of_bounds_count);

            let center = telemetry
                .samples
                .iter()
                .find(|sample| sample.output_xy == [256, 256])
                .unwrap();
            assert_close(center.displacement[0], expected.center_displacement[0]);
            assert_close(center.displacement[1], expected.center_displacement[1]);
            assert_close(center.source_uv[0], expected.center_source_uv[0]);
            assert_close(center.source_uv[1], expected.center_source_uv[1]);
            assert_eq!(center.sample_xy, Some(expected.center_sample_xy));
            assert!(!center.out_of_bounds);
        }
    }

    #[test]
    fn ae_table_matches_frida_cmd11_state_for_single_frame_trace() {
        let input = Canvas::transparent(512, 512);
        let resolved = TurbulentDisplaceParams::from_json(
            &json!({
                "0001": 1,
                "0002": 45,
                "0003": 65,
                "0005": 2,
                "0006": 0,
                "0010": 0,
                "0012": 3,
                "0013": false,
                "0014": true
            }),
            0.0,
        )
        .resolved_for_input(&input);
        let table = ae_turbulent_table(resolved);
        let expected = [
            0.456_588_047_941_204_17,
            0.122_241_144_698_637_07,
            0.096_215_241_219_060_44,
            0.682_504_073_069_549_5,
            0.041_757_244_616_746_9,
            -0.346_158_675_795_971_1,
            0.219_985_181_104_799_48,
            0.539_152_087_685_922_2,
            -0.144_988_835_470_294_62,
            -0.083_446_784_209_309_05,
            0.370_167_017_696_028_1,
            0.739_003_158_013_585_5,
        ];
        for (index, expected) in expected.iter().copied().enumerate() {
            assert!(
                (table[index] - expected).abs() < 1e-12,
                "table[{index}] native={} ae={expected}",
                table[index]
            );
        }
    }
}
