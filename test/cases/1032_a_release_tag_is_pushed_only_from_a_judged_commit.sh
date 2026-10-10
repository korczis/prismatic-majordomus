# majordomus-covers: none
# claims: release-tag-leaves-a-judged-commit
# A release tag leaves only from a judged commit.
#
# Three things went wrong with one release. v0.19.0 was tagged and pushed by hand before its
# commit had a `ci` check-run, and the release failed in its first job; nothing local had
# asked. The tag of v0.19.1, on a commit that had passed, was then refused by
# `.githooks/pre-push`, because the checkout it was pushed from held another session's
# uncommitted file outside the task's scope: the hook asked every push the finish contract,
# and a tag carries a commit, not a working tree. The hook asked the wrong question and
# nobody asked the right one.
#
# This case drives the repository's own hook, in a repository of its own, against a bare
# `origin` beside it and a stub `gh` on PATH that answers the two endpoints the hook's
# questions reach: a commit's check-runs, which `scripts/ci/release-verdict` reads, and a
# tag's release. No push here reaches a network, and the hook under test is the one in this
# tree. It holds, with the mutation of `.githooks/pre-push` that makes each fail:
#
#   judged      a v* tag whose commit's `ci` concluded success is pushed, annotated or not,
#               and the forge was asked about that commit
#               (M1, the verdict is never asked: `said=""` in place of the line that runs
#               scripts/ci/release-verdict; the forge hears nothing, and every tag leaves)
#   refused     no check-run, a running one, a failed one, an unreadable answer: the push is
#               refused naming the tag, the commit an annotated tag names and the verdict's
#               own words, and origin has no such tag afterwards
#               (M2, `"$2"` for `"$2^{commit}"`: the refusal names the tag object;
#               M6, the line `[ "$refused" = 0 ] || exit` removed: the refusal is printed
#               and the tag is pushed)
#   remedy      an unread verdict says to wait for validate and push again; a failed one
#               does not, because waiting will not change it
#   whole push  one unjudged tag refuses every ref pushed with it, the judged one included
#               (M8, the verdict run without `</dev/null`: it takes the refs that follow
#               from the loop, and the unjudged tag leaves unseen)
#   tag only    a push of release tags alone passes from a tree the finish contract refuses
#               (M3, `only_exempt=0` for a release tag as well: the old refusal returns)
#   contract    a branch in the push and the finish contract judges all of it, the tag
#               included; so does a tag that is not a release tag
#               (M4, `only_exempt` left at 1 for every ref: the dirty branch is pushed)
#   published   a tag with a release behind it is neither deleted nor moved; one the forge
#               cannot be asked about stays too; one with no release behind it may go, as
#               docs/DISTRIBUTION.md allows, and a moved one is still judged
#               (M5, `released` returns 1 without asking: the published tag is deleted;
#               M7, it returns 1 for any failed answer: the unasked one is deleted)
#
# All eight were run against this case on 2026-10-10, each one line of the hook, and the
# case failed under every one of them.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is required"
HOOK="$ROOT/.githooks/pre-push"
expect_file "$HOOK"
# Whatever git a caller of the suite was inside of, every repository here is this case's.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY
S="$(mktemp -d "${TMPDIR:-/tmp}/mj1032.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# ---------------------------------------------------------------- the forge
# A commit's check-runs are $S/runs/<sha>.json, and a commit with no file has none. A tag has
# a release behind it when $S/releases/<tag> exists. $S/forge-down makes every answer fail.
mkdir -p "$S/bin" "$S/runs" "$S/releases"
cat > "$S/bin/gh" <<STUB
#!/usr/bin/env bash
echo "\$*" >> "$S/asked"
# A client is free to read its standard input, and this one does: a hook that lends it the
# refs of the push loses every ref after the one it was judging.
cat > /dev/null
[ -f "$S/forge-down" ] && { echo "HTTP 502: Bad Gateway" >&2; exit 1; }
case "\$*" in
  "api repos/example/repo/commits/"*"/check-runs?check_name=ci&per_page=100")
    sha="\${2#repos/example/repo/commits/}"; sha="\${sha%%/*}"
    if [ -f "$S/runs/\$sha.json" ]; then cat "$S/runs/\$sha.json"; else printf '{"total_count":0,"check_runs":[]}\n'; fi
    exit 0 ;;
  "api repos/example/repo/releases/tags/"*)
    tag="\${2##*/}"
    [ -f "$S/releases/\$tag" ] && { printf '{"tag_name":"%s"}\n' "\$tag"; exit 0; }
    printf '{"message":"Not Found","status":"404"}\n'; echo "gh: Not Found (HTTP 404)" >&2; exit 1 ;;
