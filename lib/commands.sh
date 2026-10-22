#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
# sourced by doctor and watch; guard against re-sourcing
[ -n "${MJ_LIB_commands:-}" ] && return 0 || MJ_LIB_commands=1
# commands — the validators for the command surface.
#
# share/commands.yaml says what a command means; bin/majordomus says what runs. Neither
# derives from the other, so these two doctrines are the reconciliation between them and
# between the surface and the tests that prove it.
#
# Both ship with the tool, so both read share/, not the AI layer. A repository configures
# Majordomus; it does not declare Majordomus's own commands.

MJ_CMDREG_FLAT=""
mj_cmdreg_load() {
  [ -n "$MJ_CMDREG_FLAT" ] && [ -f "$MJ_CMDREG_FLAT" ] && return 0
  local reg="$MJ_BIN_DIR/../share/commands.yaml"
  [ -f "$reg" ] || return 1
  MJ_CMDREG_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.cmd.XXXXXX")"
  mj_yaml_flatten "$reg" > "$MJ_CMDREG_FLAT" 2>/dev/null || { rm -f "$MJ_CMDREG_FLAT"; MJ_CMDREG_FLAT=""; return 1; }
  [ "$(mj_yget "$MJ_CMDREG_FLAT" version)" = 1 ] || return 1
}
mj_cmdreg() { mj_yget "$MJ_CMDREG_FLAT" "commands.$1.$2"; }
# the class of one command, by id: read-only, state-mutating or generated-output-mutating.
# One pass, for the same reason mj_cmdreg_public makes one: the callers are loops. Empty
# when the registry does not describe the command.
mj_cmdreg_class() {
  awk -F= -v want="$1" '/^commands\.[0-9]+\.(id|class)=/ {
      split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v)
      if (k[3] == "id") id[k[2]] = v; else cls[k[2]] = v
      if (k[2] + 0 > max) max = k[2] + 0 }
    END { for (i = 0; i <= max; i++) if ((i in id) && id[i] == want) { print cls[i]; exit } }' "$MJ_CMDREG_FLAT"
}
# the class of one whole argument vector: `mj_cmdreg_effective <command> [<arg>...]` prints
# "<class><TAB><what decided it>". The command's own class is the worst case of everything
# it can do; the registry's `effects` say where subcommands or flags differ. A subcommand
# entry replaces the command's class for that subcommand, and a flag entry only ever raises
# it, so a vector is never admitted as less than its worst part. The subcommand is the first
# word that is not a flag. Empty when the registry does not describe the command.
mj_cmdreg_effective() {
  local cmd="$1"; shift
  awk -F= -v want="$cmd" -v args="$*" '
    function rank(c) { return c == "read-only" ? 0 : (c == "state-mutating" ? 1 : 2) }
    /^commands\.[0-9]+\.id=/ { split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v); id[k[2]] = v; next }
    /^commands\.[0-9]+\.class=/ { split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v); cls[k[2]] = v; next }
    /^commands\.[0-9]+\.effects\.[0-9]+\.(subcommand|flag|class)=/ {
      split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v)
      e = k[2] SUBSEP k[4]; if (k[5] == "class") ecls[e] = v; else { ekind[e] = k[5]; ename[e] = v }
      if (k[2] + 0 > max) max = k[2] + 0; if (k[4] + 0 > emax[k[2]]) emax[k[2]] = k[4] + 0; next }
    END {
      for (i in id) if (id[i] == want) c = i
      if (c == "") exit
      n = split(args, w, " "); subc = ""
      for (j = 1; j <= n; j++) if (w[j] !~ /^-/) { subc = w[j]; break }
      cur = cls[c]; why = "command"
      for (m = 0; m <= emax[c] + 0; m++) { e = c SUBSEP m
        if ((e in ekind) && ekind[e] == "subcommand" && ename[e] == subc && subc != "") { cur = ecls[e]; why = "subcommand \047" subc "\047" } }
      for (j = 1; j <= n; j++) { f = w[j]; if (f !~ /^--/) continue; sub(/=.*/, "", f)
        for (m = 0; m <= emax[c] + 0; m++) { e = c SUBSEP m
          if ((e in ekind) && ekind[e] == "flag" && ename[e] == f && rank(ecls[e]) > rank(cur)) { cur = ecls[e]; why = "flag \047" f "\047" } } }
      printf "%s\t%s\n", cur, why
    }' "$MJ_CMDREG_FLAT"
}
# ids of the commands the registry marks public, one per line, in registry order. One pass
# over the flat file: the previous shape ran one awk per index per field, and was called
# from inside loops, which is where the command-coverage validator spent three seconds.
mj_cmdreg_public() {
  awk -F= '/^commands\.[0-9]+\.(id|visibility)=/ {
      split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v)
      if (k[3] == "id") id[k[2]] = v; else vis[k[2]] = v
      if (k[2] + 0 > max) max = k[2] + 0 }
    END { for (i = 0; i <= max; i++) if ((i in id) && vis[i] == "public") print id[i] }' "$MJ_CMDREG_FLAT"
}
mj_cmdreg_ids() {
  awk -F= '/^commands\.[0-9]+\.id=/ { split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v); id[k[2]] = v; if (k[2] + 0 > max) max = k[2] + 0 }
    END { for (i = 0; i <= max; i++) if (i in id) print id[i] }' "$MJ_CMDREG_FLAT"
}
# the commands the binary actually dispatches, read from the dispatch table
mj_dispatched() {
  grep -oE '^  [a-z|]+\)$' "$MJ_BIN_DIR/majordomus" | tr -d ' )' | tr '|' '\n' | LC_ALL=C sort -u
}

