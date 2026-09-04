# majordomus-covers: none
#
# Every scenario a command page demonstrates is executed here against the real binary.
#
# This is the case that stops the website becoming a second implementation. A demonstration
# is a projection of behaviour, not a description of it, so the same fixture that renders the
# demonstration is the fixture that runs: same setup script, same argument vector, same
# expected exit code and output. A page cannot drift from the tool without this going red.
#
# A fixture carries no shell. Preparation is a real script under test/fixtures/commands/setup/,
# which the page displays and this case sources, so nothing here executes a string from data.
. "$ROOT/test/lib.sh"
FIX="$ROOT/test/fixtures/commands"
FIXTURE_SETUP="$FIX/setup"; export FIXTURE_SETUP
[ -d "$FIX" ] || { echo "    no fixtures at test/fixtures/commands/"; exit 1; }
command -v jq >/dev/null || { echo "    jq is required to read the fixtures"; exit 1; }

# The repository this case runs in is the disposable one test/run.sh created; each scenario
# needs its own, because a scenario's setup assumes a repository nothing has touched.
BASE="$T"
n=0 files=0

for fx in "$FIX"/*.json; do
  [ -f "$fx" ] || continue
  files=$((files + 1))
  cmd="$(jq -r .command "$fx")"
  fxname="$(basename "$fx" .json)"
  [ "$cmd" = "$fxname" ] || { echo "    $fxname.json declares command '$cmd'"; exit 1; }
  [ "$(jq -r .schema "$fx")" = 1 ] || { echo "    $fxname.json is not schema 1"; exit 1; }
  [ "$(jq -r '.scenarios | length' "$fx")" -ge 2 ] || {
    echo "    $fxname.json has fewer than two scenarios; one scenario demonstrates nothing"; exit 1; }

  ids="$(jq -r '.scenarios[].id' "$fx")"
  [ -z "$(printf '%s\n' "$ids" | sort | uniq -d)" ] || { echo "    $fxname.json has duplicate scenario ids"; exit 1; }

  for sid in $ids; do
    n=$((n + 1))
    setup="$(jq -r --arg s "$sid" '.scenarios[] | select(.id==$s) | .setup' "$fx")"
    [ -f "$FIXTURE_SETUP/$setup.sh" ] || {
      echo "    $fxname/$sid names setup '$setup', which has no script in test/fixtures/commands/setup/"; exit 1; }
    want_exit="$(jq -r --arg s "$sid" '.scenarios[] | select(.id==$s) | .expect.exit' "$fx")"
    [ -n "$(jq -r --arg s "$sid" '.scenarios[] | select(.id==$s) | .explain' "$fx")" ] || {
      echo "    $fxname/$sid has no explanation; a demonstration that says nothing teaches nothing"; exit 1; }

    # a repository of its own, so scenarios cannot depend on each other's order. The
    # runner's own scratch files live outside it: a file this case wrote into the
    # repository would be counted as the task's work and fail its own scope check.
    W="$BASE/repos/$fxname-$sid"
    SCRATCH="$BASE/scratch"
    mkdir -p "$W" "$SCRATCH"
    ( cd "$W" && git init -q . && git config user.email t@example.com && git config user.name t ) || {
      echo "    could not create a repository for $fxname/$sid"; exit 1; }

    # argv is read as JSON and passed as arguments, never spliced into a command line
    argv_file="$SCRATCH/argv"
    jq -r --arg s "$sid" '.scenarios[] | select(.id==$s) | .run[]' "$fx" > "$argv_file"
    set --
    while IFS= read -r a; do set -- "$@" "$a"; done < "$argv_file"
    rm -f "$argv_file"

    # commands that read a body take it from a file, named by the scenario; the page shows
    # the same file, so what is demonstrated is what was run
    stdin="$(jq -r --arg s "$sid" '.scenarios[] | select(.id==$s) | .stdin // ""' "$fx")"
    if [ -n "$stdin" ]; then
      [ -f "$FIX/stdin/$stdin" ] || { echo "    $fxname/$sid names stdin '$stdin', which does not exist"; exit 1; }
    fi

    out="$SCRATCH/out"; rc=0
    # the setup script is named by the fixture, so its path is not a constant
    # shellcheck disable=SC1090
    if [ -n "$stdin" ]; then
      ( cd "$W" && . "$FIXTURE_SETUP/$setup.sh" >/dev/null 2>&1 && "$MJ" "$@" < "$FIX/stdin/$stdin" ) > "$out" 2>&1 || rc=$?
    else
      ( cd "$W" && . "$FIXTURE_SETUP/$setup.sh" >/dev/null 2>&1 && "$MJ" "$@" ) > "$out" 2>&1 || rc=$?
    fi

    if [ "$rc" != "$want_exit" ]; then
      echo "    $fxname/$sid: expected exit $want_exit, got $rc"
      echo "      run: majordomus $*"
      sed 's/^/      | /' "$out"
      exit 1
    fi
    jq -r --arg s "$sid" '.scenarios[] | select(.id==$s) | .expect.stdout_contains[]' "$fx" > "$SCRATCH/want"
    while IFS= read -r want; do
      [ -n "$want" ] || continue
      grep -qE -- "$want" "$out" || {
        echo "    $fxname/$sid: expected /$want/ in the output of: majordomus $*"
        sed 's/^/      | /' "$out"
        exit 1; }
    done < "$SCRATCH/want"
    rm -rf "$W"
  done
done

[ "$files" -gt 0 ] || { echo "    no fixture files found"; exit 1; }

# The check is not vacuous: a scenario whose expectation is wrong must fail. Proved by
# running one real scenario against a deliberately wrong expected exit code.
probe="$T/probe.json"
jq '.scenarios = [.scenarios[0]] | .scenarios[0].expect.exit = 99' "$FIX/finish.json" > "$probe"
[ "$(jq -r '.scenarios[0].expect.exit' "$probe")" = 99 ] || { echo "    the probe did not take"; exit 1; }
W="$T/repos/probe-run"; SCRATCH="$T/scratch"; mkdir -p "$W" "$SCRATCH"
( cd "$W" && git init -q . && git config user.email t@example.com && git config user.name t ) >/dev/null
setup="$(jq -r '.scenarios[0].setup' "$probe")"
# shellcheck disable=SC1090
set --
jq -r '.scenarios[0].run[]' "$probe" > "$SCRATCH/argv"
while IFS= read -r a; do set -- "$@" "$a"; done < "$SCRATCH/argv"
rc=0
# shellcheck disable=SC1090
( cd "$W" && . "$FIXTURE_SETUP/$setup.sh" >/dev/null 2>&1 && "$MJ" "$@" ) >/dev/null 2>&1 || rc=$?
[ "$rc" != 99 ] || { echo "    the exit-code assertion is vacuous: the probe matched 99"; exit 1; }
rm -rf "$W"

printf '    %s scenarios across %s command fixtures, all executed\n' "$n" "$files"
