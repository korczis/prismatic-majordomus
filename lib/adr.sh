#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_adr:-}" ] && return 0 || MJ_LIB_adr=1
# adr — the repository's architecture decisions, as data.
#
# An ADR is one Markdown file under the manifest's adrs section: YAML front matter (the
# contract in share/schemas/adr.schema.json) over a body carrying Context, Decision and
# Consequences. Nothing here is a registry: which files are decisions is decided by the
# source class `adr` in .ai/repo/knowledge/sources.yaml, the same declaration the Rust
# executable indexes and serves as majordomus://adr/<id>, so the two never disagree about
# what exists. This file reads that discovery, validates against the allow-list generated
# from the schema, and projects.
#
# Two things this command will not do, and both are the point:
#
#   propose never writes `accepted`. A decision the tool derived from a local record is a
#   candidate for a person to read, and a candidate that can be born accepted is a way for
#   an inference to become repository truth by being written down.
#
#   an identity is allocated under a lock and never reused. Two worktrees proposing at the
#   same moment is the ordinary case here, and it is exactly how this repository ended up
#   with two 0005s and two 0007s before anything checked.

# shellcheck source=knowledge.sh
. "$MJ_LIB_DIR/knowledge.sh"

MJ_ADR_SCHEMA="adr/v1"
MJ_ADR_STATUSES="proposed accepted superseded rejected"
MJ_ADR_ORIGINS="authored extracted"
MJ_ADR_SECTIONS="Context Decision Consequences"
# The reference vocabulary, shared by the decision and knowledge records: a
# reference is <type>:<value>, and a file: or test: reference must resolve.
MJ_ADR_REF_TYPES="decision session commit issue file test"

mj_cmd_adr() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    list) mj_adr_list "$@" ;;
    show) [ $# -ge 1 ] || mj_die "$MJ_EX_USAGE" "adr show: an adr id is required"; mj_adr_show "$@" ;;
    propose) mj_adr_propose "$@" ;;
    check) mj_adr_check "$@" ;;
    --help|-h|"") mj_adr_usage; [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "adr: unknown subcommand '$sub' (list|show|propose|check)" ;;
  esac
}

mj_adr_usage() {
  cat <<H
usage: majordomus adr list [--status <status>] [--json]      every decision: id, status, date, title
       majordomus adr show <id> [--json]                     one decision: its path, then the file as written
       majordomus adr propose "<title>" [--from <ref>]...    write a new decision with status: proposed
                              [--tag <tag>]... [--supersedes <id>]
       majordomus adr check [--json]                         validate every decision and every reference it makes
  a decision is $(mj_rel "$MJ_ADRS_DIR")/<NNNN>-<slug>.md: front matter (schema: $MJ_ADR_SCHEMA, id: adr-NNNN,
  kind, title, status: $(printf '%s' "$MJ_ADR_STATUSES" | sed 's/ /|/g'), date; optional tags, supersedes, superseded_by, provenance)
  over a body with the sections $(printf '%s' "$MJ_ADR_SECTIONS" | sed -e 's/ /, # /g' -e 's/^/# /')
  propose writes 'proposed' and refuses to write any other status: accepting a decision is a person's act
  a --from reference is <type>:<value>, the type one of $(printf '%s' "$MJ_ADR_REF_TYPES" | sed 's/ /, /g')
  discovery is the source class 'adr' in $(mj_rel "$MJ_KNOWLEDGE_DIR")/sources.yaml, shared with the Rust executable
H
}

# ---------------------------------------------------------------- discovery
# One row per decision file the source class discovers, in discovery order (the index
# order, which is path order): repository-relative path <TAB> sha256.
mj_adr_files() {
  mj_knowledge_discover shared | awk -F'\t' '$1 == "adr" { printf "%s\t%s\n", $5, $4 }'
}

