# majordomus-covers: none
# `doctor`'s `clone unpushed` states what kind of work exists on one disk, not just how much.
#
# The old line printed one number per branch and nothing about what the number was of. On
# 2026-09-12 it read:
#
#   WARN clone unpushed — commits on no remote: feature/cooperation-is-repository-wide(1)
#        feature/landing-closure(4) fix/the-order-ratchet-is-paid-not-moved(22) …
#
# and it sent a session to the edge of pushing another session's orphaned branch to rescue
# nothing. Measured the same afternoon, on the same refs:
#
#   - fix/the-order-ratchet-is-paid-not-moved held ZERO commits that exist nowhere. The 22
#     were master's own, arriving through a local `Merge remote-tracking branch
#     'origin/master'` — every one of them already on the remote. `@{upstream}..<b>` reads
#     22 there; `<b> --not --remotes`, the ref-set question, reads 1.
#   - that remaining 1, and the only unique commit of a second branch, was that merge
#     commit. A merge of already-pushed history is a gesture; re-merging costs a command.
#   - the single genuine survivor, int/entities-are-routable, held one unique non-merge
#     commit: `chore(derive): regenerate the derived artifacts for the merge result`, over
#     docs/generated/, site/content/ and two status files. Regenerable by definition.
#
# Real exposure: zero authored commits, reported as four branches and twenty-eight commits.
# A count with no denominator — the number was true about a question nobody asked.
#
# So this case proves the three-way split, and proves the WARN stays loud for the one case
# that matters. The fixture is three branches, one per class, and each mutation is asserted
# to have applied against git before any verdict is read: a fixture that silently no-ops
# makes the suite the lying instrument, and one was caught doing exactly that today.
#
# It also proves the classifier does not carry its own idea of "generated": the last block
# removes the `merge=derived` declaration from the fixture and the derive branch flips to at
# risk. lib/changed.sh reads the five declarations that already say what is derived — among
# them the .gitattributes block scripts/gitattributes writes out of
# docs/generated/artifacts.json and scripts/generate-site-data. A second hand-kept copy of
# that set is its own defect.
#
# The rule is project.a-verdict-states-its-subject.
. "$ROOT/test/lib.sh"

expect_file "$ROOT/lib/doctor.sh"
expect_file "$ROOT/lib/changed.sh"
# The helper is the other subsystem's; this case fails loudly rather than quietly growing a
# second classifier if it ever moves.
grep -q 'mj_changed_is_derived()' "$ROOT/lib/changed.sh" \
  || { echo "    lib/changed.sh no longer defines mj_changed_is_derived"; exit 1; }
grep -q 'mj_changed_is_derived' "$ROOT/lib/doctor.sh" \
  || { echo "    lib/doctor.sh does not classify through mj_changed_is_derived"; exit 1; }

export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.invalid
export GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.invalid
G="git -c commit.gpgsign=false -c protocol.file.allow=always"

# ------------------------------------------------------------------------ the fixture
$G init -q --bare -b master "$T/remote"
$G init -q -b master "$T/clone"
cd "$T/clone"
mkdir -p docs/generated lib
printf 'docs/generated/**  merge=derived\n' > .gitattributes
printf 'base\n' > lib/base.sh
$G add -A && $G commit -qm base
$G remote add origin "$T/remote"
$G push -q -u origin master

# master advances on the remote, through a second clone, so the fixture's local merge of
# origin/master brings commits that really are pushed — the whole point of branch (a).
$G clone -q "$T/remote" "$T/other"
( cd "$T/other" && printf 'more\n' >> lib/base.sh && $G commit -qam "master moves" && $G push -q origin master )
$G fetch -q origin

# (a) its only unique commit is a local merge of already-pushed history
$G checkout -qb merge-only master
$G merge -q --no-ff -m "Merge remote-tracking branch 'origin/master' into merge-only" origin/master
# (b) its only unique commit is a derive, touching only declared-derived paths
$G checkout -q -b derive-only master
printf '{"a":1}\n' > docs/generated/artifacts.json
$G add -A && $G commit -qm "chore(derive): regenerate the derived artifacts"
# (c) one authored source commit that exists on no remote
$G checkout -q -b authored master
printf 'work\n' > lib/thing.sh
$G add -A && $G commit -qm "feat: real work"
$G checkout -q master
cd "$ROOT"

