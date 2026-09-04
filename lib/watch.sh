#!/usr/bin/env bash
# watch — what has drifted? Read-only. Exit 11 when any drift is found, 0 otherwise.
#
# watch enforces the same doctrines as check and doctor and asks a different question of
# them: not "is this wrong" but "has this moved". It dispatches from the same registry, so
# a doctrine cannot be watched without being declared, and the findings carry DRIFT.
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

  # The task-scoped doctrines need the task loaded before they run; the rest do not
  # care. A checkout with no active task still watches its policy and projections.
  local have_task=0
  if mj_load_current; then
    have_task=1
    mj_load_profile "$(mj_cur profile)" 2>/dev/null || mj_drift state "$(mj_cur id)" "profile '$(mj_cur profile)' has no file"
  fi

  mj_doctrine_dispatch watch

  if [ "$have_task" = 1 ]; then
    local id; id="$(mj_cur id)"
    case "$(mj_cur outcome)" in
      active) mj_ok state "$id" "active; scope, checkpoint and blockers reported above" ;;
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
  else mj_info state "-" "no active task"; fi

  [ "$MJ_JSON" = 1 ] || printf 'watch: %s drift finding(s)\n' "$MJ_FAILS"
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_DRIFT"
}
# handover.sh provides mj_check_sections
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
