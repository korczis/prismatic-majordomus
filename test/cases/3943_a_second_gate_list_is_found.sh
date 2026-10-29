# majordomus-covers: none
# A second list of gates is found when it is put back (I3943, I2350 acceptance criterion 2).
#
# On 2026-10-23 the Plan agent found three scripts that carried a list of gates beside the
# model: scripts/ci/core-check, the recipe `ci-structure` in .just/ci.just, which ran shell-lint
# and core-check by hand and called them "the gates every plan runs", and the pre-push hook.
# Case 77 had already held one such recipe to the dispatcher; nothing looked for the next.
# `scripts/ci-plan --inventory` does: a tracked file outside the model that runs the commands of
# two gates near each other, or enumerates three gate ids on a line, is a finding, and a finding
# the baseline (.ai/repo/ci/gate-list-baseline.txt) does not hold fails, as does a baseline entry
# that matches nothing.
#
# The fixtures are tiny repositories carrying the repository's real model, and the gate ids and
# commands they use are read from it, so a gate renamed or re-commanded cannot leave this case
# testing a list that no longer is one. The last two sections are the repository's own tree, and
# the mutation: the list is put back into .just/ci.just in a copy, and the inventory must find it.
. "$ROOT/test/lib.sh"
PLAN="$ROOT/scripts/ci-plan"; MODEL="$ROOT/.ai/repo/ci/gates.yaml"
command -v jq >/dev/null 2>&1 || skip "jq absent"
command -v git >/dev/null 2>&1 || skip "git absent"
[ -x "$PLAN" ] || { echo "    scripts/ci-plan is missing or not executable"; exit 1; }

# the command the model gives a gate, never typed here
"$PLAN" --format json --files /dev/null > "$T/plan.json" || { echo "    the plan could not be computed"; exit 1; }
runs() { jq -r --arg g "$1" '.gates[] | select(.id == $g) | .runs' "$T/plan.json"; }
LINT="$(runs shell-lint)"; CORE="$(runs core-check)"; PRIV="$(runs prompt-privacy)"
[ -n "$LINT" ] && [ -n "$CORE" ] && [ -n "$PRIV" ] || { echo "    the model no longer declares shell-lint, core-check and prompt-privacy; name three gates it does"; exit 1; }

# a fixture: a tiny repository (the inventory reads tracked files), and a baseline of its own
fx() { rm -rf "$T/fx"; mkdir -p "$T/fx"; git -C "$T/fx" init -q; : > "$T/none.txt"; }
track() { git -C "$T/fx" add -A; }
inv() { "$PLAN" --inventory --model "$MODEL" --tree "$T/fx" --baseline "${1:-$T/none.txt}"; }

# ---------------------------------------------------------------- the clean repository passes
fx
printf '#!/usr/bin/env bash\nset -eu\n%s\n' "$LINT" > "$T/fx/one-gate.sh"
# two gates in a sentence are a mention, and two ids on a line are not a list of three
printf 'Run %s after %s, and read the %s summary.\n# %s, then %s\n' "$LINT" "$CORE" "$PRIV" "$PRIV" "$CORE" > "$T/fx/prose.md"
track
expect_exit 0 inv || exit 1
expect_grep 'inventory: 0 finding\(s\), 0 in the baseline, 0 to fix' || exit 1

# ---------------------------------------------------------------- a script that runs two gates' commands
fx
printf '#!/usr/bin/env bash\nset -eu\n%s\n%s\n' "$LINT" "$CORE" > "$T/fx/ci.sh"
track
expect_exit 10 inv || exit 1
expect_grep '^ci\.sh:3: runs the commands of 2 gates: core-check, shell-lint \(not in the baseline\)$' || exit 1
expect_grep 'inventory: 1 finding\(s\), 0 in the baseline, 1 to fix' || exit 1
# a list is commands near each other: the same two commands thirty lines apart are two scripts' worth
{ printf '#!/usr/bin/env bash\n%s\n' "$LINT"; i=0; while [ "$i" -lt 30 ]; do printf 'true\n'; i=$((i+1)); done; printf '%s\n' "$CORE"; } > "$T/fx/ci.sh"
track
expect_exit 0 inv || exit 1
# a command that is only quoted in a comment is not run
printf '#!/usr/bin/env bash\n%s\n# %s\n' "$LINT" "$CORE" > "$T/fx/ci.sh"
track
expect_exit 0 inv || exit 1
# one command that contains another is the longer gate alone: ui-integrity runs the command of
# cockpit-projection with a flag, and one invocation of it is one gate
a="$(runs cockpit-projection)"; b="$(runs ui-integrity)"
case "$b" in "$a"*) printf '#!/usr/bin/env bash\n%s\n' "$b" > "$T/fx/ci.sh"; track; expect_exit 0 inv || exit 1 ;; esac

# ---------------------------------------------------------------- a line naming three gate ids
fx
printf '#!/usr/bin/env bash\nfor g in shell-lint core-check prompt-privacy; do echo "$g"; done\n' > "$T/fx/each.sh"
track
expect_exit 10 inv || exit 1
expect_grep '^each\.sh:2: lists 3 gates: core-check, prompt-privacy, shell-lint \(not in the baseline\)$' || exit 1
# two ids are not three, and three ids spread over a sentence are prose, not an enumeration
printf 'for g in shell-lint core-check; do\nthe shell-lint gate was slower than the core-check gate until the prompt-privacy gate\n' > "$T/fx/each.sh"
track
expect_exit 0 inv || exit 1

