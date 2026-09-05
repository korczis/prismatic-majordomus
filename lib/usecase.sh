#!/usr/bin/env bash
# usecase — the executable use cases of this repository: the canonical objects under the
# manifest's `use-cases` section, listed, shown, validated, run against the real tool with
# the evidence recorded, tallied against every public command, guaranteed claim and MCP
# tool, traced from a change to what it affects, and scaffolded where coverage is missing.
#
# A use case is a Markdown file with front matter (share/schemas/use-case.schema.json): a
# task a person performs, the commands, rules, claims, responsibilities and applications
# it names, and a scenario: a fresh repository prepared by a setup script, then real
# invocations of bin/majordomus with their expected exit codes and output. Nothing here is
# prose about the tool; a reference that does not resolve or a step that does not behave
# is a failure with the file and the step named.
#
# Evidence is written under the local half (.ai/local/evidence/use-cases/<id>.json), never
# under the tracked tree; the site generator runs the scenarios itself and embeds the
# normalised output, so a page shows what the tool did, and `--check` proves it again.

MJ_UC_DIR=""; MJ_AP_DIR=""; MJ_UC_IDS=""; MJ_UC_N=0; MJ_UC_LOADED=0
MJ_UC_EVIDENCE=""
# the command registry, for the public commands coverage is counted over
# shellcheck source=commands.sh
. "$MJ_LIB_DIR/commands.sh"

mj_uc_usage() {
  cat <<'USAGE'
usage: majordomus usecase list [--json]
       majordomus usecase show <id>
       majordomus usecase validate [--json]
       majordomus usecase run [<id>...] [--json] [--out <dir>] [--keep]
       majordomus usecase coverage [--json] [--check]
       majordomus usecase impact [--base <ref>] [--json]
       majordomus usecase scaffold [--missing] [--for command:<name>] [--dry-run]
  list      every use case: id, category, status, the commands it runs, whether it has a scenario
  show      one use case: its front matter and body, and the evidence of its last run when there is one
  validate  every reference resolves (commands, doctrines, claims, responsibilities, applications,
            categories, setup scripts, stdin files), ids are unique and match their file, the body
            carries its sections; exit 10 on any failure
  run       execute the scenarios against bin/majordomus in disposable repositories, assert every
            step, write the evidence under .ai/local/evidence/use-cases/; exit 10 when a step fails
  coverage  every public command, guaranteed claim and MCP tool against the use cases that name and
            run it; --check exits 10 on a gap the policy makes required (policy: use_cases.coverage)
  impact    from the files changed since --base (default: the upstream, else HEAD) plus the work
            tree, the commands, rules, use cases, scenarios and behavioural cases affected
  scaffold  write a draft use case for every public command no active use case names, from what the
            registry, the command fixture and the claims already know; never marks anything guaranteed
USAGE
}

# ---------------------------------------------------------------- loading
mj_uc_paths() {
  MJ_UC_DIR="$MJ_AI_DIR/$(mj_man sections.use-cases)"; MJ_AP_DIR="$MJ_AI_DIR/$(mj_man sections.applications)"
  [ -n "$(mj_man sections.use-cases)" ] || MJ_UC_DIR=""
  [ -n "$(mj_man sections.applications)" ] || MJ_AP_DIR=""
  MJ_UC_EVIDENCE="$MJ_AI_LOCAL_DIR/evidence/use-cases"
}

# the front matter of a Markdown object, flattened into a temp file; empty on failure
mj_uc_flat() {
  local f="$1" front tmp
  front="$(mj_record_front "$f")" || return 1
  [ -n "$front" ] || return 1
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ucf.XXXXXX")"
  printf '%s\n' "$front" > "$tmp.yaml"
  mj_yaml_flatten "$tmp.yaml" > "$tmp" 2>/dev/null || { rm -f "$tmp" "$tmp.yaml"; return 1; }
  rm -f "$tmp.yaml"
  printf '%s' "$tmp"
}

