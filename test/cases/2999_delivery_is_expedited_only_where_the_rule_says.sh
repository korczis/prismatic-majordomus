# An expedited landing reaches only what its rule says (ADR 0155).
#
# Rule project.an-expedited-landing-is-the-owners-word lets a named lander merge past the
# required check on the owner's recorded word. That is safe only while four things outside
# the lander's own conduct stay true, and each of them is a line somebody could change without
# meaning to change this:
#
#   1  a release is never expedited: the release workflow asks scripts/ci/release-verdict
#      before it builds or publishes anything, and the default check it asks about is `ci`
#      (what the verdict then decides is case 362);
#   2  a landing cancels nothing: a run of the validation workflow on the trunk is its own
#      concurrency group and nothing cancels a run in flight, and the publication queue does
#      not cancel the deploy in flight either;
#   3  the executor never expedites: no integration code names --admin (case 720 holds the
#      rest of that code; this is the one sentence the two rules share);
#   4  the rule and the document agree: the documented merge is pinned to the judged head,
#      and the rule says in its own words that a release is not expedited and which runs are
#      never cancelled.
#
# Read from the tracked files themselves; nothing is built and no network is reached.
. "$ROOT/test/lib.sh"
fail=0
note() { printf '    %s\n' "$1"; fail=1; }

REL="$ROOT/.github/workflows/release.yml"
VAL="$ROOT/.github/workflows/validate.yml"
PAG="$ROOT/.github/workflows/pages.yml"
RULE="$ROOT/.ai/repo/rules/project/an-expedited-landing-is-the-owners-word.v1.md"
DOC="$ROOT/docs/INTEGRATION.md"
for f in "$REL" "$VAL" "$PAG" "$RULE" "$DOC" "$ROOT/scripts/ci/release-verdict"; do
  [ -f "$f" ] || { echo "    ${f#"$ROOT"/} is missing"; exit 1; }
done

# ---------------------------------------------------------------- 1. a release asks first
line_of() { # <file> <extended pattern> -> the first line number, or nothing
  grep -nE -- "$2" "$1" | head -n 1 | cut -d: -f1
}
verdict="$(line_of "$REL" '^[[:space:]]+run:.*scripts/ci/release-verdict[[:space:]]+--tag')"
[ -n "$verdict" ] || note "release.yml no longer runs scripts/ci/release-verdict --tag"
for publishes in 'gh release create' 'gh release upload' 'actions/upload-artifact'; do
  first="$(line_of "$REL" "^[[:space:]]+[^#]*$publishes")"
  [ -n "$first" ] || { note "release.yml has no '$publishes' step to hold after the verdict"; continue; }
  [ -n "$verdict" ] && [ "$verdict" -lt "$first" ] \
    || note "release.yml reaches '$publishes' (line $first) before it asks for the verdict (line ${verdict:-none})"
done
grep -qE '^CHECK="\$\{RELEASE_VERDICT_CHECK:-ci\}"$' "$ROOT/scripts/ci/release-verdict" \
  || note "release-verdict no longer asks about the ci check by default"
# the workflow does not point the verdict at another check
if grep -nE 'RELEASE_VERDICT_CHECK' "$REL" >/dev/null; then
  note "release.yml sets RELEASE_VERDICT_CHECK: the release would follow a check other than ci"
fi

# ---------------------------------------------------------------- 2. a landing cancels nothing
top_concurrency() { # <workflow> -> the top-level concurrency block
  awk '/^concurrency:/ {on=1; print; next} on && /^[^[:space:]#]/ {exit} on {print}' "$1"
}
vc="$(top_concurrency "$VAL")"
[ -n "$vc" ] || note "validate.yml has no top-level concurrency block"
printf '%s\n' "$vc" | grep -qE '^[[:space:]]+cancel-in-progress:[[:space:]]+false[[:space:]]*$' \
  || note "validate.yml may cancel a run in flight: cancel-in-progress is not false"
printf '%s\n' "$vc" | grep -E '^[[:space:]]+group:' | grep -q 'github.run_id' \
  || note "validate.yml's group no longer gives a trunk run its own group (github.run_id)"
pc="$(top_concurrency "$PAG")"
printf '%s\n' "$pc" | grep -qE '^[[:space:]]+cancel-in-progress:[[:space:]]+false[[:space:]]*$' \
  || note "pages.yml may cancel the deploy in flight: cancel-in-progress is not false"
# no job of either workflow re-enables it
for wf in "$VAL" "$PAG" "$REL"; do
  if grep -nE '^[[:space:]]+cancel-in-progress:[[:space:]]+true' "$wf" >/dev/null; then
    note "${wf#"$ROOT"/} cancels something in progress"
  fi
done

# ---------------------------------------------------------------- 3. the executor never expedites
hits="$(grep -rnE -- '"--admin"' "$ROOT/apps/majordomus-cli/src/integration" 2>/dev/null || true)"
[ -z "$hits" ] || note "the integration code passes --admin: $(printf '%s' "$hits" | head -n 1)"

# ---------------------------------------------------------------- 4. the rule and the document
grep -qF 'gh pr merge --admin --merge --match-head-commit <head>' "$DOC" \
  || note "docs/INTEGRATION.md no longer pins the expedited merge to the judged head"
grep -qF 'merge --admin --merge --match-head-commit <head>' "$RULE" \
  || note "the rule no longer pins the expedited merge to the judged head"
grep -qF '**A release is never expedited.**' "$RULE" \
  || note "the rule no longer says a release is never expedited"
grep -qF "These are never cancelled: the trunk's only run of a commit, a deployment in" "$RULE" \
  || note "the rule no longer names the runs that are never cancelled"
grep -qF '**The integration executor never passes `--admin`.**' "$RULE" \
  || note "the rule no longer leaves the executor out of the exception"

[ "$fail" = 0 ] || exit 1
echo "    a release asks for the ci verdict before it builds, a landing cancels neither a trunk run nor a deploy, the executor names no --admin, and the rule and its document pin the merge to the judged head"