# ------------------------------------------------- every mutation applied, proved in git
# Asserted before the verdict is read and not after: a patch that no-ops leaves a verdict
# that is correct about a fixture nobody built, and reading it in that order cannot tell.
assert_shape() {
  local b="$1" want_all="$2" want_nm="$3" a n
  a="$($G -C "$T/clone" rev-list --count "$b" --not --remotes)"
  n="$($G -C "$T/clone" rev-list --count --no-merges "$b" --not --remotes)"
  [ "$a" = "$want_all" ] && [ "$n" = "$want_nm" ] && return 0
  printf '    fixture branch %s did not apply: %s unique / %s non-merge, wanted %s / %s\n' \
    "$b" "$a" "$n" "$want_all" "$want_nm"
  return 1
}
assert_shape merge-only  1 0 || exit 1
assert_shape derive-only 1 1 || exit 1
assert_shape authored    1 1 || exit 1
# and the derive branch's file really is claimed by the fixture's own declaration
[ "$($G -C "$T/clone" check-attr merge -- docs/generated/artifacts.json)" \
  = "docs/generated/artifacts.json: merge: derived" ] \
  || { echo "    the fixture's merge=derived declaration did not apply"; exit 1; }

# ------------------------------------------------------------------------- the verdict
# doctor's own code, against the fixture's refs. MJ_SCOPE_FILE and the policy are cleared so
# that the fixture's .gitattributes is the *only* thing that can answer "is this generated" —
# if the answer were hardcoded in lib/, the last block below could not move it.
verdict() {
  ( set +u
    export MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" MJ_VERSION=test MJ_SELF="$ROOT/bin/majordomus"
    . "$ROOT/lib/common.sh"
    . "$ROOT/lib/changed.sh"
    . "$ROOT/lib/doctor.sh"
    MJ_ROOT="$T/clone"; MJ_SCOPE_FILE=""; MJ_POLICY_FILE=""
    MJ_STATE_DIR="$T/clone/.ai/local/state"
    mj_report_clone ) 2>&1
}
out="$T/verdict.txt"
verdict > "$out"
cat "$out"

# the authored branch is the WARN's subject, with all three quantities stated
expect_grep "^WARN +clone" "$out"
expect_grep "authored\(1 authored of 1 non-merge of 1 on no remote\)" "$out"
# and the two that are not exposure are not named as exposure
expect_no_grep "merge-only\(" "$out"
expect_no_grep "derive-only\(" "$out"
# they are reported as measured and explicitly not at risk, not silently dropped
expect_grep "2 branch\(es\) hold only merges or regenerable output and are not at risk" "$out"

# ---------------------------------------------- the WARN stays loud when it should: proof
# The inverse risk of this change is that it makes the line quieter. A branch with authored
# work on no remote must still produce a WARN naming it, and nothing else here does.
grep -cE '^WARN' "$out" | grep -qx 1 \
  || { echo "    expected exactly one WARN, got:"; grep -E '^WARN|^OK' "$out"; exit 1; }

# --------------------------------------- mutation: the declaration is what decides, not lib
# Drop the fixture's merge=derived line. Nothing in lib/ changes; the derive branch must
# become exposure, because the repository no longer declares that path generated. The
# declaration is read from the working tree, which is where a clone's merge policy lives,
# so withdrawing it needs no commit.
: > "$T/clone/.gitattributes"
grep -q 'merge=derived' "$T/clone/.gitattributes" \
  && { echo "    the declaration mutation did not apply"; exit 1; }
[ "$($G -C "$T/clone" check-attr merge -- docs/generated/artifacts.json)" \
  = "docs/generated/artifacts.json: merge: unspecified" ] \
  || { echo "    git still reports the withdrawn declaration"; exit 1; }
out2="$T/verdict2.txt"
verdict > "$out2"
cat "$out2"
expect_grep "2 branch\(es\) hold authored commits" "$out2"
expect_grep "derive-only\(1 authored of 1 non-merge of 1 on no remote\)" "$out2"
# the merge-only branch is unaffected: a merge is not authored work whatever the paths say
expect_no_grep "merge-only\(" "$out2"

# ------------------------------------------------ and a clone with nothing local says so
$G init -q -b master "$T/clean"
( cd "$T/clean" && printf 'x\n' > a.txt && $G add -A && $G commit -qm a \
  && $G remote add origin "$T/remote" && $G push -q -f origin master:clean )
out3="$T/verdict3.txt"
( set +u
  export MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" MJ_VERSION=test MJ_SELF="$ROOT/bin/majordomus"
  . "$ROOT/lib/common.sh"; . "$ROOT/lib/changed.sh"; . "$ROOT/lib/doctor.sh"
  MJ_ROOT="$T/clean"; MJ_SCOPE_FILE=""; MJ_POLICY_FILE=""; MJ_STATE_DIR="$T/clean/.ai/local/state"
  mj_report_clone ) > "$out3" 2>&1
cat "$out3"
expect_grep "^OK +clone +unpushed — every local commit is on a remote" "$out3"
