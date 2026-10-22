# claim: derived-data-current
# A derive does not run while a merge, cherry-pick or revert is uncommitted (I2863).
#
# While MERGE_HEAD is present, HEAD is still the commit the merge is leaving, and every
# projection that reads history reads HEAD's. On 2026-10-21 #1753 merged master and was
# derived before the merge commit existed: the released_in rule found an older merge in the
# old base's history, idd-workspace was committed as released in v0.30.0, and CI's site job
# called roadmap.json stale after forty minutes of validate. This case runs the derive's own
# preflight in a disposable repository holding a copy of scripts/derive and its libraries:
#
#   1  a clean tree passes
#   2  an uncommitted merge (a real conflict-free --no-commit merge) is refused before the
#      lock is taken, naming MERGE_HEAD and the commit being left
#   3  committed, it passes
#   4  a cherry-pick stopped on a conflict (CHERRY_PICK_HEAD) is refused the same way
. "$ROOT/test/lib.sh"

mkdir -p scripts lib docs
cp "$ROOT/scripts/derive" scripts/derive
cp "$ROOT/lib/machine_lock.sh" "$ROOT/lib/rust_bin.sh" lib/
echo seed > docs/seed.md
git add . && git commit -qm "a derive and nothing else"
D="$T/scripts/derive"
export MAJORDOMUS_DERIVE_LOCK="$T/machine/.derive.lock"
export MAJORDOMUS_DERIVE_PRIORITY="$T/machine/.derive.priority"
fail() { printf '    %s\n' "$*"; exit 1; }
base="$(git rev-parse --abbrev-ref HEAD)"

# ---------------------------------------------------------------- 1. a clean tree passes
expect_exit 0 "$D" --preflight

# ---------------------------------------------------------------- 2. an uncommitted merge is refused
git checkout -q -b side
echo side > docs/side.md && git add docs/side.md && git commit -qm "side"
git checkout -q "$base"
echo main > docs/main.md && git add docs/main.md && git commit -qm "main"
left="$(git rev-parse --short=11 HEAD)"
git merge -q --no-ff --no-commit side >/dev/null 2>&1
[ -e "$(git rev-parse --git-dir)/MERGE_HEAD" ] || fail "the fixture did not leave a merge uncommitted"
expect_exit 10 "$D" --preflight
expect_grep 'refused before anything ran: MERGE_HEAD is present, so merge is uncommitted'
expect_grep "HEAD $left"
expect_grep 'Commit it'
# the full derive refuses the same way, before it waits for the lock or builds anything
expect_exit 10 "$D"
expect_no_grep 'building the Rust executable'
expect_no_grep 'waiting:'
[ ! -e "$MAJORDOMUS_DERIVE_LOCK" ] || fail 'a refused derive took the machine lock'

# ---------------------------------------------------------------- 3. committed, it passes
git commit -qm "merge side" --no-edit
expect_exit 0 "$D" --preflight

# ---------------------------------------------------------------- 4. a stopped cherry-pick
# a conflicting pick, because `--no-commit` leaves no CHERRY_PICK_HEAD and a conflict does
git checkout -q -b pick
echo pick > docs/seed.md && git add docs/seed.md && git commit -qm "pick"
c="$(git rev-parse HEAD)"
git checkout -q "$base"
echo other > docs/seed.md && git add docs/seed.md && git commit -qm "other"
git cherry-pick "$c" >/dev/null 2>&1 || true
[ -e "$(git rev-parse --git-dir)/CHERRY_PICK_HEAD" ] || fail "the fixture did not leave a cherry-pick pending"
expect_exit 10 "$D" --preflight
expect_grep 'CHERRY_PICK_HEAD is present, so cherry-pick is uncommitted'