# ---------------------------------------------------------------- command_surface_complete
# The installed command surface is coherent: everything dispatched is described, everything
# described is dispatched, and "public" means exactly "listed in the help text".
mj_validate_command_surface() {
  if ! mj_cmdreg_load; then
    MJ_DOCTRINE_SKIPPED=1
    mj_doctrine_skip command "surface" "no share/commands.yaml in this installation" "ls share/commands.yaml"
    return 0
  fi
  local ids public dispatched c bad=0 dupes
  ids="$(mj_cmdreg_ids)"; public="$(mj_cmdreg_public)"; dispatched="$(mj_dispatched)"

  dupes="$(printf '%s\n' "$ids" | LC_ALL=C sort | uniq -d)"
  if [ -n "$dupes" ]; then
    mj_doctrine_fail command "registry" "duplicate command id(s): $(printf '%s' "$dupes" | tr '\n' ' ')" "grep -n 'id:' share/commands.yaml"
    bad=1
  fi

  # Whether a word is a line of a list is a pattern match on the list between newlines, in
  # this shell: a grep per command, each on a here-string, was most of this validator, and
  # a pipe into `grep -q` raced its own writer (an EPIPE diagnostic in doctor's recorded
  # output, on unlucky timing). The usage text is read once for every public command.
  local nl='
' unlisted
  for c in $dispatched; do
    case "$nl$ids$nl" in *"$nl$c$nl"*) ;; *)
      mj_doctrine_fail command "$c" "dispatched by bin/majordomus but absent from share/commands.yaml" "grep -n 'id: $c' share/commands.yaml"; bad=1 ;;
    esac
  done
  for c in $public; do
    [ "$c" = version ] && continue      # dispatched ahead of the option parser
    case "$nl$dispatched$nl" in *"$nl$c$nl"*) ;; *)
      mj_doctrine_fail command "$c" "declared public but bin/majordomus does not dispatch it" "grep -n '$c)' bin/majordomus"; bad=1 ;;
    esac
  done
  # a command is public exactly when the usage text lists it, in both directions: a line of
  # bin/majordomus that is two spaces, the command, then a space or its end
  unlisted="$(awk 'NR == FNR { if ($0 != "") want[++n] = $0; next } { line[++m] = $0 }
    END { for (i = 1; i <= n; i++) { re = "^  " want[i] "( |$)"; ok = 0
            for (j = 1; j <= m; j++) if (line[j] ~ re) { ok = 1; break }
            if (!ok) print want[i] } }' - "$MJ_BIN_DIR/majordomus" <<<"$public")"
  for c in $unlisted; do
    mj_doctrine_fail command "$c" "declared public but the usage text does not list it" "bin/majordomus --help"; bad=1
  done

  # An effect names something the command takes and a class the registry knows. A subcommand
  # must be a word of the command's syntax, a flag must be written in it, and a flag may not
  # claim to be read-only, because a flag only ever raises what the command already is.
  local problem
  while IFS= read -r problem; do
    [ -n "$problem" ] || continue
    mj_doctrine_fail command "${problem%%: *}" "${problem#*: }" "grep -n 'effects:' share/commands.yaml"; bad=1
  done < <(awk -F= '
    /^commands\.[0-9]+\.(id|syntax)=/ { split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v); if (k[3] == "id") id[k[2]] = v; else syn[k[2]] = v; next }
    /^commands\.[0-9]+\.effects\.[0-9]+\.(subcommand|flag|class)=/ {
      split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v); e = k[2] SUBSEP k[4]
      if (k[5] == "class") ecls[e] = v; else { ekind[e] = k[5]; ename[e] = v }
      cmd[e] = k[2]; next }
    END { for (e in cmd) { c = cmd[e]; n = id[c]; s = syn[c]
        if (ecls[e] !~ /^(read-only|state-mutating|generated-output-mutating)$/) { print n ": effect " ename[e] " declares class \047" ecls[e] "\047, which is not a class the registry has"; continue }
        if (ekind[e] == "flag" && ecls[e] == "read-only") print n ": flag " ename[e] " is declared read-only; a flag can only raise the command class"
        if (ekind[e] == "flag" && index(s, ename[e]) == 0) print n ": flag " ename[e] " is in an effect but not in the command syntax"
        if (ekind[e] == "subcommand" && s !~ ("(^|[^a-z0-9-])" ename[e] "([^a-z0-9-]|$)")) print n ": subcommand " ename[e] " is in an effect but not in the command syntax" } }' "$MJ_CMDREG_FLAT")

  [ "$bad" = 0 ] && mj_doctrine_ok command "surface" \
    "$(printf '%s\n' "$public" | wc -w | tr -d ' ') public command(s), reconciled against the dispatch table" \
    "majordomus doctor"
  return 0
}

