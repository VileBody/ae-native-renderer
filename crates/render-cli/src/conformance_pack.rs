use anyhow::{Context, Result};
use raster_cpu::Canvas;
use render_ir::{
    Asset, AssetKind, Composition, CompositionNode, EffectSpec, KeyframeEase, Layer,
    MotionBlurSettings, PositionExpression, Rect, ScalarKeyframe, Scene, TextAnimatorSpec,
    TextExpressionSelector, TextRangeSelector, TextSelectorBasedOn, Transform2D,
    Transform2DAnimation, Transform2DExpression, Vec2Keyframe,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct RunOptions {
    pub pack: PathBuf,
    pub out: PathBuf,
    pub cases: Vec<String>,
    pub threshold_mean: Option<f64>,
    pub threshold_max: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
struct PackManifest {
    schema: String,
    pack_id: String,
    composition: PackComposition,
    cases: Vec<PackCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct PackComposition {
    width: u32,
    height: u32,
    fps: f64,
    case_duration_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct PackCase {
    id: String,
    title: String,
    ladder: String,
    #[serde(default)]
    modules: Vec<String>,
    #[serde(default)]
    frames_to_compare: Vec<u32>,
    #[serde(default)]
    assets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PrimitiveAsset {
    id: String,
    path: String,
    #[serde(default)]
    frames: Option<u32>,
}

#[derive(Debug, Clone)]
struct CaseRecipe {
    scene: Scene,
    notes: Vec<String>,
}

struct CaseBuilder<'a> {
    manifest: &'a PackManifest,
    pack_root: &'a Path,
    case_id: &'a str,
    layers: Vec<Layer>,
    compositions: Vec<CompositionNode>,
    notes: Vec<String>,
}

struct PackFootageProvider {
    pack_root: PathBuf,
    fps: f64,
    assets: HashMap<String, PrimitiveAsset>,
    still_cache: HashMap<String, Canvas>,
    sequence_cache: HashMap<(String, u32), Canvas>,
}

pub fn run_pack(options: RunOptions) -> Result<bool> {
    let started = Instant::now();
    let pack_root = options.pack;
    let manifest = load_manifest(&pack_root)?;
    validate_pack_manifest(&manifest)?;
    let primitive_assets = load_primitive_assets(&pack_root)?;
    let selected_cases = select_cases(&manifest, &options.cases)?;

    fs::create_dir_all(&options.out)?;
    let mut case_reports = Vec::new();
    let mut failures = 0_u32;
    let mut rendered_cases = 0_u32;

    for case in selected_cases {
        let report = match build_recipe(&manifest, &pack_root, case) {
            Ok(recipe) => {
                let mut provider = PackFootageProvider::new(
                    pack_root.clone(),
                    manifest.composition.fps,
                    primitive_assets.clone(),
                );
                run_case(
                    case,
                    recipe,
                    &pack_root,
                    &options.out,
                    &mut provider,
                    options.threshold_mean,
                    options.threshold_max,
                )?
            }
            Err(err) => {
                failures += 1;
                json!({
                    "case": case.id,
                    "title": case.title,
                    "status": "recipe_error",
                    "ok": false,
                    "error": format!("{err:#}")
                })
            }
        };
        if report.get("status").and_then(Value::as_str) != Some("recipe_error") {
            rendered_cases += 1;
        }
        if !report.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            failures += 1;
        }
        case_reports.push(report);
    }

    let ok = failures == 0;
    let report = json!({
        "schema": "ae-native-renderer.native-conformance-report.v1",
        "ok": ok,
        "pack": {
            "root": pack_root.display().to_string(),
            "schema": manifest.schema,
            "pack_id": manifest.pack_id
        },
        "out": options.out.display().to_string(),
        "thresholds": {
            "mean_abs_diff": options.threshold_mean,
            "max_abs_diff": options.threshold_max
        },
        "summary": {
            "cases_requested": case_reports.len(),
            "cases_rendered": rendered_cases,
            "failures": failures,
            "elapsed_ms": elapsed_ms(started)
        },
        "cases": case_reports
    });
    fs::write(
        options.out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    println!(
        "conformance-pack.done ok={} cases={} report={}",
        ok,
        report["summary"]["cases_requested"],
        options.out.join("report.json").display()
    );
    Ok(ok)
}

fn load_manifest(pack_root: &Path) -> Result<PackManifest> {
    let path = pack_root.join("manifest.json");
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn validate_pack_manifest(manifest: &PackManifest) -> Result<()> {
    anyhow::ensure!(
        manifest.schema == "ae-native-renderer.conformance-pack.v1",
        "unsupported conformance pack schema {:?}",
        manifest.schema
    );
    anyhow::ensure!(
        manifest.composition.width > 0 && manifest.composition.height > 0,
        "conformance composition dimensions must be positive"
    );
    anyhow::ensure!(
        manifest.composition.fps > 0.0,
        "conformance composition fps must be positive"
    );
    anyhow::ensure!(!manifest.cases.is_empty(), "conformance pack has no cases");
    Ok(())
}

fn load_primitive_assets(pack_root: &Path) -> Result<HashMap<String, PrimitiveAsset>> {
    let path = pack_root.join("assets/primitives/assets_manifest.json");
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let assets: Vec<PrimitiveAsset> =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    Ok(assets
        .into_iter()
        .map(|asset| (asset.id.clone(), asset))
        .collect())
}

fn select_cases<'a>(manifest: &'a PackManifest, requested: &[String]) -> Result<Vec<&'a PackCase>> {
    if requested.is_empty() {
        return Ok(manifest.cases.iter().collect());
    }

    let by_id = manifest
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    requested
        .iter()
        .map(|id| {
            by_id
                .get(id.as_str())
                .copied()
                .with_context(|| format!("case {id} is missing from conformance pack"))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn run_case(
    case: &PackCase,
    recipe: CaseRecipe,
    pack_root: &Path,
    out_root: &Path,
    provider: &mut PackFootageProvider,
    threshold_mean: Option<f64>,
    threshold_max: Option<u8>,
) -> Result<Value> {
    let case_started = Instant::now();
    let case_dir = out_root.join(&case.id);
    let native_dir = case_dir.join("native");
    let ae_dir = case_dir.join("ae");
    let diff_dir = case_dir.join("diff");
    fs::create_dir_all(&native_dir)?;
    fs::create_dir_all(&ae_dir)?;
    fs::create_dir_all(&diff_dir)?;
    reset_trace_sidecars(&case_dir)?;
    fs::write(
        case_dir.join("scene.json"),
        serde_json::to_string_pretty(&recipe.scene)?,
    )?;

    let mut frame_reports = Vec::new();
    let mut rgba_summary = MetricsAccumulator::new(4);
    let mut rgb_summary = MetricsAccumulator::new(3);
    let mut alpha_summary = MetricsAccumulator::new(1);
    let mut background_alpha_normalized_summary = MetricsAccumulator::new(4);
    let mut foreground_rgb_summary = MetricsAccumulator::new(3);
    let mut rgb_under_alpha_policy_summary = MetricsAccumulator::new(3);
    let mut rgb_over_native_background_summary = MetricsAccumulator::new(3);
    let mut rgb_over_ae_background_summary = MetricsAccumulator::new(3);
    let mut background_corner_alpha_diff_frames = Vec::new();
    let mut text_passport_frame_reports = Vec::new();

    for frame in &case.frames_to_compare {
        let time = *frame as f64 / recipe.scene.composition.fps;
        let native_name = format!("{}_{:05}.png", case.id, frame);
        let ae_name = native_name.clone();
        let diff_name = format!("{}_{:05}_diff.png", case.id, frame);
        let native_path = native_dir.join(&native_name);
        let ae_source_path = pack_root
            .join("ae_goldens/png")
            .join(&case.id)
            .join(&ae_name);
        let ae_path = ae_dir.join(&ae_name);
        let diff_path = diff_dir.join(&diff_name);

        let (native_canvas, render_trace) =
            match render_core::layer_eval::render_frame_with_footage_traced(
                &recipe.scene,
                *frame,
                provider,
            ) {
                Ok(rendered) => rendered,
                Err(err) => {
                    return Ok(json!({
                        "case": case.id,
                        "title": case.title,
                        "ladder": case.ladder,
                        "modules": case.modules,
                        "assets": case.assets,
                        "status": "render_error",
                        "ok": false,
                        "error": format!("{err:#}"),
                        "scene": case_dir.join("scene.json").display().to_string(),
                        "notes": recipe.notes
                    }));
                }
            };
        let effects_debug =
            write_effect_debug_sidecars(out_root, &case.id, *frame, &render_trace.effect_debug)?;
        let trace_sidecars = write_trace_sidecars(&case_dir, &render_trace)?;
        let text_passport_report =
            compare_frame_text_passport(pack_root, case, *frame, &render_trace)?;
        write_warps_fields_debug_sidecars(case, &recipe.scene, *frame, time, &case_dir)?;
        native_canvas.save_png(&native_path)?;
        fs::copy(&ae_source_path, &ae_path)
            .with_context(|| format!("copying AE golden {}", ae_source_path.display()))?;

        let ae_image = testkit::load_rgba_png(&ae_path)?;
        anyhow::ensure!(
            native_canvas.width == ae_image.width && native_canvas.height == ae_image.height,
            "{} frame {} dimensions differ: native={}x{} AE={}x{}",
            case.id,
            frame,
            native_canvas.width,
            native_canvas.height,
            ae_image.width,
            ae_image.height
        );
        let rgba_metrics = testkit::write_diff_png(
            &native_canvas.data,
            &ae_image.data,
            native_canvas.width,
            native_canvas.height,
            &diff_path,
        )?;
        let split_metrics = testkit::SplitDiffMetrics {
            rgba: rgba_metrics,
            rgb: testkit::diff_rgb8(&native_canvas.data, &ae_image.data),
            alpha: testkit::diff_alpha8(&native_canvas.data, &ae_image.data),
        };
        let native_corner = native_canvas.pixel(0, 0);
        let ae_corner = rgba_pixel_at(&ae_image.data, ae_image.width, ae_image.height, 0, 0);
        let background_corner = background_corner_diagnostics(native_corner, ae_corner);
        let background_alpha_normalized_metrics = testkit::diff_rgba8_background_alpha_normalized(
            &native_canvas.data,
            &ae_image.data,
            rgb(native_corner),
            rgb(ae_corner),
        );
        let foreground_rgb_metrics = testkit::diff_rgb8_foreground_masked(
            &native_canvas.data,
            &ae_image.data,
            rgb(native_corner),
            rgb(ae_corner),
        );
        let rgb_under_alpha_policy_metrics = testkit::diff_rgb8_under_alpha_policy(
            &native_canvas.data,
            &ae_image.data,
            rgb(ae_corner),
        );
        let rgb_over_native_background_metrics = testkit::diff_rgb8_over_background(
            &native_canvas.data,
            &ae_image.data,
            rgb(native_corner),
        );
        let rgb_over_ae_background_metrics =
            testkit::diff_rgb8_over_background(&native_canvas.data, &ae_image.data, rgb(ae_corner));
        if background_corner.rgb_matches_alpha_differs {
            background_corner_alpha_diff_frames.push(*frame);
        }

        rgba_summary.push(split_metrics.rgba);
        rgb_summary.push(split_metrics.rgb);
        alpha_summary.push(split_metrics.alpha);
        background_alpha_normalized_summary.push(background_alpha_normalized_metrics);
        foreground_rgb_summary.push(foreground_rgb_metrics);
        rgb_under_alpha_policy_summary.push(rgb_under_alpha_policy_metrics);
        rgb_over_native_background_summary.push(rgb_over_native_background_metrics);
        rgb_over_ae_background_summary.push(rgb_over_ae_background_metrics);
        frame_reports.push(json!({
            "frame": frame,
            "time": time,
            "native": native_path.display().to_string(),
            "ae": ae_path.display().to_string(),
            "diff": diff_path.display().to_string(),
            "width": native_canvas.width,
            "height": native_canvas.height,
            "max_abs_diff": split_metrics.rgba.max_abs_diff,
            "mean_abs_diff": split_metrics.rgba.mean_abs_diff,
            "rmse_abs_diff": split_metrics.rgba.rmse_abs_diff,
            "changed_pixels": split_metrics.rgba.changed_pixels,
            "total_pixels": split_metrics.rgba.total_pixels,
            "metrics": {
                "rgba": metric_json(split_metrics.rgba),
                "rgb": metric_json(split_metrics.rgb),
                "alpha": metric_json(split_metrics.alpha),
                "background_alpha_normalized": metric_json(background_alpha_normalized_metrics),
                "foreground_rgb": metric_json(foreground_rgb_metrics),
                "rgb_under_alpha_policy": metric_json(rgb_under_alpha_policy_metrics),
                "rgb_over_native_background": metric_json(rgb_over_native_background_metrics),
                "rgb_over_ae_background": metric_json(rgb_over_ae_background_metrics)
            },
            "background_corner": background_corner.to_json(),
            "effects_debug": effects_debug,
            "trace_sidecars": trace_sidecars,
            "text_passport": text_passport_frame_ref(&text_passport_report)
        }));
        text_passport_frame_reports.push(text_passport_report);
    }

    let rgba_metrics = rgba_summary.metrics();
    let rgb_metrics = rgb_summary.metrics();
    let alpha_metrics = alpha_summary.metrics();
    let background_alpha_normalized_metrics = background_alpha_normalized_summary.metrics();
    let foreground_rgb_metrics = foreground_rgb_summary.metrics();
    let rgb_under_alpha_policy_metrics = rgb_under_alpha_policy_summary.metrics();
    let rgb_over_native_background_metrics = rgb_over_native_background_summary.metrics();
    let rgb_over_ae_background_metrics = rgb_over_ae_background_summary.metrics();
    let background_corner_summary =
        background_corner_summary_json(&background_corner_alpha_diff_frames);
    let text_passport_summary = text_passport_summary_json(&text_passport_frame_reports);
    let text_passport_report_path = case_dir.join("text_passport_comparison.json");
    let text_passport_report = json!({
        "schema": "ae-native-renderer.text-passport-comparison.v1",
        "case": case.id,
        "reference_root": pack_root.join("ae_goldens/text_telemetry").display().to_string(),
        "summary": text_passport_summary,
        "frames": text_passport_frame_reports
    });
    fs::write(
        &text_passport_report_path,
        serde_json::to_string_pretty(&text_passport_report)?,
    )?;
    let threshold_ok = threshold_mean.map_or(true, |limit| rgba_metrics.mean_abs_diff <= limit)
        && threshold_max.map_or(true, |limit| rgba_metrics.max_abs_diff <= limit);
    let status = if threshold_mean.is_some() || threshold_max.is_some() {
        if threshold_ok {
            "pass"
        } else {
            "fail"
        }
    } else {
        "measured"
    };
    let metrics = json!({
        "case": case.id,
        "status": status,
        "ok": threshold_ok,
        "metric_contract": testkit::RGB_ALPHA_METRIC_POLICY,
        "thresholds": {
            "mean_abs_diff": threshold_mean,
            "max_abs_diff": threshold_max
        },
        "summary": {
            "frames": case.frames_to_compare.len(),
            "max_abs_diff": rgba_metrics.max_abs_diff,
            "mean_abs_diff": rgba_metrics.mean_abs_diff,
            "rmse_abs_diff": rgba_metrics.rmse_abs_diff,
            "changed_pixels": rgba_metrics.changed_pixels,
            "total_pixels": rgba_metrics.total_pixels,
            "changed_pixel_ratio": changed_pixel_ratio(rgba_metrics),
            "metrics": {
                "rgba": metric_json(rgba_metrics),
                "rgb": metric_json(rgb_metrics),
                "alpha": metric_json(alpha_metrics),
                "background_alpha_normalized": metric_json(background_alpha_normalized_metrics),
                "foreground_rgb": metric_json(foreground_rgb_metrics),
                "rgb_under_alpha_policy": metric_json(rgb_under_alpha_policy_metrics),
                "rgb_over_native_background": metric_json(rgb_over_native_background_metrics),
                "rgb_over_ae_background": metric_json(rgb_over_ae_background_metrics)
            },
            "background_corner": background_corner_summary,
            "text_passport": text_passport_report["summary"].clone(),
            "elapsed_ms": elapsed_ms(case_started)
        },
        "frames": frame_reports
    });
    fs::write(
        case_dir.join("metrics.json"),
        serde_json::to_string_pretty(&metrics)?,
    )?;
    Ok(json!({
        "case": case.id,
        "title": case.title,
        "ladder": case.ladder,
        "modules": case.modules,
        "assets": case.assets,
        "status": status,
        "ok": threshold_ok,
        "scene": case_dir.join("scene.json").display().to_string(),
        "metrics": case_dir.join("metrics.json").display().to_string(),
        "native_dir": native_dir.display().to_string(),
        "ae_dir": ae_dir.display().to_string(),
        "diff_dir": diff_dir.display().to_string(),
        "text_passport": text_passport_report_path.display().to_string(),
        "summary": metrics["summary"].clone(),
        "notes": recipe.notes
    }))
}

#[derive(Debug, Clone, Copy)]
struct MetricsAccumulator {
    components_per_pixel: u64,
    max_abs_diff: u8,
    changed_pixels: u64,
    total_pixels: u64,
    sum_abs: f64,
    sum_sq: f64,
}

impl MetricsAccumulator {
    fn new(components_per_pixel: u64) -> Self {
        Self {
            components_per_pixel,
            max_abs_diff: 0,
            changed_pixels: 0,
            total_pixels: 0,
            sum_abs: 0.0,
            sum_sq: 0.0,
        }
    }

    fn push(&mut self, metrics: testkit::DiffMetrics) {
        let components = (metrics.total_pixels * self.components_per_pixel).max(1) as f64;
        self.max_abs_diff = self.max_abs_diff.max(metrics.max_abs_diff);
        self.changed_pixels += metrics.changed_pixels;
        self.total_pixels += metrics.total_pixels;
        self.sum_abs += metrics.mean_abs_diff * components;
        self.sum_sq += metrics.rmse_abs_diff * metrics.rmse_abs_diff * components;
    }

    fn metrics(&self) -> testkit::DiffMetrics {
        let components = (self.total_pixels * self.components_per_pixel).max(1) as f64;
        testkit::DiffMetrics {
            max_abs_diff: self.max_abs_diff,
            mean_abs_diff: self.sum_abs / components,
            rmse_abs_diff: (self.sum_sq / components).sqrt(),
            changed_pixels: self.changed_pixels,
            total_pixels: self.total_pixels,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BackgroundCornerDiagnostics {
    native_rgba: [u8; 4],
    ae_rgba: [u8; 4],
    rgb_matches: bool,
    alpha_differs: bool,
    rgb_matches_alpha_differs: bool,
}

impl BackgroundCornerDiagnostics {
    fn to_json(self) -> Value {
        json!({
            "native_rgba": self.native_rgba,
            "ae_rgba": self.ae_rgba,
            "rgb_matches": self.rgb_matches,
            "alpha_differs": self.alpha_differs,
            "rgb_matches_alpha_differs": self.rgb_matches_alpha_differs,
            "description": if self.rgb_matches_alpha_differs {
                json!("Native and AE background-corner RGB match, but alpha differs; raw RGBA is dominated by background alpha.")
            } else {
                Value::Null
            }
        })
    }
}

fn metric_json(metrics: testkit::DiffMetrics) -> Value {
    json!({
        "max_abs_diff": metrics.max_abs_diff,
        "mean_abs_diff": metrics.mean_abs_diff,
        "rmse_abs_diff": metrics.rmse_abs_diff,
        "changed_pixels": metrics.changed_pixels,
        "total_pixels": metrics.total_pixels,
        "changed_pixel_ratio": changed_pixel_ratio(metrics)
    })
}

fn background_corner_summary_json(rgb_matches_alpha_differs_frames: &[u32]) -> Value {
    json!({
        "rgb_matches_alpha_differs_count": rgb_matches_alpha_differs_frames.len(),
        "rgb_matches_alpha_differs_frames": rgb_matches_alpha_differs_frames,
        "description": if rgb_matches_alpha_differs_frames.is_empty() {
            Value::Null
        } else {
            json!("At least one frame has matching native/AE background-corner RGB with differing alpha; use rgb or background_alpha_normalized metrics before formula tuning.")
        }
    })
}

#[derive(Debug, Clone, Default)]
struct TextPassportSnapshot {
    layouts: BTreeMap<String, Vec<Value>>,
    selectors: BTreeMap<String, Vec<Value>>,
}

#[derive(Debug, Clone, Default)]
struct TextPassportCompareStats {
    fields_compared: u64,
    mismatches: u64,
    max_abs_delta: f64,
    first_mismatch: Option<Value>,
}

impl TextPassportCompareStats {
    fn is_ok(&self) -> bool {
        self.mismatches == 0
    }

    fn push_match(&mut self) {
        self.fields_compared += 1;
    }

    fn push_mismatch(&mut self, path: &str, expected: &Value, actual: Option<&Value>) {
        self.fields_compared += 1;
        self.mismatches += 1;
        if self.first_mismatch.is_none() {
            self.first_mismatch = Some(json!({
                "path": path,
                "expected": expected,
                "actual": actual.cloned().unwrap_or(Value::Null)
            }));
        }
    }

    fn push_numeric_mismatch(
        &mut self,
        path: &str,
        expected: &Value,
        actual: &Value,
        delta: f64,
        tolerance: f64,
    ) {
        self.fields_compared += 1;
        self.mismatches += 1;
        self.max_abs_delta = self.max_abs_delta.max(delta.abs());
        if self.first_mismatch.is_none() {
            self.first_mismatch = Some(json!({
                "path": path,
                "expected": expected,
                "actual": actual,
                "abs_delta": delta.abs(),
                "tolerance": tolerance
            }));
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "ok": self.is_ok(),
            "fields_compared": self.fields_compared,
            "mismatches": self.mismatches,
            "max_abs_delta": self.max_abs_delta,
            "first_mismatch": self.first_mismatch
        })
    }
}

fn compare_frame_text_passport(
    pack_root: &Path,
    case: &PackCase,
    frame: u32,
    trace: &render_core::layer_eval::FrameRenderTrace,
) -> Result<Value> {
    let reference_path = text_passport_reference_path(pack_root, &case.id, frame);
    let native = text_passport_snapshot_from_trace(trace);
    let native_summary = text_passport_snapshot_summary(&native);
    if !reference_path.exists() {
        return Ok(json!({
            "frame": frame,
            "status": "missing_reference",
            "ok": Value::Null,
            "reference": reference_path.display().to_string(),
            "native": native_summary,
            "contract": text_passport_contract_json()
        }));
    }

    let reference_records = read_jsonl_values(&reference_path)?;
    let reference = text_passport_snapshot_from_jsonl_records(&reference_records);
    let reference_summary = text_passport_snapshot_summary(&reference);
    let stats = compare_text_passport_snapshots(&reference, &native);
    let status = if reference.layouts.is_empty() && reference.selectors.is_empty() {
        "empty_reference"
    } else {
        "compared"
    };
    Ok(json!({
        "frame": frame,
        "status": status,
        "ok": stats.is_ok(),
        "reference": reference_path.display().to_string(),
        "native": native_summary,
        "reference_summary": reference_summary,
        "comparison": stats.to_json(),
        "contract": text_passport_contract_json()
    }))
}

fn text_passport_reference_path(pack_root: &Path, case_id: &str, frame: u32) -> PathBuf {
    pack_root
        .join("ae_goldens/text_telemetry")
        .join(case_id)
        .join(format!("{case_id}_{frame:05}.jsonl"))
}

fn read_jsonl_values(path: &Path) -> Result<Vec<Value>> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line =
            line.with_context(|| format!("reading {} line {}", path.display(), index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        values.push(
            serde_json::from_str(&line)
                .with_context(|| format!("parsing {} line {}", path.display(), index + 1))?,
        );
    }
    Ok(values)
}

fn text_passport_snapshot_from_trace(
    trace: &render_core::layer_eval::FrameRenderTrace,
) -> TextPassportSnapshot {
    let mut snapshot = TextPassportSnapshot::default();
    for record in &trace.text_layouts {
        push_text_layout_record(&mut snapshot, record);
    }
    for record in &trace.text_selector_weights {
        push_text_selector_record(&mut snapshot, record);
    }
    snapshot
}

fn text_passport_snapshot_from_jsonl_records(records: &[Value]) -> TextPassportSnapshot {
    let mut snapshot = TextPassportSnapshot::default();
    for line in records {
        let event = line.get("event").and_then(Value::as_str);
        let record = line.get("record").unwrap_or(line);
        match event {
            Some("text.layout") => push_text_layout_record(&mut snapshot, record),
            Some("text.selector_weights") => push_text_selector_record(&mut snapshot, record),
            _ => {
                if record.get("layout").is_some() {
                    push_text_layout_record(&mut snapshot, record);
                }
                if record.get("units").is_some() {
                    push_text_selector_record(&mut snapshot, record);
                }
            }
        }
    }
    snapshot
}

fn push_text_layout_record(snapshot: &mut TextPassportSnapshot, record: &Value) {
    let key = text_layout_key(record);
    let mut projected = serde_json::Map::new();
    copy_json_field(&mut projected, record, "composition");
    copy_json_field(&mut projected, record, "layer_id");
    copy_json_field(&mut projected, record, "render_path");
    let glyphs = record
        .pointer("/layout/glyphs")
        .and_then(Value::as_array)
        .map(|glyphs| {
            glyphs
                .iter()
                .map(project_text_glyph_row)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    projected.insert("glyphs".to_string(), Value::Array(glyphs));
    if let Some(line_boxes) = record.pointer("/layout/line_boxes") {
        projected.insert("line_boxes".to_string(), line_boxes.clone());
    }
    snapshot
        .layouts
        .entry(key)
        .or_default()
        .push(Value::Object(projected));
}

fn push_text_selector_record(snapshot: &mut TextPassportSnapshot, record: &Value) {
    let key = text_selector_key(record);
    let mut projected = serde_json::Map::new();
    copy_json_field(&mut projected, record, "composition");
    copy_json_field(&mut projected, record, "layer_id");
    copy_json_field(&mut projected, record, "animator");
    copy_json_field(&mut projected, record, "selector");
    copy_json_field(&mut projected, record, "expression_selector");
    let units = record
        .get("units")
        .and_then(Value::as_array)
        .map(|units| {
            units
                .iter()
                .map(project_text_selector_unit)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    projected.insert("units".to_string(), Value::Array(units));
    snapshot
        .selectors
        .entry(key)
        .or_default()
        .push(Value::Object(projected));
}

fn project_text_glyph_row(glyph: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for field in [
        "character",
        "font_glyph_id",
        "glyph_run_index",
        "char_index",
        "word_index",
        "line_index",
        "advance",
        "advance_x",
        "advance_y",
        "bbox",
        "cooltype_bbox_minmax",
        "bbox_center",
        "baseline",
        "baseline_delta",
        "metric_source",
        "cooltype_reference_status",
        "font_postscript_name",
    ] {
        copy_json_field(&mut out, glyph, field);
    }
    Value::Object(out)
}

fn project_text_selector_unit(unit: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for field in [
        "index",
        "selector_index",
        "selector_position_percent",
        "range_weight",
        "expression_weight",
        "final_weight",
        "expression",
        "glyph_passport",
        "animator_contribution",
    ] {
        copy_json_field(&mut out, unit, field);
    }
    Value::Object(out)
}

fn copy_json_field(out: &mut serde_json::Map<String, Value>, src: &Value, field: &str) {
    if let Some(value) = src.get(field) {
        out.insert(field.to_string(), value.clone());
    }
}

fn text_layout_key(record: &Value) -> String {
    format!(
        "{}::{}",
        record
            .get("composition")
            .and_then(Value::as_str)
            .unwrap_or("<composition>"),
        record
            .get("layer_id")
            .and_then(Value::as_str)
            .unwrap_or("<layer>")
    )
}

fn text_selector_key(record: &Value) -> String {
    format!(
        "{}::{}::{}",
        record
            .get("composition")
            .and_then(Value::as_str)
            .unwrap_or("<composition>"),
        record
            .get("layer_id")
            .and_then(Value::as_str)
            .unwrap_or("<layer>"),
        record
            .get("animator")
            .and_then(Value::as_str)
            .unwrap_or("<animator>")
    )
}

fn compare_text_passport_snapshots(
    expected: &TextPassportSnapshot,
    actual: &TextPassportSnapshot,
) -> TextPassportCompareStats {
    let mut stats = TextPassportCompareStats::default();
    compare_text_passport_record_map("layouts", &expected.layouts, &actual.layouts, &mut stats);
    compare_text_passport_record_map(
        "selectors",
        &expected.selectors,
        &actual.selectors,
        &mut stats,
    );
    stats
}

fn compare_text_passport_record_map(
    root: &str,
    expected: &BTreeMap<String, Vec<Value>>,
    actual: &BTreeMap<String, Vec<Value>>,
    stats: &mut TextPassportCompareStats,
) {
    for (key, expected_records) in expected {
        let path = format!("{root}.{key}");
        let Some(actual_records) = actual.get(key) else {
            stats.push_mismatch(&path, &json!({ "records": expected_records.len() }), None);
            continue;
        };
        compare_json_subset(
            &Value::Array(expected_records.clone()),
            &Value::Array(actual_records.clone()),
            &path,
            stats,
        );
    }
}

fn compare_json_subset(
    expected: &Value,
    actual: &Value,
    path: &str,
    stats: &mut TextPassportCompareStats,
) {
    match (expected, actual) {
        (Value::Object(expected_object), Value::Object(actual_object)) => {
            for (key, expected_value) in expected_object {
                let next_path = format!("{path}.{key}");
                let Some(actual_value) = actual_object.get(key) else {
                    stats.push_mismatch(&next_path, expected_value, None);
                    continue;
                };
                compare_json_subset(expected_value, actual_value, &next_path, stats);
            }
        }
        (Value::Array(expected_array), Value::Array(actual_array)) => {
            if expected_array.len() != actual_array.len() {
                stats.push_mismatch(
                    &format!("{path}.len"),
                    &json!(expected_array.len()),
                    Some(&json!(actual_array.len())),
                );
            }
            for (index, (expected_value, actual_value)) in
                expected_array.iter().zip(actual_array.iter()).enumerate()
            {
                compare_json_subset(
                    expected_value,
                    actual_value,
                    &format!("{path}[{index}]"),
                    stats,
                );
            }
        }
        (Value::Number(expected_number), Value::Number(actual_number)) => {
            compare_json_numbers(
                expected_number,
                actual_number,
                expected,
                actual,
                path,
                stats,
            );
        }
        _ if expected == actual => stats.push_match(),
        _ => stats.push_mismatch(path, expected, Some(actual)),
    }
}

fn compare_json_numbers(
    expected_number: &serde_json::Number,
    actual_number: &serde_json::Number,
    expected: &Value,
    actual: &Value,
    path: &str,
    stats: &mut TextPassportCompareStats,
) {
    if expected_number.is_i64()
        || expected_number.is_u64()
        || actual_number.is_i64()
        || actual_number.is_u64()
    {
        if expected == actual {
            stats.push_match();
        } else {
            stats.push_mismatch(path, expected, Some(actual));
        }
        return;
    }

    let Some(expected_value) = expected_number.as_f64() else {
        stats.push_mismatch(path, expected, Some(actual));
        return;
    };
    let Some(actual_value) = actual_number.as_f64() else {
        stats.push_mismatch(path, expected, Some(actual));
        return;
    };
    let delta = (expected_value - actual_value).abs();
    let tolerance = text_passport_numeric_tolerance(path);
    if delta <= tolerance {
        stats.push_match();
        stats.max_abs_delta = stats.max_abs_delta.max(delta);
    } else {
        stats.push_numeric_mismatch(path, expected, actual, delta, tolerance);
    }
}

fn text_passport_numeric_tolerance(path: &str) -> f64 {
    if path.contains("weight")
        || path.contains("selector_position_percent")
        || path.contains("opacity_alpha_scale")
    {
        0.0001
    } else if path.contains("matrix")
        || path.contains("bbox")
        || path.contains("advance")
        || path.contains("baseline")
        || path.contains("center")
        || path.contains("position")
        || path.contains("scale")
        || path.contains("rotation")
        || path.contains("blur_radius")
    {
        0.001
    } else {
        0.0
    }
}

fn text_passport_snapshot_summary(snapshot: &TextPassportSnapshot) -> Value {
    let layout_records = snapshot.layouts.values().map(Vec::len).sum::<usize>();
    let selector_records = snapshot.selectors.values().map(Vec::len).sum::<usize>();
    let glyph_rows = snapshot
        .layouts
        .values()
        .flatten()
        .map(|record| {
            record
                .get("glyphs")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        })
        .sum::<usize>();
    let selector_units = snapshot
        .selectors
        .values()
        .flatten()
        .map(|record| {
            record
                .get("units")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        })
        .sum::<usize>();
    json!({
        "layout_layers": snapshot.layouts.len(),
        "layout_records": layout_records,
        "glyph_rows": glyph_rows,
        "selector_animators": snapshot.selectors.len(),
        "selector_records": selector_records,
        "selector_units": selector_units
    })
}

fn text_passport_summary_json(frame_reports: &[Value]) -> Value {
    let mut status_counts = BTreeMap::<String, u64>::new();
    let mut compared_frames = 0_u64;
    let mut missing_reference_frames = Vec::new();
    let mut empty_reference_frames = Vec::new();
    let mut mismatched_frames = Vec::new();
    let mut total_mismatches = 0_u64;
    let mut max_abs_delta = 0.0_f64;
    let mut first_mismatch = None;

    for report in frame_reports {
        let frame = report.get("frame").and_then(Value::as_u64).unwrap_or(0);
        let status = report
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        *status_counts.entry(status.to_string()).or_default() += 1;
        match status {
            "compared" => compared_frames += 1,
            "missing_reference" => missing_reference_frames.push(frame),
            "empty_reference" => empty_reference_frames.push(frame),
            _ => {}
        }
        if let Some(comparison) = report.get("comparison") {
            let mismatches = comparison
                .get("mismatches")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            total_mismatches += mismatches;
            if mismatches > 0 {
                mismatched_frames.push(frame);
            }
            max_abs_delta = max_abs_delta.max(
                comparison
                    .get("max_abs_delta")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
            );
            if first_mismatch.is_none() {
                first_mismatch = comparison
                    .get("first_mismatch")
                    .filter(|value| !value.is_null())
                    .cloned();
            }
        }
    }

    let all_present_ok = compared_frames > 0
        && total_mismatches == 0
        && missing_reference_frames.is_empty()
        && empty_reference_frames.is_empty();
    json!({
        "ok": all_present_ok,
        "frames": frame_reports.len(),
        "compared_frames": compared_frames,
        "status_counts": status_counts,
        "missing_reference_frames": missing_reference_frames,
        "empty_reference_frames": empty_reference_frames,
        "mismatched_frames": mismatched_frames,
        "total_mismatches": total_mismatches,
        "max_abs_delta": max_abs_delta,
        "first_mismatch": first_mismatch
    })
}

fn text_passport_frame_ref(report: &Value) -> Value {
    json!({
        "status": report.get("status").cloned().unwrap_or(Value::Null),
        "ok": report.get("ok").cloned().unwrap_or(Value::Null),
        "reference": report.get("reference").cloned().unwrap_or(Value::Null),
        "native": report.get("native").cloned().unwrap_or(Value::Null),
        "comparison": report.get("comparison").cloned().unwrap_or(Value::Null)
    })
}

fn text_passport_contract_json() -> Value {
    json!({
        "schema": "ae-native-renderer.text-passport-reference.v1",
        "reference_path": "ae_goldens/text_telemetry/<case_id>/<case_id>_<frame>.jsonl",
        "reference_format": "JSONL records using text.layout/text.selector_weights events; reference records may contain only the fields that should be compared",
        "comparison": "reference-as-subset; extra native fields are ignored",
        "numeric_tolerances": {
            "weights": 0.0001,
            "geometry": 0.001,
            "default": 0.0
        }
    })
}

fn write_warps_fields_debug_sidecars(
    case: &PackCase,
    scene: &Scene,
    frame: u32,
    time: f64,
    case_dir: &Path,
) -> Result<()> {
    if !matches!(case.id.as_str(), "EFF_040" | "EFF_060") {
        return Ok(());
    }

    let input = Canvas::transparent(scene.composition.width, scene.composition.height);
    let frame_dir = case_dir
        .join("effects_debug")
        .join(format!("frame_{frame:05}"));

    for layer in &scene.layers {
        let (layer_id, effects) = sidecar_layer_effects(layer);
        for (effect_index, spec) in effects.iter().enumerate() {
            let Some(sidecar) = warps_fields_debug_json(
                case,
                scene,
                frame,
                time,
                layer_id,
                effect_index,
                spec,
                &input,
            ) else {
                continue;
            };
            fs::create_dir_all(&frame_dir)?;
            let effect_name = sidecar_effect_name(&spec.match_name);
            fs::write(
                frame_dir.join(format!("{layer_id}_{effect_index:02}_{effect_name}.json")),
                serde_json::to_string_pretty(&sidecar)?,
            )?;
        }
    }

    Ok(())
}

fn reset_trace_sidecars(case_dir: &Path) -> Result<()> {
    for name in [
        "adjustment_effects.jsonl",
        "temporal_telemetry.jsonl",
        "text_telemetry.jsonl",
        "expression_telemetry.jsonl",
        "collapse_telemetry.jsonl",
    ] {
        let path = case_dir.join(name);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("removing stale trace sidecar {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_trace_sidecars(
    case_dir: &Path,
    trace: &render_core::layer_eval::FrameRenderTrace,
) -> Result<Value> {
    let adjustment_count = write_adjustment_trace_sidecar(case_dir, trace)?;
    let temporal_count = write_temporal_trace_sidecar(case_dir, trace)?;
    let text_layout_count = write_value_trace_sidecar(
        case_dir,
        "text_telemetry.jsonl",
        "text.layout",
        trace,
        &trace.text_layouts,
    )?;
    let text_selector_count = write_value_trace_sidecar(
        case_dir,
        "text_telemetry.jsonl",
        "text.selector_weights",
        trace,
        &trace.text_selector_weights,
    )?;
    let expression_count = write_value_trace_sidecar(
        case_dir,
        "expression_telemetry.jsonl",
        "expression.position",
        trace,
        &trace.position_expressions,
    )?;
    let collapse_count = write_value_trace_sidecar(
        case_dir,
        "collapse_telemetry.jsonl",
        "collapse",
        trace,
        &trace.collapse,
    )?;

    Ok(json!({
        "adjustment_effects": trace_sidecar_ref(case_dir, "adjustment_effects.jsonl", adjustment_count),
        "temporal_telemetry": trace_sidecar_ref(case_dir, "temporal_telemetry.jsonl", temporal_count),
        "text_telemetry": trace_sidecar_ref(case_dir, "text_telemetry.jsonl", text_layout_count + text_selector_count),
        "expression_telemetry": trace_sidecar_ref(case_dir, "expression_telemetry.jsonl", expression_count),
        "collapse_telemetry": trace_sidecar_ref(case_dir, "collapse_telemetry.jsonl", collapse_count)
    }))
}

fn trace_sidecar_ref(case_dir: &Path, file_name: &str, records: usize) -> Value {
    json!({
        "path": case_dir.join(file_name).display().to_string(),
        "records_written_for_frame": records
    })
}

fn write_adjustment_trace_sidecar(
    case_dir: &Path,
    trace: &render_core::layer_eval::FrameRenderTrace,
) -> Result<usize> {
    let mut count = 0;
    for record in &trace.adjustment_effects {
        append_trace_json_line(
            &case_dir.join("adjustment_effects.jsonl"),
            &json!({
                "frame": trace.frame,
                "time": trace.time,
                "composition": record.composition,
                "layer_id": record.layer_id,
                "effect_index": record.effect_index,
                "match_name": record.match_name,
                "comp_time": record.comp_time,
                "layer_time": record.layer_time,
                "lower_stack_time": record.lower_stack_time,
                "bucket_time": record.posterize.map(|posterize| posterize.bucket_time).unwrap_or(record.lower_stack_time),
                "param_time": record.param_time,
                "input_hash": record.input_hash,
                "output_hash": record.output_hash,
                "posterize": record.posterize.map(|posterize| json!({
                    "frame_rate": posterize.frame_rate,
                    "bucket": posterize.bucket,
                    "bucket_time": posterize.bucket_time
                }))
            }),
        )?;
        count += 1;
    }
    Ok(count)
}

fn write_temporal_trace_sidecar(
    case_dir: &Path,
    trace: &render_core::layer_eval::FrameRenderTrace,
) -> Result<usize> {
    let mut count = 0;
    for record in &trace.temporal {
        append_trace_json_line(
            &case_dir.join("temporal_telemetry.jsonl"),
            &json!({
                "frame": trace.frame,
                "time": trace.time,
                "record": temporal_trace_record_json(record)
            }),
        )?;
        count += 1;
    }
    for record in &trace.motion_blur {
        append_trace_json_line(
            &case_dir.join("temporal_telemetry.jsonl"),
            &json!({
                "frame": trace.frame,
                "time": trace.time,
                "record": motion_blur_trace_json(record)
            }),
        )?;
        count += 1;
    }
    Ok(count)
}

fn temporal_trace_record_json(record: &render_core::layer_eval::TemporalTraceRecord) -> Value {
    json!({
        "event": record.event,
        "composition": record.composition,
        "layer_id": record.layer_id,
        "layer_type": record.layer_type,
        "comp_time": record.comp_time,
        "layer_start": record.layer_start,
        "layer_time": record.layer_time,
        "posterized_time": record.posterized_time,
        "source_id": record.source_id,
        "source_start": record.source_start,
        "source_time": record.source_time,
        "source_frame_id": record.source_frame_id,
        "adjustment_lower_stack_time": record.adjustment_lower_stack_time,
        "posterize": record.posterize.map(|posterize| json!({
            "frame_rate": posterize.frame_rate,
            "bucket": posterize.bucket,
            "bucket_time": posterize.bucket_time
        }))
    })
}

fn motion_blur_trace_json(record: &render_core::layer_eval::MotionBlurTrace) -> Value {
    json!({
        "event": "temporal.motion_blur",
        "composition": record.composition,
        "layer_id": record.layer_id,
        "comp_time": record.comp_time,
        "frame_duration": record.frame_duration,
        "shutter_open": record.shutter_open,
        "shutter_close": record.shutter_close,
        "shutter_angle": record.shutter_angle,
        "shutter_phase": record.shutter_phase,
        "requested_samples": record.requested_samples,
        "effective_samples": record.effective_samples,
        "divisor": record.divisor,
        "samples": record.samples.iter().map(|sample| json!({
            "sample_index": sample.sample_index,
            "sample_time": sample.sample_time,
            "layer_time": sample.layer_time,
            "posterized_time": sample.posterized_time,
            "source_time": sample.source_time,
            "source_frame_id": sample.source_frame_id,
            "active": sample.active,
            "opacity": sample.opacity,
            "weight": sample.weight
        })).collect::<Vec<_>>()
    })
}

fn write_value_trace_sidecar(
    case_dir: &Path,
    file_name: &str,
    event: &str,
    trace: &render_core::layer_eval::FrameRenderTrace,
    records: &[Value],
) -> Result<usize> {
    for record in records {
        append_trace_json_line(
            &case_dir.join(file_name),
            &json!({
                "event": event,
                "frame": trace.frame,
                "time": trace.time,
                "record": record
            }),
        )?;
    }
    Ok(records.len())
}

fn append_trace_json_line(path: &Path, value: &Value) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening trace sidecar {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

fn warps_fields_debug_json(
    case: &PackCase,
    scene: &Scene,
    frame: u32,
    time: f64,
    layer_id: &str,
    effect_index: usize,
    spec: &EffectSpec,
    input: &Canvas,
) -> Option<Value> {
    match (case.id.as_str(), spec.match_name.as_str()) {
        ("EFF_040", "ADBE Geometry2") => {
            let debug = effects::geometry::geometry2_debug_data(input, &spec.params, time);
            Some(json!({
                "schema": "ae-native-renderer.geometry2-debug.v1",
                "case": case.id,
                "frame": frame,
                "time": time,
                "composition": {
                    "id": scene.composition.id,
                    "width": scene.composition.width,
                    "height": scene.composition.height,
                    "fps": scene.composition.fps
                },
                "layer_id": layer_id,
                "effect_index": effect_index,
                "match_name": spec.match_name,
                "input_size": [input.width, input.height],
                "raw_params": debug.raw_params,
                "resolved": {
                    "anchor": debug.resolved.anchor,
                    "position": debug.resolved.position,
                    "scale": debug.resolved.scale,
                    "rotation": debug.resolved.rotation,
                    "skew": debug.resolved.skew,
                    "skew_axis": debug.resolved.skew_axis,
                    "pixel_aspect": debug.resolved.pixel_aspect
                },
                "forward_matrix": debug.forward_matrix,
                "inverse_matrix": debug.inverse_matrix,
                "samples": debug.samples.iter().map(|sample| json!({
                    "output_xy": sample.output_xy,
                    "source_uv": sample.source_uv,
                    "sample_xy": sample.sample_xy,
                    "sample_rgba": sample.sample_rgba,
                    "out_of_bounds": sample.out_of_bounds
                })).collect::<Vec<_>>(),
                "sampler_mode": debug.sampler_mode,
                "edge_policy": debug.edge_policy,
                "out_of_bounds_count": debug.out_of_bounds_count
            }))
        }
        ("EFF_060", "ADBE Turbulent Displace") => {
            let debug = effects::turbulent_displace::turbulent_displace_field_telemetry(
                input,
                &spec.params,
                time,
            );
            Some(json!({
                "schema": "ae-native-renderer.turbulent-displace-field.v1",
                "case": case.id,
                "frame": frame,
                "time": time,
                "composition": {
                    "id": scene.composition.id,
                    "width": scene.composition.width,
                    "height": scene.composition.height,
                    "fps": scene.composition.fps
                },
                "layer_id": layer_id,
                "effect_index": effect_index,
                "match_name": spec.match_name,
                "input_size": [input.width, input.height],
                "raw_params": debug.raw_params,
                "resolved": {
                    "displacement_type": debug.resolved.displacement_type,
                    "amount": debug.resolved.amount,
                    "size": debug.resolved.size,
                    "offset": debug.resolved.offset,
                    "complexity": debug.resolved.complexity,
                    "evolution": debug.resolved.evolution,
                    "random_seed": debug.resolved.random_seed,
                    "pinning": debug.resolved.pinning,
                    "resize_layer": debug.resolved.resize_layer,
                    "amplitude": debug.resolved.amplitude,
                    "phase_radians": debug.resolved.phase_radians
                },
                "samples": debug.samples.iter().map(|sample| json!({
                    "output_xy": sample.output_xy,
                    "noise": sample.noise,
                    "displacement": sample.displacement,
                    "source_uv": sample.source_uv,
                    "sample_xy": sample.sample_xy,
                    "out_of_bounds": sample.out_of_bounds
                })).collect::<Vec<_>>(),
                "field_hash": format!("{:016x}", debug.field_hash),
                "field_hash_u64": debug.field_hash,
                "sampler_mode": debug.sampler_mode,
                "edge_policy": debug.edge_policy,
                "out_of_bounds_count": debug.out_of_bounds_count
            }))
        }
        _ => None,
    }
}

fn sidecar_layer_effects(layer: &Layer) -> (&str, &[EffectSpec]) {
    match layer {
        Layer::Solid { id, effects, .. }
        | Layer::Footage { id, effects, .. }
        | Layer::Text { id, effects, .. }
        | Layer::Precomp { id, effects, .. }
        | Layer::Adjustment { id, effects, .. } => (id, effects),
    }
}

fn sidecar_effect_name(match_name: &str) -> &'static str {
    match match_name {
        "ADBE Geometry2" => "geometry2",
        "ADBE Turbulent Displace" => "turbulent_displace",
        _ => "effect",
    }
}

fn write_effect_debug_sidecars(
    out_root: &Path,
    case_id: &str,
    frame: u32,
    records: &[render_core::layer_eval::EffectDebugRecord],
) -> Result<Vec<String>> {
    if records.is_empty() {
        return Ok(Vec::new());
    }

    let frame_dir = out_root
        .join("effects_debug")
        .join(case_id)
        .join(frame.to_string());
    fs::create_dir_all(&frame_dir)?;

    let mut paths = Vec::new();
    for record in records {
        let file_name = format!(
            "{}_{}_{}.json",
            effects_debug_path_segment(&record.layer_id),
            record.effect_index,
            effects_debug_path_segment(&record.match_name)
        );
        let path = frame_dir.join(file_name);
        let payload = json!({
            "case": case_id,
            "frame": frame,
            "composition": &record.composition,
            "layer_id": &record.layer_id,
            "effect_index": record.effect_index,
            "match_name": &record.match_name,
            "application": record.application,
            "effect_time": record.effect_time,
            "trace": &record.trace
        });
        fs::write(&path, serde_json::to_string_pretty(&payload)?)?;
        paths.push(path.display().to_string());
    }

    Ok(paths)
}

fn effects_debug_path_segment(value: &str) -> String {
    let mut sanitized = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "unnamed".to_string()
    } else {
        sanitized.to_string()
    }
}

fn changed_pixel_ratio(metrics: testkit::DiffMetrics) -> f64 {
    metrics.changed_pixels as f64 / metrics.total_pixels.max(1) as f64
}

fn background_corner_diagnostics(
    native_rgba: [u8; 4],
    ae_rgba: [u8; 4],
) -> BackgroundCornerDiagnostics {
    let rgb_matches = rgb(native_rgba) == rgb(ae_rgba);
    let alpha_differs = native_rgba[3] != ae_rgba[3];
    BackgroundCornerDiagnostics {
        native_rgba,
        ae_rgba,
        rgb_matches,
        alpha_differs,
        rgb_matches_alpha_differs: rgb_matches && alpha_differs,
    }
}

fn rgba_pixel_at(data: &[u8], width: u32, height: u32, x: u32, y: u32) -> [u8; 4] {
    if x >= width || y >= height {
        return [0, 0, 0, 0];
    }
    let idx = ((y * width + x) * 4) as usize;
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

fn rgb(rgba: [u8; 4]) -> [u8; 3] {
    [rgba[0], rgba[1], rgba[2]]
}

impl PackFootageProvider {
    fn new(pack_root: PathBuf, fps: f64, assets: HashMap<String, PrimitiveAsset>) -> Self {
        Self {
            pack_root,
            fps,
            assets,
            still_cache: HashMap::new(),
            sequence_cache: HashMap::new(),
        }
    }

    fn load_still(&mut self, id: &str, asset: &PrimitiveAsset) -> Result<Canvas> {
        if let Some(canvas) = self.still_cache.get(id) {
            return Ok(canvas.clone());
        }
        let path = self.pack_root.join(&asset.path);
        let image =
            testkit::load_rgba_png(&path).with_context(|| format!("loading {}", path.display()))?;
        let canvas = Canvas::from_rgba(image.width, image.height, image.data)?;
        self.still_cache.insert(id.to_string(), canvas.clone());
        Ok(canvas)
    }

    fn load_sequence_frame(
        &mut self,
        id: &str,
        asset: &PrimitiveAsset,
        time: f64,
    ) -> Result<Canvas> {
        let frame_count = asset.frames.unwrap_or(1).max(1);
        let frame = ((time * self.fps) + 1.0e-9)
            .floor()
            .max(0.0)
            .min((frame_count - 1) as f64) as u32;
        let key = (id.to_string(), frame);
        if let Some(canvas) = self.sequence_cache.get(&key) {
            return Ok(canvas.clone());
        }
        let first = Path::new(&asset.path);
        let parent = first.parent().unwrap_or_else(|| Path::new(""));
        let path = self
            .pack_root
            .join(parent)
            .join(format!("frame_{frame:04}.png"));
        let image =
            testkit::load_rgba_png(&path).with_context(|| format!("loading {}", path.display()))?;
        let canvas = Canvas::from_rgba(image.width, image.height, image.data)?;
        self.sequence_cache.insert(key, canvas.clone());
        Ok(canvas)
    }
}

impl render_core::FootageProvider for PackFootageProvider {
    fn frame_at(&mut self, source: &str, time: f64) -> Result<Option<Canvas>> {
        let asset = self
            .assets
            .get(source)
            .cloned()
            .with_context(|| format!("primitive asset {source} was not found"))?;
        if asset.frames.is_some() {
            self.load_sequence_frame(source, &asset, time).map(Some)
        } else {
            self.load_still(source, &asset).map(Some)
        }
    }
}

fn build_recipe(manifest: &PackManifest, pack_root: &Path, case: &PackCase) -> Result<CaseRecipe> {
    let mut b = CaseBuilder {
        manifest,
        pack_root,
        case_id: &case.id,
        layers: Vec::new(),
        compositions: Vec::new(),
        notes: Vec::new(),
    };
    let d = manifest.composition.case_duration_seconds;

    match case.id.as_str() {
        "PRI_010" => {
            b.place("impulse", "impulse_center", 128.0, 128.0, 100.0, 100.0, vec![]);
            b.place("alpha", "alpha_square", 384.0, 128.0, 70.0, 100.0, vec![]);
            b.place("coordinate", "coordinate_field", 128.0, 384.0, 70.0, 100.0, vec![]);
            b.place("color_bars", "color_bars", 384.0, 384.0, 70.0, 100.0, vec![]);
        }
        "INT_010" => {
            let mut linear = placed_transform(96.0, 170.0, 45.0, 100.0);
            linear.animation.position = vec![
                vec2_key(0.0, [96.0, 170.0], false, None),
                vec2_key(d, [416.0, 170.0], false, None),
            ];
            b.place_with_transform("linear", "alpha_square", linear, 0.0, d, 0.0, vec![]);

            let mut hold = placed_transform(96.0, 340.0, 45.0, 100.0);
            hold.animation.position = vec![
                vec2_key(0.0, [96.0, 340.0], true, None),
                vec2_key(d, [416.0, 340.0], false, None),
            ];
            hold.animation.opacity = vec![
                scalar_key(0.0, 100.0, false, None),
                scalar_key(d * 0.5, 35.0, false, None),
                scalar_key(d, 100.0, false, None),
            ];
            b.place_with_transform("hold", "alpha_square", hold, 0.0, d, 0.0, vec![]);
        }
        "INT_020" => {
            let ease = Some(ease_in_out());
            let mut transform = placed_transform(96.0, 256.0, 45.0, 100.0);
            transform.animation.position = vec![
                vec2_key(0.0, [96.0, 256.0], false, ease),
                vec2_key(d, [416.0, 256.0], false, ease),
            ];
            transform.animation.opacity = vec![
                scalar_key(0.0, 0.0, false, ease),
                scalar_key(d * 0.5, 100.0, false, ease),
                scalar_key(d, 0.0, false, ease),
            ];
            b.place_with_transform("ease", "alpha_square", transform, 0.0, d, 0.0, vec![]);
            b.notes.push("Bezier mapping is approximate pending AE tangent parity.".to_string());
        }
        "TMP_010" => {
            b.place_with_transform(
                "numbered_offset",
                "numbered_frames",
                placed_transform(256.0, 256.0, 100.0, 100.0),
                0.25,
                d - 0.25,
                0.75,
                vec![],
            );
        }
        "TMP_020" => {
            b.place(
                "numbered_posterize",
                "numbered_frames",
                256.0,
                256.0,
                100.0,
                100.0,
                vec![effect("ADBE Posterize Time", json!({ "0001": 6 }))],
            );
        }
        "TMP_030" => {
            b.enable_motion_blur();
            let mut transform = placed_transform(80.0, 256.0, 38.0, 100.0);
            transform.motion_blur = true;
            transform.animation.position = vec![
                vec2_key(0.0, [80.0, 256.0], false, None),
                vec2_key(d, [432.0, 256.0], false, None),
            ];
            b.place_with_transform("motion_blur_probe", "alpha_square", transform, 0.0, d, 0.0, vec![]);
        }
        "EFF_010" => b.place(
            "drop_shadow",
            "alpha_square",
            256.0,
            256.0,
            65.0,
            100.0,
            vec![effect(
                "ADBE Drop Shadow",
                json!({ "0001": [0, 0, 0, 1], "0002": 180, "0003": 135, "0004": 28, "0005": 18, "0006": 0 }),
            )],
        ),
        "EFF_020" => b.place(
            "glow",
            "luma_ramp",
            256.0,
            256.0,
            100.0,
            100.0,
            vec![effect("ADBE Glo2", json!({ "0002": 120, "0003": 35, "0004": 1.25 }))],
        ),
        "EFF_030" => b.place(
            "box_blur",
            "impulse_center",
            256.0,
            256.0,
            100.0,
            100.0,
            vec![effect("ADBE Box Blur2", json!({ "0001": 18, "0002": 3 }))],
        ),
        "EFF_040" => b.place(
            "geometry",
            "coordinate_field",
            256.0,
            256.0,
            100.0,
            100.0,
            vec![effect(
                "ADBE Geometry2",
                json!({ "0001": [128, 128], "0002": [256, 256], "0003": 82, "0004": 120, "0008": 72, "rotation": 17 }),
            )],
        ),
        "EFF_050" => b.place(
            "minimax",
            "alpha_square",
            256.0,
            256.0,
            70.0,
            100.0,
            vec![effect("ADBE Minimax", json!({ "0001": 2, "0002": 12, "0003": 1 }))],
        ),
        "EFF_060" => {
            b.place("checker", "checkerboard_16", 256.0, 256.0, 100.0, 100.0, vec![]);
            b.place(
                "field_turbulent",
                "coordinate_field",
                256.0,
                256.0,
                100.0,
                80.0,
                vec![effect(
                    "ADBE Turbulent Displace",
                    json!({ "0002": 45, "0003": 65, "0005": 2, "0006": animated_scalar_param(0.0, 0.0, d, 180.0) }),
                )],
            );
        }
        "EFF_070" => {
            b.place(
                "animated_blur",
                "impulse_center",
                170.0,
                256.0,
                100.0,
                100.0,
                vec![effect(
                    "ADBE Box Blur2",
                    json!({ "0001": animated_scalar_param(0.0, 1.0, d, 28.0), "0002": 2 }),
                )],
            );
            b.place(
                "animated_glow",
                "luma_ramp",
                342.0,
                256.0,
                55.0,
                100.0,
                vec![effect(
                    "ADBE Glo2",
                    json!({ "0002": 160, "0003": animated_scalar_param(0.0, 10.0, d, 55.0), "0004": 0.5 }),
                )],
            );
        }
        "TXT_010" => b.text(
            "word_reveal",
            "WORD REVEAL\nMONTSERRAT TEST",
            font_montserrat(pack_root),
            58.0,
            [256.0, 256.0],
            vec![range_animator("words_reveal", TextSelectorBasedOn::Words, 0.0, 100.0, d)],
        ),
        "TXT_020" => {
            b.text(
                "character_reveal",
                "CHARACTER REVEAL",
                font_montserrat(pack_root),
                48.0,
                [256.0, 190.0],
                vec![range_animator("characters_reveal", TextSelectorBasedOn::Characters, 0.0, 100.0, d)],
            );
            b.text(
                "line_reveal",
                "LINE ONE\nLINE TWO\nLINE THREE",
                font_montserrat(pack_root),
                42.0,
                [256.0, 330.0],
                vec![range_animator("lines_reveal", TextSelectorBasedOn::Lines, 0.0, 100.0, d)],
            );
        }
        "TXT_030" => b.text(
            "glyph_motion",
            "GLYPH MOTION",
            font_point_light(pack_root),
            74.0,
            [256.0, 256.0],
            vec![glyph_animator(d)],
        ),
        "TXT_040" => b.text(
            "bounce_selector",
            "BOUNCE SELECTOR",
            font_point_light(pack_root),
            64.0,
            [256.0, 256.0],
            vec![bounce_animator()],
        ),
        "EXP_010" => {
            let mut transform = placed_transform(256.0, 256.0, 55.0, 100.0);
            transform.animation.expression = Transform2DExpression {
                position: Some(PositionExpression::EdgeWobble {
                    intro: 0.25,
                    outro: 0.25,
                    amp: 34.0,
                    freq: 2.0,
                    source: "ae_conformance_edge_wobble".to_string(),
                }),
            };
            b.place_with_transform("edge_wobble", "alpha_square", transform, 0.0, d, 0.0, vec![]);
        }
        "STK_010" => b.place(
            "shadow_stack",
            "alpha_square",
            256.0,
            256.0,
            60.0,
            100.0,
            vec![
                effect("ADBE Drop Shadow", json!({ "0002": 210, "0003": 135, "0004": 8, "0005": 8 })),
                effect("ADBE Drop Shadow", json!({ "0002": 160, "0003": 45, "0004": 22, "0005": 18 })),
            ],
        ),
        "STK_020" => {
            b.place(
                "left_blur_minimax",
                "hard_edge",
                150.0,
                256.0,
                70.0,
                100.0,
                vec![
                    effect("ADBE Box Blur2", json!({ "0001": 10, "0002": 2 })),
                    effect("ADBE Minimax", json!({ "0001": 2, "0002": 6, "0003": 1 })),
                ],
            );
            b.place(
                "right_minimax_blur",
                "hard_edge",
                362.0,
                256.0,
                70.0,
                100.0,
                vec![
                    effect("ADBE Minimax", json!({ "0001": 2, "0002": 6, "0003": 1 })),
                    effect("ADBE Box Blur2", json!({ "0001": 10, "0002": 2 })),
                ],
            );
        }
        "STK_030" => {
            b.place("numbered_stack", "numbered_frames", 256.0, 256.0, 100.0, 70.0, vec![]);
            b.place("coordinate_stack", "coordinate_field", 256.0, 256.0, 100.0, 100.0, vec![]);
            b.adjustment(vec![
                effect("ADBE Geometry2", json!({ "0003": 96, "0004": 110, "0008": 92 })),
                effect("ADBE Posterize Time", json!({ "0001": 6 })),
                effect("ADBE Minimax", json!({ "0001": 2, "0002": 4, "0003": 1 })),
                effect(
                    "ADBE Turbulent Displace",
                    json!({ "0002": 24, "0003": 72, "0005": 2, "0006": animated_scalar_param(0.0, 0.0, d, 180.0) }),
                ),
            ]);
        }
        "GPH_010" => b.collapse_probe(),
        "CMP_010" => {
            b.place("checker", "checkerboard_16", 256.0, 256.0, 100.0, 100.0, vec![]);
            b.place("premult_probe", "premult_probe", 256.0, 256.0, 100.0, 80.0, vec![]);
            b.place("coordinate_alpha", "coordinate_field", 256.0, 256.0, 55.0, 45.0, vec![]);
        }
        other => anyhow::bail!("no native conformance recipe for case {other}"),
    }

    Ok(b.finish())
}

impl<'a> CaseBuilder<'a> {
    fn enable_motion_blur(&mut self) {
        self.notes.push(
            "Motion blur is rendered by native sampling, pending AE shutter parity.".to_string(),
        );
    }

    fn place(
        &mut self,
        layer_id: &str,
        source: &str,
        x: f32,
        y: f32,
        scale: f32,
        opacity: f32,
        effects: Vec<EffectSpec>,
    ) {
        self.place_with_transform(
            layer_id,
            source,
            placed_transform(x, y, scale, opacity),
            0.0,
            self.manifest.composition.case_duration_seconds,
            0.0,
            effects,
        );
    }

    fn place_with_transform(
        &mut self,
        layer_id: &str,
        source: &str,
        transform: Transform2D,
        start: f64,
        duration: f64,
        source_start: f64,
        effects: Vec<EffectSpec>,
    ) {
        self.layers.push(Layer::Footage {
            id: format!("{}_{}", self.case_id, layer_id),
            start,
            duration,
            source: source.to_string(),
            source_start,
            transform,
            effects,
        });
    }

    fn text(
        &mut self,
        layer_id: &str,
        text: &str,
        font: String,
        font_size: f32,
        position: [f32; 2],
        text_animators: Vec<TextAnimatorSpec>,
    ) {
        let mut transform = Transform2D::default();
        transform.anchor = [
            self.manifest.composition.width as f32 * 0.5,
            self.manifest.composition.height as f32 * 0.5,
        ];
        transform.position = position;
        self.layers.push(Layer::Text {
            id: format!("{}_{}", self.case_id, layer_id),
            start: 0.0,
            duration: self.manifest.composition.case_duration_seconds,
            text: text.to_string(),
            font,
            fontSize: font_size,
            fill: [255, 255, 255, 255],
            box_: Some(Rect {
                x: 0.0,
                y: 0.0,
                w: self.manifest.composition.width as f32,
                h: self.manifest.composition.height as f32,
            }),
            transform,
            text_animators,
            effects: Vec::new(),
        });
    }

    fn adjustment(&mut self, effects: Vec<EffectSpec>) {
        self.layers.push(Layer::Adjustment {
            id: format!("{}_adjustment", self.case_id),
            start: 0.0,
            duration: self.manifest.composition.case_duration_seconds,
            effects,
        });
    }

    fn collapse_probe(&mut self) {
        let child_comp = Composition {
            id: "GPH_010_child_text".to_string(),
            width: self.manifest.composition.width,
            height: self.manifest.composition.height,
            fps: self.manifest.composition.fps,
            duration: self.manifest.composition.case_duration_seconds,
            background: [0, 0, 0, 0],
            motion_blur: MotionBlurSettings::default(),
        };
        let mut child_transform = Transform2D::default();
        child_transform.anchor = [
            self.manifest.composition.width as f32 * 0.5,
            self.manifest.composition.height as f32 * 0.5,
        ];
        child_transform.position = [
            self.manifest.composition.width as f32 * 0.5,
            self.manifest.composition.height as f32 * 0.5,
        ];
        let child_text = Layer::Text {
            id: "GPH_010_child_collapse_text".to_string(),
            start: 0.0,
            duration: self.manifest.composition.case_duration_seconds,
            text: "COLLAPSE".to_string(),
            font: font_montserrat(self.pack_root),
            fontSize: 48.0,
            fill: [255, 255, 255, 255],
            box_: Some(Rect {
                x: 0.0,
                y: 0.0,
                w: self.manifest.composition.width as f32,
                h: self.manifest.composition.height as f32,
            }),
            transform: child_transform,
            text_animators: Vec::new(),
            effects: Vec::new(),
        };
        self.compositions.push(CompositionNode {
            composition: child_comp,
            layers: vec![child_text],
        });

        self.layers.push(Layer::Precomp {
            id: "GPH_010_rasterized_precomp".to_string(),
            start: 0.0,
            duration: self.manifest.composition.case_duration_seconds,
            composition: "GPH_010_child_text".to_string(),
            collapse_transformations: false,
            transform: precomp_transform(150.0, 256.0, 180.0),
            effects: Vec::new(),
        });
        self.layers.push(Layer::Precomp {
            id: "GPH_010_collapsed_precomp".to_string(),
            start: 0.0,
            duration: self.manifest.composition.case_duration_seconds,
            composition: "GPH_010_child_text".to_string(),
            collapse_transformations: true,
            transform: precomp_transform(362.0, 256.0, 180.0),
            effects: Vec::new(),
        });
        self.notes.push(
            "Collapse probe depends on native scale-aware text rasterization approximation."
                .to_string(),
        );
    }

    fn finish(mut self) -> CaseRecipe {
        self.layers.reverse();
        let mut composition = Composition {
            id: self.case_id.to_string(),
            width: self.manifest.composition.width,
            height: self.manifest.composition.height,
            fps: self.manifest.composition.fps,
            duration: self.manifest.composition.case_duration_seconds,
            background: ae_background_rgba8(),
            motion_blur: MotionBlurSettings::default(),
        };
        if self
            .layers
            .iter()
            .any(|layer| matches!(layer, Layer::Footage { transform, .. } if transform.motion_blur))
        {
            composition.motion_blur = MotionBlurSettings {
                enabled: true,
                samples: 8,
                shutter_angle: 180.0,
                shutter_phase: -90.0,
            };
        }
        CaseRecipe {
            scene: Scene {
                version: "0.1".to_string(),
                composition,
                compositions: self.compositions,
                assets: scene_assets(),
                layers: self.layers,
            },
            notes: self.notes,
        }
    }
}

fn scene_assets() -> Vec<Asset> {
    [
        ("transparent", "assets/primitives/transparent.png"),
        ("impulse_center", "assets/primitives/impulse_center.png"),
        ("impulse_grid", "assets/primitives/impulse_grid.png"),
        ("alpha_square", "assets/primitives/alpha_square.png"),
        ("alpha_ramp", "assets/primitives/alpha_ramp.png"),
        ("luma_ramp", "assets/primitives/luma_ramp.png"),
        ("coordinate_field", "assets/primitives/coordinate_field.png"),
        ("checkerboard_16", "assets/primitives/checkerboard_16.png"),
        ("hard_edge", "assets/primitives/hard_edge.png"),
        ("color_bars", "assets/primitives/color_bars.png"),
        ("premult_probe", "assets/primitives/premult_probe.png"),
        (
            "numbered_frames",
            "assets/primitives/numbered_frames/frame_0000.png",
        ),
    ]
    .into_iter()
    .map(|(id, path)| Asset {
        id: id.to_string(),
        kind: AssetKind::Image,
        path: path.to_string(),
    })
    .collect()
}

fn placed_transform(x: f32, y: f32, scale: f32, opacity: f32) -> Transform2D {
    Transform2D {
        anchor: [128.0, 128.0],
        position: [x, y],
        scale: [scale, scale],
        rotation: 0.0,
        opacity,
        motion_blur: false,
        animation: Transform2DAnimation::default(),
    }
}

fn precomp_transform(x: f32, y: f32, scale: f32) -> Transform2D {
    Transform2D {
        anchor: [256.0, 256.0],
        position: [x, y],
        scale: [scale, scale],
        rotation: 0.0,
        opacity: 100.0,
        motion_blur: false,
        animation: Transform2DAnimation::default(),
    }
}

fn effect(match_name: &str, params: Value) -> EffectSpec {
    EffectSpec {
        match_name: match_name.to_string(),
        params,
    }
}

fn animated_scalar_param(t0: f64, v0: f32, t1: f64, v1: f32) -> Value {
    json!({
        "keyframes": [
            { "t": t0, "v": v0 },
            { "t": t1, "v": v1 }
        ]
    })
}

fn vec2_key(time: f64, value: [f32; 2], hold: bool, ease: Option<KeyframeEase>) -> Vec2Keyframe {
    Vec2Keyframe {
        time,
        value,
        hold,
        approximate: ease.is_some(),
        ease,
    }
}

fn scalar_key(time: f64, value: f32, hold: bool, ease: Option<KeyframeEase>) -> ScalarKeyframe {
    ScalarKeyframe {
        time,
        value,
        hold,
        approximate: ease.is_some(),
        ease,
    }
}

fn ease_in_out() -> KeyframeEase {
    KeyframeEase {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    }
}

fn range_animator(
    name: &str,
    based_on: TextSelectorBasedOn,
    start_value: f32,
    end_value: f32,
    duration: f64,
) -> TextAnimatorSpec {
    TextAnimatorSpec {
        name: name.to_string(),
        opacity: 0.0,
        selector: TextRangeSelector {
            start: start_value,
            end: 100.0,
            start_keyframes: vec![
                scalar_key(0.0, start_value, false, None),
                scalar_key(duration, end_value, false, None),
            ],
            based_on,
            ..TextRangeSelector::default()
        },
        ..TextAnimatorSpec::default()
    }
}

fn glyph_animator(duration: f64) -> TextAnimatorSpec {
    TextAnimatorSpec {
        name: "glyph_position_scale_rotation_blur".to_string(),
        opacity: 100.0,
        position: Some([0.0, -72.0]),
        scale: Some([125.0, 125.0]),
        rotation: Some(18.0),
        blur: Some([10.0, 10.0]),
        selector: TextRangeSelector {
            start: 0.0,
            end: 100.0,
            start_keyframes: vec![
                scalar_key(0.0, 0.0, false, None),
                scalar_key(duration, 100.0, false, None),
            ],
            based_on: TextSelectorBasedOn::Characters,
            ..TextRangeSelector::default()
        },
        ..TextAnimatorSpec::default()
    }
}

fn bounce_animator() -> TextAnimatorSpec {
    TextAnimatorSpec {
        name: "expression_selector_bounce".to_string(),
        scale: Some([0.0, 0.0]),
        selector: TextRangeSelector {
            start: 0.0,
            end: 100.0,
            based_on: TextSelectorBasedOn::Characters,
            ..TextRangeSelector::default()
        },
        expression_selector: Some(TextExpressionSelector::PerCharacterBounce {
            delay: 0.05,
            freq: 2.0,
            amplitude: 100.0,
            decay: 8.0,
            source: "ae_conformance_bounce_selector".to_string(),
        }),
        ..TextAnimatorSpec::default()
    }
}

fn font_montserrat(pack_root: &Path) -> String {
    pack_root
        .join("assets/fonts/Montserrat-Italic[wght].ttf")
        .display()
        .to_string()
}

fn font_point_light(pack_root: &Path) -> String {
    pack_root
        .join("assets/fonts/Point-Light.ttf")
        .display()
        .to_string()
}

fn ae_background_rgba8() -> [u8; 4] {
    [5, 5, 6, 0]
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_core::FootageProvider;

    fn pack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/ae_conformance_pack")
    }

    #[test]
    fn all_manifest_cases_have_native_recipes() {
        let root = pack_root();
        let manifest = load_manifest(&root).unwrap();
        for case in &manifest.cases {
            build_recipe(&manifest, &root, case)
                .unwrap_or_else(|err| panic!("{} should have a native recipe: {err:#}", case.id));
        }
    }

    #[test]
    fn pack_provider_loads_still_and_numbered_frame() {
        let root = pack_root();
        let assets = load_primitive_assets(&root).unwrap();
        let mut provider = PackFootageProvider::new(root, 30.0, assets);

        let still = provider.frame_at("alpha_square", 0.0).unwrap().unwrap();
        assert_eq!((still.width, still.height), (256, 256));

        let frame = provider
            .frame_at("numbered_frames", 10.0 / 30.0)
            .unwrap()
            .unwrap();
        assert_eq!((frame.width, frame.height), (256, 256));
        assert_eq!(frame.pixel(0, 20)[0], 10);
    }

    #[test]
    fn primitive_recipe_renders_selected_frame() {
        let root = pack_root();
        let manifest = load_manifest(&root).unwrap();
        let assets = load_primitive_assets(&root).unwrap();
        let case = manifest
            .cases
            .iter()
            .find(|case| case.id == "PRI_010")
            .unwrap();
        let recipe = build_recipe(&manifest, &root, case).unwrap();
        let mut provider = PackFootageProvider::new(root, manifest.composition.fps, assets);
        let canvas =
            render_core::layer_eval::render_frame_with_footage(&recipe.scene, 0, &mut provider)
                .unwrap();
        assert_eq!((canvas.width, canvas.height), (512, 512));
    }

    #[test]
    fn text_passport_compare_accepts_reference_subset() {
        let mut expected = TextPassportSnapshot::default();
        expected.layouts.insert(
            "TXT_030::text".to_string(),
            vec![json!({
                "glyphs": [{
                    "glyph_run_index": 0,
                    "font_glyph_id": 42,
                    "advance_x": 12.0,
                    "bbox": [1.0, 2.0, 3.0, 4.0]
                }]
            })],
        );

        let mut actual = TextPassportSnapshot::default();
        actual.layouts.insert(
            "TXT_030::text".to_string(),
            vec![json!({
                "composition": "TXT_030",
                "layer_id": "text",
                "glyphs": [{
                    "glyph_run_index": 0,
                    "font_glyph_id": 42,
                    "advance_x": 12.0005,
                    "bbox": [1.0, 2.0, 3.0, 4.0],
                    "metric_source": "fontdue"
                }]
            })],
        );

        let stats = compare_text_passport_snapshots(&expected, &actual);

        assert!(stats.is_ok(), "{:?}", stats.first_mismatch);
        assert_eq!(stats.mismatches, 0);
        assert!(stats.fields_compared > 0);
    }

    #[test]
    fn text_passport_compare_reports_first_divergent_glyph_field() {
        let mut expected = TextPassportSnapshot::default();
        expected.layouts.insert(
            "TXT_030::text".to_string(),
            vec![json!({
                "glyphs": [{
                    "glyph_run_index": 0,
                    "font_glyph_id": 42,
                    "advance_x": 12.0
                }]
            })],
        );

        let mut actual = TextPassportSnapshot::default();
        actual.layouts.insert(
            "TXT_030::text".to_string(),
            vec![json!({
                "glyphs": [{
                    "glyph_run_index": 0,
                    "font_glyph_id": 43,
                    "advance_x": 14.0
                }]
            })],
        );

        let stats = compare_text_passport_snapshots(&expected, &actual);

        assert!(!stats.is_ok());
        assert_eq!(stats.mismatches, 2);
        assert!(stats.max_abs_delta >= 2.0);
        let first = stats.first_mismatch.unwrap();
        assert!(first["path"].as_str().unwrap().contains("glyphs"));
    }

    #[test]
    fn jsonl_text_passport_snapshot_reads_native_sidecar_events() {
        let records = vec![
            json!({
                "event": "text.layout",
                "frame": 0,
                "record": {
                    "composition": "TXT_030",
                    "layer_id": "text",
                    "layout": {
                        "glyphs": [{
                            "character": "G",
                            "font_glyph_id": 42,
                            "glyph_run_index": 0,
                            "advance_x": 12.0
                        }]
                    }
                }
            }),
            json!({
                "event": "text.selector_weights",
                "frame": 0,
                "record": {
                    "composition": "TXT_030",
                    "layer_id": "text",
                    "animator": "glyph_motion",
                    "units": [{
                        "index": 0,
                        "glyph_passport": {
                            "glyph_run_indices": [0],
                            "font_glyph_ids": [42]
                        }
                    }]
                }
            }),
        ];

        let snapshot = text_passport_snapshot_from_jsonl_records(&records);

        assert_eq!(snapshot.layouts.len(), 1);
        assert_eq!(snapshot.selectors.len(), 1);
        assert_eq!(
            text_passport_snapshot_summary(&snapshot)["glyph_rows"],
            json!(1)
        );
        assert_eq!(
            text_passport_snapshot_summary(&snapshot)["selector_units"],
            json!(1)
        );
    }

    #[test]
    fn jsonl_text_passport_reference_can_omit_unknown_fields() {
        let reference = text_passport_snapshot_from_jsonl_records(&[json!({
            "event": "text.layout",
            "record": {
                "composition": "TXT_030",
                "layer_id": "text",
                "layout": {
                    "glyphs": [{
                        "character": "G",
                        "char_index": 0
                    }]
                }
            }
        })]);
        let native = text_passport_snapshot_from_jsonl_records(&[json!({
            "event": "text.layout",
            "record": {
                "composition": "TXT_030",
                "layer_id": "text",
                "layout": {
                    "glyphs": [{
                        "character": "G",
                        "char_index": 0,
                        "font_glyph_id": 42,
                        "advance_x": 59.3,
                        "metric_source": "fontdue"
                    }]
                }
            }
        })]);

        let stats = compare_text_passport_snapshots(&reference, &native);

        assert!(stats.is_ok(), "{:?}", stats.first_mismatch);
    }
}
