#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
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
# This file implements discovery, extraction and the graph over the sources, and — since
# ADR 0058 — the one writer the kind has: the deriver that turns the ledger lines an
# episode stamped into candidate records under the tracked tree, the acts that promote or
# reject a candidate, and the check that every record is evidenced and well-formed.
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
  # the classes become variables once; every field read below is an expansion
  mj_yload "$MJ_KSRC_FLAT" ksrc; MJ_KSRC_COUNT="$n"
  return 0
}
MJ_KSRC_COUNT=0
mj_ksrc()       { mj_yv ksrc "sources.$1.$2"; }
mj_ksrc_count() { printf '%s' "$MJ_KSRC_COUNT"; }

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
  # every class lists its files into one buffer; one hashing process then covers all of
  # them, and the rows come out in the order the classes produced them
  local buf; buf="$(mktemp "${TMPDIR:-/tmp}/mj.kdb.XXXXXX")"
  mj_kdisc_collect "$want_scope" > "$buf" || { rm -f "$buf"; return "$MJ_EX_INTERNAL"; }
  if [ -s "$buf" ]; then
    awk -F'\t' '{ printf "%s%c", $4, 0 }' "$buf" | mj_sha256_many > "$buf.hash"
    awk -F'\t' 'NR == FNR { h[$2] = $1; next } { printf "%s\t%s\t%s\t%s\t%s\n", $1, $2, $3, h[$4], $5 }' "$buf.hash" "$buf"
  fi
  rm -f "$buf" "$buf.hash"
  return 0
}
mj_kdisc_collect() {
  local want_scope="$1" i n cls kind scope disc spec req found
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
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$MJ_ROOT/$p" "$p"
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
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$abs" "$rel"
    MJ_KDISC_COUNT=$((MJ_KDISC_COUNT + 1))
    return 0
  fi
  [ -d "$abs" ] || return 1
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    [ -f "$abs/$f" ] || continue
    printf '%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$abs/$f" "$rel/$f"
    MJ_KDISC_COUNT=$((MJ_KDISC_COUNT + 1)); any=1
  done < <(ls -1 "$abs" 2>/dev/null | LC_ALL=C sort)
  [ "$any" = 1 ]
}

