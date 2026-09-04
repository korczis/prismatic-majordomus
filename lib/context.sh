#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_CTX_HAVE_TASK and MJ_CTX_LABEL are read by mj_context_json
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_context:-}" ] && return 0 || MJ_LIB_context=1
# context — assemble the minimum sufficient context for whoever works next.
#
# Read-only. Nothing here invokes a model, and nothing is persisted: the output is a
# projection of durable state, regenerated on demand. Sections appear in authority order,
# highest first, because a worker that runs out of budget must lose the least reliable
# evidence rather than the most: git, then the task and its policy, then blockers, then
# authored records, then event history.
#
# Which sections a profile asks for is declared in the profile's context block. What was
# left out, and why, is always printed: an under-filled context is debugged from the
# exclusion list, not by guessing.
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
# shellcheck source=history.sh
. "$MJ_LIB_DIR/history.sh"
# shellcheck source=prompt.sh
. "$MJ_LIB_DIR/prompt.sh"
# the scoped context documents: discovery, resolution, validation, impact
# shellcheck source=context_docs.sh
. "$MJ_LIB_DIR/context_docs.sh"

mj_cmd_context() {
  local for_provider="" budget="" prompt_name="" no_prompt="${MJ_NO_PROMPT:-0}"
  case "${1:-}" in
    list|resolve|explain|validate|affected|check-sync) local sub="$1"; shift; mj_context_docs_cmd "$sub" "$@"; return $? ;;
  esac
  while [ $# -gt 0 ]; do case "$1" in
    --for) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--for needs a provider name"; for_provider="$2"; shift 2 ;;
    --for=*) for_provider="${1#--for=}"; shift ;;
    --budget-lines) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--budget-lines needs a number"; budget="$2"; shift 2 ;;
    --budget-lines=*) budget="${1#--budget-lines=}"; shift ;;
    --prompt) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--prompt needs a name"; prompt_name="$2"; shift 2 ;;
    --prompt=*) prompt_name="${1#--prompt=}"; shift ;;
    --help|-h) cat <<H
usage: majordomus context [--for <provider>] [--prompt <name>] [--budget-lines <n>] [--json]
       majordomus context list | resolve <path> | explain <path> | validate | affected | check-sync
  prints what a worker needs to know now: git identity, the task and its profile, the
  context documents that apply to its scope, open questions, recent decisions, the newest
  checkpoint, the most relevant handover, recent history — as far as the policy's
  context.builder_budget_lines allows, in that order
  --for      wrap the same body for one provider named in the policy's projections
  --prompt   append a rendered prompt asset from .ai/repo/prompts/
  the subcommands read the scoped context documents under .ai/ (see context <sub> --help):
  list       every document: id, path, scope, composition, providers, status
  resolve    the documents that apply to a path, in effective order, with provenance
  explain    the same, with why each one applies and why each candidate was left out
  validate   the whole tree against the contract; exit 10 on any problem
  affected   which documents a change set touches (--base <ref>, --staged, or the working tree)
  check-sync validate, then the projections against their stamps, then affected; 10 / 11 / 0
  read-only: writes nothing, records nothing, calls no model
  exit 0 normally, 10 when the sections that cannot be dropped already exceed the budget
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "context: unknown option $1" ;;
  esac; done
  case "${budget:-0}" in ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "context: --budget-lines must be a number" ;; esac
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"

  local target=""
  if [ -n "$for_provider" ]; then
    local j=0 known=""
    while [ -n "$(mj_pol "projections.$j.provider")" ]; do
      known="$known $(mj_pol "projections.$j.provider")"
      [ "$(mj_pol "projections.$j.provider")" = "$for_provider" ] && target="$(mj_pol "projections.$j.target")"
      j=$((j + 1))
    done
    [ -n "$target" ] || mj_die "$MJ_EX_USAGE" "context: no provider '$for_provider' in the policy (have:${known})"
  fi
  [ -n "$budget" ] || budget="$(mj_pol_req context.builder_budget_lines)"

  MJ_CTX_TMP="$(mktemp -d "${TMPDIR:-/tmp}/mj.ctx.XXXXXX")"
  mj_context_sections "$prompt_name" "$no_prompt"
  local rc=0
  mj_context_emit "$budget" "$for_provider" "$target" || rc=$?
  rm -rf "$MJ_CTX_TMP"
  exit "$rc"
}

# ---------------------------------------------------------------- section building
# Every section is written to $MJ_CTX_TMP/<order>.<id> as final text, and its exclusions to
# $MJ_CTX_TMP/excluded. Assembly and budgeting never re-read state, so the text and JSON
# projections describe the same selection.
mj_ctx_excl() { printf '%s — %s\n' "$1" "$2" >> "$MJ_CTX_TMP/excluded"; }

