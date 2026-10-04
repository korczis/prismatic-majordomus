#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_decision:-}" ] && return 0 || MJ_LIB_decision=1
# decision — append a durable decision record, or read the ones already recorded.
# One file, append-only, newest at the bottom: .ai/local/state/decisions.md.
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
                               [--supersedes "<part of an earlier decision's title>"]
       majordomus decision list [--task <id>] [--limit <n>]
       majordomus decision show "<text>"
  appends one entry to .ai/local/state/decisions.md; the task id and git head are computed
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
  # the store is checkout-local and absent on a fresh clone; the first decision seeds it
  [ -f "$file" ] || { mkdir -p "$MJ_STATE_DIR"; cp "$MJ_SKELETON_DIR/templates/decisions.md" "$file"; }

  local task_id=none
  mj_load_current && task_id="$(mj_cur id)"

  # A supersession names one decision, by its title. The text used to be searched for
  # anywhere in the file, so a field label (`--supersedes Why`) or a word of some rationale
  # was accepted as "a recorded decision", and the entry pointed at nothing in particular.
  # It is matched against the titles alone, must identify exactly one, and the title it
  # identified is what is recorded, so `list` can say which decision it replaced.
  if [ "$supersedes" != "-" ]; then
    local matched n
    matched="$(mj_decision_titles "$file" | grep -F -- "$supersedes" || true)"
    n="$(printf '%s' "$matched" | grep -c '' || true)"
    case "$n" in
      0) mj_die "$MJ_EX_USAGE" "decision add: --supersedes '$supersedes' matches no recorded decision's title (majordomus decision list)" ;;
      1) supersedes="$matched" ;;
      *) mj_die "$MJ_EX_USAGE" "decision add: --supersedes '$supersedes' matches $n recorded decisions; quote more of one title: $(printf '%s' "$matched" | paste -sd '|' - | sed 's/|/; /g')" ;;
    esac
  fi

  printf '\n## %s — %s\nTask: %s\nHead: %s\nWhy: %s\nRejected: %s\nEvidence: %s\nSupersedes: %s\n' \
    "$(date -u +%Y-%m-%d)" "$title" "$task_id" "$(mj_git_head)" "$why" "$rejected" "$evidence" "$supersedes" >> "$file"
  mj_ledger_append decision.recorded "\"task_id\":\"$task_id\",\"decision\":\"$(mj_json_esc "$title")\""
  printf 'recorded: %s\n' "$title"
}

# the title of every recorded decision, oldest first: the heading after its date
mj_decision_titles() {
  [ -f "$1" ] || return 0
  awk '
    /<!--/ { c=1 } /-->/ { c=0; next }
    c { next }
    /^## / { t = $0; sub(/^## [^ ]+ — /, "", t); print t }' "$1"
}

# print entries, newest first. mj_decision_entries FILE [TASK] [LIMIT]
# An entry a later one superseded says so under its heading: the file is append-only, so
# the record itself never changes, and without the mark a reader of `list` could not tell
# a replaced decision from a standing one. A later entry's `Supersedes:` is the replaced
# title; one recorded before titles were required may be any part of it.
mj_decision_entries() {
  awk -v want="${2:-}" -v limit="${3:-0}" '
    /<!--/ { c=1 } /-->/ { c=0; next }
    c { next }
    /^## / { n++; head[n]=$0; body[n]=""; task[n]=""; sup[n]=""
             t=$0; sub(/^## [^ ]+ — /, "", t); title[n]=t; next }
    n>0 && /^Task: / { task[n]=substr($0,7) }
    n>0 && /^Supersedes: / { s=substr($0,13); if (s!="-") sup[n]=s }
    n>0 { body[n]=body[n] $0 "\n" }
    END{
      for (j=1; j<=n; j++) {
        if (sup[j]=="") continue
        exact=0
        for (i=1; i<j; i++) if (title[i]==sup[j]) { by[i]=title[j]; exact=1 }
        if (!exact) for (i=1; i<j; i++) if (index(title[i], sup[j])>0) by[i]=title[j]
      }
      shown=0
      for (i=n; i>=1; i--) {
        if (want!="" && task[i]!=want) continue
        if (limit>0 && shown>=limit) break
        printf "%s\n", head[i]
        if (by[i]!="") printf "Superseded by: %s\n", by[i]
        printf "%s", body[i]
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
  # absent on a fresh clone: nothing recorded here yet, which is an answer, not a failure
  [ -f "$file" ] || { printf '(none)\n'; return 0; }
  mj_decision_entries "$file" "$want" "$limit"
}

mj_decision_show() {
  [ $# -ge 1 ] || mj_die "$MJ_EX_USAGE" "decision show: text to match is required"
  mj_require_installed
  local file; file="$(mj_decision_file)"
  [ -f "$file" ] || { mj_err "no decision matches '$1' (nothing recorded in this checkout yet)"; return "$MJ_EX_MISSING"; }
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
