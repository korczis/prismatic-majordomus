#!/usr/bin/env bash
# handover — write an append-only continuation record (body on stdin), or --resolve the most
# relevant prior one. Never stages, commits, or modifies any other file.
mj_cmd_handover() {
  local resolve=0 path_only=0 close=0 no_task=0
  while [ $# -gt 0 ]; do case "$1" in
    --resolve) resolve=1; shift ;; --path) path_only=1; shift ;; --close) close=1; shift ;; --no-task) no_task=1; shift ;;
    --help|-h) cat <<H
usage: majordomus handover [--close] [--no-task] < body.md
       majordomus handover --resolve [--path]
  writes .majordomus/state/handovers/<ts>--<branch>--<head>--<rand>.md (mode 0600, atomic, never staged)
  body needs these non-empty level-one headings: the policy's handover.required_sections
  --close     also mark the active task handed_over so a new task may start
  --no-task   allow writing without an active task
  --resolve   print the most relevant prior handover for this worktree and branch (never repo-wide)
  --path      with --resolve, print only the path
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "handover: unknown option $1" ;;
  esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  if [ "$resolve" = 1 ]; then mj_handover_resolve "$path_only"; return; fi

  local task_id="" profile="" owner=""
  if mj_load_current; then task_id="$(mj_cur id)"; profile="$(mj_cur profile)"; owner="$(mj_cur owner)"
  elif [ "$no_task" != 1 ]; then mj_die "$MJ_EX_MISSING" "no active task; run majordomus start, or pass --no-task"; fi

  local body; body="$(mktemp "${TMPDIR:-/tmp}/mj.hb.XXXXXX")"; cat > "$body"
  [ -s "$body" ] || { rm -f "$body"; mj_die "$MJ_EX_CONTRACT" "handover: empty body on stdin"; }
  if grep -qE '^(schema_version|created_at|task_id|repository_id|worktree|branch|head|working_tree|changed_files):' "$body"; then
    rm -f "$body"; mj_die "$MJ_EX_CONTRACT" "handover: body must not contain identity fields; they are computed"
  fi
  local missing; missing="$(mj_check_sections "$body" "$(mj_ylist "$MJ_POL_FLAT" handover.required_sections | tr '\n' '|')")"
  [ -z "$missing" ] || { rm -f "$body"; mj_die "$MJ_EX_CONTRACT" "handover: missing or empty section(s): $missing"; }

  local dir="$MJ_DIR/state/handovers" head; mkdir -p "$dir"; head="$(mj_git_head)"
  local tmp; tmp="$(mktemp "$dir/.tmp.XXXXXX")"; chmod 600 "$tmp"
  {
    printf -- '---\nschema_version: 1\ncreated_at: %s\ntask_id: %s\nprofile: %s\nowner: "%s"\n' "$(mj_now)" "${task_id:-none}" "${profile:-none}" "$owner"
    printf 'repository_id: %s\nworktree: %s\nbranch: %s\nhead: %s\nworking_tree: %s\nchanged_files:\n' \
      "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$head" "$(mj_git_dirty)"
    mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //' | sed 's/^/  - /'
    printf -- '---\n\n'; cat "$body"
  } > "$tmp"
  local final n=0
  while :; do
    final="$dir/$(mj_now_compact)--$(mj_branch_key)--${head:0:7}--$(mj_rand16).md"
    ln "$tmp" "$final" 2>/dev/null && break
    n=$((n+1)); [ "$n" -lt 10 ] || { rm -f "$tmp" "$body"; mj_die "$MJ_EX_INTERNAL" "could not create a unique handover file"; }
  done
  rm -f "$tmp" "$body"
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
  local path_only="$1" dir="$MJ_DIR/state/handovers" f fm flat tier best="" best_key="" key
  local my_id my_branch; my_id="$(mj_git_repo_id)"; my_branch="$(mj_git_branch)"
  [ -d "$dir" ] || { echo "No relevant handover."; return 0; }
  for f in "$dir"/*.md; do
    [ -f "$f" ] || continue
    fm="$(mktemp "${TMPDIR:-/tmp}/mj.fm.XXXXXX")"; awk 'NR==1&&$0!="---"{exit 2} NR>1&&$0=="---"{exit} NR>1' "$f" > "$fm" || { rm -f "$fm"; mj_err "warning: skipped $f: no front matter"; continue; }
    flat="$(mktemp "${TMPDIR:-/tmp}/mj.fl.XXXXXX")"; mj_yaml_flatten "$fm" > "$flat" 2>/dev/null || { rm -f "$fm" "$flat"; mj_err "warning: skipped $f: malformed front matter"; continue; }
    if [ "$(mj_yget "$flat" schema_version)" != 1 ] || [ -z "$(mj_yget "$flat" head)" ] || [ -z "$(mj_yget "$flat" created_at)" ]; then
      rm -f "$fm" "$flat"; mj_err "warning: skipped $f: missing required fields"; continue; fi
    tier=""
    if [ "$(mj_yget "$flat" repository_id)" = "$my_id" ]; then
      if [ "$(mj_yget "$flat" worktree)" = "$MJ_ROOT" ] && [ "$(mj_yget "$flat" branch)" = "$my_branch" ]; then tier=0
      elif [ "$my_branch" != DETACHED ] && [ "$(mj_yget "$flat" branch)" = "$my_branch" ]; then tier=1; fi
    fi
    if [ -n "$tier" ]; then
      key="$tier|$(mj_yget "$flat" created_at)"   # lower tier wins; then newest
      if [ -z "$best" ] || [ "${key%%|*}" -lt "${best_key%%|*}" ] || { [ "${key%%|*}" = "${best_key%%|*}" ] && [ "${key#*|}" \> "${best_key#*|}" ]; }; then
        best="$f"; best_key="$key"; BEST_HEAD="$(mj_yget "$flat" head)"; BEST_BRANCH="$(mj_yget "$flat" branch)"; BEST_DIRTY="$(mj_yget "$flat" working_tree)"
      fi
    fi
    rm -f "$fm" "$flat"
  done
  [ -n "$best" ] || { echo "No relevant handover."; return 0; }
  if [ "$path_only" = 1 ]; then printf '%s\n' "${best#"$MJ_ROOT/"}"; return 0; fi
  printf 'Handover: %s\nMatch: %s\nGit state: %s\n' "${best#"$MJ_ROOT/"}" \
    "$([ "${best_key%%|*}" = 0 ] && echo same_worktree_same_branch || echo same_branch)" "$(mj_git_label "$BEST_HEAD" "$BEST_BRANCH")"
  [ "$BEST_DIRTY" != "$(mj_git_dirty)" ] && printf 'Divergence: working tree was %s, now %s\n' "$BEST_DIRTY" "$(mj_git_dirty)"
  printf -- '---\n'; awk 'c>=2{print} /^---$/{c++}' "$best"
}
