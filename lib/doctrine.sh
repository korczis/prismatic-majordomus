#!/usr/bin/env bash
# doctrine — the registry, the dispatcher, and the wiring verifier.
#
# The registry (share/doctrines.yaml) says what must be true. A validator function
# named mj_validate_<validator> determines whether it is true. This dispatcher decides
# when a validator runs: it iterates the registry and calls every doctrine that names
# the running command. Nothing here selects validators by hand, so a doctrine added to
# the registry runs from that moment, and a doctrine whose validator does not exist is
# a reported failure rather than a silent skip.
#
# Fail-closed: a missing validator, an unknown class, and a validator that crashes are
# all failures of the command, and each is a different message.

MJ_DOC_FLAT=""            # flattened registry
MJ_DOCTRINE_ID=""         # the doctrine currently executing
MJ_DOCTRINE_CLASS=""      # its class; mj_doctrine_fail routes on this
MJ_DOCTRINE_CMD=""        # the command the dispatcher is running for
MJ_DOCTRINE_RAN=""        # ids dispatched during this command, space-separated
MJ_DOCTRINE_SKIPPED=0     # a validator sets this when the doctrine does not apply to this case
MJ_DOCTRINE_RESULTS=""    # "<id>:pass|fail|skipped" per dispatched doctrine
MJ_DOCTRINE_ERRORS=0      # validator-execution and configuration errors, not rule violations
export MJ_DOCTRINE_ID MJ_DOCTRINE_CLASS MJ_DOCTRINE_CMD

mj_doctrine_registry() { printf '%s\n' "$MJ_BIN_DIR/../share/doctrines.yaml"; }

mj_doctrine_load() {
  [ -n "$MJ_DOC_FLAT" ] && [ -f "$MJ_DOC_FLAT" ] && return 0
  local reg; reg="$(mj_doctrine_registry)"
  [ -f "$reg" ] || mj_die "$MJ_EX_INTERNAL" "doctrine registry missing: $reg"
  MJ_DOC_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.doc.XXXXXX")"
  mj_yaml_flatten "$reg" > "$MJ_DOC_FLAT" 2>/dev/null || mj_die "$MJ_EX_INTERNAL" "doctrine registry does not parse: $reg"
  [ "$(mj_yget "$MJ_DOC_FLAT" version)" = 1 ] || mj_die "$MJ_EX_INTERNAL" "doctrine registry version must be 1"
}

# mj_doc <index> <field>            -> value
mj_doc() { mj_yget "$MJ_DOC_FLAT" "doctrines.$1.$2"; }
# mj_doc_list <index> <field>       -> one element per line
mj_doc_list() { mj_ylist "$MJ_DOC_FLAT" "doctrines.$1.$2"; }
# mj_doc_index <id>                 -> index, or empty
mj_doc_index() {
  local i=0
  while [ -n "$(mj_doc "$i" id)" ]; do [ "$(mj_doc "$i" id)" = "$1" ] && { printf '%s' "$i"; return 0; }; i=$((i+1)); done
  return 1
}
mj_doc_count() { local i=0; while [ -n "$(mj_doc "$i" id)" ]; do i=$((i+1)); done; printf '%s' "$i"; }
mj_doc_ids() { local i=0; while [ -n "$(mj_doc "$i" id)" ]; do mj_doc "$i" id; i=$((i+1)); done; }

mj_is_function() { type "$1" 2>/dev/null | head -n1 | grep -q 'function'; }
# `doctrine status` reports on the installation, not on this process — it must read the
# source. Only the dispatcher, which has the libraries loaded, asks the live process.
mj_validator_defined() { grep -rqE "^mj_validate_$1\(\)" "$MJ_BIN_DIR/../lib"; }

# does doctrine <index> apply to command <name>?
mj_doctrine_applies() {
  local c
  for c in $(mj_doc_list "$1" enforced_by); do [ "$c" = "$2" ] && return 0; done
  return 1
}

