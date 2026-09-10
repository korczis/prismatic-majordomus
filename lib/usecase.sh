#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
# usecase — the executable use cases of this repository: the canonical objects under the
# manifest's `use-cases` section, listed, shown, validated, run against the real tool with
# the evidence recorded, tallied against every public command, guaranteed claim and MCP
# tool, traced from a change to what it affects, and scaffolded where coverage is missing.
#
# A use case is a Markdown file with front matter (share/schemas/majordomus/use-case/use-case.v1.schema.json): a
# task a person performs, the commands, rules, claims, responsibilities and applications
# it names, and a scenario: real invocations of bin/majordomus with their expected exit
# codes and output. Nothing here is prose about the tool; a reference that does not resolve
# or a step that does not behave is a failure with the file and the step named.
#
# A scenario declares where it runs (ADR 38). `mode: fixture`, the default, prepares a
# fresh repository with a setup script and proves the tool behaves: its evidence is
# derived, committed and reproduced by CI. `mode: live` asks its questions of the
# repository the command was invoked in, may run only commands share/commands.yaml
# declares `class: read-only`, and may assert an obligation instead of running anything —
# a step that is discharged, unmet or stale rather than passing or failing. A live
# scenario is how a class of work states what it owes; `check` is how one task's own
# promises are judged, and both read the same judgement in lib/evidence.sh.
#
# Evidence is written under the local half (.ai/local/evidence/use-cases/<id>.json), never
# under the tracked tree; the site generator runs the scenarios itself and embeds the
# normalised output, so a page shows what the tool did, and `--check` proves it again.

MJ_UC_DIR=""; MJ_AP_DIR=""; MJ_UC_IDS=""; MJ_UC_N=0; MJ_UC_LOADED=0
MJ_UC_EVIDENCE=""
# the command registry, for the public commands coverage is counted over, and for the
# class a live scenario's steps are held to
# shellcheck source=commands.sh
. "$MJ_LIB_DIR/commands.sh"
# the obligation vocabulary and the one judgement of whether an obligation is discharged;
# a live scenario asserts obligations and must reach the same answer check reaches
# shellcheck source=evidence.sh
. "$MJ_LIB_DIR/evidence.sh"