# load every use case and application once: MJ_UC_FILE_<i>, MJ_UC_FLAT_<i>, MJ_UC_ID_<i>;
# same for applications with AP. Order is the sorted file order, which is stable.
mj_uc_load() {
  [ "$MJ_UC_LOADED" = 1 ] && return 0
  mj_uc_paths
  local f i=0 j=0 flat
  MJ_UC_IDS=""; MJ_AP_IDS=""
  if [ -n "$MJ_UC_DIR" ] && [ -d "$MJ_UC_DIR" ]; then
    for f in "$MJ_UC_DIR"/*.md; do
      [ -f "$f" ] || continue
      flat="$(mj_uc_flat "$f")" || flat=""
      # a context document beside the use cases (the section's README) is not a use case
      [ -n "$flat" ] && [ "$(mj_yget "$flat" kind)" = context ] && { rm -f "$flat"; continue; }
      printf -v "MJ_UC_FILE_$i" '%s' "$f"; printf -v "MJ_UC_FLAT_$i" '%s' "$flat"
      # every field becomes a variable once; the readers below are expansions, not processes
      [ -n "$flat" ] && mj_yload "$flat" "MJUC$i"
      MJ_UC_IDS="$MJ_UC_IDS $(mj_uc_v "$i" id)"
      i=$((i+1))
    done
  fi
  MJ_UC_N=$i
  if [ -n "$MJ_AP_DIR" ] && [ -d "$MJ_AP_DIR" ]; then
    for f in "$MJ_AP_DIR"/*.md; do
      [ -f "$f" ] || continue
      flat="$(mj_uc_flat "$f")" || flat=""
      [ -n "$flat" ] && [ "$(mj_yget "$flat" kind)" = context ] && { rm -f "$flat"; continue; }
      printf -v "MJ_AP_FILE_$j" '%s' "$f"; printf -v "MJ_AP_FLAT_$j" '%s' "$flat"
      [ -n "$flat" ] && mj_yload "$flat" "MJAP$j"
      MJ_AP_IDS="$MJ_AP_IDS $(mj_ap_v "$j" id)"
      j=$((j+1))
    done
  fi
  MJ_AP_N=$j
  MJ_UC_LOADED=1
}
mj_uc_v()    { mj_yv "MJUC$1" "$2"; }
# the same value without a process: MJ_V is set, empty when absent
mj_uc_get()  { local k="${2//-/___}"; k="${k//./__}"; k="MJUC${1}__$k"; MJ_V="${!k:-}"; }
mj_ap_get()  { local k="${2//-/___}"; k="${k//./__}"; k="MJAP${1}__$k"; MJ_V="${!k:-}"; }
mj_uc_list() { mj_yvlist "MJUC$1" "$2"; }
# membership without a process: the list is joined once per use case and field, then
# every test is a pattern match in this shell
mj_uc_has() { # index field value
  local n="MJUCS${1}__$2" v i=0 k acc
  if [ -z "${!n+x}" ]; then
    acc=" "; while k="MJUC${1}__${2}__$i"; [ -n "${!k:-}" ]; do acc="$acc${!k} "; i=$((i+1)); done
    printf -v "$n" '%s' "$acc"
  fi
  v="${!n}"; case "$v" in *" $3 "*) return 0 ;; esac; return 1
}
mj_uc_runs() { # index command: does the scenario run it?
  local n="MJUCR$1" i=0 k acc
  if [ -z "${!n+x}" ]; then
    acc=" "; while k="MJUC${1}__scenario__steps__${i}__run__0"; [ -n "${!k:-}" ]; do acc="$acc${!k} "; i=$((i+1)); done
    printf -v "$n" '%s' "$acc"
  fi
  case "${!n}" in *" $2 "*) return 0 ;; esac; return 1
}
mj_uc_file() { local n="MJ_UC_FILE_$1"; printf '%s' "${!n}"; }
mj_ap_v()    { mj_yv "MJAP$1" "$2"; }
mj_ap_list() { mj_yvlist "MJAP$1" "$2"; }
mj_ap_file() { local n="MJ_AP_FILE_$1"; printf '%s' "${!n}"; }
# the index of a use case by id, or failure
mj_uc_index() { local i=0; while [ "$i" -lt "$MJ_UC_N" ]; do [ "$(mj_uc_v "$i" id)" = "$1" ] && { printf '%s' "$i"; return 0; }; i=$((i+1)); done; return 1; }
mj_ap_index() { local j=0; while [ "$j" -lt "$MJ_AP_N" ]; do [ "$(mj_ap_v "$j" id)" = "$1" ] && { printf '%s' "$j"; return 0; }; j=$((j+1)); done; return 1; }
# the categories of the taxonomy, one id per line
mj_uc_categories() {
  local t="$MJ_UC_DIR/taxonomy.yaml" flat
  [ -f "$t" ] || return 0
  flat="$(mktemp "${TMPDIR:-/tmp}/mj.tax.XXXXXX")"
  mj_yaml_flatten "$t" > "$flat" 2>/dev/null || { rm -f "$flat"; return 0; }
  awk -F= '/^categories\.[0-9]+\.id=/ { print substr($0, index($0, "=") + 1) }' "$flat"
  rm -f "$flat"
}
# the commands a scenario runs, one per line, in order
mj_uc_scenario_commands() { local i="$1" k=0 c; while c="$(mj_uc_v "$i" "scenario.steps.$k.run.0")"; [ -n "$c" ]; do printf '%s\n' "$c"; k=$((k+1)); done; }
mj_uc_has_scenario() { local k="MJUC${1}__scenario__setup"; [ -n "${!k:-}" ]; }
mj_uc_active() { local k="MJUC${1}__status"; [ "${!k:-}" = active ]; }
# the fixture directory the scenarios draw setup scripts and stdin bodies from: the
# repository's own when it has one, otherwise the distribution's (a managed repository
# uses the tool's prepared states)
mj_uc_fixture_dir() {
  if [ -d "$MJ_ROOT/test/fixtures/commands/setup" ]; then printf '%s' "$MJ_ROOT/test/fixtures/commands"
  else printf '%s' "$MJ_BIN_DIR/../test/fixtures/commands"; fi
}

# ---------------------------------------------------------------- the command
mj_cmd_usecase() {
  local sub="${1:-}"; shift 2>/dev/null || true
  case "$sub" in
    list) mj_uc_cmd_list "$@" ;;
    show) mj_uc_cmd_show "$@" ;;
    validate) mj_uc_cmd_validate "$@" ;;
    run) mj_uc_cmd_run "$@" ;;
    coverage) mj_uc_cmd_coverage "$@" ;;
    impact) mj_uc_cmd_impact "$@" ;;
    scaffold) mj_uc_cmd_scaffold "$@" ;;
    --help|-h|help|'') mj_uc_usage; [ -n "$sub" ] && return 0; return "$MJ_EX_USAGE" ;;
    *) mj_err "usecase: unknown subcommand '$sub'"; mj_uc_usage >&2; return "$MJ_EX_USAGE" ;;
  esac
}

mj_uc_require() {
  mj_require_installed
  mj_load_policy >/dev/null 2>&1 || true
  mj_uc_load
  [ -n "$MJ_UC_DIR" ] || mj_die "$MJ_EX_MISSING" "the manifest names no use-cases section; add \`use-cases: repo/use-cases\` under sections: (majordomus init --extend seeds it)"
  [ -d "$MJ_UC_DIR" ] || mj_die "$MJ_EX_MISSING" "no $(mj_rel "$MJ_UC_DIR")/ in $MJ_ROOT; the manifest names it (majordomus init --extend)"
}

# ---------------------------------------------------------------- list / show
mj_uc_cmd_list() {
  local json="${MJ_JSON:-0}" i first=1
  while [ $# -gt 0 ]; do case "$1" in --json) json=1; shift ;; --help|-h) mj_uc_usage; return 0 ;; *) mj_die "$MJ_EX_USAGE" "usecase list: unknown option $1" ;; esac; done
  mj_uc_require
  if [ "$json" = 1 ]; then
    printf '{"schema":"majordomus/use-cases/v1","count":%s,"use_cases":[' "$MJ_UC_N"
    i=0
    while [ "$i" -lt "$MJ_UC_N" ]; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"id":"%s","title":"%s","category":"%s","status":"%s","commands":%s,"scenario":%s,"path":"%s"}' \
        "$(mj_json_esc "$(mj_uc_v "$i" id)")" "$(mj_json_esc "$(mj_uc_v "$i" title)")" "$(mj_json_esc "$(mj_uc_v "$i" category)")" \
        "$(mj_json_esc "$(mj_uc_v "$i" status)")" "$(mj_uc_list "$i" commands | mj_uc_jarr)" \
        "$( mj_uc_has_scenario "$i" && printf true || printf false )" "$(mj_json_esc "$(mj_rel "$(mj_uc_file "$i")")")"
      i=$((i+1))
    done
    printf ']}\n'
    return 0
  fi
  i=0
  while [ "$i" -lt "$MJ_UC_N" ]; do
    printf '%-38s %-12s %-10s %-9s %s\n' "$(mj_uc_v "$i" id)" "$(mj_uc_v "$i" category)" "$(mj_uc_v "$i" status)" \
      "$( mj_uc_has_scenario "$i" && printf scenario || printf described )" "$(mj_uc_list "$i" commands | tr '\n' ' ')"
    i=$((i+1))
  done
  printf 'use cases: %s in %s\n' "$MJ_UC_N" "$(mj_rel "$MJ_UC_DIR")"
}
# lines on stdin as a JSON array of strings
mj_uc_jarr() { local out="" l; while IFS= read -r l; do [ -n "$l" ] || continue; out="$out\"$(mj_json_esc "$l")\","; done; printf '[%s]' "${out%,}"; }

mj_uc_cmd_show() {
  local id="${1:-}" i ev
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "usecase show: which use case? (majordomus usecase list)"
  mj_uc_require
  i="$(mj_uc_index "$id")" || mj_die "$MJ_EX_MISSING" "no use case '$id' under $(mj_rel "$MJ_UC_DIR")/ (majordomus usecase list)"
  cat "$(mj_uc_file "$i")"
  ev="$MJ_UC_EVIDENCE/$id.json"
  if [ -f "$ev" ]; then
    printf '\n--- evidence: %s (%s)\n' "$(mj_rel "$ev")" "$(grep -o '"result":"[a-z]*"' "$ev" | head -1 | cut -d'"' -f4)"
  else
    printf '\n--- evidence: none yet (majordomus usecase run %s)\n' "$id"
  fi
}

# ---------------------------------------------------------------- validate
# Every finding is a doctrine finding under the category use-case, so that doctor and this
# command print the same lines. Returns the number of failures in MJ_UC_BAD.
MJ_UC_BAD=0
mj_uc_bad() { mj_doctrine_fail use-case "$1" "$2" "${3:-}"; MJ_UC_BAD=$((MJ_UC_BAD+1)); }
mj_uc_validate_all() {
  local i j id f base ref k cats cmds dispatch fix stdin_dir body claims resp sid seen="" cmd_list found cmd unk flatn sids st
  MJ_UC_BAD=0
  mj_uc_load
  # the universes a reference may name, each one string tested with a pattern, no process
  cats=" $(mj_uc_categories | tr '\n' ' ') "
  dispatch=" $(grep -oE '^  [a-z|-]+\)$' "$MJ_BIN_DIR/majordomus" | tr -d ' )' | tr '|\n' '  ') "
  fix="$(mj_uc_fixture_dir)"; stdin_dir="$fix/stdin"
  claims=" $(grep -E '^  - id: ' "$MJ_ROOT/docs/CLAIMS.yaml" 2>/dev/null | sed 's/^  - id: //' | tr '\n' ' ') "
  resp=" $(grep -E '^  - id: ' "$MJ_ROOT/docs/RESPONSIBILITIES.yaml" 2>/dev/null | sed 's/^  - id: //' | tr '\n' ' ') "
  local tools=""
  [ -f "$MJ_ROOT/docs/generated/registry.json" ] && tools=" $(grep -o '"tool": *"[a-z_]*"' "$MJ_ROOT/docs/generated/registry.json" | sed 's/.*"\([a-z_]*\)"$/\1/' | tr '\n' ' ') "
  [ -f "$MJ_UC_DIR/taxonomy.yaml" ] || mj_uc_bad "$(mj_rel "$MJ_UC_DIR")/taxonomy.yaml" "absent; the categories use cases are filed under are declared there" "majordomus init --extend"
  i=0
  while [ "$i" -lt "$MJ_UC_N" ]; do
    f="$(mj_uc_file "$i")"; base="${f##*/}"; base="${base%.md}"; mj_uc_get "$i" id; id="$MJ_V"
    if [ -z "$id" ]; then mj_uc_bad "$(mj_rel "$f")" "front matter does not parse or declares no id" "majordomus usecase validate"; i=$((i+1)); continue; fi
    [ "$id" = "$base" ] || mj_uc_bad "$(mj_rel "$f")" "id '$id' is not the file name" "mv $(mj_rel "$f") $(mj_rel "$MJ_UC_DIR")/$id.md"
    case " $seen " in *" $id "*) mj_uc_bad "$id" "declared twice" "majordomus usecase list" ;; esac; seen="$seen $id"
    mj_uc_get "$i" kind; [ "$MJ_V" = "use-case" ] || mj_uc_bad "$id" "kind is '$MJ_V', not use-case" ""
    mj_uc_get "$i" status; st="$MJ_V"
    case "$st" in active|draft|deprecated) ;; *) mj_uc_bad "$id" "status '$st' is not active, draft or deprecated" "" ;; esac
    for k in title summary category; do mj_uc_get "$i" "$k"; [ -n "$MJ_V" ] || mj_uc_bad "$id" "declares no $k" ""; done
    # keys the schema does not declare: the allow-list is generated from the schema
    flatn="MJ_UC_FLAT_$i"
    unk="$(mj_yaml_unknown_keys "${!flatn}" "$MJ_ALLOW_DIR/use-case.txt" || true)"
    [ -z "$unk" ] || mj_uc_bad "$id" "unknown key(s): $(printf '%s' "$unk" | tr '\n' ' ')" "share/schemas/use-case.schema.json"
    mj_uc_get "$i" category; case "$cats" in *" $MJ_V "*) ;; *) mj_uc_bad "$id" "category '$MJ_V' is not in taxonomy.yaml" "$(mj_rel "$MJ_UC_DIR")/taxonomy.yaml" ;; esac
    cmd_list="$(mj_uc_list "$i" commands)"
    [ -n "$cmd_list" ] || mj_uc_bad "$id" "names no command; a use case that runs nothing is a description" ""
    for ref in $cmd_list; do case "$dispatch" in *" $ref "*) ;; *) mj_uc_bad "$id" "names command '$ref', which bin/majordomus does not dispatch" "majordomus --help" ;; esac; done
    for ref in $(mj_uc_list "$i" doctrines); do mj_doc_index "$ref" >/dev/null || mj_uc_bad "$id" "names doctrine '$ref', which no rule declares" "majordomus doctrine list"; done
    for ref in $(mj_uc_list "$i" claims); do case "$claims" in *" $ref "*) ;; *) mj_uc_bad "$id" "names claim '$ref', which docs/CLAIMS.yaml does not have" "grep -n 'id: ' docs/CLAIMS.yaml" ;; esac; done
    for ref in $(mj_uc_list "$i" responsibilities); do case "$resp" in *" $ref "*) ;; *) mj_uc_bad "$id" "names responsibility '$ref', which docs/RESPONSIBILITIES.yaml does not have" "" ;; esac; done
    for ref in $(mj_uc_list "$i" applications); do
      j="$(mj_ap_index "$ref")" || { mj_uc_bad "$id" "names application '$ref', which does not exist" "ls $(mj_rel "$MJ_AP_DIR")"; continue; }
      case " $(mj_ap_list "$j" use_cases | tr '\n' ' ') " in *" $id "*) ;; *) mj_uc_bad "$id" "names application '$ref', which does not name it back" "$(mj_rel "$(mj_ap_file "$j")")" ;; esac
    done
    if [ -n "$tools" ]; then
      for ref in $(mj_uc_list "$i" mcp_tools); do case "$tools" in *" $ref "*) ;; *) mj_uc_bad "$id" "names MCP tool '$ref', which the registry does not project" "docs/generated/registry.json" ;; esac; done
    fi
    # the body carries the sections a reader expects
    body="$(mj_record_body "$f")"
    for k in "# Situation" "# Outcome"; do case "$body" in *"$k"*) ;; *) mj_uc_bad "$id" "body has no '$k' heading" "" ;; esac; done
    # the scenario: setup exists, stdin bodies exist, every step names a declared command,
    # step ids are unique, every step expects an exit code
    if mj_uc_has_scenario "$i"; then
      mj_uc_get "$i" scenario.setup
      [ -f "$fix/setup/$MJ_V.sh" ] || mj_uc_bad "$id" "scenario names setup '$MJ_V', which $(mj_rel "$fix")/setup/ does not have" ""
      k=0; sids=""
      while mj_uc_get "$i" "scenario.steps.$k.id"; sid="$MJ_V"; [ -n "$sid" ]; do
        case " $sids " in *" $sid "*) mj_uc_bad "$id" "step '$sid' is declared twice" "" ;; esac; sids="$sids $sid"
        mj_uc_get "$i" "scenario.steps.$k.run.0"; cmd="$MJ_V"
        [ -n "$cmd" ] || mj_uc_bad "$id" "step '$sid' runs nothing" ""
        case " $(printf '%s' "$cmd_list" | tr '\n' ' ') " in *" $cmd "*) ;; *) mj_uc_bad "$id" "step '$sid' runs '$cmd', which the use case does not list under commands" "" ;; esac
        mj_uc_get "$i" "scenario.steps.$k.expect.exit"; case "$MJ_V" in ''|*[!0-9]*) mj_uc_bad "$id" "step '$sid' expects no exit code" "" ;; esac
        mj_uc_get "$i" "scenario.steps.$k.stdin"; found="$MJ_V"
        [ -z "$found" ] || [ -f "$stdin_dir/$found" ] || mj_uc_bad "$id" "step '$sid' names stdin '$found', which $(mj_rel "$stdin_dir")/ does not have" ""
        k=$((k+1))
      done
      [ "$k" -gt 0 ] || mj_uc_bad "$id" "scenario has no steps" ""
    else
      mj_uc_get "$i" target
      [ "$st" = active ] && [ "$MJ_V" = guaranteed ] && mj_uc_bad "$id" "targets guaranteed and has no scenario; a guarantee needs executable evidence" "add a scenario, or target: advisory"
    fi
    i=$((i+1))
  done
  # applications: mutual references, both lists present
  j=0
  while [ "$j" -lt "$MJ_AP_N" ]; do
    f="$(mj_ap_file "$j")"; mj_ap_get "$j" id; id="$MJ_V"
    [ -n "$id" ] || { mj_uc_bad "$(mj_rel "$f")" "application front matter does not parse or declares no id" ""; j=$((j+1)); continue; }
    base="${f##*/}"; [ "$id" = "${base%.md}" ] || mj_uc_bad "$id" "application id is not the file name" ""
    mj_ap_get "$j" fits_when.0; [ -n "$MJ_V" ] || mj_uc_bad "$id" "application declares no fits_when" ""
    mj_ap_get "$j" does_not_fit_when.0; [ -n "$MJ_V" ] || mj_uc_bad "$id" "application declares no does_not_fit_when; a catalogue that only lists fits is marketing" ""
    for ref in $(mj_ap_list "$j" use_cases); do
      k="$(mj_uc_index "$ref")" || { mj_uc_bad "$id" "application names use case '$ref', which does not exist" ""; continue; }
      mj_uc_has "$k" applications "$id" || mj_uc_bad "$id" "application names use case '$ref', which does not name it back" ""
    done
    for ref in $(mj_ap_list "$j" doctrines); do mj_doc_index "$ref" >/dev/null || mj_uc_bad "$id" "application names doctrine '$ref', which no rule declares" ""; done
    case "$(mj_record_body "$f")" in *"# Context"*) ;; *) mj_uc_bad "$id" "application body has no '# Context' heading" "" ;; esac
    j=$((j+1))
  done
  return 0
}

