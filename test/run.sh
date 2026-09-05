#!/usr/bin/env bash
# Runs every test/cases/*.sh in a disposable temporary repository each.
# A case script gets: $MJ (path to bin/majordomus), $T (its scratch dir, already a git repo).
# Helpers come from test/lib.sh, which every case sources first.
#
#   bash test/run.sh                    every case, one after the other, output streamed
#   bash test/run.sh <name>             one case (a name that matches nothing is exit 2)
#   MJ_TEST_JOBS=4 bash test/run.sh     bounded parallel run: four cases at a time, each
#                                       with its own log; then the cases that declare
#                                       "# majordomus-exclusive: <reason>" one at a time;
#                                       the verdicts are rendered in name order at the end
#   MJ_TEST_REPORT=<file>               also write one TSV row per case:
#                                       name, result, seconds, phase (parallel|exclusive|serial)
#   MAJORDOMUS_BIN=<path>               the Rust cases drive this prebuilt executable instead
#                                       of building the crate (see rust_bin in test/lib.sh)
#
# A case runs in its own repository and may read the checkout it lives in; it may not write
# into that checkout while other cases run. A case that must (the site cases build into
# site/public, the derived-artifacts case edits and regenerates a document) says so with the
# exclusive header and runs alone. The parallel phase proves the invariant: when it leaves
# `git status` of the checkout changed, the run fails naming the paths.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MJ="$ROOT/bin/majordomus"; export MJ ROOT
pass=0; fail=0; failed_names=""

# ---------------------------------------------------------------- one case
# Runs one case in a fresh repository. The case's output streams through; the status is
# 0 passed, 1 failed, 2 the fixture could not be set up.
run_case() {
  local case="$1" T rc=0
  T="$(mktemp -d "${TMPDIR:-/tmp}/mj-test.XXXXXX")"
  ( cd "$T" && git init -q . && git config user.email t@example.com && git config user.name t \
    && git commit -q --allow-empty -m init ) || { rm -rf "$T"; return 2; }
  # a case that runs the runner itself (26 does, in a harness of its own) gets a fresh
  # serial runner, never this run's worker mode, log directory, pool or report
  ( cd "$T" && unset MJ_TEST_WORKER MJ_TEST_LOGDIR MJ_TEST_JOBS MJ_TEST_REPORT && T="$T" bash -eu "$case" ) || rc=1
  rm -rf "$T"
  return "$rc"
}
verdict() {   # name status seconds phase -> counts it, prints its line, records it
  local name="$1" rc="$2" sec="$3" phase="$4"
  case "$rc" in
    0) pass=$((pass+1)); echo "ok   $name" ;;
    2) fail=$((fail+1)); failed_names="$failed_names $name"; echo "FAIL $name (setup)" ;;
    *) fail=$((fail+1)); failed_names="$failed_names $name"; echo "FAIL $name" ;;
  esac
  [ -n "${report:-}" ] && printf '%s\t%s\t%s\t%s\n' "$name" "$([ "$rc" = 0 ] && echo ok || echo FAIL)" "$sec" "$phase" >> "$report"
  return 0
}

# A worker, spawned by the parallel phase for one case: the log, the status and the
# duration go into files the parent renders; the verdict line is progress, printed as the
# case finishes. It always exits 0: the case's status is the .rc file, and a non-zero exit
# here would make xargs stop dispatching the cases behind it.
if [ "${MJ_TEST_WORKER:-}" = 1 ]; then
  name="$1"; L="$MJ_TEST_LOGDIR"
  t0="$(date +%s)"
  run_case "$ROOT/test/cases/$name.sh" > "$L/$name.log" 2>&1; rc=$?
  sec=$(( $(date +%s) - t0 ))
  printf '%s\n' "$rc" > "$L/$name.rc"; printf '%s\n' "$sec" > "$L/$name.sec"
  [ "$rc" = 0 ] && printf 'ok   %s  %ss\n' "$name" "$sec" || printf 'FAIL %s  %ss\n' "$name" "$sec"
  exit 0
fi

only="${1:-}"
jobs="${MJ_TEST_JOBS:-1}"
case "$jobs" in ''|*[!0-9]*|0) echo "run.sh: MJ_TEST_JOBS must be a positive integer, got '$jobs'" >&2; exit 2 ;; esac
report="${MJ_TEST_REPORT:-}"
[ -n "$report" ] && : > "$report"

