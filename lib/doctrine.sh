#!/usr/bin/env bash
# doctrine — the registry, the dispatcher, and the wiring verifier.
#
# The registry is the repository's effective rule set: every rule under its rules section
# (the vendored Majordomus baseline plus its own project rules) whose x-majordomus block
# names a validator, in resolved dependency order. A validator function named
# mj_validate_<validator> determines whether the rule holds. This dispatcher decides when a
# validator runs: it iterates the registry and calls every doctrine that names the running
# command. Nothing here selects validators by hand, so a rule added to the package runs
# from that moment, and a rule whose validator does not exist is a reported failure rather
# than a silent skip.
#
# Fail-closed: a set of rules that does not resolve, a missing validator, an unknown class,
# and a validator that crashes are all failures of the command, and each is a different
# message.
# shellcheck source=rules.sh
. "$MJ_LIB_DIR/rules.sh"

MJ_DOC_FLAT=""            # the registry, flattened: doctrines.N.<field>
MJ_DOCTRINE_ID=""         # the doctrine currently executing
MJ_DOCTRINE_CLASS=""      # its class; mj_doctrine_fail routes on this
MJ_DOCTRINE_CMD=""        # the command the dispatcher is running for
MJ_DOCTRINE_RAN=""        # ids dispatched during this command, space-separated
MJ_DOCTRINE_SKIPPED=0     # a validator sets this when the doctrine does not apply to this case
MJ_DOCTRINE_RESULTS=""    # "<id>:pass|fail|skipped" per dispatched doctrine
MJ_DOCTRINE_ERRORS=0      # validator-execution and configuration errors, not rule violations
export MJ_DOCTRINE_ID MJ_DOCTRINE_CLASS MJ_DOCTRINE_CMD

# Derive the registry from the effective rules. A set that does not resolve is a failure
# of the command that needed it, named by the loader; there is no partial registry.
mj_doctrine_load() {
  [ -n "$MJ_DOC_FLAT" ] && [ -f "$MJ_DOC_FLAT" ] && { [ "${#MJ_DOC_ROW[@]}" -gt 0 ] || mj_doctrine_rows; return 0; }
  mj_rules_load || mj_die "$MJ_EX_CONTRACT" "the rules do not resolve, so nothing can be enforced: $MJ_RULES_ERROR (see: majordomus rules list)"
  MJ_DOC_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.doc.XXXXXX")"
  # one pass over the effective set: every enforced rule becomes doctrines.N.*, with the
  # scalar fields first, the description and statement as summary and statement, the
  # list fields in the order the rule declared them, and the first test as test
  awk '
    function flush(  j, nk, ks) {
      if (cur == "" || s["enforced"] != 1) return
      nk = split("id title class validator category policy_key exit_code file provenance", ks, " ")
      for (j = 1; j <= nk; j++) if (s[ks[j]] != "") print "doctrines." n "." ks[j] "=" s[ks[j]]
      print "doctrines." n ".summary=" s["description"]
      print "doctrines." n ".statement=" s["statement"]
      for (j = 1; j <= nl; j++) print "doctrines." n "." L[j]
      print "doctrines." n ".test=" t
      n++
    }
    BEGIN { print "version=1"; n = 0; cur = "" }
    {
      eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
      if (split(k, p, ".") < 3 || p[1] != "rules") next
      i = p[2]; f = substr(k, length("rules." i ".") + 1)
      if (i != cur) { flush(); for (kk in s) delete s[kk]; nl = 0; t = ""; tset = 0; cur = i }
      if (!(f in s)) s[f] = v
      if (f ~ /^(enforced_by|claims|tests|depends_on)\./) L[++nl] = f "=" v
      if (!tset && f ~ /^tests\.[0-9]+$/) { t = v; tset = 1 }
    }
    END { flush() }' "$MJ_RULES_FLAT" > "$MJ_DOC_FLAT"
  mj_doctrine_rows
  return 0
}

