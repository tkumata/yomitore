#!/usr/bin/env bash
set -euo pipefail

AGENT="${1:-codex}"

STATE_DIR=".agent-hooks/state"
STATE_FILE="${STATE_DIR}/pipeline_state"
SNAPSHOT_FILE="${STATE_DIR}/review_snapshot"
LOG_DIR="${STATE_DIR}/logs"

mkdir -p "${LOG_DIR}"

if [ ! -f "${STATE_FILE}" ]; then
  echo "check_pending" > "${STATE_FILE}"
fi

PHASE="$(cat "${STATE_FILE}")"
INPUT="$(cat)"

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

summarize_diff() {
  git diff --stat -- . ':(exclude).agent-hooks/state'
}

worktree_signature() {
  git status --short --untracked-files=all -- . ':(exclude).agent-hooks/state' | shasum -a 256 | awk '{print $1}'
}

if [ "${PHASE}" != "review_pending" ]; then
  emit_continue "Review can only run after make build succeeds. Current pipeline phase: ${PHASE}."
  exit 0
fi

REVIEW_DECISION="$(jq -r '.decision // .reviewDecision // .status // empty' <<<"${INPUT}")"
REVIEW_FINDINGS="$(jq -r '
  if has("findings") and (.findings | type) == "array" then
    .findings
    | map(
        if type == "string" then
          .
        else
          [
            (.severity // "medium"),
            (.file // .path // "unknown"),
            (if .line then ":" + (.line | tostring) else "" end),
            (if .message then " " + .message else "" end)
          ]
          | join("")
        end
      )
    | join("\n")
  else
    empty
  end
' <<<"${INPUT}")"

if [ "${REVIEW_DECISION}" = "approve" ] || [ "${REVIEW_DECISION}" = "pass" ] || [ "${REVIEW_DECISION}" = "approved" ]; then
  worktree_signature > "${SNAPSHOT_FILE}"
  echo "done" > "${STATE_FILE}"
  emit_stop "Code review passed. Task complete."
  exit 0
fi

run_and_log "review" "git status --short && printf '\n--- diff stat ---\n' && git diff --stat -- . ':(exclude).agent-hooks/state'"

if [ -n "${REVIEW_FINDINGS}" ]; then
  emit_continue "Code review found actionable findings. Fix them, then rerun the Review hook.\n\n${REVIEW_FINDINGS}"
  exit 0
fi

emit_continue "Code review is required before stopping. Inspect .agent-hooks/state/logs/review.log, review the current diff, then rerun the Review hook with decision=approve when there are no findings.\n\nCurrent diff:\n$(summarize_diff)"
