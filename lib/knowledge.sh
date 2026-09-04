#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_knowledge:-}" ] && return 0 || MJ_LIB_knowledge=1
# knowledge — a compiler over what this repository already states.
#
# It is not a wiki, not a database, not a memory service, and not a second place to write
# things down. Every source it reads is a file somebody already maintains; everything it
# produces is derived and regenerable, and none of it outranks the file it came from.
#
# Which files are sources is declared twice, by two owners: the repository declares its
# shared knowledge in its AI layer, the tool declares the operational records it writes.
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

# Two declarations, one list. The repository's AI layer declares the shared sources — the
# tracked files that are repository knowledge — under its knowledge section; the tool ships
# the operational classes, the records it writes itself under the state directory. The
# repository's classes come first, and the scope is decided by which file declared a class:
# shared for the repository's, operational for the tool's. Neither file names the other.
mj_knowledge_repo_sources()  { printf '%s' "$MJ_KNOWLEDGE_DIR/sources.yaml"; }
mj_knowledge_state_sources() { printf '%s' "$MJ_SHARE_DIR/knowledge-sources.yaml"; }

mj_ksrc_load() {
  [ -n "$MJ_KSRC_FLAT" ] && [ -f "$MJ_KSRC_FLAT" ] && return 0
  local f n=0 tmp
  MJ_KSRC_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ksrc.XXXXXX")"
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ksr.XXXXXX")"
  for f in "$(mj_knowledge_repo_sources)" "$(mj_knowledge_state_sources)"; do
    [ -f "$f" ] || { [ "$f" = "$(mj_knowledge_state_sources)" ] && mj_die "$MJ_EX_INTERNAL" "knowledge source list missing: $f"; continue; }
    mj_yaml_flatten "$f" > "$tmp" 2>/dev/null || mj_die "$MJ_EX_CONTRACT" "knowledge source list does not parse: $(mj_rel "$f")"
    [ "$(mj_yget "$tmp" version)" = 1 ] || mj_die "$MJ_EX_CONTRACT" "knowledge source list version must be 1: $(mj_rel "$f")"
    # renumber this file's classes after the ones already loaded, and stamp the scope
    local scope=operational i=0
    [ "$f" = "$(mj_knowledge_repo_sources)" ] && scope=shared
    while [ -n "$(mj_yget "$tmp" "sources.$i.id")" ]; do
      sed -n "s/^sources\.$i\./sources.$n./p" "$tmp" >> "$MJ_KSRC_FLAT"
      printf 'sources.%s.scope=%s\n' "$n" "$scope" >> "$MJ_KSRC_FLAT"
      i=$((i + 1)); n=$((n + 1))
    done
  done
  rm -f "$tmp"
  return 0
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
# discovered here rather than from the index because the state directory is never tracked;
# the pathspec is relative to it, and nothing else on the filesystem is looked at.
mj_kdisc_state() {
  local cls="$1" kind="$2" scope="$3" spec="$4" abs f any=0 rel
  abs="$MJ_STATE_DIR/$spec"; rel="$(mj_rel "$MJ_STATE_DIR")/$spec"
  if [ -f "$abs" ]; then
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$(mj_sha256 "$abs")" "$rel"
    MJ_KDISC_COUNT=$((MJ_KDISC_COUNT + 1))
    return 0
  fi
  [ -d "$abs" ] || return 1
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    [ -f "$abs/$f" ] || continue
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$(mj_sha256 "$abs/$f")" "$rel/$f"
    MJ_KDISC_COUNT=$((MJ_KDISC_COUNT + 1)); any=1
  done < <(ls -1 "$abs" 2>/dev/null | LC_ALL=C sort)
  [ "$any" = 1 ]
}

# ---------------------------------------------------------------- command
mj_cmd_knowledge() {
  local sub="${1:-sources}"
  case "$sub" in
    --help|-h|help) mj_knowledge_usage; return 0 ;;
    sources|nodes|edges) shift || true ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge: unknown subcommand '$sub' (see: majordomus knowledge --help)" ;;
  esac
  mj_require_installed
  case "$sub" in
    sources) mj_knowledge_sources "$@" ;;
    nodes)   mj_knowledge_nodes_cmd "$@" ;;
    edges)   mj_knowledge_edges_cmd "$@" ;;
  esac
}