mj_uc_usage() {
  cat <<'USAGE'
usage: majordomus usecase list [--json]
       majordomus usecase show <id>
       majordomus usecase validate [--json]
       majordomus usecase run [<id>...] [--json] [--out <dir>] [--keep] [--live]
       majordomus usecase coverage [--json] [--check]
       majordomus usecase impact [--base <ref>] [--json]
       majordomus usecase scaffold [--missing] [--for command:<name>] [--dry-run]
  list      every use case: id, category, status, the commands it runs, whether it has a scenario
  show      one use case: its front matter and body, and the evidence of its last run when there is one
  validate  every reference resolves (commands, doctrines, claims, responsibilities, applications,
            categories, setup scripts, stdin files), ids are unique and match their file, the body
            carries its sections; exit 10 on any failure
  run       execute the scenarios against bin/majordomus and assert every step: a fixture scenario
            in a disposable repository, evidence under .ai/local/evidence/use-cases/; a live one
            against this repository, read-only, evidence under .ai/local/evidence/live/ and never
            committed. Fixtures run by default; --live adds the live ones, and naming an id runs it
            whatever its mode. Exit 10 when a step fails or an obligation is unmet
  coverage  every public command, guaranteed claim and MCP tool against the use cases that name and
            run it in a fixture; --check exits 10 on a gap the policy makes required
            (policy: use_cases.coverage)
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

# The `# Scenario` section's fenced YAML block, verbatim, or nothing.
#
# The scenario is the one part of a use case that is executed rather than read, and it
# used to live in the front matter. It does not any more: front matter is what the object
# *is* — its identity and its classification — and a twenty-line program is neither. A
# reader opening the file now sees the narrative and the proof in the order they happen,
# and the block stays YAML because it is still run.
mj_uc_scenario_yaml() {
  awk '
    /^# Scenario[ \t]*$/ { sec = 1; next }
    sec && /^```yaml[ \t]*$/ { fence = 1; next }
    sec && fence && /^```[ \t]*$/ { exit }
    sec && fence { print; next }
    sec && /^# / { exit }
  ' "$1"
}

# the front matter of a Markdown object, flattened into a temp file, with the scenario
# section folded in under the `scenario.` prefix so that every reader below sees one flat
# namespace and neither knows nor cares which half of the file a key came from;
# empty on failure
mj_uc_flat() {
  local f="$1" front tmp scenario
  front="$(mj_record_front "$f")" || return 1
  [ -n "$front" ] || return 1
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ucf.XXXXXX")"
  printf '%s\n' "$front" > "$tmp.yaml"
  mj_yaml_flatten "$tmp.yaml" > "$tmp" 2>/dev/null || { rm -f "$tmp" "$tmp.yaml"; return 1; }
  scenario="$(mj_uc_scenario_yaml "$f")"
  if [ -n "$scenario" ]; then
    printf '%s\n' "$scenario" > "$tmp.yaml"
    # a scenario that does not parse leaves the use case scenario-less, which `check`
    # reports as a use case targeting a guarantee without evidence — never as a silence
    mj_yaml_flatten "$tmp.yaml" 2>/dev/null | sed 's/^/scenario./' >> "$tmp" || true
  fi
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
    acc=" "; while k="MJUC${1}__scenario__steps__${i}__id"; [ -n "${!k:-}" ]; do
      k="MJUC${1}__scenario__steps__${i}__run__0"; if [ -n "${!k:-}" ]; then acc="$acc${!k} "; fi; i=$((i+1)); done
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
# the commands a scenario runs, one per line, in order. The walk is over step ids, not
# over `run.0`: an obligation step has no command, and a loop that stopped at the first
# one would silently truncate the scenario at it.
mj_uc_scenario_commands() { local i="$1" k=0; while mj_uc_get "$i" "scenario.steps.$k.id"; [ -n "$MJ_V" ]; do mj_uc_get "$i" "scenario.steps.$k.run.0"; if [ -n "$MJ_V" ]; then printf '%s\n' "$MJ_V"; fi; k=$((k+1)); done; }
# a scenario is its steps. A fixture scenario also names a setup; a live one deliberately
# has none, so the setup cannot be what says a scenario is there.
mj_uc_has_scenario() { local k="MJUC${1}__scenario__steps__0__id"; [ -n "${!k:-}" ]; }
# Where a scenario runs (ADR 38): `fixture` in a disposable repository, the default and
# what CI reproduces; `live` in the repository the command was invoked in, read-only.
mj_uc_mode() { local k="MJUC${1}__scenario__mode"; printf '%s' "${!k:-fixture}"; }
mj_uc_is_live() { local k="MJUC${1}__scenario__mode"; [ "${!k:-fixture}" = live ]; }
# Whether this run includes this scenario. A bare `usecase run` runs the fixtures and only
# the fixtures, so what CI means by it does not change under a repository that adds a live
# scenario; `--live` adds them, and naming an id runs it whatever its mode.
mj_uc_selected() { # index named(0|1) want_live(0|1)
  mj_uc_is_live "$1" || return 0
  [ "$2" = 1 ] || [ "$3" = 1 ]
}
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
    # keys the schema does not declare: the allow-list is generated from the schema, and
    # covers the front matter only. The scenario the loader folded in under `scenario.`
    # came from the body, where the section list is what governs it, so it is dropped here
    # rather than measured against a list that deliberately no longer mentions it.
    flatn="MJ_UC_FLAT_$i"
    unk="$(grep -v '^scenario\.' "${!flatn}" 2>/dev/null | mj_yaml_unknown_keys /dev/stdin "$MJ_ALLOW_DIR/use-case.txt" || true)"
    [ -z "$unk" ] || mj_uc_bad "$id" "unknown key(s): $(printf '%s' "$unk" | tr '\n' ' ')" "share/schemas/majordomus/use-case/use-case.v1.proto"
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
    for k in "# Situation" "# Scenario" "# Outcome"; do case "$body" in *"$k"*) ;; *) mj_uc_bad "$id" "body has no '$k' heading" "" ;; esac; done
    # the scenario: setup exists, stdin bodies exist, every step names a declared command,
    # step ids are unique, every step expects an exit code
    if mj_uc_has_scenario "$i"; then
      local mode obl
      mode="$(mj_uc_mode "$i")"
      case "$mode" in
        fixture)
          mj_uc_get "$i" scenario.setup
          [ -n "$MJ_V" ] || mj_uc_bad "$id" "a fixture scenario names no setup; the repository it runs in has to be prepared" "scenario: mode: live, to ask about this repository instead"
          [ -z "$MJ_V" ] || [ -f "$fix/setup/$MJ_V.sh" ] || mj_uc_bad "$id" "scenario names setup '$MJ_V', which $(mj_rel "$fix")/setup/ does not have" ""
          ;;
        live)
          mj_uc_get "$i" scenario.setup
          [ -z "$MJ_V" ] || mj_uc_bad "$id" "a live scenario names setup '$MJ_V'; it runs in this repository and prepares nothing" "remove the setup, or drop mode: live"
          ;;
        *) mj_uc_bad "$id" "scenario mode '$mode' is not fixture or live" "ADR 38" ;;
      esac
      k=0; sids=""
      while mj_uc_get "$i" "scenario.steps.$k.id"; sid="$MJ_V"; [ -n "$sid" ]; do
        case " $sids " in *" $sid "*) mj_uc_bad "$id" "step '$sid' is declared twice" "" ;; esac; sids="$sids $sid"
        mj_uc_get "$i" "scenario.steps.$k.run.0"; cmd="$MJ_V"
        mj_uc_get "$i" "scenario.steps.$k.obligation"; obl="$MJ_V"
        if [ -n "$obl" ]; then
          # an obligation step asserts that work was done; it runs nothing and there is
          # nothing in a fixture to owe it
          [ -z "$cmd" ] || mj_uc_bad "$id" "step '$sid' both runs '$cmd' and asserts obligation '$obl'; a step is one or the other" ""
          [ "$mode" = live ] || mj_uc_bad "$id" "step '$sid' asserts obligation '$obl' in a fixture scenario; a disposable repository owes nothing" "mode: live"
          mj_obligations_load 2>/dev/null || true
          mj_obligation_known "$obl" || mj_uc_bad "$id" "step '$sid' asserts obligation '$obl', which share/obligations.yaml does not declare" "majordomus evidence --help"
        else
          [ -n "$cmd" ] || mj_uc_bad "$id" "step '$sid' runs nothing" ""
          case " $(printf '%s' "$cmd_list" | tr '\n' ' ') " in *" $cmd "*) ;; *) mj_uc_bad "$id" "step '$sid' runs '$cmd', which the use case does not list under commands" "" ;; esac
          mj_uc_get "$i" "scenario.steps.$k.expect.exit"; case "$MJ_V" in ''|*[!0-9]*) mj_uc_bad "$id" "step '$sid' expects no exit code" "" ;; esac
          mj_uc_get "$i" "scenario.steps.$k.stdin"; found="$MJ_V"
          [ -z "$found" ] || [ -f "$stdin_dir/$found" ] || mj_uc_bad "$id" "step '$sid' names stdin '$found', which $(mj_rel "$stdin_dir")/ does not have" ""
          # A live scenario is a question about this repository, never an action on it.
          # The class is read from share/commands.yaml, which already declares it, so
          # safety is a property of the declaration and not of the author's care (ADR 38).
          if [ "$mode" = live ] && [ -n "$cmd" ]; then
            mj_cmdreg_load || true
            local cls; cls="$(mj_cmdreg_class "$cmd")"
            [ "$cls" = read-only ] || mj_uc_bad "$id" "step '$sid' runs '$cmd', which is ${cls:-undeclared}, in a live scenario; only read-only commands may run against this repository" "share/commands.yaml"
          fi
        fi
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
# The owner is the operating system's user name: `start` records ${USER} when nobody names
# one, so the evidence would read `korczis` on one machine and `runner` on another, and the
# committed artifact generated from it differs by who generated it. That is the same defect
# the branch name had, with the same consequence — generate-site-data --check fails on the
# next machine — so it is normalised here rather than fixed in every scenario. All three
# renderings of the same field are covered: the aligned column a command prints, the
# flattened `owner=` a record dump shows, and the JSON member.
#
# A derived checkpoint or handover states the commit it describes in prose — "On main at
# 813a294", "the task opened at 813a294; git has moved to e655a41 since" — and a scenario
# that records one therefore records a hash that is different in every checkout. The rules
# above mask a short head where a command prints it in a column or after the word `head`;
# these are the shapes prose puts it in. The blockquote is one of them: a handover quotes
# the newest checkpoint whole, and that quotation carries the checkpoint's own commit behind
# a `> `. They are contextual for the same reason the rules above are: a bare seven hex
# digits also spells a plausible number, and masking every one of them would hide counts as
# well as commits.
#
# Two findings are decided by the clock rather than by the repository. `check` reports the
# checkpoint as fresh or stale by the time elapsed since the task's last checkpoint against
# the profile's interval, and `doctor` reports its own duration against a budget; both flip
# between OK and WARN with the load of the machine that recorded them, and each flip made
# `generate-site-data --check` call the catalogue stale for no change anyone made. Their
# lines are replaced whole, status included, and an age in minutes, hours or days becomes
# one token, since a scenario that ran slowly enough crosses from one unit into the next.
#
# The EPIPE diagnostic goes for the same reason. A reader that stops early closes the pipe
# under the writer, and bash reports the failed write on stderr, which the recorder captures
# along with everything else. Whether the race fires depends on the machine, so recording it
# makes the artifact differ by where it was generated. It is a fact about the recording, not
# about the command, and the pipes that produce it are removed where they are ours.
mj_uc_normalise() { # repo-path
  local real; real="$(cd "$1" 2>/dev/null && pwd -P)"
  sed -E \
    -e "s#$real#<repo>#g" \
    -e "s#$1#<repo>#g" \
    -e "s#$MJ_BIN_DIR/majordomus#majordomus#g" \
    -e "s#${HOME}#<home>#g" \
    -e 's/[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z/<time>/g' \
    -e 's/[0-9]{8}T[0-9]{6}Z/<time>/g' \
    -e 's/[0-9]{4}-[0-9]{2}-[0-9]{2}/<date>/g' \
    -e 's/t-[0-9]{14}-[0-9a-f]{4}/t-<id>/g' \
    -e 's/s-[0-9]+-[0-9a-f]+/s-<id>/g' \
    -e 's/--[0-9a-f]{7}--[0-9a-f]{16}\.md/--<head>--<hash>.md/g' \
    -e 's/[0-9a-f]{40}/<sha>/g' \
    -e 's/[0-9a-f]{16}/<hash16>/g' \
    -e 's/(policy|match the last update \(|inputs )[0-9a-f]{12}/\1<hash12>/g' \
    -e 's/(head +)[0-9a-f]{7}/\1<head>/g' \
    -e 's/\(head [0-9a-f]{7}\)/(head <head>)/g' \
    -e 's/(  +)[0-9a-f]{7}(  |$)/\1<head>\2/g' \
    -e 's/( at | moved to )[0-9a-f]{7}([,;. ]|$)/\1<head>\2/g' \
    -e 's/^(> )?At [0-9a-f]{7}([,;. ]|$)/\1At <head>\2/g' \
    -e 's/^([a-z_-]+ +(cold|warm) +[a-z]+ +[0-9]+) +[0-9]+ +[0-9]+ +[0-9]+ +[0-9]+/\1  <ms>  <ms>  <ms>  <ms>/' \
    -e 's/^(INFO|WARN) +budget +([a-z]+) — .*$/·    budget      \2 — <timed against the policy budget>/' \
    -e 's/^(OK|WARN|FAIL) +checkpoint +([^ ]+) — .*$/·    checkpoint  \2 — <timed against the checkpoint interval>/' \
    -e 's/(exit [0-9]+, )[0-9]+s$/\1<s>s/' \
    -e 's/[0-9]+ ms/<n> ms/g' \
    -e 's/[0-9]+ ms of/<n> ms of/g' \
    -e 's/\([0-9]+[mhd] ago/(<age> ago/g' \
    -e 's/ [0-9]+[mhd] ago/ <age> ago/g' \
    -e 's/(bash|git|jq|shellcheck) [0-9][0-9.]*/\1 <version>/g' \
    -e 's/^(owner +).*$/\1<owner>/' \
    -e 's/^( *owner=).*$/\1<owner>/' \
    -e 's/"owner":"[^"]*"/"owner":"<owner>"/g' \
    -e '/: printf: write error: Broken pipe$/d'
}
# a JSON string body: backslash and quote escaped, newlines and tabs as escapes, every
# other control byte dropped; the newlines of a command's output are its structure
mj_uc_jesc() { printf '%s' "$1" | awk 'BEGIN{ORS=""} { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); gsub(/[\001-\010\013-\037]/, ""); if (NR > 1) printf "\\n"; printf "%s", $0 }'; }