# ---------------------------------------------------------------- one record
# The flattened front matter of the record last loaded, and why the load failed when it
# did. The reason is a variable rather than output because every caller needs the flat
# file afterwards, and a command substitution would take it into a subshell and leave the
# caller reading the previous record's fields.
MJ_ADR_FLAT=""
MJ_ADR_ERROR=""
mj_adr_load() {
  local f="$1" fm flat
  MJ_ADR_ERROR=""
  [ -n "$MJ_ADR_FLAT" ] && rm -f "$MJ_ADR_FLAT"
  fm="$(mktemp "${TMPDIR:-/tmp}/mj.af.XXXXXX")"; flat="$(mktemp "${TMPDIR:-/tmp}/mj.al.XXXXXX")"; MJ_ADR_FLAT="$flat"
  if ! mj_record_front "$f" > "$fm" 2>/dev/null; then rm -f "$fm"; MJ_ADR_ERROR="no front matter"; return 1; fi
  if ! mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then rm -f "$fm"; MJ_ADR_ERROR="malformed front matter"; return 1; fi
  rm -f "$fm"; return 0
}
# "-" is the placeholder an empty field is written as; every reader normalises it back.
mj_adr_un() { [ "$1" = - ] && printf '' || printf '%s' "$1"; }

mj_adr_get()  { mj_yget "$MJ_ADR_FLAT" "$1"; }
mj_adr_lst()  { mj_ylist "$MJ_ADR_FLAT" "$1"; }

# mj_adr_ref_valid REF — the reference is <type>:<value> with a known type, and a file:
# or a commit: reference resolves. Prints the reason and exits 1 when it does not.
mj_adr_ref_valid() {
  local ref="$1" t="${1%%:*}" v="${1#*:}"
  case "$ref" in *:*) ;; *) printf 'reference "%s" is not <type>:<value>\n' "$ref"; return 1 ;; esac
  case " $MJ_ADR_REF_TYPES " in
    *" $t "*) ;;
    *) printf 'reference "%s" has an unknown type "%s"\n' "$ref" "$t"; return 1 ;;
  esac
  [ -n "$v" ] || { printf 'reference "%s" has an empty value\n' "$ref"; return 1; }
  case "$t" in
    file|test) [ -e "$MJ_ROOT/$v" ] || { printf 'reference "%s" names a path that does not exist\n' "$ref"; return 1; } ;;
  esac
  return 0
}