# A validator reports a violation through this, never through mj_fail directly, so the
# registry's class is what decides whether the command stops. An advisory doctrine that
# is mislabelled blocking changes behaviour here and a test sees it.
# watch answers a different question with the same doctrines — what has drifted, not
# what is wrong — so its findings carry DRIFT and its exit code is 11. The rule, the
# validator and the message are the same; only the level differs.
mj_doctrine_fail() {
  # A class says whether a violation blocks work. watch never blocks work — it reports
  # what has moved and exits 11 to say it found something — so under watch every
  # deviation is drift, advisory ones included. A stale checkpoint is a nudge during
  # check and a fact during watch.
  [ "$MJ_DOCTRINE_CMD" = watch ] && { mj_drift "$@"; return 0; }
  case "$MJ_DOCTRINE_CLASS" in
    advisory) mj_warn "$@" ;;
    blocking|*) mj_fail "$@" ;;
  esac
}
mj_doctrine_ok() { mj_ok "$@"; }
mj_doctrine_skip() { mj_info "$@"; }

# mj_doctrine_dispatch <command>
# Runs every applicable doctrine's validator. Returns 0 always; the outcome is in
# MJ_FAILS and MJ_DOCTRINE_ERRORS, which the calling command turns into an exit code.
mj_doctrine_dispatch() {
  local cmd="$1" i=0 id val cls fn rc f0
  mj_doctrine_load
  MJ_DOCTRINE_RAN=""; MJ_DOCTRINE_RESULTS=""; MJ_DOCTRINE_CMD="$cmd"
  while [ -n "$(mj_doc "$i" id)" ]; do
    if mj_doctrine_applies "$i" "$cmd"; then
      id="$(mj_doc "$i" id)"; val="$(mj_doc "$i" validator)"; cls="$(mj_doc "$i" class)"
      case "$cls" in
        blocking|advisory) ;;
        *) mj_fail doctrine "$id" "unknown class '$cls' (blocking | advisory)" "grep -n 'id: $id' share/doctrines.yaml"
           MJ_DOCTRINE_ERRORS=$((MJ_DOCTRINE_ERRORS+1)); i=$((i+1)); continue ;;
      esac
      fn="mj_validate_$val"
      if ! mj_is_function "$fn"; then
        mj_fail doctrine "$id" "declares validator '$val' but no function $fn exists" "grep -rn 'mj_validate_$val' lib/"
        MJ_DOCTRINE_ERRORS=$((MJ_DOCTRINE_ERRORS+1)); i=$((i+1)); continue
      fi
      MJ_DOCTRINE_ID="$id"; MJ_DOCTRINE_CLASS="$cls"; MJ_DOCTRINE_SKIPPED=0; f0="$MJ_FAILS"
      rc=0; "$fn" || rc=$?
      if [ "$MJ_DOCTRINE_SKIPPED" = 1 ]; then MJ_DOCTRINE_RESULTS="$MJ_DOCTRINE_RESULTS $id:skipped"
      elif [ "$MJ_FAILS" -gt "$f0" ]; then MJ_DOCTRINE_RESULTS="$MJ_DOCTRINE_RESULTS $id:fail"
      else MJ_DOCTRINE_RESULTS="$MJ_DOCTRINE_RESULTS $id:pass"; fi
      MJ_DOCTRINE_ID=""; MJ_DOCTRINE_CLASS=""
      # A validator signals a rule violation with mj_doctrine_fail, not with its exit
      # status. A non-zero return therefore means the validator itself broke, which is
      # a different fact and must not be reported as a clean run.
      if [ "$rc" != 0 ]; then
        mj_fail doctrine "$id" "validator $val exited $rc; this is a validator failure, not a rule result" "bash -x bin/majordomus $cmd"
        MJ_DOCTRINE_ERRORS=$((MJ_DOCTRINE_ERRORS+1))
      fi
      MJ_DOCTRINE_RAN="$MJ_DOCTRINE_RAN $id"
    fi
    i=$((i+1))
  done
  MJ_DOCTRINE_CMD=""
  return 0
}

# the pass/fail/skipped line one doctrine produced during the last dispatch
mj_doctrine_result() {
  local e
  for e in $MJ_DOCTRINE_RESULTS; do case "$e" in "$1":*) printf '%s' "${e#*:}"; return 0 ;; esac; done
  return 1
}