mj_context_sections() {
  local prompt_name="$1" no_prompt="$2"
  : > "$MJ_CTX_TMP/excluded"
  local have_task=0 id="" profile="" label=""
  mj_load_current && have_task=1
  [ "$have_task" = 1 ] && { id="$(mj_cur id)"; profile="$(mj_cur profile)"; }
  if [ "$have_task" = 1 ] && ! mj_load_profile "$profile"; then
    mj_ctx_excl "profile $profile" "the task names a profile with no file; context toggles unknown"
    profile=""
  fi
  MJ_CTX_HAVE_TASK="$have_task"

  # 1. git — the highest authority, always present
  {
    printf '## GIT\n'
    printf 'repository   %s\n' "$MJ_ROOT"
    printf 'branch       %s\n' "$(mj_git_branch)"
    printf 'head         %s\n' "$(mj_git_head)"
    printf 'working_tree %s\n' "$(mj_git_dirty)"
    if [ "$have_task" = 1 ]; then
      label="$(mj_git_label "$(mj_cur head)" "$(mj_cur branch)")"
      printf 'task_record  %s (recorded head %s)\n' "$label" "$(mj_cur head | cut -c1-7)"
      case "$label" in
        diverged|different_context) printf 'WARNING      the task record no longer describes this checkout; trust git, not the records below\n' ;;
      esac
    fi
  } > "$MJ_CTX_TMP/10.git"
  MJ_CTX_LABEL="${label:-none}"

  # 2. task
  if [ "$have_task" = 0 ]; then
    printf '## TASK\nnone active — run: majordomus start "<task>" --scope <paths>\n' > "$MJ_CTX_TMP/20.task"
  elif [ -n "$profile" ] && [ "$(mj_pro context.task)" = false ]; then
    mj_ctx_excl "task" "profile $profile sets context.task: false"
  else
    local cp_age; cp_age="$(mj_age_minutes "$(mj_cur checkpoint_at)" || true)"
    {
      printf '## TASK\n'
      printf 'id           %s\n' "$id"
      printf 'task         %s\n' "$(mj_cur task)"
      printf 'profile      %s\n' "$(mj_cur profile)"
      printf 'owner        %s\n' "$(mj_cur owner)"
      printf 'outcome      %s\n' "$(mj_cur outcome)"
      printf 'scope        %s\n' "$(mj_ylist "$MJ_CUR_FLAT" scope | paste -sd, - | sed 's/,/, /g')"
      printf 'started      %s (%s)\n' "$(mj_cur started_at)" "$(mj_age_human "$(mj_age_minutes "$(mj_cur started_at)" || true)")"
      printf 'checkpoint   %s (%s, interval %s)\n' "$(mj_cur checkpoint_at)" "$(mj_age_human "$cp_age")" "${profile:+$(mj_pro checkpoint_interval)}"
    } > "$MJ_CTX_TMP/20.task"
  fi

  # 3. profile: the operating constraints the worker is under
  if [ -n "$profile" ]; then
    {
      printf '## PROFILE %s\n' "$profile"
      printf 'capability   %s\n' "$(mj_pro capability)"
      printf 'effort       %s\n' "$(mj_pro effort)"
      printf 'verbosity    %s\n' "$(mj_pro verbosity)"
      printf 'presentation %s\n' "$(mj_pro presentation)"
      printf 'verification %s\n' "$(mj_ctx_verification)"
      printf 'output       %s\n' "$(mj_ylist "$MJ_PRO_FLAT" output_contract | paste -sd, - | sed 's/,/, /g')"
    } > "$MJ_CTX_TMP/30.profile"
  fi

  # 3b. the context documents that apply to the task's scope: the chain for each claimed
  #     path, ids only; the documents themselves are read where they live
  if [ "$have_task" = 1 ]; then
    mj_ctxd_load
    if mj_ctxd_problems; then
      mj_ctx_excl "context documents" "the tree does not validate (majordomus context validate)"
    else
      local sp seen="" chain
      { printf '## CONTEXT DOCUMENTS (for the scope, root first; majordomus context explain <path>)\n'
        for sp in $(mj_ylist "$MJ_CUR_FLAT" scope); do
          case " $seen " in *" $sp "*) continue ;; esac; seen="$seen $sp"
          [ -e "$MJ_ROOT/$sp" ] || { printf '%s  (absent)\n' "$sp"; continue; }
          mj_ctxd_target "$sp"; mj_ctxd_resolve "" ""
          chain="$(while IFS="$MJ_CTXD_TAB" read -r i _; do mj_ctxd "$i" id; done < "$MJ_CTXD_CHAIN" | paste -sd, - | sed 's/,/, /g')"
          rm -f "$MJ_CTXD_CHAIN" "$MJ_CTXD_EXCLUDED"
          printf '%s  %s\n' "$sp" "${chain:-(none)}"
        done
      } > "$MJ_CTX_TMP/35.documents"
    fi
  else mj_ctx_excl "context documents" "no active task names a scope; run: majordomus context resolve <path>"; fi

  # 4. open questions — blockers are never dropped for budget
  local qf="$MJ_STATE_DIR/open-questions.md" maxi
  maxi="$(mj_pol_req context.max_list_items)"
  if [ -f "$qf" ] && [ "$have_task" = 1 ]; then
    local qn; qn="$(grep -cE "^- \[unresolved\] $id " "$qf" 2>/dev/null || true)"
    if [ "${qn:-0}" -gt 0 ]; then
      {
        printf '## OPEN QUESTIONS (%s unresolved — every one refuses finish --outcome completed)\n' "$qn"
        grep -E "^- \[unresolved\] $id " "$qf" | sed -e "s/^- \[unresolved\] $id — /- /" | head -n "$maxi"
        [ "$qn" -gt "$maxi" ] && printf '- (%s more; majordomus question list)\n' "$((qn - maxi))"
      } > "$MJ_CTX_TMP/40.questions"
    fi
  fi

  # 5. decisions
  local dfile="$MJ_STATE_DIR/decisions.md" dmax
  dmax="$(mj_pol_req context.recent_decisions)"
  if [ -z "$profile" ]; then :
  elif [ "$(mj_pro context.architecture_notes)" = true ]; then
    if [ -f "$dfile" ]; then
      { printf '## DECISIONS (%s most recent in this repository)\n' "$dmax"; mj_decision_entries "$dfile" "" "$dmax"; } > "$MJ_CTX_TMP/50.decisions"
    fi
  elif [ "$(mj_pro context.decisions)" = true ]; then
    if [ -f "$dfile" ] && [ "$have_task" = 1 ]; then
      { printf '## DECISIONS (%s most recent for this task)\n' "$dmax"; mj_decision_entries "$dfile" "$id" "$dmax"; } > "$MJ_CTX_TMP/50.decisions"
    fi
  else
    mj_ctx_excl "decisions" "profile $profile sets context.decisions: false"
  fi

  # 6. newest checkpoint for this task
  if [ "$have_task" = 1 ] && [ -n "$profile" ] && [ "$(mj_pro context.current_state)" != false ]; then
    if mj_resolve_latest "$MJ_STATE_DIR/checkpoints" "$id"; then
      {
        printf '## LATEST CHECKPOINT (%s, %s, %s)\n' "${MJ_RES_PATH#"$MJ_ROOT/"}" \
          "$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" "$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")"
        mj_record_body "$MJ_RES_PATH" | sed '/^$/d'
      } > "$MJ_CTX_TMP/60.checkpoint"
      printf '%s\n' "${MJ_RES_PATH#"$MJ_ROOT/"}" > "$MJ_CTX_TMP/60.checkpoint.path"
    fi
  fi

  # 7. most relevant handover — task-scoped first, then this worktree and branch
  if [ -z "$profile" ] || [ "$(mj_pro context.current_state)" != false ]; then
    local found=0
    if [ "$have_task" = 1 ] && mj_resolve_latest "$MJ_STATE_DIR/handovers" "$id"; then found=1
    elif mj_resolve_latest "$MJ_STATE_DIR/handovers" ""; then found=1; fi
    if [ "$found" = 1 ]; then
      {
        printf '## LATEST COMPATIBLE HANDOVER (%s)\n' "${MJ_RES_PATH#"$MJ_ROOT/"}"
        printf 'match        %s\n' "$MJ_RES_MATCH"
        printf 'git state    %s\n' "$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")"
        printf 'created      %s (%s)\n' "$MJ_RES_CREATED" "$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")"
        printf 'task         %s\n' "$MJ_RES_TASK"
        [ "$MJ_RES_DIRTY" != "$(mj_git_dirty)" ] && printf 'divergence   working tree was %s, now %s\n' "$MJ_RES_DIRTY" "$(mj_git_dirty)"
        mj_record_body "$MJ_RES_PATH" | sed '/^$/d'
      } > "$MJ_CTX_TMP/70.handover"
      printf '%s\n' "${MJ_RES_PATH#"$MJ_ROOT/"}" > "$MJ_CTX_TMP/70.handover.path"
    else
      mj_ctx_excl "handover" "no record for this worktree and branch — absence, not a stale match"
    fi
  fi

  # 8. relevant files: what is already touched inside the claimed scope
  if [ "$have_task" = 1 ] && [ -n "$profile" ]; then
    if [ "$(mj_pro context.relevant_files)" = true ]; then
      local f s inside n=0 tmpf; tmpf="$MJ_CTX_TMP/files.tmp"; : > "$tmpf"
      for f in $(mj_git_touched "$(mj_cur head)"); do
        mj_is_ai_path "$f" && continue
        inside=0; for s in $(mj_ylist "$MJ_CUR_FLAT" scope); do mj_path_contains "$s" "$f" && { inside=1; break; }; done
        [ "$inside" = 1 ] && { printf '%s\n' "$f" >> "$tmpf"; n=$((n + 1)); }
      done
      if [ "$n" -gt 0 ]; then
        { printf '## FILES TOUCHED IN SCOPE (%s)\n' "$n"; head -n "$maxi" "$tmpf"
          [ "$n" -gt "$maxi" ] && printf '(%s more; git status)\n' "$((n - maxi))"; } > "$MJ_CTX_TMP/80.files"
      fi
    else mj_ctx_excl "relevant_files" "profile $profile sets context.relevant_files: false"; fi
    [ "$(mj_pro context.failing_output)" = true ] && \
      mj_ctx_excl "failing_output" "profile $profile asks for it; Majordomus does not capture command output — paste it yourself"
  fi

  # 9. recent history
  if [ -n "$profile" ]; then
    local depth; depth="$(mj_pro context.recent_history_depth)"; [ -n "$depth" ] || depth=0
    if [ "$depth" -gt 0 ] && [ -f "$MJ_STATE_DIR/ledger.jsonl" ]; then
      { printf '## RECENT HISTORY (last %s events)\n' "$depth"
        ( export MJ_JSON=0; mj_cmd_history --limit "$depth" 2>/dev/null | sed '/^(/d' ); } > "$MJ_CTX_TMP/90.history"
    else
      mj_ctx_excl "history" "profile $profile sets context.recent_history_depth: $depth"
    fi
  fi

  # 10. prompt asset. An asset whose body asks for {{CONTEXT}} is excluded rather than
  #     rendered: the result would be this same context nested inside itself, which is
  #     duplication the budget then has to pay for twice.
  if [ -n "$prompt_name" ]; then
    local pfile; pfile="$MJ_PROMPTS_DIR/$prompt_name.md"
    if [ "$no_prompt" = 1 ] || { [ -f "$pfile" ] && grep -q '^{{CONTEXT}}$' "$pfile"; }; then
      mj_ctx_excl "prompt $prompt_name" "a prompt asset cannot include the context it is being rendered into"
    else
      { printf '## PROMPT %s\n' "$prompt_name"; MJ_NO_PROMPT=1 mj_prompt_render "$prompt_name"; } > "$MJ_CTX_TMP/95.prompt" \
        || { rm -f "$MJ_CTX_TMP/95.prompt"; mj_die "$MJ_EX_MISSING" "context: prompt '$prompt_name' could not be rendered"; }
    fi
  fi
}