# ---------------------------------------------------------------- command
mj_cmd_knowledge() {
  local sub="${1:-sources}"
  case "$sub" in
    --help|-h|help) mj_knowledge_usage; return 0 ;;
    sources|nodes|edges|derive|candidates|promote|reject|check) shift || true ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge: unknown subcommand '$sub' (see: majordomus knowledge --help)" ;;
  esac
  mj_require_installed
  case "$sub" in
    sources)    mj_knowledge_sources "$@" ;;
    nodes)      mj_knowledge_nodes_cmd "$@" ;;
    edges)      mj_knowledge_edges_cmd "$@" ;;
    derive)     mj_knowledge_derive_cmd "$@" ;;
    candidates) mj_knowledge_candidates_cmd "$@" ;;
    promote)    mj_knowledge_promote_cmd "$@" ;;
    reject)     mj_knowledge_reject_cmd "$@" ;;
    check)      mj_knowledge_check_cmd "$@" ;;
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
  derive [--episode <id>] [--dry-run] [--json]
        write what one episode's ledger lines state into candidate records under
        the knowledge section; deterministic, idempotent, no model
  candidates [--json]
        the records awaiting review, with the branch of the episode each came from (read-only)
  promote <id> [--class <fact|convention|constraint|memory|lesson>] < evidence.md
        move a candidate to curated/ as verified; the evidence on stdin is required
  reject <id> --reason "<why>" [--by <id>]
        supersede a candidate in place, recording why and what replaces it
  check [--json]
        every record is schema-valid, evidenced and resolves what it names  (read-only)

  Discovery is driven by the version-control index for repository knowledge and by the
  state directories Majordomus owns for operational records. An untracked file is not a
  source. derive, promote and reject write under the knowledge section and append to the
  ledger; nothing is ever staged.

  scopes: shared knowledge may be projected to a public surface; operational records are
  this checkout's own and are never part of a shared projection. Default: all.

  The executable answers the read side over every surface: majordomus-cli knowledge
  candidates, knowledge record <id>, knowledge status.
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
    while IFS="$MJ_TAB" read -r c s k h p; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"class":"%s","scope":"%s","kind":"%s","hash":"%s","path":"%s"}' \
        "$c" "$s" "$k" "$h" "$(mj_json_esc "$p")"
    done < "$out"
    printf '],"empty_required":['
    first=1; for a in $MJ_KDISC_EMPTY; do [ "$first" = 1 ] || printf ','; printf '"%s"' "$a"; first=0; done
    printf ']}\n'
  else
    local c s k h p
    while IFS="$MJ_TAB" read -r c s k h p; do
      printf '%-11s %-11s %-10s %s  %s\n' "$c" "$s" "$k" "${h:0:12}" "$p"
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
  local src="$1" cls scope kind hash path abs tab
  tab="$(printf '\t')"
  # Every tracked path, so that a link can be told apart three ways: it resolves to a node,
  # it resolves to a real file this compiler does not model, or it resolves to nothing. The
  # list comes from the index rather than from the filesystem, for the same reason discovery
  # does — repository truth, and no untracked file mistaken for a valid target.
  mj_git ls-files -z 2>/dev/null | tr '\0' '\n' | awk 'NF { printf "T\t%s\n", $0 }'
  # One source row per discovered file, then the content rows by kind, each kind's files
  # read by one process: the YAML kinds and the front-matter kinds flattened by one awk
  # each, the documents' headings and links by one awk, the line stores by one awk. The
  # extractor keys every content row by its path, so the grouping changes nothing it sees.
  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.kn.XXXXXX")"
  mkdir -p "$tmp/yaml" "$tmp/front"
  : > "$tmp/yaml.map"; : > "$tmp/front.map"; : > "$tmp/docs"; : > "$tmp/lines"; : > "$tmp/ctx"
  local ny=0 nf=0
  while IFS="$tab" read -r cls scope kind hash path; do
    [ -n "$path" ] || continue
    abs="$MJ_ROOT/$path"
    # A section's README declares itself a context document, whatever class discovered it
    # (share/kinds.yaml: `declared: [context]`). It is read as the kind it declares, not as
    # an instance of the kind that lives beside it, which would be a node nobody meant.
    #
    # It is also one object however many classes reach it. The context class discovers
    # every README of the layer, and the class that lives beside the file — applications/,
    # rules/, knowledge/candidates/ — reaches the same README through its own glob. Both
    # rows arrive here as the one document, so the first is the source and a later one is
    # the same file seen again, not a second claim: passing it on would report the document
    # as claimed by itself. Two classes claiming one file as their own kind still collide
    # in the extractor, which is the failure test 73 keeps.
    if mj_is_context_doc "$abs"; then
      grep -qxF -- "$path" "$tmp/ctx" && continue
      printf '%s\n' "$path" >> "$tmp/ctx"
      kind=document
    fi
    printf 'S\t%s\t%s\t%s\t%s\t%s\n' "$cls" "$scope" "$kind" "$hash" "$path"
    case "$kind" in
      decision|question) printf '%s\n' "$abs" >> "$tmp/lines" ;;
      document) printf '%s\n' "$abs" >> "$tmp/docs" ;;
      implementation|test) ;;
      # a knowledge record declares its identity and its provenance in front matter, so it
      # is read the way a rule or an ADR is: the node is knowledge:<id>, and every
      # derived_from and relation it states becomes an edge (ADR 0058). The section's
      # README is a context document, reclassified above, and is still read as prose.
      session|handover|checkpoint|prompt|rule|adr|skill|use-case|application|knowledge) nf=$((nf + 1)); printf '%s\t%s\n' "$nf" "$path" >> "$tmp/front.map"; printf '%s\n' "$abs" >> "$tmp/front.list" ;;
      policy|scope|profile|milestone|issue|claim|doctrine) ny=$((ny + 1)); printf '%s\t%s\n' "$ny" "$path" >> "$tmp/yaml.map"; printf '%s\n' "$abs" >> "$tmp/yaml.list" ;;
      *) ;;   # a kind this reader has no rule for gets no content rows; the extractor says so once
    esac
  done < "$src"
  local group
  for group in yaml front; do
    [ -s "$tmp/$group.list" ] || continue
    set -- ; while IFS= read -r abs; do set -- "$@" "$abs"; done < "$tmp/$group.list"
    if [ "$group" = front ]; then mj_yaml_flatten_many "$tmp/front" --numbered --front "$@"
    else mj_yaml_flatten_many "$tmp/yaml" --numbered "$@"; fi
    # a file that does not parse has no rows and one warning; a record without front
    # matter is not an error here, its flat is simply empty, as mj_record_front left it
    awk -F'\t' -v map="$tmp/$group.map" -v errs="$tmp/$group/.errors" -v front="$([ "$group" = front ] && echo 1 || echo 0)" '
      BEGIN { while ((getline l < map) > 0) { split(l, a, "\t"); path[a[1]] = a[2] }; close(map)
              while ((getline l < errs) > 0) { split(l, a, "\t"); bad[a[1]] = a[2] }; close(errs)
              for (n in path) if (n in bad && !(front && bad[n] == "no front matter"))
                printf "X\tWARN\tunparsed_source\t%s\tdoes not parse as the restricted YAML subset; no node was extracted from it\n", path[n] }
      FNR == 1 { n = FILENAME; sub(/.*\//, "", n); p = path[n] }
      n in bad { next }
      { eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1); gsub(/\t/, " ", v); printf "F\t%s\t%s\t%s\n", p, k, v }' \
      "$tmp/$group"/[0-9]*
  done
  if [ -s "$tmp/docs" ]; then
    set -- ; while IFS= read -r abs; do set -- "$@" "$abs"; done < "$tmp/docs"
    awk -v root="$MJ_ROOT/" '
      FNR == 1 { p = FILENAME; sub("^" root, "", p); fence = 0; seen = 0 }
      /^[ \t]*(```|~~~)/ { fence = !fence; next }
      fence { next }
      /^# / && !seen { t = substr($0, 3); gsub(/\t/, " ", t); printf "D\t%s\t%s\n", p, t; seen = 1 }
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
          printf "K\t%s\t%s\t%s\n", p, FNR, t
        }
      }' "$@"
  fi
  if [ -s "$tmp/lines" ]; then
    set -- ; while IFS= read -r abs; do set -- "$@" "$abs"; done < "$tmp/lines"
    awk -v root="$MJ_ROOT/" 'FNR == 1 { p = FILENAME; sub("^" root, "", p) } { gsub(/\t/, " "); printf "L\t%s\t%s\t%s\n", p, FNR, $0 }' "$@"
  fi
  rm -rf "$tmp"
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
  local t0
  t0="$(mj_phase_begin knowledge:discover)"
  mj_knowledge_discover "$scope" > "$disc" || { rm -f "$disc" "$rows"; return "$MJ_EX_INTERNAL"; }
  mj_phase_end knowledge:discover "$t0"; t0="$(mj_phase_begin knowledge:rows)"
  mj_knowledge_rows "$disc" > "$rows"
  mj_phase_end knowledge:rows "$t0"; t0="$(mj_phase_begin knowledge:extract)"
  awk -f "$MJ_LIB_DIR/knowledge.awk" "$rows" | LC_ALL=C sort
  mj_phase_end knowledge:extract "$t0"
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
  local t0; t0="$(mj_phase_begin knowledge:format)"

  local t a b c d e g first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"scope":"%s","nodes":[' "$scope"
    while IFS="$MJ_TAB" read -r t a b c d e g; do
      [ "$t" = N ] || continue
      [ -n "$want_kind" ] && [ "$b" != "$want_kind" ] && continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"id":"%s","kind":"%s","scope":"%s","source":"%s","hash":"%s","title":"%s"}' \
        "$(mj_json_esc "$a")" "$b" "$c" "$(mj_json_esc "$d")" "$e" "$(mj_json_esc "$g")"
    done < "$out"
    printf '],"findings":['
    first=1
    while IFS="$MJ_TAB" read -r t a b c d; do
      [ "$t" = X ] || continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"level":"%s","code":"%s","subject":"%s","message":"%s"}' \
        "$a" "$b" "$(mj_json_esc "$c")" "$(mj_json_esc "$d")"
    done < "$out"
    printf ']}\n'
  else
    while IFS="$MJ_TAB" read -r t a b c d e g; do
      [ "$t" = N ] || continue
      [ -n "$want_kind" ] && [ "$b" != "$want_kind" ] && continue
      printf '%-11s %-11s %s  %-52s %s\n' "$b" "$c" "${e:0:12}" "$a" "$g"
      n=$((n + 1))
    done < "$out"
    printf 'knowledge nodes: %s in scope %s%s\n' "$n" "$scope" "${want_kind:+, kind $want_kind}"
    while IFS="$MJ_TAB" read -r t a b c d; do
      [ "$t" = X ] || continue
      case "$a" in FAIL) mj_fail knowledge "$c" "$d" "majordomus knowledge nodes --json"; fails=$((fails + 1)) ;;
                   *)    mj_warn knowledge "$c" "$d" "majordomus knowledge nodes --json" ;; esac
    done < "$out"
  fi
  rm -f "$out"
  mj_phase_end knowledge:format "$t0"
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
    while IFS="$MJ_TAB" read -r t a b c d; do
      [ "$t" = E ] || continue
      [ -n "$want_type" ] && [ "$c" != "$want_type" ] && continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"from":"%s","to":"%s","type":"%s","provenance":"%s"}' \
        "$(mj_json_esc "$a")" "$(mj_json_esc "$b")" "$c" "$(mj_json_esc "$d")"
    done < "$out"
    printf ']}\n'
  else
    while IFS="$MJ_TAB" read -r t a b c d; do
      [ "$t" = E ] || continue
      [ -n "$want_type" ] && [ "$c" != "$want_type" ] && continue
      printf '%-15s %-46s %-46s %s\n' "$c" "$a" "$b" "$d"
      n=$((n + 1))
    done < "$out"
    printf 'knowledge edges: %s in scope %s%s\n' "$n" "$scope" "${want_type:+, type $want_type}"
    while IFS="$MJ_TAB" read -r t a b c d; do
      [ "$t" = X ] || continue
      case "$a" in FAIL) mj_fail knowledge "$c" "$d" "majordomus knowledge edges --json"; fails=$((fails + 1)) ;;
                   *)    mj_warn knowledge "$c" "$d" "majordomus knowledge edges --json" ;; esac
    done < "$out"
  fi
  rm -f "$out"
  [ "$fails" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}

# ================================================================ the writer (ADR 0058)
# Everything above reads. What follows is the one place the knowledge kind is written by
# the tool, and it writes exactly one thing: what an episode's ledger lines already state,
# as candidate records a person then promotes or rejects.
#
# The evidence is the ledger and git, and nothing else. A decision recorded, a question
# resolved, a task finished with a typed outcome — each is a line the writer stamped with
# the episode's id at the moment it happened, and each says something true about the
# repository that is otherwise reachable on one machine until the retention cap removes
# it. Nothing here reads a conversation, a handover, a prompt or a checkpoint: a record
# derived from a derived record would be a projection of a projection, and a record
# derived from prose would be a summary, which is the thing project.never-store-transcripts
# refuses.
#
# Same ledger, same git, same bytes. The record's id is the episode followed by a digest
# of the evidence it came from, so two worktrees never write one name for different
# content and a second run over the same evidence rewrites the same file byte for byte;
# its date is the day of the evidence line, not the clock; and no random value enters the
# file. That is what lets a provider hook run it: it can be wrong only if a ledger line is
# wrong, and the ledger has its own gate.
#
# `verified` is never written here. A candidate becomes verified through `knowledge
# promote` with evidence on standard input, or through a person editing the file; the
# deriver skips an id that has been promoted or superseded, so neither act is undone at
# the next episode boundary.

mj_knowledge_candidates_dir() { printf '%s/candidates' "$MJ_KNOWLEDGE_DIR"; }
mj_knowledge_curated_dir()    { printf '%s/curated' "$MJ_KNOWLEDGE_DIR"; }
mj_knowledge_ledger()         { printf '%s/ledger.jsonl' "$MJ_STATE_DIR"; }
MJ_KNOWLEDGE_CLASSES="fact convention constraint memory lesson"
MJ_KNOWLEDGE_STATUSES="candidate verified superseded"
MJ_KNOWLEDGE_EPISTEMICS="observed inferred decided"
MJ_KNOWLEDGE_ORIGINS="authored extracted"
MJ_KNOWLEDGE_RELATIONS="relates_to depends_on documents supports contradicts supersedes derived_from"
# The line shape that marks a conversation, shared with the working-context store
# (lib/session_context.sh): a record whose title, description or body opens a line this
# way is a transcript wearing a record's front matter.
MJ_KNOWLEDGE_TRANSCRIPT='^(transcript|messages|assistant|completion|response)[: ]'

# The episode to derive from: the one named, else the one open for this process. Empty
# means none, and the caller says so rather than guessing at the pointer.
mj_knowledge_episode_resolve() {
  if [ -n "${1:-}" ]; then printf '%s' "$1"; return 0; fi
  mj_open_session_id
}

# One pass over the ledger, selecting the lines the episode stamped and turning each
# derivable one into a tab-separated evidence row:
#
#   kind  task_id  ts  canon  title  description  class  epistemics  tag  outcome
#
# Free text is unescaped here, once, from the JSON the writer produced: `\"` becomes `"`,
# `\\` becomes `\`, and a `\n` becomes a space, because a record's title is one line.
# mj_json_field is not used for it — it cuts at the first escaped quote — and nothing
# re-escapes until the record is composed, where YAML quoting is applied exactly once.
# `canon` is the canonical evidence string the record id is digested from, its lines
# joined with the unit separator so that the row stays one line.
#
# The mapping is closed, and it is the ADR's table: a recorded decision is a convention
# that was decided; a resolved question is an observed fact; a task that ended blocked,
# failed or without a match is an inferred lesson; a task that completed with a
# verification command is an observed fact. Any other line yields nothing.
mj_knowledge_evidence() {
  local sid="$1" led; led="$(mj_knowledge_ledger)"
  [ -f "$led" ] || return 0
  awk -v sid="$sid" '
    BEGIN { OFS = "\t"; US = sprintf("%c", 31) }
    function jstr(s, key,   i, n, c, out, at) {
      at = index(s, "\"" key "\":\"")
      if (at == 0) return ""
      i = at + length(key) + 4; n = length(s); out = ""
      while (i <= n) {
        c = substr(s, i, 1)
        if (c == "\\") {
          i++; c = substr(s, i, 1)
          if (c == "n" || c == "t" || c == "r") out = out " "
          else out = out c
        } else if (c == "\"") break
        else out = out c
        i++
      }
      gsub(/\t/, " ", out)
      return out
    }
    function jraw(s, key,   at, r) {
      at = index(s, "\"" key "\":")
      if (at == 0) return ""
      r = substr(s, at + length(key) + 3); sub(/[,}].*$/, "", r)
      return r
    }
    index($0, "\"session\":\"" sid "\"") == 0 { next }
    {
      e = jstr($0, "event"); ts = jstr($0, "ts"); task = jstr($0, "task_id")
      if (task == "") task = "none"       # a row never carries an empty column
      if (e == "decision.recorded") {
        d = jstr($0, "decision"); if (d == "") next
        if (task == "" || task == "none") desc = "Recorded as a decision outside a task in episode " sid "."
        else desc = "Recorded as a decision under task " task " in episode " sid "."
        print "decision", task, ts, "decision.recorded" US task US d, d, desc, "convention", "decided", "decision", ""
      } else if (e == "question.resolved") {
        q = jstr($0, "question"); a = jstr($0, "answer"); if (a == "") next
        print "question", task, ts, "question.resolved" US task US q US a, a, \
          "Answers the question \"" q "\" resolved under task " task " in episode " sid ".", "fact", "observed", "question", ""
      } else if (e == "task.finished") {
        o = jstr($0, "outcome")
        if (o == "blocked" || o == "failed" || o == "no_match")
          print "task", task, ts, "task.finished" US task US o, "Task " task " ended " o, \
            "The task reached the outcome " o " in episode " sid ".", "lesson", "inferred", "task", o
        else if (o == "completed" && index($0, "\"verify\":{") > 0)
          print "task", task, ts, "task.finished" US task US o, \
            "Task " task " completed; verified by: " jstr($0, "command") " (exit " jraw($0, "exit") ")", \
            "The verification command passed when the task finished in episode " sid ".", "fact", "observed", "task", o
      }
    }' "$led"
}

# The first non-blank line of one level-one section of a note, or nothing. Used for the
# `# Reason` a task that ended blocked or failed left under state/completed/.
mj_knowledge_note_line() {
  [ -f "$1" ] || return 0
  awk -v want="$2" '
    /^# / { cur = substr($0, 3); sub(/[ \t]+$/, "", cur); on = (cur == want); next }
    on && NF { print; exit }' "$1"
}

# <episode>-<twelve hex characters of sha256 over the canonical evidence>. The digest is of
# the evidence, not of the record: a change in how a title is worded does not move the
# file, and the same decision text always lands in the same place.
mj_knowledge_record_id() {
  local sid="$1" canon="$2" digest
  digest="$(printf '%s' "$canon" | tr '\037' '\n' | mj_sha256 /dev/stdin | cut -c1-12)"
  printf '%s-%s' "$sid" "$digest"
}

# The commits the episode made that touched the rules or the decisions sections, one
# `sha<TAB>path` row per path, in the order the commits were made. An episode whose
# opening commit is not in this history — a rebase in between — names none: the list
# would be computed across a history that no longer connects.
mj_knowledge_episode_commits() {
  local sid="$1" led line start rules adrs sha
  led="$(mj_knowledge_ledger)"; [ -f "$led" ] || return 0
  line="$(grep -F '"event":"session.started"' "$led" 2>/dev/null | grep -F "\"session\":\"$sid\"" | head -n 1)"
  [ -n "$line" ] || return 0
  start="$(mj_json_field "$line" head)"
  [ -n "$start" ] && [ "$start" != NONE ] || return 0
  [ "$(mj_git_head)" != NONE ] || return 0
  mj_git merge-base --is-ancestor "$start" HEAD 2>/dev/null || return 0
  rules="$(mj_rel "$MJ_RULES_DIR")"; adrs="$(mj_rel "$MJ_ADRS_DIR")"
  for sha in $(mj_git rev-list --reverse "$start..HEAD" 2>/dev/null); do
    mj_git diff-tree --no-commit-id --name-only -r "$sha" -- "$rules" "$adrs" 2>/dev/null \
      | awk -v s="$sha" 'NF { printf "%s\t%s\n", s, $0 }'
  done
  return 0
}

