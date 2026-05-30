use raster_cpu::Canvas;

use crate::{AreEventStream, EventClass};

pub const P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR: &str =
    "AE_NATIVE_RENDERER_P6_AD68_TEXT_PIXELS_OPT_IN";
pub const P6_AD68_TEXT_PIXELS_HANDOFF_ENV_VAR: &str =
    "AE_NATIVE_RENDERER_P6_AD68_TEXT_PIXELS_HANDOFF";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum P6Ad68PixelOptInFlagState {
    EnabledDefault,
    DisabledExplicit,
    DisabledInvalid,
    Enabled,
}

impl P6Ad68PixelOptInFlagState {
    pub fn enabled(self) -> bool {
        matches!(self, Self::EnabledDefault | Self::Enabled)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnabledDefault => "enabled_default",
            Self::DisabledExplicit => "disabled_explicit",
            Self::DisabledInvalid => "disabled_invalid",
            Self::Enabled => "enabled",
        }
    }
}

pub fn p6_ad68_pixel_opt_in_enabled() -> bool {
    p6_ad68_pixel_opt_in_flag_state().enabled()
}

pub fn p6_ad68_pixel_opt_in_flag_state() -> P6Ad68PixelOptInFlagState {
    p6_ad68_pixel_opt_in_flag_state_from_value(
        std::env::var(P6_AD68_TEXT_PIXELS_OPT_IN_ENV_VAR).ok(),
    )
}

fn p6_ad68_pixel_opt_in_flag_state_from_value(value: Option<String>) -> P6Ad68PixelOptInFlagState {
    let Some(value) = value else {
        return P6Ad68PixelOptInFlagState::EnabledDefault;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "false" | "off" | "no" | "disabled" => {
            P6Ad68PixelOptInFlagState::DisabledExplicit
        }
        "1" | "true" | "on" | "yes" | "enabled" | "p6_ad68_pixels" | "p6-ad68-pixels" => {
            P6Ad68PixelOptInFlagState::Enabled
        }
        _ => P6Ad68PixelOptInFlagState::DisabledInvalid,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum P6Ad68PixelHandoffMode {
    SupersampleDownsample,
    DirectByteToCanvas,
}

impl P6Ad68PixelHandoffMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SupersampleDownsample => {
                "outline_supersample_domain_downsampled_to_canvas_pixels"
            }
            Self::DirectByteToCanvas => "ad68_pixel_domain_direct_byte_to_canvas",
        }
    }

    pub fn coordinate_basis(self) -> &'static str {
        self.as_str()
    }

    pub fn source_path_supersample(self, coverage_supersample: u32) -> u32 {
        match self {
            Self::SupersampleDownsample => coverage_supersample,
            Self::DirectByteToCanvas => 1,
        }
    }

    pub fn coordinate_scale(self, coverage_supersample: u32) -> u32 {
        match self {
            Self::SupersampleDownsample => coverage_supersample.max(1),
            Self::DirectByteToCanvas => 1,
        }
    }
}

pub fn p6_ad68_pixel_handoff_mode() -> P6Ad68PixelHandoffMode {
    p6_ad68_pixel_handoff_mode_from_value(std::env::var(P6_AD68_TEXT_PIXELS_HANDOFF_ENV_VAR).ok())
}

