#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_handover:-}" ] && return 0 || MJ_LIB_handover=1
# handover — write an append-only continuation record (body on stdin), or --resolve the most
# relevant prior one. Never stages, commits, or modifies any other file.
mj_cmd_handover() {
  local resolve=0 path_only=0 close=0 no_task=0 list=0 want_task=""
  while [ $# -gt 0 ]; do case "$1" in
    --resolve) resolve=1; shift ;; --path) path_only=1; shift ;; --close) close=1; shift ;; --no-task) no_task=1; shift ;;
    --list) list=1; shift ;;
    --task) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--task needs a task id"; want_task="$2"; shift 2 ;;
    --task=*) want_task="${1#--task=}"; shift ;;
    --help|-h) cat <<H
usage: majordomus handover [--close] [--no-task] < body.md
       majordomus handover --resolve [--task <id>] [--path]
       majordomus handover --list
  writes .ai/local/state/handovers/<ts>--<branch>--<head>--<rand>.md (mode 0600, atomic, never staged)
  body needs these non-empty level-one headings: the policy's handover.required_sections
  --close     also mark the active task handed_over so a new task may start
  --no-task   allow writing without an active task
  --resolve   print the most relevant prior handover for this worktree and branch (never repo-wide)
  --task ID   with --resolve, consider only handovers written for that task
  --path      with --resolve, print only the path
  --list      list this worktree's handovers, newest first
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "handover: unknown option $1" ;;
  esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  if [ "$list" = 1 ]; then mj_record_list "$MJ_STATE_DIR/handovers" handover; return; fi
  if [ "$resolve" = 1 ]; then mj_handover_resolve "$path_only" "$want_task"; return; fi

  local task_id="" profile="" owner=""
  if mj_load_current; then task_id="$(mj_cur id)"; profile="$(mj_cur profile)"; owner="$(mj_cur owner)"
  elif [ "$no_task" != 1 ]; then mj_die "$MJ_EX_MISSING" "no active task; run majordomus start, or pass --no-task"; fi

  local body; body="$(mktemp "${TMPDIR:-/tmp}/mj.hb.XXXXXX")"; cat > "$body"
  [ -s "$body" ] || { rm -f "$body"; mj_die "$MJ_EX_CONTRACT" "handover: empty body on stdin"; }
  if mj_reject_identity "$body"; then
    rm -f "$body"; mj_die "$MJ_EX_CONTRACT" "handover: body must not contain identity fields; they are computed"
  fi
  local missing; missing="$(mj_check_sections "$body" "$(mj_ylist "$MJ_POL_FLAT" handover.required_sections | tr '\n' '|')")"
  [ -z "$missing" ] || { rm -f "$body"; mj_die "$MJ_EX_CONTRACT" "handover: missing or empty section(s): $missing"; }

  local rec; rec="$(mktemp "${TMPDIR:-/tmp}/mj.hr.XXXXXX")"
  { mj_record_front_matter "${task_id:-none}" "${profile:-none}" "$owner"; cat "$body"; } > "$rec"
  local final; final="$(mj_publish_record "$MJ_STATE_DIR/handovers" "" "$rec")" \
    || { rm -f "$rec" "$body"; mj_die "$MJ_EX_INTERNAL" "could not create a unique handover file"; }
  rm -f "$rec" "$body"

  if [ -n "$task_id" ]; then
    local now; now="$(mj_now)"
    if [ "$close" = 1 ]; then
      sed -e "s/^checkpoint_at: .*/checkpoint_at: $now/" -e 's/^outcome: .*/outcome: handed_over/' "$MJ_CUR" > "$MJ_CUR.mj-tmp"
    else sed -e "s/^checkpoint_at: .*/checkpoint_at: $now/" "$MJ_CUR" > "$MJ_CUR.mj-tmp"; fi
    mv "$MJ_CUR.mj-tmp" "$MJ_CUR"
    mj_ledger_append task.handed_over "\"task_id\":\"$task_id\",\"handover_path\":\"$(mj_json_esc "${final#"$MJ_ROOT/"}")\",\"closed\":$close"
  fi
  printf '%s\n' "${final#"$MJ_ROOT/"}"
}

# prints names of required sections that are missing or empty; $2 = "A|B|C"
mj_check_sections() {
  awk -v req="$2" '
    BEGIN{ n=split(req,r,"|"); for(i=1;i<=n;i++) if(r[i]!="") { want[r[i]]=1; has[r[i]]=0 } }
    /^# / { cur=substr($0,3); sub(/[ \t]+$/,"",cur); next }
    { if (cur!="" && (cur in want) && $0 ~ /[^ \t]/ && $0 !~ /^<.*>$/) has[cur]=1 }
    END{ for(k in want) if(!has[k]) printf "%s%s", (out++?", ":""), k }' "$1"
}

mj_handover_resolve() {
  local path_only="$1" want_task="${2:-}"
  if ! mj_resolve_latest "$MJ_STATE_DIR/handovers" "$want_task"; then echo "No relevant handover."; return 0; fi
  if [ "$path_only" = 1 ]; then printf '%s\n' "${MJ_RES_PATH#"$MJ_ROOT/"}"; return 0; fi
  printf 'Handover: %s\nMatch: %s\nGit state: %s\nCreated: %s (%s)\nTask: %s\n' \
    "${MJ_RES_PATH#"$MJ_ROOT/"}" "$MJ_RES_MATCH" "$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")" \
    "$MJ_RES_CREATED" "$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" "$MJ_RES_TASK"
  [ "$MJ_RES_DIRTY" != "$(mj_git_dirty)" ] && printf 'Divergence: working tree was %s, now %s\n' "$MJ_RES_DIRTY" "$(mj_git_dirty)"
  printf -- '---\n'; mj_record_body "$MJ_RES_PATH"
}

# list records in a directory, newest first: created_at, task, git label, path
mj_record_list() {
  local dir="$1" kind="$2" f fm flat n=0
  [ -d "$dir" ] || { printf 'no %s records\n' "$kind"; return 0; }
  for f in $(ls -1 "$dir"/*.md 2>/dev/null | sort -r); do
    fm="$(mktemp "${TMPDIR:-/tmp}/mj.lf.XXXXXX")"; flat="$(mktemp "${TMPDIR:-/tmp}/mj.lg.XXXXXX")"
    if mj_record_front "$f" > "$fm" 2>/dev/null && mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then
      printf '%s  %-24s %-18s %s\n' "$(mj_yget "$flat" created_at)" "$(mj_yget "$flat" task_id)" \
        "$(mj_git_label "$(mj_yget "$flat" head)" "$(mj_yget "$flat" branch)")" "${f#"$MJ_ROOT/"}"
      n=$((n + 1))
    else mj_err "warning: skipped ${f#"$MJ_ROOT/"}: malformed record"; fi
    rm -f "$fm" "$flat"
  done
  [ "$n" = 0 ] && printf 'no %s records for this worktree\n' "$kind"
  return 0
}