# mj_doc <index> <field>            -> value
mj_doc() { mj_yget "$MJ_DOC_FLAT" "doctrines.$1.$2"; }
# mj_doc_list <index> <field>       -> one element per line
mj_doc_list() { mj_ylist "$MJ_DOC_FLAT" "doctrines.$1.$2"; }
# ---------------------------------------------------------------- doctrine rows
# The registry, one row per doctrine, built once from the flat file and kept in memory:
# index, id, validator, class, file, first test, policy_key, and the enforced_by, tests
# and claims lists comma-joined. The dispatcher and the wiring verifier read these rows
# with builtins, so a doctor run no longer pays a process per field per doctrine. The
# separator is a tab: the one byte the flattener refuses in every front matter, so no
# rule value can carry it, and a registry line that does carry one (a file name could)
# stops the load rather than shifting a row.
MJ_DOC_ROW=()
MJ_DOC_SEP=$'\t'
mj_doctrine_rows() {
  local line rows
  MJ_DOC_ROW=()
  rows="$(mktemp "${TMPDIR:-/tmp}/mj.rows.XXXXXX")"
  awk '
    function flush() { if (!have) return; printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n", cur, s["id"], s["validator"], s["class"], s["file"], s["test"], s["policy_key"], eb, ts, cl }
    BEGIN { have = 0 }
    index($0, "\t") { print "ERROR:line " NR " of the registry carries a tab character" > "/dev/stderr"; exit 3 }
    { eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
      if (split(k, p, ".") < 3 || p[1] != "doctrines") next
      i = p[2]; f = substr(k, length("doctrines." i ".") + 1)
      if (!have || i != cur) { flush(); for (kk in s) delete s[kk]; eb = ""; ts = ""; cl = ""; cur = i; have = 1 }
      if (!(f in s)) s[f] = v
      if (f ~ /^enforced_by\.[0-9]+$/) eb = (eb == "" ? v : eb "," v)
      else if (f ~ /^tests\.[0-9]+$/) ts = (ts == "" ? v : ts "," v)
      else if (f ~ /^claims\.[0-9]+$/) cl = (cl == "" ? v : cl "," v) }
    END { flush() }' "$MJ_DOC_FLAT" > "$rows" 2> "$rows.err" \
    || { rm -f "$rows"; mj_die "$MJ_EX_INTERNAL" "cannot build the doctrine registry: $(sed -n 's/^ERROR://p' "$rows.err" | head -n 1); a rule file name or path is the only value the flattener does not refuse a tab in"; }
  while IFS= read -r line; do MJ_DOC_ROW[${#MJ_DOC_ROW[@]}]="$line"; done < "$rows"
  rm -f "$rows" "$rows.err"
}
# mj_doc_row <index> -> MJ_DR_ID MJ_DR_VAL MJ_DR_CLASS MJ_DR_FILE MJ_DR_TEST MJ_DR_KEY
#                       MJ_DR_EB MJ_DR_TESTS MJ_DR_CLAIMS, or 1 past the last row. Fields
#                       are cut by expansion, so an empty one stays in its place.
MJ_DR_ID=""; MJ_DR_VAL=""; MJ_DR_CLASS=""; MJ_DR_FILE=""; MJ_DR_TEST=""; MJ_DR_KEY=""; MJ_DR_EB=""; MJ_DR_TESTS=""; MJ_DR_CLAIMS=""
# shellcheck disable=SC2034  # the row fields are read by the dispatcher and by doctor
mj_doc_row() {
  [ "$1" -lt "${#MJ_DOC_ROW[@]}" ] || return 1
  local r="${MJ_DOC_ROW[$1]}" s="$MJ_DOC_SEP"
  r="${r#*"$s"}"
  MJ_DR_ID="${r%%"$s"*}";    r="${r#*"$s"}"
  MJ_DR_VAL="${r%%"$s"*}";   r="${r#*"$s"}"
  MJ_DR_CLASS="${r%%"$s"*}"; r="${r#*"$s"}"
  MJ_DR_FILE="${r%%"$s"*}";  r="${r#*"$s"}"
  MJ_DR_TEST="${r%%"$s"*}";  r="${r#*"$s"}"
  MJ_DR_KEY="${r%%"$s"*}";   r="${r#*"$s"}"
  MJ_DR_EB="${r%%"$s"*}";    r="${r#*"$s"}"
  MJ_DR_TESTS="${r%%"$s"*}"; r="${r#*"$s"}"
  MJ_DR_CLAIMS="$r"
}
# mj_doc_field <index> <n> -> field n of the row (1 index, 2 id, 3 validator, 4 class,
# 5 file, 6 test, 7 policy_key, 8 enforced_by, 9 tests, 10 claims), by expansion alone
mj_doc_field() {
  [ "$1" -lt "${#MJ_DOC_ROW[@]}" ] || return 1
  local row="${MJ_DOC_ROW[$1]}" n=1
  while [ "$n" -lt "$2" ]; do row="${row#*"$MJ_DOC_SEP"}"; n=$((n+1)); done
  printf '%s' "${row%%"$MJ_DOC_SEP"*}"
}
# mj_doc_index <id>                 -> index, or empty
mj_doc_index() {
  local i=0 row rest
  while [ "$i" -lt "${#MJ_DOC_ROW[@]}" ]; do
    row="${MJ_DOC_ROW[$i]}"; rest="${row#*"$MJ_DOC_SEP"}"
    [ "${rest%%"$MJ_DOC_SEP"*}" = "$1" ] && { printf '%s' "$i"; return 0; }
    i=$((i+1))
  done
  return 1
}
mj_doc_count() { printf '%s' "${#MJ_DOC_ROW[@]}"; }
mj_doc_ids() { local i=0; while [ "$i" -lt "${#MJ_DOC_ROW[@]}" ]; do mj_doc_field "$i" 2; printf '\n'; i=$((i+1)); done; }

mj_is_function() { type "$1" 2>/dev/null | head -n1 | grep -q 'function'; }
# `doctrine status` reports on the installation, not on this process — it must read the
# source. Only the dispatcher, which has the libraries loaded, asks the live process.
mj_validator_defined() { grep -rqE "^mj_validate_$1\(\)" "$MJ_LIB_DIR"; }

mj_doctrine_applies() {
  local eb; eb="$(mj_doc_field "$1" 8)" || return 1
  case ",$eb," in *",$2,"*) return 0 ;; esac
  return 1
}

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
# Runs every doctrine that names <command> in enforced_by. Records which ran, and how
# each ended, so that a command can report doctrines that were never reached.
mj_doctrine_dispatch() {
  local cmd="$1" i=0 id val cls file fn rc f0 t0
  t0="$(mj_phase_begin doctrine:load)"; mj_doctrine_load; mj_phase_end doctrine:load "$t0"
  MJ_DOCTRINE_RAN=""; MJ_DOCTRINE_RESULTS=""; MJ_DOCTRINE_CMD="$cmd"
  while mj_doc_row "$i"; do
    case ",$MJ_DR_EB," in *",$cmd,"*) ;; *) i=$((i+1)); continue ;; esac
    id="$MJ_DR_ID"; val="$MJ_DR_VAL"; cls="$MJ_DR_CLASS"; file="$MJ_DR_FILE"
    case "$cls" in
      blocking|advisory) ;;
      *) mj_fail doctrine "$id" "unknown class '$cls' (blocking | advisory)" "grep -n '^class:' $file"
         MJ_DOCTRINE_ERRORS=$((MJ_DOCTRINE_ERRORS+1)); i=$((i+1)); continue ;;
    esac
    fn="mj_validate_$val"
    if ! mj_is_function "$fn"; then
      mj_fail doctrine "$id" "declares validator '$val' but no function $fn exists" "grep -rn 'mj_validate_$val' lib/"
      MJ_DOCTRINE_ERRORS=$((MJ_DOCTRINE_ERRORS+1)); i=$((i+1)); continue
    fi
    MJ_DOCTRINE_ID="$id"; MJ_DOCTRINE_CLASS="$cls"; MJ_DOCTRINE_SKIPPED=0; f0="$MJ_FAILS"
    t0="$(mj_phase_begin "validate:$val")"
    rc=0; "$fn" || rc=$?
    mj_phase_end "validate:$val" "$t0"
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
  a doctrine is a rule of the effective set (majordomus rules list) that names a validator
  status   declared, blocking, advisory, unwired and uncovered counts, all derived
  list     one line per doctrine: id, class, validator, the commands that enforce it
  show     the full record for one doctrine: claims, tests, dependencies, the rule file