# ---------------------------------------------------------------- what the inventory does not read
fx
# the records directory is spelled in two parts: case 94 gives every case that names a plan
# record path to plan-record-cases, and this one only needs a directory of that name
rec="$T/fx/.ai/repo/project"
mkdir -p "$T/fx/test/cases" "$T/fx/.github/workflows" "$T/fx/docs/generated" "$rec/issues" "$T/fx/.ai/repo/ci"
printf '%s\n%s\n' "$LINT" "$CORE" > "$T/fx/test/cases/1_x.sh"
printf '%s\n%s\n' "$LINT" "$CORE" > "$T/fx/docs/generated/x.txt"
printf '%s\n%s\n' "$LINT" "$CORE" > "$rec/issues/I1.yaml"
printf '%s\n%s\n' "$LINT" "$CORE" > "$T/fx/.ai/repo/ci/gates.yaml"
# the workflow's reads of the plan's outputs are held by case 26_ci_wiring
printf '      - run: %s\n        if: needs.plan.outputs.shell_lint == true && needs.plan.outputs.core_check == true\n' "$LINT" > "$T/fx/.github/workflows/validate.yml"
printf '      - run: %s needs.plan.outputs.x\n      - run: %s needs.plan.outputs.y\n' "$LINT" "$CORE" >> "$T/fx/.github/workflows/validate.yml"
track
expect_exit 0 inv || exit 1

# ---------------------------------------------------------------- the baseline: a ratchet that only shrinks
fx
printf '#!/usr/bin/env bash\n%s\n%s\n' "$LINT" "$CORE" > "$T/fx/ci.sh"
track
printf '# a reason\nci.sh:runs\n' > "$T/held.txt"
expect_exit 0 inv "$T/held.txt" || exit 1
expect_grep 'inventory: 1 finding\(s\), 1 in the baseline, 0 to fix' || exit 1
# the entry is for the kind of finding: a different file, or the other kind, is not held
printf 'other.sh:runs\nci.sh:names\n' > "$T/other.txt"
expect_exit 10 inv "$T/other.txt" || exit 1
expect_grep 'not in the baseline' || exit 1
expect_grep '^STALE other\.sh:runs' || exit 1
expect_grep '^STALE ci\.sh:names' || exit 1
# the list was fixed and the entry stayed: the baseline can only shrink
fx
printf '#!/usr/bin/env bash\n%s\n' "$LINT" > "$T/fx/ci.sh"
track
expect_exit 10 inv "$T/held.txt" || exit 1
expect_grep '^STALE ci\.sh:runs: the baseline holds an entry that matches nothing; delete the line$' || exit 1
expect_no_grep 'not in the baseline' || exit 1

# ---------------------------------------------------------------- what it cannot read it says
expect_exit 12 "$PLAN" --inventory --model "$MODEL" --tree "$T/fx" --baseline "$T/does-not-exist.txt" || exit 1
expect_grep 'no baseline at' || exit 1
# $T is itself a repository, so git is told not to look for one above the directory
mkdir -p "$T/notgit"; export GIT_CEILING_DIRECTORIES="$T"
expect_exit 12 "$PLAN" --inventory --model "$MODEL" --tree "$T/notgit" --baseline "$T/none.txt" || exit 1
expect_grep 'is not a git checkout' || exit 1
unset GIT_CEILING_DIRECTORIES

# ---------------------------------------------------------------- the repository's own tree passes
cd "$ROOT" || exit 1
expect_exit 0 "$PLAN" --inventory || { cd "$T" || true; exit 1; }
expect_grep 'inventory: [0-9]+ finding\(s\), [0-9]+ in the baseline, 0 to fix' || { cd "$T" || true; exit 1; }
cd "$T" || exit 1
# the baseline is a header and entries, and every entry names a file that is tracked
grep -v '^#' "$ROOT/.ai/repo/ci/gate-list-baseline.txt" | grep . | while IFS= read -r entry; do
  f="${entry%:*}"; w="${entry##*:}"
  case "$w" in runs|names) ;; *) echo "    baseline entry '$entry' is not file:runs or file:names"; exit 1 ;; esac
  git -C "$ROOT" ls-files --error-unmatch -- "$f" >/dev/null 2>&1 || { echo "    baseline entry '$entry' names a file that is not tracked"; exit 1; }
done || exit 1

# ---------------------------------------------------------------- the mutation: put the list back
# `ci-structure` in .just/ci.just once ran shell-lint and core-check by hand. The real recipe
# passes; the same file with the two commands back in it is found.
fx
mkdir -p "$T/fx/.just"
cp "$ROOT/.just/ci.just" "$T/fx/.just/ci.just"
track
expect_exit 0 inv || { echo "    the recipe as it stands is a list of gates"; exit 1; }
awk -v lint="$LINT" -v core="$CORE" '
  /^ci-structure:/ { print; print "    " lint; print "    " core; skip = 1; next }
  skip && /^    / { next }
  { skip = 0; print }' "$ROOT/.just/ci.just" > "$T/fx/.just/ci.just"
grep -q "^    $CORE\$" "$T/fx/.just/ci.just" && grep -q "^    $LINT\$" "$T/fx/.just/ci.just" \
  || { echo "    the mutation did not take"; exit 1; }
track
expect_exit 10 inv || exit 1
expect_grep '^\.just/ci\.just:[0-9]+: runs the commands of 2 gates: core-check, shell-lint ' || exit 1
