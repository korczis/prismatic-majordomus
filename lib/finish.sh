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
MJ_FINISH_VEXIT=""; MJ_FINISH_VSECS=""; MJ_FINISH_VTREE=""
export MJ_FINISH_OUTCOME

# The pre-push judgement (ADR 0105, amendment of 2026-10-15; I1990). git hands a pre-push
# hook one line per ref it pushes — `<local ref> <local sha> <remote ref> <remote sha>` — and
# the hook passes them here. Each pushed branch is judged by the task of the checkout that has
# it checked out, which is its canonical worktree; the current checkout's task is asked of its
# own branch only. Before this, every push asked the pushing checkout's task, so pushing
# branch X from a checkout whose task is on Y failed with `different_context`, and pushing
# from the wrong checkout was the way round it. A branch no checkout holds, or whose checkout
# has no active task, is a push without a task, as it always was: admitted, with a line that
# names it. refs/majordomus/* is the continuity store and a deleted ref carries nothing; both
# are left out. $1 is a file holding the lines, or - for standard input.
mj_finish_check_refs() {
  local src="$1" refs wts lref lsha rref branch holder state rec fails=0 refused="" judged=0 one
  refs="$(cat -- "$src")" || mj_die "$MJ_EX_USAGE" "finish --check --refs: cannot read $src"
  wts="$(mj_git worktree list --porcelain)"
  state="$(mj_rel "$MJ_STATE_DIR")"
  while read -r lref lsha rref _; do
    [ -n "$lref" ] || continue
    case "$rref" in refs/majordomus/*) continue ;; esac
    case "$lsha" in *[!0]*) ;; *) continue ;; esac
    case "$lref" in
      refs/heads/*) branch="${lref#refs/heads/}" ;;
      *) mj_info finish "$lref" "not a branch: no task judges it; admitted"; continue ;;
    esac
    judged=$((judged + 1))
    holder="$(printf '%s\n' "$wts" | awk -v want="branch refs/heads/$branch" \
      'index($0, "worktree ") == 1 { w = substr($0, 10) } $0 == want { print w; exit }')"
    if [ -z "$holder" ]; then
      mj_info finish "$branch" "no checkout holds this branch: pushed without a task; admitted"
      continue
    fi
    rec="$holder/$state/current.yaml"
    if [ ! -f "$rec" ] || ! grep -q '^outcome: active$' "$rec"; then
      mj_info finish "$branch" "its checkout $holder has no active task: pushed without a task; admitted"
      continue
    fi
    mj_info finish "$branch" "judged by the task of its checkout $holder"
    one=0
    if [ "$MJ_JSON" = 1 ]; then "$MJ_SELF" --repo "$holder" --json finish --check || one=$?
    else "$MJ_SELF" --repo "$holder" finish --check || one=$?; fi
    [ "$one" = 0 ] || { fails=$((fails + 1)); refused="$refused $branch"; }
  done <<<"$refs"
  [ "$MJ_JSON" = 1 ] || printf 'finish --check: %s pushed branch(es) judged, %s refused%s\n' \
    "$judged" "$fails" "${refused:+:$refused}"
  [ "$fails" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
}

mj_cmd_finish() {
  local outcome="" verify="" check=0 note="" refs=""
  while [ $# -gt 0 ]; do case "$1" in
    --outcome) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--outcome needs a value"; outcome="$2"; shift 2 ;;
    --outcome=*) outcome="${1#--outcome=}"; shift ;;
    --verify-command) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--verify-command needs a command"; verify="$2"; shift 2 ;;
    --note) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--note needs a file"; note="$2"; shift 2 ;;
    --check) check=1; shift ;;
    --refs) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--refs needs a file, or - for standard input"; refs="$2"; shift 2 ;;
    --help|-h) cat <<H
usage: majordomus finish --outcome <completed|partial|blocked|no_match|failed> [--verify-command "<cmd>"] [--note <file>]
       majordomus finish --check [--refs <file>|-]
  evaluates every line of the finish contract, prints pass/fail for each, refuses (10) if any fails
  --verify-command  the project's own verification; its command, exit code, duration and the
                    tree it ran over are recorded, and a tree that changed under it is not a pass.
                    A command that cannot verify anything (true, :, exit 0, only echo) is refused
  --note            a completion note (required sections as a handover); otherwise the newest handover for this task is used
  --check           evaluate the current task against scope and state without writing; exit 0 when no task is active
  --refs            with --check: the refs a push carries, as git hands them to a pre-push hook
                    (<local ref> <local sha> <remote ref> <remote sha>, one per line); each pushed
                    branch is judged by the task of the checkout holding it, and one no checkout
                    holds is admitted with a line naming it
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "finish: unknown option $1" ;;
  esac; done
  [ -z "$refs" ] || [ "$check" = 1 ] || mj_die "$MJ_EX_USAGE" "finish: --refs is a question for --check"
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  [ -z "$refs" ] || mj_finish_check_refs "$refs"
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

  # The note is read once, here, and every later reader takes the copy. `--note` may be a pipe,
  # /dev/stdin or a process substitution: the doctrine's read used to drain it, and the record's
  # `cp` then failed (macOS refuses to fcopyfile /dev/stdin) after the outcome was written, so
  # the task stood completed with an empty note and no task.finished line. A note that cannot
  # be read is left as given, and the note doctrine refuses it as before (a recorded refusal).
  if [ -n "$note" ] && [ -r "$note" ] && [ ! -d "$note" ]; then
    local staged="$MJ_STATE_DIR/completed/$id.md.pending"
    mkdir -p "$MJ_STATE_DIR/completed" && cat "$note" > "$staged" && note="$staged"
  fi
  MJ_FINISH_OUTCOME="$outcome"; MJ_FINISH_VERIFY="$verify"; MJ_FINISH_NOTE="$note"
  MJ_FINISH_VEXIT=""; MJ_FINISH_VSECS=""; MJ_FINISH_VTREE=""
  # A scope that `start` could not claim — no server answered then — is claimed now, before
  # the contract is judged (I2289): a server that came up since is all the task was missing.
  if ! grep -q '^mesh_claim: ' "$MJ_CUR" 2>/dev/null && mj_mesh_declared_enabled; then
    mj_mesh_task_claim "$id" "$(mj_ylist "$MJ_CUR_FLAT" scope | tr '\n' ' ')" "$(mj_cur task)" "$(mj_cur issue)"
    mj_load_current || true
  fi
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
    # A refusal is a record. Until it was one, a task refused four times before it passed
    # left the same ledger as a task that passed first try, so the one claim this command
    # exists to make falsifiable — "done" — left no trace of every time it was false. The
    # doctrines that refused are named, because a count nobody can attribute is not evidence.
    # `finish --check` returned above and writes nothing: a check is a question, not a claim.
    local d refused_json=""
    for d in $refused; do refused_json="$refused_json\"$d\","; done
    mj_ledger_append task.refused "\"task_id\":\"$id\",\"outcome\":\"$outcome\",\"unmet\":$unmet,\"refused\":[${refused_json%,}],\"contract\":$contract"
    if [ "$MJ_JSON" != 1 ]; then
      printf 'finish: refused, %s unmet\n' "$unmet"
      # one doctrine per line: the list used to be joined with its own separators deleted.
      # $refused is a space-separated list of doctrine ids, split here on purpose.
      # shellcheck disable=SC2086
      [ -n "$refused" ] && { printf 'blocking doctrines:\n'; printf '%s\n' $refused | sed 's/^/- /'; }
    fi
    exit "$MJ_EX_CONTRACT"
  fi

  # the note is in place before the outcome says so: nothing records completion it cannot keep
  # (the staged copy is moved; a note that could not be staged is copied, never moved away)
  if [ -n "$note" ]; then
    local kept="$MJ_STATE_DIR/completed/$id.md"
    if [ "$note" = "$kept.pending" ]; then mv "$note" "$kept"
    else mkdir -p "$MJ_STATE_DIR/completed" && cat "$note" > "$kept"; fi \
      || mj_die "$MJ_EX_INTERNAL" "finish: the note could not be recorded; task $id stays active"
  fi
  local now; now="$(mj_now)"
  sed -e "s/^outcome: .*/outcome: $outcome/" -e "s/^checkpoint_at: .*/checkpoint_at: $now/" "$MJ_CUR" > "$MJ_CUR.mj-tmp" && mv "$MJ_CUR.mj-tmp" "$MJ_CUR"
  local vj=null; [ -n "$MJ_FINISH_VEXIT" ] && vj="{\"command\":\"$(mj_json_esc "$verify")\",\"exit\":$MJ_FINISH_VEXIT,\"seconds\":$MJ_FINISH_VSECS${MJ_FINISH_VTREE:+,\"tree\":\"$MJ_FINISH_VTREE\"}}"
  local cps=0; cps="$(find "$MJ_STATE_DIR/checkpoints" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  mj_ledger_append task.finished "\"task_id\":\"$id\",\"outcome\":\"$outcome\",\"contract\":$contract,\"verify\":$vj,\"checkpoints\":$cps"
  mj_mesh_task_release "$id"
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
# Is <cmd> a verification command that cannot verify anything? The rule is that a completed
# outcome is verified, not described, and `--verify-command true` satisfied it: the suite
# asserted so, and the ledger recorded `verify:{command:"true",exit:0}` as the proof. A command
# whose every part is a no-op builtin (true, :, exit 0, return 0) or only prints (echo,
# printf) exits 0 whatever the state of the work, so it is refused before it runs. This does
# not judge whether a real command is the right one — only that it is a command at all.
# Separators ; && || and newlines split the parts; a comment-only or empty part is a no-op.
mj_verify_is_vacuous() {
  local part
  printf '%s\n' "$1" | awk '{ gsub(/&&|\|\||;/, "\n"); print }' | {
    while IFS= read -r part || [ -n "$part" ]; do
      part="$(printf '%s' "$part" | sed -e 's/^[[:space:](){}]*//' -e 's/[[:space:](){}]*$//')"
      case "$part" in
        ''|'#'*|true|:|/bin/true|/usr/bin/true|exit|'exit 0'|return|'return 0') ;;
        echo|'echo '*|printf|'printf '*) ;;
        *) exit 1 ;;
      esac
    done
    exit 0
  }
}

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
  if mj_verify_is_vacuous "$MJ_FINISH_VERIFY"; then
    mj_doctrine_fail verification "$id" "\"$MJ_FINISH_VERIFY\" cannot verify anything: it is only a no-op or a print, so its exit 0 proves nothing (majordomus.verification-integrity)" "majordomus finish --outcome completed --verify-command \"<a test or check of this project, e.g. make test>\""; return 0; fi
  # The tree is read on both sides of the run. An exit code alone says that some command
  # once exited zero; it does not say over what, and a checkout with more than one worker
  # in it — a second agent, a person saving a file, a watcher regenerating an artefact —
  # moves under a long verification routinely. A run whose subject changed proved nothing
  # about the tree it started on and nothing about the tree it left, so it is not a pass,
  # and the tree that a run did prove is recorded with it.
  local t0 t1 vexit before after; before="$(mj_worktree_tree_id)"; t0="$(date +%s)"
  # The verify command is the project's own verification, run as a worker would run it. The
  # outcome this finish claims is its own state: a `check` the command runs that saw it would
  # judge obligations and gates as a completed finish and refuse, so it is not exported there.
  if ( cd "$MJ_ROOT" && unset MJ_FINISH_OUTCOME && sh -c "$MJ_FINISH_VERIFY" ) > /dev/null 2>&1; then vexit=0; else vexit=$?; fi
  t1="$(date +%s)"; after="$(mj_worktree_tree_id)"
  MJ_FINISH_VEXIT="$vexit"; MJ_FINISH_VSECS=$((t1-t0)); MJ_FINISH_VTREE="$after"
  if [ "$vexit" != 0 ]; then
    mj_doctrine_fail verification "$id" "$MJ_FINISH_VERIFY — exit $vexit, ${MJ_FINISH_VSECS}s" "$MJ_FINISH_VERIFY"; return 0; fi
  if [ -n "$before" ] && [ -n "$after" ] && [ "$before" != "$after" ]; then
    MJ_FINISH_VTREE=""
    mj_doctrine_fail verification "$id" \
      "$MJ_FINISH_VERIFY — exit 0, ${MJ_FINISH_VSECS}s, but the tree changed while it ran (${before:0:12} → ${after:0:12}); the run describes neither state" \
      "git status --porcelain; $MJ_FINISH_VERIFY"; return 0; fi
  mj_doctrine_ok verification "$id" "$MJ_FINISH_VERIFY — exit 0, ${MJ_FINISH_VSECS}s${after:+, tree ${after:0:12}}"
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
  else nf="$(grep -l "^task_id: $id$" "$MJ_STATE_DIR"/handovers/*.md 2>/dev/null | LC_ALL=C sort | tail -n1)"; fi
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