mj_uc_cmd_validate() {
  while [ $# -gt 0 ]; do case "$1" in --json) MJ_JSON=1; shift ;; --help|-h) mj_uc_usage; return 0 ;; *) mj_die "$MJ_EX_USAGE" "usecase validate: unknown option $1" ;; esac; done
  mj_uc_require
  # shellcheck disable=SC1091
  . "$MJ_LIB_DIR/rules.sh"; . "$MJ_LIB_DIR/doctrine.sh"; mj_doctrine_load
  mj_uc_validate_all
  if [ "$MJ_UC_BAD" = 0 ]; then
    mj_doctrine_ok use-case "$MJ_UC_N use case(s), $MJ_AP_N application(s)" "every command, doctrine, claim, responsibility, application, category, setup and stdin resolves; every scenario step names a listed command"
    printf 'usecase validate: 0 failure(s)\n'; return 0
  fi
  printf 'usecase validate: %s failure(s)\n' "$MJ_UC_BAD"; return "$MJ_EX_CONTRACT"
}

# ---------------------------------------------------------------- run
# Normalise what the tool printed so that two runs of one scenario are byte-identical
# wherever the behaviour is: the scenario repository's path, the tool's own path, the
# home directory, timestamps, task and session ids, record hashes, durations.
mj_uc_normalise() { # repo-path
  sed -E \
    -e "s#$1#<repo>#g" \
    -e "s#$MJ_BIN_DIR/majordomus#majordomus#g" \
    -e "s#${HOME}#<home>#g" \
    -e 's/[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z/<time>/g' \
    -e 's/[0-9]{8}T[0-9]{6}Z/<time>/g' \
    -e 's/[0-9]{4}-[0-9]{2}-[0-9]{2}/<date>/g' \
    -e 's/\bt-[0-9]{14}-[0-9a-f]{4}\b/t-<id>/g' \
    -e 's/\bs-[0-9]+-[0-9a-f]+\b/s-<id>/g' \
    -e 's/--[0-9a-f]{7}--[0-9a-f]{16}\.md/--<head>--<hash>.md/g' \
    -e 's/\b[0-9a-f]{40}\b/<sha>/g' \
    -e 's/\b[0-9a-f]{16}\b/<hash16>/g' \
    -e 's/\b[0-9a-f]{12}\b/<hash12>/g' \
    -e 's/\b[0-9]+ ms\b/<n> ms/g' \
    -e 's/\([0-9]+m ago/(<n>m ago/g' \
    -e 's/\b(bash|git|jq|shellcheck) [0-9][0-9.]*/\1 <version>/g'
}

# run one use case's scenario; prints the evidence JSON to the file named; returns 0 on
# pass, 1 on a failed step, 2 when the use case has no scenario
mj_uc_run_one() { # index, evidence-file, keep(0|1)
  local i="$1" out="$2" keep="${3:-0}" id setup base W k sid argv_n a rc want pat ok all_ok=1 t0 t1 dur stdin_f
  local fix steps_json="" first=1 raw norm asserts fail_reason
  id="$(mj_uc_v "$i" id)"; setup="$(mj_uc_v "$i" scenario.setup)"
  [ -n "$setup" ] || return 2
  fix="$(mj_uc_fixture_dir)"
  base="$(mktemp -d "${TMPDIR:-/tmp}/mj-uc.XXXXXX")"
  W="$base/repo"; mkdir -p "$W"
  ( cd "$W" && git init -q . && git config user.email t@example.com && git config user.name t && git commit -q --allow-empty -m init ) || { rm -rf "$base"; mj_err "usecase run: cannot create a repository for $id"; return 1; }
  # the setup script prepares the repository, with the same helpers the test suite gives it
  local setup_out="$base/setup.out"
  # the helpers the setup scripts use (pj_* for a plan model) come from the tool's test
  # library, which the distribution ships beside the fixtures
  local helpers="$fix/../../lib.sh"
  ( cd "$W" && MJ="$MJ_BIN_DIR/majordomus" FIXTURE_SETUP="$fix/setup" ROOT="$MJ_ROOT" && export MJ FIXTURE_SETUP ROOT \
      && if [ -f "$helpers" ]; then . "$helpers"; fi \
      && . "$fix/setup/$setup.sh" ) > "$setup_out" 2>&1 || {
    printf '{"schema":"majordomus/use-case-evidence/v1","use_case":"%s","setup":"%s","result":"fail","reason":"setup failed","setup_output":"%s","steps":[]}\n' \
      "$id" "$setup" "$(mj_json_esc "$(mj_uc_normalise "$W" < "$setup_out" | head -c 4000)")" > "$out"
    [ "$keep" = 1 ] || rm -rf "$base"; return 1; }
  k=0
  while sid="$(mj_uc_v "$i" "scenario.steps.$k.id")"; [ -n "$sid" ]; do
    set --; argv_n=0
    while a="$(mj_uc_v "$i" "scenario.steps.$k.run.$argv_n")"; [ -n "$a" ]; do set -- "$@" "$a"; argv_n=$((argv_n+1)); done
    stdin_f="$(mj_uc_v "$i" "scenario.steps.$k.stdin")"
    want="$(mj_uc_v "$i" "scenario.steps.$k.expect.exit")"
    raw="$base/step-$k.out"; rc=0; t0="$(mj_ms)"
    if [ -n "$stdin_f" ]; then ( cd "$W" && "$MJ_BIN_DIR/majordomus" "$@" < "$fix/stdin/$stdin_f" ) > "$raw" 2>&1 || rc=$?
    else ( cd "$W" && "$MJ_BIN_DIR/majordomus" "$@" < /dev/null ) > "$raw" 2>&1 || rc=$?; fi
    t1="$(mj_ms)"; dur=$((t1 - t0))
    norm="$(mj_uc_normalise "$W" < "$raw")"
    ok=1; asserts=""; fail_reason=""
    if [ "$rc" != "$want" ]; then ok=0; fail_reason="expected exit $want, got $rc"; fi
    asserts="$asserts{\"kind\":\"exit\",\"expected\":$want,\"observed\":$rc,\"result\":\"$([ "$rc" = "$want" ] && printf pass || printf fail)\"},"
    local n=0
    while pat="$(mj_uc_v "$i" "scenario.steps.$k.expect.stdout_contains.$n")"; [ -n "$pat" ]; do
      if grep -qE -- "$pat" "$raw"; then asserts="$asserts{\"kind\":\"stdout_contains\",\"pattern\":\"$(mj_json_esc "$pat")\",\"result\":\"pass\"},"
      else ok=0; [ -n "$fail_reason" ] || fail_reason="expected /$pat/ in the output"; asserts="$asserts{\"kind\":\"stdout_contains\",\"pattern\":\"$(mj_json_esc "$pat")\",\"result\":\"fail\"},"; fi
      n=$((n+1))
    done
    n=0
    while pat="$(mj_uc_v "$i" "scenario.steps.$k.expect.stdout_not_contains.$n")"; [ -n "$pat" ]; do
      if grep -qE -- "$pat" "$raw"; then ok=0; [ -n "$fail_reason" ] || fail_reason="did not expect /$pat/ in the output"; asserts="$asserts{\"kind\":\"stdout_not_contains\",\"pattern\":\"$(mj_json_esc "$pat")\",\"result\":\"fail\"},"
      else asserts="$asserts{\"kind\":\"stdout_not_contains\",\"pattern\":\"$(mj_json_esc "$pat")\",\"result\":\"pass\"},"; fi
      n=$((n+1))
    done
    n=0
    while pat="$(mj_uc_v "$i" "scenario.steps.$k.expect.files_exist.$n")"; [ -n "$pat" ]; do
      if [ -e "$W/$pat" ]; then asserts="$asserts{\"kind\":\"file_exists\",\"path\":\"$(mj_json_esc "$pat")\",\"result\":\"pass\"},"
      else ok=0; [ -n "$fail_reason" ] || fail_reason="expected $pat to exist"; asserts="$asserts{\"kind\":\"file_exists\",\"path\":\"$(mj_json_esc "$pat")\",\"result\":\"fail\"},"; fi
      n=$((n+1))
    done
    n=0
    while pat="$(mj_uc_v "$i" "scenario.steps.$k.expect.files_contain.$n.path")"; [ -n "$pat" ]; do
      local fpat; fpat="$(mj_uc_v "$i" "scenario.steps.$k.expect.files_contain.$n.pattern")"
      if [ -f "$W/$pat" ] && grep -qE -- "$fpat" "$W/$pat"; then asserts="$asserts{\"kind\":\"file_contains\",\"path\":\"$(mj_json_esc "$pat")\",\"pattern\":\"$(mj_json_esc "$fpat")\",\"result\":\"pass\"},"
      else ok=0; [ -n "$fail_reason" ] || fail_reason="expected /$fpat/ in $pat"; asserts="$asserts{\"kind\":\"file_contains\",\"path\":\"$(mj_json_esc "$pat")\",\"pattern\":\"$(mj_json_esc "$fpat")\",\"result\":\"fail\"},"; fi
      n=$((n+1))
    done
    [ "$first" = 1 ] || steps_json="$steps_json,"; first=0
    steps_json="$steps_json{\"id\":\"$(mj_json_esc "$sid")\",\"command\":\"$(mj_json_esc "majordomus $*")\",\"argv\":$(printf '%s\n' "$@" | mj_uc_jarr),\"stdin\":$( [ -n "$stdin_f" ] && printf '"%s"' "$(mj_json_esc "$stdin_f")" || printf null ),\"exit\":$rc,\"expected_exit\":$want,\"output\":\"$(mj_json_esc "$(printf '%s' "$norm" | head -c 12000)")\",\"assertions\":[${asserts%,}],\"result\":\"$([ "$ok" = 1 ] && printf pass || printf fail)\",\"reason\":$( [ -n "$fail_reason" ] && printf '"%s"' "$(mj_json_esc "$fail_reason")" || printf null ),\"timing\":{\"duration_ms\":$dur}}"
    [ "$ok" = 1 ] || { all_ok=0; break; }
    k=$((k+1))
  done
  printf '{"schema":"majordomus/use-case-evidence/v1","use_case":"%s","setup":"%s","result":"%s","steps":[%s]}\n' \
    "$id" "$setup" "$([ "$all_ok" = 1 ] && printf pass || printf fail)" "$steps_json" > "$out"
  if [ "$keep" = 1 ]; then printf 'kept: %s\n' "$W" >&2; else rm -rf "$base"; fi
  [ "$all_ok" = 1 ]
}

mj_uc_cmd_run() {
  local json="${MJ_JSON:-0}" outdir="" keep=0 ids="" i id ev rc fails=0 ran=0 skipped=0 first=1
  while [ $# -gt 0 ]; do case "$1" in
    --json) json=1; shift ;; --out) outdir="$2"; shift 2 ;; --keep) keep=1; shift ;;
    --help|-h) mj_uc_usage; return 0 ;; -*) mj_die "$MJ_EX_USAGE" "usecase run: unknown option $1" ;;
    *) ids="$ids $1"; shift ;; esac; done
  mj_uc_require
  mkdir -p "$MJ_UC_EVIDENCE"
  [ -z "$outdir" ] || mkdir -p "$outdir"
  [ -n "$ids" ] || ids="$MJ_UC_IDS"
  [ "$json" = 1 ] && printf '{"schema":"majordomus/use-case-run/v1","results":['
  for id in $ids; do
    i="$(mj_uc_index "$id")" || mj_die "$MJ_EX_MISSING" "no use case '$id' (majordomus usecase list)"
    if ! mj_uc_has_scenario "$i"; then
      skipped=$((skipped+1))
      [ "$json" = 1 ] || printf '%-38s described (no scenario)\n' "$id"
      continue
    fi
    ev="$MJ_UC_EVIDENCE/$id.json"; rc=0
    mj_uc_run_one "$i" "$ev" "$keep" || rc=$?
    ran=$((ran+1))
    [ -z "$outdir" ] || cp "$ev" "$outdir/$id.json"
    if [ "$json" = 1 ]; then [ "$first" = 1 ] || printf ','; first=0; cat "$ev" | tr -d '\n'
    else
      if [ "$rc" = 0 ]; then printf '%-38s pass  %s step(s)\n' "$id" "$(grep -o '"id":"' "$ev" | wc -l | tr -d ' ')"
      else fails=$((fails+1)); printf '%-38s FAIL  %s\n' "$id" "$(grep -o '"reason":"[^"]*"' "$ev" | grep -v 'null' | head -1 | cut -d'"' -f4)"
        grep -o '"output":"[^"]*"' "$ev" | tail -1 | cut -d'"' -f4 | sed 's/\\n/\n/g' | sed 's/^/      | /' | head -20; fi
    fi
  done
  if [ "$json" = 1 ]; then printf '],"ran":%s,"failed":%s,"skipped":%s}\n' "$ran" "$fails" "$skipped"
  else printf 'usecase run: %s scenario(s), %s failed, %s described only; evidence under %s/\n' "$ran" "$fails" "$skipped" "$(mj_rel "$MJ_UC_EVIDENCE")"; fi
  mj_ledger_append use_cases.ran "\"ran\":$ran,\"failed\":$fails" 2>/dev/null || true
  [ "$fails" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}

# ---------------------------------------------------------------- coverage
# What the policy requires, per class: required | advisory | off
mj_uc_policy() {
  case "$1" in
    commands) mj_pol_req use_cases.coverage.commands ;;
    claims) mj_pol_req use_cases.coverage.claims ;;
    mcp_tools) mj_pol_req use_cases.coverage.mcp_tools ;;
  esac
}
# lines: class<TAB>id<TAB>named<TAB>executed<TAB>evidence<TAB>status
mj_uc_coverage_rows() {
  local c i n_named n_exec n_ev id named exec ev cls st
  mj_cmdreg_load || true
  for c in $(mj_cmdreg_public); do
    n_named=0; n_exec=0; n_ev=0
    i=0
    while [ "$i" -lt "$MJ_UC_N" ]; do
      mj_uc_active "$i" || { i=$((i+1)); continue; }
      if mj_uc_has "$i" commands "$c"; then
        n_named=$((n_named+1))
        if mj_uc_runs "$i" "$c"; then n_exec=$((n_exec+1))
          local idk="MJUC${i}__id"; ev="$MJ_UC_EVIDENCE/${!idk}.json"; [ -f "$ev" ] && grep -q '"result":"pass"' "$ev" && n_ev=$((n_ev+1)); fi
      fi
      i=$((i+1))
    done
    mj_uc_status_of "$n_named" "$n_exec"; printf 'command\t%s\t%s\t%s\t%s\t%s\n' "$c" "$n_named" "$n_exec" "$n_ev" "$MJ_UC_ST"
  done
  # guaranteed claims of the shell tool's responsibilities (responsibility != none)
  awk '/^  - id: /{id=$3} /^    status: /{st=$2} /^    responsibility: /{r=$2; if (st=="guaranteed" && r!="none") print id}' "$MJ_ROOT/docs/CLAIMS.yaml" 2>/dev/null | while IFS= read -r cls; do
    n_named=0; n_exec=0
    i=0
    while [ "$i" -lt "$MJ_UC_N" ]; do
      mj_uc_active "$i" && mj_uc_has "$i" claims "$cls" && { n_named=$((n_named+1)); mj_uc_has_scenario "$i" && n_exec=$((n_exec+1)); }
      i=$((i+1))
    done
    mj_uc_status_of "$n_named" "$n_exec"; printf 'claim\t%s\t%s\t%s\t%s\t%s\n' "$cls" "$n_named" "$n_exec" "$n_exec" "$MJ_UC_ST"
  done
  if [ -f "$MJ_ROOT/docs/generated/registry.json" ]; then
    grep -o '"tool": *"[a-z_]*"' "$MJ_ROOT/docs/generated/registry.json" | sed 's/.*"\([a-z_]*\)"$/\1/' | sort -u | while IFS= read -r cls; do
      n_named=0
      i=0
      while [ "$i" -lt "$MJ_UC_N" ]; do
        mj_uc_active "$i" && mj_uc_has "$i" mcp_tools "$cls" && n_named=$((n_named+1))
        i=$((i+1))
      done
      mj_uc_status_of "$n_named" "$n_named"; printf 'mcp_tool\t%s\t%s\t%s\t%s\t%s\n' "$cls" "$n_named" "$n_named" "$n_named" "$MJ_UC_ST"
    done
  fi
}
mj_uc_status_of() { if [ "$2" -gt 0 ]; then MJ_UC_ST=covered; elif [ "$1" -gt 0 ]; then MJ_UC_ST=partial; else MJ_UC_ST=gap; fi; }

