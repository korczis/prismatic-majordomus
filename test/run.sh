#!/usr/bin/env bash
# Runs every test/cases/*.sh in a disposable temporary repository each.
# A case script gets: $MJ (path to bin/majordomus), $T (its scratch dir, already a git repo).
# Helpers come from test/lib.sh, which every case sources first.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MJ="$ROOT/bin/majordomus"; export MJ ROOT
pass=0; fail=0; failed_names=""

only="${1:-}"
for case in "$ROOT"/test/cases/*.sh; do
  name="$(basename "$case" .sh)"
  [ -n "$only" ] && [ "$name" != "$only" ] && continue
  T="$(mktemp -d "${TMPDIR:-/tmp}/mj-test.XXXXXX")"
  ( cd "$T" && git init -q . && git config user.email t@example.com && git config user.name t \
    && git commit -q --allow-empty -m init ) || { echo "FAIL $name (setup)"; fail=$((fail+1)); continue; }
  if ( cd "$T" && T="$T" bash -eu "$case" ); then pass=$((pass+1)); echo "ok   $name"
  else fail=$((fail+1)); failed_names="$failed_names $name"; echo "FAIL $name"; fi
  rm -rf "$T"
done
echo "tests: $pass passed, $fail failed"
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