# run one use case's scenario; prints the evidence JSON to the file named; returns 0 on
# pass, 1 on a failed step, 2 when the use case has no scenario
mj_uc_run_one() { # index, evidence-file, keep(0|1)
  local i="$1" out="$2" keep="${3:-0}" id setup tmp W k sid argv_n a rc want pat ok all_ok=1 t0 t1 dur stdin_f
  local fix steps_json="" first=1 raw norm asserts fail_reason
  id="$(mj_uc_v "$i" id)"; setup="$(mj_uc_v "$i" scenario.setup)"
  [ -n "$setup" ] || return 2
  fix="$(mj_uc_fixture_dir)"
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj-uc.XXXXXX")"
  W="$tmp/repo"; mkdir -p "$W"
  # The scenario's repository is created on a fixed branch. `git init` alone takes the branch
  # from the operator's init.defaultBranch, and the branch name reaches the evidence through
  # every record path a scenario writes (a checkpoint and a handover are named after it), so
  # the recorded evidence — a committed derived artifact — would otherwise differ between a
  # machine configured for `main` and one left on git's built-in default. That is a generated
  # file whose content depends on who generated it, which is the one thing derivation may not
  # do; `scripts/generate-site-data --check` fails on the next machine and names the line.
  # symbolic-ref rather than `git init -b`, which needs git 2.28.
  ( cd "$W" && git init -q . && git symbolic-ref HEAD refs/heads/main \
      && git config user.email t@example.com && git config user.name t \
      && git commit -q --allow-empty -m init ) || { rm -rf "$tmp"; mj_err "usecase run: cannot create a repository for $id"; return 1; }
  # the setup script prepares the repository, with the same helpers the test suite gives it
  local setup_out="$tmp/setup.out"
  # the helpers the setup scripts use (pj_* for a plan model) come from the tool's test
  # library, which the distribution ships beside the fixtures
  local helpers="$fix/../../lib.sh"
  # The tool under test is on PATH for the setup and every step, as an installed launcher
  # would be. A provider hook the setup installs resolves the executable the way the
  # provider would — the repository's own bin/, then PATH — and the scenario's repository has
  # no bin/ of its own, so what it finds is whatever PATH holds. On a machine where
  # `majordomus` is on PATH the recorded state is `verified`; on a runner where it is not,
  # `wired`; and that difference reached the committed evidence, which is the one thing
  # derivation may not do. The tool being exercised is the one the hook should find.
  case ":$PATH:" in *":$MJ_BIN_DIR:"*) ;; *) PATH="$MJ_BIN_DIR:$PATH"; export PATH ;; esac
  ( cd "$W" && MJ="$MJ_BIN_DIR/majordomus" FIXTURE_SETUP="$fix/setup" ROOT="$MJ_ROOT" && export MJ FIXTURE_SETUP ROOT \
      && if [ -f "$helpers" ]; then # shellcheck disable=SC1090
        . "$helpers"; fi \
      && { # shellcheck disable=SC1090
        . "$fix/setup/$setup.sh"; } ) > "$setup_out" 2>&1 || {
    printf '{"schema":"majordomus/use-case-evidence/v1","use_case":"%s","setup":"%s","result":"fail","reason":"setup failed","setup_output":"%s","steps":[]}\n' \
      "$id" "$setup" "$(mj_uc_jesc "$(mj_uc_normalise "$W" < "$setup_out" | head -c 4000)")" > "$out"
    [ "$keep" = 1 ] || rm -rf "$tmp"; return 1; }
  k=0
  while sid="$(mj_uc_v "$i" "scenario.steps.$k.id")"; [ -n "$sid" ]; do
    set --; argv_n=0
    while a="$(mj_uc_v "$i" "scenario.steps.$k.run.$argv_n")"; [ -n "$a" ]; do set -- "$@" "$a"; argv_n=$((argv_n+1)); done
    stdin_f="$(mj_uc_v "$i" "scenario.steps.$k.stdin")"
    want="$(mj_uc_v "$i" "scenario.steps.$k.expect.exit")"
    raw="$tmp/step-$k.out"; rc=0; t0="$(mj_ms)"
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
    steps_json="$steps_json{\"id\":\"$(mj_json_esc "$sid")\",\"command\":\"$(mj_json_esc "majordomus $*")\",\"argv\":$(printf '%s\n' "$@" | mj_uc_jarr),\"stdin\":$( [ -n "$stdin_f" ] && printf '"%s"' "$(mj_json_esc "$stdin_f")" || printf null ),\"exit\":$rc,\"expected_exit\":$want,\"output\":\"$(mj_uc_jesc "$(printf '%s' "$norm" | head -c 12000)")\",\"assertions\":[${asserts%,}],\"result\":\"$([ "$ok" = 1 ] && printf pass || printf fail)\",\"reason\":$( [ -n "$fail_reason" ] && printf '"%s"' "$(mj_json_esc "$fail_reason")" || printf null ),\"timing\":{\"duration_ms\":$dur}}"
    [ "$ok" = 1 ] || { all_ok=0; break; }
    k=$((k+1))
  done
  printf '{"schema":"majordomus/use-case-evidence/v1","use_case":"%s","setup":"%s","result":"%s","steps":[%s]}\n' \
    "$id" "$setup" "$([ "$all_ok" = 1 ] && printf pass || printf fail)" "$steps_json" > "$out"
  if [ "$keep" = 1 ]; then printf 'kept: %s\n' "$W" >&2; else rm -rf "$tmp"; fi
  [ "$all_ok" = 1 ]
}

