#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_adr:-}" ] && return 0 || MJ_LIB_adr=1
# adr — the repository's architecture decisions, as data.
#
# An ADR is one Markdown file under the manifest's adrs section: YAML front matter (the
# contract in share/schemas/majordomus/adr/adr.v1.schema.json) over a body carrying Context, Decision and
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
#   an identity is allocated above the high-water mark and never reused. Two worktrees
#   proposing at the same moment is the ordinary case here, and it is exactly how this
#   repository ended up with two 0005s and two 0007s before anything checked, and with three
#   branches on 0044 and two on 0043 a year of commits later. `adr next` is that survey made
#   askable — every ref, every tag, every sibling worktree and the peer board, each saying
#   what it claimed — and `adr check` refuses, on the branch, an identity this tree adds that
#   another ref already carries a different decision at (ADR 0053).

# shellcheck source=knowledge.sh
. "$MJ_LIB_DIR/knowledge.sh"
# shellcheck source=rules.sh
. "$MJ_LIB_DIR/rules.sh"

MJ_ADR_SCHEMA="adr/v1"
MJ_ADR_STATUSES="proposed accepted superseded rejected"
MJ_ADR_ORIGINS="authored extracted"
MJ_ADR_SECTIONS="Context Decision Consequences"
# The reference vocabulary, shared by the decision and knowledge records: a
# reference is <type>:<value>, and a file: or test: reference must resolve.
MJ_ADR_REF_TYPES="decision session commit issue file test"
# what a decision put in force, as opposed to where it came from: a rule of the effective
# set, a claim of the matrix, a document or implementation, a behavioural case. The graph
# turns each into an edge, so nothing writes the reverse direction down.
MJ_ADR_REL_TYPES="rule claim file test"

mj_cmd_adr() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    list) mj_adr_list "$@" ;;
    show) [ $# -ge 1 ] || mj_die "$MJ_EX_USAGE" "adr show: an adr id is required"; mj_adr_show "$@" ;;
    next) mj_adr_next "$@" ;;
    propose) mj_adr_propose "$@" ;;
    check) mj_adr_check "$@" ;;
    affected) mj_adr_affected "$@" ;;
    --help|-h|"") mj_adr_usage; [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "adr: unknown subcommand '$sub' (list|show|next|propose|check|affected)" ;;
  esac
}

