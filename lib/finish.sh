#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
# finish — evaluate the finish contract; refuse if unmet. --check evaluates without writing.
#
# The contract is a doctrine bundle, not a list in this file. Every line comes from
# the effective rule set (the rules whose enforced_by names finish), and the policy's
# verification.finish_requires selects which of them this repository applies. A refusal
# names the doctrines that caused it.
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"

MJ_FINISH_OUTCOME=""; MJ_FINISH_VERIFY=""; MJ_FINISH_NOTE=""
MJ_FINISH_VEXIT=""; MJ_FINISH_VSECS=""
export MJ_FINISH_OUTCOME

mj_cmd_finish() {
  local outcome="" verify="" check=0 note=""
  while [ $# -gt 0 ]; do case "$1" in
    --outcome) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--outcome needs a value"; outcome="$2"; shift 2 ;;
    --outcome=*) outcome="${1#--outcome=}"; shift ;;
    --verify-command) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--verify-command needs a command"; verify="$2"; shift 2 ;;
    --note) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--note needs a file"; note="$2"; shift 2 ;;
    --check) check=1; shift ;;
    --help|-h) cat <<H
usage: majordomus finish --outcome <completed|partial|blocked|no_match|failed> [--verify-command "<cmd>"] [--note <file>]
       majordomus finish --check
  evaluates every line of the finish contract, prints pass/fail for each, refuses (10) if any fails
  --verify-command  the project's own verification; its exit code, duration and command are recorded
  --note            a completion note (required sections as a handover); otherwise the newest handover for this task is used
  --check           evaluate the current task against scope and state without writing; exit 0 when no task is active
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "finish: unknown option $1" ;;
  esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  if ! mj_load_current; then
    [ "$check" = 1 ] && { mj_info finish "-" "no active task; nothing to enforce"; return 0; }
    mj_die "$MJ_EX_MISSING" "no active task ($(mj_rel "$MJ_STATE_DIR")/current.yaml); run: majordomus start"
  fi
  local id profile; id="$(mj_cur id)"; profile="$(mj_cur profile)"
  # another checkout's record: report it, enforce nothing, and never write to it
  if mj_task_is_foreign; then
    mj_info finish "$id" "belongs to $(mj_cur worktree), not this checkout; nothing enforced here" "cat $(mj_rel "$MJ_STATE_DIR")/current.yaml"
    [ "$check" = 1 ] && return 0
    mj_die "$MJ_EX_REFUSED" "task $id belongs to $(mj_cur worktree); finish it there"
  fi
  mj_load_profile "$profile" || mj_die "$MJ_EX_MISSING" "task references profile '$profile' which has no file"
  case "$(mj_cur outcome)" in
    active) ;;
    *) [ "$check" = 1 ] && { mj_info finish "$id" "already $(mj_cur outcome)"; return 0; }
       mj_die "$MJ_EX_REFUSED" "task $id is already $(mj_cur outcome); run majordomus start for new work" ;;
  esac

  if [ "$check" = 1 ]; then
    mj_run_task_checks
    [ "$MJ_JSON" = 1 ] || printf 'finish --check: %s failing\n' "$MJ_FAILS"
    [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
  fi
  case "$outcome" in completed|partial|blocked|no_match|failed) ;;
    "") mj_die "$MJ_EX_USAGE" "finish: --outcome is required (completed|partial|blocked|no_match|failed)" ;;
    *) mj_die "$MJ_EX_USAGE" "finish: unknown outcome '$outcome'" ;; esac

  MJ_FINISH_OUTCOME="$outcome"; MJ_FINISH_VERIFY="$verify"; MJ_FINISH_NOTE="$note"
  MJ_FINISH_VEXIT=""; MJ_FINISH_VSECS=""
  mj_doctrine_dispatch finish

  # The policy may name a requirement the registry does not define. That is a
  # configuration error, not a passing contract, so it is reported and it refuses.
  local r
  for r in $(mj_ylist "$MJ_POL_FLAT" verification.finish_requires); do
    mj_doctrine_for_policy_key "$r" >/dev/null || mj_fail contract "$r" "verification.finish_requires names '$r', which no doctrine defines" "majordomus doctrine list"
  done

  local unmet="$MJ_FAILS" contract="" e refused=""
  for e in $MJ_DOCTRINE_RESULTS; do
    contract="$contract\"${e%%:*}\":\"${e#*:}\","
    [ "${e#*:}" = fail ] && refused="$refused ${e%%:*}"
  done
  contract="{${contract%,}}"
  if [ "$unmet" -gt 0 ]; then
    if [ "$MJ_JSON" != 1 ]; then
      printf 'finish: refused, %s unmet\n' "$unmet"
      [ -n "$refused" ] && printf 'blocking doctrines:%s\n' "$(printf '%s' "$refused" | tr ' ' '\n' | sed '/^$/d' | sed 's/^/\n- /' | tr -d '\n' | sed 's/^/\n/')"
    fi
    exit "$MJ_EX_CONTRACT"
  fi

  local now; now="$(mj_now)"
  sed -e "s/^outcome: .*/outcome: $outcome/" -e "s/^checkpoint_at: .*/checkpoint_at: $now/" "$MJ_CUR" > "$MJ_CUR.mj-tmp" && mv "$MJ_CUR.mj-tmp" "$MJ_CUR"
  [ -n "$note" ] && { mkdir -p "$MJ_STATE_DIR/completed"; cp "$note" "$MJ_STATE_DIR/completed/$id.md"; }
  local vj=null; [ -n "$MJ_FINISH_VEXIT" ] && vj="{\"command\":\"$(mj_json_esc "$verify")\",\"exit\":$MJ_FINISH_VEXIT,\"seconds\":$MJ_FINISH_VSECS}"
  local cps=0; cps="$(find "$MJ_STATE_DIR/checkpoints" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  mj_ledger_append task.finished "\"task_id\":\"$id\",\"outcome\":\"$outcome\",\"contract\":$contract,\"verify\":$vj,\"checkpoints\":$cps"
  [ "$MJ_JSON" = 1 ] || printf 'finish: %s %s\n' "$id" "$outcome"
}