# mj_adr_validate FILE BASENAME — every reason on its own line; exit 1 when any. Reads
# only this file; cross-record checks (unique ids, reciprocal supersession) are
# mj_adr_check's, which has the whole catalogue in hand.
mj_adr_validate() {
  local f="$1" base="$2"
  mj_adr_load "$f" || { printf '%s\n' "$MJ_ADR_ERROR"; return 1; }
  mj_adr_validate_loaded "$f" "$base"
}
mj_adr_validate_loaded() {
  local f="$1" base="$2" rc=0 v s h unk missing="" origin refs n
  unk="$(mj_yaml_unknown_keys "$MJ_ADR_FLAT" "$MJ_ALLOW_DIR/adr.txt" || true)"
  [ -n "$unk" ] && { printf 'unknown front-matter key(s): %s\n' "$(printf '%s' "$unk" | tr '\n' ' ' | sed 's/ $//')"; rc=1; }
  v="$(mj_adr_get schema)"
  [ "$v" = "$MJ_ADR_SCHEMA" ] || { printf 'schema must be %s (found "%s")\n' "$MJ_ADR_SCHEMA" "$v"; rc=1; }
  [ "$(mj_adr_get kind)" = adr ] || { printf 'kind must be adr (found "%s")\n' "$(mj_adr_get kind)"; rc=1; }
  v="$(mj_adr_get id)"
  if [ -z "$v" ]; then printf 'id is missing\n'; rc=1
  else
    case "$v" in
      adr-[0-9][0-9][0-9][0-9]) ;;
      *) printf 'id "%s" is not adr-NNNN\n' "$v"; rc=1 ;;
    esac
    # the identity fixes the file name, so a retitle cannot silently orphan a reference
    case "$base" in
      "${v#adr-}"-*.md) ;;
      *) printf 'id "%s" does not match the file name "%s" (expected %s-<slug>.md)\n' "$v" "$base" "${v#adr-}"; rc=1 ;;
    esac
  fi
  [ -n "$(mj_adr_get title)" ] || { printf 'title is empty\n'; rc=1; }
  s="$(mj_adr_get status)"
  case " $MJ_ADR_STATUSES " in *" $s "*) ;; *) printf 'status must be one of %s (found "%s")\n' "$(printf '%s' "$MJ_ADR_STATUSES" | sed 's/ /, /g')" "$s"; rc=1 ;; esac
  v="$(mj_adr_get date)"
  case "$v" in
    [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) ;;
    *) printf 'date must be YYYY-MM-DD (found "%s")\n' "$v"; rc=1 ;;
  esac
  for v in $(mj_adr_lst tags); do
    case "$v" in *[!a-z0-9-]*|[!a-z]*) printf 'tag "%s" is not lower-case letters, digits and hyphens starting with a letter\n' "$v"; rc=1 ;; esac
  done
  # superseded_by is present exactly when the status is superseded: a record that says it
  # was replaced and does not say by what is a dangling end of the chain
  v="$(mj_adr_get superseded_by)"
  if [ "$s" = superseded ] && [ -z "$v" ]; then printf 'status is superseded but superseded_by is missing\n'; rc=1; fi
  if [ "$s" != superseded ] && [ -n "$v" ]; then printf 'superseded_by is set but the status is "%s", not superseded\n' "$s"; rc=1; fi
  # provenance: an extracted record is a candidate for a person to read, and it says what
  # it was derived from. Nothing the tool wrote may claim to have been accepted.
  origin="$(mj_adr_get provenance.origin)"
  if [ -n "$origin" ]; then
    case " $MJ_ADR_ORIGINS " in *" $origin "*) ;; *) printf 'provenance.origin must be one of %s (found "%s")\n' "$(printf '%s' "$MJ_ADR_ORIGINS" | sed 's/ /, /g')" "$origin"; rc=1 ;; esac
  fi
  refs="$(mj_adr_lst provenance.derived_from)"; n=0
  for v in $refs; do n=$((n + 1)); mj_adr_ref_valid "$v" || rc=1; done
  if [ "$origin" = extracted ]; then
    [ "$n" -gt 0 ] || { printf 'provenance.origin is extracted but derived_from names nothing: an extracted record without evidence is an assertion\n'; rc=1; }
    # only `accepted` is refused, and for one reason: acceptance is the person's act, and a
    # tool that can write it turns its own inference into repository truth. Every other
    # status is something that happened to the proposal rather than a decision it claims —
    # rejected, or superseded once a later record stood in for it — and a candidate the
    # person never accepted can still be replaced by one they did.
    if [ "$s" = accepted ]; then printf 'provenance.origin is extracted and status is "%s": a record the tool derived is proposed until a person accepts it\n' "$s"; rc=1; fi
  fi
  # the body is the decision: the sections a reader relies on exist and are not empty
  for h in $MJ_ADR_SECTIONS; do
    if ! mj_record_body "$f" | awk -v h="## $h" '
        $0 == h { in_s = 1; found = 1; next }
        in_s && /^## / { in_s = 0 }
        in_s && NF { body = 1 }
        END { exit (found && body) ? 0 : 1 }'; then missing="$missing $h"; fi
  done
  [ -n "$missing" ] && { printf 'body is missing or empty in section(s):%s\n' "$missing"; rc=1; }
  return "$rc"
}

