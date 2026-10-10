# claim: derived-data-current
# A derive refuses an untracked file before it waits or builds (project.a-derive-sees-what-the-commit-will).
#
# A derive projects only what git tracks (`majordomus generate --discovery vcs`, and the site
# generator's own `git ls-files`). A file the next commit adds is invisible to it, and the
# commit is then refused by the derived-current gate after the derive has cost its minutes.
# On 2026-10-09 that cost a release two extra derives. This case runs the derive's own
# preflight in a disposable repository holding a copy of scripts/derive and its libraries.
# It proves four things: an untracked file is named and refused before the lock is taken or
# anything is built; an added file passes; a file ignored by .gitignore or by
# .git/info/exclude never refuses; and the override and `--locked` behave as documented.
. "$ROOT/test/lib.sh"

mkdir -p scripts lib docs
cp "$ROOT/scripts/derive" scripts/derive
cp "$ROOT/lib/machine_lock.sh" "$ROOT/lib/rust_bin.sh" lib/
printf 'target-cov/\n' > .gitignore
echo seed > docs/seed.md
git add . && git commit -qm "a derive and nothing else"
D="$T/scripts/derive"
export MAJORDOMUS_DERIVE_LOCK="$T/machine/.derive.lock"
export MAJORDOMUS_DERIVE_PRIORITY="$T/machine/.derive.priority"
fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- a clean tree passes
expect_exit 0 "$D" --preflight
expect_grep 'preflight clean'

# ---------------------------------------------------------------- an untracked file is refused, first
echo new > docs/new-page.md
expect_exit 10 "$D" --preflight
expect_grep 'refused before anything ran: 1 untracked file'
expect_grep '      docs/new-page.md'
expect_grep 'git add each one'
# the full derive refuses the same way, before it waits for the lock or builds anything
expect_exit 10 "$D"
expect_grep 'docs/new-page.md'
expect_no_grep 'building the Rust executable'
expect_no_grep 'waiting:'
[ ! -e "$MAJORDOMUS_DERIVE_LOCK" ] || fail 'a refused derive took the machine lock'

# ---------------------------------------------------------------- added, it passes
git add docs/new-page.md
expect_exit 0 "$D" --preflight

# ---------------------------------------------------------------- ignored or excluded never refuses
mkdir -p target-cov && echo junk > target-cov/report.json
mkdir -p "$(git rev-parse --git-common-dir)/info"
printf 'scratch/\n' >> "$(git rev-parse --git-common-dir)/info/exclude"
mkdir -p scratch && echo note > scratch/notes.txt
expect_exit 0 "$D" --preflight
expect_no_grep 'target-cov|scratch'

# ---------------------------------------------------------------- the override and --locked
echo stray > docs/stray.md
MAJORDOMUS_DERIVE_ALLOW_UNTRACKED=1 expect_exit 0 "$D" --preflight
expect_grep "1 untracked file\(s\) are invisible to this derive; going on"
# a suite under the lock projects nothing, so it owes no preflight
expect_exit 0 "$D" --locked -- true
[ ! -e "$MAJORDOMUS_DERIVE_LOCK" ] || fail 'the lock was left behind'