# the cases in name order, the exclusive ones told apart by their header, never by a list here
parallel_names=""; exclusive_names=""
for case in "$ROOT"/test/cases/*.sh; do
  name="$(basename "$case" .sh)"
  [ -n "$only" ] && [ "$name" != "$only" ] && continue
  if grep -q '^# majordomus-exclusive:' "$case"; then exclusive_names="$exclusive_names $name"
  else parallel_names="$parallel_names $name"; fi
done

if [ "$jobs" = 1 ]; then
  # ---------------------------------------------------------------- serial
  # Name order, every case streaming its own output as it runs.
  for name in $(printf '%s\n' $parallel_names $exclusive_names | sort); do
    t0="$(date +%s)"
    run_case "$ROOT/test/cases/$name.sh"; rc=$?
    verdict "$name" "$rc" $(( $(date +%s) - t0 )) serial
  done
else
  # ---------------------------------------------------------------- parallel
  L="$(mktemp -d "${TMPDIR:-/tmp}/mj-test-logs.XXXXXX")"
  MJ_TEST_LOGDIR="$L"; export MJ_TEST_LOGDIR
  before="$(git -C "$ROOT" status --porcelain --untracked-files=all 2>/dev/null || true)"
  # xargs bounds the pool: -P workers, one case per worker. bash 3.2 and BSD xargs both
  # have it, and no case is ever started with a bare `&`.
  if [ -n "$parallel_names" ]; then
    echo "run.sh: $(printf '%s\n' $parallel_names | wc -l | tr -d ' ') cases, $jobs at a time"
    printf '%s\n' $parallel_names | MJ_TEST_WORKER=1 xargs -n1 -P"$jobs" bash "$0"
  fi
  after="$(git -C "$ROOT" status --porcelain --untracked-files=all 2>/dev/null || true)"
  dirtied=""
  [ "$before" = "$after" ] || dirtied="$(printf '%s\n%s\n' "$before" "$after" | sort | uniq -u | sed 's/^...//' | sort -u | tr '\n' ' ')"
  if [ -n "$exclusive_names" ]; then
    echo "run.sh: $(printf '%s\n' $exclusive_names | wc -l | tr -d ' ') exclusive cases, one at a time"
    printf '%s\n' $exclusive_names | MJ_TEST_WORKER=1 xargs -n1 -P1 bash "$0"
  fi
  # the verdicts in name order; a failing case's whole log comes right before its line
  echo "run.sh: results"
  for name in $(printf '%s\n' $parallel_names $exclusive_names | sort); do
    phase=parallel; case " $exclusive_names " in *" $name "*) phase=exclusive ;; esac
    rc="$(cat "$L/$name.rc" 2>/dev/null || echo 2)"; sec="$(cat "$L/$name.sec" 2>/dev/null || echo 0)"
    [ "$rc" = 0 ] || [ "$rc" = 2 ] || cat "$L/$name.log" 2>/dev/null
    verdict "$name" "$rc" "$sec" "$phase"
  done
  rm -rf "$L"
  if [ -n "$dirtied" ]; then
    fail=$((fail+1)); failed_names="$failed_names (checkout)"
    echo "FAIL run.sh: the checkout changed during the parallel phase: ${dirtied}— a case that writes into the checkout declares '# majordomus-exclusive: <reason>' and runs alone (or something else edited the checkout while the suite ran)"
  fi
fi
echo "tests: $pass passed, $fail failed"
[ -n "$failed_names" ] && echo "failed:$failed_names"
# A filter that matched nothing is not a pass. `run.sh 51_something` on a repository
# without that case printed "0 passed, 0 failed" and exited 0, and that zero was read as
# success — the same shape as a green CI that never ran the suite. Selecting a case that
# does not exist is a usage error, not an empty success.
if [ -n "$only" ] && [ "$pass" = 0 ] && [ "$fail" = 0 ]; then
  echo "run.sh: no case matches '$only' (test/cases/$only.sh does not exist)" >&2
  exit 2
fi
[ "$pass" = 0 ] && [ "$fail" = 0 ] && { echo "run.sh: no cases found in $ROOT/test/cases" >&2; exit 2; }
[ "$fail" = 0 ]