esac
echo "stub gh: unexpected call: \$*" >&2
exit 1
STUB
chmod +x "$S/bin/gh"
PATH="$S/bin:$PATH"; export PATH
GITHUB_REPOSITORY=example/repo; export GITHUB_REPOSITORY
# judge <commit> <status> <conclusion>: the `ci` check-run that commit carries from now on
judge() { printf '{"total_count":1,"check_runs":[{"id":7,"name":"ci","status":"%s","conclusion":%s,"html_url":"https://example.invalid/run/7"}]}\n' "$2" "$3" > "$S/runs/$1.json"; }
unjudged() { rm -f "$S/runs/$1.json"; }

# ---------------------------------------------------------------- the repository
# The fixture is a supervised repository whose hook is this tree's, reaching the tool and the
# verdict where the hook looks for them: bin/majordomus and scripts/ci/release-verdict.
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib other bin scripts/ci .githooks
echo a > lib/a; echo o > other/o
printf '#!/bin/sh\nexec "%s" "$@"\n' "$MJ" > bin/majordomus
printf '#!/bin/sh\nexec "%s" "$@"\n' "$ROOT/scripts/ci/release-verdict" > scripts/ci/release-verdict
cp "$HOOK" .githooks/pre-push
chmod +x bin/majordomus scripts/ci/release-verdict .githooks/pre-push
git add -A >/dev/null && git commit -qm "feat: a tool with a hook"
O="$S/origin.git"; git init -q --bare "$O"
git remote add origin "$O"
git config core.hooksPath .githooks
git config commit.gpgsign false; git config tag.gpgsign false
remote() { git --git-dir "$O" rev-parse -q --verify "$1" 2>/dev/null || true; }
absent() { [ -z "$(remote "$1")" ] || { echo "    origin has $1, which the hook should have refused"; return 1; }; }
present() { [ -n "$(remote "$1")" ] || { echo "    origin has no $1, which the hook should have let through"; return 1; }; }

"$MJ" start "a task in lib" --scope lib >/dev/null
echo b >> lib/a; git commit -qam "feat: the release"
SHA="$(git rev-parse HEAD)"

# ---------------------------------------------------------------- judged
echo "  a release tag whose commit passed ci is pushed"
judge "$SHA" completed '"success"'
git tag -a v1.0.0 -m v1.0.0
[ "$(git rev-parse v1.0.0)" != "$SHA" ] || { echo "    the annotated tag is not an object of its own"; exit 1; }
expect_exit 0 git push origin v1.0.0 || exit 1
present refs/tags/v1.0.0 || exit 1
grep -qF "commits/$SHA/check-runs" "$S/asked" 2>/dev/null \
  || { echo "    the tag left and the forge was never asked about $SHA"; exit 1; }
git tag v1.0.1
expect_exit 0 git push origin refs/tags/v1.0.1 || exit 1
present refs/tags/v1.0.1 || exit 1

# ---------------------------------------------------------------- refused
echo "  a tag whose commit has no ci check-run is refused, and says what to do"
unjudged "$SHA"
git tag -a v1.1.0 -m v1.1.0
expect_exit 1 git push origin v1.1.0 || exit 1
expect_grep "pre-push: refused: release tag v1\.1\.0 names commit $SHA, and its verdict does not pass \(exit 12\)" || exit 1
expect_grep "has no 'ci' check-run; nothing says it passed" || exit 1
expect_grep "wait for master's validate on that commit, then push again" || exit 1
expect_grep "scripts/ci/release-verdict --commit $SHA --wait" || exit 1
expect_no_grep "names commit $(git rev-parse v1.1.0)" || exit 1
absent refs/tags/v1.1.0 || exit 1

echo "  a tag whose ci is still running is refused"
judge "$SHA" in_progress null
expect_exit 1 git push origin v1.1.0 || exit 1
expect_grep "'ci' check of ${SHA:0:12} is in_progress" || exit 1
expect_grep "wait for master's validate on that commit, then push again" || exit 1
absent refs/tags/v1.1.0 || exit 1

echo "  a tag whose ci failed is refused, and waiting is not the remedy"
judge "$SHA" completed '"failure"'
expect_exit 1 git push origin v1.1.0 || exit 1
expect_grep 'release tag v1\.1\.0 names commit .* \(exit 10\)' || exit 1
expect_grep 'concluded failure; a release follows the verdict' || exit 1
expect_no_grep "wait for master's validate" || exit 1
absent refs/tags/v1.1.0 || exit 1

echo "  a verdict that could not be read is refused"
judge "$SHA" completed '"success"'
touch "$S/forge-down"
expect_exit 1 git push origin v1.1.0 || exit 1
expect_grep 'could not be read' || exit 1
absent refs/tags/v1.1.0 || exit 1
rm -f "$S/forge-down"