mj_adr_usage() {
  cat <<H
usage: majordomus adr list [--status <status>] [--json]      every decision: id, status, date, title
       majordomus adr show <id> [--json]                     one decision: its path, then the file as written
       majordomus adr next [--json]                          the next free identity, and every source that was surveyed
       majordomus adr propose "<title>" [--from <ref>]...    write a new decision with status: proposed
                              [--tag <tag>]... [--supersedes <id>]
       majordomus adr check [--json]                         validate every decision and every reference it makes
       majordomus adr affected [--base <ref>|--staged]        the decisions a change set touches, from what they name
                               [--worktree] [--json]
  a decision is $(mj_rel "$MJ_ADRS_DIR")/<NNNN>-<slug>.md: front matter (schema: $MJ_ADR_SCHEMA, id: adr-NNNN,
  kind, title, status: $(printf '%s' "$MJ_ADR_STATUSES" | sed 's/ /|/g'), date; optional tags, supersedes, superseded_by, provenance)
  over a body with the sections $(printf '%s' "$MJ_ADR_SECTIONS" | sed -e 's/ /, # /g' -e 's/^/# /')
  propose writes 'proposed' and refuses to write any other status: accepting a decision is a person's act
  an identity is allocated above the high-water mark and never recycled: 'next' says which source set it
  check reads the other refs too and refuses an identity this branch adds that another ref already carries
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
# whether MJ_ADR_FLAT points into the prefetched cache, which the next load must not unlink
MJ_ADR_FLAT_CACHED=0
MJ_ADR_CACHE=""
# mj_adr_prefetch <list> — every decision named in <list> flattened in one process, so
# that mj_adr_load below is a lookup. The walkers call this; `adr validate <file>`
# deliberately does not, because reading one record must not cost the flatten of fifty.
#
# It takes the walker's own list rather than calling mj_adr_files: discovery is not
# cached, so a prefetch that discovered for itself would pay a second git walk and a
# second hash of the whole corpus — measured at more than the fifty awk processes it
# saves. A batch that has to re-derive its input is not a batch, it is a second loop.
mj_adr_prefetch() {
  # the list is copied out of $1 first: `set --` below clears the positional parameters,
  # and the redirection at the end of the loop is expanded after it has
  local f lst="${1:-}"
  [ -n "$MJ_ADR_CACHE" ] && return 0
  [ -n "$lst" ] && [ -f "$lst" ] || return 0
  MJ_ADR_CACHE="$(mktemp -d "${TMPDIR:-/tmp}/mj.adrc.XXXXXX")"
  set --
  while IFS="$MJ_TAB" read -r f _; do
    [ -n "$f" ] && set -- "$@" "$MJ_ROOT/$f"
  done < "$lst"
  # a cache the batch could not build is no cache: every load flattens for itself, which
  # is what this repository did before, so the failure is slower and never wrong
  mj_front_cache_build "$MJ_ADR_CACHE" "$@" || return 0
}
mj_adr_load() {
  local f="$1" fm flat hit=0
  MJ_ADR_ERROR=""
  [ -n "$MJ_ADR_FLAT" ] && [ "$MJ_ADR_FLAT_CACHED" = 0 ] && rm -f "$MJ_ADR_FLAT"
  MJ_ADR_FLAT_CACHED=0
  mj_front_cache_get "$f" || hit=$?
  if [ "$hit" != 2 ]; then
    # the cache's own output file, empty exactly where the per-record path left an empty
    # one, so that every reader below sees what it always saw
    MJ_ADR_FLAT="$MJ_FRONT_FLAT"; MJ_ADR_FLAT_CACHED=1
    [ "$hit" = 0 ] && return 0
    MJ_ADR_ERROR="$MJ_FRONT_ERROR"; return 1
  fi
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

# mj_adr_rel_valid REF — a forward reference: the type is one this record may state, and
# the target exists where that type says it lives. A rule is looked up in the effective set
# and a claim in the matrix, because a decision that names something nothing declares is a
# dangling end the reader cannot follow.
mj_adr_rel_valid() {
  local ref="$1" t="${1%%:*}" v="${1#*:}"
  case "$ref" in *:*) ;; *) printf 'related "%s" is not <type>:<value>\n' "$ref"; return 1 ;; esac
  case " $MJ_ADR_REL_TYPES " in
    *" $t "*) ;;
    *) printf 'related "%s" has an unknown type "%s" (one of %s)\n' "$ref" "$t" "$(printf '%s' "$MJ_ADR_REL_TYPES" | sed 's/ /, /g')"; return 1 ;;
  esac
  [ -n "$v" ] || { printf 'related "%s" has an empty value\n' "$ref"; return 1; }
  case "$t" in
    file|test) [ -e "$MJ_ROOT/$v" ] || { printf 'related "%s" names a path that does not exist\n' "$ref"; return 1; } ;;
    rule)
      if mj_rules_load; then
        mj_rule_index "$v" >/dev/null || { printf 'related "%s" names a rule the effective set does not have\n' "$ref"; return 1; }
      else printf 'related "%s" cannot be checked: the rules do not resolve (%s)\n' "$ref" "$MJ_RULES_ERROR"; return 1; fi ;;
    claim)
      if [ -f "$MJ_ROOT/docs/CLAIMS.yaml" ]; then
        grep -q "^  - id: $v\$" "$MJ_ROOT/docs/CLAIMS.yaml" || { printf 'related "%s" names a claim docs/CLAIMS.yaml does not have\n' "$ref"; return 1; }
      fi ;;
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
  # what the decision put in force. Optional: a decision may be taken before anything
  # implements it, and a record that names nothing is not thereby wrong.
  for v in $(mj_adr_lst related); do mj_adr_rel_valid "$v" || rc=1; done
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
  local tmp out lst f base reasons id st date title sb sup
  [ -n "$MJ_ADR_ROWS" ] && [ -f "$MJ_ADR_ROWS" ] && return 0
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ac.XXXXXX")"
  out="$(mktemp "${TMPDIR:-/tmp}/mj.ao.XXXXXX")"; MJ_ADR_ROWS="$out"
  MJ_ADR_N=0; MJ_ADR_INVALID=0
  # discovery once, read twice: the prefetch below and the loop under it are the same walk
  lst="$(mktemp "${TMPDIR:-/tmp}/mj.adrl.XXXXXX")"; mj_adr_files > "$lst"
  mj_adr_prefetch "$lst"
  # The effective rule set, once, here. mj_adr_rel_valid resolves a `related: rule:...`
  # through mj_rules_load, and it is reached from the validator below through a command
  # substitution — a subshell, which takes MJ_RULES_LOADED away with it when it exits. So
  # every record that names a rule reloaded the whole set: `adr list` on this repository
  # ran mj_rule_scan 3350 times, twenty-five loads of a hundred and thirty-four rules,
  # where one load is enough. Loading in this shell leaves the cache where the subshells
  # can see it. A failure is not handled here: the loader records its reason in
  # MJ_RULES_ERROR without marking itself loaded, so the per-record path below retries and
  # reports exactly what it reported before.
  mj_rules_load || true
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
  done < "$lst"
  LC_ALL=C sort -t"$MJ_TAB" -k2,2 "$tmp" > "$out"
  rm -f "$tmp" "$lst"
  return 0
}