fn p6_ad68_pixel_handoff_mode_from_value(value: Option<String>) -> P6Ad68PixelHandoffMode {
    let Some(value) = value else {
        return P6Ad68PixelHandoffMode::DirectByteToCanvas;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "default" | "preferred" => P6Ad68PixelHandoffMode::DirectByteToCanvas,
        "direct"
        | "direct_byte"
        | "direct-byte"
        | "ad68_direct"
        | "ad68_pixel_domain_direct_byte_to_canvas" => P6Ad68PixelHandoffMode::DirectByteToCanvas,
        "current"
        | "supersample"
        | "downsample"
        | "outline_supersample_domain_downsampled_to_canvas_pixels" => {
            P6Ad68PixelHandoffMode::SupersampleDownsample
        }
        _ => P6Ad68PixelHandoffMode::DirectByteToCanvas,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P6Ad68PixelPlacement {
    pub origin_x: i32,
    pub origin_y: i32,
    pub width: usize,
    pub height: usize,
    pub clip: Option<[i32; 4]>,
    pub coordinate_scale: u32,
}

impl P6Ad68PixelPlacement {
    pub fn pixel_domain(origin_x: i32, origin_y: i32, width: usize, height: usize) -> Self {
        Self {
            origin_x,
            origin_y,
            width,
            height,
            clip: None,
            coordinate_scale: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P6Ad68PixelWriteReport {
    pub pixel_write_count: usize,
    pub nonzero_coverage_pixel_count: usize,
    pub source_sample_count: usize,
    pub class0_sample_count: usize,
    pub class1_sample_count: usize,
    pub class2_sample_count: usize,
    pub class0_span_count: usize,
    pub class1_span_count: usize,
    pub class2_span_count: usize,
    pub class2_byte_count: usize,
    pub source_coverage_byte_sum: u64,
    pub final_coverage_byte_sum: u64,
    pub clipped_sample_count: usize,
    pub used_coverage_bitmap_for_pixel_bytes: bool,
    pub used_coverage_rows_for_pixel_bytes: bool,
    pub used_typed_span_proof: bool,
    pub used_fixture_payload: bool,
    pub used_synthetic_payload: bool,
}

impl P6Ad68PixelWriteReport {
    fn new() -> Self {
        Self {
            pixel_write_count: 0,
            nonzero_coverage_pixel_count: 0,
            source_sample_count: 0,
            class0_sample_count: 0,
            class1_sample_count: 0,
            class2_sample_count: 0,
            class0_span_count: 0,
            class1_span_count: 0,
            class2_span_count: 0,
            class2_byte_count: 0,
            source_coverage_byte_sum: 0,
            final_coverage_byte_sum: 0,
            clipped_sample_count: 0,
            used_coverage_bitmap_for_pixel_bytes: false,
            used_coverage_rows_for_pixel_bytes: false,
            used_typed_span_proof: false,
            used_fixture_payload: false,
            used_synthetic_payload: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum P6Ad68PixelWriteError {
    InvalidCoordinateScale,
    InvalidPlacementSize {
        width: usize,
        height: usize,
    },
    InvalidEventIndex {
        row_y: i32,
        event_index: usize,
        objects_len: usize,
    },
    InvalidCursorWidth {
        row_y: i32,
        current_x: i32,
        next_x: i32,
    },
    MissingPayloadWindow {
        row_y: i32,
        event_index: usize,
    },
    MissingPayloadBacking {
        row_y: i32,
        event_index: usize,
        backing_id: usize,
    },
    InvalidPayloadWindow {
        row_y: i32,
        event_index: usize,
        offset: usize,
        len: usize,
        backing_len: usize,
    },
    PayloadWindowShorterThanWidth {
        row_y: i32,
        event_index: usize,
        window_len: usize,
        width: usize,
    },
}

pub fn blend_p6_ad68_event_stream_to_canvas(
    canvas: &mut Canvas,
    stream: &AreEventStream,
    placement: P6Ad68PixelPlacement,
    color: [u8; 4],
) -> Result<P6Ad68PixelWriteReport, P6Ad68PixelWriteError> {
    let scale = placement.coordinate_scale as usize;
    if scale == 0 {
        return Err(P6Ad68PixelWriteError::InvalidCoordinateScale);
    }
    if placement.width == 0 || placement.height == 0 {
        return Err(P6Ad68PixelWriteError::InvalidPlacementSize {
            width: placement.width,
            height: placement.height,
        });
    }

    let mut report = P6Ad68PixelWriteReport::new();
    let mut coverage_accum = vec![0u32; placement.width * placement.height];
    let divisor = (scale * scale) as u32;

    for row in &stream.rows {
        for cursor in &row.cursors {
            let event = stream.objects.get(cursor.event_index).ok_or(
                P6Ad68PixelWriteError::InvalidEventIndex {
                    row_y: row.row_y,
                    event_index: cursor.event_index,
                    objects_len: stream.objects.len(),
                },
            )?;
            let width = cursor.next_x.checked_sub(cursor.current_x).ok_or(
                P6Ad68PixelWriteError::InvalidCursorWidth {
                    row_y: row.row_y,
                    current_x: cursor.current_x,
                    next_x: cursor.next_x,
                },
            )?;
            if width < 0 {
                return Err(P6Ad68PixelWriteError::InvalidCursorWidth {
                    row_y: row.row_y,
                    current_x: cursor.current_x,
                    next_x: cursor.next_x,
                });
            }
            let width = width as usize;

            match event.event_class {
                EventClass::Class0 => {
                    report.class0_span_count += 1;
                    report.class0_sample_count += width;
                    report.source_sample_count += width;
                }
                EventClass::Class1 => {
                    report.class1_span_count += 1;
                    report.class1_sample_count += width;
                    accumulate_constant_coverage(
                        &mut coverage_accum,
                        placement.width,
                        placement.height,
                        scale,
                        row.row_y,
                        cursor.current_x,
                        width,
                        u8::MAX,
                        &mut report,
                    );
                }
                EventClass::Class2 => {
                    report.class2_span_count += 1;
                    let payload_window = event.payload_window.ok_or(
                        P6Ad68PixelWriteError::MissingPayloadWindow {
                            row_y: row.row_y,
                            event_index: cursor.event_index,
                        },
                    )?;
                    let backing = stream
                        .payload_backings
                        .iter()
                        .find(|backing| backing.id == payload_window.backing_id)
                        .ok_or(P6Ad68PixelWriteError::MissingPayloadBacking {
                            row_y: row.row_y,
                            event_index: cursor.event_index,
                            backing_id: payload_window.backing_id.0,
                        })?;
                    let payload_end = payload_window
                        .offset
                        .checked_add(payload_window.len)
                        .ok_or(P6Ad68PixelWriteError::InvalidPayloadWindow {
                            row_y: row.row_y,
                            event_index: cursor.event_index,
                            offset: payload_window.offset,
                            len: payload_window.len,
                            backing_len: backing.bytes.len(),
                        })?;
                    if payload_end > backing.bytes.len() {
                        return Err(P6Ad68PixelWriteError::InvalidPayloadWindow {
                            row_y: row.row_y,
                            event_index: cursor.event_index,
                            offset: payload_window.offset,
                            len: payload_window.len,
                            backing_len: backing.bytes.len(),
                        });
                    }
                    if payload_window.len < width {
                        return Err(P6Ad68PixelWriteError::PayloadWindowShorterThanWidth {
                            row_y: row.row_y,
                            event_index: cursor.event_index,
                            window_len: payload_window.len,
                            width,
                        });
                    }
                    let payload =
                        &backing.bytes[payload_window.offset..payload_window.offset + width];
                    report.class2_byte_count += payload.len();
                    report.class2_sample_count += payload.len();
                    accumulate_payload_coverage(
                        &mut coverage_accum,
                        placement.width,
                        placement.height,
                        scale,
                        row.row_y,
                        cursor.current_x,
                        payload,
                        &mut report,
                    );
                }
            }
        }
    }

    let clip = placement
        .clip
        .unwrap_or([0, 0, canvas.width as i32, canvas.height as i32]);
    for by in 0..placement.height {
        for bx in 0..placement.width {
            let accumulated = coverage_accum[by * placement.width + bx];
            if accumulated == 0 || color[3] == 0 {
                continue;
            }
            let coverage = ((accumulated + divisor / 2) / divisor).min(u8::MAX as u32) as u8;
            if coverage == 0 {
                continue;
            }
            report.nonzero_coverage_pixel_count += 1;
            report.final_coverage_byte_sum += u64::from(coverage);

            let px = placement.origin_x + bx as i32;
            let py = placement.origin_y + by as i32;
            if px < clip[0] || py < clip[1] || px >= clip[2] || py >= clip[3] {
                continue;
            }
            if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                continue;
            }

            let dst = canvas.pixel(px as u32, py as u32);
            let out = blend_text_pixel_ae_u8(dst, color, coverage);
            canvas.set_pixel(px as u32, py as u32, out);
            report.pixel_write_count += 1;
        }
    }

    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn accumulate_constant_coverage(
    coverage_accum: &mut [u32],
    width: usize,
    height: usize,
    scale: usize,
    row_y: i32,
    start_x: i32,
    len: usize,
    coverage: u8,
    report: &mut P6Ad68PixelWriteReport,
) {
    for offset in 0..len {
        accumulate_source_sample(
            coverage_accum,
            width,
            height,
            scale,
            row_y,
            start_x + offset as i32,
            coverage,
            report,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn accumulate_payload_coverage(
    coverage_accum: &mut [u32],
    width: usize,
    height: usize,
    scale: usize,
    row_y: i32,
    start_x: i32,
    payload: &[u8],
    report: &mut P6Ad68PixelWriteReport,
) {
    for (offset, coverage) in payload.iter().copied().enumerate() {
        accumulate_source_sample(
            coverage_accum,
            width,
            height,
            scale,
            row_y,
            start_x + offset as i32,
            coverage,
            report,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn accumulate_source_sample(
    coverage_accum: &mut [u32],
    width: usize,
    height: usize,
    scale: usize,
    row_y: i32,
    x: i32,
    coverage: u8,
    report: &mut P6Ad68PixelWriteReport,
) {
    report.source_sample_count += 1;
    report.source_coverage_byte_sum += u64::from(coverage);
    if coverage == 0 {
        return;
    }
    let bx = x.div_euclid(scale as i32);
    let by = row_y.div_euclid(scale as i32);
    if bx < 0 || by < 0 || bx >= width as i32 || by >= height as i32 {
        report.clipped_sample_count += 1;
        return;
    }
    coverage_accum[by as usize * width + bx as usize] += u32::from(coverage);
}

fn blend_text_pixel_ae_u8(dst: [u8; 4], src: [u8; 4], coverage: u8) -> [u8; 4] {
    let src_alpha = if coverage == u8::MAX {
        src[3]
    } else {
        mul_u8_ae(src[3], coverage)
    };
    if src_alpha == 0 {
        return dst;
    }
    if src_alpha == u8::MAX {
        return [src[0], src[1], src[2], src_alpha];
    }

    let dst_alpha = dst[3];
    if dst_alpha == 0 {
        return [src[0], src[1], src[2], src_alpha];
    }

    let inv_mul = mul_u8_ae(u8::MAX - src_alpha, u8::MAX - dst_alpha);
    let out_alpha = u8::MAX - inv_mul;
    let mut out = [0u8; 4];
    for channel in 0..3 {
        let dst_premul = mul_u8_ae(dst[channel], dst_alpha) as i32;
        let src_delta = src[channel] as i32 - dst_premul;
        let out_premul = dst_premul + div255_signed_ae(src_delta * src_alpha as i32);
        out[channel] = unpremultiply_u8_ae(out_premul, out_alpha);
    }
    out[3] = out_alpha;
    out
}

fn mul_u8_ae(a: u8, b: u8) -> u8 {
    let x = a as u32 * b as u32 + 0x80;
    (((x >> 8) + x) >> 8).min(255) as u8
}

fn div255_signed_ae(value: i32) -> i32 {
    let x = value + 0x80;
    (x + (x >> 8)) >> 8
}

fn unpremultiply_u8_ae(premul: i32, alpha: u8) -> u8 {
    if alpha == 0 {
        return 0;
    }
    ((premul * 255 + alpha as i32 / 2) / alpha as i32).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AreCursorState, AreEventObject, AreEventRow, ArePayloadBacking, ArePayloadBackingId,
        ArePayloadWindow,
    };

    fn row(row_y: i32, cursors: Vec<AreCursorState>) -> AreEventRow {
        AreEventRow { row_y, cursors }
    }

    fn cursor(current_x: i32, next_x: i32, event_index: usize) -> AreCursorState {
        AreCursorState {
            current_x,
            next_x,
            event_index,
            materialize_flag: false,
        }
    }

    fn object(event_class: EventClass) -> AreEventObject {
        AreEventObject {
            event_class,
            payload_window: None,
            state: 0,
        }
    }

    fn class2(offset: usize, len: usize) -> AreEventObject {
        AreEventObject {
            event_class: EventClass::Class2,
            payload_window: Some(ArePayloadWindow {
                backing_id: ArePayloadBackingId(0),
                offset,
                len,
            }),
            state: 0,
        }
    }

    fn stream(
        rows: Vec<AreEventRow>,
        objects: Vec<AreEventObject>,
        bytes: &[u8],
    ) -> AreEventStream {
        AreEventStream {
            y_min: 0,
            y_max: rows.last().map(|row| row.row_y).unwrap_or(0),
            rows,
            objects,
            payload_backings: vec![ArePayloadBacking {
                id: ArePayloadBackingId(0),
                bytes: bytes.to_vec(),
            }],
        }
    }

    #[test]
    fn p6_ad68_pixel_path_enabled_by_default() {
        assert_eq!(
            p6_ad68_pixel_opt_in_flag_state_from_value(None),
            P6Ad68PixelOptInFlagState::EnabledDefault
        );
        assert!(p6_ad68_pixel_opt_in_flag_state_from_value(None).enabled());
    }

    #[test]
    fn p6_ad68_pixel_env_still_supports_explicit_enable_disable_and_invalid_safe_off() {
        assert_eq!(
            p6_ad68_pixel_opt_in_flag_state_from_value(Some("1".to_string())),
            P6Ad68PixelOptInFlagState::Enabled
        );
        assert!(p6_ad68_pixel_opt_in_flag_state_from_value(Some("1".to_string())).enabled());
        assert_eq!(
            p6_ad68_pixel_opt_in_flag_state_from_value(Some("0".to_string())),
            P6Ad68PixelOptInFlagState::DisabledExplicit
        );
        assert!(!p6_ad68_pixel_opt_in_flag_state_from_value(Some("0".to_string())).enabled());
        assert_eq!(
            p6_ad68_pixel_opt_in_flag_state_from_value(Some("wat".to_string())),
            P6Ad68PixelOptInFlagState::DisabledInvalid
        );
        assert!(!p6_ad68_pixel_opt_in_flag_state_from_value(Some("wat".to_string())).enabled());
    }

    #[test]
    fn p6_ad68_pixel_handoff_defaults_to_direct_byte() {
        assert_eq!(
            p6_ad68_pixel_handoff_mode_from_value(None),
            P6Ad68PixelHandoffMode::DirectByteToCanvas
        );
        assert_eq!(
            p6_ad68_pixel_handoff_mode_from_value(Some("wat".to_string())),
            P6Ad68PixelHandoffMode::DirectByteToCanvas
        );
        assert_eq!(
            p6_ad68_pixel_handoff_mode_from_value(Some("default".to_string())),
            P6Ad68PixelHandoffMode::DirectByteToCanvas
        );
    }

    #[test]
    fn p6_ad68_pixel_handoff_current_downsample_is_explicit_override() {
        let mode = p6_ad68_pixel_handoff_mode_from_value(Some("current".to_string()));
        assert_eq!(mode, P6Ad68PixelHandoffMode::SupersampleDownsample);
        assert_eq!(
            mode.coordinate_basis(),
            "outline_supersample_domain_downsampled_to_canvas_pixels"
        );
        assert_eq!(mode.source_path_supersample(16), 16);
        assert_eq!(mode.coordinate_scale(16), 16);
    }

    #[test]
    fn p6_ad68_pixel_handoff_direct_byte_alias_is_supported() {
        let mode = p6_ad68_pixel_handoff_mode_from_value(Some("direct_byte".to_string()));
        assert_eq!(mode, P6Ad68PixelHandoffMode::DirectByteToCanvas);
        assert_eq!(
            mode.coordinate_basis(),
            "ad68_pixel_domain_direct_byte_to_canvas"
        );
        assert_eq!(mode.source_path_supersample(16), 1);
        assert_eq!(mode.coordinate_scale(16), 1);
    }

    #[test]
    fn p6_ad68_pixel_adapter_writes_class2_payload_bytes() {
        let stream = stream(
            vec![row(0, vec![cursor(0, 2, 0)])],
            vec![class2(0, 2)],
            &[64, 128],
        );
        let mut canvas = Canvas::transparent(4, 2);
        let report = blend_p6_ad68_event_stream_to_canvas(
            &mut canvas,
            &stream,
            P6Ad68PixelPlacement::pixel_domain(0, 0, 4, 2),
            [10, 20, 30, 255],
        )
        .unwrap();

        assert_eq!(report.class2_span_count, 1);
        assert_eq!(report.class2_byte_count, 2);
        assert_eq!(report.class2_sample_count, 2);
        assert_eq!(report.source_sample_count, 2);
        assert_eq!(report.source_coverage_byte_sum, 192);
        assert_eq!(report.final_coverage_byte_sum, 192);
        assert_eq!(report.pixel_write_count, 2);
        assert_eq!(canvas.pixel(0, 0), [10, 20, 30, 64]);
        assert_eq!(canvas.pixel(1, 0), [10, 20, 30, 128]);
    }

    #[test]
    fn p6_ad68_pixel_adapter_does_not_write_class0() {
        let stream = stream(
            vec![row(0, vec![cursor(0, 3, 0)])],
            vec![object(EventClass::Class0)],
            &[],
        );
        let mut canvas = Canvas::transparent(4, 2);
        let report = blend_p6_ad68_event_stream_to_canvas(
            &mut canvas,
            &stream,
            P6Ad68PixelPlacement::pixel_domain(0, 0, 4, 2),
            [255, 255, 255, 255],
        )
        .unwrap();

        assert_eq!(report.class0_span_count, 1);
        assert_eq!(report.class0_sample_count, 3);
        assert_eq!(report.source_sample_count, 3);
        assert_eq!(report.source_coverage_byte_sum, 0);
        assert_eq!(report.final_coverage_byte_sum, 0);
        assert_eq!(report.pixel_write_count, 0);
        assert!(canvas.data.chunks_exact(4).all(|pixel| pixel[3] == 0));
    }

    #[test]
    fn p6_ad68_pixel_adapter_writes_class1_as_full_coverage_span() {
        let stream = stream(
            vec![row(0, vec![cursor(1, 3, 0)])],
            vec![object(EventClass::Class1)],
            &[],
        );
        let mut canvas = Canvas::transparent(4, 2);
        let report = blend_p6_ad68_event_stream_to_canvas(
            &mut canvas,
            &stream,
            P6Ad68PixelPlacement::pixel_domain(0, 0, 4, 2),
            [5, 6, 7, 255],
        )
        .unwrap();

        assert_eq!(report.class1_span_count, 1);
        assert_eq!(report.class1_sample_count, 2);
        assert_eq!(report.source_sample_count, 2);
        assert_eq!(report.source_coverage_byte_sum, 510);
        assert_eq!(report.final_coverage_byte_sum, 510);
        assert_eq!(report.pixel_write_count, 2);
        assert_eq!(canvas.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(canvas.pixel(1, 0), [5, 6, 7, 255]);
        assert_eq!(canvas.pixel(2, 0), [5, 6, 7, 255]);
    }

    #[test]
    fn p6_ad68_pixel_adapter_accumulates_subpixel_domain_samples() {
        let stream = stream(
            vec![row(0, vec![cursor(0, 2, 0)]), row(1, vec![cursor(0, 2, 0)])],
            vec![class2(0, 2)],
            &[255, 255],
        );
        let mut canvas = Canvas::transparent(2, 2);
        let report = blend_p6_ad68_event_stream_to_canvas(
            &mut canvas,
            &stream,
            P6Ad68PixelPlacement {
                origin_x: 0,
                origin_y: 0,
                width: 1,
                height: 1,
                clip: None,
                coordinate_scale: 2,
            },
            [11, 12, 13, 255],
        )
        .unwrap();

        assert_eq!(report.source_sample_count, 4);
        assert_eq!(report.class2_sample_count, 4);
        assert_eq!(report.source_coverage_byte_sum, 1020);
        assert_eq!(report.final_coverage_byte_sum, 255);
        assert_eq!(report.pixel_write_count, 1);
        assert_eq!(canvas.pixel(0, 0), [11, 12, 13, 255]);
    }

    #[test]
    fn p6_ad68_pixel_adapter_clips_to_canvas() {
        let stream = stream(
            vec![row(0, vec![cursor(0, 2, 0)])],
            vec![class2(0, 2)],
            &[255, 255],
        );
        let mut canvas = Canvas::transparent(4, 2);
        let report = blend_p6_ad68_event_stream_to_canvas(
            &mut canvas,
            &stream,
            P6Ad68PixelPlacement {
                origin_x: 0,
                origin_y: 0,
                width: 4,
                height: 2,
                clip: Some([1, 0, 4, 2]),
                coordinate_scale: 1,
            },
            [20, 30, 40, 255],
        )
        .unwrap();

        assert_eq!(report.nonzero_coverage_pixel_count, 2);
        assert_eq!(report.pixel_write_count, 1);
        assert_eq!(canvas.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(canvas.pixel(1, 0), [20, 30, 40, 255]);
    }
}
