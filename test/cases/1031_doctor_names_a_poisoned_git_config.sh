# majordomus-covers: doctor
# majordomus-negative: doctor
# claims: fixture-cannot-write-its-repository
# Doctor names a poisoned git configuration.
#
# The configuration of a repository is shared by every worktree of it and is in no tree, so
# no gate over a tree sees it. On 2026-10-10 a suite started from a pre-push hook ran its
# fixtures with git's hook environment still exported, and the fixtures' `git config` and
# `git init --bare` wrote into the repository being pushed: user.name=t,
# user.email=t@example.com, core.bare=true and a merge driver with no command. Case 1030
# holds the door that let it in. This case holds the reading that would have said so within
# the minute, had it existed: `majordomus doctor` fails, names each of the three and prints
# the command that undoes it, and writes nothing while it does.
#
#   1  a fixture is not poisoned: a repository whose every commit is a fixture's carries a
#      fixture identity on purpose, and reads OK
#   2  a fixture identity in a repository whose history is somebody else's is named, by
#      key, for each of the addresses and for the name; the owner's own identity is not
#   3  core.bare=true in a checkout is named from a linked worktree, and from the checkout
#      itself, where no other command of the tool gets as far as a finding; a repository
#      that is bare on purpose is not named
#   4  paths marked merge=derived with no driver behind the name are named, an empty driver
#      as much as an absent one; where the policy declares the driver as an enforcement
#      entry, the wiring check names it and this one does not name it a second time
#   5  every one of those readings leaves the configuration byte-identical
. "$ROOT/test/lib.sh"

no() { printf '    %s\n' "$*"; exit 1; }