mj_ctx_verification() {
  local v="" r
  r="$(mj_pro verification.verify_command_required)"
  case "$r" in true) v="verify command" ;; if_files_changed) v="verify command if files changed" ;; *) v="none" ;; esac
  [ "$(mj_pro verification.regression_test_required)" = true ] && v="$v, regression test"
  [ "$(mj_pro verification.decision_record_required)" = true ] && v="$v, decision record"
  printf '%s' "$v"
}

# ---------------------------------------------------------------- assembly and budget
# Sections are dropped in a fixed order, least reliable evidence first, and every drop is
# named in EXCLUDED. Bodies of authored records degrade to a pointer rather than vanish.
MJ_CTX_DROP_ORDER="90.history 80.files 50.decisions 60.checkpoint 70.handover"
MJ_CTX_ORDER="10.git 20.task 30.profile 35.documents 40.questions 50.decisions 60.checkpoint 70.handover 80.files 90.history 95.prompt"

# Render the whole document, including its own header and trailer, into $1. The budget
# governs what a worker actually receives, so the count must be of this file and not of
# the sections alone — a "40 of 40 lines" printed on a 50-line page is the kind of number
# this tool exists to catch.
mj_ctx_render() {
  local out="$1" budget="$2" provider="$3" target="$4" dropped="$5" f
  {
    printf '# Majordomus context — %s\n' "$(mj_now)"
    printf '# a projection of durable state, not a source of truth: validate every line against git\n'
    [ -n "$provider" ] && printf '# provider %s — its always-loaded instructions are %s\n' "$provider" "$target"
    for f in $MJ_CTX_ORDER; do [ -f "$MJ_CTX_TMP/$f" ] || continue; printf '\n'; cat "$MJ_CTX_TMP/$f"; done
    printf '\n## EXCLUDED\n'
    if [ -s "$MJ_CTX_TMP/excluded" ]; then sed 's/^/- /' "$MJ_CTX_TMP/excluded"; else printf -- '- nothing\n'; fi
    printf '\n## BUDGET\n'
  } > "$out"
  # the count includes the budget line about to be appended
  printf '%s of %s lines%s\n' "$(( $(mj_lines "$out") + 1 ))" "$budget" "${dropped:+ (dropped:$dropped)}" >> "$out"
}