# the doctrine index whose policy_key is <key>, or failure
mj_doctrine_for_policy_key() {
  local i=0
  mj_doctrine_load
  while [ -n "$(mj_doc "$i" id)" ]; do [ "$(mj_doc "$i" policy_key)" = "$1" ] && { printf '%s' "$i"; return 0; }; i=$((i+1)); done
  return 1
}
# is the doctrine currently executing selected by this repository's policy?
# A doctrine with no policy_key is not selectable and always applies.
mj_finish_selected() {
  local i key
  i="$(mj_doc_index "$MJ_DOCTRINE_ID")" || return 0
  key="$(mj_doc "$i" policy_key)"; [ -n "$key" ] || return 0
  local r; for r in $(mj_ylist "$MJ_POL_FLAT" verification.finish_requires); do [ "$r" = "$key" ] && return 0; done
  return 1
}

# ---------------------------------------------------------------- finish-only validators
mj_validate_verification() {
  local id; id="$(mj_cur id)"
  mj_finish_selected || { mj_doctrine_skip verification "$id" "not in verification.finish_requires"; MJ_DOCTRINE_SKIPPED=1; return 0; }
  if [ "$MJ_FINISH_OUTCOME" != completed ]; then
    mj_doctrine_skip verification "$id" "skipped for outcome $MJ_FINISH_OUTCOME"; MJ_DOCTRINE_SKIPPED=1; return 0; fi
  local need; need="$(mj_pro verification.verify_command_required)"
  [ "$need" = if_files_changed ] && { [ "$MJ_TOUCHED_IN" -gt 0 ] && need=true || need=false; }
  if [ "$need" != true ]; then
    mj_doctrine_skip verification "$id" "not required by profile $(mj_cur profile)"; MJ_DOCTRINE_SKIPPED=1; return 0; fi
  if [ -z "$MJ_FINISH_VERIFY" ]; then
    mj_doctrine_fail verification "$id" "profile $(mj_cur profile) requires --verify-command" "majordomus finish --outcome completed --verify-command \"<cmd>\""; return 0; fi
  local t0 t1 vexit; t0="$(date +%s)"
  if ( cd "$MJ_ROOT" && sh -c "$MJ_FINISH_VERIFY" ) > /dev/null 2>&1; then vexit=0; else vexit=$?; fi
  t1="$(date +%s)"; MJ_FINISH_VEXIT="$vexit"; MJ_FINISH_VSECS=$((t1-t0))
  if [ "$vexit" = 0 ]; then mj_doctrine_ok verification "$id" "$MJ_FINISH_VERIFY — exit 0, ${MJ_FINISH_VSECS}s"
  else mj_doctrine_fail verification "$id" "$MJ_FINISH_VERIFY — exit $vexit, ${MJ_FINISH_VSECS}s" "$MJ_FINISH_VERIFY"; fi
  return 0
}

