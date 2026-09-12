# The sessions section publishes exactly what it says it publishes.
#
# `/sessions/` on the public site is introduced, in its own words, as "Every closed execution
# episode this repository holds: what it started from, what it produced, and how it ended.
# Written by the tool from git and the ledger, never from a conversation." That is a claim with
# a quantifier in it, and a quantifier is a relation checked in one direction unless somebody
# checks the other: everything published is a record, and every record is published.
#
# Only the first direction had an owner. The pages are generated from `session list --all`,
# which reads `.ai/repo/sessions/`, so a published page is a record by construction. Nothing
# asked the converse. A record the tool cannot parse, one whose front matter drifted, one
# written by a lifecycle hook that changed its stem — any of those disappears from the section
# and the page still says "every". That is this repository's most repeated defect class, and
# the session record is the one durable carrier the whole lifecycle rests on.
#
# The distinction the page depends on is also asserted here: what may be published is the
# TRACKED record under `.ai/repo/sessions/`, and never the working session context under
# `.ai/local/session-contexts/`, which is the same episode in the half of the layer that is
# nobody's business. The two differ by one directory and by everything else.
. "$ROOT/test/lib.sh"

command -v jq >/dev/null 2>&1 || { echo "    skip: jq is required"; exit 0; }
S="$ROOT/site/data/generated/sessions.json"
[ -f "$S" ] || { echo "    site/data/generated/sessions.json is missing; run scripts/derive"; exit 1; }
INDEX="$ROOT/site/content/sessions/_index.md"
[ -f "$INDEX" ] || { echo "    site/content/sessions/_index.md is missing; run scripts/derive"; exit 1; }

# --- the dataset names the tracked half of the layer, and only it
src="$(jq -r '.source' "$S")"
[ "$src" = ".ai/repo/sessions/" ] || {
  echo "    the published sessions dataset says it comes from '$src', not .ai/repo/sessions/"
  echo "    the local half of the layer holds the working session contexts and is never published"
  exit 1; }
if grep -q 'local/session-contexts' "$S"; then
  echo "    the published sessions dataset names .ai/local/session-contexts/:"
  grep -o '[^"]*local/session-contexts[^"]*' "$S" | head -3
  exit 1
fi

# --- the section still claims every episode; if it stops, the assertions below change with it
grep -q 'Every closed execution episode this repository holds' "$INDEX" || {
  echo "    the sessions section no longer claims to publish every episode; this case asserts"
  echo "    the claim that is written there, so it must be rewritten with the claim:"
  sed -n '1,8p' "$INDEX"
  exit 1; }

# --- every record is published
#
# The record's stem is <stamp>--<session id>--<branch>--<commit>--<digest>.md; the id is the
# second field. README.md is the directory's context document, not a record.
records="$(mktemp "${TMPDIR:-/tmp}/mj-rec.XXXXXX")"
published="$(mktemp "${TMPDIR:-/tmp}/mj-pub.XXXXXX")"
pages="$(mktemp "${TMPDIR:-/tmp}/mj-pag.XXXXXX")"
trap 'rm -f "$records" "$published" "$pages"' EXIT
for f in "$ROOT"/.ai/repo/sessions/*.md; do
  b="$(basename "$f")"
  case "$b" in README.md) continue ;; esac
  printf '%s\n' "$b" | awk -F'--' '{print $2}'
done | LC_ALL=C sort -u > "$records"
jq -r '.sessions[].session_id' "$S" | LC_ALL=C sort -u > "$published"
ls "$ROOT"/site/content/sessions/*.md 2>/dev/null | while IFS= read -r f; do
  b="$(basename "$f" .md)"; case "$b" in _index) continue ;; esac; printf '%s\n' "$b"
done | LC_ALL=C sort -u > "$pages"

missing="$(comm -23 "$records" "$published")"
if [ -n "$missing" ]; then
  echo "    the section says it publishes every episode, and these records are not in it:"
  printf '%s\n' "$missing" | sed 's/^/      /'
  echo "    a record the tool cannot read disappears from the site while the page still says 'every'"
  exit 1
fi

# --- and everything published is a record
extra="$(comm -13 "$records" "$published")"
if [ -n "$extra" ]; then
  echo "    the section publishes episodes that .ai/repo/sessions/ does not hold:"
  printf '%s\n' "$extra" | sed 's/^/      /'
  exit 1
fi

# --- and the rendered pages are the same set again
#
# The dataset and the content tree are written by two different stages of the generator, so
# agreeing with the records is not the same fact as agreeing with each other.
if ! diff -q "$published" "$pages" >/dev/null 2>&1; then
  echo "    the published dataset and the rendered pages are different sets:"
  diff "$published" "$pages" | head -10
  exit 1
fi

# --- the count the dataset states is the count it carries
n="$(jq -r '.count' "$S")"
have="$(jq -r '.sessions | length' "$S")"
[ "$n" = "$have" ] || {
  echo "    the dataset states count $n and carries $have session(s)"; exit 1; }
[ "$n" = "$(wc -l < "$records" | tr -d ' ')" ] || {
  echo "    the dataset states count $n and the layer holds $(wc -l < "$records" | tr -d ' ') record(s)"; exit 1; }
