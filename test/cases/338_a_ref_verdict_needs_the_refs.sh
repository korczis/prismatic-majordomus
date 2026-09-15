# majordomus-covers: none
# A verdict about a ref is worth only what the clone was given.
#
# `scripts/ci/link-check` decides the site's links to the forge against the local repository.
# Two of its questions need history (`commit/<sha>`, `compare/a...b`) and two need tags
# (`releases/tag/<t>`, `tree/<tag>`). The history half already refused to decide when the
# clone was shallow; the tag half asserted instead, and `v0.3.1`, `v0.5.0` and `v0.6.0` — three
# tags with published releases — were reported as `does not exist` on every Pages run after
# #356, because `actions/checkout` had `fetch-depth: 1` and fetches no tags at that depth.
# Six false findings, and `publish` skipped behind them: the site stopped updating for a day.
#
# The distinction this case holds is not "be quiet when unsure". It is that absence of evidence
# is reported as a refusal (exit 12, which site-check still counts as a failure) and never as a
# finding about the subject. §3 is what stops the lazy repair — a checker that simply never
# decides a tag would pass §2 and fail there.
. "$ROOT/test/lib.sh"

R="$T/site"
mkdir -p "$R/site/public" "$R/site/data/generated" "$R/scripts/ci"
cp "$ROOT/scripts/ci/link-check" "$R/scripts/ci/link-check"
chmod +x "$R/scripts/ci/link-check"

cat > "$R/site/config.toml" <<'TOML'
base_url = "https://example.test"
TOML
cat > "$R/site/data/generated/project.json" <<'JSON'
{ "repository_url": "https://github.com/example/repo" }
JSON

page() {
  cat > "$R/site/public/index.html" <<HTML
<html><body>
$(for u in "$@"; do printf '<a href="%s">x</a>\n' "$u"; done)
</body></html>
HTML
}

cd "$R" || { echo "    could not enter the fixture"; exit 1; }
git init -q .
git add -A && git -c user.email=t@t -c user.name=t commit -qm init >/dev/null
git tag v1.0.0

# `rc=0; cmd || rc=$?`, never `cmd; rc=$?`: this case's whole subject is non-zero exits, and
# under `set -e` the second form takes the case down before the assignment is ever reached.
run() { rc=0; MJ_ROOT="$1" "$1/scripts/ci/link-check" > "$2" 2> "$3" || rc=$?; }

# --- 1. with the tag in the clone, an existing tag is decided clean
page "https://github.com/example/repo/releases/tag/v1.0.0" \
     "https://github.com/example/repo/tree/v1.0.0"
run "$R" "$T/out" "$T/err"
[ "$rc" = 0 ] || {
  echo "    a clone holding the tag did not decide it clean; exit $rc"
  cat "$T/out" "$T/err" | head -5; exit 1; }
echo "    a tag the clone holds is decided clean"

# --- 2. the same links, in a clone fetched without tags, are refused and never denied
# `--depth 1 --no-tags` is what `actions/checkout@v4` with `fetch-depth: 1` produces, and it is
# the exact condition under which the six false findings were emitted.
S="$T/shallow"
git clone -q --depth 1 --no-tags "file://$R" "$S" 2>/dev/null || {
  echo "    could not make a shallow clone of the fixture"; exit 1; }
mkdir -p "$S/scripts/ci"
# The clone carries what was committed; the page under site/public is written per section and is
# untracked, so it is copied across rather than expected to arrive with the history.
mkdir -p "$S/site/public"
cp -R "$R/site/public/." "$S/site/public/"
cp "$R/scripts/ci/link-check" "$S/scripts/ci/link-check"
chmod +x "$S/scripts/ci/link-check"
[ "$(git -C "$S" tag -l | wc -l | tr -d ' ')" = 0 ] || {
  echo "    the fixture clone has tags, so it cannot stand for the CI checkout"; exit 1; }

run "$S" "$T/out2" "$T/err2"
if grep -q 'does not exist\|is no ref of this repository' "$T/out2" "$T/err2"; then
  echo "    a clone with no tags denied a tag it was never given:"
  grep -h 'does not exist\|is no ref' "$T/out2" "$T/err2" | head -3
  exit 1
fi
[ "$rc" = 12 ] || {
  echo "    a clone that cannot decide a tag exited $rc, not 12 (a refusal)"
  cat "$T/out2" "$T/err2" | head -5; exit 1; }
grep -q 'tags' "$T/err2" || {
  echo "    the refusal does not name tags as what the clone lacks:"; cat "$T/err2" | head -3; exit 1; }
echo "    a clone fetched without tags refuses to decide, and denies nothing"

# --- 3. the mutation: a tag that truly is absent is still a finding
# Without this, "never decide a tag" would pass §2 and the check would be decorative. The clone
# here holds tags — it simply does not hold this one — so the verdict is available and owed.
page "https://github.com/example/repo/releases/tag/v9.9.9"
run "$R" "$T/out" "$T/err"
[ "$rc" = 10 ] || {
  echo "    a tag absent from a clone that holds tags exited $rc, not 10 (a finding)"
  cat "$T/out" "$T/err" | head -5; exit 1; }
grep -q 'v9.9.9' "$T/out" || {
  echo "    the finding does not name the missing tag:"; cat "$T/out" | head -3; exit 1; }
echo "    a tag absent from a clone that holds tags is still a finding"

echo "    a ref verdict needs the refs: missing evidence refuses, it does not deny"
