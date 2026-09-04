#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_knowledge:-}" ] && return 0 || MJ_LIB_knowledge=1
# knowledge — a compiler over what this repository already states.
#
# It is not a wiki, not a database, not a memory service, and not a second place to write
# things down. Every source it reads is a file somebody already maintains; everything it
# produces is derived and regenerable, and none of it outranks the file it came from.
#
# This file currently implements the first stage, discovery. The rest of the pipeline —
# extract, resolve, graph, index, validate, query — arrives with the issues that specify
# each of them.
#
# Discovery answers one question: which files are knowledge sources, in which class, at
# which content hash. It answers it from the version-control index rather than from a
# filesystem walk, because a walk returns build output, vendored trees and untracked files,
# and returns them in an order that differs between two machines.

MJ_KSRC_FLAT=""

mj_knowledge_sources_file() { printf '%s' "$MJ_SHARE_DIR/knowledge-sources.yaml"; }

mj_ksrc_load() {
  [ -n "$MJ_KSRC_FLAT" ] && [ -f "$MJ_KSRC_FLAT" ] && return 0
  local f; f="$(mj_knowledge_sources_file)"
  [ -f "$f" ] || mj_die "$MJ_EX_INTERNAL" "knowledge source list missing: $f"
  MJ_KSRC_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ksrc.XXXXXX")"
  mj_yaml_flatten "$f" > "$MJ_KSRC_FLAT" 2>/dev/null \
    || mj_die "$MJ_EX_INTERNAL" "knowledge source list does not parse: $f"
  [ "$(mj_yget "$MJ_KSRC_FLAT" version)" = 1 ] \
    || mj_die "$MJ_EX_INTERNAL" "knowledge source list version must be 1"
}
mj_ksrc()       { mj_yget "$MJ_KSRC_FLAT" "sources.$1.$2"; }
mj_ksrc_count() { local i=0; while [ -n "$(mj_ksrc "$i" id)" ]; do i=$((i + 1)); done; printf '%s' "$i"; }

# ---------------------------------------------------------------- discovery
# Prints one tab-separated row per discovered source:
#   class <TAB> scope <TAB> kind <TAB> sha256 <TAB> repository-relative path
#
# Order is the class order of the source list, then path order within a class. Both are
# deterministic and neither depends on the filesystem: the index is already sorted, and
# the state listings are sorted here under the C collation so that two machines with
# different locales agree.
#
# Sets MJ_KDISC_EMPTY to the space-separated ids of required classes that found nothing.
MJ_KDISC_EMPTY=""
MJ_KDISC_COUNT=0
mj_knowledge_discover() {
  local want_scope="${1:-all}" i n cls kind scope disc spec req found
  mj_ksrc_load
  MJ_KDISC_EMPTY=""; MJ_KDISC_COUNT=0
  n="$(mj_ksrc_count)"; i=0
  while [ "$i" -lt "$n" ]; do
    cls="$(mj_ksrc "$i" id)"; kind="$(mj_ksrc "$i" kind)"; scope="$(mj_ksrc "$i" scope)"
    disc="$(mj_ksrc "$i" discovery)"; spec="$(mj_ksrc "$i" pathspec)"; req="$(mj_ksrc "$i" required)"
    i=$((i + 1))
    case "$want_scope" in
      all) ;;
      "$scope") ;;
      *) continue ;;
    esac
    found=0
    case "$disc" in
      vcs)   mj_kdisc_vcs   "$cls" "$kind" "$scope" "$spec" && found=1 ;;
      state) mj_kdisc_state "$cls" "$kind" "$scope" "$spec" && found=1 ;;
      *) mj_err "knowledge: source class '$cls' declares unknown discovery '$disc'"; return "$MJ_EX_INTERNAL" ;;
    esac
    # A required class that finds nothing is a finding, not a silence: the whole point of a
    # curated list is that a path can be forgotten, and a forgotten path looks exactly like
    # a repository that does not have that file.
    [ "$found" = 0 ] && [ "$req" = true ] && MJ_KDISC_EMPTY="$MJ_KDISC_EMPTY $cls"
  done
  MJ_KDISC_EMPTY="${MJ_KDISC_EMPTY# }"
  return 0
}