# The commit rows reduced to what a record carries: one `commit:<sha>` per commit, in
# order, and one `file:<path>` per distinct path the tree still tracks, sorted under the
# C collation. Computed once per derivation, not once per record.
mj_knowledge_episode_refs() {
  local commits="$1" p
  [ -s "$commits" ] || return 0
  cut -f1 "$commits" | awk '!seen[$0]++ { print "commit:" $0 }'
  cut -f2 "$commits" | LC_ALL=C sort -u | while IFS= read -r p; do
    [ -n "$p" ] || continue
    mj_git ls-files --error-unmatch -- "$p" >/dev/null 2>&1 && printf 'file:%s\n' "$p"
  done
  return 0
}

# YAML double-quoted scalar content: backslash and double quote escaped, once.
mj_knowledge_yq() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

# One record, composed to OUT from one evidence row and the episode's reference list, in
# the exact key order the section's README documents. Prints the record id.
mj_knowledge_compose() {
  local sid="$1" row="$2" refs="$3" out="$4"
  local kind task ts canon title desc class epi tag rest id
  IFS="$MJ_TAB" read -r kind task ts canon title desc class epi tag rest <<EOF
$row
EOF
  : "$rest"     # the outcome column belongs to the derive loop, not to composition
  # A lesson carries the reason the task's own note gave, when it left one.
  if [ "$class" = lesson ] && [ -n "$task" ] && [ "$task" != none ]; then
    local reason; reason="$(mj_knowledge_note_line "$MJ_STATE_DIR/completed/$task.md" Reason)"
    [ -n "$reason" ] && title="$title: $reason"
  fi
  id="$(mj_knowledge_record_id "$sid" "$canon")"
  {
    printf -- '---\nschema: knowledge/v1\nid: %s\nkind: knowledge\nclass: %s\n' "$id" "$class"
    printf 'title: "%s"\ndescription: "%s"\n' "$(mj_knowledge_yq "$title")" "$(mj_knowledge_yq "$desc")"
    printf 'status: candidate\nepistemics: %s\ndate: %s\ntags:\n  - derived\n  - %s\n' "$epi" "${ts%%T*}" "$tag"
    printf 'provenance:\n  origin: extracted\n  derived_from:\n    - session:%s\n' "$sid"
    # A decision has no identity of its own in the ledger; the task it was recorded under
    # is the reference, as the schema's own comment uses it. A decision recorded outside a
    # task names the episode and nothing else (a literal `none` is not a task).
    if [ -n "$task" ] && [ "$task" != none ]; then
      case "$kind" in decision) printf '    - decision:%s\n' "$task" ;; *) printf '    - task:%s\n' "$task" ;; esac
    fi
    [ -s "$refs" ] && sed -n 's/^commit:/    - commit:/p' "$refs"
    if [ -s "$refs" ] && grep -q '^file:' "$refs"; then
      printf 'relations:\n'
      grep '^file:' "$refs" | while IFS= read -r p; do printf '  - type: relates_to\n    target: %s\n' "$p"; done
    fi
    printf -- '---\n\n# %s\n\n%s\n' "$title" "$desc"
  } > "$out"
  printf '%s' "$id"
}

# The atomic, idempotent write: compose into a staging file inside the directory, compare,
# and rename over the id-named file only when the bytes differ. The `.tmp.` prefix is the
# one .gitignore already refuses and mj_sweep_record_temps already collects, so a writer
# killed between the two steps leaves nothing that can reach a commit. Nothing here stages.
# Prints `written` or `unchanged`; returns 1 when the directory cannot be written.
mj_knowledge_write_record() {
  local dir="$1" id="$2" src="$3" tmp final
  final="$dir/$id.md"
  mkdir -p "$dir" 2>/dev/null || return 1
  mj_sweep_record_temps "$dir"
  tmp="$(mktemp "$dir/.tmp.XXXXXX" 2>/dev/null)" || return 1
  cat "$src" > "$tmp" 2>/dev/null || { rm -f "$tmp"; return 1; }
  chmod 644 "$tmp" 2>/dev/null || true
  if [ -f "$final" ] && cmp -s "$tmp" "$final"; then rm -f "$tmp"; printf 'unchanged'; return 0; fi
  mv -f "$tmp" "$final" 2>/dev/null || { rm -f "$tmp"; return 1; }
  printf 'written'
}

# The candidates directory exists the moment a record needs it, and it carries its
# contract from the distribution when it is created here: a directory of the layer
# without one is a finding (ADR 0011), and the episode that wrote the first candidate did
# not cause that.
mj_knowledge_candidates_ready() {
  local dir; dir="$(mj_knowledge_candidates_dir)"
  [ -d "$dir" ] && return 0
  mkdir -p "$dir" 2>/dev/null || return 1
  [ -f "$dir/README.md" ] && return 0
  [ -f "$MJ_SKELETON_DIR/ai/repo/knowledge/candidates/README.md" ] || return 0
  cp "$MJ_SKELETON_DIR/ai/repo/knowledge/candidates/README.md" "$dir/README.md" 2>/dev/null || true
  return 0
}

mj_knowledge_record_status() { sed -n 's/^status: //p' "$1" 2>/dev/null | head -n 1; }

