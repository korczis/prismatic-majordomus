#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_question:-}" ] && return 0 || MJ_LIB_question=1
# question — open questions as explicit state instead of prose buried in a handover.
#
# One mutable index: .majordomus/state/open-questions.md. Resolving edits its line, because
# an index of what is still open must not accumulate; the append-only record of opening and
# resolving lives in the ledger. Any unresolved entry for the active task blocks
# `finish --outcome completed`, which is why this file has a machine-written line format.

mj_cmd_question() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    add) mj_question_add "$@" ;;
    resolve) mj_question_resolve "$@" ;;
    list) mj_question_list "$@" ;;
    --help|-h|"") cat <<H
usage: majordomus question add "<question>"
       majordomus question resolve <n|"<text>"> --answer "<answer>"
       majordomus question list [--all] [--task <id>]
  add       appends "- [unresolved] <task id> — <question> (<date>)" to open-questions.md
  resolve   rewrites that one line to "[resolved <date>]" with the answer; n is the number
            printed by \`question list\` for the active task
  list      unresolved entries for the active task; --all includes resolved and other tasks
  every unresolved entry for the active task refuses \`finish --outcome completed\`
H
      [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "question: unknown subcommand '$sub' (add|resolve|list)" ;;
  esac
}

mj_question_file() { printf '%s' "$MJ_DIR/state/open-questions.md"; }
mj_question_require() {
  mj_require_installed
  MJ_Q="$(mj_question_file)"
  [ -f "$MJ_Q" ] || mj_die "$MJ_EX_MISSING" "no $MJ_Q (run: majordomus init)"
}

mj_question_add() {
  local text=""
  while [ $# -gt 0 ]; do case "$1" in
    -*) mj_die "$MJ_EX_USAGE" "question add: unknown option $1" ;;
    *) [ -z "$text" ] || mj_die "$MJ_EX_USAGE" "question add: the question must be one argument (quote it)"; text="$1"; shift ;;
  esac; done
  [ -n "$text" ] || mj_die "$MJ_EX_USAGE" "question add: the question text is required"
  mj_is_multiline "$text" && mj_die "$MJ_EX_USAGE" "question add: the question must be single-line" 
  case "$text" in *" — "*) mj_die "$MJ_EX_USAGE" "question add: the question must not contain ' — '; it separates the fields" ;; esac
  mj_question_require
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task; a question is opened against a task"
  local id; id="$(mj_cur id)"
  printf -- '- [unresolved] %s — %s (%s)\n' "$id" "$text" "$(date -u +%Y-%m-%d)" >> "$MJ_Q"
  mj_ledger_append question.opened "\"task_id\":\"$id\",\"question\":\"$(mj_json_esc "$text")\""
  printf 'opened for %s: %s\n' "$id" "$text"
}

# unresolved lines for a task, in file order
mj_question_unresolved() { grep -nE "^- \[unresolved\] $1 " "$2" 2>/dev/null || true; }

mj_question_resolve() {
  local sel="" answer=""
  while [ $# -gt 0 ]; do case "$1" in
    --answer) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--answer needs text"; answer="$2"; shift 2 ;;
    --answer=*) answer="${1#--answer=}"; shift ;;
    -*) mj_die "$MJ_EX_USAGE" "question resolve: unknown option $1" ;;
    *) [ -z "$sel" ] || mj_die "$MJ_EX_USAGE" "question resolve: one selector only"; sel="$1"; shift ;;
  esac; done
  [ -n "$sel" ] || mj_die "$MJ_EX_USAGE" "question resolve: a number or matching text is required"
  [ -n "$answer" ] || mj_die "$MJ_EX_USAGE" "question resolve: --answer is required (resolving without an answer loses the answer)"
  mj_is_multiline "$answer" && mj_die "$MJ_EX_USAGE" "question resolve: the answer must be single-line" 
  mj_question_require
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task; a question is resolved against a task"
  local id; id="$(mj_cur id)"

  local open; open="$(mj_question_unresolved "$id" "$MJ_Q")"
  [ -n "$open" ] || mj_die "$MJ_EX_MISSING" "no unresolved question for $id"
  local hits line_no
  case "$sel" in
    ''|*[!0-9]*) hits="$(printf '%s\n' "$open" | grep -F -- "$sel" || true)" ;;
    *) hits="$(printf '%s\n' "$open" | sed -n "${sel}p")" ;;
  esac
  [ -n "$hits" ] || mj_die "$MJ_EX_MISSING" "question resolve: '$sel' matches no unresolved question for $id"
  [ "$(printf '%s\n' "$hits" | wc -l | tr -d ' ')" = 1 ] || mj_die "$MJ_EX_USAGE" "question resolve: '$sel' matches more than one question; be more specific"
  line_no="${hits%%:*}"

  local tmp; tmp="$(mktemp "${TMPDIR:-/tmp}/mj.q.XXXXXX")"
  awk -v n="$line_no" -v d="$(date -u +%Y-%m-%d)" -v a="$answer" '
    NR==n { sub(/^- \[unresolved\]/, "- [resolved " d "]"); print $0 " — " a; next }
    { print }' "$MJ_Q" > "$tmp" && mv "$tmp" "$MJ_Q"
  local text; text="$(printf '%s' "$hits" | sed -e 's/^[0-9]*:- \[unresolved\] [^ ]* — //' -e 's/ ([0-9-]*)$//')"
  mj_ledger_append question.resolved "\"task_id\":\"$id\",\"question\":\"$(mj_json_esc "$text")\",\"answer\":\"$(mj_json_esc "$answer")\""
  printf 'resolved for %s: %s\n' "$id" "$text"
}

mj_question_list() {
  local all=0 want=""
  while [ $# -gt 0 ]; do case "$1" in
    --all) all=1; shift ;;
    --task) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--task needs an id"; want="$2"; shift 2 ;;
    --task=*) want="${1#--task=}"; shift ;;
    *) mj_die "$MJ_EX_USAGE" "question list: unknown option $1" ;;
  esac; done
  mj_question_require
  if [ -z "$want" ] && [ "$all" != 1 ]; then
    mj_load_current || mj_die "$MJ_EX_MISSING" "no active task; use --all or --task <id>"
    want="$(mj_cur id)"
  fi
  local n=0 line
  if [ "$all" = 1 ] && [ -z "$want" ]; then
    # the file's own template lives in an HTML comment and is not an entry
    while IFS= read -r line; do n=$((n+1)); printf '%s\n' "$line"; done < <(awk '/<!--/{c=1} /-->/{c=0;next} !c && /^- \[(unresolved|resolved )/' "$MJ_Q" 2>/dev/null || true)
  else
    while IFS= read -r line; do n=$((n+1)); printf '%s  %s\n' "$n" "$(printf '%s' "$line" | sed 's/^- //')"; done < <(mj_question_unresolved "$want" "$MJ_Q" | sed 's/^[0-9]*://')
    if [ "$all" = 1 ]; then grep -E "^- \[resolved [0-9-]+\] $want " "$MJ_Q" 2>/dev/null | sed 's/^- /-  /' && n=$((n+1)); fi
  fi
  [ "$n" = 0 ] && printf 'no open questions%s\n' "${want:+ for $want}"
  return 0
}

# line numbers of entries that look like questions but do not parse. A gate that cannot
# read an entry is a gate that can be bypassed by mistyping one, so this is a failure,
# not a warning. Lines inside an HTML comment are the file's own documented example.
mj_question_malformed() {
  [ -f "$1" ] || return 0
  awk '
    /<!--/ { c=1 } /-->/ { c=0; next }
    c { next }
    /^- \[/ {
      if ($0 ~ /^- \[unresolved\] [^ ]+ — .+ \([0-9]{4}-[0-9]{2}-[0-9]{2}\)$/) next
      if ($0 ~ /^- \[resolved [0-9]{4}-[0-9]{2}-[0-9]{2}\] [^ ]+ — .+$/) next
      printf "%s ", NR
    }' "$1"
}
