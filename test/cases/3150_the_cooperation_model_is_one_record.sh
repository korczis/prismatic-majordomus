# The cooperation model is one record, read everywhere (ADR 0167).
#
# Rule project.the-cooperation-model-is-the-owners-word is the one record of the seats, the
# mesh claim each one holds and what the landing seat may merge. Three things outside the rule
# could drift from it without anyone meaning to change it, and each is read here:
#
#   1  the Mesh screen of the UI draws a seat for every scope the rule's seat table names,
#      so who holds a seat is read from the live claims by the same scope the rule gives
#      (the screen's own test, every_seat_is_named_by_the_rule, holds the converse);
#   2  the merge the rule permits stays pinned: `--admin` only with `--merge` and
#      `--match-head-commit`, on a head whose ci succeeded and which contains master;
#   3  the integration, mesh and release documents and the UI document point at the rule
#      and ADR 0167, and none of them copies a seat's scope: a copy would be a second record.
#
# Read from the tracked files themselves; nothing is built and no network is reached.
. "$ROOT/test/lib.sh"
fail=0
note() { printf '    %s\n' "$1"; fail=1; }

RULE="$ROOT/.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md"
UI="$ROOT/apps/majordomus-cli/src/ui/mesh.rs"
ADR_GLOB="$ROOT/.ai/repo/adrs/0167-"
for f in "$RULE" "$UI"; do
  [ -f "$f" ] || { echo "    ${f#"$ROOT"/} is missing"; exit 1; }
done
ADR="$(ls "$ADR_GLOB"*.md 2>/dev/null | head -n 1)"
[ -n "$ADR" ] || { echo "    ADR 0167 is missing"; exit 1; }
grep -q '^id: adr-0167$' "$ADR" || note "${ADR#"$ROOT"/} does not say id: adr-0167"

# ---------------------------------------------------------------- 1. the seats the rule names
# a row of the rule's seat table: | <seat> | `<scope>` | <what it does> |
scopes="$(grep -E '^   \| (landing seat|release seat|repair lane) \| `[^`]+` \|' "$RULE" \
  | sed -E 's/^[^`]*`([^`]+)`.*$/\1/')"
n="$(printf '%s\n' "$scopes" | grep -c . || true)"
[ "$n" -eq 3 ] || note "the rule's seat table names $n seat scopes, expected 3 (landing, release, repair)"
while IFS= read -r scope; do
  [ -n "$scope" ] || continue
  grep -qF "scope: \"$scope\"," "$UI" \
    || note "the Mesh screen draws no seat for the rule's scope $scope"
done <<EOF_SCOPES
$scopes
EOF_SCOPES
grep -qF '"mesh.state"' "$UI" || note "the Mesh screen no longer reads the live claims from mesh.state"
grep -qF '"open":true' "$UI" || note "the Mesh screen reads claims that no longer hold"

# ---------------------------------------------------------------- 2. the merge stays pinned
grep -qF 'gh pr merge <n> --admin --merge --match-head-commit <sha>' "$RULE" \
  || note "the rule's merge is no longer pinned to the judged head"
grep -qE "concluded success and the head contains the current .origin/master" "$RULE" \
  || note "the rule no longer asks that the judged head's ci succeeded and that it contains master"
grep -qE 'never force-pushed' "$RULE" || note "the rule no longer says a pushed branch is never force-pushed"

# ---------------------------------------------------------------- 3. one record
for doc in docs/INTEGRATION.md docs/MESH.md docs/RELEASE.md docs/MAJORDOMUS_UI.md; do
  f="$ROOT/$doc"
  [ -f "$f" ] || { note "$doc is missing"; continue; }
  grep -qF 'rules/project/the-cooperation-model-is-the-owners-word.v2.md' "$f" \
    || note "$doc does not link the rule"
  while IFS= read -r scope; do
    [ -n "$scope" ] || continue
    if grep -qF "\`$scope\`" "$f"; then
      note "$doc copies the seat scope $scope: the rule is its one record"
    fi
  done <<EOF_SCOPES
$scopes
EOF_SCOPES
done
grep -qF 'adrs/0167-' "$ROOT/docs/INTEGRATION.md" || note "docs/INTEGRATION.md does not link ADR 0167"

[ "$fail" -eq 0 ] || exit 1
echo "    the seats are the rule's, the merge is pinned, and the documents point at one record"
