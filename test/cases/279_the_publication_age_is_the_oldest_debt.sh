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
