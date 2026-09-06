# majordomus-covers: none
# The gate that refuses an authored document naming somebody's home directory.
#
# `scripts/site-check` already refuses a built site carrying one, and that is the gate that
# caught this — at the moment of deployment, with the text already on master and the deploy
# already red. The reasoning of I0908, an issue whose stated purpose is that "no absolute
# developer path exists anywhere in the image", named a home directory; it reached plan.json,
# reached the site, and broke master's deploy. The cost of the late gate is that it bills the
# person publishing rather than the person writing.
#
# So the same pattern runs over the sources, in core-check. What must hold is that it is
# wired, that this repository passes it, and — the part worth a test — that it actually fails
# when a document names a home directory. A gate that cannot fail is decoration.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- wired, and passing here
expect_grep 'no authored document names a home directory' "$ROOT/scripts/ci/core-check"

hits="$(git -C "$ROOT" grep -n -I -E '/Users/[a-z]+/|/home/[a-z]+/dev/' -- \
          '.ai/repo/**' 'docs/**' 'site/content-src/**' 'README.md' 2>/dev/null || true)"
[ -z "$hits" ] || {
  echo "    an authored document names a home directory:"
  printf '    | %s\n' "$hits" | head -5
  exit 1; }

# ---------------------------------------------------------------- and it can fail
# The check as core-check runs it, against a repository that carries the fault. Extracted by
# shape rather than by sourcing core-check, which would run every other gate with it.
R="$T/repo"; mkdir -p "$R/.ai/repo/project/issues"
git -C "$R" init -q
git -C "$R" config user.email t@example.com
git -C "$R" config user.name Test
printf 'why: "A Machine has no /Users/somebody/dev."\n' > "$R/.ai/repo/project/issues/I0001.yaml"
git -C "$R" add -A
git -C "$R" -c core.hooksPath=/dev/null commit -qm fixture

hits="$(git -C "$R" grep -n -I -E '/Users/[a-z]+/|/home/[a-z]+/dev/' -- \
          '.ai/repo/**' 'docs/**' 'site/content-src/**' 'README.md' 2>/dev/null || true)"
[ -n "$hits" ] || { echo "    the check did not notice a home directory in an issue"; exit 1; }

# and the same document, written as what the path means, passes
printf 'why: "A Machine has no developer'"'"'s checkout."\n' > "$R/.ai/repo/project/issues/I0001.yaml"
git -C "$R" add -A
git -C "$R" -c core.hooksPath=/dev/null commit -qm reworded
hits="$(git -C "$R" grep -n -I -E '/Users/[a-z]+/|/home/[a-z]+/dev/' -- \
          '.ai/repo/**' 'docs/**' 'site/content-src/**' 'README.md' 2>/dev/null || true)"
[ -z "$hits" ] || { echo "    the reworded document still trips the check: $hits"; exit 1; }
