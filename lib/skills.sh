#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_skills:-}" ] && return 0 || MJ_LIB_skills=1
# skills — the repository's provider-neutral operational procedures, as data.
#
# A skill is one directory under the manifest's skills section holding SKILL.md: YAML front
# matter (the contract in share/schemas/skill.schema.json) over a Markdown body that is the
# procedure. Nothing here is a registry: which files are skills is decided by the source
# class `skill` in .ai/repo/knowledge/sources.yaml, the same declaration the Rust executable
# indexes and serves, so the two never disagree about what exists. Everything below reads
# that discovery, validates against the allow-list generated from the schema, and projects.
#
# The catalogue printed by mj_skills_catalogue is the one derivation every consumer reads:
# `skills list|show|check`, the doctor validator, and scripts/generate-site-data. A
# consumer that walked the directory itself would be a second scanner with its own order.

# shellcheck source=knowledge.sh
. "$MJ_LIB_DIR/knowledge.sh"

MJ_SKILL_SCHEMA="skill/v1"
MJ_SKILL_STATUSES="draft active deprecated"
MJ_SKILL_SECTIONS="Purpose Procedure Output"
MJ_SKILL_FILE="SKILL.md"

mj_cmd_skills() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    list) mj_skills_list "$@" ;;
    show) [ $# -ge 1 ] || mj_die "$MJ_EX_USAGE" "skills show: a skill id is required"; mj_skills_show "$@" ;;
    check) mj_skills_check "$@" ;;
    --help|-h|"") mj_skills_usage; [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "skills: unknown subcommand '$sub' (list|show|check)" ;;
  esac
}

