#!/usr/bin/env bash
set -euo pipefail

: "${RUST_GEN_DEPLOY_HOST:?set RUST_GEN_DEPLOY_HOST}"
: "${RUST_GEN_MANAGER_IMAGE:?set immutable RUST_GEN_MANAGER_IMAGE}"
: "${RUST_GEN_MANAGER_TOKEN:?set RUST_GEN_MANAGER_TOKEN}"
: "${RUST_GEN_REGISTRY_TOKEN:?set RUST_GEN_REGISTRY_TOKEN}"
: "${RUST_GEN_RENDERER_IMAGE:=${RUST_GEN_MANAGER_IMAGE}}"

DEPLOY_USER="${RUST_GEN_DEPLOY_USER:-root}"
SERVICE_USER="${RUST_GEN_SERVICE_USER:-rustgen}"
REGISTRY_USER="${RUST_GEN_REGISTRY_USER:-${GITHUB_ACTOR:-}}"
DEPLOY_PORT="${RUST_GEN_DEPLOY_PORT:-22}"
WORK_ROOT="${RUST_GEN_WORK_ROOT:-/var/lib/rust-gen/jobs}"
MAX_PARALLEL="${RUST_GEN_MAX_PARALLEL_JOBS:-1}"
DEFAULT_TIMEOUT="${RUST_GEN_DEFAULT_TIMEOUT_S:-1800}"
DEFAULT_MEMORY="${RUST_GEN_DEFAULT_MEMORY:-12g}"
DEFAULT_CPUS="${RUST_GEN_DEFAULT_CPUS:-6}"
RETENTION="${RUST_GEN_JOB_RETENTION_S:-86400}"
SSH=(ssh -p "$DEPLOY_PORT" -o BatchMode=yes -o StrictHostKeyChecking=accept-new "${DEPLOY_USER}@${RUST_GEN_DEPLOY_HOST}")
SCP=(scp -P "$DEPLOY_PORT" -o BatchMode=yes -o StrictHostKeyChecking=accept-new)
if [[ -n "${RUST_GEN_SSH_KEY_PATH:-}" ]]; then
  SSH=(ssh -i "$RUST_GEN_SSH_KEY_PATH" -p "$DEPLOY_PORT" -o BatchMode=yes -o StrictHostKeyChecking=accept-new "${DEPLOY_USER}@${RUST_GEN_DEPLOY_HOST}")
  SCP=(scp -i "$RUST_GEN_SSH_KEY_PATH" -P "$DEPLOY_PORT" -o BatchMode=yes -o StrictHostKeyChecking=accept-new)
fi

if [[ "$RUST_GEN_MANAGER_IMAGE" != *@sha256:* ]]; then
  echo "RUST_GEN_MANAGER_IMAGE must be immutable image@sha256:digest" >&2
  exit 2
fi
if [[ -z "$REGISTRY_USER" ]]; then
  echo "set RUST_GEN_REGISTRY_USER when deploying outside GitHub Actions" >&2
  exit 2
fi

"${SSH[@]}" "install -d -o '$SERVICE_USER' -g '$SERVICE_USER' -m 700 /home/'$SERVICE_USER'/.config/rust-gen /home/'$SERVICE_USER'/.config/systemd/user; loginctl enable-linger '$SERVICE_USER'; runuser -u '$SERVICE_USER' -- env XDG_RUNTIME_DIR=/run/user/\$(id -u '$SERVICE_USER') systemctl --user enable --now podman.socket"
"${SCP[@]}" infra/systemd/rust-gen-manager.service "${DEPLOY_USER}@${RUST_GEN_DEPLOY_HOST}:/tmp/rust-gen-manager.service"
printf '%s' "$RUST_GEN_REGISTRY_TOKEN" | "${SSH[@]}" "runuser -u '$SERVICE_USER' -- podman login --username '$REGISTRY_USER' --password-stdin ghcr.io; install -o '$SERVICE_USER' -g '$SERVICE_USER' -m 600 /tmp/rust-gen-manager.service /home/'$SERVICE_USER'/.config/systemd/user/rust-gen-manager.service; rm -f /tmp/rust-gen-manager.service"

env_payload=$(cat <<EOF
RUST_GEN_MANAGER_IMAGE=$RUST_GEN_MANAGER_IMAGE
RUST_GEN_RENDERER_IMAGE=$RUST_GEN_RENDERER_IMAGE
RUST_GEN_MANAGER_TOKEN=$RUST_GEN_MANAGER_TOKEN
RUST_GEN_WORK_ROOT=$WORK_ROOT
RUST_GEN_MAX_PARALLEL_JOBS=$MAX_PARALLEL
RUST_GEN_DEFAULT_TIMEOUT_S=$DEFAULT_TIMEOUT
RUST_GEN_DEFAULT_MEMORY=$DEFAULT_MEMORY
RUST_GEN_DEFAULT_CPUS=$DEFAULT_CPUS
RUST_GEN_JOB_RETENTION_S=$RETENTION
EOF
)
printf '%s\n' "$env_payload" | "${SSH[@]}" "umask 077; cat > /home/'$SERVICE_USER'/.config/rust-gen/manager.env; chown '$SERVICE_USER':'$SERVICE_USER' /home/'$SERVICE_USER'/.config/rust-gen/manager.env; runuser -u '$SERVICE_USER' -- env XDG_RUNTIME_DIR=/run/user/\$(id -u '$SERVICE_USER') systemctl --user daemon-reload; runuser -u '$SERVICE_USER' -- env XDG_RUNTIME_DIR=/run/user/\$(id -u '$SERVICE_USER') systemctl --user restart rust-gen-manager; sleep 3; curl --fail --silent http://127.0.0.1:8090/health"
