#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_LABEL, MJ_TOUCHED_IN, MJ_BLOCKED are read by finish.sh and watch.sh
# check — is the current task consistent with policy, scope, and state? Read-only,
# except --checkpoint which updates checkpoint_at (the one documented write).
#
# check runs no validator by name. It asks the doctrine dispatcher for everything the
# registry declares for this command, so a doctrine added to share/doctrines.yaml is
# enforced here without this file changing.
# shellcheck source=doctrine.sh
. "$MJ_LIB_DIR/doctrine.sh"
mj_cmd_check() {
  local explain=0 overlap=0 checkpoint=0 only=""
  while [ $# -gt 0 ]; do case "$1" in
    --explain) explain=1; shift ;; --overlap) overlap=1; shift ;; --checkpoint) checkpoint=1; shift ;;
    --rule) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--rule needs a doctrine id"; only="$2"; shift 2 ;;
    --rule=*) only="${1#--rule=}"; shift ;;
    --help|-h) cat <<H
usage: majordomus check [--explain] [--overlap] [--checkpoint] [--rule <id>] [--json]
  --explain     print the effective policy and profile for the current task, exit 0
  --overlap     print scope containment against other worktrees' active tasks, exit 0
  --checkpoint  record a checkpoint (updates checkpoint_at) after running the checks
  --rule <id>   run only this doctrine (see: majordomus doctrine list)
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
    printf '# doctrines enforced by check\n'; mj_doctrine_load
    local k=0; while [ -n "$(mj_doc "$k" id)" ]; do
      mj_doctrine_applies "$k" check && printf '  %s (%s)\n' "$(mj_doc "$k" id)" "$(mj_doc "$k" class)"; k=$((k+1)); done
    return 0
  fi
  if [ "$overlap" = 1 ]; then mj_report_overlap_from_current; return 0; fi

  if [ -n "$only" ]; then mj_doctrine_one "$only" check; else mj_run_task_checks; fi
  if [ "$checkpoint" = 1 ]; then
    local now; now="$(mj_now)"
    sed "s/^checkpoint_at: .*/checkpoint_at: $now/" "$MJ_CUR" > "$MJ_CUR.mj-tmp" && mv "$MJ_CUR.mj-tmp" "$MJ_CUR"
    mj_ledger_append task.checkpoint "\"task_id\":\"$id\""
    mj_info checkpoint "$id" "recorded at $now"
  fi
  [ "$MJ_JSON" = 1 ] || printf 'check: %s finding(s), %s failing\n' "$MJ_FINDINGS" "$MJ_FAILS"
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
}

# run one doctrine by id, refusing if it does not apply to this command
mj_doctrine_one() {
  local id="$1" cmd="$2" i fn
  mj_doctrine_load
  i="$(mj_doc_index "$id")" || mj_die "$MJ_EX_MISSING" "no doctrine '$id' (see: majordomus doctrine list)"
  mj_doctrine_applies "$i" "$cmd" || mj_die "$MJ_EX_USAGE" "doctrine '$id' is not enforced by $cmd (it is enforced by: $(mj_doc_list "$i" enforced_by | paste -sd, -))"
  fn="mj_validate_$(mj_doc "$i" validator)"
  mj_is_function "$fn" || mj_die "$MJ_EX_INTERNAL" "doctrine '$id' declares validator '$(mj_doc "$i" validator)' but no function $fn exists"
  MJ_DOCTRINE_ID="$id"; MJ_DOCTRINE_CLASS="$(mj_doc "$i" class)"
  "$fn"; MJ_DOCTRINE_ID=""; MJ_DOCTRINE_CLASS=""
}

# shared by check, watch and finish; expects current, policy, profile loaded.
# Sets MJ_LABEL, MJ_TOUCHED_IN, MJ_BLOCKED for the caller.
MJ_LABEL=""; MJ_TOUCHED_IN=0; MJ_BLOCKED=0
mj_run_task_checks() { mj_doctrine_dispatch check; }

# When the dispatcher is running for finish, a doctrine carrying a policy_key applies
# only if this repository selected it in verification.finish_requires. During check the
# policy list has no say: check reports, it does not accept work.
mj_finish_gate() {
  [ "$MJ_DOCTRINE_CMD" = finish ] || return 0
  mj_finish_selected && return 0
  mj_doctrine_skip "$1" "$(mj_cur id)" "not in verification.finish_requires"
  MJ_DOCTRINE_SKIPPED=1
  return 1
}

# A task-scoped doctrine has nothing to say when no task is active. watch dispatches
# with or without one; check and finish refuse earlier.
mj_task_gate() {
  [ -n "${MJ_CUR_FLAT:-}" ] && [ -f "${MJ_CUR_FLAT:-/nonexistent}" ] && return 0
  mj_doctrine_skip "$1" "-" "no active task"
  MJ_DOCTRINE_SKIPPED=1
  return 1
}