mj_context_emit() {
  local budget="$1" provider="$2" target="$3" dropped="" d
  local doc; doc="$MJ_CTX_TMP/doc"
  mj_ctx_render "$doc" "$budget" "$provider" "$target" "$dropped"
  for d in $MJ_CTX_DROP_ORDER; do
    [ "$(mj_lines "$doc")" -le "$budget" ] && break
    [ -f "$MJ_CTX_TMP/$d" ] || continue
    case "$d" in
      60.checkpoint|70.handover)
        if [ -f "$MJ_CTX_TMP/$d.path" ]; then
          { head -n 1 "$MJ_CTX_TMP/$d"; printf 'body omitted for budget — read %s\n' "$(cat "$MJ_CTX_TMP/$d.path")"; } > "$MJ_CTX_TMP/$d.short"
          mv "$MJ_CTX_TMP/$d.short" "$MJ_CTX_TMP/$d"
        else rm -f "$MJ_CTX_TMP/$d"; fi
        dropped="$dropped ${d#*.}-body"; mj_ctx_excl "${d#*.} body" "context budget $budget lines" ;;
      *)
        rm -f "$MJ_CTX_TMP/$d"; dropped="$dropped ${d#*.}"; mj_ctx_excl "${d#*.}" "context budget $budget lines" ;;
    esac
    mj_ctx_render "$doc" "$budget" "$provider" "$target" "$dropped"
  done

  local total; total="$(mj_lines "$doc")"
  if [ "$MJ_JSON" = 1 ]; then mj_context_json "$budget" "$total" "$dropped" "$provider" "$target"; return 0; fi
  cat "$doc"
  if [ "$total" -gt "$budget" ]; then
    mj_err "context: $total lines over the budget of $budget with nothing left to drop"
    return "$MJ_EX_CONTRACT"
  fi
  return 0
}

