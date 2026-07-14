# Rust Gen Manager

`render-manager` is the only service allowed to control Podman on a Rust render node. The
orchestrator sends a request to the manager, then polls its status. It never receives the Podman
socket and it never passes S3 credentials to a renderer container.

## HTTP contract

- `POST /render` accepts `ae-native-renderer.manager-request.v1` and returns `202` with a stable
  `render_id`.
- `GET /render/{render_id}` returns `queued`, `running`, `succeeded`, `failed`, or `cancelled`.
- `DELETE /render/{render_id}` requests cancellation and removes the active Podman container.
- `GET /health` is intentionally lightweight for deployment probes.

The full request schema is [`../schemas/render-manager-request-v1.schema.json`](../schemas/render-manager-request-v1.schema.json).
The manager accepts either an already-native renderer request or a bot envelope. Asset URLs and
output upload URLs must be short-lived HTTP(S) presigned URLs. The manager returns stable
`artifact_ref` values, never the presigned URLs themselves.

```json
{
  "schema": "ae-native-renderer.manager-request.v1",
  "job_id": "job_42",
  "input": {
    "kind": "native_request",
    "url": "https://object-store.example/request.json",
    "sha256": "..."
  },
  "assets": [{
    "role": "footage",
    "url": "https://object-store.example/clip.mp4",
    "destination": "app/media/video/clip.mp4"
  }],
  "uploads": {
    "video": {"url": "https://object-store.example/put-video", "artifact_ref": "s3://artifacts/job_42/result.mp4"},
    "manifest": {"url": "https://object-store.example/put-manifest", "artifact_ref": "s3://artifacts/job_42/output-manifest.json"}
  },
  "limits": {"timeout_s": 1800, "memory": "12g", "cpus": "6"}
}
```

## Runtime safety

Every job is materialized into an isolated directory. Its renderer container is rootless,
read-only, network-disabled, capability-free, CPU/memory/pid-limited and named from the validated
render ID. The manager retains logs and local artifacts for `RUST_GEN_JOB_RETENTION_S` before
removing the job directory.

For a containerized manager, mount the rootless Podman socket and `RUST_GEN_WORK_ROOT` at the
same absolute host path into the manager container. This makes the per-job mount visible to the
host Podman daemon without giving the orchestrator socket access. The production unit binds the
manager to `0.0.0.0:8090`; Timeweb firewall rules and the bearer token are both required.

## Configuration

- `RUST_GEN_BIND`, default `127.0.0.1:8090`
- `RUST_GEN_WORK_ROOT`, default `/var/lib/rust-gen/jobs`
- `RUST_GEN_RENDERER_IMAGE`, immutable image digest in production
- `RUST_GEN_MAX_PARALLEL_JOBS`, default `1`
- `RUST_GEN_DEFAULT_TIMEOUT_S`, `RUST_GEN_DEFAULT_MEMORY`, `RUST_GEN_DEFAULT_CPUS`
- `RUST_GEN_JOB_RETENTION_S`, default `86400`
- `RUST_GEN_MANAGER_TOKEN`, optional bearer token required by all render endpoints