# ---------------------------------------------------------------- the catalogue
# One pass over discovery producing one row per decision, in id order:
#   path <TAB> id <TAB> status <TAB> date <TAB> title <TAB> superseded_by <TAB> supersedes(comma) <TAB> reasons(;-joined, empty when valid)
# Every consumer below reads this; a second walker would be a second order.
MJ_ADR_N=0
MJ_ADR_INVALID=0
MJ_ADR_ROWS=""
mj_adr_catalogue() {
  local tmp out f base reasons id st date title sb sup
  [ -n "$MJ_ADR_ROWS" ] && [ -f "$MJ_ADR_ROWS" ] && return 0
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ac.XXXXXX")"
  out="$(mktemp "${TMPDIR:-/tmp}/mj.ao.XXXXXX")"; MJ_ADR_ROWS="$out"
  MJ_ADR_N=0; MJ_ADR_INVALID=0
  # The loop stays in this shell so that the counts survive it, and the front matter is
  # loaded here rather than inside the validator's command substitution: a subshell would
  # take MJ_ADR_FLAT with it and every field below would read the previous record's.
  while IFS="$MJ_TAB" read -r f _; do
    [ -n "$f" ] || continue
    base="${f##*/}"
    MJ_ADR_N=$((MJ_ADR_N + 1))
    if mj_adr_load "$MJ_ROOT/$f"; then
      if reasons="$(mj_adr_validate_loaded "$MJ_ROOT/$f" "$base")"; then reasons=""; else MJ_ADR_INVALID=$((MJ_ADR_INVALID + 1)); fi
    else
      reasons="$MJ_ADR_ERROR"; MJ_ADR_INVALID=$((MJ_ADR_INVALID + 1))
    fi
    reasons="$(printf '%s' "$reasons" | tr '\n' ';' | sed 's/;$//')"
    id="$(mj_adr_get id)"; st="$(mj_adr_get status)"; date="$(mj_adr_get date)"
    title="$(mj_adr_get title)"; sb="$(mj_adr_get superseded_by)"
    sup="$(mj_adr_lst supersedes | tr '\n' ',' | sed 's/,$//')"
    # a tab is whitespace, so `read` collapses a run of them and one empty field would
    # shift every field after it; "-" stands for empty and is read back as empty
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$f" "${id:--}" "${st:--}" "${date:--}" "${title:--}" "${sb:--}" "${sup:--}" "${reasons:--}" >> "$tmp"
  done < <(mj_adr_files)
  sort -t"$MJ_TAB" -k2,2 "$tmp" > "$out"
  rm -f "$tmp"
  return 0
}