# majordomus knowledge derive [--episode <id>] [--dry-run]
mj_knowledge_derive_cmd() {
  local episode="" dry=0
  while [ $# -gt 0 ]; do case "$1" in
    --episode) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--episode needs a value"; episode="$2"; shift 2 ;;
    --episode=*) episode="${1#--episode=}"; shift ;;
    --dry-run) dry=1; shift ;;
    --help|-h) mj_knowledge_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge derive: unknown option $1" ;;
  esac; done
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  local sid; sid="$(mj_knowledge_episode_resolve "$episode")"
  local dry_json=false; [ "$dry" = 1 ] && dry_json=true
  if [ -z "$sid" ]; then
    if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"episode":null,"dry_run":%s,"written":[],"unchanged":[],"skipped":[]}\n' "$dry_json"
    else printf 'knowledge derive: no episode to derive from; open one or pass --episode <id>\n'; fi
    return 0
  fi
  local t0; t0="$(mj_phase_begin knowledge:derive)"
  local tmp cdir kdir; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.kdv.XXXXXX")"
  cdir="$(mj_knowledge_candidates_dir)"; kdir="$(mj_knowledge_curated_dir)"
  mj_knowledge_evidence "$sid" > "$tmp/rows"
  mj_knowledge_episode_commits "$sid" > "$tmp/commits"
  mj_knowledge_episode_refs "$tmp/commits" > "$tmp/refs"

  local row id rel w=0 u=0 s=0 W="" U="" S="" paths="" verb kind task ts canon title desc rest
  while IFS= read -r row <&3; do
    [ -n "$row" ] || continue
    IFS="$MJ_TAB" read -r kind task ts canon title desc rest <<EOF
$row
EOF
    id="$(mj_knowledge_compose "$sid" "$row" "$tmp/refs" "$tmp/rec")"
    rel="$(mj_rel "$cdir")/$id.md"
    mj_count knowledge_evidence
    # Refused at derive time rather than written for the validator to fail: a title or a
    # description that opens like a turn of a conversation is not an assertion.
    if printf '%s\n%s\n' "$title" "$desc" | grep -Eq "$MJ_KNOWLEDGE_TRANSCRIPT"; then
      verb="skipped (transcript)"; s=$((s + 1)); S="$S$rel"$'\n'
    elif [ -f "$kdir/$id.md" ]; then
      verb="skipped (promoted)"; s=$((s + 1)); S="$S$rel"$'\n'
    elif [ -f "$cdir/$id.md" ] && [ "$(mj_knowledge_record_status "$cdir/$id.md")" != candidate ]; then
      verb="skipped (superseded)"; s=$((s + 1)); S="$S$rel"$'\n'
    elif [ "$dry" = 1 ]; then
      if [ -f "$cdir/$id.md" ] && cmp -s "$tmp/rec" "$cdir/$id.md"; then verb=unchanged; u=$((u + 1)); U="$U$rel"$'\n'
      else verb="would write"; w=$((w + 1)); W="$W$rel"$'\n'; fi
    else
      mj_knowledge_candidates_ready || { rm -rf "$tmp"; mj_die "$MJ_EX_INTERNAL" "knowledge derive: cannot create $(mj_rel "$cdir")/"; }
      verb="$(mj_knowledge_write_record "$cdir" "$id" "$tmp/rec")" \
        || { rm -rf "$tmp"; mj_die "$MJ_EX_INTERNAL" "knowledge derive: could not write $rel"; }
      case "$verb" in
        written) w=$((w + 1)); W="$W$rel"$'\n'; paths="$paths${paths:+ }$rel" ;;
        *) u=$((u + 1)); U="$U$rel"$'\n' ;;
      esac
    fi
    case "$verb" in
      "skipped ("*) [ "$MJ_JSON" = 1 ] || printf 'skipped %s %s\n' "$rel" "${verb#skipped }" ;;
      *) [ "$MJ_JSON" = 1 ] || printf '%s %s\n' "$verb" "$rel" ;;
    esac
  done 3< "$tmp/rows"
  rm -rf "$tmp"

  # The ledger line, even when nothing was written: "derived, nothing new" and "never
  # derived" are different facts, and the stopped-writer check tells them apart by it.
  [ "$dry" = 1 ] || mj_ledger_append knowledge.derived \
    "\"episode\":\"$(mj_json_esc "$sid")\",\"written\":$w,\"unchanged\":$u,\"skipped\":$s,\"paths\":\"$(mj_json_esc "$paths")\""
  mj_phase_end knowledge:derive "$t0"

  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"episode":"%s","dry_run":%s,"written":[%s],"unchanged":[%s],"skipped":[%s]}\n' \
      "$(mj_json_esc "$sid")" "$dry_json" "$(mj_knowledge_json_list "$W")" "$(mj_knowledge_json_list "$U")" "$(mj_knowledge_json_list "$S")"
  else
    printf 'knowledge derive: %s written, %s unchanged, %s skipped for episode %s\n' "$w" "$u" "$s" "$sid"
  fi
  return 0
}

# newline-separated values -> the inside of a JSON array of strings
mj_knowledge_json_list() {
  printf '%s' "$1" | sed '/^$/d' | while IFS= read -r v; do printf '"%s",' "$(mj_json_esc "$v")"; done | sed 's/,$//'
}

# ---------------------------------------------------------------- the review queue
# Which branch an episode worked on: the tracked session record when the episode has
# closed and been committed, the open episode's own file while it is still open, and the
# ledger's opening line for everything in between. One `sid<TAB>branch` row per episode
# this checkout knows, first source wins. A candidate whose episode none of them knows is
# named as such by every reader, never attributed to the current branch by proximity.
mj_knowledge_branch_map() {
  # shellcheck source=session.sh
  . "$MJ_LIB_DIR/session.sh"
  local store led; store="$(mj_session_store)"; led="$(mj_knowledge_ledger)"
  {
    if [ -d "$store" ]; then
      awk 'FNR == 1 { fm = 0; b = ""; s = "" }
           FNR == 1 && $0 == "---" { fm = 1; next }
           fm && $0 == "---" { if (s != "") print s "\t" b; fm = 0; nextfile }
           fm && /^branch: / { b = substr($0, 9) }
           fm && /^session_id: / { s = substr($0, 13) }' "$store"/*.md 2>/dev/null
    fi
    if [ -d "$(mj_session_open_dir)" ]; then
      awk 'FNR == 1 { if (s != "") print s "\t" b; b = ""; s = "" }
           /^branch: / { b = substr($0, 9) }
           /^session_id: / { s = substr($0, 13) }
           END { if (s != "") print s "\t" b }' "$(mj_session_open_dir)"/*.yaml 2>/dev/null
    fi
    if [ -f "$led" ]; then
      awk 'index($0, "\"event\":\"session.started\"") == 0 { next }
           index($0, "\"session\":\"") == 0 { next }
           { s = $0; sub(/^.*"session":"/, "", s); sub(/".*$/, "", s)
             b = $0; sub(/^.*"branch":"/, "", b); sub(/".*$/, "", b)
             if (s != "") print s "\t" b }' "$led"
    fi
  } | awk -F'\t' '!seen[$1]++'
}