# ---------------------------------------------------------------- affected
# What a change set touches, read from what the decisions themselves name. A record is
# affected when its own file changed, or when a path it names in `related` changed — the
# forward edge doing the work in the direction a reader needs: this file has a decision
# behind it, go and read whether it still holds.
#
# Every item is a review note, never a failure. Whether a decision still stands after the
# code it governs moved is exactly the judgement a tool may not make, and a command that
# exited non-zero here would be asserting it had.
mj_adr_affected() {
  local mode=worktree base="" changed tmp lst f id title rel n=0 first=1
  while [ $# -gt 0 ]; do case "$1" in
    --base) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--base needs a ref"; mode=base; base="$2"; shift 2 ;;
    --base=*) mode=base; base="${1#--base=}"; shift ;;
    --staged) mode=staged; shift ;;
    --worktree) mode=worktree; shift ;;
    --json) MJ_JSON=1; shift ;;
    *) mj_die "$MJ_EX_USAGE" "adr affected: unknown option $1" ;;
  esac; done
  mj_require_installed
  [ "$mode" != base ] || mj_git rev-parse --verify --quiet "$base^{commit}" >/dev/null 2>&1 \
    || mj_die "$MJ_EX_USAGE" "adr affected: --base '$base' is not a commit in this repository"
  changed="$(mj_change_set "$mode" "$base" | awk -F"$MJ_TAB" '{ print $2; if ($3 != "") print $3 }' | LC_ALL=C sort -u)"
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.adraff.XXXXXX")"
  lst="$(mktemp "${TMPDIR:-/tmp}/mj.adral.XXXXXX")"; mj_adr_files > "$lst"
  mj_adr_prefetch "$lst"
  mj_rules_load || true    # once in this shell, for the reason mj_adr_catalogue gives
  while IFS="$MJ_TAB" read -r f _; do
    [ -n "$f" ] || continue
    mj_adr_load "$MJ_ROOT/$f" || continue
    id="$(mj_adr_get id)"; title="$(mj_adr_get title)"
    # the record itself
    printf '%s\n' "$changed" | grep -Fxq "$f" && printf '%s\t%s\t%s\t%s\n' "${id:--}" "$f" "record" "$f" >> "$tmp"
    # and every path it says it put in force, a directory reference covering what is below it
    for rel in $(mj_adr_lst related); do
      case "$rel" in file:*|test:*) ;; *) continue ;; esac
      rel="${rel#*:}"
      printf '%s\n' "$changed" | awk -v r="$rel" '$0 == r || index($0, r "/") == 1 { print; found = 1 } END { exit found ? 0 : 1 }' \
        | while IFS= read -r hit; do printf '%s\t%s\t%s\t%s\n' "${id:--}" "$f" "related" "$hit" >> "$tmp"; done
    done
  done < "$lst"
  rm -f "$lst"
  n="$(awk -F"$MJ_TAB" '{ print $1 }' "$tmp" 2>/dev/null | LC_ALL=C sort -u | grep -c . || true)"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"mode":"%s","base":%s,"affected":[' "$mode" "$([ -n "$base" ] && printf '"%s"' "$(mj_json_esc "$base")" || printf null)"
    while IFS="$MJ_TAB" read -r id f why hit; do
      [ -n "$id" ] || continue
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"adr":"%s","path":"%s","reason":"%s","changed":"%s"}' "$id" "$(mj_json_esc "$f")" "$why" "$(mj_json_esc "$hit")"
    done < <(LC_ALL=C sort -u "$tmp")
    printf '],"count":%s}\n' "$n"
  else
    if [ ! -s "$tmp" ]; then printf 'adr affected: no decision names anything this change set touches\n'
    else
      while IFS="$MJ_TAB" read -r id f why hit; do
        [ -n "$id" ] || continue
        case "$why" in
          record) printf 'WARN adr %-10s the record itself changed (%s)\n' "$id" "$f" ;;
          *)      printf 'WARN adr %-10s names %s, which this change set touches — read whether the decision still holds (%s)\n' "$id" "$hit" "$f" ;;
        esac
      done < <(LC_ALL=C sort -u "$tmp")
      printf 'adr affected: %s decision(s) to read\n' "$n"
    fi
  fi
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

