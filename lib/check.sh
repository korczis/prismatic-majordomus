#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_check:-}" ] && return 0 || MJ_LIB_check=1
# shellcheck disable=SC2034  # MJ_LABEL, MJ_TOUCHED_IN, MJ_BLOCKED are read by finish.sh and watch.sh
# check — is the current task consistent with policy, scope, and state? Read-only,
# except --checkpoint which updates checkpoint_at (the one documented write).
mj_cmd_check() {
  local explain=0 overlap=0 checkpoint=0
  while [ $# -gt 0 ]; do case "$1" in
    --explain) explain=1; shift ;; --overlap) overlap=1; shift ;; --checkpoint) checkpoint=1; shift ;;
    --help|-h) cat <<H
usage: majordomus check [--explain] [--overlap] [--checkpoint] [--json]
  --explain     print the effective policy and profile for the current task, exit 0
  --overlap     print scope containment against other worktrees' active tasks, exit 0
  --checkpoint  record a checkpoint (updates checkpoint_at) after running the checks
  exit 0 with no FAIL findings, 10 with FAIL findings, 12 with no active task
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "check: unknown option $1" ;;
  esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task (.majordomus/state/current.yaml); run: majordomus start"
  local id profile; id="$(mj_cur id)"; profile="$(mj_cur profile)"
  mj_load_profile "$profile" || mj_die "$MJ_EX_MISSING" "task references profile '$profile' which has no file"

  if [ "$explain" = 1 ]; then
    printf '# task\n'; sed 's/^/  /' "$MJ_CUR_FLAT"
    printf '# profile %s\n' "$profile"; sed 's/^/  /' "$MJ_PRO_FLAT"
    printf '# policy\n'; grep -E '^(version|context|profiles|verification|handover)' "$MJ_POL_FLAT" | sed 's/^/  /'
    return 0
  fi
  if [ "$overlap" = 1 ]; then mj_report_overlap_from_current; return 0; fi

  mj_run_task_checks
  if [ "$checkpoint" = 1 ]; then
    local now; now="$(mj_now)"
    sed "s/^checkpoint_at: .*/checkpoint_at: $now/" "$MJ_CUR" > "$MJ_CUR.mj-tmp" && mv "$MJ_CUR.mj-tmp" "$MJ_CUR"
    mj_ledger_append task.checkpoint "\"task_id\":\"$id\""
    mj_info checkpoint "$id" "recorded at $now"
  fi
  [ "$MJ_JSON" = 1 ] || printf 'check: %s finding(s), %s failing\n' "$MJ_FINDINGS" "$MJ_FAILS"
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
}

