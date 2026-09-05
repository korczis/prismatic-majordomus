#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_LABEL, MJ_TOUCHED_IN, MJ_BLOCKED are read by finish.sh and watch.sh
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_check:-}" ] && return 0 || MJ_LIB_check=1
# check — is the current task consistent with policy, scope, and state? Read-only,
# except --checkpoint which updates checkpoint_at (the one documented write).
#
# check runs no validator by name. It asks the doctrine dispatcher for everything the
# effective rule set declares for this command, so a rule added to the package is
# enforced here without this file changing, and a check that exists here without a
# rule declaring it is a failure doctor reports.
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
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task ($(mj_rel "$MJ_STATE_DIR")/current.yaml); run: majordomus start"
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
  MJ_DOCTRINE_ID="$id"; MJ_DOCTRINE_CLASS="$(mj_doc "$i" class)"; MJ_DOCTRINE_CMD="$cmd"
  "$fn"; MJ_DOCTRINE_ID=""; MJ_DOCTRINE_CLASS=""; MJ_DOCTRINE_CMD=""
}

# shared by check, watch and finish; expects current, policy, profile loaded.
# Sets MJ_LABEL, MJ_TOUCHED_IN, MJ_BLOCKED for the caller.
MJ_LABEL=""; MJ_TOUCHED_IN=0; MJ_BLOCKED=0; MJ_FOREIGN=0

# Is the loaded task record about this checkout? The record is tracked, so it travels with
# the branch: another worktree on the same branch reads it and would otherwise be held to a
# scope it never claimed. "One active task per checkout" is checked at start; this is the
# same fact represented in the record so that every reader can apply it.
# A record written before `worktree` existed has no opinion, and is treated as local.
mj_task_is_foreign() {
  local w; w="$(mj_cur worktree)"
  [ -n "$w" ] || return 1
  [ "$w" = "$MJ_ROOT" ] && return 1
  return 0
}

# Foreign-ness is decided once, before any doctrine runs, because it is not a rule about
# the work — it is the question of whether these rules address this checkout at all.
mj_run_task_checks() {
  if mj_task_is_foreign; then
    MJ_FOREIGN=1; MJ_LABEL=exact; MJ_TOUCHED_IN=0; MJ_BLOCKED=0
    mj_info task "$(mj_cur id)" "belongs to $(mj_cur worktree), not this checkout; nothing enforced here" "cat $(mj_rel "$MJ_STATE_DIR")/current.yaml"
    return 0
  fi
  MJ_FOREIGN=0
  mj_doctrine_dispatch check
}

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
  if [ -n "${MJ_CUR_FLAT:-}" ] && [ -f "${MJ_CUR_FLAT:-/nonexistent}" ]; then
    mj_task_is_foreign || return 0
    mj_doctrine_skip "$1" "$(mj_cur id)" "task belongs to $(mj_cur worktree), not this checkout"
    MJ_DOCTRINE_SKIPPED=1
    return 1
  fi
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
  return 0
}

mj_validate_scope() {
  mj_task_gate scope || return 0
  mj_finish_gate scope || return 0
  local id f inside n_out=0 n_in=0 s allow_gen
  id="$(mj_cur id)"; allow_gen="$(mj_projection_targets | tr '\n' ' ')"
  for f in $(mj_git_touched "$(mj_cur head)"); do
    mj_is_ai_path "$f" && continue
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
  # Freshness is a statement about work in progress. A finished task has no next checkpoint
  # due, so reporting one as overdue is a finding nobody can act on — the reproduce command
  # would record progress on work that is done — and under watch it is drift that never
  # clears, which trains a reader to skip the drift report.
  case "$(mj_cur outcome)" in
    active) ;;
    *) mj_doctrine_skip checkpoint "$(mj_cur id)" "outcome is $(mj_cur outcome); freshness applies while a task is active"
       MJ_DOCTRINE_SKIPPED=1; return 0 ;;
  esac
  local id cp interval now_e cp_e age; id="$(mj_cur id)"
  cp="$(mj_cur checkpoint_at)"; interval="$(mj_duration_secs "$(mj_pro checkpoint_interval)")" || interval=900
  now_e="$(mj_epoch "$(mj_now)")"; cp_e="$(mj_epoch "$cp")"
  if [ -z "$cp_e" ]; then
    mj_doctrine_fail checkpoint "$id" "checkpoint_at '$cp' is missing or is not a timestamp this platform can read" "grep -n checkpoint_at $(mj_rel "$MJ_STATE_DIR")/current.yaml"
  else
    age=$((now_e - cp_e))
    if [ "$age" -le "$interval" ]; then mj_doctrine_ok checkpoint "$id" "$((age/60))m ago, interval $(mj_pro checkpoint_interval)"
    else mj_doctrine_fail checkpoint "$id" "$((age/60))m ago, interval $(mj_pro checkpoint_interval) — run: majordomus check --checkpoint" "majordomus check --checkpoint"; fi
  fi
  # where the next worker would resume from; informational, never a finding
  if mj_resolve_latest "$MJ_STATE_DIR/checkpoints" "$id"; then
    mj_info checkpoint "${MJ_RES_PATH#"$MJ_ROOT/"}" "newest record, $(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" "majordomus checkpoint --show"
  fi
  return 0
}