# Every record under candidates/, whatever its status, one row each and sorted by id:
#
#   id  class  status  epistemics  date  episode  branch  path  title
#
# The directory is read, not the index: a candidate the hook wrote a moment ago must be
# listed before anybody has staged it, which the version-control discovery cannot do. The
# section's README is a context document and is not a record. Front matter is flattened
# for all files in one process; a file that does not parse yields no row here and a
# finding under `knowledge check`.
mj_knowledge_candidate_rows() {
  local dir; dir="$(mj_knowledge_candidates_dir)"
  [ -d "$dir" ] || return 0
  local tmp f n=0; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.kcr.XXXXXX")"; mkdir -p "$tmp/flat"
  : > "$tmp/map"
  set --
  for f in "$dir"/*.md; do
    [ -f "$f" ] || continue
    mj_is_context_doc "$f" && continue
    set -- "$@" "$f"; n=$((n + 1)); printf '%s\t%s\n' "$n" "$(mj_rel "$f")" >> "$tmp/map"
  done
  [ $# -gt 0 ] || { rm -rf "$tmp"; return 0; }
  mj_yaml_flatten_many "$tmp/flat" --numbered --front "$@"
  mj_knowledge_branch_map > "$tmp/branches"
  awk -F'\t' '
    function unq(s,   i, n, c, out) {
      # the flattener strips the quotes and keeps the escapes; a listing shows the text
      n = length(s); out = ""
      for (i = 1; i <= n; i++) { c = substr(s, i, 1); if (c == "\\" && i < n) { i++; c = substr(s, i, 1) } out = out c }
      return out
    }
    # an empty column is written as `-`: a tab is whitespace to the shell, and a reader
    # splitting on it would fold two empty fields into none and shift every column after
    function nz(x) { return (x == "") ? "-" : x }
    function flush() {
      if (cur == "") return
      ep = ""; for (i = 0; (("provenance.derived_from." i) in v); i++) if (v["provenance.derived_from." i] ~ /^session:/) { ep = substr(v["provenance.derived_from." i], 9); break }
      b = (ep in branch) ? branch[ep] : ""
      if (("id" in v) && v["id"] != "")
        printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n", v["id"], nz(v["class"]), nz(v["status"]), nz(v["epistemics"]), nz(v["date"]), nz(ep), nz(b), path[cur], nz(unq(v["title"]))
      delete v; cur = ""
    }
    FILENAME == map { path[$1] = $2; next }
    FILENAME == branches { branch[$1] = $2; next }
    FNR == 1 { flush(); cur = FILENAME; sub(/.*\//, "", cur) }
    { eq = index($0, "="); v[substr($0, 1, eq - 1)] = substr($0, eq + 1) }
    END { flush() }' map="$tmp/map" branches="$tmp/branches" "$tmp/map" "$tmp/branches" "$tmp/flat"/[0-9]* 2>/dev/null \
    | LC_ALL=C sort -t "$MJ_TAB" -k1,1
  rm -rf "$tmp"
  return 0
}

# The lines the start briefing prints for one branch: `- <id>  <title>` per record with
# status candidate whose episode worked on BRANCH. An empty BRANCH selects the candidates
# whose episode this checkout cannot place, so that they are named rather than hidden.
mj_knowledge_briefing_lines() {
  mj_knowledge_candidate_rows | awk -F'\t' -v b="${1:--}" '$3 == "candidate" && $7 == b { printf "- %s  %s\n", $1, $9 }'
}

# The knowledge section of a briefing, on stdout: the candidates whose episode worked on
# BRANCH and nobody has yet judged (ADR 0058), the count on one line and the ids beneath it
# bounded to five, else the absence sentence; then, when any, the candidates whose episode
# this checkout cannot place, named apart rather than folded into the branch or dropped —
# hidden is the one thing a review queue must not be. Defined once, so that the start
# briefing (lib/derive.sh) and `majordomus context` (lib/context.sh) print the same bytes.
# It bounds through mj_derive_bounded and counts through mj_derive_nlines: both callers hold
# lib/derive.sh, and the shell resolves the names when this is called, not when it is read.
mj_knowledge_briefing_section() {
  local cand unplaced
  cand="$(mj_knowledge_briefing_lines "$1")"
  if [ -n "$cand" ]; then
    printf 'Knowledge candidates awaiting review on this branch: %s\n' "$(mj_derive_nlines "$cand")"
    printf '%s\n' "$cand" | mj_derive_bounded 5 "candidate(s)"
  else
    printf 'No knowledge candidates await review on this branch. That is an answer, not a gap: `majordomus knowledge candidates` lists every branch.\n'
  fi
  unplaced="$(mj_knowledge_briefing_lines "")"
  if [ -n "$unplaced" ]; then
    printf '%s candidate(s) whose episode is not known in this checkout:\n' "$(mj_derive_nlines "$unplaced")"
    printf '%s\n' "$unplaced" | mj_derive_bounded 5 "candidate(s)"
  fi
  return 0
}

# majordomus knowledge candidates
mj_knowledge_candidates_cmd() {
  while [ $# -gt 0 ]; do case "$1" in
    --help|-h) mj_knowledge_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge candidates: unknown option $1" ;;
  esac; done
  local out n=0; out="$(mktemp "${TMPDIR:-/tmp}/mj.kca.XXXXXX")"
  mj_knowledge_candidate_rows | awk -F'\t' '$3 == "candidate"' > "$out"
  local id cls st epi date ep br path title first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"candidates":['
    while IFS="$MJ_TAB" read -r id cls st epi date ep br path title; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"id":"%s","class":"%s","status":"%s","epistemics":"%s","date":"%s","episode":"%s","branch":%s,"path":"%s","title":"%s"}' \
        "$(mj_json_esc "$id")" "$cls" "$st" "$epi" "$date" "$([ "$ep" != - ] && mj_json_esc "$ep")" \
        "$([ "$br" != - ] && printf '"%s"' "$(mj_json_esc "$br")" || printf 'null')" "$(mj_json_esc "$path")" "$(mj_json_esc "$title")"
    done < "$out"
    printf ']}\n'
  else
    while IFS="$MJ_TAB" read -r id cls st epi date ep br path title; do
      printf '%-46s %-10s %-10s %-24s %s\n' "$id" "$cls" "$date" "${br:--}" "$title"
      n=$((n + 1))
    done < "$out"
    printf 'knowledge candidates: %s awaiting review\n' "$n"
  fi
  rm -f "$out"
  return 0
}

# ---------------------------------------------------------------- the acts
# A person promotes or rejects; the tool records that they did. `verified` is written
# here and by a person editing the file, and nowhere else.

# majordomus knowledge promote <id> [--class <c>] < evidence.md
mj_knowledge_promote_cmd() {
  local id="" class=""
  while [ $# -gt 0 ]; do case "$1" in
    --class) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--class needs a value"; class="$2"; shift 2 ;;
    --class=*) class="${1#--class=}"; shift ;;
    --help|-h) mj_knowledge_usage; return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "knowledge promote: unknown option $1" ;;
    *) [ -z "$id" ] || mj_die "$MJ_EX_USAGE" "knowledge promote: one id, not two"; id="$1"; shift ;;
  esac; done
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "knowledge promote: the candidate id is required (usage: majordomus knowledge promote <id> < evidence.md)"
  if [ -n "$class" ]; then case " $MJ_KNOWLEDGE_CLASSES " in *" $class "*) ;;
    *) mj_die "$MJ_EX_CONTRACT" "knowledge promote: --class must be one of: $MJ_KNOWLEDGE_CLASSES" ;; esac; fi
  local cdir kdir src; cdir="$(mj_knowledge_candidates_dir)"; kdir="$(mj_knowledge_curated_dir)"; src="$cdir/$id.md"
  [ -f "$src" ] || mj_die "$MJ_EX_MISSING" "knowledge promote: no candidate with id $id under $(mj_rel "$cdir")/"
  local status; status="$(mj_knowledge_record_status "$src")"
  [ "$status" = candidate ] || mj_die "$MJ_EX_CONTRACT" "knowledge promote: $id has status $status, not candidate; only a candidate is promoted"
  grep -q '^  origin: ' "$src" || mj_die "$MJ_EX_CONTRACT" "knowledge promote: $id carries no provenance.origin, and a verified record must say where it came from"

  # An act without evidence is an assertion, and a record is never a transcript.
  local ev; ev="$(mktemp "${TMPDIR:-/tmp}/mj.kev.XXXXXX")"
  if [ ! -t 0 ]; then cat > "$ev"; fi
  grep -q '[^[:space:]]' "$ev" 2>/dev/null \
    || { rm -f "$ev"; mj_die "$MJ_EX_CONTRACT" "knowledge promote: the evidence is required on stdin; an act without evidence is an assertion"; }
  grep -Eq "$MJ_KNOWLEDGE_TRANSCRIPT" "$ev" \
    && { rm -f "$ev"; mj_die "$MJ_EX_CONTRACT" "knowledge promote: the evidence carries a conversation (a line opening with transcript, messages, assistant, completion or response); a record is an assertion, never a transcript"; }

  local sid today rec; sid="$(mj_open_session_id)"; today="$(mj_now)"; today="${today%%T*}"
  rec="$(mktemp "${TMPDIR:-/tmp}/mj.kpr.XXXXXX")"
  {
    printf -- '---\n'
    mj_record_front "$src" | awk -v cls="$class" -v date="$today" -v sid="$sid" '
      /^status: / { print "status: verified"; next }
      /^class: /  { if (cls != "") { print "class: " cls; next } }
      /^date: /   { print "date: " date; next }
      /^  derived_from:/ { indf = 1; print; next }
      indf && /^    - / { if ($0 == "    - session:" sid) have = 1; print; next }
      indf { if (sid != "" && !have) print "    - session:" sid; indf = 0 }
      { print }
      END { if (indf && sid != "" && !have) print "    - session:" sid }'
    printf -- '---\n'
    mj_record_body "$src"
    printf '\n# Evidence\n\n'
    cat "$ev"; [ -n "$(tail -c1 "$ev")" ] && printf '\n'
  } > "$rec"
  rm -f "$ev"
  mj_knowledge_write_record "$kdir" "$id" "$rec" >/dev/null \
    || { rm -f "$rec"; mj_die "$MJ_EX_INTERNAL" "knowledge promote: could not write $(mj_rel "$kdir")/$id.md"; }
  rm -f "$rec" "$src"
  local rel; rel="$(mj_rel "$kdir")/$id.md"
  mj_ledger_append knowledge.promoted "\"id\":\"$(mj_json_esc "$id")\",\"path\":\"$(mj_json_esc "$rel")\""
  printf '%s\n' "$rel"
}

# majordomus knowledge reject <id> --reason "<why>" [--by <id>]
mj_knowledge_reject_cmd() {
  local id="" reason="" by=""
  while [ $# -gt 0 ]; do case "$1" in
    --reason) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--reason needs a value"; reason="$2"; shift 2 ;;
    --reason=*) reason="${1#--reason=}"; shift ;;
    --by) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--by needs a value"; by="$2"; shift 2 ;;
    --by=*) by="${1#--by=}"; shift ;;
    --help|-h) mj_knowledge_usage; return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "knowledge reject: unknown option $1" ;;
    *) [ -z "$id" ] || mj_die "$MJ_EX_USAGE" "knowledge reject: one id, not two"; id="$1"; shift ;;
  esac; done
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "knowledge reject: the candidate id is required"
  printf '%s' "$reason" | grep -q '[^[:space:]]' || mj_die "$MJ_EX_USAGE" "knowledge reject: --reason is required and must say why"
  local cdir kdir src; cdir="$(mj_knowledge_candidates_dir)"; kdir="$(mj_knowledge_curated_dir)"; src="$cdir/$id.md"
  [ -f "$src" ] || mj_die "$MJ_EX_MISSING" "knowledge reject: no candidate with id $id under $(mj_rel "$cdir")/"
  local status; status="$(mj_knowledge_record_status "$src")"
  [ "$status" = candidate ] || mj_die "$MJ_EX_CONTRACT" "knowledge reject: $id has status $status, not candidate; only a candidate is rejected"
  if [ -n "$by" ]; then
    [ -f "$cdir/$by.md" ] || [ -f "$kdir/$by.md" ] \
      || mj_die "$MJ_EX_CONTRACT" "knowledge reject: --by names $by, and no record with that id exists under candidates/ or curated/"
  fi
  local rec; rec="$(mktemp "${TMPDIR:-/tmp}/mj.krj.XXXXXX")"
  {
    printf -- '---\n'
    mj_record_front "$src" | awk -v by="$by" '
      /^status: / { print "status: superseded"; if (by != "") print "superseded_by: " by; next }
      /^superseded_by: / { next }
      { print }'
    printf -- '---\n'
    mj_record_body "$src"
    printf '\n# Rejected\n\n%s\n' "$reason"
  } > "$rec"
  mj_knowledge_write_record "$cdir" "$id" "$rec" >/dev/null \
    || { rm -f "$rec"; mj_die "$MJ_EX_INTERNAL" "knowledge reject: could not rewrite $(mj_rel "$src")"; }
  rm -f "$rec"
  local fields; fields="\"id\":\"$(mj_json_esc "$id")\",\"reason\":\"$(mj_json_esc "$reason")\""
  [ -n "$by" ] && fields="$fields,\"by\":\"$(mj_json_esc "$by")\""
  mj_ledger_append knowledge.rejected "$fields"
  printf '%s\n' "$(mj_rel "$src")"
}

# ---------------------------------------------------------------- the check
# Every record under candidates/ and curated/, judged in one pass: the front matter of all
# of them flattened by one process, every reference they name collected once and resolved
# in batches — the ledger read once for every session and task, git asked once for every
# commit — and the findings written to a file the command and the validator both read.
#
# mj_knowledge_check_files OUTDIR FILE...   findings: <relpath>\t<code>\t<message>
#                                            records:  <relpath> per file judged
mj_knowledge_check_files() {
  local tmp="$1"; shift
  mkdir -p "$tmp/flat"; : > "$tmp/findings"; : > "$tmp/records"; : > "$tmp/map"
  [ $# -gt 0 ] || return 0
  local f n=0 t0
  for f in "$@"; do n=$((n + 1)); printf '%s\t%s\n' "$n" "$(mj_rel "$f")" >> "$tmp/map"; printf '%s\n' "$(mj_rel "$f")" >> "$tmp/records"; done
  t0="$(mj_phase_begin knowledge:check-flatten)"
  mj_yaml_flatten_many "$tmp/flat" --numbered --front "$@"
  [ -f "$tmp/flat/.errors" ] || : > "$tmp/flat/.errors"
  # the bodies, one process: a conversation, and the section a rejection records
  awk 'FNR == 1 { n++; fences = 0 }
       /^---[ \t]*$/ && fences < 2 { fences++; next }
       fences < 2 { next }
       /^(transcript|messages|assistant|completion|response)[: ]/ { if (!t[n]++) print n "\tTRANSCRIPT" }
       /^# Rejected[ \t]*$/ { if (!r[n]++) print n "\tREJECTED" }' "$@" > "$tmp/body"
  mj_phase_end knowledge:check-flatten "$t0"
  local cand; cand="$(mj_rel "$(mj_knowledge_candidates_dir)")/"
  t0="$(mj_phase_begin knowledge:check-shape)"
  awk -F'\t' -v cand="$cand" -v classes=" $MJ_KNOWLEDGE_CLASSES " -v statuses=" $MJ_KNOWLEDGE_STATUSES " \
      -v epis=" $MJ_KNOWLEDGE_EPISTEMICS " -v origins=" $MJ_KNOWLEDGE_ORIGINS " -v rels=" $MJ_KNOWLEDGE_RELATIONS " '
    function fail(code, msg) { printf "F\t%s\t%s\t%s\n", path[cur], code, msg }
    function has(k) { return (k in v) && v[k] != "" }
    function idpat(s) { return s ~ /^[a-z][a-z0-9-]*$/ }
    function flush(   i, k, key, ok, base, t) {
      if (cur == "") return
      if (cur in bad) { fail("shape", "does not parse as knowledge/v1 front matter: " bad[cur]); delete v; cur = ""; return }
      for (key in v) {
        ok = 0
        for (i = 1; i <= nallow; i++) if (key ~ allow[i]) { ok = 1; break }
        if (!ok) fail("unknown_key", "unknown key `" key "`; the schema knowledge/v1 declares no such field")
      }
      if (v["schema"] != "knowledge/v1") fail("schema", "schema must be knowledge/v1, not `" v["schema"] "`")
      if (v["kind"] != "knowledge") fail("kind", "kind must be knowledge, not `" v["kind"] "`")
      if (!has("id")) fail("id", "id is required")
      else {
        if (!idpat(v["id"])) fail("id", "id `" v["id"] "` is not lower-case letters, digits and hyphens")
        base = path[cur]; sub(/.*\//, "", base); sub(/\.md$/, "", base)
        if (base != v["id"]) fail("id", "id `" v["id"] "` does not equal the file name `" base "`; the file name is the identity")
        if (v["id"] in seen) { fail("duplicate_id", "id `" v["id"] "` is also claimed by " seen[v["id"]]); printf "F\t%s\tduplicate_id\tid `%s` is also claimed by %s\n", seen[v["id"]], v["id"], path[cur] }
        else seen[v["id"]] = path[cur]
      }
      if (!has("title")) fail("title", "title is required and must be one assertion")
      if (!has("description")) fail("description", "description is required")
      if (index(classes, " " v["class"] " ") == 0) fail("class", "class `" v["class"] "` is not one of:" classes)
      if (index(statuses, " " v["status"] " ") == 0) fail("status", "status `" v["status"] "` is not one of:" statuses)
      if (index(epis, " " v["epistemics"] " ") == 0) fail("epistemics", "epistemics `" v["epistemics"] "` is not one of:" epis)
      if (v["date"] !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) fail("date", "date `" v["date"] "` is not YYYY-MM-DD")
      for (i = 0; ("tags." i) in v; i++) if (!idpat(v["tags." i])) fail("tags", "tag `" v["tags." i] "` is not lower-case letters, digits and hyphens")
      if (has("provenance.origin") && index(origins, " " v["provenance.origin"] " ") == 0) fail("provenance", "provenance.origin `" v["provenance.origin"] "` is not one of:" origins)
      if (v["status"] == "verified" && !has("provenance.origin")) fail("provenance", "verified, and carries no provenance.origin; nothing is confirmed by having been written down")
      if (v["provenance.origin"] == "extracted" && !(("provenance.derived_from.0") in v)) fail("provenance", "origin extracted, and derived_from names no evidence")
      if (index(path[cur], cand) == 1 && v["status"] == "verified") fail("status", "claims verified under " cand "; the deriver never writes verified, and promotion moves a record to curated/")
      if (v["status"] == "superseded") {
        if (has("superseded_by")) { if (!idpat(v["superseded_by"])) fail("superseded_by", "superseded_by `" v["superseded_by"] "` is not a record id"); else printf "R\t%s\tknowledge:%s\tsuperseded_by\n", path[cur], v["superseded_by"] }
        else if (!((cur "\tREJECTED") in body)) fail("superseded", "superseded, and names neither superseded_by nor a `# Rejected` section saying why")
      } else if (has("superseded_by")) fail("superseded_by", "names superseded_by while its status is " v["status"] "; superseded_by is present exactly when the status is superseded")
      for (i = 0; ("provenance.derived_from." i) in v; i++) {
        t = v["provenance.derived_from." i]
        if (t !~ /^[a-z]+:.+$/) fail("reference", "derived_from `" t "` is not a typed reference (<kind>:<identity>)")
        else printf "R\t%s\t%s\tprovenance.derived_from.%d\n", path[cur], t, i
      }
      for (i = 0; ("relations." i ".type") in v || ("relations." i ".target") in v; i++) {
        if (index(rels, " " v["relations." i ".type"] " ") == 0) fail("relation", "relations." i ".type `" v["relations." i ".type"] "` is not one of:" rels)
        t = v["relations." i ".target"]
        if (t !~ /^[a-z]+:.+$/) fail("reference", "relations." i ".target `" t "` is not a typed reference (<kind>:<identity>)")
        else printf "R\t%s\t%s\trelations.%d.target\n", path[cur], t, i
      }
      if (v["title"] ~ /^(transcript|messages|assistant|completion|response)[: ]/) fail("transcript", "the title opens like a turn of a conversation; a record is an assertion, never a transcript")
      if (v["description"] ~ /^(transcript|messages|assistant|completion|response)[: ]/) fail("transcript", "the description opens like a turn of a conversation; a record is an assertion, never a transcript")
      if ((cur "\tTRANSCRIPT") in body) fail("transcript", "the body carries a conversation (a line opening with transcript, messages, assistant, completion or response)")
      delete v; cur = ""
    }
    FILENAME == mapf { path[$1] = $2; next }
    FILENAME == errf { bad[$1] = $2; next }
    FILENAME == bodyf { body[$1 "\t" $2] = 1; next }
    FILENAME == allowf { if ($0 !~ /^#/ && $0 != "") allow[++nallow] = $0; next }
    FNR == 1 { flush(); cur = FILENAME; sub(/.*\//, "", cur) }
    { eq = index($0, "="); v[substr($0, 1, eq - 1)] = substr($0, eq + 1) }
    END { flush(); for (k in bad) if (!(k in path)) next; for (k in bad) { cur = k; if (!(k in done)) { fail("shape", "does not parse as knowledge/v1 front matter: " bad[k]) } } }' \
    mapf="$tmp/map" errf="$tmp/flat/.errors" bodyf="$tmp/body" allowf="$MJ_ALLOW_DIR/knowledge.txt" \
    "$tmp/map" "$tmp/flat/.errors" "$tmp/body" "$MJ_ALLOW_DIR/knowledge.txt" "$tmp/flat"/[0-9]* 2>/dev/null > "$tmp/shape"
  mj_phase_end knowledge:check-shape "$t0"
  sed -n 's/^F\t//p' "$tmp/shape" > "$tmp/findings"
  # every reference, resolved once, and one finding per record that names one nothing resolves
  t0="$(mj_phase_begin knowledge:check-resolve)"
  sed -n 's/^R\t//p' "$tmp/shape" > "$tmp/refs"
  if [ -s "$tmp/refs" ]; then
    cut -f2 "$tmp/refs" | LC_ALL=C sort -u | mj_knowledge_resolve_refs > "$tmp/resolved"
    awk -F'\t' 'FILENAME == res { st[$1] = $2; next }
      { r = st[$2]
        if (r == "ok") next
        if (r == "refused") printf "%s\treference\t%s names `%s`, and `none` is not an identity: a decision recorded outside a task names the episode only\n", $1, $3, $2
        else if (r == "unknown") printf "%s\treference\t%s names `%s`, whose kind nothing in this layer resolves\n", $1, $3, $2
        else printf "%s\treference\t%s names `%s`, which resolves to nothing: not a record, not a ledger object, not a commit, not a tracked file\n", $1, $3, $2 }' \
      res="$tmp/resolved" "$tmp/resolved" "$tmp/refs" >> "$tmp/findings"
  fi
  mj_phase_end knowledge:check-resolve "$t0"
  LC_ALL=C sort -t "$MJ_TAB" -k1,1 -o "$tmp/findings" "$tmp/findings"
  return 0
}

# Distinct typed references on stdin, one per line; prints `ref<TAB>ok|missing|refused|unknown`.
# The batches: one pass over the ledger for every episode and task named, one
# `git cat-file --batch-check` for every commit, one listing of ids for the rules, the
# decisions and the project model; a file is a stat.
mj_knowledge_resolve_refs() {
  local tmp led; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.krr.XXXXXX")"; led="$(mj_knowledge_ledger)"
  LC_ALL=C sort -u > "$tmp/refs"
  [ -s "$tmp/refs" ] || { rm -rf "$tmp"; return 0; }
  : > "$tmp/have"
  if grep -Eq '^(session|task|decision):' "$tmp/refs"; then
    mj_count knowledge_resolve_ledger
    if [ -f "$led" ]; then
      awk '{ if (index($0, "\"session\":\"")) { s = $0; sub(/^.*"session":"/, "", s); sub(/".*$/, "", s); if (s != "" && !ss[s]++) print "session\t" s }
             if (index($0, "\"session_id\":\"")) { s = $0; sub(/^.*"session_id":"/, "", s); sub(/".*$/, "", s); if (s != "" && !ss[s]++) print "session\t" s }
             if (index($0, "\"task_id\":\"")) { t = $0; sub(/^.*"task_id":"/, "", t); sub(/".*$/, "", t); if (t != "" && !tt[t]++) print "task\t" t } }' "$led" >> "$tmp/have"
    fi
    mj_knowledge_branch_map | cut -f1 | sed 's/^/session\t/' >> "$tmp/have"
    [ -f "$MJ_STATE_DIR/current.yaml" ] && sed -n 's/^id: /task\t/p' "$MJ_STATE_DIR/current.yaml" | head -n 1 >> "$tmp/have"
    local f; for f in "$MJ_STATE_DIR"/completed/*.md; do [ -f "$f" ] && printf 'task\t%s\n' "$(basename "$f" .md)"; done >> "$tmp/have"
  fi
  if grep -q '^commit:' "$tmp/refs"; then
    mj_count knowledge_resolve_git
    sed -n 's/^commit://p' "$tmp/refs" | mj_git cat-file --batch-check 2>/dev/null \
      | awk '$2 != "missing" && NF >= 3 { print "commit\t" $1 }' >> "$tmp/have"
  fi
  if grep -Eq '^(rule|adr):' "$tmp/refs"; then
    grep -rh --include='*.md' '^id: ' "$MJ_RULES_DIR" 2>/dev/null | sed 's/^id: */rule\t/' >> "$tmp/have"
    grep -rh --include='*.md' '^id: ' "$MJ_ADRS_DIR" 2>/dev/null | sed 's/^id: */adr\t/' >> "$tmp/have"
  fi
  if grep -q '^issue:' "$tmp/refs"; then
    grep -rh --include='*.yaml' '^id: ' "$MJ_PROJECT_DIR" 2>/dev/null | sed 's/^id: */issue\t/' >> "$tmp/have"
  fi
  local ref kind ident st cdir kdir; cdir="$(mj_knowledge_candidates_dir)"; kdir="$(mj_knowledge_curated_dir)"
  while IFS= read -r ref; do
    kind="${ref%%:*}"; ident="${ref#*:}"; st=missing
    case "$kind" in
      file|test) [ -e "$MJ_ROOT/$ident" ] && st=ok ;;
      knowledge) { [ -f "$cdir/$ident.md" ] || [ -f "$kdir/$ident.md" ]; } && st=ok ;;
      session|commit|rule|adr|issue) grep -Fxq "$kind$MJ_TAB$ident" "$tmp/have" && st=ok ;;
      task|decision) if [ "$ident" = none ]; then st=refused; else grep -Fxq "task$MJ_TAB$ident" "$tmp/have" && st=ok; fi ;;
      *) st=unknown ;;
    esac
    printf '%s\t%s\n' "$ref" "$st"
  done < "$tmp/refs"
  rm -rf "$tmp"
  return 0
}