# ---------------------------------------------------------------- mj_covers_resolve
# mj_covers_resolve <root> <name>
# Resolves one prefixed name of a `# majordomus-covers:` or `# majordomus-negative:` header
# against the models under <root>. Exit 0: it names something that exists. Exit 1: it does
# not, and stdout says why. Exit 2: it is a bare name, which the caller checks against the
# public command surface. Exit 3: <root> does not carry the model that kind is resolved
# against at all (an installed distribution ships bin/, lib/, share/ and test/, not scripts/
# or .ai/), so the name can be neither confirmed nor refuted there, and stdout says which. The doctor check and test/cases/31 both call this one function, so
# the rule cannot drift between them.
#
# Every match is literal. A name is data, not a pattern: `gate:.*` names no gate, and a
# regular expression built from it would have matched every one. A script is an executable
# under scripts/, never a path that climbs out of it.
mj_covers_resolve() {
  local root="$1" c="$2" v
  case "$c" in
    gate:*)
      v="${c#gate:}"
      [ -f "$root/.ai/repo/ci/gates.yaml" ] || { echo ".ai/repo/ci/gates.yaml"; return 3; }
      [ -n "$v" ] && grep -Fxq "  - id: $v" "$root/.ai/repo/ci/gates.yaml" 2>/dev/null && return 0
      echo "a test case declares coverage of gate '$v', which the model does not declare"
      return 1 ;;
    capability:*)
      # The registry projection rather than the executable: this runs where the executable
      # may not be built, and the projection is committed and gate-held current.
      v="${c#capability:}"
      [ -f "$root/docs/generated/registry.json" ] || { echo "docs/generated/registry.json"; return 3; }
      [ -n "$v" ] && grep -Fq "\"id\": \"$v\"" "$root/docs/generated/registry.json" 2>/dev/null && return 0
      echo "a test case declares coverage of capability '$v', which the registry does not carry"
      return 1 ;;
    workflow:*)
      v="${c#workflow:}"
      [ -d "$root/.github/workflows" ] || { echo ".github/workflows/"; return 3; }
      case "$v" in ''|*/*|.*) ;; *) [ -f "$root/.github/workflows/$v" ] && return 0 ;; esac
      echo "a test case declares coverage of workflow '$v', which is not a file under .github/workflows/"
      return 1 ;;
    script:*)
      v="${c#script:}"
      [ -d "$root/scripts" ] || { echo "scripts/"; return 3; }
      case "$v" in
        */../*|*/..|*/./*) ;;
        scripts/?*) [ -f "$root/$v" ] && [ -x "$root/$v" ] && return 0 ;;
      esac
      echo "a test case declares coverage of script '$v', which is not an executable under scripts/"
      return 1 ;;
    *:*)
      echo "a test case declares coverage under an unknown vocabulary: '$c'; the kinds are gate:, script:, workflow: and capability:, or a bare public command"
      return 1 ;;
  esac
  return 2
}

# ---------------------------------------------------------------- command_coverage_complete
# Every public command is exercised and refuted by a test that declares it. This is a rule
# about Majordomus's own suite, so it applies only in the repository that carries one.
mj_validate_command_coverage() {
  # The installation's own suite and the installation's own models: `MJ_ROOT` is the
  # repository under supervision, which is not necessarily this one, and a coverage check that
  # read its gates from there would be measuring a different tree than the cases it read.
  local root="$MJ_BIN_DIR/.."
  local cases="$root/test/cases"
  if ! mj_cmdreg_load || [ ! -d "$cases" ]; then
    MJ_DOCTRINE_SKIPPED=1
    mj_doctrine_skip command "coverage" "this installation carries no test suite to measure" "ls test/cases"
    return 0
  fi
  local f c bad=0 declared="" behaviour="" negative="" public heads nl='
'
  # every header of every case in one pass: the first covers line and the first negative
  # line of each file, as the per-file sed pipelines read them before. A file is left once it
  # has given both, rather than read to its end twice, once for each header (I0820).
  heads="$(awk 'FNR == 1 { c = 0; g = 0 }
      c == 0 && /^# majordomus-covers:/ { v = $0; sub(/^# majordomus-covers: */, "", v); b = b " " v; c = 1 }
      g == 0 && /^# majordomus-negative:/ { v = $0; sub(/^# majordomus-negative: */, "", v); n = n " " v; g = 1 }
      c && g { nextfile }
      END { printf "%s\n%s.", b, n }' "$cases"/*.sh)"
  behaviour="${heads%%"$nl"*}"; negative="${heads#*"$nl"}"; negative="${negative%.}"
  declared="$behaviour"
  public="$(mj_cmdreg_public)"
  [ -n "$(printf '%s' "$declared" | tr -d ' ')" ] || {
    MJ_DOCTRINE_SKIPPED=1
    mj_doctrine_skip command "coverage" "no case declares what it covers" "grep -rn 'majordomus-covers' test/cases/"
    return 0; }

  for c in $public; do
    case " $behaviour " in *" $c "*) ;;
      *) mj_doctrine_fail command "$c" "no test case declares behaviour coverage of it" "grep -rn 'majordomus-covers' test/cases/"; bad=1 ;;
    esac
    case " $negative " in *" $c "*) ;;
      *) mj_doctrine_fail command "$c" "no test case declares a failure mode of it" "grep -rn 'majordomus-negative' test/cases/"; bad=1 ;;
    esac
  done
  # A header naming something that does not exist is a broken reference, not documentation.
  #
  # Two vocabularies, because this header had one and it could name exactly one kind of thing:
  # a public command of the shell tool. Every case about a gate, a script, a workflow or a
  # capability of the Rust executable therefore declared `none` — not out of neglect, but
  # because there was no word for its subject. On 2026-09-15 that was 74 of 215 cases, and the
  # doctrine line read "command coverage", which a reader takes for "test coverage". The check
  # was right about the world it could see and the world had grown past it.
  #
  #   a bare name        a public command, as before
  #   gate:<id>          an entry of .ai/repo/ci/gates.yaml
  #   script:<path>      an executable under scripts/, named as the repository names it
  #   workflow:<name>    a workflow file under .github/workflows/
  #   capability:<id>    a capability of the Rust executable, from docs/generated/registry.json
  #
  # A prefixed name is not a command and is deliberately not counted as command coverage: the
  # loop above still requires every public command to be named by a bare one, so widening the
  # vocabulary cannot be used to satisfy the narrower obligation. mj_covers_resolve holds the
  # rule for the prefixed names; test/cases/31 calls the same function.
  #
  # The rule is called in this shell, its answer written to a file and read back only when
  # there is one, and a bare name is looked up in the public list by a pattern match: a
  # command substitution and a grep per name were most of this validator (I0820).
  local why rc undecided="" absent="" said
  said="$(mktemp "${TMPDIR:-/tmp}/mj.covers.XXXXXX")"
  for c in $(printf '%s\n' $behaviour $negative | LC_ALL=C sort -u); do
    [ "$c" = none ] && continue
    rc=0; mj_covers_resolve "$root" "$c" > "$said" || rc=$?
    why=""; case "$rc" in 1|3) IFS= read -r why < "$said" || : ;; esac
    case "$rc" in
      0) continue ;;
      1) mj_doctrine_fail command "$c" "$why" "grep -rn '$c' test/cases/ | grep majordomus-"; bad=1
         continue ;;
      3) undecided="$undecided $c"
         case " $absent " in *" $why "*) ;; *) absent="$absent $why" ;; esac
         continue ;;
    esac
    case "$nl$public$nl" in *"$nl$c$nl"*) ;; *)
      mj_doctrine_fail command "$c" "a test case declares coverage of it, but it is not a public command" "grep -rn '$c' test/cases/ | grep majordomus-"; bad=1 ;;
    esac
  done
  rm -f "$said"
  # A name this installation cannot resolve is reported as undecided, never as resolved and
  # never as broken: the suite was copied without the models it names (case 28 and case 65
  # run from such a copy), and the repository's own run of case 31 decides it.
  [ -z "$undecided" ] || mj_doctrine_skip command "coverage" \
    "not resolved here, this installation carries no${absent}:${undecided}" "bash test/run.sh 31_command_coverage"

  [ "$bad" = 0 ] && mj_doctrine_ok command "coverage" "every public command is exercised and refuted" "bash test/run.sh 31_command_coverage"
  return 0
}
