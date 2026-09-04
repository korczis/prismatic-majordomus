#!/usr/bin/env bash
# watch — what has drifted? Read-only. Exit 11 when any drift is found, 0 otherwise.
#
# watch enforces the same doctrines as check and doctor and asks a different question of
# them: not "is this wrong" but "has this moved". It dispatches from the same registry, so
# a rule cannot be watched without being declared, and every deviation carries DRIFT —
# advisory ones included, because watch never blocks work and its exit 11 only says it
# found something. What remains here is watch's own: the outcome-specific reporting and
# the resolver's view of the newest handover, neither of which is a rule.
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
# shellcheck source=doctor.sh
. "$MJ_LIB_DIR/doctor.sh"
mj_cmd_watch() {
  local a; for a in "$@"; do case "$a" in
    --help|-h) echo "usage: majordomus watch [--json]   (read-only; exit 0 no drift, 11 drift found)"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "watch: unknown option $a" ;; esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"

  # The task-scoped doctrines need the task loaded before they run; the rest do not care.
  # A checkout with no active task still watches its policy, projections and stores.
  local have_task=0
  if mj_load_current; then
    have_task=1
    mj_load_profile "$(mj_cur profile)" 2>/dev/null || mj_drift state "$(mj_cur id)" "profile '$(mj_cur profile)' has no file"
  fi

  mj_doctrine_dispatch watch

  if [ "$have_task" = 1 ]; then
    local id; id="$(mj_cur id)"
    case "$(mj_cur outcome)" in
      active) mj_ok state "$id" "active; scope, checkpoint, blockers and records reported above" ;;
      completed|partial|blocked|no_match|failed)
        if grep -q "\"event\":\"task.finished\".*\"task_id\":\"$id\"" "$MJ_DIR/state/ledger.jsonl" 2>/dev/null; then mj_ok verification "$id" "$(mj_cur outcome) with a finish record"
        else mj_drift verification "$id" "marked $(mj_cur outcome) but no task.finished record in the ledger" "grep task.finished .majordomus/state/ledger.jsonl"; fi ;;
      handed_over)
        local hv; hv="$(grep -l "^task_id: $id$" "$MJ_DIR"/state/handovers/*.md 2>/dev/null | sort | tail -n1)"
        if [ -z "$hv" ]; then mj_drift handover "$id" "marked handed_over but no handover file names it" "ls .majordomus/state/handovers"
        else local miss; miss="$(mj_check_sections "$hv" "$(mj_ylist "$MJ_POL_FLAT" handover.required_sections | tr '\n' '|')")"
          if [ -z "$miss" ]; then mj_ok handover "$id" "$(basename "$hv")"; else mj_drift handover "$(basename "$hv")" "missing section(s): $miss" "grep -n '^# ' $hv"; fi; fi ;;
      *) mj_drift state "$id" "unknown outcome '$(mj_cur outcome)'" "cat .majordomus/state/current.yaml" ;;
    esac
    # a checkpoint record for the active task, distinct from checkpoint_at
    if [ "$(mj_cur outcome)" = active ]; then
      if mj_resolve_latest "$MJ_DIR/state/checkpoints" "$id"; then
        mj_ok checkpoint "$(basename "$MJ_RES_PATH")" "$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")"
      else mj_info checkpoint "$id" "no checkpoint record; only checkpoint_at is set" "majordomus checkpoint"; fi
    fi
  else mj_info state "-" "no active task"; fi

  [ "$MJ_JSON" = 1 ] || printf 'watch: %s drift finding(s)\n' "$MJ_FAILS"
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_DRIFT"
}

# The newest handover this checkout resolves to, against the git state it recorded. This
# is the watch branch of handover_integrity: doctor asks whether the resolver works,
# watch asks whether what it resolves to still describes this history.
mj_watch_resolver() {
  if mj_resolve_latest "$MJ_DIR/state/handovers" ""; then
    local label; label="$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")"
    case "$label" in
      diverged|different_context)
        mj_doctrine_fail handover "$(basename "$MJ_RES_PATH")" "$label — the newest record for this branch describes a history this checkout no longer has" "majordomus handover --resolve" ;;
      *) mj_doctrine_ok handover "$(basename "$MJ_RES_PATH")" "$label, $(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" ;;
    esac
  else mj_doctrine_ok resolver "handovers" "no record for this worktree and branch (absence, not a stale match)" "majordomus handover --resolve"; fi
  [ "${MJ_RES_SKIPPED:-0}" -gt 0 ] && mj_doctrine_fail handover "handovers" "$MJ_RES_SKIPPED record(s) skipped as malformed" "majordomus handover --list"
  return 0
}

# An installation whose projected instructions point workers at `prompt list` and which
# has no assets at all is drift, not a broken asset — so it belongs to watch's branch.
mj_watch_prompts_empty() {
  local f n=0
  [ -d "$MJ_DIR/prompts" ] || return 0
  for f in "$MJ_DIR"/prompts/*.md; do [ -f "$f" ] && n=$((n + 1)); done
  [ "$n" = 0 ] && mj_doctrine_fail prompt ".majordomus/prompts" "no assets; the projected instructions point workers at prompt list" "majordomus update"
  return 0
}
# handover.sh provides mj_check_sections; the rest provide the store validators and builder
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
# shellcheck source=question.sh
. "$MJ_LIB_DIR/question.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
# shellcheck source=prompt.sh
. "$MJ_LIB_DIR/prompt.sh"
# shellcheck source=context.sh
. "$MJ_LIB_DIR/context.sh"