mj_uc_cmd_coverage() {
  local json="${MJ_JSON:-0}" check=0 rows cls id named exec ev st pol gaps=0 total=0 covered=0 partial=0 first=1
  while [ $# -gt 0 ]; do case "$1" in --json) json=1; shift ;; --check) check=1; shift ;; --help|-h) mj_uc_usage; return 0 ;; *) mj_die "$MJ_EX_USAGE" "usecase coverage: unknown option $1" ;; esac; done
  mj_uc_require
  rows="$(mj_uc_coverage_rows)"
  [ "$json" = 1 ] && printf '{"schema":"majordomus/use-case-coverage/v1","policy":{"commands":"%s","claims":"%s","mcp_tools":"%s"},"rows":[' "$(mj_uc_policy commands)" "$(mj_uc_policy claims)" "$(mj_uc_policy mcp_tools)"
  while IFS=$'\t' read -r cls id named exec ev st; do
    [ -n "$cls" ] || continue
    total=$((total+1))
    case "$cls" in command) pol="$(mj_uc_policy commands)" ;; claim) pol="$(mj_uc_policy claims)" ;; mcp_tool) pol="$(mj_uc_policy mcp_tools)" ;; esac
    case "$st" in covered) covered=$((covered+1)) ;; partial) partial=$((partial+1)) ;; esac
    [ "$st" = covered ] || [ "$pol" != required ] || gaps=$((gaps+1))
    if [ "$json" = 1 ]; then [ "$first" = 1 ] || printf ','; first=0
      printf '{"class":"%s","id":"%s","use_cases":%s,"executable":%s,"evidence":%s,"status":"%s","policy":"%s"}' "$cls" "$id" "$named" "$exec" "$ev" "$st" "$pol"
    else printf '%-9s %-42s %3s %3s %3s  %-8s %s\n' "$cls" "$id" "$named" "$exec" "$ev" "$st" "$pol"; fi
  done <<EOF