mj_adr_numbers_here() {
  local n num
  for n in "$1"/[0-9][0-9][0-9][0-9]-*.md; do
    [ -e "$n" ] || continue
    num="${n##*/}"; printf '%s\n' "${num%%-*}"
  done
}

# ---------------------------------------------------------------- the survey
# Every claim on an identity this repository can see, from four places rather than one.
#
# One directory is not enough, and this is not theoretical: a single session here proposed
# 0028, found it held by a peer's uncommitted work, took 0029, and found that claimed by
# master while the branch was being written. The propose lock is exclusive over *this*
# worktree; it says nothing about the thirty others, and nothing about a branch. On the
# night of 2026-09-11 three branches claimed 0044 and two claimed 0043, and three separate
# sessions each hand-rolled this same survey to find their way out, independently and
# correctly, because nothing offered it.
#
#   the working tree      what a person can see, and what the first implementation read
#   the sibling worktrees the case the refs cannot answer. Work that is authored but not
#                         yet committed exists only on a filesystem, and with several
#                         sessions in one repository that is exactly where the next number
#                         is already taken.
#   every git ref         every number ever added under the decisions directory on any
#                         branch, tag or remote-tracking ref, merged or not. One process. A
#                         number freed by a rename stays spent, which is the conservative
#                         direction: allocation is monotonic and two workers never meet.
#   the peer board        the case neither git nor a filesystem can answer: an identity a
#                         session has decided to take and has not written anywhere yet. It is
#                         how 0034 was reserved for feature/peer-board-survives-restart while
#                         existing as no file on any branch and in no working tree.
#
# mj_adr_claims — one row per claim on an identity, in no particular order:
#
#   NNNN <TAB> <source> <TAB> <where>
#
# `source` is one of tree, worktree, ref, peer. Nothing is de-duplicated: two sources
# claiming one identity is the interesting fact, and `adr next` reports the population each
# source contributed, because a verdict that says "0053" without saying what it looked at is
# the hand-rolled survey this command exists to replace.
mj_adr_claims() {
  local rel worktree num
  rel="${MJ_ADRS_DIR#$MJ_ROOT/}"
  # this working tree: what a person can see
  mj_adr_numbers_here "$MJ_ADRS_DIR" \
    | while IFS= read -r num; do printf '%s\ttree\t%s\n' "$num" "$(mj_rel "$MJ_ADRS_DIR")"; done
  # every other worktree of this repository, as it stands on disk right now: authored and
  # not yet committed is a real claim, and with several sessions in one repository it is
  # exactly where the next number is already taken
  git -C "$MJ_ROOT" worktree list --porcelain 2>/dev/null \
    | sed -n 's/^worktree //p' \
    | while IFS= read -r worktree; do
        [ "$worktree" = "$MJ_REPO" ] && continue
        [ -d "$worktree/$rel" ] || continue
        mj_adr_numbers_here "$worktree/$rel" \
          | while IFS= read -r num; do printf '%s\tworktree\t%s\n' "$num" "$worktree"; done
      done
  # every identity ever added under the decisions directory on any ref. `--all` is every
  # branch, every tag, every remote-tracking ref and — unless --single-worktree is given —
  # the HEAD of every linked worktree, which is one process rather than four hundred.
  # `--no-renames` is load-bearing: rename detection is on by default, and a decision whose
  # slug was edited then arrives as an R and not an A. The survey measured forty identities
  # on the refs of a tree holding forty-eight until this flag was added.
  git -C "$MJ_ROOT" log --all --no-renames --pretty=format: --name-only --diff-filter=A -- "$rel" 2>/dev/null \
    | sed -n 's|.*/\([0-9][0-9][0-9][0-9]\)-.*\.md$|\1|p' \
    | while IFS= read -r num; do printf '%s\tref\tgit log --all --no-renames --diff-filter=A\n' "$num"; done
  # the board, when a server is there to answer
  mj_adr_board_claims
}

