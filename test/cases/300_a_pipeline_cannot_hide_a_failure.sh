# majordomus-covers: none
# The gate of the pipefail ratchet, driven against fixture trees rather than against this
# checkout: a case that asserted this repository's own exposed set would be a copy of the
# baseline, and would go red the day somebody legitimately adds `set -o pipefail` to a
# script. What is asserted here is the gate's behaviour — what it refuses, what it lets
# through, and what it says while doing it.
#
# The gate existed and nothing had ever seen it fail. That is the defect this case fixes:
# `scripts/ci/pipefail-check` was named by no test in the suite, so the proposition "a new
# script that hides a pipeline's failure is refused" was a claim about a program nobody had
# run in anger. Every section below writes the violation rather than describing it.
#
# The three propositions the gate exists for:
#
#   1. the denominator is measured from the tree — a shell script that sets -e and pipes is
#      measured the day it lands, and nothing else is measured at all;
#   2. a script the baseline does not carry is NEW debt and is refused by name;
#   3. the ratchet reports in the other direction too — a baseline entry that has since
#      been fixed is named, without failing, so the baseline can be tightened deliberately.
#
# Proposition 1 is the one a reader is most likely to doubt, so it is exercised from both
# sides: a script that pipes without `set -e` is not the gate's business, and a file that is
# not a shell script is not either, however many pipes it contains.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/pipefail-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# A script under one of the three measured trees. `exposed` pipes under `set -e` with no
# pipefail; `guarded` is the same script written correctly.
exposed_script() {   # exposed_script <file>
  mkdir -p "$(dirname "$1")"
  printf '#!/usr/bin/env bash\nset -eu\ncurl -sf https://example.invalid | tail -5\n' > "$1"
  chmod +x "$1"
}
guarded_script() {   # guarded_script <file>
  mkdir -p "$(dirname "$1")"
  printf '#!/usr/bin/env bash\nset -eu\nset -o pipefail\ncurl -sf https://example.invalid | tail -5\n' > "$1"
  chmod +x "$1"
}

# A tree of the shape the gate reads: the three directories it measures, one script written
# correctly, one written the old way, and a baseline recording exactly the second. The
# baseline is written by the gate itself — a baseline typed here would be this case's
# opinion of the gate's format rather than the gate's own.
fixture() {   # fixture <n> -> prints the tree
  F="$T/tree$1"
  mkdir -p "$F/scripts" "$F/lib" "$F/bin" "$F/.ai/repo"
  guarded_script "$F/scripts/safe"
  exposed_script "$F/scripts/legacy"
  MJ_ROOT="$F" "$GATE" --write-baseline >/dev/null
  printf '%s' "$F"
}

# ---------------------------------------------------------------- the tree it accepts
F="$(fixture 0)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 of 2 script\(s\) exposed, baseline 1 — no new debt'

# and the baseline it wrote is the measurement, not a list somebody kept: the correct
# script is absent from it and the old one is in it, by path
expect_grep '^scripts/legacy$' "$F/.ai/repo/pipefail-baseline.txt"
expect_no_grep '^scripts/safe$' "$F/.ai/repo/pipefail-baseline.txt"

# ---------------------------------------------------------------- 1. the denominator
# A script that pipes and does not set -e is not exposed to anything: without -e the
# pipeline's status was never going to end the script, so there is nothing to hide.
F="$(fixture 1)"
mkdir -p "$F/lib"
printf '#!/usr/bin/env bash\nls | wc -l\n' > "$F/lib/unguarded.sh"
chmod +x "$F/lib/unguarded.sh"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 of 2 script\(s\) exposed'

# A file that is not a shell script is not measured however it is written — the gate reads
# the shebang, not the extension and not a list. A python program that sets -e in a string
# and pipes in a comment is not a finding.
F="$(fixture 2)"
printf '#!/usr/bin/env python3\n# set -eu and a | pipe, in a language that has neither\nprint("set -e | tail")\n' \
  > "$F/scripts/report"
chmod +x "$F/scripts/report"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 of 2 script\(s\) exposed'

# And a script that sets -e and never pipes is not measured either, so adding one does not
# move the denominator.
F="$(fixture 3)"
printf '#!/usr/bin/env bash\nset -eu\necho hello\n' > "$F/bin/quiet"
chmod +x "$F/bin/quiet"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 of 2 script\(s\) exposed'

# ---------------------------------------------------------------- 2. new debt is refused
# The violation the gate exists for, written rather than described: a script that pipes a
# failing producer into `tail` under `set -e`, so the script's exit status is tail's. This
# is not hypothetical — a deploy was read as having succeeded exactly this way.
F="$(fixture 4)"
exposed_script "$F/bin/publish"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep "a pipeline's failure would be hidden here"
expect_grep 'NEW  bin/publish pipes under set -e without pipefail'
expect_grep "add 'set -o pipefail' beside the script's 'set -e'"

# the same file, written correctly, is accepted: what the gate refuses is the missing
# setting and not the pipeline
guarded_script "$F/bin/publish"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 of 3 script\(s\) exposed, baseline 1 — no new debt'

# and removing it restores the tree the gate accepted at the start
rm -f "$F/bin/publish"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 of 2 script\(s\) exposed, baseline 1 — no new debt'

# A directory the gate does not measure is not a hiding place either way: the same script
# under docs/ is neither refused nor counted, because the denominator is scripts, lib, bin.
F="$(fixture 5)"
exposed_script "$F/docs/publish"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 of 2 script\(s\) exposed'

# ---------------------------------------------------------------- 3. the other direction
# A baseline entry that has since been fixed is reported and does not fail. This is the
# half that keeps the ratchet honest: without it the baseline only ever records debt and
# never learns that debt was paid, and a "tighten the baseline" commit has no prompt.
F="$(fixture 6)"
guarded_script "$F/scripts/legacy"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'FIXED scripts/legacy — tighten the baseline'
expect_grep '0 of 2 script\(s\) exposed'

# a baseline entry whose file is gone is reported the same way, rather than crashing on a
# path that no longer resolves
F="$(fixture 7)"
rm -f "$F/scripts/legacy"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'FIXED scripts/legacy'

# ---------------------------------------------------------------- --strict
# The sweep the ratchet is deliberately not. It is what a repository runs on the day it
# decides to pay the whole debt, and it must refuse a tree the default mode accepts — or
# the two modes are the same mode.
F="$(fixture 8)"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --strict
expect_grep "1 of 2 script\\(s\\) still hide a pipeline's failure \\(--strict\\)"
guarded_script "$F/scripts/legacy"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --strict

# ---------------------------------------------------------------- an unusable tree
# No baseline is not "clean". A gate whose baseline is missing knows nothing about the tree
# it is measuring, and a check that reports success from no information is worse than no
# check: 12 is the exit that says so, and it is not the 10 that means a finding.
F="$(fixture 9)"
rm -f "$F/.ai/repo/pipefail-baseline.txt"
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'no baseline at .ai/repo/pipefail-baseline.txt \(run --write-baseline\)'

# and an option it does not know is a usage error, not a verdict about the tree
expect_exit 2 env MJ_ROOT="$F" "$GATE" --sweep
expect_grep 'unknown option --sweep'

echo "    the pipefail ratchet refuses new debt, reports paid debt, and measures its own denominator"