$rows
EOF
  if [ "$json" = 1 ]; then printf '],"total":%s,"covered":%s,"partial":%s,"gaps_required":%s}\n' "$total" "$covered" "$partial" "$gaps"
  else printf 'usecase coverage: %s target(s), %s covered, %s partial, %s required gap(s)  (columns: use cases, executable, evidence)\n' "$total" "$covered" "$partial" "$gaps"; fi
  [ "$check" = 1 ] && [ "$gaps" -gt 0 ] && return "$MJ_EX_CONTRACT"
  return 0
}

# the doctor validator: the same tally, as findings, gated by the policy
mj_validate_use_case_coverage() {
  local rows cls id named exec ev st pol gaps=0 total=0 covered=0 advisory=""
  mj_uc_load
  [ -n "$MJ_UC_DIR" ] && [ -d "$MJ_UC_DIR" ] || { mj_doctrine_skip use-case "-" "no use-cases section in the manifest; nothing to cover"; MJ_DOCTRINE_SKIPPED=1; return 0; }
  rows="$(mj_uc_coverage_rows)"
  while IFS=$'\t' read -r cls id named exec ev st; do
    [ -n "$cls" ] || continue
    total=$((total+1))
    case "$cls" in command) pol="$(mj_uc_policy commands)" ;; claim) pol="$(mj_uc_policy claims)" ;; mcp_tool) pol="$(mj_uc_policy mcp_tools)" ;; esac
    if [ "$st" = covered ]; then covered=$((covered+1)); continue; fi
    case "$pol" in
      required) mj_doctrine_fail use-case "$cls $id" "$st: $named use case(s) name it, $exec run it; the policy requires an executable use case" "majordomus usecase scaffold --for $cls:$id"; gaps=$((gaps+1)) ;;
      advisory) advisory="$advisory $cls:$id" ;;
    esac
  done <<EOF
