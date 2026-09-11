# majordomus-covers: none
# A publication that is owed is refused when the deploy is not coming, and not when it is.
#
# `scripts/ci/pages-check` decides whether the published site still projects master. When
# master carries a commit the site was not built from, the site owes a publication, and the
# gate has to tell a deploy in flight from a deploy that is never going to happen. It judged
# that by the commit's age, and a commit's age is not the obligation's: a commit authored an
# hour before it is pushed is overdue the instant it lands. On 2026-09-10 the gate failed
# with "changed what it is built from 3862s ago" ninety seconds before that very deploy
# succeeded — the commit was an hour old and the push was two minutes old.
#
# The question is now asked of the deploy rather than of the clock, and this case measures
# both directions over one fixture whose owed commit is deliberately far older than the
# window: with a run in flight the gate passes, and with no run at all it still refuses. The
# second half is what stops the exemption from becoming a way of never refusing anything.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/pages-check"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj127.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# --- a repository whose site is one commit behind, and whose commits are an hour old ------
# The gate reads the published commit out of the deploy commit's subject on gh-pages, so the
# fixture is a bare origin with both branches and a clone that carries the gate itself (the
# gate measures the repository it lives in).
OLD="2026-09-10T21:00:00 +0000"          # comfortably outside the 1800s window
export GIT_AUTHOR_DATE="$OLD" GIT_COMMITTER_DATE="$OLD"
mkdir -p "$S/origin" && ( cd "$S/origin" && git init -q --bare . )
mkdir -p "$S/work" && cd "$S/work"
git init -q . && git config user.email t@example.com && git config user.name t
git remote add origin "$S/origin"
mkdir -p scripts/ci && cp "$GATE" scripts/ci/pages-check && chmod +x scripts/ci/pages-check
echo one > page.md
git add -A && git commit -qm "the tree the site was built from"
published="$(git rev-parse HEAD)"
git push -q origin HEAD:refs/heads/master

# gh-pages, naming that commit in the subject scripts/site-deploy writes
git checkout -q --orphan gh-pages && git rm -rqf . 2>/dev/null || true
echo built > index.html
git add -A && git commit -qm "deploy: site from ${published:0:7} (registry deadbeef), 1 routes"
git push -q origin HEAD:refs/heads/gh-pages
git checkout -q master

# and one more commit on master, which is what makes a publication owed
echo two >> page.md
git add -A && git commit -qm "a change the site has not been built from"
owed="$(git rev-parse HEAD)"
git push -q origin HEAD:refs/heads/master
unset GIT_AUTHOR_DATE GIT_COMMITTER_DATE

run_gate() { env -u MAJORDOMUS_SHARE MJ_PAGES_RUNS_FIXTURE="${1:-}" scripts/ci/pages-check --offline; }

# --- 1. no deploy at all: the refusal stands ----------------------------------------------
: > "$S/none.tsv"
out="$(run_gate "$S/none.tsv" 2>&1)" && {
  echo "    a publication owed for an hour with no deploy was accepted"; echo "$out"; exit 1; }
case "$out" in
  *"owes a publication"*) ;;
  *) echo "    refused, but not for the owed publication:"; echo "$out"; exit 1 ;;
esac
echo "  a publication nobody is deploying is still refused"

# --- 2. the deploy is in flight: the same tree passes --------------------------------------
# Nothing about the tree changed between the two runs; only the answer to "is it happening".
printf '%s\tin_progress\t\n' "$owed" > "$S/flight.tsv"
out="$(run_gate "$S/flight.tsv" 2>&1)" || {
  echo "    a deploy in flight was refused as an overdue publication"; echo "$out"; exit 1; }
case "$out" in
  *"deploy is in_progress right now"*) ;;
  *) echo "    passed, but not because the deploy is in flight:"; echo "$out"; exit 1 ;;
esac
echo "  a publication whose deploy is running is not overdue"

# queued is the same answer: a run waiting for a runner is a deploy that is coming, and this
# repository has waited twenty-five minutes for one.
printf '%s\tqueued\t\n' "$owed" > "$S/queued.tsv"
run_gate "$S/queued.tsv" >/dev/null 2>&1 || { echo "    a queued deploy was refused"; exit 1; }
echo "  a queued deploy is a deploy that is coming"

# --- 3. a finished run is not an excuse ----------------------------------------------------
# The exemption is for a deploy that is happening, not for one that happened and failed:
# otherwise a failed deploy would exempt itself forever.
printf '%s\tcompleted\tfailure\n' "$owed" > "$S/failed.tsv"
out="$(run_gate "$S/failed.tsv" 2>&1)" && {
  echo "    a deploy that already failed excused the publication it owes"; echo "$out"; exit 1; }
case "$out" in
  *"owes a publication"*) ;;
  *) echo "    refused, but not for the owed publication:"; echo "$out"; exit 1 ;;
esac
echo "  a deploy that already failed excuses nothing"

# --- 4. the fixture names another commit ---------------------------------------------------
# A run in flight for some other commit says nothing about this one.
printf '%s\tin_progress\t\n' "$published" > "$S/other.tsv"
run_gate "$S/other.tsv" >/dev/null 2>&1 && {
  echo "    a deploy of a different commit excused this one"; exit 1; }
echo "  a deploy of another commit is not this commit's deploy"