# escape a whole file into one JSON string body
mj_json_file() {
  [ -f "$1" ] || { printf ''; return; }
  sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/\t/\\t/g' "$1" | awk '{ printf "%s\\n", $0 }'
}

mj_context_json() {
  local budget="$1" total="$2" dropped="$3" provider="$4" target="$5" d first
  printf '{"schema":1,"generated_at":"%s"' "$(mj_now)"
  [ -n "$provider" ] && printf ',"provider":{"name":"%s","always_loaded":"%s"}' "$provider" "$target"
  printf ',"git":{"repository":"%s","branch":"%s","head":"%s","working_tree":"%s","task_record":"%s"}' \
    "$(mj_json_esc "$MJ_ROOT")" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)" "$MJ_CTX_LABEL"
  if [ "$MJ_CTX_HAVE_TASK" = 1 ]; then
    printf ',"task":{"id":"%s","task":"%s","profile":"%s","owner":"%s","outcome":"%s","started_at":"%s","checkpoint_at":"%s","scope":[' \
      "$(mj_cur id)" "$(mj_json_esc "$(mj_cur task)")" "$(mj_cur profile)" "$(mj_json_esc "$(mj_cur owner)")" \
      "$(mj_cur outcome)" "$(mj_cur started_at)" "$(mj_cur checkpoint_at)"
    first=1; for d in $(mj_ylist "$MJ_CUR_FLAT" scope); do [ "$first" = 1 ] || printf ','; printf '"%s"' "$(mj_json_esc "$d")"; first=0; done
    printf ']}'
  else printf ',"task":null'; fi
  printf ',"sections":['
  first=1
  for d in 30.profile 35.documents 40.questions 50.decisions 60.checkpoint 70.handover 80.files 90.history 95.prompt; do
    [ -f "$MJ_CTX_TMP/$d" ] || continue
    [ "$first" = 1 ] || printf ','
    printf '{"id":"%s","lines":%s,"text":"%s"}' "${d#*.}" "$(mj_lines "$MJ_CTX_TMP/$d")" "$(mj_json_file "$MJ_CTX_TMP/$d")"
    first=0
  done
  printf '],"excluded":['
  first=1
  if [ -s "$MJ_CTX_TMP/excluded" ]; then
    while IFS= read -r line; do
      [ "$first" = 1 ] || printf ','
      printf '{"item":"%s","reason":"%s"}' "$(mj_json_esc "${line%% — *}")" "$(mj_json_esc "${line#* — }")"; first=0
    done < "$MJ_CTX_TMP/excluded"
  fi
  printf '],"budget":{"lines":%s,"limit":%s,"dropped":[' "$total" "$budget"
  first=1; for d in $dropped; do [ "$first" = 1 ] || printf ','; printf '"%s"' "$d"; first=0; done
  printf ']}}\n'
}