# ---------------------------------------------------------------- doctrine status
# Counts derived from the registry and the source, never written down.
mj_cmd_doctrine() {
  local sub="${1:-status}"
  case "$sub" in
    --help|-h|help) cat <<H
usage: majordomus doctrine [status|list|show <id>] [--json]
  status   declared, blocking, advisory, unwired and uncovered counts, all derived
  list     one line per doctrine: id, class, validator, the commands that enforce it
  show     the full record for one doctrine, including its claims and test
H
      return 0 ;;
    status|list) shift || true ;;
    show) shift; mj_doctrine_show "${1:-}"; return ;;
    *) mj_die "$MJ_EX_USAGE" "doctrine: unknown subcommand '$sub'" ;;
  esac
  mj_doctrine_load
  [ "$sub" = list ] && { mj_doctrine_list; return 0; }
  local n bl ad un uc i id
  n="$(mj_doc_count)"; bl=0; ad=0; un=0; uc=0; i=0
  while [ -n "$(mj_doc "$i" id)" ]; do
    id="$(mj_doc "$i" id)"
    case "$(mj_doc "$i" class)" in blocking) bl=$((bl+1)) ;; advisory) ad=$((ad+1)) ;; esac
    mj_validator_defined "$(mj_doc "$i" validator)" || un=$((un+1))
    [ -f "$MJ_BIN_DIR/../$(mj_doc "$i" test)" ] || uc=$((uc+1))
    i=$((i+1))
  done
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"declared":%s,"blocking":%s,"advisory":%s,"unwired":%s,"untested":%s}\n' "$n" "$bl" "$ad" "$un" "$uc"
  else
    printf 'declared doctrines:   %s\n' "$n"
    printf 'blocking:             %s\n' "$bl"
    printf 'advisory:             %s\n' "$ad"
    printf 'missing validators:   %s\n' "$un"
    printf 'without a test file:  %s\n' "$uc"
    printf '\nwiring is verified by: majordomus doctor\n'
  fi
  [ "$un" = 0 ] && [ "$uc" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
}
mj_doctrine_list() {
  local i=0
  while [ -n "$(mj_doc "$i" id)" ]; do
    printf '%-26s %-9s %-20s %s\n' "$(mj_doc "$i" id)" "$(mj_doc "$i" class)" \
      "mj_validate_$(mj_doc "$i" validator)" "$(mj_doc_list "$i" enforced_by | paste -sd, -)"
    i=$((i+1))
  done
}
mj_doctrine_show() {
  local id="$1" i
  mj_doctrine_load
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "doctrine show needs an id (see: majordomus doctrine list)"
  i="$(mj_doc_index "$id")" || mj_die "$MJ_EX_MISSING" "no doctrine '$id' (see: majordomus doctrine list)"
  printf 'id          %s\n' "$id"
  printf 'title       %s\n' "$(mj_doc "$i" title)"
  printf 'class       %s\n' "$(mj_doc "$i" class)"
  printf 'principle   %s\n' "$(mj_doc "$i" principle)"
  printf 'summary     %s\n' "$(mj_doc "$i" summary)"
  printf 'validator   mj_validate_%s\n' "$(mj_doc "$i" validator)"
  printf 'enforced by %s\n' "$(mj_doc_list "$i" enforced_by | paste -sd, -)"
  printf 'exit code   %s\n' "$(mj_doc "$i" exit_code)"
  printf 'claims      %s\n' "$(mj_doc_list "$i" claims | paste -sd, -)"
  printf 'test        %s\n' "$(mj_doc "$i" test)"
  mj_validator_defined "$(mj_doc "$i" validator)" \
    && printf 'wired       yes (%s defines it)\n' "$(grep -rlE "^mj_validate_$(mj_doc "$i" validator)\(\)" "$MJ_BIN_DIR/../lib" | sed "s#.*/lib/#lib/#" | head -1)" \
    || printf 'wired       NO — validator function does not exist\n'
}