$rows
EOF
  [ -z "$advisory" ] || mj_info use-case "$(printf '%s\n' $advisory | wc -l | tr -d ' ') advisory gap(s)" "not covered by an active use case, and the policy does not require it:$advisory" "majordomus usecase coverage"
  [ "$gaps" = 0 ] && mj_doctrine_ok use-case "$covered of $total target(s) covered" "every public command the policy requires is named and run by an active use case (policy use_cases.coverage)"
  return 0
}

# ---------------------------------------------------------------- impact
mj_uc_cmd_impact() {
  local base="" json="${MJ_JSON:-0}" files f cmds="" rules="" ucs="" cases="" i id c r n first=1 claims_touched=0 all=0
  while [ $# -gt 0 ]; do case "$1" in --base) base="$2"; shift 2 ;; --json) json=1; shift ;; --help|-h) mj_uc_usage; return 0 ;; *) mj_die "$MJ_EX_USAGE" "usecase impact: unknown option $1" ;; esac; done
  mj_uc_require
  [ -n "$base" ] || base="$(mj_git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null || true)"
  [ -n "$base" ] || base="HEAD"
  files="$( { mj_git diff --name-only "$base" 2>/dev/null; mj_git status --porcelain 2>/dev/null | cut -c4- | sed 's/^.* -> //'; } | sort -u )"
  mj_cmdreg_load || true
  for f in $files; do
    case "$f" in
      bin/majordomus|lib/common.sh|share/commands.yaml|share/allow/*|.ai/manifest.yaml|.ai/repo/policy.yaml) all=1 ;;
      lib/*.sh) c="$(basename "$f" .sh)"; c="${c//_/-}"; printf '%s\n' "$(mj_dispatched)" | grep -qx "$c" && cmds="$cmds $c" ;;
      .ai/repo/rules/*.md) r="$(mj_record_front "$MJ_ROOT/$f" 2>/dev/null | awk -F': ' '$1=="id"{print $2}')"; [ -n "$r" ] && rules="$rules $r" ;;
      .ai/repo/use-cases/*.md) ucs="$ucs $(basename "$f" .md)" ;;
      .ai/repo/applications/*.md) ;;
      test/fixtures/commands/setup/*.sh) n="$(basename "$f" .sh)"; i=0; while [ "$i" -lt "$MJ_UC_N" ]; do [ "$(mj_uc_v "$i" scenario.setup)" = "$n" ] && ucs="$ucs $(mj_uc_v "$i" id)"; i=$((i+1)); done ;;
      test/fixtures/commands/stdin/*) n="$(basename "$f")"; i=0; while [ "$i" -lt "$MJ_UC_N" ]; do grep -q "stdin: $n\$" "$(mj_uc_file "$i")" && ucs="$ucs $(mj_uc_v "$i" id)"; i=$((i+1)); done ;;
      docs/CLAIMS.yaml) claims_touched=1 ;;
      apps/majordomus-cli/*) i=0; while [ "$i" -lt "$MJ_UC_N" ]; do [ -n "$(mj_uc_v "$i" mcp_tools.0)" ] && ucs="$ucs $(mj_uc_v "$i" id)"; i=$((i+1)); done ;;
    esac
  done
  # responsibilities map files to commands (docs/RESPONSIBILITIES.yaml)
  if [ -f "$MJ_ROOT/docs/RESPONSIBILITIES.yaml" ]; then
    local rf; rf="$(mktemp "${TMPDIR:-/tmp}/mj.resp.XXXXXX")"; mj_yaml_flatten "$MJ_ROOT/docs/RESPONSIBILITIES.yaml" > "$rf" 2>/dev/null || true
    for f in $files; do
      for c in $(awk -F= -v f="$f" '$0 ~ /^responsibilities\.[0-9]+\.(files\.[0-9]+|implementation)=/ && substr($0, index($0,"=")+1) == f { split($1,k,"."); print k[2] }' "$rf" | sort -u); do
        r="$(mj_yget "$rf" "responsibilities.$c.command")"; [ -n "$r" ] && [ "$r" != none ] && cmds="$cmds $r"
      done
    done
    rm -f "$rf"
  fi
  if [ "$all" = 1 ]; then cmds="$(mj_cmdreg_public | tr '\n' ' ')"; fi
  # commands and rules to use cases
  i=0
  while [ "$i" -lt "$MJ_UC_N" ]; do
    id="$(mj_uc_v "$i" id)"
    for c in $cmds; do mj_uc_list "$i" commands | grep -qx "$c" && ucs="$ucs $id"; done
    for r in $rules; do mj_uc_list "$i" doctrines | grep -qx "$r" && ucs="$ucs $id"; done
    [ "$claims_touched" = 1 ] && [ -n "$(mj_uc_v "$i" claims.0)" ] && ucs="$ucs $id"
    i=$((i+1))
  done
  cmds="$(printf '%s\n' $cmds | sort -u | tr '\n' ' ')"; rules="$(printf '%s\n' $rules | sort -u | tr '\n' ' ')"; ucs="$(printf '%s\n' $ucs | sort -u | tr '\n' ' ')"
  cmds="${cmds% }"; rules="${rules% }"; ucs="${ucs% }"
  # behavioural cases that declare coverage of an affected command, and the rules' tests
  for c in $cmds; do cases="$cases $(grep -lE "^# majordomus-covers:.*\b$c\b" "$MJ_ROOT"/test/cases/*.sh 2>/dev/null | sed "s#^$MJ_ROOT/##" | tr '\n' ' ')"; done
  for r in $rules; do n="$(mj_doc_index "$r" 2>/dev/null)" && cases="$cases $(mj_doc_list "$n" tests | tr '\n' ' ')"; done
  cases="$(printf '%s\n' $cases | sort -u | tr '\n' ' ')"; cases="${cases% }"
  local scen=""; for id in $ucs; do i="$(mj_uc_index "$id")" && mj_uc_has_scenario "$i" && scen="$scen $id"; done; scen="${scen# }"
  if [ "$json" = 1 ]; then
    printf '{"schema":"majordomus/use-case-impact/v1","base":"%s","files":%s,"commands":%s,"rules":%s,"use_cases":%s,"scenarios":%s,"cases":%s}\n' \
      "$(mj_json_esc "$base")" "$(printf '%s\n' $files | mj_uc_jarr)" "$(printf '%s\n' $cmds | mj_uc_jarr)" "$(printf '%s\n' $rules | mj_uc_jarr)" \
      "$(printf '%s\n' $ucs | mj_uc_jarr)" "$(printf '%s\n' $scen | mj_uc_jarr)" "$(printf '%s\n' $cases | mj_uc_jarr)"
    return 0
  fi
  printf 'impact since %s: %s file(s) changed\n' "$base" "$(printf '%s\n' $files | grep -c . || true)"
  printf '  commands   %s\n' "${cmds:-none}"
  printf '  rules      %s\n' "${rules:-none}"
  printf '  use cases  %s\n' "${ucs:-none}"
  printf '  scenarios  %s\n' "${scen:-none}"
  printf '  cases      %s\n' "${cases:-none}"
  [ -z "$scen" ] || printf 'next: majordomus usecase run%s\n' "$(printf ' %s' $scen)"
  return 0
}

# ---------------------------------------------------------------- scaffold
mj_uc_cmd_scaffold() {
  local missing=0 for_="" dry=0 c targets="" written=0 f stage cat fx setup run0 exp resp_cmd claims_of
  while [ $# -gt 0 ]; do case "$1" in --missing) missing=1; shift ;; --for) for_="$2"; shift 2 ;; --dry-run) dry=1; shift ;; --help|-h) mj_uc_usage; return 0 ;; *) mj_die "$MJ_EX_USAGE" "usecase scaffold: unknown option $1" ;; esac; done
  mj_uc_require
  mj_cmdreg_load || mj_die "$MJ_EX_MISSING" "no command registry"
  if [ -n "$for_" ]; then case "$for_" in command:*) targets="${for_#command:}" ;; *) mj_die "$MJ_EX_USAGE" "usecase scaffold: --for takes command:<name>" ;; esac
  elif [ "$missing" = 1 ]; then
    targets="$(mj_uc_coverage_rows | awk -F'\t' '$1=="command" && $6=="gap" {print $2}' | tr '\n' ' ')"
  else mj_die "$MJ_EX_USAGE" "usecase scaffold: say --missing, or --for command:<name>"; fi
  [ -n "$targets" ] || { printf 'usecase scaffold: nothing missing\n'; return 0; }
  for c in $targets; do
    f="$MJ_UC_DIR/$c-draft.md"
    [ -f "$f" ] && { printf 'exists: %s\n' "$(mj_rel "$f")"; continue; }
    mj_uc_index "$c-draft" >/dev/null 2>&1 && continue
    stage="$(awk -F= -v c="$c" '/^commands\.[0-9]+\.id=/ && substr($0,index($0,"=")+1)==c { split($1,k,"."); i=k[2] } /^commands\.[0-9]+\.stage=/ { split($1,k,"."); if (k[2]==i) print substr($0,index($0,"=")+1) }' "$MJ_CMDREG_FLAT")"
    case "$stage" in setup) cat=adoption ;; begin) cat=workers ;; work) cat=continuity ;; verify) cat=policy ;; conclude) cat=completion ;; inspect) cat=knowledge ;; *) cat=knowledge ;; esac
    resp_cmd="$(awk -F= -v c="$c" '/^responsibilities\.[0-9]+\.command=/ && substr($0,index($0,"=")+1)==c { split($1,k,"."); i=k[2] } /^responsibilities\.[0-9]+\.id=/ { split($1,k,"."); if (k[2]==i) print substr($0,index($0,"=")+1) }' <(mj_yaml_flatten "$MJ_ROOT/docs/RESPONSIBILITIES.yaml" 2>/dev/null) | head -1)"
    claims_of=""; [ -n "$resp_cmd" ] && claims_of="$(awk -v r="$resp_cmd" '/^  - id: /{id=$3} /^    status: /{st=$2} /^    responsibility: /{if ($2==r && st=="guaranteed") print id}' "$MJ_ROOT/docs/CLAIMS.yaml" | head -4 | tr '\n' ' ')"
    fx="$(mj_uc_fixture_dir)/$c.json"; setup=bare; run0="$c"; exp=0
    if [ -f "$fx" ] && command -v jq >/dev/null 2>&1; then
      setup="$(jq -r '.scenarios[0].setup' "$fx")"; exp="$(jq -r '.scenarios[0].expect.exit' "$fx")"
      run0="$(jq -r --arg q "'" '.scenarios[0].run | map($q + . + $q) | join(", ")' "$fx")"
    else run0="'$c'"; fi
    if [ "$dry" = 1 ]; then printf 'would write: %s (category %s, setup %s)\n' "$(mj_rel "$f")" "$cat" "$setup"; continue; fi
    {
      printf -- '---\nid: %s-draft\nkind: use-case\ntitle: %s\nsummary: %s\ncategory: %s\nstatus: draft\ntarget: advisory\nweight: 100\nactors: [maintainer]\ndifficulty: basic\ncommands: [%s]\n' \
        "$c" "'TODO: the task \`$c\` answers'" "'TODO: one sentence; scaffolded from the registry, the command fixture and the claims, nothing here is verified'" "$cat" "$c"
      printf 'doctrines: []\nclaims: [%s]\nresponsibilities: [%s]\napplications: []\nscenario:\n  setup: %s\n  given:\n    - %s\n  steps:\n    - id: first\n      run: [%s]\n      expect:\n        exit: %s\n  then:\n    - %s\n---\n\n# Situation\n\nTODO\n\n# Outcome\n\nTODO\n' \
        "$(printf '%s' "$claims_of" | sed 's/ *$//; s/ /, /g')" "$resp_cmd" "$setup" "'TODO: the state the setup script prepares'" "$run0" "$exp" "'TODO: what is true afterwards'"
    } > "$f"
    written=$((written+1)); printf 'wrote: %s (draft; complete the narrative and the assertions, then set status: active)\n' "$(mj_rel "$f")"
  done
  printf 'usecase scaffold: %s draft(s) written\n' "$written"
  return 0
}