# ---------------------------------------------------------------- context documents
# The subcommands over the scoped context documents. Every one is read-only and answers
# from the repository as it is: nothing is cached, and the same tree gives the same answer.
mj_context_docs_cmd() {
  local sub="$1"; shift
  local provider="" audience="" mode=worktree base="" target="" a
  while [ $# -gt 0 ]; do case "$1" in
    --provider) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--provider needs a name"; provider="$2"; shift 2 ;;
    --provider=*) provider="${1#--provider=}"; shift ;;
    --audience) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--audience needs human or agent"; audience="$2"; shift 2 ;;
    --audience=*) audience="${1#--audience=}"; shift ;;
    --base) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--base needs a git ref"; mode=base; base="$2"; shift 2 ;;
    --base=*) mode=base; base="${1#--base=}"; shift ;;
    --staged) mode=staged; shift ;;
    --worktree) mode=worktree; shift ;;
    --help|-h) mj_context_docs_help "$sub"; return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "context $sub: unknown option $1" ;;
    *) [ -z "$target" ] || mj_die "$MJ_EX_USAGE" "context $sub: one path only (got '$target' and '$1')"; target="$1"; shift ;;
  esac; done
  case "$sub" in
    resolve|explain) [ -n "$target" ] || mj_die "$MJ_EX_USAGE" "context $sub: which path? (usage: majordomus context $sub <path>)" ;;
    *) [ -z "$target" ] || mj_die "$MJ_EX_USAGE" "context $sub: takes no path (got '$target')" ;;
  esac
  case "$audience" in ""|human|agent) ;; *) mj_die "$MJ_EX_USAGE" "context $sub: --audience must be human or agent" ;; esac
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  if [ -n "$provider" ]; then
    case " $(mj_ctxd_providers) " in *" $provider "*) ;; *) mj_die "$MJ_EX_USAGE" "context $sub: unknown-provider '$provider'; the policy projects: $(mj_ctxd_providers)" ;; esac
  fi
  local rc=0
  case "$sub" in
    list) mj_context_docs_list ;;
    resolve|explain) mj_context_docs_resolve "$sub" "$target" "$provider" "$audience" ;;
    validate) mj_context_docs_validate || rc=$? ;;
    affected) mj_context_docs_affected "$mode" "$base" || rc=$? ;;
    check-sync) mj_context_docs_check_sync "$mode" "$base" || rc=$? ;;
  esac
  mj_ctxd_cleanup
  exit "$rc"
}

mj_context_docs_help() {
  case "$1" in
    list) cat <<H
usage: majordomus context list [--json]
  every context document under .ai/: id, path, scope, composition, providers, status, in
  the order resolution uses (depth, order, path); deprecated ones are listed and never applied
H
;;
    resolve|explain) cat <<H
usage: majordomus context $1 <path> [--provider <name>] [--audience human|agent] [--json]
  the context documents that apply to <path> (a directory, or a file's directory), least
  specific first: the tree root, each ancestor, the target; outside .ai/ the tree root plus
  every document whose tracks cover the path. --provider and --audience filter; a document
  superseded by one below it leaves the chain. Refuses (10) when the tree does not validate.
  a document with no audience line addresses both human and agent
$([ "$1" = explain ] && printf '  explain adds why each document applies and lists every candidate left out, with the reason\n')
H
;;
    validate) cat <<H
usage: majordomus context validate [--json]
  every document against the contract (schema context/v1, the front-matter keys, ids,
  providers), then across documents: one file per id, references that resolve, no
  supersedes outside the ancestor chain, no final document superseded, no cycle; the file
  conventions the manifest names carry the contract wherever they appear
  exit 0 when nothing is wrong, 10 with every problem named
H
;;
    affected) cat <<H
usage: majordomus context affected [--base <ref> | --staged | --worktree] [--json]
  what a change set touches: a changed document and every document resolved below it, a
  move with its ancestry change, a deletion, the manifest (everything), the policy (the
  projections), and every document whose tracks cover a changed path (WARN: review it).
  An unrelated change prints nothing. Read-only; exit 0, or 10 when the tree does not validate.
H
;;
    check-sync) cat <<H
usage: majordomus context check-sync [--base <ref> | --staged | --worktree] [--json]
  validate (a problem is FAIL, exit 10), then each projection the policy names against its
  stamp (a hand edit is stale-projection, DRIFT, exit 11), then affected; WARN and INFO
  never change the exit code