# One reference: 0 when it resolves, 1 otherwise.
mj_knowledge_ref_resolves() {
  [ "$(printf '%s\n' "$1" | mj_knowledge_resolve_refs | cut -f2)" = ok ]
}

# The records the check covers: every Markdown file under candidates/ and curated/ that
# is not the section's context document, sorted, absolute paths one per line.
mj_knowledge_record_files() {
  local d f
  for d in "$(mj_knowledge_candidates_dir)" "$(mj_knowledge_curated_dir)"; do
    [ -d "$d" ] || continue
    for f in "$d"/*.md; do
      [ -f "$f" ] || continue
      mj_is_context_doc "$f" && continue
      printf '%s\n' "$f"
    done
  done | LC_ALL=C sort
}

# One record, judged alone: prints `FAIL<TAB><code><TAB><message>` per finding.
mj_knowledge_check_record() {
  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.kck.XXXXXX")"
  mj_knowledge_check_files "$tmp" "$1"
  awk -F'\t' '{ printf "FAIL\t%s\t%s\n", $2, $3 }' "$tmp/findings"
  rm -rf "$tmp"
}

# Every record, judged together. Leaves findings and records under OUTDIR.
mj_knowledge_check_all() {
  local tmp="$1" f
  set --
  while IFS= read -r f; do [ -n "$f" ] && set -- "$@" "$f"; done <<EOF
$(mj_knowledge_record_files)
EOF
  mj_knowledge_check_files "$tmp" "$@"
}

# majordomus knowledge check
mj_knowledge_check_cmd() {
  while [ $# -gt 0 ]; do case "$1" in
    --help|-h) mj_knowledge_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "knowledge check: unknown option $1" ;;
  esac; done
  local tmp n f failing; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.kch.XXXXXX")"
  mj_knowledge_check_all "$tmp"
  n="$(mj_lines "$tmp/records")"
  failing="$(cut -f1 "$tmp/findings" | LC_ALL=C sort -u | grep -c . || true)"
  local rel code msg first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"records":%s,"failing":%s,"findings":[' "$n" "$failing"
    while IFS="$MJ_TAB" read -r rel code msg; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"path":"%s","code":"%s","message":"%s"}' "$(mj_json_esc "$rel")" "$code" "$(mj_json_esc "$msg")"
    done < "$tmp/findings"
    printf ']}\n'
  else
    while IFS= read -r f; do
      [ -n "$f" ] || continue
      if grep -q "^$(printf '%s' "$f" | sed 's/[][\.*^$]/\\&/g')$MJ_TAB" "$tmp/findings"; then
        grep "^$(printf '%s' "$f" | sed 's/[][\.*^$]/\\&/g')$MJ_TAB" "$tmp/findings" | while IFS="$MJ_TAB" read -r rel code msg; do
          mj_fail knowledge "$rel" "$msg" "majordomus knowledge check"
        done
      else
        mj_ok knowledge "$f" "valid against knowledge/v1; every reference resolves"
      fi
    done < "$tmp/records"
    printf 'knowledge check: %s record(s), %s failing\n' "$n" "$failing"
  fi
  rm -rf "$tmp"
  [ "$failing" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}

# ---------------------------------------------------------------- doctrine
# Three validators, declared by three rules of the standard package
# (majordomus.knowledge-observed, majordomus.knowledge-integrity,
# majordomus.candidates-reviewed) and dispatched from the doctrine registry. Each returns 0
# and reports only through mj_doctrine_fail / ok / skip.

# The ADR 0052 invariant, applied to this writer: while episodes keep closing, a knowledge
# derivation follows every close. Two halves. The wiring half reads the source: the close
# path and the compaction adapter must call the deriver while their switches are on. The
# freshness half reads the ledger: the newest closed episode is named by a knowledge.derived
# line, or it is younger than the stale threshold. It compares episode ids, never line
# order or timestamps between the two events, because the close derives before it appends
# session.closed. It judges only once a derivation has run in this checkout: the day the
# writer arrives, no checkout turns red for what it could not yet have done.
mj_validate_knowledge_lifecycle() {
  local led bad=0; led="$(mj_knowledge_ledger)"
  if [ "$(mj_pol session.knowledge_on_end)" != false ] && ! grep -q 'mj_cmd_knowledge derive' "$MJ_LIB_DIR/session.sh" 2>/dev/null; then
    mj_doctrine_fail knowledge "lib/session.sh" "session.knowledge_on_end is on and the close path never calls the deriver" "grep -n 'mj_cmd_knowledge derive' lib/session.sh"; bad=1
  fi
  if [ "$(mj_pol session.knowledge_on_compact)" != false ] && ! grep -q 'mj_cmd_knowledge derive' "$MJ_LIB_DIR/capture.sh" 2>/dev/null; then
    mj_doctrine_fail knowledge "lib/capture.sh" "session.knowledge_on_compact is on and the compaction adapter never calls the deriver" "grep -n 'mj_cmd_knowledge derive' lib/capture.sh"; bad=1
  fi
  [ "$bad" = 0 ] && mj_doctrine_ok knowledge "wiring" "the close path and the compaction adapter call the deriver"

  if [ ! -f "$led" ]; then mj_doctrine_ok knowledge "deriver" "no ledger yet; no episode has closed here"; return 0; fi
  local closed; closed="$(grep -c '"event":"session.closed"' "$led" 2>/dev/null || true)"; : "${closed:=0}"
  if [ "$closed" = 0 ]; then mj_doctrine_ok knowledge "deriver" "no episode has closed here; nothing to judge"; return 0; fi
  if [ "$(mj_pol session.knowledge_on_end)" = false ]; then
    mj_doctrine_skip knowledge "deriver" "session.knowledge_on_end is false; the deriver is switched off here and its silence is not judged"
    [ "$bad" = 0 ] && MJ_DOCTRINE_SKIPPED=1
    return 0
  fi
  local derived; derived="$(grep -c '"event":"knowledge.derived"' "$led" 2>/dev/null || true)"; : "${derived:=0}"
  if [ "$derived" = 0 ]; then
    mj_doctrine_skip knowledge "deriver" "no knowledge derivation has run in this checkout yet, so the newest close is not judged; the first `majordomus knowledge derive` starts the judgement"
    [ "$bad" = 0 ] && MJ_DOCTRINE_SKIPPED=1
    return 0
  fi
  local line stamp ts age stale
  line="$(grep -F '"event":"session.closed"' "$led" | tail -n 1)"
  stamp="$(mj_json_field "$line" session)"; ts="$(mj_json_field "$line" ts)"
  if [ -z "$stamp" ]; then
    mj_doctrine_skip knowledge "deriver" "the newest session.closed line names no episode; nothing to compare a derivation against"
    [ "$bad" = 0 ] && MJ_DOCTRINE_SKIPPED=1
    return 0
  fi
  if grep -F '"event":"knowledge.derived"' "$led" | grep -Fq "\"episode\":\"$stamp\""; then
    mj_doctrine_ok knowledge "deriver" "the newest closed episode ($stamp) was followed by a knowledge derivation"
    return 0
  fi
  if ! stale="$(mj_pol_req session.freshness.stale_minutes)"; then
    mj_doctrine_fail knowledge "deriver" "policy declares no session.freshness.stale_minutes" "add it under session.freshness: in $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"; return 0
  fi
  age="$(mj_age_minutes "$ts" || true)"
  if [ -z "$age" ]; then
    mj_doctrine_fail knowledge "deriver" "the newest session.closed line cannot be dated ($ts), so nothing can say whether the writer is still running" "majordomus history --event session.closed"
  elif [ "$age" -ge "$stale" ]; then
    mj_doctrine_fail knowledge "deriver" \
      "episodes close and no knowledge.derived followed the newest ($stamp, $(mj_age_human "$age")); the writer has stopped. fix: majordomus knowledge derive --episode $stamp" \
      "majordomus history --event session.closed; majordomus history --event knowledge.derived"
  else
    mj_doctrine_ok knowledge "deriver" "the newest closed episode ($stamp) awaits its derivation ($(mj_age_human "$age"), under the $(mj_duration_human "$stale") this repository calls stale)"
  fi
  return 0
}

# Every record is schema-valid, evidenced, uniquely identified, resolves what it names and
# carries no conversation. `majordomus knowledge check` is the same judgement as a command.
mj_validate_knowledge_integrity() {
  local cdir kdir; cdir="$(mj_knowledge_candidates_dir)"; kdir="$(mj_knowledge_curated_dir)"
  if [ ! -d "$cdir" ] && [ ! -d "$kdir" ]; then
    mj_doctrine_ok knowledge "records" "no knowledge records; nothing to validate"; return 0
  fi
  local tmp n rel code msg bad=0; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.kvi.XXXXXX")"
  mj_knowledge_check_all "$tmp"
  n="$(mj_lines "$tmp/records")"
  while IFS="$MJ_TAB" read -r rel code msg; do
    [ -n "$rel" ] || continue
    mj_doctrine_fail knowledge "$rel" "$code: $msg" "majordomus knowledge check"; bad=1
  done < "$tmp/findings"
  rm -rf "$tmp"
  [ "$bad" = 0 ] && mj_doctrine_ok knowledge "records" "$n record(s) valid against knowledge/v1; every reference resolves" "majordomus knowledge check"
  return 0
}

# Candidates are reviewed, not accumulated. More records awaiting review than the policy's
# cap, or one waiting longer than the policy's age, is a finding that names them; both
# numbers are declared once in the policy. A candidate's review age is the age of its
# queue entry — the oldest knowledge.derived line naming the file, else the commit that
# added it, else its own date — and the finding says which. A candidate nobody has staged
# reaches no other surface, and is named for the reason an unstaged session record is.
mj_validate_knowledge_accumulation() {
  local cap max_age cdir rel; cdir="$(mj_knowledge_candidates_dir)"; rel="$(mj_rel "$cdir")"
  if ! cap="$(mj_pol_req knowledge.candidates_max_files)"; then
    mj_doctrine_fail knowledge "candidates" "policy declares no knowledge.candidates_max_files" "add a knowledge: block to $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"; return 0
  fi
  if ! max_age="$(mj_pol_req knowledge.candidate_max_age_minutes)"; then
    mj_doctrine_fail knowledge "candidates" "policy declares no knowledge.candidate_max_age_minutes" "add it under knowledge: in $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"; return 0
  fi
  [ -d "$cdir" ] || { mj_doctrine_ok knowledge "candidates" "none await review, cap $cap"; return 0; }
  local rows n; rows="$(mktemp "${TMPDIR:-/tmp}/mj.kva.XXXXXX")"
  mj_knowledge_candidate_rows | awk -F'\t' '$3 == "candidate"' > "$rows"
  n="$(mj_lines "$rows")"
  if [ "$n" -gt "$cap" ]; then
    mj_doctrine_fail knowledge "candidates" "$n candidate(s) await review, over cap $cap: $(cut -f1 "$rows" | head -n 3 | tr '\n' ' ')$( [ "$n" -gt 3 ] && printf '…' )" "majordomus knowledge candidates"
  else
    mj_doctrine_ok knowledge "candidates" "$n await review, cap $cap"
  fi
  # the queue entry's age, from the ledger in one pass: the first derivation naming each path
  local led entered; led="$(mj_knowledge_ledger)"; entered="$(mktemp "${TMPDIR:-/tmp}/mj.kve.XXXXXX")"
  if [ -f "$led" ]; then
    awk 'index($0, "\"event\":\"knowledge.derived\"") == 0 { next }
         { ts = $0; sub(/^.*"ts":"/, "", ts); sub(/".*$/, "", ts)
           p = $0; sub(/^.*"paths":"/, "", p); sub(/".*$/, "", p)
           n = split(p, a, " "); for (i = 1; i <= n; i++) if (a[i] != "" && !(a[i] in seen)) { seen[a[i]] = 1; print a[i] "\t" ts } }' "$led" > "$entered"
  fi
  local id cls st epi date ep br path title ts src age old=0 named=""
  while IFS="$MJ_TAB" read -r id cls st epi date ep br path title; do
    ts="$(awk -F'\t' -v p="$path" '$1 == p { print $2; exit }' "$entered")"; src="the derivation that wrote it"
    if [ -z "$ts" ]; then
      # in UTC, whatever zone the committer was in: the clock this is compared against is UTC
      ts="$(TZ=UTC mj_git log --diff-filter=A --date=iso-strict-local --format=%cd -1 -- "$path" 2>/dev/null | sed 's/+00:00$/Z/')"
      src="the commit that added it"
    fi
    case "$ts" in *Z) ;; *) ts="" ;; esac
    if [ -z "$ts" ]; then ts="${date}T00:00:00Z"; src="its own date"; fi
    age="$(mj_age_minutes "$ts" || true)"
    [ -n "$age" ] && [ "$age" -ge "$max_age" ] || continue
    old=$((old + 1)); [ "$old" -le 3 ] && named="$named $id"
    mj_doctrine_fail knowledge "$path" "awaiting review for $(mj_age_human "$age"), older than knowledge.candidate_max_age_minutes ($(mj_duration_human "$max_age")), dated by $src; promote it or reject it" "majordomus knowledge candidates"
  done < "$rows"
  [ "$old" = 0 ] && [ "$n" -gt 0 ] && mj_doctrine_ok knowledge "review age" "no candidate has waited past $(mj_duration_human "$max_age")"
  rm -f "$rows" "$entered"
  # staged nowhere, seen nowhere
  local orphans="" no=0 name
  while IFS= read -r name; do
    [ -n "$name" ] || continue
    mj_is_context_doc "$cdir/$name" && continue     # the section's own contract is not a record
    no=$((no + 1)); [ "$no" -le 3 ] && orphans="$orphans $name"
  done <<EOF
$(cd "$cdir" 2>/dev/null && git ls-files --others --exclude-standard -- '*.md' 2>/dev/null)
EOF
  [ "$no" -gt 0 ] && mj_doctrine_fail knowledge "$rel/" \
    "$no candidate(s) exist here and are committed nowhere, so they reach no other surface:$orphans$( [ "$no" -gt 3 ] && printf ' …' ). fix: git add $rel && majordomus derive" \
    "git status --porcelain $rel"
  return 0
}