# ---------------------------------------------------------------- list
mj_adr_list() {
  local want=""
  while [ $# -gt 0 ]; do case "$1" in
    --status) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--status needs a value"; want="$2"; shift 2 ;;
    --status=*) want="${1#--status=}"; shift ;;
    --json) MJ_JSON=1; shift ;;
    *) mj_die "$MJ_EX_USAGE" "adr list: unknown option $1" ;;
  esac; done
  [ -n "$want" ] && { case " $MJ_ADR_STATUSES " in *" $want "*) ;; *) mj_die "$MJ_EX_USAGE" "adr list: --status must be one of $(printf '%s' "$MJ_ADR_STATUSES" | sed 's/ /, /g')" ;; esac; }
  mj_require_installed
  mj_adr_catalogue
  local f id st date title sb sup reasons first=1 n=0
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"status":"%s","adrs":[' "${want:-all}"
    while IFS="$MJ_TAB" read -r f id st date title sb sup reasons; do
      sb="$(mj_adr_un "$sb")"; sup="$(mj_adr_un "$sup")"; reasons="$(mj_adr_un "$reasons")"
      [ -n "$want" ] && [ "$st" != "$want" ] && continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"id":"%s","status":"%s","date":"%s","title":"%s","path":"%s","superseded_by":%s,"supersedes":[' \
        "$id" "$st" "$date" "$(mj_json_esc "$title")" "$(mj_json_esc "$f")" \
        "$([ -n "$sb" ] && printf '"%s"' "$sb" || printf 'null')"
      local sfirst=1 s
      for s in $(printf '%s' "$sup" | tr ',' ' '); do [ "$sfirst" = 1 ] || printf ','; printf '"%s"' "$s"; sfirst=0; done
      printf '],"valid":%s}' "$([ -z "$reasons" ] && printf true || printf false)"
    done < "$MJ_ADR_ROWS"
    printf ']}\n'
    return 0
  fi
  while IFS="$MJ_TAB" read -r f id st date title sb sup reasons; do
    sb="$(mj_adr_un "$sb")"; sup="$(mj_adr_un "$sup")"; reasons="$(mj_adr_un "$reasons")"
    [ -n "$want" ] && [ "$st" != "$want" ] && continue
    n=$((n + 1))
    printf '%-10s %-11s %s  %s%s\n' "$id" "$st" "$date" "$title" "$([ -n "$sb" ] && printf ' (superseded by %s)' "$sb")"
  done < "$MJ_ADR_ROWS"
  [ "$n" = 0 ] && printf '(none)\n'
  return 0
}

# ---------------------------------------------------------------- show
mj_adr_show() {
  local want=""
  while [ $# -gt 0 ]; do case "$1" in
    --json) MJ_JSON=1; shift ;;
    -*) mj_die "$MJ_EX_USAGE" "adr show: unknown option $1" ;;
    *) want="$1"; shift ;;
  esac; done
  mj_require_installed
  # a bare number is the ordinary way a person refers to a decision
  case "$want" in [0-9][0-9][0-9][0-9]) want="adr-$want" ;; esac
  mj_adr_catalogue
  local f id st date title sb sup reasons found=""
  while IFS="$MJ_TAB" read -r f id st date title sb sup reasons; do
    sb="$(mj_adr_un "$sb")"; sup="$(mj_adr_un "$sup")"; reasons="$(mj_adr_un "$reasons")"
    [ "$id" = "$want" ] && { found="$f"; break; }
  done < "$MJ_ADR_ROWS"
  [ -n "$found" ] || { mj_err "adr show: no decision with id '$want' (see: majordomus adr list)"; return "$MJ_EX_MISSING"; }
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"id":"%s","status":"%s","date":"%s","title":"%s","path":"%s","valid":%s,"body":"%s"}\n' \
      "$id" "$st" "$date" "$(mj_json_esc "$title")" "$(mj_json_esc "$found")" \
      "$([ -z "$reasons" ] && printf true || printf false)" \
      "$(mj_json_esc "$(mj_record_body "$MJ_ROOT/$found")")"
    return 0
  fi
  printf '%s\n\n' "$found"
  cat "$MJ_ROOT/$found"
  return 0
}

# ---------------------------------------------------------------- propose
# The identity is allocated under an exclusive lock over the decisions directory: mkdir is
# the one create-or-fail primitive every POSIX filesystem gives us, so two worktrees
# proposing in the same second get two identities rather than one. The file is written to a
# temporary beside its destination and moved into place, so a reader never sees half a
# record and an interrupted propose leaves nothing.
mj_adr_next_id() {
  local n max=0 num
  for n in "$MJ_ADRS_DIR"/[0-9][0-9][0-9][0-9]-*.md; do
    [ -e "$n" ] || continue
    num="${n##*/}"; num="${num%%-*}"
    num="$(printf '%s' "$num" | sed 's/^0*//')"; [ -z "$num" ] && num=0
    [ "$num" -gt "$max" ] && max="$num"
  done
  printf '%04d' "$((max + 1))"
}

mj_adr_slug() {
  printf '%s' "$1" | tr 'A-Z' 'a-z' | sed -e 's/[^a-z0-9]\{1,\}/-/g' -e 's/^-//' -e 's/-$//' | cut -c1-60
}