# The blocker gate and the store it reads are one rule: a gate that cannot parse an entry
# can be bypassed by mistyping one, so an unreadable question blocks exactly as an
# unresolved question does.
mj_validate_blockers() {
  mj_task_gate blockers || return 0
  mj_finish_gate blockers || return 0
  local id q bad; id="$(mj_cur id)"; q="$MJ_STATE_DIR/open-questions.md"
  MJ_BLOCKED=0
  bad="$(mj_question_malformed "$q")"
  if [ -n "$bad" ]; then
    mj_doctrine_fail blockers "open-questions.md" "line(s) $(printf '%s' "$bad" | sed 's/ $//') do not parse; an unreadable entry cannot block acceptance" "majordomus question list --all"
    MJ_BLOCKED=1; return 0
  fi
  # Every unresolved question refuses a completed finish, not only one this task opened.
  # A question is a person owing an answer; the work it blocks outlives the task that
  # asked, and a gate that forgets at the task boundary is a gate a handover walks past.
  # The store is tracked, so git scopes this to the branch without anything being stored.
  local open n first
  open="$(mj_question_unresolved_any "$q")"
  # Only a *completed* finish is refused. `blocked`, `partial`, `no_match` and `failed` are
  # honest statements that the work did not complete, and refusing them would leave a worker
  # with an open question able to record nothing but `blocked` — which buys a green gate by
  # forcing a mislabelled outcome.
  if [ "$MJ_DOCTRINE_CMD" = finish ] && [ -n "${MJ_FINISH_OUTCOME:-}" ] && [ "$MJ_FINISH_OUTCOME" != completed ]; then
    [ -n "$open" ] && MJ_BLOCKED=1
    mj_doctrine_skip blockers "$id" "outcome is $MJ_FINISH_OUTCOME, not completed; open questions do not refuse it"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi
  if [ -n "$open" ]; then
    n="$(printf '%s\n' "$open" | wc -l | tr -d ' ')"
    first="$(printf '%s\n' "$open" | head -n1 | sed -e 's/^[0-9]*:- \[unresolved\] //' -e 's/ ([0-9-]*)$//')"
    mj_doctrine_fail blockers "$id" "$n unresolved question(s) on this branch; first: $(printf '%s' "$first" | cut -c1-80)" "majordomus question list"
    MJ_BLOCKED=1
  else mj_doctrine_ok blockers "$id" "none open"; fi
  return 0
}

# The ledger is the one durable record nothing else can reconstruct, so a malformed line
# blocks. decisions.md is hand-editable by design, so an unattributable entry is reported
# and does not block — that difference is the two classes, declared in the registry.
mj_validate_ledger() {
  local bad; bad="$(mj_ledger_bad_lines "$MJ_STATE_DIR/ledger.jsonl")"
  if [ -n "$bad" ]; then mj_doctrine_fail records "ledger.jsonl" "line(s) $(printf '%s' "$bad" | sed 's/ $//') are not well-formed events" "majordomus history --validate"
  else mj_doctrine_ok records "ledger.jsonl" "every line is a well-formed event"; fi
  return 0
}

mj_validate_decisions() {
  local bad; bad="$(mj_decision_malformed "$MJ_STATE_DIR/decisions.md")"
  if [ -n "$bad" ]; then mj_doctrine_fail records "decisions.md" "entry at line(s) $(printf '%s' "$bad" | sed 's/ $//') lacks Task, Head or Why; nothing will find it" "majordomus decision list"
  else mj_doctrine_ok records "decisions.md" "every entry is attributable"; fi
  return 0
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
# the use-case coverage doctrine, gated by the policy; commands.sh for the registry it reads
# shellcheck source=commands.sh
. "$MJ_LIB_DIR/commands.sh"
# shellcheck source=usecase.sh
. "$MJ_LIB_DIR/usecase.sh"
