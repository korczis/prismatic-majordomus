#!/usr/bin/env bash
# finish — evaluate the finish contract; refuse if unmet. --check evaluates without writing.
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
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
    mj_die "$MJ_EX_MISSING" "no active task (.majordomus/state/current.yaml); run: majordomus start"
  fi
  local id profile; id="$(mj_cur id)"; profile="$(mj_cur profile)"
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

  # run the shared checks quietly into counters, then print the contract explicitly
  local saved_json="$MJ_JSON"; MJ_JSON=1; mj_run_task_checks > /dev/null; MJ_JSON="$saved_json"
  local scope_ok=1 state_ok=1
  # shellcheck disable=SC2034
  MJ_FINDINGS=0; MJ_FAILS=0
  case "$MJ_LABEL" in exact|advanced) ;; *) state_ok=0 ;; esac
  local f s inside; for f in $(mj_git_touched "$(mj_cur head)"); do
    case "$f" in .majordomus/*) continue ;; esac; case " $(mj_projection_targets | tr '\n' ' ') " in *" $f "*) continue ;; esac
    inside=0; for s in $(mj_ylist "$MJ_CUR_FLAT" scope); do mj_path_contains "$s" "$f" && { inside=1; break; }; done
    [ "$inside" = 1 ] || scope_ok=0
  done

  local unmet=0 contract="" r vexit="" vsecs="" line
  for r in $(mj_ylist "$MJ_POL_FLAT" verification.finish_requires); do
    case "$r" in
      scope_respected)
        if [ "$scope_ok" = 1 ]; then mj_ok scope "$id" "$MJ_TOUCHED_IN touched file(s), all within scope"; line=pass
        else mj_fail scope "$id" "touched files outside claimed scope" "majordomus check"; line=fail; fi ;;
      verification_ran)
        if [ "$outcome" != completed ]; then mj_info verification "$id" "skipped for outcome $outcome"; line=skipped
        else
          local need; need="$(mj_pro verification.verify_command_required)"
          [ "$need" = if_files_changed ] && { [ "$MJ_TOUCHED_IN" -gt 0 ] && need=true || need=false; }
          if [ "$need" != true ]; then mj_info verification "$id" "not required by profile $profile"; line=skipped
          elif [ -z "$verify" ]; then mj_fail verification "$id" "profile $profile requires --verify-command" "majordomus finish --outcome completed --verify-command \"<cmd>\""; line=fail
          else
            local t0 t1; t0="$(date +%s)"
            if ( cd "$MJ_ROOT" && sh -c "$verify" ) > /dev/null 2>&1; then vexit=0; else vexit=$?; fi
            t1="$(date +%s)"; vsecs=$((t1-t0))
            if [ "$vexit" = 0 ]; then mj_ok verification "$id" "$verify — exit 0, ${vsecs}s"; line=pass
            else mj_fail verification "$id" "$verify — exit $vexit, ${vsecs}s" "$verify"; line=fail; fi
          fi
        fi ;;
      state_updated)
        if [ "$state_ok" = 1 ]; then mj_ok state "$id" "$MJ_LABEL (head $(mj_git_head | cut -c1-7))"; line=pass
        else mj_fail state "$id" "$MJ_LABEL — the task record no longer describes this checkout" "majordomus check"; line=fail; fi ;;
      no_open_blockers)
        if [ "$outcome" = blocked ]; then mj_info blockers "$id" "outcome is blocked; open questions expected"; line=skipped
        elif [ "$MJ_BLOCKED" = 0 ]; then mj_ok blockers "$id" "none open"; line=pass
        else mj_fail blockers "$id" "unresolved entry in open-questions.md" "grep -n 'unresolved' .majordomus/state/open-questions.md"; line=fail; fi ;;
      note_present)
        local nf="" need_sec
        case "$outcome" in completed) need_sec="$(mj_ylist "$MJ_POL_FLAT" handover.required_sections | tr '\n' '|')" ;;
          partial|blocked) need_sec="Next Action|" ;; *) need_sec="Reason|" ;; esac
        if [ -n "$note" ]; then nf="$note"
        else nf="$(grep -l "^task_id: $id$" "$MJ_DIR"/state/handovers/*.md 2>/dev/null | sort | tail -n1)"; fi
        if [ -z "$nf" ] || [ ! -f "$nf" ]; then mj_fail note "$id" "no --note file and no handover for this task" "majordomus handover < note.md"; line=fail
        else local miss; miss="$(mj_check_sections "$nf" "$need_sec")"
          if [ -z "$miss" ]; then mj_ok note "$id" "$(basename "$nf")"; line=pass
          else mj_fail note "$id" "$(basename "$nf") lacks section(s): $miss" "grep -n '^# ' $nf"; line=fail; fi
        fi ;;
      *) mj_warn contract "$r" "unknown finish requirement; ignored"; line=skipped ;;
    esac
    [ "$line" = fail ] && unmet=$((unmet+1)); contract="$contract\"$r\":\"$line\","
  done
  # profile-level requirements, only for completed
  if [ "$outcome" = completed ]; then
    if [ "$(mj_pro verification.regression_test_required)" = true ]; then
      if mj_git_touched "$(mj_cur head)" | grep -qiE '(^|/)(test|tests|spec|specs)(/|_|\.|$)|_test\.|\.test\.|_spec\.|\.spec\.'; then mj_ok regression "$id" "a test path was touched"
      else mj_fail regression "$id" "profile $profile requires a regression test; no test path among touched files" "git diff --name-only $(mj_cur head) HEAD; git status --porcelain"; unmet=$((unmet+1)); fi
      contract="$contract\"regression_test\":\"$([ "$unmet" = 0 ] && echo pass || echo fail)\","
    fi
    if [ "$(mj_pro verification.decision_record_required)" = true ]; then
      if grep -q "^Task: $id" "$MJ_DIR/state/decisions.md" 2>/dev/null; then mj_ok decisions "$id" "decision record present"
      else mj_fail decisions "$id" "profile $profile requires an entry 'Task: $id' in decisions.md" "grep -n 'Task:' .majordomus/state/decisions.md"; unmet=$((unmet+1)); fi
    fi
  fi
  contract="{${contract%,}}"
  if [ "$unmet" -gt 0 ]; then
    [ "$MJ_JSON" = 1 ] || printf 'finish: refused, %s unmet\n' "$unmet"; exit "$MJ_EX_CONTRACT"; fi

  local now; now="$(mj_now)"
  sed -e "s/^outcome: .*/outcome: $outcome/" -e "s/^checkpoint_at: .*/checkpoint_at: $now/" "$MJ_CUR" > "$MJ_CUR.mj-tmp" && mv "$MJ_CUR.mj-tmp" "$MJ_CUR"
  [ -n "$note" ] && { mkdir -p "$MJ_DIR/state/completed"; cp "$note" "$MJ_DIR/state/completed/$id.md"; }
  local vj=null; [ -n "$vexit" ] && vj="{\"command\":\"$(mj_json_esc "$verify")\",\"exit\":$vexit,\"seconds\":$vsecs}"
  mj_ledger_append task.finished "\"task_id\":\"$id\",\"outcome\":\"$outcome\",\"contract\":$contract,\"verify\":$vj"
  [ "$MJ_JSON" = 1 ] || printf 'finish: %s %s\n' "$id" "$outcome"
}