# ---------------------------------------------------------------- validators
# Each is the executable meaning of one doctrine. They report through
# mj_doctrine_fail, so the registry's class — not the function — decides whether the
# command stops. None of them is called by name from a command.

mj_validate_state() {
  mj_task_gate state || return 0
  mj_finish_gate state || return 0
  local id label head branch; id="$(mj_cur id)"; head="$(mj_cur head)"; branch="$(mj_cur branch)"
  label="$(mj_git_label "$head" "$branch")"
  case "$label" in
    exact|advanced) mj_doctrine_ok state "$id" "$label (head ${head:0:7})" ;;
    diverged) mj_doctrine_fail state "$id" "diverged from recorded head ${head:0:7}; history was rewritten or the task record is stale" "git merge-base --is-ancestor $head HEAD" ;;
    different_context) mj_doctrine_fail state "$id" "recorded on branch '$branch', now on '$(mj_git_branch)'" "git branch --show-current" ;;
  esac
  MJ_LABEL="$label"
}

mj_validate_scope() {
  mj_task_gate scope || return 0
  mj_finish_gate scope || return 0
  local id f inside n_out=0 n_in=0 s allow_gen
  id="$(mj_cur id)"; allow_gen="$(mj_projection_targets | tr '\n' ' ')"
  for f in $(mj_git_touched "$(mj_cur head)"); do
    case "$f" in .majordomus/*) continue ;; esac
    case " $allow_gen " in *" $f "*) continue ;; esac
    inside=0
    for s in $(mj_ylist "$MJ_CUR_FLAT" scope); do mj_path_contains "$s" "$f" && { inside=1; break; }; done
    if [ "$inside" = 1 ]; then n_in=$((n_in+1)); else n_out=$((n_out+1)); mj_doctrine_fail scope "$f" "outside claimed scope ($(mj_ylist "$MJ_CUR_FLAT" scope | paste -sd, -))" "git status --porcelain; git diff --name-only $(mj_cur head) HEAD"; fi
  done
  [ "$n_out" = 0 ] && mj_doctrine_ok scope "$id" "$n_in touched file(s), all within scope"
  MJ_TOUCHED_IN="$n_in"
  return 0
}

mj_validate_checkpoint() {
  mj_task_gate checkpoint || return 0
  local id cp interval now_e cp_e age; id="$(mj_cur id)"
  cp="$(mj_cur checkpoint_at)"; interval="$(mj_duration_secs "$(mj_pro checkpoint_interval)")" || interval=900
  now_e="$(mj_epoch "$(mj_now)")"; cp_e="$(mj_epoch "$cp")"
  [ -n "$cp_e" ] || { mj_doctrine_fail checkpoint "$id" "checkpoint_at '$cp' is not a timestamp this platform can read" "grep -n checkpoint_at .majordomus/state/current.yaml"; return 0; }
  age=$((now_e - cp_e))
  if [ "$age" -le "$interval" ]; then mj_doctrine_ok checkpoint "$id" "$((age/60))m ago, interval $(mj_pro checkpoint_interval)"
  else mj_doctrine_fail checkpoint "$id" "$((age/60))m ago, interval $(mj_pro checkpoint_interval) — run: majordomus check --checkpoint" "majordomus check --checkpoint"; fi
  return 0
}

mj_validate_blockers() {
  mj_task_gate blockers || return 0
  mj_finish_gate blockers || return 0
  local id q; id="$(mj_cur id)"; q="$MJ_DIR/state/open-questions.md"
  if [ "$MJ_DOCTRINE_CMD" = finish ] && [ "${MJ_FINISH_OUTCOME:-}" = blocked ]; then
    mj_doctrine_skip blockers "$(mj_cur id)" "outcome is blocked; open questions expected"; MJ_DOCTRINE_SKIPPED=1; return 0
  fi
  if [ -f "$q" ] && grep -qE "^\- \[unresolved\] $id " "$q"; then
    mj_doctrine_fail blockers "$id" "unresolved: $(grep -E "^\- \[unresolved\] $id " "$q" | head -n1 | sed "s/^- \[unresolved\] $id — //" | cut -c1-80)" "grep -n 'unresolved' .majordomus/state/open-questions.md"
    MJ_BLOCKED=1
  else mj_doctrine_ok blockers "$id" "none open"; MJ_BLOCKED=0; fi
  return 0
}

mj_projection_targets() { local j=0; while [ -n "$(mj_pol "projections.$j.target")" ]; do printf '%s\n' "$(mj_pol "projections.$j.target")"; j=$((j+1)); done; }
mj_report_overlap_from_current() {
  # shellcheck disable=SC2046
  mj_report_overlap "$(mj_ylist "$MJ_CUR_FLAT" scope | tr '\n' ' ')"
  [ "$MJ_FINDINGS" = 0 ] && mj_info overlap "$(mj_cur id)" "no other worktree claims an overlapping path"
}
# start.sh defines mj_report_overlap; check needs it too
# shellcheck source=start.sh
. "$MJ_LIB_DIR/start.sh"