mj_validate_note() {
  local id nf="" need_sec miss; id="$(mj_cur id)"
  mj_finish_selected || { mj_doctrine_skip note "$id" "not in verification.finish_requires"; MJ_DOCTRINE_SKIPPED=1; return 0; }
  case "$MJ_FINISH_OUTCOME" in
    completed) need_sec="$(mj_ylist "$MJ_POL_FLAT" handover.required_sections | tr '\n' '|')" ;;
    partial|blocked) need_sec="Next Action|" ;;
    *) need_sec="Reason|" ;;
  esac
  if [ -n "$MJ_FINISH_NOTE" ]; then nf="$MJ_FINISH_NOTE"
  else nf="$(grep -l "^task_id: $id$" "$MJ_STATE_DIR"/handovers/*.md 2>/dev/null | sort | tail -n1)"; fi
  if [ -z "$nf" ] || [ ! -f "$nf" ]; then
    mj_doctrine_fail note "$id" "no --note file and no handover for this task" "majordomus handover < note.md"; return 0; fi
  miss="$(mj_check_sections "$nf" "$need_sec")"
  if [ -z "$miss" ]; then mj_doctrine_ok note "$id" "$(basename "$nf")"
  else mj_doctrine_fail note "$id" "$(basename "$nf") lacks section(s): $miss" "grep -n '^# ' $nf"; fi
  return 0
}

mj_validate_profile_requirements() {
  local id profile any=0; id="$(mj_cur id)"; profile="$(mj_cur profile)"
  if [ "$MJ_FINISH_OUTCOME" != completed ]; then
    mj_doctrine_skip regression "$id" "profile requirements apply to completed only"; MJ_DOCTRINE_SKIPPED=1; return 0; fi
  if [ "$(mj_pro verification.regression_test_required)" = true ]; then
    any=1
    if mj_git_touched "$(mj_cur head)" | grep -qiE '(^|/)(test|tests|spec|specs)(/|_|\.|$)|_test\.|\.test\.|_spec\.|\.spec\.'; then
      mj_doctrine_ok regression "$id" "a test path was touched"
    else mj_doctrine_fail regression "$id" "profile $profile requires a regression test; no test path among touched files" "git diff --name-only $(mj_cur head) HEAD; git status --porcelain"; fi
  fi
  if [ "$(mj_pro verification.decision_record_required)" = true ]; then
    any=1
    if grep -q "^Task: $id" "$MJ_STATE_DIR/decisions.md" 2>/dev/null; then mj_doctrine_ok decisions "$id" "decision record present"
    else mj_doctrine_fail decisions "$id" "profile $profile requires an entry 'Task: $id' in decisions.md" "grep -n 'Task:' $(mj_rel "$MJ_STATE_DIR")/decisions.md"; fi
  fi
  [ "$any" = 0 ] && { mj_doctrine_skip regression "$id" "profile $profile adds no requirement beyond the shared contract"; MJ_DOCTRINE_SKIPPED=1; }
  return 0
}

# Whatever stops short of completed must leave the next worker something to act on.
# note_present already requires the section; this requires that a task claiming to be
# handed over actually has a handover record. Advisory: the note's Next Action is a
# real continuation record, just a weaker one than a handover.
mj_validate_continuity() {
  local id; id="$(mj_cur id)"
  case "$MJ_FINISH_OUTCOME" in
    partial|blocked) ;;
    *) mj_doctrine_skip continuity "$id" "applies to partial and blocked"; MJ_DOCTRINE_SKIPPED=1; return 0 ;;
  esac
  if mj_resolve_latest "$MJ_STATE_DIR/handovers" "$id"; then
    mj_doctrine_ok continuity "$id" "handover $(basename "$MJ_RES_PATH") carries the next action"
  else
    mj_doctrine_fail continuity "$id" "no handover for this task; the note's Next Action is the only continuation record" "majordomus handover < note.md"
  fi
  return 0
}
