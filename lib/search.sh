#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_search:-}" ] && return 0 || MJ_LIB_search=1
# search — find durable knowledge without reading all of it.
#
# Deterministic, literal, and transparent: a fixed-string grep over the record kinds, in a
# fixed order, printing kind, path, line and the matching line. No index, no ranking, no
# embedding. The corpus is a handful of Markdown files and one JSONL; anything cleverer
# would be a second source of truth that has to be kept in step with the first.

mj_cmd_search() {
  local term="" kinds="" want_task="" limit=40
  while [ $# -gt 0 ]; do case "$1" in
    --kind) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--kind needs a name"; kinds="$kinds $2"; shift 2 ;;
    --kind=*) kinds="$kinds ${1#--kind=}"; shift ;;
    --task) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--task needs an id"; want_task="$2"; shift 2 ;;
    --task=*) want_task="${1#--task=}"; shift ;;
    --limit) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--limit needs a number"; limit="$2"; shift 2 ;;
    --limit=*) limit="${1#--limit=}"; shift ;;
    --help|-h) cat <<H
usage: majordomus search "<text>" [--kind <handover|checkpoint|decision|question|prompt|history>]
                         [--task <id>] [--limit <n>] [--json]
  literal (not regular expression) search over durable records, in authority order
  --kind    restrict to one record kind; repeatable
  --task    only records whose task_id or entry names that task
  exit 0 with matches, 12 with none
H
      return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "search: unknown option $1" ;;
    *) [ -z "$term" ] || mj_die "$MJ_EX_USAGE" "search: one search term only (quote it)"; term="$1"; shift ;;
  esac; done
  [ -n "$term" ] || mj_die "$MJ_EX_USAGE" "search: a search term is required"
  case "$limit" in ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "search: --limit must be a number" ;; esac
  mj_require_installed
  kinds="$(printf '%s' "${kinds:-handover checkpoint decision question prompt history}" | sed 's/^ *//')"
  local k; for k in $kinds; do
    case "$k" in handover|checkpoint|decision|question|prompt|history) ;;
      *) mj_die "$MJ_EX_USAGE" "search: unknown kind '$k' (handover|checkpoint|decision|question|prompt|history)" ;;
    esac
  done

  MJ_SEARCH_HITS=0
  for k in $kinds; do
    case "$k" in
      handover)   mj_search_records handover   "$MJ_STATE_DIR/handovers"   "$term" "$want_task" "$limit" ;;
      checkpoint) mj_search_records checkpoint "$MJ_STATE_DIR/checkpoints" "$term" "$want_task" "$limit" ;;
      decision)   mj_search_lines   decision   "$MJ_STATE_DIR/decisions.md" "$term" "$want_task" "$limit" ;;
      question)   mj_search_lines   question   "$MJ_STATE_DIR/open-questions.md" "$term" "$want_task" "$limit" ;;
      prompt)     mj_search_dir     prompt     "$MJ_PROMPTS_DIR" "$term" "$limit" ;;
      history)    mj_search_lines   history    "$MJ_STATE_DIR/ledger.jsonl" "$term" "$want_task" "$limit" ;;
    esac
  done
  if [ "$MJ_SEARCH_HITS" = 0 ]; then
    [ "$MJ_JSON" = 1 ] || printf 'no match for "%s" in %s\n' "$term" "$(printf '%s' "$kinds" | tr ' ' ',')"
    exit "$MJ_EX_MISSING"
  fi
  [ "$MJ_JSON" = 1 ] || printf 'search: %s match(es)\n' "$MJ_SEARCH_HITS"
}

mj_search_emit() { # kind path line text
  MJ_SEARCH_HITS=$((MJ_SEARCH_HITS + 1))
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"kind":"%s","path":"%s","line":%s,"text":"%s"}\n' "$1" "$(mj_json_esc "$2")" "$3" "$(mj_json_esc "$4")"
  else
    printf '%-11s %s:%s  %s\n' "$1" "$2" "$3" "$4"
  fi
}

# records: one file per record, task filter reads the front matter
mj_search_records() {
  local kind="$1" dir="$2" term="$3" want="$4" limit="$5" f hit n=0
  [ -d "$dir" ] || return 0
  for f in $(ls -1 "$dir"/*.md 2>/dev/null | sort -r); do
    [ "$n" -ge "$limit" ] && break
    if [ -n "$want" ]; then grep -qx "task_id: $want" "$f" || continue; fi
    while IFS= read -r hit; do
      [ "$n" -ge "$limit" ] && break
      mj_search_emit "$kind" "${f#"$MJ_ROOT/"}" "${hit%%:*}" "$(printf '%s' "${hit#*:}" | cut -c1-120)"
      n=$((n + 1))
    done < <(grep -nF -i -- "$term" "$f" 2>/dev/null || true)
  done
}

# line-oriented stores: decisions, questions, ledger
mj_search_lines() {
  local kind="$1" file="$2" term="$3" want="$4" limit="$5" hit n=0
  [ -f "$file" ] || return 0
  while IFS= read -r hit; do
    [ "$n" -ge "$limit" ] && break
    if [ -n "$want" ]; then
      case "$hit" in *"$want"*) ;; *) continue ;; esac
    fi
    mj_search_emit "$kind" "${file#"$MJ_ROOT/"}" "${hit%%:*}" "$(printf '%s' "${hit#*:}" | cut -c1-120)"
    n=$((n + 1))
  done < <(grep -nF -i -- "$term" "$file" 2>/dev/null || true)
}

mj_search_dir() {
  local kind="$1" dir="$2" term="$3" limit="$4" f hit n=0
  [ -d "$dir" ] || return 0
  for f in $(ls -1 "$dir"/*.md 2>/dev/null | sort); do
    [ "$n" -ge "$limit" ] && break
    while IFS= read -r hit; do
      [ "$n" -ge "$limit" ] && break
      mj_search_emit "$kind" "${f#"$MJ_ROOT/"}" "${hit%%:*}" "$(printf '%s' "${hit#*:}" | cut -c1-120)"
      n=$((n + 1))
    done < <(grep -nF -i -- "$term" "$f" 2>/dev/null || true)
  done
}