# The identities the attached peers have announced, from what they announced: the scope
# paths they named and the words of the intent. Both are read, because a session announcing
# an allocation writes it in whichever of the two it thinks of — tonight's announcements
# carried it in both — and a survey that read only one of them would miss the reservation.
#
# Silent when there is no board. The network call itself is `mj_peer_board` in lib/context.sh:
# SECURITY.md, project.no-network-no-eval and test/cases/08_no_forbidden_constructs.sh all
# name that file as the single declared exception, so this is a second caller and not a
# second call site.
MJ_ADR_BOARD_READ=0
mj_adr_board_claims() {
  MJ_ADR_BOARD_READ=0
  # shellcheck source=context.sh
  . "$MJ_LIB_DIR/context.sh"
  local board rows pid name text num
  board="$(mj_peer_board)" || return 0
  MJ_ADR_BOARD_READ=1
  rows="$(printf '%s' "$board" | jq -r '
    .peers[] as $p
    | ( $p.claims // (if $p.announcement then [$p.announcement] else [] end) )[]?
    | [ $p.id, (.name // "-"), (((.scope // []) | join(" ")) + " " + (.intent // "")) ] | @tsv
  ' 2>/dev/null)" || return 0
  while IFS="$MJ_TAB" read -r pid name text; do
    [ -n "$pid" ] || continue
    # every token, with punctuation turned into space: "adr-0052", "ADR 0052" and
    # ".ai/repo/adrs/0052-x.md" all become the word `adr` or `adrs` followed by the number
    printf '%s\n' "$text" | tr 'A-Z' 'a-z' | sed 's/[^a-z0-9]\{1,\}/ /g' \
      | awk '{ for (i = 1; i < NF; i++) if (($i == "adr" || $i == "adrs") && $(i+1) ~ /^[0-9][0-9][0-9][0-9]$/) print $(i+1) }' \
      | LC_ALL=C sort -u \
      | while IFS= read -r num; do printf '%s\tpeer\t%s (%s)\n' "$num" "$pid" "$name"; done
  done <<EOF
$rows
EOF
  return 0
}

# The next identity: one above the highest anything has ever claimed.
#
# Monotonic, and that is the whole answer to the number that is cited and written nowhere.
# 0034 is on no branch, in no working tree and in no tag; the sequence has a hole there and
# the hole is not free, because master's ADR 0035 and docs/ENTRY_AUDIT.md already cite it and
# a board reservation stands behind it. A gap means an identity was taken and withdrawn, or
# taken and not yet written — both are spent, and no survey can distinguish them from a
# number that was never used at all. So this never recycles: `max + 1`, every time. It costs
# a sparse sequence, which costs nothing, and it removes the only case where two correct
# surveys of the same evidence can disagree.
mj_adr_next_id() {
  mj_adr_claims | awk -F"$MJ_TAB" '
    { n = $1 + 0; if (n > max) max = n }
    END { printf "%04d\n", max + 1 }'
}

# Which authored files cite an identity nothing writes. Only ever asked about the gaps, so
# it is one `grep` over the tracked tree and not a per-identity scan; it is evidence for the
# reader, never the mechanism — a gap is spent whether or not anything is found to cite it.
mj_adr_citations() {
  local num="$1" n
  n="$(printf '%s' "$num" | sed 's/^0*//')"; [ -n "$n" ] || n=0
  git -C "$MJ_ROOT" grep -l -I -E "(adr-$num|ADR $num|ADR $n\b)" -- \
    ':(exclude).ai/local' ':(exclude)'"${MJ_ADRS_DIR#$MJ_ROOT/}/$num"'-*' 2>/dev/null \
    | LC_ALL=C sort | head -6
}

# ---------------------------------------------------------------- next
# The question three sessions answered by hand on one night: what is the next free identity?
#
# It answers with the number *and* with what it looked at, because the reason each of those
# sessions had to write its own was that no answer here carried its denominator. A reader who
# cannot see which sources were reachable cannot tell a survey of four sources from a survey
# of three, and the one that is routinely missing — the board — is the only source that knows
# about an identity a session has taken and not yet written.
mj_adr_next() {
  while [ $# -gt 0 ]; do case "$1" in
    --json) MJ_JSON=1; shift ;;
    *) mj_die "$MJ_EX_USAGE" "adr next: unknown option $1" ;;
  esac; done
  mj_require_installed
  [ -d "$MJ_ADRS_DIR" ] || mj_die "$MJ_EX_MISSING" "no $(mj_rel "$MJ_ADRS_DIR")/ in this repository (the manifest names it)"

  local rows next max
  rows="$(mktemp "${TMPDIR:-/tmp}/mj.an.XXXXXX")"
  mj_adr_claims > "$rows"
  next="$(awk -F"$MJ_TAB" '{ n = $1 + 0; if (n > max) max = n } END { printf "%04d\n", max + 1 }' "$rows")"
  max="$(awk -F"$MJ_TAB" '{ n = $1 + 0; if (n > max) max = n } END { printf "%04d\n", max }' "$rows")"

  # per source: how many distinct identities it claimed, and whether it could be read at all
  local counted; counted="$(awk -F"$MJ_TAB" '
    !seen[$2 FS $1]++ { n[$2]++ }
    END { split("tree worktree ref peer", o, " "); for (i = 1; i <= 4; i++) printf "%s\t%d\n", o[i], n[o[i]] + 0 }' "$rows")"
  local holder; holder="$(awk -F"$MJ_TAB" -v m="$max" '($1 + 0) == (m + 0) { printf "%s (%s)\n", $2, $3; exit }' "$rows")"
  # the holes below the high-water mark, which are spent and not free
  local gaps; gaps="$(awk -F"$MJ_TAB" '{ n = $1 + 0; seen[n] = 1; if (n > max) max = n }
    END { for (i = 1; i < max; i++) if (!(i in seen)) printf "%04d\n", i }' "$rows")"

  if [ "$MJ_JSON" = 1 ]; then
    local first=1 g src cnt
    printf '{"schema":1,"next":"%s","highest_claimed":"%s","sources":[' "$next" "$max"
    while IFS="$MJ_TAB" read -r src cnt; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"source":"%s","identities":%s,"reachable":%s}' "$src" "$cnt" \
        "$([ "$src" = peer ] && { [ "$MJ_ADR_BOARD_READ" = 1 ] && printf true || printf false; } || printf true)"
    done <<EOF
$counted
EOF
    printf '],"spent_gaps":['; first=1
    for g in $gaps; do [ "$first" = 1 ] || printf ','; first=0; printf '"%s"' "$g"; done
    printf ']}\n'
  else
    printf 'next free identity: adr-%s\n\n' "$next"
    printf 'surveyed, and what each claimed:\n'
    local src cnt label
    while IFS="$MJ_TAB" read -r src cnt; do
      case "$src" in
        tree)     label="this working tree" ;;
        worktree) label="every sibling worktree, as it stands on disk" ;;
        ref)      label="every ref: branches, tags, remotes, worktree HEADs" ;;
        peer)     label="the peer board of the shared server" ;;
      esac
      if [ "$src" = peer ] && [ "$MJ_ADR_BOARD_READ" != 1 ]; then
        printf '  %-52s not reachable — no server, or no jq/curl\n' "$label"
      else
        printf '  %-52s %s identit%s\n' "$label" "$cnt" "$([ "$cnt" = 1 ] && printf y || printf ies)"
      fi
    done <<EOF