mj_knowledge_usage() {
  cat <<H
usage: majordomus knowledge <subcommand> [options]

  sources [--scope shared|operational|all] [--json]
        the curated source classes and the files each one discovers    (read-only)
  nodes [--scope shared|operational|all] [--kind <k>] [--json]
        one node per canonical object, with its identity and kind      (read-only)
  edges [--scope ...] [--type <t>] [--json]
        one edge per stated relationship, with where it was observed   (read-only)

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

# ---------------------------------------------------------------- extraction
# Turn the discovered sources into the rows lib/knowledge.awk reads. This function does the
# reading; it decides nothing. What a node is, what its id is and what kind it carries are
# all settled in the awk, so there is one implementation of those semantics rather than one
# here and another in whatever calls it next.
#
# Structured sources are flattened through the same restricted YAML subset the policy and
# the project model use, so a file that this tool refuses everywhere else is refused here
# too rather than being parsed by a second, laxer reader.
mj_knowledge_rows() {
  local src="$1" cls scope kind hash path abs flat
  # Every tracked path, so that a link can be told apart three ways: it resolves to a node,
  # it resolves to a real file this compiler does not model, or it resolves to nothing. The
  # list comes from the index rather than from the filesystem, for the same reason discovery
  # does — repository truth, and no untracked file mistaken for a valid target.
  mj_git ls-files -z 2>/dev/null | tr '\0' '\n' | awk 'NF { printf "T\t%s\n", $0 }'
  while IFS="$(printf '\t')" read -r cls scope kind hash path; do
    [ -n "$path" ] || continue
    abs="$MJ_ROOT/$path"
    printf 'S\t%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$hash" "$path"
    case "$kind" in
      decision|question)
        # Line-oriented stores: their entries are not YAML and are not pretended to be.
        awk -v p="$path" '{ gsub(/\t/, " "); printf "L\t%s\t%s\t%s\n", p, NR, $0 }' "$abs"
        ;;
      document)
        # Two things are taken from a document body, and only these two, because only these
        # two state a fact ABOUT the document rather than in it: its first level-one heading,
        # and the inline links its author wrote.
        awk -v p="$path" '/^# / { t = substr($0, 3); gsub(/\t/, " ", t); printf "D\t%s\t%s\n", p, t; exit }' "$abs"
        mj_knowledge_links "$path" "$abs"
        ;;
      implementation|test)
        # Code and cases are nodes so that the chain a claim declares resolves, and that is
        # all they are. Nothing reads their contents: a leading comment is a good summary and
        # reading it would be reading prose, which is the line this compiler does not cross.
        ;;
      session|handover|checkpoint|prompt|rule)
        # Records, prompt assets and rule objects carry YAML front matter and an authored
        # body. Only the front matter is a source of facts.
        flat="$(mktemp "${TMPDIR:-/tmp}/mj.kf.XXXXXX")"
        mj_record_front "$abs" > "$flat" 2>/dev/null || : > "$flat"
        mj_knowledge_flat_rows "$path" "$flat"
        rm -f "$flat"
        ;;
      policy|profile|milestone|issue|claim|doctrine)
        mj_knowledge_flat_rows "$path" "$abs"
        ;;
      *)
        # A kind this reader has no rule for gets no content rows at all. Sending it to the
        # YAML flattener anyway was the shell guessing: a Markdown file handed to the
        # structured reader produced "does not parse as the restricted YAML subset", which
        # is a complaint about a file that was never claimed to be YAML. The extractor emits
        # an `unknown` node for it and says so once, which is the honest report.
        ;;
    esac
  done < "$src"
}

