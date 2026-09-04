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
# Both ship with the tool, so both read share/, not .majordomus/. A repository configures
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
# ids of the commands the registry marks public, one per line
mj_cmdreg_public() {
  local i=0
  while [ -n "$(mj_cmdreg "$i" id)" ]; do
    [ "$(mj_cmdreg "$i" visibility)" = public ] && mj_cmdreg "$i" id
    i=$((i+1))
  done
}
mj_cmdreg_ids() { local i=0; while [ -n "$(mj_cmdreg "$i" id)" ]; do mj_cmdreg "$i" id; i=$((i+1)); done; }
# the commands the binary actually dispatches, read from the dispatch table
mj_dispatched() {
  grep -oE '^  [a-z|]+\)$' "$MJ_BIN_DIR/majordomus" | tr -d ' )' | tr '|' '\n' | sort -u
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
  local ids public c bad=0 dupes
  ids="$(mj_cmdreg_ids)"; public="$(mj_cmdreg_public)"

  dupes="$(printf '%s\n' "$ids" | sort | uniq -d)"
  if [ -n "$dupes" ]; then
    mj_doctrine_fail command "registry" "duplicate command id(s): $(printf '%s' "$dupes" | tr '\n' ' ')" "grep -n 'id:' share/commands.yaml"
    bad=1
  fi

  for c in $(mj_dispatched); do
    printf '%s\n' "$ids" | grep -Fxq "$c" || {
      mj_doctrine_fail command "$c" "dispatched by bin/majordomus but absent from share/commands.yaml" "grep -n 'id: $c' share/commands.yaml"; bad=1; }
  done
  for c in $public; do
    [ "$c" = version ] && continue      # dispatched ahead of the option parser
    printf '%s\n' "$(mj_dispatched)" | grep -Fxq "$c" || {
      mj_doctrine_fail command "$c" "declared public but bin/majordomus does not dispatch it" "grep -n '$c)' bin/majordomus"; bad=1; }
  done
  # a command is public exactly when the usage text lists it, in both directions
  for c in $public; do
    grep -qE "^  $c( |\$)" "$MJ_BIN_DIR/majordomus" || {
      mj_doctrine_fail command "$c" "declared public but the usage text does not list it" "bin/majordomus --help"; bad=1; }
  done

  [ "$bad" = 0 ] && mj_doctrine_ok command "surface" \
    "$(printf '%s\n' "$public" | wc -w | tr -d ' ') public command(s), reconciled against the dispatch table" \
    "majordomus doctor"
  return 0
}

# ---------------------------------------------------------------- command_coverage_complete
# Every public command is exercised and refuted by a test that declares it. This is a rule
# about Majordomus's own suite, so it applies only in the repository that carries one.
mj_validate_command_coverage() {
  local cases="$MJ_BIN_DIR/../test/cases"
  if ! mj_cmdreg_load || [ ! -d "$cases" ]; then
    MJ_DOCTRINE_SKIPPED=1
    mj_doctrine_skip command "coverage" "this installation carries no test suite to measure" "ls test/cases"
    return 0
  fi
  local f c bad=0 declared="" behaviour="" negative=""
  for f in "$cases"/*.sh; do
    [ -f "$f" ] || continue
    declared="$declared $(sed -n 's/^# majordomus-covers: *//p' "$f" | head -1)"
    behaviour="$behaviour $(sed -n 's/^# majordomus-covers: *//p' "$f" | head -1)"
    negative="$negative $(sed -n 's/^# majordomus-negative: *//p' "$f" | head -1)"
  done
  [ -n "$(printf '%s' "$declared" | tr -d ' ')" ] || {
    MJ_DOCTRINE_SKIPPED=1
    mj_doctrine_skip command "coverage" "no case declares what it covers" "grep -rn 'majordomus-covers' test/cases/"
    return 0; }

  for c in $(mj_cmdreg_public); do
    case " $behaviour " in *" $c "*) ;;
      *) mj_doctrine_fail command "$c" "no test case declares behaviour coverage of it" "grep -rn 'majordomus-covers' test/cases/"; bad=1 ;;
    esac
    case " $negative " in *" $c "*) ;;
      *) mj_doctrine_fail command "$c" "no test case declares a failure mode of it" "grep -rn 'majordomus-negative' test/cases/"; bad=1 ;;
    esac
  done
  # a header naming a command that does not exist is a broken reference, not documentation
  for c in $(printf '%s\n' $behaviour $negative | sort -u); do
    [ "$c" = none ] && continue
    mj_cmdreg_public | grep -Fxq "$c" || {
      mj_doctrine_fail command "$c" "a test case declares coverage of it, but it is not a public command" "grep -rn '$c' test/cases/ | grep majordomus-"; bad=1; }
  done

  [ "$bad" = 0 ] && mj_doctrine_ok command "coverage" "every public command is exercised and refuted" "bash test/run.sh 31_command_coverage"
  return 0
}
