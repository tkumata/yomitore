#!/usr/bin/env bash
set -euo pipefail

AGENT="${1:-codex}"
STATE_FILE=".agent-hooks/state/pipeline_state"
LOG_DIR=".agent-hooks/state/logs"
REVIEW_INSTRUCTION="Full Rust verification passed. Before stopping, review the current uncommitted changes for correctness, regressions, security, test coverage, and documentation consistency. Fix every actionable finding within the requested scope. If you change Rust-related files, finish the fixes and let the Stop hook rerun verification before claiming completion. If there are no findings, report that explicitly and stop."
RUST_PATHS=(
  ':(glob)*.rs'
  ':(glob)**/*.rs'
  Cargo.toml
  Cargo.lock
  build.rs
  rust-toolchain
  rust-toolchain.toml
  ':(glob).cargo/**'
)

mkdir -p "${LOG_DIR}"

save_state() {
  printf '%s %s\n' "$1" "${2:-}" > "${STATE_FILE}"
}

has_rust_changes() {
  ! git diff --quiet HEAD -- "${RUST_PATHS[@]}" \
    || [ -n "$(git ls-files --others --exclude-standard -- "${RUST_PATHS[@]}")" ]
}

rust_fingerprint() {
  {
    git diff --binary HEAD -- "${RUST_PATHS[@]}"

    while IFS= read -r -d '' path; do
      printf 'untracked:%s\0' "${path}"
      git hash-object -- "${path}"
    done < <(git ls-files -z --others --exclude-standard -- "${RUST_PATHS[@]}" | sort -z)
  } | shasum -a 256 | awk '{print $1}'
}

run_and_log() {
  local name="$1"
  local cmd="$2"

  {
    echo "\$ ${cmd}"
    echo
    bash -lc "${cmd}"
  } >"${LOG_DIR}/${name}.log" 2>&1
}

emit_continue() {
  local reason="$1"

  if [ "${AGENT}" = "copilot" ]; then
    jq -nc --arg reason "${reason}" '{continue:false, message:$reason}'
  else
    jq -nc --arg reason "${reason}" '{decision:"block", reason:$reason}'
  fi
}

emit_stop() {
  local message="$1"

  if [ "${AGENT}" = "copilot" ]; then
    jq -nc --arg message "${message}" '{continue:true, message:$message}'
  else
    jq -nc --arg message "${message}" \
      '{continue:false, stopReason:$message, systemMessage:$message}'
  fi
}

STATE="idle"
STORED_FINGERPRINT=""
if [ -f "${STATE_FILE}" ]; then
  read -r STATE STORED_FINGERPRINT < "${STATE_FILE}" || true
fi

if ! has_rust_changes; then
  save_state idle
  emit_stop "No Rust-related changes require validation."
  exit 0
fi

FINGERPRINT="$(rust_fingerprint)"

if [ "${STATE}" = "done" ] && [ "${STORED_FINGERPRINT}" = "${FINGERPRINT}" ]; then
  emit_stop "Validation and code review request already completed for the current Rust changes."
  exit 0
fi

if [ "${STATE}" = "checked" ] && [ "${STORED_FINGERPRINT}" = "${FINGERPRINT}" ]; then
  if ! run_and_log build "make build"; then
    emit_continue "make build failed. Fix the root cause and review .agent-hooks/state/logs/build.log."
    exit 0
  fi

  if [ "$(rust_fingerprint)" != "${FINGERPRINT}" ]; then
    save_state idle
    emit_continue "Rust-related changes changed during build. Run make check again."
    exit 0
  fi

  save_state done "${FINGERPRINT}"
  emit_continue "${REVIEW_INSTRUCTION}"
  exit 0
fi

if run_and_log check "make check"; then
  save_state checked "${FINGERPRINT}"
  emit_continue "make check passed. Next run make build if Rust-related changes remain unchanged."
else
  save_state idle
  emit_continue "make check failed. Fix the root cause and review .agent-hooks/state/logs/check.log."
fi