# doctor, in the repository named, with the configuration read before and after: a finding
# that repaired what it found would be a second writer of a file this case exists to guard.
doctor_in() {  # directory [majordomus options...]
  local dir="$1" gd before after; shift
  gd="$(git -C "$dir" rev-parse --git-common-dir)"; case "$gd" in /*) ;; *) gd="$dir/$gd" ;; esac
  before="$(cksum < "$gd/config")"
  LAST_OUT="$( cd "$dir" && "$MJ" "$@" doctor 2>&1 )" && LAST_RC=0 || LAST_RC=$?
  after="$(cksum < "$gd/config")"
  [ "$before" = "$after" ] || no "doctor wrote $gd/config; it reads and the person repairs"
}
names() {  # subject pattern-of-the-detail
  expect_grep "^FAIL git-config +$1 — .*$2.*\\[reproduce: .+\\]$" \
    || no "doctor did not name $1 as a failure with the command that undoes it"
  [ "$LAST_RC" != 0 ] || no "doctor named $1 and still exited 0"
}
clean() {  # what
  expect_no_grep '^FAIL git-config' || no "$1: doctor named a poisoned configuration"
  expect_grep '^OK +git-config +local — ' || no "$1: doctor did not report the configuration as read"
}
# the command a finding prints, run as printed: a remedy nobody ran is a guess
remedy() {  # subject
  local cmd; cmd="$(printf '%s\n' "$LAST_OUT" | sed -n "s/^FAIL git-config  *$1 — .*\\[reproduce: \\(.*\\)\\]\$/\\1/p" | head -n 1)"
  [ -n "$cmd" ] || no "the finding for $1 carries no command"
  ( cd "$R" && eval "$cmd" ) || no "the command the finding for $1 prints failed: $cmd"
}

# ---------------------------------------------------------------- 1. a fixture is not poisoned
# $T is what test/run.sh made: user.email=t@example.com, user.name=t, one commit by them.
[ "$(git config --local --get user.email)" = t@example.com ] \
  || no "the runner's fixture no longer carries the fixture identity this case starts from"
"$MJ" init >/dev/null
doctor_in "$T"
clean "a repository whose every commit is a fixture's"

# ---------------------------------------------------------------- the repository of a person
# History written by somebody who is not a fixture, an installed layer, a file checked out
# (so the checkout has an index), a linked worktree, and no identity of its own: commits
# name theirs on the command line.
R="$T/real"
git init -q "$R"
( cd "$R" && "$MJ" init >/dev/null )
git -C "$R" add -A
git -C "$R" -c user.email=owner@majordomus.dev -c user.name=Owner commit -qm 'a repository of a person'
git -C "$R" worktree add -q -b feature/work "$T/real-wt"
doctor_in "$R"
clean "a repository with no identity of its own"

# ---------------------------------------------------------------- 2. a fixture identity
git -C "$R" config user.email t@example.com
doctor_in "$R"
names identity 'user\.email=t@example\.com in this repository'"'"'s own configuration is a test fixture'"'"'s identity'
expect_no_grep 'user\.name=' || no "doctor named a user.name nobody set"
expect_grep 'reproduce: git config --local --unset user\.email\]$'
# and as one JSON object, with the command
doctor_in "$R" --json
expect_grep '^\{"level":"FAIL","category":"git-config","subject":"identity",.*"reproduce":"git config --local --unset user\.email"\}$'

for address in t@t t@e ci@example.invalid; do
  git -C "$R" config user.email "$address"
  doctor_in "$R"
  names identity "user\\.email=$address"
done
git -C "$R" config --unset user.email

git -C "$R" config user.name t
doctor_in "$R"
names identity 'user\.name=t in this repository'
expect_no_grep 'user\.email=' || no "doctor named a user.email nobody set"

# both, from a linked worktree: the configuration is the repository's, whoever reads it
git -C "$R" config user.email suite@example.org
doctor_in "$T/real-wt"
names identity 'user\.email=suite@example\.org, user\.name=t'
remedy identity
[ -z "$(git -C "$R" config --local --get user.email || true)$(git -C "$R" config --local --get user.name || true)" ] \
  || no "the command the identity finding prints left an identity behind"

# the owner's own identity, set in the repository, is nobody's business
git -C "$R" config user.email owner@majordomus.dev
git -C "$R" config user.name Owner
doctor_in "$R"
clean "after the command the identity finding prints, a repository carrying its owner's identity"
git -C "$R" config --unset user.email; git -C "$R" config --unset user.name

# ---------------------------------------------------------------- 3. core.bare in a checkout
[ -f "$R/.git/index" ] || no "the fixture checkout has no index, so it is not the checkout this part is about"
git -C "$R" config core.bare true
# from a linked worktree the repository still resolves, and the finding names the checkout
doctor_in "$T/real-wt"
names core.bare "true in .*/real/\\.git/config, and .*/real is a checkout with an index"
# from the checkout itself nothing resolves: every other command stops at the door
expect_exit 2 "$MJ" --repo "$R" context
expect_grep 'not inside a git repository'
doctor_in "$R"
names core.bare "true in .*/real/\\.git/config, and .*/real is a checkout with an index"
[ "$LAST_RC" = 10 ] || no "doctor exited $LAST_RC in a checkout marked bare; a failure is 10"
expect_grep '^doctor: 1 failure\(s\)$'
expect_no_grep 'not inside a git repository' || no "doctor stopped at the door instead of naming what closed it"
# and from anywhere, by --repo
LAST_OUT="$("$MJ" --repo "$R" doctor 2>&1)" && LAST_RC=0 || LAST_RC=$?
names core.bare "true in .*/real/\\.git/config"
remedy core.bare
[ "$(git -C "$R" config --local --bool --get core.bare)" = false ] || no "the command the core.bare finding prints did not clear it"

# A repository that is bare on purpose, with linked worktrees around it: both shapes people
# keep, the directory named for the project and the one named .git.
git clone -q --bare "$R" "$T/kept.git"
git -C "$T/kept.git" worktree add -q "$T/kept-wt" feature/work
doctor_in "$T/kept-wt"
clean "a worktree of a repository that is bare on purpose"
mkdir "$T/nest"; git clone -q --bare "$R" "$T/nest/.git"
git -C "$T/nest/.git" worktree add -q "$T/nest/work" feature/work
doctor_in "$T/nest/work"
clean "a worktree of a bare repository whose directory is named .git"

# ---------------------------------------------------------------- 4. merge=derived and no driver
# a comment that says the words marks nothing
printf '# docs/GENERATED.md merge=derived\n' > "$R/.gitattributes"
doctor_in "$R"
clean "after the command the core.bare finding prints, with a .gitattributes that only mentions merge=derived in a comment"
expect_no_grep 'merge=derived' || no "doctor read a comment as a marked path"

printf 'docs/GENERATED.md merge=derived\ndocs/OTHER.md  merge=derived  -diff\nREADME.md text\n' > "$R/.gitattributes"
doctor_in "$R"
names merge.derived.driver 'unset or empty in this clone while \.gitattributes marks 2 pattern\(s\) merge=derived'
git -C "$R" config merge.derived.name 'a driver with a name and no command'
git -C "$R" config merge.derived.driver ''
doctor_in "$R"
names merge.derived.driver 'unset or empty in this clone'

git -C "$R" config merge.derived.driver 'true %O %A %B %P'
doctor_in "$R"
clean "paths marked merge=derived with a driver declared"
expect_grep '^OK +git-config +local — .*merge=derived has a driver'
git -C "$R" config --unset merge.derived.driver

# Where the policy declares the driver, wiring decides it and names the declared program;
# one fact, one finding.
mkdir -p "$R/scripts"; printf '#!/bin/sh\nexit 0\n' > "$R/scripts/merge-derived"; chmod +x "$R/scripts/merge-derived"
python3 - "$R/.ai/repo/policy.yaml" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
entry = "enforcement:\n  - name: derived-merge-driver\n    path: scripts/merge-derived\n    args: []\n    wired_by: git-config:merge.derived.driver\n"
assert s.count("\nenforcement:\n") == 1, "the skeleton policy no longer has one enforcement block"
open(p, "w").write(s.replace("\nenforcement:\n", "\n" + entry, 1))
PY
doctor_in "$R"
expect_grep '^FAIL wiring +derived-merge-driver — git config merge\.derived\.driver is not set in this clone' \
  || no "the wiring check did not name the driver the policy declares"
expect_no_grep '^FAIL git-config +merge\.derived\.driver' || no "one missing driver was named by two findings"
expect_grep '^OK +git-config +local — .*the merge=derived driver is decided by wiring'

echo "    doctor names a fixture identity over another author's history, core.bare in a checkout (from the checkout too) and merge=derived with no driver, prints the command for each, leaves a fixture, an owner's identity and a deliberately bare repository alone, and writes nothing"