# ---------------------------------------------------------------- the live runner
# A live scenario asks its questions of the repository the command was invoked in. It
# creates nothing, prepares nothing and — because `usecase validate` refuses a step whose
# command is not `class: read-only` in share/commands.yaml — changes nothing.
#
# Unlike a fixture scenario it does not stop at the first failure. A fixture's steps are a
# sequence: step three runs in the state step two left, so continuing past a failure would
# assert against a repository that never reached the described state. A live scenario's
# steps are independent questions about one tree, and a gate that answered only the first
# of them would send a worker round the loop once per finding.
mj_uc_run_live() { # index, evidence-file
  local i="$1" out="$2" id W k sid cmd obl argv_n a rc want pat ok all_ok=1 t0 t1 dur
  local steps_json="" first=1 raw norm asserts fail_reason n tmp j jstate jrest jmsg jrep task
  id="$(mj_uc_v "$i" id)"; W="$MJ_ROOT"
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj-uclive.XXXXXX")"
  # the task an obligation step is asked about; a live scenario with obligation steps and
  # no active task reports them unmet, which is the honest answer and not an error
  task=""; mj_load_current 2>/dev/null && task="$(mj_cur id)" || task=""
  k=0
  while sid="$(mj_uc_v "$i" "scenario.steps.$k.id")"; [ -n "$sid" ]; do
    obl="$(mj_uc_v "$i" "scenario.steps.$k.obligation")"
    ok=1; asserts=""; fail_reason=""
    if [ -n "$obl" ]; then
      if [ -z "$task" ]; then
        ok=0; jstate=unmet; jmsg="no task is active here, so nothing owes '$obl'"; jrep="majordomus start"
      else
        j="$(mj_obligation_judge "$task" "$obl")" || true
        jstate="${j%%"$MJ_TAB"*}"; jrest="${j#*"$MJ_TAB"}"
        jmsg="${jrest%%"$MJ_TAB"*}"; jrep="${jrest#*"$MJ_TAB"}"; if [ "$jrep" = "$jrest" ]; then jrep=""; fi
        [ "$jstate" = pass ] || ok=0
      fi
      [ "$ok" = 1 ] || fail_reason="$jmsg"
      asserts="{\"kind\":\"obligation\",\"token\":\"$(mj_json_esc "$obl")\",\"state\":\"$jstate\",\"result\":\"$([ "$ok" = 1 ] && printf pass || printf "$jstate")\"}"
      [ "$first" = 1 ] || steps_json="$steps_json,"; first=0
      steps_json="$steps_json{\"id\":\"$(mj_json_esc "$sid")\",\"obligation\":\"$(mj_json_esc "$obl")\",\"state\":\"$jstate\",\"message\":\"$(mj_json_esc "$jmsg")\",\"reproduce\":$( [ -n "$jrep" ] && printf '"%s"' "$(mj_json_esc "$jrep")" || printf null ),\"assertions\":[$asserts],\"result\":\"$([ "$ok" = 1 ] && printf pass || printf "$jstate")\",\"reason\":$( [ -n "$fail_reason" ] && printf '"%s"' "$(mj_json_esc "$fail_reason")" || printf null )}"
      [ "$ok" = 1 ] || all_ok=0
      k=$((k+1)); continue
    fi
    set --; argv_n=0
    while a="$(mj_uc_v "$i" "scenario.steps.$k.run.$argv_n")"; [ -n "$a" ]; do set -- "$@" "$a"; argv_n=$((argv_n+1)); done
    want="$(mj_uc_v "$i" "scenario.steps.$k.expect.exit")"
    raw="$tmp/step-$k.out"; rc=0; t0="$(mj_ms)"
    ( cd "$W" && "$MJ_BIN_DIR/majordomus" "$@" < /dev/null ) > "$raw" 2>&1 || rc=$?
    t1="$(mj_ms)"; dur=$((t1 - t0))
    norm="$(mj_uc_normalise "$W" < "$raw")"
    if [ "$rc" != "$want" ]; then ok=0; fail_reason="expected exit $want, got $rc"; fi
    asserts="{\"kind\":\"exit\",\"expected\":$want,\"observed\":$rc,\"result\":\"$([ "$rc" = "$want" ] && printf pass || printf fail)\"},"
    n=0
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
    [ "$first" = 1 ] || steps_json="$steps_json,"; first=0
    steps_json="$steps_json{\"id\":\"$(mj_json_esc "$sid")\",\"command\":\"$(mj_json_esc "majordomus $*")\",\"argv\":$(printf '%s\n' "$@" | mj_uc_jarr),\"exit\":$rc,\"expected_exit\":$want,\"output\":\"$(mj_uc_jesc "$(printf '%s' "$norm" | head -c 12000)")\",\"assertions\":[${asserts%,}],\"result\":\"$([ "$ok" = 1 ] && printf pass || printf unmet)\",\"reason\":$( [ -n "$fail_reason" ] && printf '"%s"' "$(mj_json_esc "$fail_reason")" || printf null ),\"timing\":{\"duration_ms\":$dur}}"
    [ "$ok" = 1 ] || all_ok=0
    k=$((k+1))
  done
  printf '{"schema":"majordomus/use-case-evidence/v1","mode":"live","use_case":"%s","repository":"%s","head":"%s","result":"%s","steps":[%s]}\n' \
    "$id" "$(mj_json_esc "$(mj_rel "$W")")" "$(mj_git_head 2>/dev/null || printf unknown)" "$([ "$all_ok" = 1 ] && printf pass || printf unmet)" "$steps_json" > "$out"
  rm -rf "$tmp"
  [ "$all_ok" = 1 ] || return "$MJ_EX_CONTRACT"
  return 0
}