echo "  one unjudged tag refuses every ref pushed with it"
echo c >> lib/a; git commit -qam "feat: not judged yet"
NEXT="$(git rev-parse HEAD)"
git tag v1.2.0
expect_exit 1 git push origin v1.1.0 v1.2.0 || exit 1
expect_grep "release tag v1\.2\.0 names commit $NEXT" || exit 1
expect_no_grep 'release tag v1\.1\.0' || exit 1
absent refs/tags/v1.1.0 || exit 1
absent refs/tags/v1.2.0 || exit 1

# ---------------------------------------------------------------- tag only
echo "  a push of release tags alone is not asked the finish contract"
# the tree the old hook refused: a file outside the task's scope, changed and not committed
echo x >> other/o
expect_exit 10 "$MJ" finish --check || exit 1
expect_grep 'FAIL scope +other/o' || exit 1
expect_exit 0 git push origin v1.1.0 || exit 1
expect_no_grep 'FAIL scope' || exit 1
present refs/tags/v1.1.0 || exit 1
# with the continuity store beside it, the push is still one the contract does not describe
judge "$NEXT" completed '"success"'
expect_exit 0 git push origin v1.2.0 HEAD:refs/majordomus/continuity/x || exit 1
present refs/tags/v1.2.0 || exit 1
present refs/majordomus/continuity/x || exit 1

# ---------------------------------------------------------------- the contract
echo "  a branch in the push, and the finish contract judges all of it"
expect_exit 1 git push origin HEAD:refs/heads/topic || exit 1
expect_grep 'FAIL scope +other/o' || exit 1
absent refs/heads/topic || exit 1
git tag v1.3.0
expect_exit 1 git push origin HEAD:refs/heads/topic v1.3.0 || exit 1
expect_grep 'FAIL scope +other/o' || exit 1
absent refs/heads/topic || exit 1
absent refs/tags/v1.3.0 || exit 1
# a tag that is not a release tag is not judged by a verdict; the contract is its question
git tag notes
expect_exit 1 git push origin notes || exit 1
expect_grep 'FAIL scope +other/o' || exit 1
absent refs/tags/notes || exit 1
# and with the tree back inside the scope, the same pushes leave
git checkout -q -- other/o
expect_exit 0 git push origin HEAD:refs/heads/topic v1.3.0 notes || exit 1
present refs/heads/topic || exit 1
present refs/tags/v1.3.0 || exit 1

# ---------------------------------------------------------------- published
echo "  a tag with a release behind it is neither deleted nor moved by a push"
touch "$S/releases/v1.0.0"
was="$(remote refs/tags/v1.0.0)"
expect_exit 1 git push origin :refs/tags/v1.0.0 || exit 1
expect_grep 'pre-push: refused: v1\.0\.0 has a release behind it, and a published tag is never moved or deleted by a push' || exit 1
[ "$(remote refs/tags/v1.0.0)" = "$was" ] || { echo "    origin's v1.0.0 was deleted"; exit 1; }
git tag -f v1.0.0 "$NEXT" >/dev/null
expect_exit 1 git push --force origin v1.0.0 || exit 1
expect_grep 'v1\.0\.0 has a release behind it' || exit 1
[ "$(remote refs/tags/v1.0.0)" = "$was" ] || { echo "    origin's v1.0.0 was moved"; exit 1; }

echo "  a tag the forge cannot be asked about stays where it is"
touch "$S/forge-down"
expect_exit 1 git push origin :refs/tags/v1.0.1 || exit 1
expect_grep 'whether a release stands behind v1\.0\.1 could not be read' || exit 1
present refs/tags/v1.0.1 || exit 1
rm -f "$S/forge-down"

echo "  a tag with no release behind it may go, and a moved one is still judged"
expect_exit 0 git push origin :refs/tags/v1.0.1 || exit 1
expect_grep 'v1\.0\.1 has no release behind it' || exit 1
absent refs/tags/v1.0.1 || exit 1
echo d >> lib/a; git commit -qam "feat: nobody judged this"
was="$(remote refs/tags/v1.1.0)"
git tag -f v1.1.0 HEAD >/dev/null
expect_exit 1 git push --force origin v1.1.0 || exit 1
expect_grep "has no 'ci' check-run" || exit 1
[ "$(remote refs/tags/v1.1.0)" = "$was" ] || { echo "    origin's v1.1.0 was moved to an unjudged commit"; exit 1; }

echo "    a release tag is pushed from a commit ci passed and from no other; the working tree is not its question, a branch's still is, and a published tag stays where it is"
exit 0