# One row per tracked file matching a pathspec. Read NUL-delimited, because a pathname may
# contain a space, a newline or a non-ASCII character, and the historical way to lose one
# is to let the shell split on whitespace.
mj_kdisc_vcs() {
  local cls="$1" kind="$2" scope="$3" spec="$4" p any=0
  while IFS= read -r -d '' p; do
    [ -n "$p" ] || continue
    [ -f "$MJ_ROOT/$p" ] || continue      # tracked but deleted in the working tree
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$(mj_sha256 "$MJ_ROOT/$p")" "$p"
    MJ_KDISC_COUNT=$((MJ_KDISC_COUNT + 1)); any=1
  done < <(mj_git ls-files -z -- "$spec" 2>/dev/null)
  [ "$any" = 1 ]
}

# One row per file in a state directory, or the single file itself. Operational records are
# discovered here rather than from the index because a repository may keep its state out of
# version control; nothing else on the filesystem is looked at.
mj_kdisc_state() {
  local cls="$1" kind="$2" scope="$3" spec="$4" abs f any=0
  abs="$MJ_ROOT/$spec"
  if [ -f "$abs" ]; then
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$(mj_sha256 "$abs")" "$spec"
    MJ_KDISC_COUNT=$((MJ_KDISC_COUNT + 1))
    return 0
  fi
  [ -d "$abs" ] || return 1
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    [ -f "$abs/$f" ] || continue
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$(mj_sha256 "$abs/$f")" "$spec/$f"
    MJ_KDISC_COUNT=$((MJ_KDISC_COUNT + 1)); any=1
  done < <(ls -1 "$abs" 2>/dev/null | LC_ALL=C sort)
  [ "$any" = 1 ]
}

# ---------------------------------------------------------------- command
mj_cmd_knowledge() {
  local sub="${1:-sources}"
  case "$sub" in
    --help|-h|help) mj_knowledge_usage; return 0 ;;
    sources) shift || true ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge: unknown subcommand '$sub' (see: majordomus knowledge --help)" ;;
  esac
  mj_require_installed
  mj_knowledge_sources "$@"
}

mj_knowledge_usage() {
  cat <<H
usage: majordomus knowledge <subcommand> [options]

  sources [--scope shared|operational|all] [--json]
        the curated source classes and the files each one discovers    (read-only)

  Discovery is driven by the version-control index for repository knowledge and by the
  state directories Majordomus owns for operational records. An untracked file is not a
  source. Nothing here writes anything.

  scopes: shared knowledge may be projected to a public surface; operational records are
  this checkout's own and are never part of a shared projection. Default: all.
H
}

mj_knowledge_sources() {
  local scope=all a
  while [ $# -gt 0 ]; do case "$1" in
    --scope) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--scope needs a value"; scope="$2"; shift 2 ;;
    --scope=*) scope="${1#--scope=}"; shift ;;
    --help|-h) mj_knowledge_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge sources: unknown option $1" ;;
  esac; done
  case "$scope" in shared|operational|all) ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge sources: --scope must be shared, operational or all" ;;
  esac

  local out; out="$(mktemp "${TMPDIR:-/tmp}/mj.kdi.XXXXXX")"
  mj_knowledge_discover "$scope" > "$out" || { rm -f "$out"; return "$MJ_EX_INTERNAL"; }

  if [ "$MJ_JSON" = 1 ]; then
    local first=1 c s k h p
    printf '{"schema":1,"scope":"%s","sources":[' "$scope"
    while IFS="$(printf '\t')" read -r c s k h p; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"class":"%s","scope":"%s","kind":"%s","hash":"%s","path":"%s"}' \
        "$c" "$s" "$k" "$h" "$(mj_json_esc "$p")"
    done < "$out"
    printf '],"empty_required":['
    first=1; for a in $MJ_KDISC_EMPTY; do [ "$first" = 1 ] || printf ','; printf '"%s"' "$a"; first=0; done
    printf ']}\n'
  else
    local c s k h p
    while IFS="$(printf '\t')" read -r c s k h p; do
      printf '%-11s %-11s %-10s %s  %s\n' "$c" "$s" "$k" "$(printf '%s' "$h" | cut -c1-12)" "$p"
    done < "$out"
    printf 'knowledge sources: %s file(s) in scope %s\n' "$(mj_lines "$out")" "$scope"
    for a in $MJ_KDISC_EMPTY; do
      mj_warn knowledge "$a" "required source class discovered nothing" "majordomus knowledge sources --json"
    done
  fi
  rm -f "$out"
  return 0
}
