use anyhow::{anyhow, bail, Context, Result};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    env,
    path::{Component, Path as FsPath, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs,
    process::Command,
    sync::{Mutex, Semaphore},
    time::sleep,
};
use tracing::{error, info, warn};

const REQUEST_SCHEMA: &str = "ae-native-renderer.manager-request.v1";
const RESPONSE_SCHEMA: &str = "ae-native-renderer.manager-response.v1";

#[derive(Clone)]
struct AppState {
    config: Config,
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
    slots: Arc<Semaphore>,
}

#[derive(Clone)]
struct Config {
    bind: String,
    work_root: PathBuf,
    renderer_image: String,
    podman_bin: String,
    max_parallel_jobs: usize,
    default_timeout_s: u64,
    default_memory: String,
    default_cpus: String,
    retention_s: u64,
    bearer_token: Option<String>,
}

impl Config {
    fn from_env() -> Self {
        Self {
            bind: env_string("RUST_GEN_BIND", "127.0.0.1:8090"),
            work_root: PathBuf::from(env_string("RUST_GEN_WORK_ROOT", "/var/lib/rust-gen/jobs")),
            renderer_image: env_string(
                "RUST_GEN_RENDERER_IMAGE",
                "ghcr.io/vilebody/ae-native-renderer:latest",
            ),
            podman_bin: env_string("RUST_GEN_PODMAN_BIN", "podman"),
            max_parallel_jobs: env_usize("RUST_GEN_MAX_PARALLEL_JOBS", 1).max(1),
            default_timeout_s: env_u64("RUST_GEN_DEFAULT_TIMEOUT_S", 1800).max(1),
            default_memory: env_string("RUST_GEN_DEFAULT_MEMORY", "12g"),
            default_cpus: env_string("RUST_GEN_DEFAULT_CPUS", "6"),
            retention_s: env_u64("RUST_GEN_JOB_RETENTION_S", 86_400),
            bearer_token: env::var("RUST_GEN_MANAGER_TOKEN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
enum InputKind {
    NativeRequest,
    BotPayload,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct JobInput {
    kind: InputKind,
    #[serde(default)]
    inline: Option<Value>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AssetInput {
    role: String,
    url: String,
    destination: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    optional: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct UploadTarget {
    url: String,
    artifact_ref: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
struct UploadTargets {
    #[serde(default)]
    video: Option<UploadTarget>,
    #[serde(default)]
    manifest: Option<UploadTarget>,
    #[serde(default)]
    response: Option<UploadTarget>,
    #[serde(default)]
    logs: Option<UploadTarget>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
struct JobLimits {
    #[serde(default)]
    timeout_s: Option<u64>,
    #[serde(default)]
    memory: Option<String>,
    #[serde(default)]
    cpus: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct RenderRequest {
    schema: String,
    job_id: String,
    #[serde(default)]
    render_id: Option<String>,
    input: JobInput,
    #[serde(default)]
    assets: Vec<AssetInput>,
    #[serde(default)]
    uploads: UploadTargets,
    #[serde(default)]
    limits: JobLimits,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct JobRecord {
    render_id: String,
    job_id: String,
    status: String,
    request_hash: String,
    created_at_ms: u128,
    started_at_ms: Option<u128>,
    finished_at_ms: Option<u128>,
    artifact_refs: HashMap<String, String>,
    error: Option<String>,
    work_dir: Option<String>,
    cancel_requested: bool,
}

#[derive(Debug, Serialize)]
struct ManagerResponse {
    schema: &'static str,
    status: String,
    render_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    job: Option<JobRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let config = Config::from_env();
    fs::create_dir_all(&config.work_root)
        .await
        .with_context(|| format!("create work root {}", config.work_root.display()))?;

    let jobs = restore_jobs(&config.work_root).await?;
    let state = AppState {
        slots: Arc::new(Semaphore::new(config.max_parallel_jobs)),
        config,
        jobs: Arc::new(Mutex::new(jobs)),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/render", post(submit))
        .route("/render/{render_id}", get(status).delete(cancel))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind(&state.config.bind)
        .await
        .with_context(|| format!("bind {}", state.config.bind))?;
    info!(bind = %state.config.bind, workers = state.config.max_parallel_jobs, "rust-gen manager started");
    axum::serve(listener, app).await.context("serve manager")
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let jobs = state.jobs.lock().await;
    let queued = jobs.values().filter(|job| job.status == "queued").count();
    let running = jobs.values().filter(|job| job.status == "running").count();
    Json(json!({"ok": true, "service": "rust-gen-manager", "queued": queued, "running": running}))
}

async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RenderRequest>,
) -> impl IntoResponse {
    if let Err(error) = authorize(&state, &headers) {
        return api_error(StatusCode::UNAUTHORIZED, error.to_string());
    }
    if let Err(error) = validate_request(&request) {
        return api_error(StatusCode::BAD_REQUEST, error.to_string());
    }

    let request_hash = request_hash(&request);
    let render_id = request
        .render_id
        .clone()
        .unwrap_or_else(|| format!("rust-{}", request.job_id));
    if let Err(error) = safe_identifier(&render_id).and_then(|_| safe_identifier(&request.job_id)) {
        return api_error(StatusCode::BAD_REQUEST, error.to_string());
    }

    {
        let jobs = state.jobs.lock().await;
        if let Some(existing) = jobs.get(&render_id) {
            if existing.request_hash != request_hash {
                return api_error(
                    StatusCode::CONFLICT,
                    "render_id is already bound to another request".to_string(),
                );
            }
            return (
                StatusCode::ACCEPTED,
                Json(ManagerResponse {
                    schema: RESPONSE_SCHEMA,
                    status: "accepted".to_string(),
                    render_id,
                    job: Some(existing.clone()),
                    error: None,
                }),
            )
                .into_response();
        }
    }

    let work_dir = state.config.work_root.join(&render_id);
    if let Err(error) = fs::create_dir_all(&work_dir).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    let record = JobRecord {
        render_id: render_id.clone(),
        job_id: request.job_id.clone(),
        status: "queued".to_string(),
        request_hash,
        created_at_ms: now_ms(),
        started_at_ms: None,
        finished_at_ms: None,
        artifact_refs: HashMap::new(),
        error: None,
        work_dir: Some(work_dir.display().to_string()),
        cancel_requested: false,
    };
    state
        .jobs
        .lock()
        .await
        .insert(render_id.clone(), record.clone());
    if let Err(error) = persist_record(&record).await {
        state.jobs.lock().await.remove(&render_id);
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    let job_state = state.clone();
    let spawned_render_id = render_id.clone();
    tokio::spawn(async move {
        if let Err(error) = run_job(
            job_state.clone(),
            spawned_render_id.clone(),
            request,
            work_dir,
        )
        .await
        {
            error!(render_id = %spawned_render_id, error = %error, "rust-gen job failed");
            fail_job(&job_state, &spawned_render_id, error.to_string()).await;
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(ManagerResponse {
            schema: RESPONSE_SCHEMA,
            status: "accepted".to_string(),
            render_id,
            job: None,
            error: None,
        }),
    )
        .into_response()
}

async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(render_id): Path<String>,
) -> impl IntoResponse {
    if let Err(error) = authorize(&state, &headers) {
        return api_error(StatusCode::UNAUTHORIZED, error.to_string());
    }
    match state.jobs.lock().await.get(&render_id).cloned() {
        Some(job) => (
            StatusCode::OK,
            Json(ManagerResponse {
                schema: RESPONSE_SCHEMA,
                status: job.status.clone(),
                render_id,
                job: Some(job),
                error: None,
            }),
        )
            .into_response(),
        None => api_error(StatusCode::NOT_FOUND, "unknown render_id".to_string()),
    }
}

async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(render_id): Path<String>,
) -> impl IntoResponse {
    if let Err(error) = authorize(&state, &headers) {
        return api_error(StatusCode::UNAUTHORIZED, error.to_string());
    }
    let container_name = container_name(&render_id);
    let exists = {
        let mut jobs = state.jobs.lock().await;
        let Some(job) = jobs.get_mut(&render_id) else {
            return api_error(StatusCode::NOT_FOUND, "unknown render_id".to_string());
        };
        job.cancel_requested = true;
        !is_terminal(&job.status)
    };
    if exists {
        let _ = Command::new(&state.config.podman_bin)
            .args(["rm", "--force", &container_name])
            .output()
            .await;
    }
    let job = state.jobs.lock().await.get(&render_id).cloned();
    (
        StatusCode::ACCEPTED,
        Json(ManagerResponse {
            schema: RESPONSE_SCHEMA,
            status: "cancelling".to_string(),
            render_id,
            job,
            error: None,
        }),
    )
        .into_response()
}

async fn run_job(
    state: AppState,
    render_id: String,
    request: RenderRequest,
    work_dir: PathBuf,
) -> Result<()> {
    let _permit = state.slots.acquire().await.context("acquire render slot")?;
    if cancelled(&state, &render_id).await {
        return finish_cancelled(&state, &render_id).await;
    }
    {
        let mut jobs = state.jobs.lock().await;
        let job = jobs
            .get_mut(&render_id)
            .ok_or_else(|| anyhow!("job record disappeared"))?;
        job.status = "running".to_string();
        job.started_at_ms = Some(now_ms());
    }
    persist_current_record(&state, &render_id).await?;

    fs::create_dir_all(&work_dir).await?;
    fs::create_dir_all(work_dir.join("out")).await?;
    materialize_input(&request.input, &work_dir).await?;
    for asset in &request.assets {
        if let Err(error) = materialize_asset(asset, &work_dir).await {
            if asset.optional {
                warn!(render_id = %render_id, role = %asset.role, error = %error, "optional asset unavailable");
            } else {
                return Err(error);
            }
        }
    }
    if cancelled(&state, &render_id).await {
        return finish_cancelled(&state, &render_id).await;
    }

    let script = match request.input.kind {
        InputKind::NativeRequest => "render-cli json --request /job/input.json --response /job/out/render-response.json",
        InputKind::BotPayload => "render-cli adapt-bot-payload --input /job/input.json --out /job/request.json && render-cli json --request /job/request.json --response /job/out/render-response.json",
    };
    let timeout_s = request
        .limits
        .timeout_s
        .unwrap_or(state.config.default_timeout_s)
        .max(1);
    let memory = request
        .limits
        .memory
        .as_deref()
        .unwrap_or(&state.config.default_memory);
    let cpus = request
        .limits
        .cpus
        .as_deref()
        .unwrap_or(&state.config.default_cpus);
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_s),
        run_container(&state.config, &render_id, &work_dir, memory, cpus, script),
    )
    .await;
    match output {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(error),
        Err(_) => {
            let _ = Command::new(&state.config.podman_bin)
                .args(["rm", "--force", &container_name(&render_id)])
                .output()
                .await;
            bail!("render timed out after {timeout_s}s");
        }
    }
    if cancelled(&state, &render_id).await {
        return finish_cancelled(&state, &render_id).await;
    }

    let mut refs = HashMap::new();
    upload_if_requested(
        &request.uploads.video,
        &work_dir.join("out/result.mp4"),
        "video",
        &mut refs,
    )
    .await?;
    upload_if_requested(
        &request.uploads.manifest,
        &work_dir.join("out/output-manifest.json"),
        "manifest",
        &mut refs,
    )
    .await?;
    upload_if_requested(
        &request.uploads.response,
        &work_dir.join("out/render-response.json"),
        "response",
        &mut refs,
    )
    .await?;
    upload_if_requested(
        &request.uploads.logs,
        &work_dir.join("renderer.log"),
        "logs",
        &mut refs,
    )
    .await?;
    finish_success(&state, &render_id, refs).await;
    schedule_cleanup(state.clone(), render_id, work_dir).await;
    Ok(())
}

async fn materialize_input(input: &JobInput, work_dir: &FsPath) -> Result<()> {
    let target = work_dir.join("input.json");
    match (&input.inline, &input.url) {
        (Some(value), None) => fs::write(&target, serde_json::to_vec_pretty(value)?).await?,
        (None, Some(url)) => download(url, &target).await?,
        _ => bail!("input must provide exactly one of inline or url"),
    }
    if let Some(expected) = &input.sha256 {
        verify_sha256(&target, expected).await?;
    }
    if matches!(input.kind, InputKind::NativeRequest) {
        normalize_native_request(&target).await?;
    }
    Ok(())
}

async fn normalize_native_request(path: &FsPath) -> Result<()> {
    let raw = fs::read(path).await?;
    let mut request: Value = serde_json::from_slice(&raw).context("parse native render request")?;
    let object = request
        .as_object_mut()
        .ok_or_else(|| anyhow!("native request must be a JSON object"))?;
    object.insert(
        "schema".to_string(),
        Value::String("ae-native-renderer.render-request.v1".to_string()),
    );
    let assets = object.entry("assetsSpec").or_insert_with(|| json!({}));
    assets
        .as_object_mut()
        .ok_or_else(|| anyhow!("assetsSpec must be an object"))?
        .insert("root".to_string(), Value::String("/job".to_string()));
    let output = object.entry("outputSpec").or_insert_with(|| json!({}));
    let output = output
        .as_object_mut()
        .ok_or_else(|| anyhow!("outputSpec must be an object"))?;
    output.insert(
        "directory".to_string(),
        Value::String("/job/out".to_string()),
    );
    output.insert("video".to_string(), Value::String("result.mp4".to_string()));
    output.remove("frames");
    output
        .entry("writeScene".to_string())
        .or_insert(Value::Bool(true));
    fs::write(path, serde_json::to_vec_pretty(&request)?).await?;
    Ok(())
}

async fn materialize_asset(asset: &AssetInput, work_dir: &FsPath) -> Result<()> {
    let relative = safe_relative_path(&asset.destination)?;
    let target = work_dir.join(relative);
    download(&asset.url, &target).await?;
    if let Some(expected) = &asset.sha256 {
        verify_sha256(&target, expected).await?;
    }
    Ok(())
}

async fn download(url: &str, target: &FsPath) -> Result<()> {
    validate_url(url)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).await?;
    }
    let partial = target.with_extension("partial");
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--retry",
            "3",
            "--silent",
            "--show-error",
            "--output",
        ])
        .arg(&partial)
        .arg(url)
        .status()
        .await
        .context("start curl download")?;
    if !status.success() {
        let _ = fs::remove_file(&partial).await;
        bail!("download failed for a requested artifact");
    }
    fs::rename(partial, target).await?;
    Ok(())
}

async fn upload_if_requested(
    target: &Option<UploadTarget>,
    source: &FsPath,
    name: &str,
    refs: &mut HashMap<String, String>,
) -> Result<()> {
    let Some(target) = target else {
        return Ok(());
    };
    validate_url(&target.url)?;
    if !source.is_file() {
        bail!("expected {name} artifact was not produced");
    }
    let status = Command::new("curl")
        .args([
            "--fail",
            "--retry",
            "3",
            "--silent",
            "--show-error",
            "--upload-file",
        ])
        .arg(source)
        .arg(&target.url)
        .status()
        .await
        .context("start curl upload")?;
    if !status.success() {
        bail!("upload failed for {name} artifact");
    }
    refs.insert(name.to_string(), target.artifact_ref.clone());
    Ok(())
}

async fn run_container(
    config: &Config,
    render_id: &str,
    work_dir: &FsPath,
    memory: &str,
    cpus: &str,
    script: &str,
) -> Result<()> {
    let log_path = work_dir.join("renderer.log");
    let output = Command::new(&config.podman_bin)
        .args(["run", "--rm", "--name", &container_name(render_id)])
        .args([
            "--network",
            "none",
            "--read-only",
            "--tmpfs",
            "/tmp:rw,noexec,nosuid,size=512m",
        ])
        .args(["--pids-limit", "512", "--memory", memory, "--cpus", cpus])
        .args([
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--userns",
            "keep-id",
        ])
        .arg("--volume")
        .arg(format!("{}:/job:Z", work_dir.display()))
        .args(["--entrypoint", "/bin/sh"])
        .arg(&config.renderer_image)
        .args(["sh", "-lc", script])
        .output()
        .await
        .context("start podman render container")?;
    let mut log = output.stdout;
    log.extend_from_slice(&output.stderr);
    fs::write(log_path, log).await?;
    if !output.status.success() {
        bail!("renderer container exited with status {}", output.status);
    }
    Ok(())
}

async fn verify_sha256(path: &FsPath, expected: &str) -> Result<()> {
    let normalized = expected.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("sha256 must be a 64-character hex digest");
    }
    let data = fs::read(path).await?;
    let actual = format!("{:x}", Sha256::digest(data));
    if actual != normalized {
        bail!("sha256 mismatch for materialized artifact");
    }
    Ok(())
}

fn validate_request(request: &RenderRequest) -> Result<()> {
    if request.schema != REQUEST_SCHEMA {
        bail!("schema must be {REQUEST_SCHEMA}");
    }
    safe_identifier(&request.job_id)?;
    match (&request.input.inline, &request.input.url) {
        (Some(_), None) | (None, Some(_)) => {}
        _ => bail!("input must provide exactly one of inline or url"),
    }
    if let Some(url) = &request.input.url {
        validate_url(url)?;
    }
    for asset in &request.assets {
        if asset.role.trim().is_empty() {
            bail!("asset role must not be empty");
        }
        validate_url(&asset.url)?;
        safe_relative_path(&asset.destination)?;
    }
    for upload in [
        &request.uploads.video,
        &request.uploads.manifest,
        &request.uploads.response,
        &request.uploads.logs,
    ] {
        if let Some(upload) = upload {
            validate_url(&upload.url)?;
            if upload.artifact_ref.trim().is_empty() {
                bail!("upload artifact_ref must not be empty");
            }
        }
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<()> {
    let url = value.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        bail!("only http(s) artifact URLs are accepted");
    }
    Ok(())
}

fn safe_identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("identifier must contain only ASCII letters, digits, '.', '_' or '-'");
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = FsPath::new(value);
    if path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("asset destination must be a non-empty relative path without traversal");
    }
    Ok(path.to_path_buf())
}

fn request_hash(request: &RenderRequest) -> String {
    let bytes = serde_json::to_vec(request).expect("render request is serializable");
    format!("{:x}", Sha256::digest(bytes))
}

fn container_name(render_id: &str) -> String {
    format!("rust-gen-{render_id}")
}
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn is_terminal(status: &str) -> bool {
    matches!(status, "succeeded" | "failed" | "cancelled")
}
fn env_string(name: &str, default: &str) -> String {
    env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .trim()
        .to_string()
}
fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<()> {
    let Some(expected) = &state.config.bearer_token else {
        return Ok(());
    };
    let actual = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if actual == format!("Bearer {expected}") {
        Ok(())
    } else {
        bail!("missing or invalid bearer token")
    }
}

fn api_error(status: StatusCode, error: String) -> axum::response::Response {
    (
        status,
        Json(json!({"schema": RESPONSE_SCHEMA, "status": "failed", "error": error})),
    )
        .into_response()
}

async fn cancelled(state: &AppState, render_id: &str) -> bool {
    state
        .jobs
        .lock()
        .await
        .get(render_id)
        .map(|job| job.cancel_requested)
        .unwrap_or(true)
}

async fn finish_cancelled(state: &AppState, render_id: &str) -> Result<()> {
    let mut jobs = state.jobs.lock().await;
    let job = jobs
        .get_mut(render_id)
        .ok_or_else(|| anyhow!("job record disappeared"))?;
    job.status = "cancelled".to_string();
    job.finished_at_ms = Some(now_ms());
    let record = job.clone();
    drop(jobs);
    persist_record(&record).await?;
    Ok(())
}

async fn fail_job(state: &AppState, render_id: &str, error: String) {
    let mut jobs = state.jobs.lock().await;
    if let Some(job) = jobs.get_mut(render_id) {
        job.status = if job.cancel_requested {
            "cancelled"
        } else {
            "failed"
        }
        .to_string();
        job.error = if job.cancel_requested {
            None
        } else {
            Some(error)
        };
        job.finished_at_ms = Some(now_ms());
        let record = job.clone();
        drop(jobs);
        if let Err(error) = persist_record(&record).await {
            warn!(render_id = %render_id, error = %error, "could not persist failed job state");
        }
    }
}

async fn finish_success(state: &AppState, render_id: &str, artifact_refs: HashMap<String, String>) {
    let mut jobs = state.jobs.lock().await;
    if let Some(job) = jobs.get_mut(render_id) {
        job.status = "succeeded".to_string();
        job.artifact_refs = artifact_refs;
        job.finished_at_ms = Some(now_ms());
        let record = job.clone();
        drop(jobs);
        if let Err(error) = persist_record(&record).await {
            warn!(render_id = %render_id, error = %error, "could not persist successful job state");
        }
    }
}

async fn schedule_cleanup(state: AppState, render_id: String, work_dir: PathBuf) {
    let retention = state.config.retention_s;
    tokio::spawn(async move {
        sleep(Duration::from_secs(retention)).await;
        if let Err(error) = fs::remove_dir_all(&work_dir).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                warn!(render_id = %render_id, error = %error, "could not clean job work directory");
            }
        }
        if let Some(job) = state.jobs.lock().await.get_mut(&render_id) {
            job.work_dir = None;
        }
    });
}

async fn persist_current_record(state: &AppState, render_id: &str) -> Result<()> {
    let record = state
        .jobs
        .lock()
        .await
        .get(render_id)
        .cloned()
        .ok_or_else(|| anyhow!("job record disappeared"))?;
    persist_record(&record).await
}

async fn persist_record(record: &JobRecord) -> Result<()> {
    let Some(work_dir) = record.work_dir.as_ref() else {
        return Ok(());
    };
    let path = FsPath::new(work_dir).join("manager-status.json");
    let temporary = path.with_extension("json.partial");
    fs::write(&temporary, serde_json::to_vec_pretty(record)?).await?;
    fs::rename(temporary, path).await?;
    Ok(())
}

async fn restore_jobs(work_root: &FsPath) -> Result<HashMap<String, JobRecord>> {
    let mut recovered = HashMap::new();
    let mut entries = fs::read_dir(work_root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let status_path = path.join("manager-status.json");
        let Ok(raw) = fs::read(&status_path).await else {
            continue;
        };
        let Ok(mut record) = serde_json::from_slice::<JobRecord>(&raw) else {
            warn!(path = %status_path.display(), "ignoring unreadable persisted job state");
            continue;
        };
        if matches!(record.status.as_str(), "queued" | "running") {
            record.status = "failed".to_string();
            record.error =
                Some("manager restarted before terminal job status was persisted".to_string());
            record.finished_at_ms = Some(now_ms());
            persist_record(&record).await?;
        }
        recovered.insert(record.render_id.clone(), record);
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> RenderRequest {
        serde_json::from_value(json!({
            "schema": REQUEST_SCHEMA,
            "job_id": "job-1",
            "input": {"kind": "native_request", "inline": {"projectSpec": {}}},
            "assets": [{"role": "footage", "url": "https://example.test/source.mp4", "destination": "app/media/video/source.mp4"}],
            "uploads": {"video": {"url": "https://example.test/put", "artifact_ref": "s3://bucket/out.mp4"}}
        })).unwrap()
    }

    #[test]
    fn validates_a_materialized_native_job() {
        validate_request(&valid_request()).unwrap();
    }

    #[test]
    fn rejects_asset_path_traversal() {
        assert!(safe_relative_path("../outside.mp4").is_err());
        assert!(safe_relative_path("/absolute.mp4").is_err());
        assert!(safe_relative_path("app/media/clip.mp4").is_ok());
    }

    #[test]
    fn request_hash_is_stable() {
        let request = valid_request();
        assert_eq!(request_hash(&request), request_hash(&request));
    }
}
