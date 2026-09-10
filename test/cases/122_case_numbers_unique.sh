# majordomus-covers: none
# A case number names one case, and the gate that says so.
#
# `bash test/run.sh 96_worktree_topology` runs a case by its number, `test/cases/96_*`
# selects one, and every rule, claim and commit message here says "case 96". All three
# assume the number identifies the case, and on 2026-09-10 nineteen numbers on master were
# held by two to five files each. Parallel sessions allocate from what they can see, and
# what they can see is the tree they branched from: two branches of one fan-out took 110 in
# the same minute, and neither was pushed, so no scan could have found the other.
#
# The gate is a ratchet, because fifty files cannot be renumbered under sessions that are
# mid-flight. This case holds it to both halves: it refuses a new collision, and it does not
# refuse the debt it recorded.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/case-numbers-unique"

# A repository with cases of its own, so the gate is measured by what it decides rather than
# by whatever this checkout happens to contain today.
fixture() {  # fixture <dir> <case file>...
  local d="$1"; shift
  mkdir -p "$d/test/cases" "$d/.ai/repo/ci"
  local f; for f in "$@"; do printf '# a case\ntrue\n' > "$d/test/cases/$f"; done
}

# ---------------------------------------------------------------- every number is its own
T1="$T/unique"; fixture "$T1" 01_alpha.sh 02_beta.sh 03_gamma.sh
expect_exit 0 env MJ_ROOT="$T1" "$GATE"
expect_grep 'no case number is newly shared'
expect_exit 0 env MJ_ROOT="$T1" "$GATE" --strict
expect_grep 'every case number names one case'

# ---------------------------------------------------------------- a second file takes one
T2="$T/collides"; fixture "$T2" 01_alpha.sh 02_beta.sh 02_beta_again.sh
expect_exit 10 env MJ_ROOT="$T2" "$GATE"
expect_grep 'REFUSE'
expect_grep '02_beta.sh'
expect_grep '02_beta_again.sh'
expect_grep 'shares 02 with'
# and it says how to find a free one rather than only that this one is taken
expect_grep 'case-numbers-unique --list'

# ---------------------------------------------------------------- the ratchet
# Today's debt is recorded and stops refusing; nothing else changes.
expect_exit 0 env MJ_ROOT="$T2" "$GATE" --write-baseline
expect_grep 'recorded 2 file'
[ -f "$T2/.ai/repo/ci/case-numbers-baseline.txt" ] || { echo "    the baseline was not written"; exit 1; }
expect_exit 0 env MJ_ROOT="$T2" "$GATE"
expect_grep '2 file.s. of accepted debt'
# --strict still refuses it: the debt is accepted, not forgotten
expect_exit 10 env MJ_ROOT="$T2" "$GATE" --strict
expect_grep '02_beta.sh'

# ---------------------------------------------------------------- the debt may not grow
# A third file joining a collision the baseline already carries is still refused, or the
# ratchet would only slow the rot down.
printf '# a case\ntrue\n' > "$T2/test/cases/02_beta_thrice.sh"
expect_exit 10 env MJ_ROOT="$T2" "$GATE"
expect_grep '02_beta_thrice.sh'
rm "$T2/test/cases/02_beta_thrice.sh"
expect_exit 0 env MJ_ROOT="$T2" "$GATE"

# ---------------------------------------------------------------- a renumbering is welcome
# Removing a collision leaves a baseline that names files which no longer collide, and that
# is not a failure: the list is what may collide, not what must.
git -C "$T2" init -q . 2>/dev/null || true
mv "$T2/test/cases/02_beta_again.sh" "$T2/test/cases/04_beta_again.sh"
expect_exit 0 env MJ_ROOT="$T2" "$GATE"
expect_exit 0 env MJ_ROOT="$T2" "$GATE" --strict
expect_grep 'every case number names one case'

# ---------------------------------------------------------------- the deliberate second half
# `87_release_pipeline.sh` and `87b_release_archive_shape.sh` are one number and its second
# half, on purpose. The gate reads the text before the first underscore verbatim, so it
# never argues with a convention the repository already uses.
T3="$T/suffixed"; fixture "$T3" 87_release_pipeline.sh 87b_release_archive_shape.sh
expect_exit 0 env MJ_ROOT="$T3" "$GATE" --strict
expect_grep 'every case number names one case'

# ---------------------------------------------------------------- and it lists what is taken
T4="$T/listing"; fixture "$T4" 01_a.sh 01_b.sh 02_c.sh
expect_exit 0 env MJ_ROOT="$T4" "$GATE" --list
expect_grep '01_a.sh 01_b.sh'
expect_no_grep '02_c.sh'

echo "    a case number names one case: a new collision is refused, today's debt is recorded and may not grow, and 87b is not a duplicate of 87"