H
;;
  esac
}

mj_context_docs_list() {
  mj_ctxd_load
  local i=0 tmp; tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.ls.XXXXXX")"
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    printf '%s\t%s\t%s\t%s\n' "$(mj_ctxd "$i" depth)" "$(mj_ctxd "$i" order)" "$(mj_ctxd "$i" path)" "$i" >> "$tmp"; i=$((i + 1))
  done
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":"%s","tree":"%s","documents":[' "$MJ_CTXD_SCHEMA" "$(mj_ctxd_tree)"
    local first=1 k=1
    if [ -s "$tmp" ]; then while IFS="$MJ_CTXD_TAB" read -r _ _ _ i; do [ "$first" = 1 ] || printf ','; mj_ctxd_json_doc "$i" "" "$k"; first=0; k=$((k + 1)); done < <(LC_ALL=C sort -t "$MJ_CTXD_TAB" -k1,1n -k2,2n -k3,3 "$tmp"); fi
    printf '],"problems":['
    first=1
    if mj_ctxd_problems; then while IFS="$MJ_CTXD_TAB" read -r cls subj msg rep; do [ "$first" = 1 ] || printf ','; printf '{"class":"%s","subject":"%s","message":"%s","reproduce":"%s"}' "$cls" "$(mj_json_esc "$subj")" "$(mj_json_esc "$msg")" "$(mj_json_esc "$rep")"; first=0; done < "$MJ_CTXD_PROBLEMS"; fi
    printf ']}\n'
  else
    if [ -s "$tmp" ]; then
      while IFS="$MJ_CTXD_TAB" read -r _ _ _ i; do
        printf '%-28s %-44s %-9s %-7s %-14s %s\n' "$(mj_ctxd "$i" id)" "$(mj_ctxd "$i" path)" "$(mj_ctxd "$i" scope)" "$(mj_ctxd "$i" composition)" "$(mj_ctxd_list "$i" providers | paste -sd, -)" "$(mj_ctxd "$i" status)"
      done < <(LC_ALL=C sort -t "$MJ_CTXD_TAB" -k1,1n -k2,2n -k3,3 "$tmp")
    else printf 'no context document under %s/ (a Markdown file with front matter schema: %s, kind: context)\n' "$(mj_ctxd_tree)" "$MJ_CTXD_SCHEMA"; fi
    [ -n "$(mj_ctxd_conventions)" ] || printf 'the manifest names no file convention (context.documents); only the contract identifies a document\n'
    if mj_ctxd_problems; then mj_ctxd_report_problems; printf 'context list: %s problem(s); run: majordomus context validate\n' "$(mj_lines "$MJ_CTXD_PROBLEMS")"; fi
  fi
  rm -f "$tmp"
}

mj_context_docs_resolve() {
  local sub="$1" target="$2" provider="$3" audience="$4" i reason k=1 first
  mj_ctxd_require_valid
  mj_ctxd_target "$target"
  mj_ctxd_resolve "$provider" "$audience"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":"%s","target":"%s","directory":"%s","provider":%s,"audience":%s,"fingerprint":"%s","documents":[' \
      "$MJ_CTXD_SCHEMA" "$(mj_json_esc "$MJ_CTXD_TARGET")" "$(mj_json_esc "$MJ_CTXD_TARGET_DIR")" \
      "$([ -n "$provider" ] && printf '"%s"' "$provider" || printf null)" "$([ -n "$audience" ] && printf '"%s"' "$audience" || printf null)" "$(mj_ctxd_fingerprint)"
    first=1
    while IFS="$MJ_CTXD_TAB" read -r i reason; do [ "$first" = 1 ] || printf ','; mj_ctxd_json_doc "$i" "$reason" "$k"; first=0; k=$((k + 1)); done < "$MJ_CTXD_CHAIN"
    printf '],"excluded":['
    first=1
    if [ -s "$MJ_CTXD_EXCLUDED" ]; then while IFS="$MJ_CTXD_TAB" read -r i reason; do [ "$first" = 1 ] || printf ','; printf '{"id":"%s","path":"%s","reason":"%s"}' "$(mj_ctxd "$i" id)" "$(mj_json_esc "$(mj_ctxd "$i" path)")" "$(mj_json_esc "$reason")"; first=0; done < "$MJ_CTXD_EXCLUDED"; fi
    printf ']}\n'
  else
    printf '# effective context for %s%s%s\n' "$MJ_CTXD_TARGET_DIR" "${provider:+ — provider $provider}" "${audience:+ — audience $audience}"
    if [ -s "$MJ_CTXD_CHAIN" ]; then
      while IFS="$MJ_CTXD_TAB" read -r i reason; do
        printf '%02d  %-26s %-42s %-9s %-7s %s\n' "$k" "$(mj_ctxd "$i" id)" "$(mj_ctxd "$i" path)" "$(mj_ctxd "$i" scope)" "$(mj_ctxd "$i" composition)" "$(mj_ctxd_list "$i" providers | paste -sd, -)"
        [ "$sub" = explain ] && printf '    applies: %s\n' "$reason"
        k=$((k + 1))
      done < "$MJ_CTXD_CHAIN"
    else printf '(no document applies)\n'; fi
    if [ "$sub" = explain ]; then
      printf '\n# left out\n'
      if [ -s "$MJ_CTXD_EXCLUDED" ]; then while IFS="$MJ_CTXD_TAB" read -r i reason; do printf -- '- %s (%s): %s\n' "$(mj_ctxd "$i" id)" "$(mj_ctxd "$i" path)" "$reason"; done < "$MJ_CTXD_EXCLUDED"
      else printf '(nothing: every candidate applies)\n'; fi
      printf '\n# order\ndepth (tree root first), then order, then path; never the order the filesystem lists files in\nfingerprint %s\n' "$(mj_ctxd_fingerprint)"
    fi
  fi
  rm -f "$MJ_CTXD_CHAIN" "$MJ_CTXD_EXCLUDED"
}

