# majordomus-covers: none
# The gate of "a whole file's JSON may not travel to jq on the command line", driven against
# fixture trees as well as against this checkout.
#
# Why this is a source scan and not a runtime probe: Linux caps a SINGLE argument at
# MAX_ARG_STRLEN — 32 pages, 131072 bytes — separately from ARG_MAX, and macOS applies no
# such per-argument cap at all (its ARG_MAX is 1048576, for the whole list). A probe that
# built a 131 KB argument and ran it would therefore pass on every machine this repository is
# written on, and fail only in CI. The failure this case exists to prevent is invisible here,
# so what is asserted is the SHAPE of the call, which is decidable on both machines.
#
# The defect that motivated it: scripts/generate-site-data passed the claims matrix as one
# `--argjson`. It was 129593 bytes on master and 131363 once two claims were added, so it
# crossed the cap by 291 bytes; jq never ran, the generator exited 13, and a gate, a CI job
# and twelve suite cases failed downstream of a message that named none of it.
#
# The refusals are exercised by planting the violation, because a check nobody has watched
# fail is a check nobody knows works. The non-refusals matter as much: a gate that flagged
# every `--arg` fed from a shell helper would be turned off within a week, since a scalar's
# size is bounded by what it names.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/argv-limit-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# A tree of the shape the gate reads: shell scripts, found by their shebang.
fixture() { # <n> <body of the jq call>
  local f="$T/tree$1"
  rm -rf "$f"; mkdir -p "$f/scripts" "$f/lib" "$f/bin"
  { printf '#!/usr/bin/env bash\n'; printf 'set -eu\n'; printf '%s\n' "$2"; } > "$f/scripts/gen"
  chmod +x "$f/scripts/gen"
  printf '%s' "$f"
}

# 1. this checkout is clean, and says so rather than saying nothing
expect_exit 0 "$GATE"
expect_grep 'no whole-file payload reaches jq on a command line'

# 2. the defect's own shape is refused: a whole collection read from a file into argv
t="$(fixture A 'jq -S --argjson claims "$(jq -c '"'"'.claims'"'"' "$OUT/capabilities.json")" -f x.jq "$REG" > out.json')"
MJ_ROOT="$t" expect_exit 10 "$GATE"
expect_grep 'scripts/gen:3'
expect_grep '131072 bytes \(MAX_ARG_STRLEN\)'
expect_grep 'slurpfile'

# 3. the other spelling of the same mistake: cat
t="$(fixture B 'jq -n --arg blob "$(cat "$OUT/big.json")" ".")'
)"
MJ_ROOT="$t" expect_exit 10 "$GATE"
expect_grep 'scripts/gen:3'

# 3b. the spelling the check once missed: git's listing of tracked paths as one argument.
#     generate-site-data's forge_paths did exactly this; the listing was 130298 bytes on
#     master and 131102 on the branch that crossed the cap, and the site job failed on Linux.
t="$(fixture B2 'jq -Rn --arg files "$(git -C "$ROOT" ls-files)" "[inputs]"')"
MJ_ROOT="$t" expect_exit 10 "$GATE"
expect_grep 'scripts/gen:3'
# ...and its correction, the listing read from the file it was written to, is accepted
t="$(fixture B3 'jq -Rn --rawfile files "$LINK_TRACKED" "[inputs]"')"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 4. the correction is accepted — this is what the fixed generator does
t="$(fixture C 'jq -S --slurpfile caps "$OUT/capabilities.json" -f x.jq "$REG" > out.json')"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 5. a bounded scalar is deliberately NOT refused: its size is what it names, and refusing
#    these would make the gate a sweep over every generator in the tree rather than a ratchet
t="$(fixture D 'jq -n --arg repo_url "$(jq -r '"'"'.repository_url'"'"' "$OUT/project.json")" --argjson n "$(yv "$P" version)" ".")'
)"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 6. a tree with no shell script at all is clean, not unusable
t="$T/treeE"; rm -rf "$t"; mkdir -p "$t/scripts"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 7. a root that does not exist is unusable, and says so — never a pass
MJ_ROOT="$T/nope" expect_exit 12 "$GATE"

# 8. the gate carries no copy of the call it looks for: a scan that matched its own source
#    would refuse itself the day somebody quoted an example into it
grep -qE -- '--arg(json)? [A-Za-z_][A-Za-z0-9_]* "\$\((jq [^)]*-c|cat |git [^)]*ls-files)' "$GATE" \
  && { echo "    the gate contains the very construct it refuses, so it would flag itself"; exit 1; }
true