mj_adr_propose() {
  local title="" tags="" refs="" supersedes=""
  while [ $# -gt 0 ]; do case "$1" in
    --from) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--from needs a <type>:<value> reference"; refs="$refs $2"; shift 2 ;;
    --from=*) refs="$refs ${1#--from=}"; shift ;;
    --tag) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--tag needs a tag"; tags="$tags $2"; shift 2 ;;
    --tag=*) tags="$tags ${1#--tag=}"; shift ;;
    --supersedes) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--supersedes needs an adr id"; supersedes="$supersedes $2"; shift 2 ;;
    --supersedes=*) supersedes="$supersedes ${1#--supersedes=}"; shift ;;
    --status|--status=*) mj_die "$MJ_EX_REFUSED" "adr propose: the status is not yours to choose; a proposed decision is accepted by a person editing the record" ;;
    --json) MJ_JSON=1; shift ;;
    -*) mj_die "$MJ_EX_USAGE" "adr propose: unknown option $1" ;;
    *) [ -z "$title" ] || mj_die "$MJ_EX_USAGE" "adr propose: the title must be one argument (quote it)"; title="$1"; shift ;;
  esac; done
  [ -n "$title" ] || mj_die "$MJ_EX_USAGE" "adr propose: the title is required"
  mj_is_multiline "$title" && mj_die "$MJ_EX_USAGE" "adr propose: the title must be single-line"
  mj_require_installed
  [ -d "$MJ_ADRS_DIR" ] || mj_die "$MJ_EX_MISSING" "no $(mj_rel "$MJ_ADRS_DIR")/ in this repository (the manifest names it)"

  local r
  for r in $refs; do mj_adr_ref_valid "$r" >/dev/null || mj_die "$MJ_EX_USAGE" "adr propose: $(mj_adr_ref_valid "$r")"; done
  for r in $tags; do
    case "$r" in *[!a-z0-9-]*|[!a-z]*) mj_die "$MJ_EX_USAGE" "adr propose: tag '$r' is not lower-case letters, digits and hyphens starting with a letter" ;; esac
  done
  for r in $supersedes; do
    case "$r" in adr-[0-9][0-9][0-9][0-9]) ;; *) mj_die "$MJ_EX_USAGE" "adr propose: --supersedes '$r' is not adr-NNNN" ;; esac
    ls "$MJ_ADRS_DIR/${r#adr-}"-*.md >/dev/null 2>&1 || mj_die "$MJ_EX_USAGE" "adr propose: --supersedes '$r' matches no decision here"
  done

  local lock num slug dest tmp origin=extracted waited=0
  lock="$MJ_ADRS_DIR/.id.lock"
  while ! mkdir "$lock" 2>/dev/null; do
    waited=$((waited + 1))
    [ "$waited" -gt 100 ] && mj_die "$MJ_EX_INTERNAL" "adr propose: the identity lock $(mj_rel "$lock") has been held for too long; remove it if no other worker is proposing"
    sleep 0.1 2>/dev/null || sleep 1
  done
  # from here the identity is ours until the record exists; every exit releases the lock
  trap 'rmdir "'"$lock"'" 2>/dev/null || true' EXIT
  num="$(mj_adr_next_id)"
  slug="$(mj_adr_slug "$title")"
  [ -n "$slug" ] || slug="decision"
  dest="$MJ_ADRS_DIR/$num-$slug.md"
  tmp="$dest.tmp.$$"
  [ -n "$refs" ] || origin=authored

  {
    printf -- '---\nschema: %s\nid: adr-%s\nkind: adr\ntitle: %s\nstatus: proposed\ndate: %s\n' \
      "$MJ_ADR_SCHEMA" "$num" "$title" "$(date -u +%Y-%m-%d)"
    if [ -n "$tags" ]; then printf 'tags:\n'; for r in $tags; do printf -- '  - %s\n' "$r"; done; fi
    if [ -n "$supersedes" ]; then printf 'supersedes:\n'; for r in $supersedes; do printf -- '  - %s\n' "$r"; done; fi
    printf 'provenance:\n  origin: %s\n' "$origin"
    if [ -n "$refs" ]; then printf '  derived_from:\n'; for r in $refs; do printf -- '    - %s\n' "$r"; done; fi
    printf -- '---\n\n# %s. %s\n\n' "$((10#$num))" "$title"
    printf '## Context\n\nWhat forced the decision. %s\n\n' \
      "$([ -n "$refs" ] && printf 'Derived from:%s.' "$(printf '%s' "$refs" | sed 's/^/ /')" || printf 'Written by hand; nothing was derived.')"
    printf '## Decision\n\n%s\n\n' "$title"
    printf '## Alternatives rejected\n\nWhat else was considered, and why it was not taken.\n\n'
    printf '## Consequences\n\nWhat this costs, what it forecloses, and what now has to be true.\n'
  } > "$tmp"
  mv "$tmp" "$dest"
  rmdir "$lock" 2>/dev/null || true
  trap - EXIT

  mj_ledger_append adr.proposed "\"adr\":\"adr-$num\",\"title\":\"$(mj_json_esc "$title")\""
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"id":"adr-%s","status":"proposed","path":"%s"}\n' "$num" "$(mj_json_esc "$(mj_rel "$dest")")"
  else
    printf 'proposed: adr-%s  %s\n%s\n' "$num" "$title" "$(mj_rel "$dest")"
    printf 'status is "proposed"; accepting it is a person editing that field.\n'
  fi
  return 0
}

