# The age of an owed publication is the oldest unpublished commit's, not the newest's.
#
# `pages-check` refuses by age rather than by distance, on the reasoning that a publication
# owed for ninety seconds is a deploy in flight and one owed for an hour is a deploy that is
# not coming. The range `<published>..master` was read with `git log -1`, which answers a
# different question: how long since the last thing landed. On a trunk that moves every few
# minutes that answer is always young, so the window never closed and the gate could not
# fire while the repository was busy — which is exactly when a deploy breaks. It reported ok
# for five hours on 2026-09-12 while the published site stood 34 commits back, each new merge
# resetting the age it was reading.
#
# This case builds a repository whose publication is behind by two commits — one old enough
# to be a refusal, one young enough not to be — and pins the verdict to the older. Reading
# the newer one passes, so the two commits are what make the assertion discriminating.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj279.XXXXXX")"; trap 'rm -rf "$S"' EXIT

R="$S/repo"; mkdir -p "$R"
git init -q -b master "$R"
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t

mkdir -p "$R/site" "$R/scripts/ci"
cp "$ROOT/scripts/ci/pages-check" "$R/scripts/ci/pages-check"
printf 'x\n' > "$R/site/index.md"
git -C "$R" add -A
git -C "$R" -c core.hooksPath=/dev/null commit -qm base
published="$(git -C "$R" rev-parse HEAD)"

# Two commits ahead of the publication, both touching site/. The older is the debt; the
# newer is the one a `log -1` reading would have measured instead.
old_at="$(( $(date +%s) - 7200 ))"
printf 'older\n' > "$R/site/a.md"
git -C "$R" add -A
GIT_AUTHOR_DATE="$old_at +0000" GIT_COMMITTER_DATE="$old_at +0000" \
  git -C "$R" -c core.hooksPath=/dev/null commit -qm "older change to the site"
old_sha="$(git -C "$R" rev-parse --short=12 HEAD)"

printf 'newer\n' > "$R/site/b.md"
git -C "$R" add -A
git -C "$R" -c core.hooksPath=/dev/null commit -qm "newer change to the site"

# A gh-pages whose tip names the published commit in the subject the gate parses. Built with
# plumbing rather than an orphan checkout: the work tree is dirty by design here, and
# `checkout --orphan` refuses to step over it.
empty_tree="$(git -C "$R" hash-object -t tree /dev/null)"
gp="$(printf 'deploy: site from %s\n' "$published" | git -C "$R" commit-tree "$empty_tree")"
git -C "$R" update-ref refs/heads/gh-pages "$gp"
git -C "$R" remote add origin "$R"
git -C "$R" fetch -q origin 2>/dev/null || true

out="$S/out.txt"
status=0
( cd "$R" && env -u MAJORDOMUS_SHARE scripts/ci/pages-check --offline --owed-after 1800 ) \
  > "$out" 2>&1 || status=$?

# `cmd; status=$?` never reaches the assignment under set -e, hence the `|| status=$?` above.
if [ "$status" != 10 ]; then
  echo "want exit 10 for a publication owed two hours ago, got $status" >&2
  cat "$out" >&2
  exit 1
fi
expect_grep "owes a publication" "$out"
expect_grep "$old_sha" "$out"
expect_no_grep "inside the 1800s deploy window" "$out"

# ------------------------------------------------ GitHub's build names what it is a build of
#
# The same afternoon printed, directly under that refusal:
#
#   ok    GitHub's own Pages build of 7a7fe00b4abb is built
#
# True. 7a7fe00b4abb was the head of gh-pages, the site from acebf3d, 34 commits behind
# master's head, and the line did not say so, so beside the FAIL it read as "the current build
# is fine". A verdict that does not name its subject (project.a-verdict-states-its-subject).
#
# What this part tests, and what it does not. GitHub's API is not exercised: the gate asks it
# through `scripts/pages built --porcelain`, whose answer is an exit status and one tab-separated
# line, and a stub gives that answer here. What is tested is the gate's own words over a given
# answer and the git facts it adds: which gh-pages commit was built, which trunk commit that is
# the site of, and how far behind it stands. The live site is not simulated either: it is a
# loopback port nothing listens on, so every run below also carries that FAIL and exits 10.
empty_parent="$(printf 'deploy: site from %s\n' "$published" | git -C "$R" commit-tree "$empty_tree")"
gp="$(printf 'deploy: site from %s\n' "$published" | git -C "$R" commit-tree "$empty_tree" -p "$empty_parent")"
git -C "$R" update-ref refs/heads/gh-pages "$gp"
git -C "$R" fetch -q origin 2>/dev/null || true
gp_old="$empty_parent"

