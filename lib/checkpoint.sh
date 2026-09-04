#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_checkpoint:-}" ] && return 0 || MJ_LIB_checkpoint=1
# checkpoint — a compact progress record inside an active task. Append-only.
#
# checkpoint != handover. A handover is a deliberate continuation package written when a
# worker stops; a checkpoint is what was true a moment ago, small enough to be quoted into
# the next worker's context verbatim. The policy caps its length for exactly that reason.
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"

mj_cmd_checkpoint() {
  local show=0 list=0 path_only=0
  while [ $# -gt 0 ]; do case "$1" in
    --show) show=1; shift ;; --list) list=1; shift ;; --path) path_only=1; shift ;;
    --help|-h) cat <<H
usage: majordomus checkpoint < body.md      record progress (body optional; empty body = timestamp only)
       majordomus checkpoint --show [--path]
       majordomus checkpoint --list
  writes .ai/local/state/checkpoints/<ts>--<branch>--<head>--<rand>.md (mode 0600, atomic, never staged)
  the body is free text, not sections: at most the policy's checkpoint.max_body_lines lines
  it must not contain identity fields; those are computed from git
  --show   print the newest checkpoint for the active task in this worktree and branch
  --list   list this worktree's checkpoints, newest first
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "checkpoint: unknown option $1" ;;
  esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  local dir="$MJ_STATE_DIR/checkpoints"
  if [ "$list" = 1 ]; then mj_record_list "$dir" checkpoint; return; fi
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task ($(mj_rel "$MJ_STATE_DIR")/current.yaml); run: majordomus start"
  local id profile owner; id="$(mj_cur id)"; profile="$(mj_cur profile)"; owner="$(mj_cur owner)"

  if [ "$show" = 1 ]; then
    if ! mj_resolve_latest "$dir" "$id"; then echo "No checkpoint for $id."; return 0; fi
    if [ "$path_only" = 1 ]; then printf '%s\n' "${MJ_RES_PATH#"$MJ_ROOT/"}"; return 0; fi
    printf 'Checkpoint: %s\nCreated: %s (%s)\nGit state: %s\n---\n' "${MJ_RES_PATH#"$MJ_ROOT/"}" \
      "$MJ_RES_CREATED" "$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" \
      "$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")"
    mj_record_body "$MJ_RES_PATH"
    return 0
  fi

  case "$(mj_cur outcome)" in
    active) ;;
    *) mj_die "$MJ_EX_REFUSED" "task $id is $(mj_cur outcome); a checkpoint records progress inside an active task" ;;
  esac

  local body; body="$(mktemp "${TMPDIR:-/tmp}/mj.cb.XXXXXX")"; cat > "$body"
  local now; now="$(mj_now)" ; local final=""
  if [ -s "$body" ]; then
    if mj_reject_identity "$body"; then
      rm -f "$body"; mj_die "$MJ_EX_CONTRACT" "checkpoint: body must not contain identity fields; they are computed"
    fi
    local cap lines; cap="$(mj_pol_req checkpoint.max_body_lines)"
    lines="$(mj_lines "$body")"
    if [ "$lines" -gt "$cap" ]; then
      rm -f "$body"
      mj_die "$MJ_EX_CONTRACT" "checkpoint: body is $lines lines, cap $cap (a checkpoint is a progress note, not a report; write a handover instead)"
    fi
    local rec; rec="$(mktemp "${TMPDIR:-/tmp}/mj.cr.XXXXXX")"
    { mj_record_front_matter "$id" "$profile" "$owner"; cat "$body"; } > "$rec"
    final="$(mj_publish_record "$dir" "" "$rec")" || { rm -f "$rec" "$body"; mj_die "$MJ_EX_INTERNAL" "could not create a unique checkpoint file"; }
    rm -f "$rec"
  fi
  rm -f "$body"

  sed "s/^checkpoint_at: .*/checkpoint_at: $now/" "$MJ_CUR" > "$MJ_CUR.mj-tmp" && mv "$MJ_CUR.mj-tmp" "$MJ_CUR"
  if [ -n "$final" ]; then
    mj_ledger_append task.checkpoint "\"task_id\":\"$id\",\"checkpoint_path\":\"$(mj_json_esc "${final#"$MJ_ROOT/"}")\""
    printf '%s\n' "${final#"$MJ_ROOT/"}"
  else
    mj_ledger_append task.checkpoint "\"task_id\":\"$id\""
    printf 'checkpoint %s at %s (no body)\n' "$id" "$now"
  fi
}