H
      return 0 ;;
    status|list) shift || true ;;
    show) shift; mj_doctrine_show "${1:-}"; return ;;
    *) mj_die "$MJ_EX_USAGE" "doctrine: unknown subcommand '$sub'" ;;
  esac
  mj_require_installed
  mj_doctrine_load
  [ "$sub" = list ] && { mj_doctrine_list; return 0; }
  local n bl ad un uc i defined
  n="${#MJ_DOC_ROW[@]}"; bl=0; ad=0; un=0; uc=0; i=0
  defined=" $(mj_validators_defined) "
  while mj_doc_row "$i"; do
    case "$MJ_DR_CLASS" in blocking) bl=$((bl+1)) ;; advisory) ad=$((ad+1)) ;; esac
    case "$defined" in *" $MJ_DR_VAL "*) ;; *) un=$((un+1)) ;; esac
    [ -f "$MJ_HOME/$MJ_DR_TEST" ] || uc=$((uc+1))
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
    printf '\nthe registry is the effective rule set: majordomus rules list\nwiring is verified by: majordomus doctor\n'
  fi
  [ "$un" = 0 ] && [ "$uc" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
}
# every validator name lib/ defines, one scan, space-separated
mj_validators_defined() { grep -rhoE '^mj_validate_[A-Za-z0-9_-]+\(\)' "$MJ_LIB_DIR" | sed -e 's/^mj_validate_//' -e 's/()$//' | paste -sd' ' -; }
mj_doctrine_list() {
  local i=0
  while mj_doc_row "$i"; do
    printf '%-38s %-9s %-24s %s\n' "$MJ_DR_ID" "$MJ_DR_CLASS" "mj_validate_$MJ_DR_VAL" "$MJ_DR_EB"
    i=$((i+1))
  done
}
mj_doctrine_show() {
  local id="$1" i
  mj_require_installed
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "doctrine show needs an id (see: majordomus doctrine list)"
  mj_doctrine_load
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
  printf 'tests       %s\n' "$(mj_doc_list "$i" tests | paste -sd, -)"
  printf 'depends on  %s\n' "$(mj_doc_list "$i" depends_on | paste -sd, -)"
  printf 'rule        %s (%s)\n' "$(mj_doc "$i" file)" "$(mj_doc "$i" provenance)"
  mj_validator_defined "$(mj_doc "$i" validator)" \
    && printf 'wired       yes (%s defines it)\n' "$(grep -rlE "^mj_validate_$(mj_doc "$i" validator)\(\)" "$MJ_LIB_DIR" | sed "s#.*/lib/#lib/#" | head -1)" \
    || printf 'wired       NO — validator function does not exist\n'
}