mkdir -p "$R/.ai/repo/ci"
printf 'deploy:\n  identity: build.json\n' > "$R/.ai/repo/ci/pages.yaml"
cat > "$R/scripts/pages" <<'STUB'
#!/usr/bin/env bash
# GitHub's answer as `scripts/pages built --porcelain` gives it: one line, and an exit status.
# Every other subcommand answers nothing, which is what the offline run above saw without it.
[ "${1:-}" = built ] || exit 0
cat "$MJ279_BUILD"
exit "$(cat "$MJ279_BUILD_RC")"
STUB
chmod +x "$R/scripts/pages"

answer() { printf '%s\n' "$1" > "$S/build.rc"; printf '%s\t%s\t%s\n' "$2" "$3" "$4" > "$S/build"; }
ask() {
  status=0
  ( cd "$R" && env -u MAJORDOMUS_SHARE MJ279_BUILD="$S/build" MJ279_BUILD_RC="$S/build.rc" \
      scripts/ci/pages-check --url http://127.0.0.1:9 --wait 0 --owed-after 1800 ) \
    > "$out" 2>&1 || status=$?
  if [ "$status" != 10 ]; then
    echo "want exit 10 (owed, and a site that does not answer), got $status" >&2; cat "$out" >&2; exit 1
  fi
  expect_grep "^FAIL +the live site at http://127.0.0.1:9 did not answer" "$out"
}
pub="${published:0:12}"
behind="the published commit $pub, the head of gh-pages, 2 commit\(s\) behind master's head"

# the defect: a build of the published commit says which commit, and how far behind
answer 0 built "$gp" ""
ask
expect_grep "^ok +GitHub's own Pages build of ${gp:0:12} is built — a build of $behind$" "$out"
expect_no_grep "^ok +GitHub's own Pages build of ${gp:0:12} is built$" "$out"

# a 0 for a build of some other commit is a broken contract, never an ok about the wrong subject
answer 0 built "$gp_old" ""
ask
expect_no_grep "^ok +GitHub's own Pages build" "$out"
expect_grep "^note +scripts/pages built answered built for ${gp_old:0:12}, not for the head of gh-pages ${gp:0:12}" "$out"

# a build of the head still in flight is the head: the line once said "not the head" here
answer 12 building "$gp" ""
ask
expect_grep "^note +GitHub's own Pages build of ${gp:0:12} — $behind — is building; not a verdict yet" "$out"
expect_no_grep "not the head of gh-pages" "$out"

# GitHub's newest build is older than the published commit: which deploy, and how many back
answer 12 built "$gp_old" ""
ask
expect_grep "^note +GitHub's newest Pages build is of ${gp_old:0:12} \(built, the site from $pub, 1 deploy\(s\) behind the head of gh-pages\), not of $behind; that build was not measured" "$out"
expect_no_grep "^ok +GitHub's own Pages build" "$out"

# an errored build of the head is still a refusal, and names its subject too
answer 10 errored "$gp" "Page build failed."
ask
expect_grep "^FAIL +GitHub's own Pages build of ${gp:0:12} — $behind — errored: Page build failed\.$" "$out"

# the publication is master's head: the line says so instead of a distance
head="$(git -C "$R" rev-parse HEAD)"
gp_head="$(printf 'deploy: site from %s\n' "$head" | git -C "$R" commit-tree "$empty_tree" -p "$gp")"
git -C "$R" update-ref refs/heads/gh-pages "$gp_head"
git -C "$R" fetch -q origin 2>/dev/null || true
answer 0 built "$gp_head" ""
ask
expect_grep "^ok +GitHub's own Pages build of ${gp_head:0:12} is built — a build of the published commit ${head:0:12}, the head of gh-pages, which is master's head$" "$out"
expect_no_grep "owes a publication" "$out"

# the publication is not on master at all: the build line does not call it behind, or current
stray="$(git -C "$R" commit-tree "$empty_tree" -p "$published" -m 'work nobody merged')"
gp_stray="$(printf 'deploy: site from %s\n' "$stray" | git -C "$R" commit-tree "$empty_tree" -p "$gp_head")"
git -C "$R" update-ref refs/heads/gh-pages "$gp_stray"
git -C "$R" fetch -q origin 2>/dev/null || true
answer 0 built "$gp_stray" ""
ask
expect_grep "^FAIL +the published commit $stray is NOT on master" "$out"
expect_grep "^ok +GitHub's own Pages build of ${gp_stray:0:12} is built — a build of the published commit ${stray:0:12}, the head of gh-pages, which is not on master$" "$out"