# Inline links, and only inline links: `[text](target)` as the author wrote it.
#
# Fenced code is dropped first. A path inside a code sample is an example of a path, not a
# reference to one, and letting one become an edge fills the graph with relationships nobody
# asserted. Reference-style links and bare URLs are left alone: the first is not resolvable
# without a second pass over the file, and the second is not a repository reference.
#
# A target with a scheme, a protocol-relative target and a bare anchor are all skipped —
# none of them names a file in this repository. A fragment on a real path is trimmed,
# because `docs/CLI.md#session` is a reference to `docs/CLI.md`.
mj_knowledge_links() {
  local path="$1" file="$2"
  awk -v p="$path" '
    /^[ \t]*(```|~~~)/ { fence = !fence; next }
    fence { next }
    {
      line = $0
      while (match(line, /\[[^]]*\]\([^)]+\)/)) {
        chunk = substr(line, RSTART, RLENGTH)
        line = substr(line, RSTART + RLENGTH)
        t = chunk
        sub(/^\[[^]]*\]\(/, "", t); sub(/\)$/, "", t)
        sub(/[ \t].*$/, "", t)              # a link title after the target
        sub(/#.*$/, "", t)                  # a fragment names a place in a file, not a file
        if (t == "") continue
        if (t ~ /^[a-zA-Z][a-zA-Z0-9+.-]*:/) continue   # any scheme
        if (t ~ /^\/\//) continue                       # protocol relative
        if (t ~ /^\//) continue                         # absolute: not a repository path
        gsub(/\t/, " ", t)
        printf "K\t%s\t%s\t%s\n", p, NR, t
      }
    }' "$file"
}

# Flatten one file and emit its keys. A file that does not parse is reported and skipped:
# one malformed input must not cost the whole build, and it must not be silently absent
# either.
mj_knowledge_flat_rows() {
  local path="$1" file="$2" flat
  flat="$(mktemp "${TMPDIR:-/tmp}/mj.kn.XXXXXX")"
  if mj_yaml_flatten "$file" > "$flat" 2>/dev/null; then
    awk -F= -v p="$path" '{ k = $1; sub(/^[^=]*=/, "", $0); gsub(/\t/, " ", $0)
                            printf "F\t%s\t%s\t%s\n", p, k, $0 }' "$flat"
  else
    printf 'X\tWARN\tunparsed_source\t%s\tdoes not parse as the restricted YAML subset; no node was extracted from it\n' "$path"
  fi
  rm -f "$flat"
}

# The node set, sorted. Sorting happens here rather than in the awk because awk has no
# portable sort, and it is done under the C collation so that two machines with different
# locales produce the same bytes.
mj_knowledge_nodes() {
  local scope="${1:-all}" disc rows
  disc="$(mktemp "${TMPDIR:-/tmp}/mj.kd.XXXXXX")"
  rows="$(mktemp "${TMPDIR:-/tmp}/mj.kr.XXXXXX")"
  mj_knowledge_discover "$scope" > "$disc" || { rm -f "$disc" "$rows"; return "$MJ_EX_INTERNAL"; }
  mj_knowledge_rows "$disc" > "$rows"
  awk -f "$MJ_LIB_DIR/knowledge.awk" "$rows" | LC_ALL=C sort
  rm -f "$disc" "$rows"
}

# The nodes subcommand. Read-only: it derives on demand and writes nothing, because a query
# that rebuilds an index as a side effect is a write wearing a read's name.
mj_knowledge_nodes_cmd() {
  local scope=all want_kind="" out n=0 fails=0
  while [ $# -gt 0 ]; do case "$1" in
    --scope) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--scope needs a value"; scope="$2"; shift 2 ;;
    --scope=*) scope="${1#--scope=}"; shift ;;
    --kind) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--kind needs a value"; want_kind="$2"; shift 2 ;;
    --kind=*) want_kind="${1#--kind=}"; shift ;;
    --help|-h) mj_knowledge_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge nodes: unknown option $1" ;;
  esac; done
  case "$scope" in shared|operational|all) ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge nodes: --scope must be shared, operational or all" ;;
  esac

  out="$(mktemp "${TMPDIR:-/tmp}/mj.kno.XXXXXX")"
  mj_knowledge_nodes "$scope" > "$out" || { rm -f "$out"; return "$MJ_EX_INTERNAL"; }

  local t a b c d e g first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"scope":"%s","nodes":[' "$scope"
    while IFS="$(printf '\t')" read -r t a b c d e g; do
      [ "$t" = N ] || continue
      [ -n "$want_kind" ] && [ "$b" != "$want_kind" ] && continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"id":"%s","kind":"%s","scope":"%s","source":"%s","hash":"%s","title":"%s"}' \
        "$(mj_json_esc "$a")" "$b" "$c" "$(mj_json_esc "$d")" "$e" "$(mj_json_esc "$g")"
    done < "$out"
    printf '],"findings":['
    first=1
    while IFS="$(printf '\t')" read -r t a b c d; do
      [ "$t" = X ] || continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"level":"%s","code":"%s","subject":"%s","message":"%s"}' \
        "$a" "$b" "$(mj_json_esc "$c")" "$(mj_json_esc "$d")"
    done < "$out"
    printf ']}\n'
  else
    while IFS="$(printf '\t')" read -r t a b c d e g; do
      [ "$t" = N ] || continue
      [ -n "$want_kind" ] && [ "$b" != "$want_kind" ] && continue
      printf '%-11s %-11s %s  %-52s %s\n' "$b" "$c" "$(printf '%s' "$e" | cut -c1-12)" "$a" "$g"
      n=$((n + 1))
    done < "$out"
    printf 'knowledge nodes: %s in scope %s%s\n' "$n" "$scope" "${want_kind:+, kind $want_kind}"
    while IFS="$(printf '\t')" read -r t a b c d; do
      [ "$t" = X ] || continue
      case "$a" in FAIL) mj_fail knowledge "$c" "$d" "majordomus knowledge nodes --json"; fails=$((fails + 1)) ;;
                   *)    mj_warn knowledge "$c" "$d" "majordomus knowledge nodes --json" ;; esac
    done < "$out"
  fi
  rm -f "$out"
  [ "$fails" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}

# The edges subcommand. Every row carries the file, and where there is one the field or
# line, in which the relationship was observed. Nothing here is inferred.
mj_knowledge_edges_cmd() {
  local scope=all want_type="" out n=0 fails=0
  while [ $# -gt 0 ]; do case "$1" in
    --scope) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--scope needs a value"; scope="$2"; shift 2 ;;
    --scope=*) scope="${1#--scope=}"; shift ;;
    --type) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--type needs a value"; want_type="$2"; shift 2 ;;
    --type=*) want_type="${1#--type=}"; shift ;;
    --help|-h) mj_knowledge_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge edges: unknown option $1" ;;
  esac; done
  case "$scope" in shared|operational|all) ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge edges: --scope must be shared, operational or all" ;;
  esac

  out="$(mktemp "${TMPDIR:-/tmp}/mj.kge.XXXXXX")"
  mj_knowledge_nodes "$scope" > "$out" || { rm -f "$out"; return "$MJ_EX_INTERNAL"; }

  local t a b c d first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"scope":"%s","edges":[' "$scope"
    while IFS="$(printf '\t')" read -r t a b c d; do
      [ "$t" = E ] || continue
      [ -n "$want_type" ] && [ "$c" != "$want_type" ] && continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"from":"%s","to":"%s","type":"%s","provenance":"%s"}' \
        "$(mj_json_esc "$a")" "$(mj_json_esc "$b")" "$c" "$(mj_json_esc "$d")"
    done < "$out"
    printf ']}\n'
  else
    while IFS="$(printf '\t')" read -r t a b c d; do
      [ "$t" = E ] || continue
      [ -n "$want_type" ] && [ "$c" != "$want_type" ] && continue
      printf '%-15s %-46s %-46s %s\n' "$c" "$a" "$b" "$d"
      n=$((n + 1))
    done < "$out"
    printf 'knowledge edges: %s in scope %s%s\n' "$n" "$scope" "${want_type:+, type $want_type}"
    while IFS="$(printf '\t')" read -r t a b c d; do
      [ "$t" = X ] || continue
      case "$a" in FAIL) mj_fail knowledge "$c" "$d" "majordomus knowledge edges --json"; fails=$((fails + 1)) ;;
                   *)    mj_warn knowledge "$c" "$d" "majordomus knowledge edges --json" ;; esac
    done < "$out"
  fi
  rm -f "$out"
  [ "$fails" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}