# shared by check, watch and finish; expects current, policy, profile loaded.
# Sets MJ_LABEL, MJ_TOUCHED_IN, MJ_BLOCKED for the caller.
MJ_LABEL=""; MJ_TOUCHED_IN=0; MJ_BLOCKED=0
mj_run_task_checks() {
  local id label head branch; id="$(mj_cur id)"; head="$(mj_cur head)"; branch="$(mj_cur branch)"
  label="$(mj_git_label "$head" "$branch")"
  case "$label" in
    exact|advanced) mj_ok state "$id" "$label (head ${head:0:7})" ;;
    diverged) mj_fail state "$id" "diverged from recorded head ${head:0:7}; history was rewritten or the task record is stale" "git merge-base --is-ancestor $head HEAD" ;;
    different_context) mj_fail state "$id" "recorded on branch '$branch', now on '$(mj_git_branch)'" "git branch --show-current" ;;
  esac
  MJ_LABEL="$label"

  # scope: touched files must lie within claimed paths (state and projections are always allowed)
  local f inside n_out=0 n_in=0 s
  local allow_gen; allow_gen="$(mj_projection_targets | tr '\n' ' ')"
  for f in $(mj_git_touched "$head"); do
    case "$f" in .majordomus/*) continue ;; esac
    case " $allow_gen " in *" $f "*) continue ;; esac
    inside=0
    for s in $(mj_ylist "$MJ_CUR_FLAT" scope); do mj_path_contains "$s" "$f" && { inside=1; break; }; done
    if [ "$inside" = 1 ]; then n_in=$((n_in+1)); else n_out=$((n_out+1)); mj_fail scope "$f" "outside claimed scope ($(mj_ylist "$MJ_CUR_FLAT" scope | paste -sd, -))" "git status --porcelain; git diff --name-only $head HEAD"; fi
  done
  [ "$n_out" = 0 ] && mj_ok scope "$id" "$n_in touched file(s), all within scope"
  MJ_TOUCHED_IN="$n_in"

  # checkpoint age
  local cp interval now_e cp_e age
  cp="$(mj_cur checkpoint_at)"; interval="$(mj_duration_secs "$(mj_pro checkpoint_interval)")" || interval=900
  now_e="$(mj_epoch "$(mj_now)")"; cp_e="$(mj_epoch "$cp")"
  if [ -n "$cp_e" ]; then age=$((now_e - cp_e))
    if [ "$age" -le "$interval" ]; then mj_ok checkpoint "$id" "$((age/60))m ago, interval $(mj_pro checkpoint_interval)"
    else mj_warn checkpoint "$id" "$((age/60))m ago, interval $(mj_pro checkpoint_interval) — run: majordomus check --checkpoint" "majordomus check --checkpoint"; fi
  fi

  # blockers, and the integrity of the two stores the gates read
  local q="$MJ_DIR/state/open-questions.md" bad
  MJ_BLOCKED=0
  bad="$(mj_question_malformed "$q")"
  if [ -n "$bad" ]; then
    mj_fail blockers "open-questions.md" "line(s) $(printf '%s' "$bad" | sed 's/ $//') do not parse; an unreadable entry cannot block acceptance" "majordomus question list --all"
    MJ_BLOCKED=1
  elif grep -qE "^\- \[unresolved\] $id " "$q" 2>/dev/null; then
    mj_fail blockers "$id" "unresolved: $(grep -E "^\- \[unresolved\] $id " "$q" | head -n1 | sed "s/^- \[unresolved\] $id — //" | cut -c1-80)" "majordomus question list"
    MJ_BLOCKED=1
  else mj_ok blockers "$id" "none open"; fi
  # decisions.md is hand-editable by design, so an unattributable entry is reported rather
  # than blocking; finish still refuses when a profile requires a record it cannot find.
  bad="$(mj_decision_malformed "$MJ_DIR/state/decisions.md")"
  [ -n "$bad" ] && mj_warn records "decisions.md" "entry at line(s) $(printf '%s' "$bad" | sed 's/ $//') lacks Task, Head or Why; nothing will find it" "majordomus decision list"
  bad="$(mj_ledger_bad_lines "$MJ_DIR/state/ledger.jsonl")"
  [ -n "$bad" ] && mj_fail records "ledger.jsonl" "line(s) $(printf '%s' "$bad" | sed 's/ $//') are not well-formed events" "majordomus history --validate"

  # continuity: where the next worker would resume from
  if mj_resolve_latest "$MJ_DIR/state/checkpoints" "$id"; then
    mj_info checkpoint "${MJ_RES_PATH#"$MJ_ROOT/"}" "newest record, $(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" "majordomus checkpoint --show"
  fi
}
mj_projection_targets() { local j=0; while [ -n "$(mj_pol "projections.$j.target")" ]; do printf '%s\n' "$(mj_pol "projections.$j.target")"; j=$((j+1)); done; }
mj_report_overlap_from_current() {
  # shellcheck disable=SC2046
  mj_report_overlap "$(mj_ylist "$MJ_CUR_FLAT" scope | tr '\n' ' ')"
  [ "$MJ_FINDINGS" = 0 ] && mj_info overlap "$(mj_cur id)" "no other worktree claims an overlapping path"
}
# start.sh defines mj_report_overlap; question.sh and decision.sh define the store validators
# shellcheck source=start.sh
. "$MJ_LIB_DIR/start.sh"
# shellcheck source=question.sh
. "$MJ_LIB_DIR/question.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
