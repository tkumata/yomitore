#!/usr/bin/env bash
set -euo pipefail

AGENT="${1:-codex}"
EVENT="${2:-Stop}"

STATE_DIR=".agent-hooks/state"
STATE_FILE="${STATE_DIR}/pipeline_state"
SNAPSHOT_FILE="${STATE_DIR}/review_snapshot"
LOG_DIR="${STATE_DIR}/logs"

mkdir -p "${LOG_DIR}"

if [ ! -f "${STATE_FILE}" ]; then
  echo "check_pending" > "${STATE_FILE}"
fi

PHASE="$(cat "${STATE_FILE}")"

run_and_log() {
  local name="$1"
  local cmd="$2"
  local log="${LOG_DIR}/${name}.log"

  {
    echo "\$ ${cmd}"
    echo
    bash -lc "${cmd}"
  } >"${log}" 2>&1
}

emit_continue() {
  local reason="$1"

  case "${AGENT}" in
    codex)
      jq -nc --arg reason "${reason}" \
        '{decision:"block", reason:$reason}'
      ;;
    copilot)
      jq -nc --arg reason "${reason}" \
        '{continue:false, message:$reason}'
      ;;
    *)
      jq -nc --arg reason "${reason}" \
        '{decision:"block", reason:$reason}'
      ;;
  esac
}

emit_stop() {
  local msg="$1"

  case "${AGENT}" in
    codex)
      jq -nc '{continue:false}'
      ;;
    copilot)
      jq -nc --arg msg "${msg}" \
        '{continue:true, message:$msg}'
      ;;
    *)
      jq -nc --arg msg "${msg}" \
        '{continue:false, stopReason:$msg, systemMessage:$msg}'
      ;;
  esac
}

worktree_signature() {
  git status --short --untracked-files=all -- . ':(exclude).agent-hooks/state' | shasum -a 256 | awk '{print $1}'
}

if [ "${PHASE}" = "done" ]; then
  CURRENT_SIGNATURE="$(worktree_signature)"
  SAVED_SIGNATURE=""

  if [ -f "${SNAPSHOT_FILE}" ]; then
    SAVED_SIGNATURE="$(cat "${SNAPSHOT_FILE}")"
  fi

  if [ -n "${CURRENT_SIGNATURE}" ] && [ "${CURRENT_SIGNATURE}" != "${SAVED_SIGNATURE}" ]; then
    echo "check_pending" > "${STATE_FILE}"
    PHASE="check_pending"
  fi
fi

if [ "${PHASE}" = "done" ]; then
  emit_stop "Validation pipeline already completed."
  exit 0
fi

if [ "${PHASE}" = "check_pending" ]; then
  if run_and_log "check" "make check"; then
    echo "build_pending" > "${STATE_FILE}"
    emit_continue "make check passed. Next run make build, inspect the result, and continue only if build succeeds."
  else
    echo "check_pending" > "${STATE_FILE}"
    emit_continue "make check failed. Fix the root cause and continue until check passes. Review .agent-hooks/state/logs/check.log before editing."
  fi
  exit 0
fi

if [ "${PHASE}" = "build_pending" ]; then
  if run_and_log "build" "make build"; then
    echo "review_pending" > "${STATE_FILE}"
    emit_continue "make build passed. Code review is required before stopping. Run ./.agent-hooks/review_pipeline.sh manually on the current diff, fix actionable findings, then approve."
  else
    echo "build_pending" > "${STATE_FILE}"
    emit_continue "make build failed. Fix the root cause and continue until build passes. Review .agent-hooks/state/logs/build.log before editing."
  fi
  exit 0
fi

if [ "${PHASE}" = "review_pending" ]; then
  emit_continue "Code review is required before stopping. Run ./.agent-hooks/review_pipeline.sh manually on the current diff, fix actionable findings, then continue."
  exit 0
fi

emit_stop "Unknown pipeline phase: ${PHASE}"