# ---------------------------------------------------------------- check
# The whole catalogue: per-record validity, then the checks only the set can answer —
# identities are unique, a superseded record's replacement exists, and supersession is
# reciprocal so that the chain can be walked from either end.
mj_adr_check() {
  while [ $# -gt 0 ]; do case "$1" in
    --json) MJ_JSON=1; shift ;;
    *) mj_die "$MJ_EX_USAGE" "adr check: unknown option $1" ;;
  esac; done
  mj_require_installed
  local out; out="$(mktemp "${TMPDIR:-/tmp}/mj.ack.XXXXXX")"
  local rc=0
  mj_adr_examine mj_adr_report_line > "$out" || rc=$?
  if [ "$MJ_JSON" = 1 ]; then
    local first=1 line
    printf '{"schema":1,"examined":%s,"findings":[' "$MJ_ADR_N"
    while IFS= read -r line; do
      [ -n "$line" ] || continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"finding":"%s"}' "$(mj_json_esc "$line")"
    done < "$out"
    printf '],"ok":%s}\n' "$([ "$rc" = 0 ] && printf true || printf false)"
  else
    if [ "$rc" = 0 ]; then
      printf 'examined %s decision(s) in %s/\n' "$MJ_ADR_N" "$(mj_rel "$MJ_ADRS_DIR")"
      printf 'every identity unique, every status known, every reference resolves\n'
    else
      cat "$out"
      printf '\n%s decision(s) examined, %s with findings\n' "$MJ_ADR_N" "$MJ_ADR_FINDINGS"
    fi
  fi
  rm -f "$out"
  [ "$rc" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}