mj_uc_cmd_run() {
  local json="${MJ_JSON:-0}" outdir="" keep=0 ids="" named=0 want_live=0 i id ev rc fails=0 unmets=0 lived=0 ran=0 skipped=0 first=1 jobs=0 maxjobs="${MJ_UC_JOBS:-8}"
  while [ $# -gt 0 ]; do case "$1" in
    --json) json=1; shift ;; --out) outdir="$2"; shift 2 ;; --keep) keep=1; shift ;; --jobs) maxjobs="$2"; shift 2 ;;
    --live) want_live=1; shift ;;
    --help|-h) mj_uc_usage; return 0 ;; -*) mj_die "$MJ_EX_USAGE" "usecase run: unknown option $1" ;;
    *) ids="$ids $1"; named=1; shift ;; esac; done
  mj_uc_require
  mkdir -p "$MJ_UC_EVIDENCE"
  # Live evidence is never committed: it describes one tree at one minute on one machine,
  # and a derived file whose content depends on who derived it is what ADR 5 forbids. It
  # lands in the local half, which .gitignore already covers.
  MJ_UC_LIVE_EVIDENCE="$MJ_AI_LOCAL_DIR/evidence/live"; mkdir -p "$MJ_UC_LIVE_EVIDENCE"
  [ -z "$outdir" ] || mkdir -p "$outdir"
  [ -n "$ids" ] || ids="$MJ_UC_IDS"
  for id in $ids; do mj_uc_index "$id" >/dev/null || mj_die "$MJ_EX_MISSING" "no use case '$id' (majordomus usecase list)"; done
  # the scenarios are independent (a repository each), so they run a few at a time; the
  # report is printed in list order once every one has finished, so the output is stable
  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj-ucrc.XXXXXX")"
  for id in $ids; do
    i="$(mj_uc_index "$id")"
    mj_uc_has_scenario "$i" || continue
    mj_uc_selected "$i" "$named" "$want_live" || continue
    if mj_uc_is_live "$i"; then continue; fi
    ( mj_uc_run_one "$i" "$MJ_UC_EVIDENCE/$id.json" "$keep"; echo $? > "$tmp/$id" ) &
    jobs=$((jobs+1))
    if [ "$jobs" -ge "$maxjobs" ]; then wait -n 2>/dev/null || wait; jobs=$((jobs-1)); fi
  done
  wait
  # The live scenarios run afterwards and one at a time. They share a single repository, so
  # there is nothing to gain from running them together, and their output is easier to read
  # in the order the list gives.
  for id in $ids; do
    i="$(mj_uc_index "$id")"
    mj_uc_has_scenario "$i" || continue
    mj_uc_is_live "$i" || continue
    mj_uc_selected "$i" "$named" "$want_live" || continue
    rc=0; mj_uc_run_live "$i" "$MJ_UC_LIVE_EVIDENCE/$id.json" || rc=$?
    echo "$rc" > "$tmp/$id"; lived=$((lived+1))
  done
  [ "$json" = 1 ] && printf '{"schema":"majordomus/use-case-run/v1","results":['
  for id in $ids; do
    i="$(mj_uc_index "$id")"
    if ! mj_uc_has_scenario "$i"; then
      skipped=$((skipped+1))
      [ "$json" = 1 ] || printf '%-38s described (no scenario)\n' "$id"
      continue
    fi
    if ! mj_uc_selected "$i" "$named" "$want_live"; then
      skipped=$((skipped+1))
      [ "$json" = 1 ] || printf '%-38s live (run it with --live)\n' "$id"
      continue
    fi
    if mj_uc_is_live "$i"; then ev="$MJ_UC_LIVE_EVIDENCE/$id.json"; else ev="$MJ_UC_EVIDENCE/$id.json"; fi
    rc="$(cat "$tmp/$id" 2>/dev/null || echo 13)"
    ran=$((ran+1))
    # A scenario's verdict is a fact about the run, not about how it is being printed.
    # Counting failures inside the text branch left `--json` reporting "failed":0 and
    # exiting 0 however the scenarios went, which made the exit code generate-site-data
    # relies on to refuse a broken demonstration permanently green.
    # A live scenario that exits 10 is reporting work not done, which is not a defect in
    # the tool and must not be counted or printed as one (ADR 38).
    if [ "$rc" = 0 ]; then :
    elif [ "$rc" = "$MJ_EX_CONTRACT" ] && mj_uc_is_live "$i"; then unmets=$((unmets+1))
    else fails=$((fails+1)); fi
    [ -z "$outdir" ] || cp "$ev" "$outdir/$id.json"
    if [ "$json" = 1 ]; then
      [ "$first" = 1 ] || printf ','; first=0
      # A scenario killed before it wrote evidence must not vanish from the document: an
      # absent result reads downstream as "not demonstrated" and silently lowers a
      # maturity, which is the difference between not knowing and knowing it is bad.
      if [ -s "$ev" ]; then tr -d '\n' < "$ev"
      else printf '{"use_case":"%s","result":"fail","reason":"the scenario wrote no evidence (exit %s)"}' "$id" "$rc"; fi
    else
      if [ "$rc" = 0 ]; then printf '%-38s pass  %s step(s)\n' "$id" "$(grep -o '"id":"' "$ev" | wc -l | tr -d ' ')"
      elif [ "$rc" = "$MJ_EX_CONTRACT" ] && mj_uc_is_live "$i"; then
        printf '%-38s UNMET %s\n' "$id" "$(grep -o '"reason":"[^"]*"' "$ev" | grep -v 'null' | head -1 | cut -d'"' -f4)"
        grep -o '"reason":"[^"]*"' "$ev" | grep -v 'null' | cut -d'"' -f4 | sed 's/^/      | /' | head -20
      else printf '%-38s FAIL  %s\n' "$id" "$(grep -o '"reason":"[^"]*"' "$ev" | grep -v 'null' | head -1 | cut -d'"' -f4)"
        grep -o '"output":"[^"]*"' "$ev" | tail -1 | cut -d'"' -f4 | sed 's/\\n/\n/g' | sed 's/^/      | /' | head -20; fi
    fi
  done
  rm -rf "$tmp"
  if [ "$json" = 1 ]; then printf '],"ran":%s,"failed":%s,"unmet":%s,"skipped":%s}\n' "$ran" "$fails" "$unmets" "$skipped"
  else printf 'usecase run: %s scenario(s), %s failed, %s unmet, %s not run; evidence under %s/%s\n' "$ran" "$fails" "$unmets" "$skipped" \
    "$(mj_rel "$MJ_UC_EVIDENCE")" "$([ "$lived" = 0 ] || printf ' and %s/' "$(mj_rel "$MJ_UC_LIVE_EVIDENCE")")"; fi
  mj_ledger_append use_cases.ran "\"ran\":$ran,\"failed\":$fails,\"unmet\":$unmets" 2>/dev/null || true
  [ "$fails" = 0 ] && [ "$unmets" = 0 ] || return "$MJ_EX_CONTRACT"
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
        # A live scenario runs the command, and CI cannot reproduce what it saw: the tree
        # it asked about was one machine's at one minute. It counts as naming the command,
        # never as covering it — a guarantee nothing can re-run is not a guarantee (ADR 38).
        if mj_uc_runs "$i" "$c" && ! mj_uc_is_live "$i"; then n_exec=$((n_exec+1))
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
      if mj_uc_active "$i" && mj_uc_has "$i" claims "$cls"; then
        n_named=$((n_named+1))
        if mj_uc_has_scenario "$i" && ! mj_uc_is_live "$i"; then n_exec=$((n_exec+1)); fi
      fi
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
      printf 'doctrines: []\nclaims: [%s]\nresponsibilities: [%s]\napplications: []\n---\n\n# Situation\n\nTODO\n\n# Scenario\n\n```yaml\nsetup: %s\ngiven:\n  - %s\nsteps:\n  - id: first\n    run: [%s]\n    expect:\n      exit: %s\nthen:\n  - %s\n```\n\n# Outcome\n\nTODO\n' \
        "$(printf '%s' "$claims_of" | sed 's/ *$//; s/ /, /g')" "$resp_cmd" "$setup" "'TODO: the state the setup script prepares'" "$run0" "$exp" "'TODO: what is true afterwards'"
    } > "$f"
    written=$((written+1)); printf 'wrote: %s (draft; complete the narrative and the assertions, then set status: active)\n' "$(mj_rel "$f")"
  done
  printf 'usecase scaffold: %s draft(s) written\n' "$written"
  return 0
}