$counted
EOF
    printf '\nhighest claimed: adr-%s' "$max"
    [ -n "$holder" ] && printf '  (%s)' "$holder"
    printf '\n'
    # The shell tool has no peer identity of its own — it reads the board over HTTP and the
    # board answers sessions, not commands — so it cannot subtract the reader from the list.
    # A session that announced an allocation and then asks is therefore stepped over its own
    # claim, which costs one number out of a sparse sequence and is the safe direction.
    case "$holder" in peer*) printf 'that is a board claim, not a file — it may be your own announcement\n' ;; esac
    if [ -n "$gaps" ]; then
      local g c
      printf '\nholes below it, spent and not free — allocation is monotonic and never recycles:\n'
      for g in $gaps; do
        c="$(mj_adr_citations "$g" | tr '\n' ' ')"
        if [ -n "$c" ]; then printf '  %s  cited by: %s\n' "$g" "$c"
        else printf '  %s  nothing cites it here; it was taken and withdrawn, or taken and not yet written\n' "$g"; fi
      done
    fi
    printf '\nannounce it before you write it: majordomus_announce, naming the identity as well as the paths\n'
  fi
  rm -f "$rows"
  return 0
}

# ---------------------------------------------------------------- propose
# The identity is allocated under an exclusive lock over the decisions directory: mkdir is
# the one create-or-fail primitive every POSIX filesystem gives us, so two worktrees
# proposing in the same second get two identities rather than one. The file is written to a
# temporary beside its destination and moved into place, so a reader never sees half a
# record and an interrupted propose leaves nothing.

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
    printf '],"refs":{"surveyed":%s' "$([ -n "$MJ_ADR_REFS_SKIP" ] && printf false || printf true)"
    [ -n "$MJ_ADR_REFS_SKIP" ] && printf ',"reason":"%s"' "$(mj_json_esc "$MJ_ADR_REFS_SKIP")"
    printf ',"added_here":%s}' "$MJ_ADR_REFS_N"
    printf ',"ok":%s}\n' "$([ "$rc" = 0 ] && printf true || printf false)"
  else
    if [ "$rc" = 0 ]; then
      printf 'examined %s decision(s) in %s/\n' "$MJ_ADR_N" "$(mj_rel "$MJ_ADRS_DIR")"
      printf 'every identity unique, every status known, every reference resolves\n'
    else
      cat "$out"
      printf '\n%s decision(s) examined, %s with findings\n' "$MJ_ADR_N" "$MJ_ADR_FINDINGS"
    fi
    # Always stated, clean or not: a survey that could not reach the other refs has not
    # cleared this branch of a collision, and a reader who cannot tell the two apart is
    # given a confident wrong answer (project.empty-is-not-failure).
    if [ -n "$MJ_ADR_REFS_SKIP" ]; then
      printf 'the other refs were NOT surveyed: %s\n' "$MJ_ADR_REFS_SKIP"
    else
      printf '%s identit%s added here, measured against %s and every other ref\n' \
        "$MJ_ADR_REFS_N" "$([ "$MJ_ADR_REFS_N" = 1 ] && printf y || printf ies)" "$(mj_adr_base_ref)"
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
  # and the question this tree cannot answer about itself: does another ref already hold a
  # different decision at an identity this one adds? The doctrine and `adr check` share it
  # for the same reason they share everything above — a second opinion about the same refs
  # would drift from this one.
  mj_adr_collisions "$report" || rc=1
  return "$rc"
}