# mj_adr_examine REPORTER — runs every check and calls REPORTER <path> <reason> for each
# finding. The doctrine validator and `adr check` share it, so the two never disagree.
MJ_ADR_FINDINGS=0
mj_adr_examine() {
  local report="$1" f id st date title sb sup reasons r rc=0
  MJ_ADR_FINDINGS=0
  mj_adr_catalogue
  # per record
  while IFS="$MJ_TAB" read -r f id st date title sb sup reasons; do
    sb="$(mj_adr_un "$sb")"; sup="$(mj_adr_un "$sup")"; reasons="$(mj_adr_un "$reasons")"
    [ -n "$reasons" ] || continue
    local one
    printf '%s\n' "$reasons" | tr ';' '\n' | while IFS= read -r one; do
      [ -n "$one" ] && "$report" "$f" "$one"
    done
    MJ_ADR_FINDINGS=$((MJ_ADR_FINDINGS + 1)); rc=1
  done < "$MJ_ADR_ROWS"
  # identities are unique. This is the check that was missing when two worktrees each
  # allocated 0005 and 0007 within hours of one another.
  local dup
  dup="$(awk -F"$MJ_TAB" '$2 != "" { c[$2]++; p[$2] = p[$2] " " $1 } END { for (k in c) if (c[k] > 1) printf "%s\t%s\n", k, p[k] }' "$MJ_ADR_ROWS")"
  if [ -n "$dup" ]; then
    while IFS="$MJ_TAB" read -r id sup; do
      [ -n "$id" ] && { "$report" "$(mj_rel "$MJ_ADRS_DIR")/" "identity $id is claimed by more than one record:$sup"; MJ_ADR_FINDINGS=$((MJ_ADR_FINDINGS + 1)); rc=1; }
    done <<EOF
$dup
EOF
  fi
  # the file-name numbers are unique too, so that a listing is not ambiguous before a
  # record is even parsed
  dup="$(ls "$MJ_ADRS_DIR" 2>/dev/null | awk '/^[0-9][0-9][0-9][0-9]-/ { n = substr($0, 1, 4); c[n]++; p[n] = p[n] " " $0 } END { for (k in c) if (c[k] > 1) printf "%s\t%s\n", k, p[k] }')"
  if [ -n "$dup" ]; then
    while IFS="$MJ_TAB" read -r id sup; do
      [ -n "$id" ] && { "$report" "$(mj_rel "$MJ_ADRS_DIR")/" "file-name number $id is used by more than one file:$sup"; MJ_ADR_FINDINGS=$((MJ_ADR_FINDINGS + 1)); rc=1; }
    done <<EOF
$dup
EOF
  fi
  # relations resolve, and supersession is reciprocal
  local ids; ids=" $(awk -F"$MJ_TAB" '{ printf "%s ", $2 }' "$MJ_ADR_ROWS")"
  while IFS="$MJ_TAB" read -r f id st date title sb sup reasons; do
    sb="$(mj_adr_un "$sb")"; sup="$(mj_adr_un "$sup")"; reasons="$(mj_adr_un "$reasons")"
    if [ -n "$sb" ]; then
      case "$ids" in *" $sb "*) ;; *) "$report" "$f" "superseded_by names $sb, which is not a decision here"; MJ_ADR_FINDINGS=$((MJ_ADR_FINDINGS + 1)); rc=1 ;; esac
    fi
    for r in $(printf '%s' "$sup" | tr ',' ' '); do
      [ -n "$r" ] || continue
      case "$ids" in
        *" $r "*)
          # the record it stands in for must say so, or the chain is walkable one way only
          local back; back="$(awk -F"$MJ_TAB" -v w="$r" '$2 == w { print $6 }' "$MJ_ADR_ROWS")"
          [ "$back" = "$id" ] || { "$report" "$f" "supersedes $r, but $r does not name $id in superseded_by (found \"$back\")"; MJ_ADR_FINDINGS=$((MJ_ADR_FINDINGS + 1)); rc=1; }
          ;;
        *) "$report" "$f" "supersedes $r, which is not a decision here"; MJ_ADR_FINDINGS=$((MJ_ADR_FINDINGS + 1)); rc=1 ;;
      esac
    done
  done < "$MJ_ADR_ROWS"
  return "$rc"
}

mj_adr_report_line() { printf '%s: %s\n' "$1" "$2"; }


# ---------------------------------------------------------------- doctrine
# The validator behind majordomus.adr-integrity. It runs the same examination `adr check`
# runs; a doctrine that re-implemented the checks would be a second opinion about the same
# files, and the two would drift.
mj_adr_report_doctrine() { mj_doctrine_fail adr "$1" "$2" "majordomus adr check"; }

mj_validate_adr() {
  [ -d "$MJ_ADRS_DIR" ] || { mj_doctrine_skip adr "$(mj_rel "$MJ_ADRS_DIR")/" "no adrs section; nothing to validate"; return 0; }
  [ -f "$MJ_ALLOW_DIR/adr.txt" ] || { mj_doctrine_fail adr "$(mj_rel "$MJ_ALLOW_DIR")/adr.txt" "allow-list absent; the schema was not projected" "majordomus generate allow"; return 0; }
  local before="$MJ_FAILS"
  mj_adr_examine mj_adr_report_doctrine || true
  if [ "$MJ_ADR_N" = 0 ]; then mj_doctrine_skip adr "$(mj_rel "$MJ_ADRS_DIR")/" "no decisions; nothing to validate"
  elif [ "$MJ_FAILS" = "$before" ]; then mj_doctrine_ok adr "$MJ_ADR_N decision(s)" "every identity unique, every status known, every reference resolves"; fi
  return 0
}