mj_skills_usage() {
  cat <<H
usage: majordomus skills list [--json]          every skill: id, status, version, description
       majordomus skills show <id> [--json]     one skill: its path, then the file as written
       majordomus skills check [--json]         validate every skill and every reference it makes
  a skill is $(mj_rel "$MJ_SKILLS_DIR")/<id>/$MJ_SKILL_FILE: front matter (schema: $MJ_SKILL_SCHEMA, id = directory,
  version, title, description, status: $(printf '%s' "$MJ_SKILL_STATUSES" | sed 's/ /|/g'); optional tags, related, inputs, outputs)
  over a body with the sections $(printf '%s' "$MJ_SKILL_SECTIONS" | sed -e 's/ /, # /g' -e 's/^/# /'); examples live in <id>/examples/*.md
  discovery is the source class 'skill' in $(mj_rel "$MJ_KNOWLEDGE_DIR")/sources.yaml, shared with the Rust executable
H
}

# ---------------------------------------------------------------- discovery
# One row per skill file the source class discovers, in discovery order (the index order,
# which is path order): repository-relative path <TAB> sha256 <TAB> directory name.
mj_skills_files() {
  mj_knowledge_discover shared | awk -F'\t' '$1 == "skill" { n = split($5, p, "/"); printf "%s\t%s\t%s\n", $5, $4, p[n - 1] }'
}
# One row per tracked example: skill directory name <TAB> repository-relative path.
mj_skills_example_files() {
  mj_knowledge_discover shared | awk -F'\t' '$1 == "skill_example" { n = split($5, p, "/"); printf "%s\t%s\n", p[n - 2], $5 }'
}
# The example rows once per process; discovery lists the index, and a listing of ten skills
# should not list it ten times.
MJ_SKILLS_EXAMPLES_CACHE=""
mj_skills_examples_cache() {
  local tmp
  [ -n "$MJ_SKILLS_EXAMPLES_CACHE" ] && [ -f "$MJ_SKILLS_EXAMPLES_CACHE" ] && return 0
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.sx.XXXXXX")"; mj_skills_example_files > "$tmp"; MJ_SKILLS_EXAMPLES_CACHE="$tmp"
}

# ---------------------------------------------------------------- one skill
# mj_skill_load FILE — flattens the front matter into MJ_SKILL_FLAT; exit 1 with a reason
# printed when there is none or it does not parse.
MJ_SKILL_FLAT=""
mj_skill_load() {
  local f="$1" fm flat
  [ -n "$MJ_SKILL_FLAT" ] && rm -f "$MJ_SKILL_FLAT"
  fm="$(mktemp "${TMPDIR:-/tmp}/mj.sf.XXXXXX")"; flat="$(mktemp "${TMPDIR:-/tmp}/mj.sl.XXXXXX")"; MJ_SKILL_FLAT="$flat"
  if ! mj_record_front "$f" > "$fm" 2>/dev/null; then rm -f "$fm"; printf 'no front matter\n'; return 1; fi
  if ! mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then rm -f "$fm"; printf 'malformed front matter\n'; return 1; fi
  rm -f "$fm"; return 0
}
mj_skill_get()  { mj_yget "$MJ_SKILL_FLAT" "$1"; }
mj_skill_list() { mj_ylist "$MJ_SKILL_FLAT" "$1"; }

# mj_skill_validate FILE DIRNAME — every reason on its own line; exit 1 when any. Reads only
# this file: references to other skills and to examples are checked by mj_skills_check,
# which has the whole catalogue in hand.
mj_skill_validate() {
  local f="$1" dir="$2"
  mj_skill_load "$f" || return 1
  mj_skill_validate_loaded "$f" "$dir"
}
# the checks over an already loaded front matter; split from the loading so that the
# catalogue can keep the parsed fields in this shell while it collects the reasons
mj_skill_validate_loaded() {
  local f="$1" dir="$2" rc=0 v unk s h missing=""
  unk="$(mj_yaml_unknown_keys "$MJ_SKILL_FLAT" "$MJ_ALLOW_DIR/skill.txt" || true)"
  [ -n "$unk" ] && { printf 'unknown front-matter key(s): %s\n' "$(printf '%s' "$unk" | tr '\n' ' ' | sed 's/ $//')"; rc=1; }
  v="$(mj_skill_get schema)"
  [ "$v" = "$MJ_SKILL_SCHEMA" ] || { printf 'schema must be %s (found "%s")\n' "$MJ_SKILL_SCHEMA" "$v"; rc=1; }
  v="$(mj_skill_get id)"
  if [ -z "$v" ]; then printf 'id is missing\n'; rc=1
  else
    case "$v" in *[!a-z0-9-]*|[!a-z]*) printf 'id "%s" is not lower-case letters, digits and hyphens starting with a letter\n' "$v"; rc=1 ;; esac
    [ "$v" = "$dir" ] || { printf 'id "%s" does not match the directory name "%s"\n' "$v" "$dir"; rc=1; }
  fi
  v="$(mj_skill_get version)"
  case "$v" in ""|*[!0-9]*|0) printf 'version must be an integer of at least 1 (found "%s")\n' "$v"; rc=1 ;; esac
  [ -n "$(mj_skill_get title)" ] || { printf 'title is empty\n'; rc=1; }
  [ -n "$(mj_skill_get description)" ] || { printf 'description is empty\n'; rc=1; }
  v="$(mj_skill_get status)"
  case " $MJ_SKILL_STATUSES " in *" $v "*) ;; *) printf 'status must be one of %s (found "%s")\n' "$(printf '%s' "$MJ_SKILL_STATUSES" | sed 's/ /, /g')" "$v"; rc=1 ;; esac
  for s in $(mj_skill_list tags) $(mj_skill_list related); do
    case "$s" in *[!a-z0-9-]*|[!a-z]*) printf 'tag or related id "%s" is not lower-case letters, digits and hyphens\n' "$s"; rc=1 ;; esac
  done
  # the body is the procedure: the sections a reader relies on exist and are not empty
  for h in $MJ_SKILL_SECTIONS; do
    if ! mj_record_body "$f" | awk -v h="# $h" '
        $0 == h { in_s = 1; found = 1; next }
        in_s && /^# / { in_s = 0 }
        in_s && NF { body = 1 }
        END { exit (found && body) ? 0 : 1 }'; then missing="$missing $h"; fi
  done
  [ -z "$missing" ] || { printf 'body lacks a non-empty section for:%s\n' "$missing"; rc=1; }
  return $rc
}

# ---------------------------------------------------------------- the catalogue
# One row per discovered skill, in discovery order, the fields separated by the unit
# separator (\037, so that an empty field survives `read`, which collapses runs of tabs):
#   id  status  version  title  description  path  sha256  dirname  tags(,)  related(,)
#   inputs(\036)  outputs(\036)  valid(0|1)  reason(;)
# Every consumer reads this. A skill whose front matter does not parse is still a row —
# with valid=0 and its reason — so a listing never silently shrinks.
mj_skills_catalogue() {
  local path sha dir reason valid id rf
  rf="$(mktemp "${TMPDIR:-/tmp}/mj.sr.XXXXXX")"
  while IFS=$'\t' read -r path sha dir; do
    [ -n "$path" ] || continue
    valid=1
    # loaded in this shell, not in a substitution, so that the fields stay readable below
    if mj_skill_load "$MJ_ROOT/$path" > "$rf"; then mj_skill_validate_loaded "$MJ_ROOT/$path" "$dir" > "$rf" || valid=0
    else valid=0; fi
    reason="$(cat "$rf")"
    id="$(mj_skill_get id 2>/dev/null)"; [ -n "$id" ] || id="$dir"
    printf '%s\037%s\037%s\037%s\037%s\037%s\037%s\037%s\037%s\037%s\037%s\037%s\037%s\037%s\n' \
      "$id" "$(mj_skill_get status)" "$(mj_skill_get version)" "$(mj_skill_get title)" "$(mj_skill_get description)" \
      "$path" "$sha" "$dir" \
      "$(mj_skill_list tags | paste -sd, -)" "$(mj_skill_list related | paste -sd, -)" \
      "$(mj_skill_list inputs | paste -sd$'\036' -)" "$(mj_skill_list outputs | paste -sd$'\036' -)" \
      "$valid" "$(printf '%s' "$reason" | tr '\n' ';' | sed 's/;$//')"
  done < <(mj_skills_files)
  rm -f "$rf"
  [ -n "$MJ_SKILL_FLAT" ] && rm -f "$MJ_SKILL_FLAT"; MJ_SKILL_FLAT=""
  return 0
}

# ---------------------------------------------------------------- json
mj_skills_json_list() { # comma-separated -> ["a","b"]
  local sep="$1" v="$2" out="" item
  [ -n "$v" ] || { printf '[]'; return 0; }
  while IFS= read -r item; do out="$out,\"$(mj_json_esc "$item")\""; done < <(printf '%s\n' "$v" | tr "$sep" '\n')
  printf '[%s]' "${out#,}"
}
# mj_skill_json ROW [with_body] — one JSON object for one catalogue row
mj_skill_json() {
  local row="$1" body="${2:-0}" id st ver title desc path sha dir tags rel ins outs valid reason ex="" d p
  IFS=$'\037' read -r id st ver title desc path sha dir tags rel ins outs valid reason <<< "$row"
  # an invalid skill may carry a version that is not a number; the document stays valid JSON
  case "$ver" in ''|*[!0-9]*) ver="\"$(mj_json_esc "$ver")\"" ;; esac
  mj_skills_examples_cache
  while IFS=$'\t' read -r d p; do [ "$d" = "$dir" ] && ex="$ex,\"$(mj_json_esc "$p")\""; done < "$MJ_SKILLS_EXAMPLES_CACHE"
  printf '{"id":"%s","uri":"majordomus://skill/%s","title":"%s","description":"%s","status":"%s","version":%s,"tags":%s,"related":%s,"inputs":%s,"outputs":%s,"path":"%s","sha256":"%s","examples":[%s],"valid":%s' \
    "$(mj_json_esc "$id")" "$(mj_json_esc "$id")" "$(mj_json_esc "$title")" "$(mj_json_esc "$desc")" "$(mj_json_esc "$st")" \
    "$ver" "$(mj_skills_json_list , "$tags")" "$(mj_skills_json_list , "$rel")" \
    "$(mj_skills_json_list $'\036' "$ins")" "$(mj_skills_json_list $'\036' "$outs")" \
    "$(mj_json_esc "$path")" "$sha" "${ex#,}" "$([ "$valid" = 1 ] && printf true || printf false)"
  [ "$valid" = 1 ] || printf ',"reason":"%s"' "$(mj_json_esc "$reason")"
  if [ "$body" = 1 ]; then
    printf ',"body":"%s"' "$(mj_record_body "$MJ_ROOT/$path" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/	/\\t/g' | awk '{ printf "%s\\n", $0 }')"
  fi
  printf '}'
}

# ---------------------------------------------------------------- list / show
mj_skills_list() {
  [ $# = 0 ] || mj_die "$MJ_EX_USAGE" "skills list: unknown option $1"
  mj_require_installed
  mj_skills_render_list
}
# the listing itself, for the command and for the site generator, which runs where the
# layer may be partial and validates the tree by other means
mj_skills_render_list() {
  local json="${MJ_JSON:-0}" row n=0 first=1 id st ver title desc rest
  [ "$json" = 1 ] && printf '{"schema":1,"source":"%s","skills":[' "$(mj_json_esc "$(mj_rel "$MJ_KNOWLEDGE_DIR")/sources.yaml#skill")"
  while IFS= read -r row; do
    [ -n "$row" ] || continue; n=$((n + 1))
    if [ "$json" = 1 ]; then [ "$first" = 1 ] || printf ','; first=0; mj_skill_json "$row"
    else
      IFS=$'\037' read -r id st ver title desc rest <<< "$row"
      printf '%-22s %-10s v%-3s %s\n' "$id" "$st" "$ver" "$desc"
    fi
  done < <(mj_skills_catalogue)
  if [ "$json" = 1 ]; then printf '],"count":%s}\n' "$n"
  elif [ "$n" = 0 ]; then printf 'no skills under %s/ (source class skill discovered nothing)\n' "$(mj_rel "$MJ_SKILLS_DIR")"; fi
  return 0
}

mj_skills_show() {
  local id="$1" json="${MJ_JSON:-0}" row hit=""; shift
  [ $# = 0 ] || mj_die "$MJ_EX_USAGE" "skills show: unknown option $1"
  case "$id" in *[!A-Za-z0-9._-]*|""|.*) mj_die "$MJ_EX_USAGE" "skills show: '$id' is not a skill id" ;; esac
  mj_require_installed
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    [ "$(printf '%s' "$row" | cut -d$'\037' -f1)" = "$id" ] && { hit="$row"; break; }
  done < <(mj_skills_catalogue)
  [ -n "$hit" ] || mj_die "$MJ_EX_MISSING" "no skill '$id' ($(mj_rel "$MJ_SKILLS_DIR")/$id/$MJ_SKILL_FILE); try: majordomus skills list"
  local path; path="$(printf '%s' "$hit" | cut -d$'\037' -f6)"
  if [ "$json" = 1 ]; then mj_skill_json "$hit" 1; printf '\n'
  else printf '%s\n' "$path"; cat "$MJ_ROOT/$path"; fi
  return 0
}

# ---------------------------------------------------------------- check
# The one check: every skill parses and satisfies its contract, no two claim one id, every
# related id names a skill, every example is a tracked Markdown file with a heading. The
# counts are printed so that a clean result is evidence of what was examined, and a
# repository with no skills is a WARN, never a pass over nothing.
MJ_SKILLS_N=0; MJ_SKILLS_VALID=0; MJ_SKILLS_EXAMPLES=0; MJ_SKILLS_REFS=0
mj_skills_examine() {
  # $1: reporter for a violation, called as <fn> <subject> <message> <reproduce>
  local report="$1" cat n=0 valid=0 refs=0 exn=0 row id st ver title desc path sha dir tags rel ins outs ok reason r d p ids="" seen="" heading
  cat="$(mktemp "${TMPDIR:-/tmp}/mj.sc.XXXXXX")"; mj_skills_catalogue > "$cat"
  while IFS=$'\037' read -r id st ver title desc path sha dir tags rel ins outs ok reason; do
    [ -n "$id" ] || continue; n=$((n + 1)); ids="$ids $id"
    if [ "$ok" = 1 ]; then valid=$((valid + 1)); else "$report" "$path" "$reason" "majordomus skills show $id"; fi
    case " $seen " in *" $id "*) "$report" "$path" "duplicate skill id '$id'; another skill already claims it" "majordomus skills list" ;; esac
    seen="$seen $id"
  done < "$cat"
  # references: related ids resolve within the catalogue
  while IFS=$'\037' read -r id st ver title desc path sha dir tags rel ins outs ok reason; do
    [ -n "$rel" ] || continue
    for r in $(printf '%s' "$rel" | tr ',' ' '); do
      refs=$((refs + 1))
      case " $ids " in *" $r "*) ;; *) "$report" "$path" "related skill '$r' does not exist" "majordomus skills list" ;; esac
    done
  done < "$cat"
  # examples: every tracked example belongs to a skill that exists and opens with a heading
  while IFS=$'\t' read -r d p; do
    [ -n "$p" ] || continue; exn=$((exn + 1)); refs=$((refs + 1))
    case " $ids " in *" $d "*) ;; *) "$report" "$p" "example under a directory that holds no valid $MJ_SKILL_FILE" "majordomus skills list" ;; esac
    heading="$(grep -m1 -E '^# ' "$MJ_ROOT/$p" || true)"
    [ -n "$heading" ] || "$report" "$p" "example has no level-one heading to be listed by" "head -1 $p"
  done < <(mj_skills_example_files)
  rm -f "$cat"
  MJ_SKILLS_N=$n; MJ_SKILLS_VALID=$valid; MJ_SKILLS_EXAMPLES=$exn; MJ_SKILLS_REFS=$refs
  return 0
}

mj_skills_report_fail() { mj_fail skill "$1" "$2" "$3"; }
mj_skills_check() {
  [ $# = 0 ] || mj_die "$MJ_EX_USAGE" "skills check: unknown option $1"
  mj_require_installed
  [ -f "$MJ_ALLOW_DIR/skill.txt" ] || mj_die "$MJ_EX_INTERNAL" "allow-list missing: $MJ_ALLOW_DIR/skill.txt (run: majordomus generate allow)"
  MJ_FAILS=0
  mj_skills_examine mj_skills_report_fail
  if [ "$MJ_SKILLS_N" = 0 ]; then mj_warn skill "$(mj_rel "$MJ_SKILLS_DIR")/" "no skills discovered; nothing was validated" "majordomus knowledge sources --scope shared"
  elif [ "$MJ_FAILS" = 0 ]; then mj_ok skill "$MJ_SKILLS_N skill(s)" "every one parses, matches its directory and carries its sections"; fi
  [ "$MJ_SKILLS_REFS" -gt 0 ] && [ "$MJ_FAILS" = 0 ] && mj_ok skill "$MJ_SKILLS_REFS reference(s)" "every related id and every example resolves"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"summary":{"skills":%s,"valid":%s,"examples":%s,"references":%s,"failures":%s}}\n' "$MJ_SKILLS_N" "$MJ_SKILLS_VALID" "$MJ_SKILLS_EXAMPLES" "$MJ_SKILLS_REFS" "$MJ_FAILS"
  else
    printf 'skills: %s discovered, %s valid; examples: %s; references: %s checked; failures: %s\n' "$MJ_SKILLS_N" "$MJ_SKILLS_VALID" "$MJ_SKILLS_EXAMPLES" "$MJ_SKILLS_REFS" "$MJ_FAILS"
  fi
  [ "$MJ_FAILS" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}

# ---------------------------------------------------------------- doctrine validator
# Dispatched by doctor and watch through the rule majordomus.skill-integrity; the same
# examination as `skills check`, reported through the doctrine channel so that a violation
# takes the rule's class and watch labels it as drift.
mj_skills_report_doctrine() { mj_doctrine_fail skill "$1" "$2" "$3"; }
mj_validate_skills() {
  [ -d "$MJ_SKILLS_DIR" ] || return 0
  [ -f "$MJ_ALLOW_DIR/skill.txt" ] || { mj_doctrine_fail skill "$(mj_rel "$MJ_ALLOW_DIR")/skill.txt" "allow-list absent; the schema was not projected" "majordomus generate allow"; return 0; }
  local before="$MJ_FAILS"
  mj_skills_examine mj_skills_report_doctrine
  if [ "$MJ_SKILLS_N" = 0 ]; then mj_doctrine_skip skill "$(mj_rel "$MJ_SKILLS_DIR")/" "no skills; nothing to validate"
  elif [ "$MJ_FAILS" = "$before" ]; then mj_doctrine_ok skill "$MJ_SKILLS_N skill(s), $MJ_SKILLS_EXAMPLES example(s)" "every one valid; $MJ_SKILLS_REFS reference(s) resolve"; fi
  return 0
}