mj_adr_report_line() { printf '%s: %s\n' "$1" "$2"; }

# ------------------------------------------------------------- across the refs
# The half of the examination that is not about this tree.
#
# `mj_adr_examine` above answers "are the decisions in front of me consistent with each
# other". It cannot see the collision that actually costs this repository anything, because
# that collision is between *two trees*: two branches, each internally perfect, each holding
# a different decision at the same identity. `majordomus generate --strict` then excludes
# every claimant of that identity and refuses the whole tree, and the error names the
# identity rather than the two sessions — so it surfaces at merge time, hours away from
# either cause. On the night of 2026-09-11 three branches held 0044 and two held 0043.
#
# So this asks the question at commit time instead, on the branch, where the answer is still
# cheap to act on: for every identity this tree adds relative to the base, does another ref
# carry a *different* document there?
#
# Different means a different file name. Two refs holding the same path is an edit, and an
# edit is resolved by merging; two refs holding 0044-cooperation-is... and 0044-the-model-
# catalogue-is... is a choice between two finished pieces of work, and one of them was
# written for nothing. Only the second is refused.
#
# Cost, because this runs inside `doctor` and `doctor` runs in the pre-commit hook: one
# `ls-tree` of the base (~10ms) and nothing else, unless this tree actually adds an
# identity. When it does, one `git log` over the decisions directory across every ref —
# measured at 0.22s over 436 refs and the whole history — and one `for-each-ref --contains`
# per colliding identity, of which there are normally none.
MJ_ADR_REFS_SKIP=""
MJ_ADR_REFS_N=0

# The ref this tree's additions are measured against: what a merge will land on.
# MJ_ADR_BASE, when set, is the base and nothing else is tried — an override that silently
# fell back to a default would be an override that cannot be tested and cannot be trusted.
mj_adr_base_ref() {
  local b
  if [ -n "${MJ_ADR_BASE:-}" ]; then
    git -C "$MJ_ROOT" rev-parse --verify -q "$MJ_ADR_BASE^{commit}" >/dev/null 2>&1 \
      && { printf '%s\n' "$MJ_ADR_BASE"; return 0; }
    return 1
  fi
  for b in origin/master origin/main master main; do
    git -C "$MJ_ROOT" rev-parse --verify -q "$b^{commit}" >/dev/null 2>&1 && { printf '%s\n' "$b"; return 0; }
  done
  return 1
}

