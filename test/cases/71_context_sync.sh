# `context check-sync`: the tree validates, the projections are what the layer says they
# are, and the review items a change set raises are named. A fresh init is in sync; a hand
# edit to a projection is drift (exit 11, stale-projection) until it is regenerated; a
# change that means nothing is not drift; and the JSON form is one finding per line with
# the four fields every finding carries.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null

# --- a fresh init is in sync: init produces a valid tree and nothing has drifted
expect_exit 0 "$MJ" context check-sync
expect_no_grep '^(FAIL|DRIFT)'

# --- with projections written and committed, still in sync
"$MJ" update >/dev/null
git add -A >/dev/null; git commit -qm "init and projections"
expect_exit 0 "$MJ" context check-sync
expect_no_grep '^(FAIL|DRIFT)'

# --- a hand-edited projection is drift, named as such
printf '\nA sentence a person typed into a generated file.\n' >> AGENTS.md
expect_exit 11 "$MJ" context check-sync
expect_grep 'stale-projection'
expect_grep 'AGENTS\.md'

# --- the JSON form: one object per line, each with level, category, subject, message
expect_exit 11 "$MJ" context check-sync --json
[ -n "$LAST_OUT" ] || { echo "    check-sync --json printed nothing for a stale projection"; exit 1; }
while IFS= read -r line; do
  [ -n "$line" ] || continue
  printf '%s\n' "$line" | jq -e 'type == "object" and has("level") and has("category") and has("subject") and has("message")' >/dev/null \
    || { echo "    not a finding object: $line"; exit 1; }
done <<<"$LAST_OUT"
expect_grep '"message":"[^"]*stale-projection'
expect_no_grep '^[^{]'

# --- regenerating passes
expect_exit 0 "$MJ" update --force
expect_exit 0 "$MJ" context check-sync
expect_no_grep 'stale-projection'
[ -z "$(git status --porcelain)" ] || { echo "    regeneration left the tree different from the commit"; git status --porcelain; exit 1; }

# --- a semantic no-op is not drift: re-saving a file unchanged
cp .ai/README.md "$T/same.md" && cp "$T/same.md" .ai/README.md
expect_exit 0 "$MJ" context check-sync
expect_no_grep '^(FAIL|DRIFT)'

# --- nor is whitespace outside the front matter
printf '\n\n' >> .ai/repo/README.md
expect_exit 0 "$MJ" context check-sync
expect_no_grep '^(FAIL|DRIFT)'
git checkout -q -- .

# --- an invalid tree is invalid before it is anything else
cp .ai/repo/README.md .ai/repo/workflows/twin.md
expect_exit 10 "$MJ" context check-sync
expect_grep 'duplicate-id'
rm -f .ai/repo/workflows/twin.md
expect_exit 0 "$MJ" context check-sync