mj_context_docs_validate() {
  mj_ctxd_load
  if mj_ctxd_problems; then
    mj_ctxd_report_problems
    [ "$MJ_JSON" = 1 ] || printf 'context validate: %s problem(s) in %s document(s)\n' "$(mj_lines "$MJ_CTXD_PROBLEMS")" "$MJ_CTXD_COUNT"
    return "$MJ_EX_CONTRACT"
  fi
  mj_ok context "$(mj_ctxd_tree)/" "$MJ_CTXD_COUNT document(s) carry the contract; ids unique, references resolve, no final document superseded, no cycle"
  [ "$MJ_JSON" = 1 ] || printf 'context validate: 0 problem(s) in %s document(s)\n' "$MJ_CTXD_COUNT"
  return 0
}

mj_context_docs_affected() {
  local mode="$1" base="$2"
  mj_ctxd_load
  if mj_ctxd_problems; then mj_ctxd_report_problems; fi
  mj_ctxd_affected "$mode" "$base"
  mj_ctxd_problems && return "$MJ_EX_CONTRACT"
  return 0
}

mj_context_docs_check_sync() {
  local mode="$1" base="$2" rc=0 j=0 tgt prov st
  mj_ctxd_load
  if mj_ctxd_problems; then mj_ctxd_report_problems; rc="$MJ_EX_CONTRACT"
  else mj_ok context "$(mj_ctxd_tree)/" "$MJ_CTXD_COUNT document(s) validate"; fi
  # the projections: absent is doctor's finding (missing, not stale); a hand edit or a
  # stamp that no longer matches is the projection saying something the policy does not
  local psha tmp; tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.pol.XXXXXX")"; mj_policy_cat > "$tmp"; psha="$(mj_sha256 "$tmp")"; rm -f "$tmp"
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"; prov="$(mj_pol "projections.$j.provider")"
    st="$(mj_projection_status "$tgt" "$(mj_projection_mode "$j")" "$prov")"
    local stamped="${st#* }"; st="${st%% *}"
    case "$st" in
      ok) if [ "$stamped" = "${psha:0:12}" ]; then mj_ok context "$tgt" "projection for $prov matches its stamp and the policy as it is"
          else mj_drift context "$tgt" "stale-projection: rendered from policy $stamped, and the policy is now ${psha:0:12}" "majordomus update --dry-run"; [ "$rc" = 0 ] && rc="$MJ_EX_DRIFT"; fi ;;
      absent|region_absent|no_template) mj_info context "$tgt" "projection for $prov is $st (majordomus doctor reports it; majordomus update writes it)" "majordomus update" ;;
      *) mj_drift context "$tgt" "stale-projection: $st; the projection for $prov no longer matches its stamp" "majordomus update --diff $tgt"; [ "$rc" = 0 ] && rc="$MJ_EX_DRIFT" ;;
    esac
    j=$((j + 1))
  done
  mj_ctxd_affected "$mode" "$base"
  [ "$MJ_JSON" = 1 ] || printf 'context check-sync: %s\n' "$(case "$rc" in 0) printf 'in sync (%s document(s), %s projection(s), %s affected)' "$MJ_CTXD_COUNT" "$j" "$MJ_CTXD_AFFECTED" ;; 10) printf 'the tree does not validate' ;; *) printf 'drift found' ;; esac)"
  return "$rc"
}
