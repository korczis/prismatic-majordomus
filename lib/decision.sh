#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_decision:-}" ] && return 0 || MJ_LIB_decision=1
# decision — append a durable decision record, or read the ones already recorded.
# One file, append-only, newest at the bottom: .majordomus/state/decisions.md.
# Supersession is expressed by a later entry naming an earlier one, never by editing it.

mj_cmd_decision() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    add) mj_decision_add "$@" ;;
    list) mj_decision_list "$@" ;;
    show) mj_decision_show "$@" ;;
    --help|-h|"") cat <<H
usage: majordomus decision add "<what was decided>" --why "<rationale>"
                               [--rejected "<alternatives>"] [--evidence "<file, test or measurement>"]
                               [--supersedes "<text from an earlier decision>"]
       majordomus decision list [--task <id>] [--limit <n>]
       majordomus decision show "<text>"
  appends one entry to .majordomus/state/decisions.md; the task id and git head are computed
  an entry is never edited or deleted: --supersedes records that a later decision replaced an earlier one
H
      [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "decision: unknown subcommand '$sub' (add|list|show)" ;;
  esac
}

mj_decision_file() { printf '%s' "$MJ_STATE_DIR/decisions.md"; }

mj_decision_add() {
  local title="" why="" rejected="-" evidence="-" supersedes="-"
  while [ $# -gt 0 ]; do case "$1" in
    --why) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--why needs text"; why="$2"; shift 2 ;;
    --why=*) why="${1#--why=}"; shift ;;
    --rejected) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--rejected needs text"; rejected="$2"; shift 2 ;;
    --rejected=*) rejected="${1#--rejected=}"; shift ;;
    --evidence) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--evidence needs text"; evidence="$2"; shift 2 ;;
    --evidence=*) evidence="${1#--evidence=}"; shift ;;
    --supersedes) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--supersedes needs text"; supersedes="$2"; shift 2 ;;
    --supersedes=*) supersedes="${1#--supersedes=}"; shift ;;
    -*) mj_die "$MJ_EX_USAGE" "decision add: unknown option $1" ;;
    *) [ -z "$title" ] || mj_die "$MJ_EX_USAGE" "decision add: the decision must be one argument (quote it)"; title="$1"; shift ;;
  esac; done
  [ -n "$title" ] || mj_die "$MJ_EX_USAGE" "decision add: the decision text is required"
  [ -n "$why" ] || mj_die "$MJ_EX_USAGE" "decision add: --why is required (a decision without a reason cannot be reviewed)"
  mj_is_multiline "$title$why$rejected$evidence$supersedes" && mj_die "$MJ_EX_USAGE" "decision add: text must be single-line" 
  mj_require_installed
  local file; file="$(mj_decision_file)"
  [ -f "$file" ] || mj_die "$MJ_EX_MISSING" "no $file (run: majordomus init)"

  local task_id=none
  mj_load_current && task_id="$(mj_cur id)"

  if [ "$supersedes" != "-" ]; then
    grep -qF -- "$supersedes" "$file" || mj_die "$MJ_EX_USAGE" "decision add: --supersedes '$supersedes' matches no recorded decision"
  fi

  printf '\n## %s — %s\nTask: %s\nHead: %s\nWhy: %s\nRejected: %s\nEvidence: %s\nSupersedes: %s\n' \
    "$(date -u +%Y-%m-%d)" "$title" "$task_id" "$(mj_git_head)" "$why" "$rejected" "$evidence" "$supersedes" >> "$file"
  mj_ledger_append decision.recorded "\"task_id\":\"$task_id\",\"decision\":\"$(mj_json_esc "$title")\""
  printf 'recorded: %s\n' "$title"
}

# print entries, newest first. mj_decision_entries FILE [TASK] [LIMIT]
mj_decision_entries() {
  awk -v want="${2:-}" -v limit="${3:-0}" '
    /<!--/ { c=1 } /-->/ { c=0; next }
    c { next }
    /^## / { n++; head[n]=$0; body[n]=""; task[n]=""; next }
    n>0 && /^Task: / { task[n]=substr($0,7) }
    n>0 { body[n]=body[n] $0 "\n" }
    END{
      shown=0
      for (i=n; i>=1; i--) {
        if (want!="" && task[i]!=want) continue
        if (limit>0 && shown>=limit) break
        printf "%s\n%s", head[i], body[i]
        shown++
      }
      if (shown==0) print "(none)"
    }' "$1"
}

mj_decision_list() {
  local want="" limit=0
  while [ $# -gt 0 ]; do case "$1" in
    --task) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--task needs an id"; want="$2"; shift 2 ;;
    --task=*) want="${1#--task=}"; shift ;;
    --limit) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--limit needs a number"; limit="$2"; shift 2 ;;
    --limit=*) limit="${1#--limit=}"; shift ;;
    *) mj_die "$MJ_EX_USAGE" "decision list: unknown option $1" ;;
  esac; done
  case "$limit" in ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "decision list: --limit must be a number" ;; esac
  mj_require_installed
  local file; file="$(mj_decision_file)"
  [ -f "$file" ] || mj_die "$MJ_EX_MISSING" "no $file (run: majordomus init)"
  mj_decision_entries "$file" "$want" "$limit"
}

mj_decision_show() {
  [ $# -ge 1 ] || mj_die "$MJ_EX_USAGE" "decision show: text to match is required"
  mj_require_installed
  local file; file="$(mj_decision_file)"
  [ -f "$file" ] || mj_die "$MJ_EX_MISSING" "no $file (run: majordomus init)"
  local out; out="$(awk -v pat="$1" '
    /<!--/ { c=1 } /-->/ { c=0; next }
    c { next }
    /^## / { if (keep) exit; keep = (index($0,pat)>0) }
    keep { print }' "$file")"
  if [ -z "$out" ]; then mj_err "no decision matches '$1'"; return "$MJ_EX_MISSING"; fi
  printf '%s\n' "$out"
}

# line numbers of decision entries missing a required field. Same reasoning as for
# questions: the deep-work profile can require a decision record, and a gate that reads
# `Task:` must be able to trust that every entry has one.
mj_decision_malformed() {
  [ -f "$1" ] || return 0
  awk '
    /<!--/ { c=1 } /-->/ { c=0; next }
    c { next }
    /^## / { if (n && !(t && h && w)) printf "%s ", start; n=1; start=NR; t=0; h=0; w=0; next }
    /^Task: /  { t=1 } /^Head: / { h=1 } /^Why: / { w=1 }
    END{ if (n && !(t && h && w)) printf "%s ", start }' "$1"
}