# mj_adr_collisions REPORTER — calls REPORTER <path> <reason> for each colliding identity.
# Sets MJ_ADR_REFS_SKIP when it could not look, which is never silent: a check that cannot
# reach its subject says so rather than exiting clean (project.empty-is-not-failure).
mj_adr_collisions() {
  local report="$1" base rel adds here num path commit refs rc=0
  MJ_ADR_REFS_SKIP=""; MJ_ADR_REFS_N=0
  mj_has git || { MJ_ADR_REFS_SKIP="git is not on PATH"; return 0; }
  git -C "$MJ_ROOT" rev-parse --git-dir >/dev/null 2>&1 \
    || { MJ_ADR_REFS_SKIP="$(mj_rel "$MJ_ROOT") is not a git repository"; return 0; }
  base="$(mj_adr_base_ref)" \
    || { MJ_ADR_REFS_SKIP="no base ref resolves here (${MJ_ADR_BASE:-origin/master, origin/main, master, main}); fetch first"; return 0; }
  rel="${MJ_ADRS_DIR#$MJ_ROOT/}"

  # what this tree adds relative to the base, by identity. An identity the base already
  # carries is not this branch's to claim and not this branch's problem.
  local basenums; basenums=" $(git -C "$MJ_ROOT" ls-tree --name-only "$base" -- "$rel/" 2>/dev/null \
    | sed -n 's|.*/\([0-9][0-9][0-9][0-9]\)-.*\.md$|\1|p' | tr '\n' ' ')"
  adds=""
  for here in "$MJ_ADRS_DIR"/[0-9][0-9][0-9][0-9]-*.md; do
    [ -e "$here" ] || continue
    num="${here##*/}"; num="${num%%-*}"
    case "$basenums" in *" $num "*) continue ;; esac
    adds="$adds $num"
  done
  MJ_ADR_REFS_N="$(printf '%s' "$adds" | wc -w | tr -d ' ')"
  [ -n "$adds" ] || return 0

  # every (path, commit) that ever added a decision on any ref, once
  local seen; seen="$(mktemp "${TMPDIR:-/tmp}/mj.ax.XXXXXX")"
  git -C "$MJ_ROOT" log --all --no-renames --diff-filter=A --name-only --pretty=tformat:'commit %H' -- "$rel" 2>/dev/null \
    | awk -v tab="$MJ_TAB" '/^commit /{ c = $2; next } /\.md$/ { if (c != "") printf "%s%s%s\n", $0, tab, c }' \
    > "$seen"

  local paths
  for num in $adds; do
    here="$(ls "$MJ_ADRS_DIR/$num"-*.md 2>/dev/null | head -1)"
    [ -n "$here" ] || continue
    here="$(mj_rel "$here")"
    # one finding per competing *document*, not per commit that introduced it: a decision
    # cherry-picked onto four branches has four add-commits and is still one collision
    paths="$(awk -F"$MJ_TAB" -v p="$rel/$num-" -v h="$here" \
      'index($1, p) == 1 && $1 != h { print $1 }' "$seen" | LC_ALL=C sort -u)"
    for path in $paths; do
      refs="$(awk -F"$MJ_TAB" -v w="$path" '$1 == w { print $2 }' "$seen" \
        | while IFS= read -r commit; do
            git -C "$MJ_ROOT" for-each-ref --contains "$commit" \
              --format='%(refname:short)' refs/heads refs/remotes/origin refs/tags 2>/dev/null
          done | LC_ALL=C sort -u | head -6 | tr '\n' ' ')"
      [ -n "$refs" ] || refs="no current ref — an unreferenced commit still in this object store"
      "$report" "$here" "identity $num is also claimed by $path, on: ${refs% }. One of the two has to be renumbered before either lands, or \`generate --strict\` will exclude both and name only the identity. \`majordomus adr next\` says which number is free."
      MJ_ADR_FINDINGS=$((MJ_ADR_FINDINGS + 1)); rc=1
    done
  done
  rm -f "$seen"
  return "$rc"
}


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
  # The cross-ref half reports its own reachability, so that a commit is never cleared of a
  # collision by a survey that could not run. This is the line that makes
  # project.work-is-claimed-before-it-is-built followable: the claim it asks for is checked
  # here, at commit time, on the branch, rather than at the merge where `generate --strict`
  # would name the identity and neither of the two sessions.
  if [ -n "$MJ_ADR_REFS_SKIP" ]; then
    mj_doctrine_skip adr "$(mj_rel "$MJ_ADRS_DIR")/" "the other refs were not surveyed: $MJ_ADR_REFS_SKIP"
  elif [ "$MJ_ADR_REFS_N" != 0 ] && [ "$MJ_FAILS" = "$before" ]; then
    mj_doctrine_ok adr "$MJ_ADR_REFS_N identity(ies) added here" "no other ref claims them"
  fi
  return 0
}
