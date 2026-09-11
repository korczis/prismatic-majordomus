# majordomus-exclusive: measures wall time against a budget; a busy pool would add the noise it measures
#
# The Cockpit answers, answers quickly, and offers everything it answers.
#
# Three facts, each of which was false on master and each of which is measured here rather
# than reasoned about:
#
#   1. /cockpit/worktrees is the one page that runs git inside every registered work tree.
#      It took 34.2 s on a repository with 111 of them — two full topology walks per render,
#      two subprocesses per work tree per walk, taken one at a time. The budget is a policy
#      value, not a number in this file: `benchmark.budget.cockpit_page_ms` for a page that
#      is a projection of state the process already holds, plus
#      `benchmark.budget.cockpit_worktrees_ms_per_worktree` for each work tree git has
#      registered, because that page's cost is per work tree and not per page.
#
#      Wall time alone is a weak guard in a fixture with a handful of tiny work trees, so
#      the subprocess census is the real assertion: a `git` on PATH that records what it was
#      asked. One render must run at most one `status` per work tree and no
#      `rev-parse --absolute-git-dir` at all. The serial form ran four subprocesses per work
#      tree per render and fails this whether the machine is fast or slow.
#
#   2. Every area the Cockpit serves is an area the sidebar offers, and every area the
#      sidebar offers is named by the feature declaration. /cockpit/activity answered 200
#      with real content and appeared in neither, because the three were three hand-kept
#      lists. They are one table now (`cockpit::nav::areas`), and this checks all three ends
#      of it — the served route, the offered link, and `.ai/repo/features/cockpit.md`.
#
#   3. /docs is a declared surface, and in a checkout that has not built the site it
#      answered 503 with a JSON error object. A declared surface that refuses is the defect;
#      it answers with a page naming the producer that builds it.
#
# And the claim the Cockpit makes about itself — "with JavaScript off every page still shows
# everything it knows" — is checked on the one page that could not keep it: search, whose
# results are 157 characters of shell until a query arrives.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
# Before init, not after: the skeleton this fixture is seeded from is the one in MAJORDOMUS_SHARE,
# and an inherited MAJORDOMUS_SHARE pointing at another checkout seeds another checkout's policy.
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# --- the budget, read from the policy: this file states no milliseconds of its own
page_ms="$(sed -n 's/^ *cockpit_page_ms: *\([0-9]*\).*/\1/p' .ai/repo/policy.yaml | head -1)"
per_wt_ms="$(sed -n 's/^ *cockpit_worktrees_ms_per_worktree: *\([0-9]*\).*/\1/p' .ai/repo/policy.yaml | head -1)"
if [ -z "$page_ms" ] || [ -z "$per_wt_ms" ]; then
  echo "    the policy declares no Cockpit budget (benchmark.budget.cockpit_page_ms,"
  echo "    benchmark.budget.cockpit_worktrees_ms_per_worktree); the budget is data, and"
  echo "    this case will not invent one"; exit 1
fi

# --- a repository that has a site and has not built it, which is the normal case and the
#     one /docs answered 503 in: the surface is declared by site/config.toml being there,
#     and target/web/docs is written by a build nobody has run here.
mkdir -p site && printf 'base_url = "http://127.0.0.1"\ntitle = "fixture"\n' > site/config.toml
git add site/config.toml && git commit -qm site >/dev/null

# --- a repository with more than one work tree, so the per-work-tree budget has a
#     denominator and the census has something to count
# Created where they belong, through the tool: a misplaced work tree is a migration step,
# a step is a work tree the plan reads, and the census below would then be counting the
# fixture's own disorder instead of the page's behaviour.
for b in alpha beta gamma delta; do
  "$RB" worktree create "$b" --repo "$T" >/dev/null 2>&1 || true
done
worktrees="$(git worktree list --porcelain | grep -c '^worktree ')"
[ "$worktrees" -ge 2 ] || { echo "    the fixture has $worktrees work tree(s); nothing to measure"; exit 1; }
budget_worktrees=$(( page_ms + worktrees * per_wt_ms ))

# --- a git that records what it was asked, ahead of the real one on the server's PATH
real_git="$(command -v git)"
mkdir -p "$T/shim"
cat > "$T/shim/git" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$T/git-calls.log"
exec "$real_git" "\$@"
EOF
chmod +x "$T/shim/git"
: > "$T/git-calls.log"

trap 'if [ -n "${SRV:-}" ]; then kill "$SRV" 2>/dev/null || true; fi' EXIT
PATH="$T/shim:$PATH" "$RB" serve --repo "$T" --port 0 > "$T/serve.out" 2> "$T/serve.err" &
SRV=$!
base=""
waited=0
while [ "$waited" -lt 120 ]; do
  base="$(sed -n 's|.*listening on \(http://127\.0\.0\.1:[0-9]*\).*|\1|p' "$T/serve.err" | head -1)"
  if [ -n "$base" ]; then break; fi
  kill -0 "$SRV" 2>/dev/null || break
  sleep 0.5; waited=$((waited + 1))
done
[ -n "$base" ] || { echo "    the server never named an address"; sed -n '1,40p' "$T/serve.err"; exit 1; }
curl -fsS -m 30 -o /dev/null "$base/cockpit" || { echo "    the Cockpit does not answer at $base"; exit 1; }

# How long one GET took, in whole milliseconds. awk and not bc: bc is not on every machine
# this suite runs on, and a case that skips on a missing calculator measures nothing.
took_ms() {
  curl -s -m 120 -o "$T/body" -w '%{time_total}\n' "$base$1" | awk '{printf "%d", $1 * 1000}'
}

# ---------------------------------------------------------------- 1. the areas are one list
# What the sidebar offers, read off the page the server rendered.
curl -fsS -m 30 "$base/cockpit" > "$T/overview.html"
grep -oE 'href="/cockpit(/[a-z]+)?"' "$T/overview.html" \
  | sed -E 's|^href="/cockpit||; s|^/||; s|"$||' | sed 's|^$|overview|' \
  | LC_ALL=C sort -u > "$T/offered"
[ -s "$T/offered" ] || { echo "    the sidebar offers nothing"; exit 1; }

# What the feature declaration names. The repository's own, not the fixture's: the areas are
# the executable's table, and the declaration that must agree with it is this repository's.
sed -n 's/^cockpit: *\[\(.*\)\] *$/\1/p' "$ROOT/.ai/repo/features/cockpit.md" \
  | tr ',' '\n' | sed 's/^ *//; s/ *$//' | grep -v '^$' | LC_ALL=C sort -u > "$T/declared"
[ -s "$T/declared" ] || { echo "    .ai/repo/features/cockpit.md declares no cockpit: areas"; exit 1; }

if ! diff -u "$T/declared" "$T/offered" > "$T/areas.diff"; then
  echo "    the sidebar and .ai/repo/features/cockpit.md name different areas."
  echo "    They are projections of one table (cockpit::nav::areas); a difference here is"
  echo "    the three-lists defect coming back. Left: declared. Right: offered."
  cat "$T/areas.diff"
  exit 1
fi

# and every offered area is a route that answers
while IFS= read -r a; do
  path="/cockpit"; [ "$a" = overview ] || path="/cockpit/$a"
  code="$(curl -s -m 120 -o /dev/null -w '%{http_code}' "$base$path")"
  [ "$code" = 200 ] || { echo "    the sidebar offers $path and it answers $code"; exit 1; }
done < "$T/offered"

# the page that was served and offered by nobody, named so that a silent removal is caught
grep -qx activity "$T/offered" || { echo "    /cockpit/activity is served and not offered"; exit 1; }

# ---------------------------------------------------------------- 2. every page in budget
while IFS= read -r a; do
  path="/cockpit"; [ "$a" = overview ] || path="/cockpit/$a"
  if [ "$a" = worktrees ]; then budget="$budget_worktrees"; else budget="$page_ms"; fi
  ms="$(took_ms "$path")"
  if [ "$ms" -gt "$budget" ]; then
    echo "    $path took ${ms} ms, over its budget of ${budget} ms"
    if [ "$a" = worktrees ]; then
      echo "    (cockpit_page_ms ${page_ms} + ${worktrees} work tree(s) x cockpit_worktrees_ms_per_worktree ${per_wt_ms})"
    else
      echo "    (policy benchmark.budget.cockpit_page_ms)"
    fi
    exit 1
  fi
done < "$T/offered"

# ---------------------------------------------------------------- 3. the subprocess census
# One render of the one page that asks git about every work tree. What it may run: one
# `status` per work tree and no `rev-parse --absolute-git-dir` at all. The serial form ran
# two of each per work tree per walk and walked twice.
: > "$T/git-calls.log"
curl -fsS -m 120 -o /dev/null "$base/cockpit/worktrees"
statuses="$(grep -c '^status ' "$T/git-calls.log" || true)"
absgit="$(grep -c 'rev-parse --absolute-git-dir' "$T/git-calls.log" || true)"
if [ "$statuses" -gt "$worktrees" ]; then
  echo "    one /cockpit/worktrees render ran $statuses 'git status' for $worktrees work tree(s);"
  echo "    at most one each. More than one means the page is walking the topology twice"
  echo "    (the migration plan reads the work trees it plans for, never all of them)."
  sed -n '1,40p' "$T/git-calls.log"
  exit 1
fi
if [ "$absgit" != 0 ]; then
  echo "    one render ran $absgit 'git rev-parse --absolute-git-dir'; the git directory of a"
  echo "    work tree is read from its .git entry, not asked for with a subprocess."
  exit 1
fi

# ---------------------------------------------------------------- 4. /docs is a page
code="$(curl -s -m 30 -o "$T/docs.html" -w '%{http_code}' "$base/docs/")"
if [ "$code" = 503 ]; then echo "    /docs still answers 503; a declared surface that refuses is the defect"; exit 1; fi
[ "$code" = 200 ] || { echo "    /docs answers $code"; exit 1; }
grep -qi '<html' "$T/docs.html" || { echo "    /docs answers 200 and is not a page"; cat "$T/docs.html"; exit 1; }
grep -q 'site-build' "$T/docs.html" || { echo "    the /docs page does not name the producer that builds it"; exit 1; }
grep -q '"ready": false' <(curl -fsS -m 30 "$base/") || {
  echo "    the home page no longer reports the unbuilt surface as not ready; the page must"
  echo "    not have cost the machine-readable half"; exit 1; }

# ---------------------------------------------------------------- 5. search without scripts
# The claim is the Cockpit's own: with JavaScript off every page still shows everything it
# knows. An empty search page knows nothing yet and must still be usable — a real GET form —
# and a search with a query must render its results in the HTML, not fetch them.
curl -fsS -m 30 "$base/cockpit/search" > "$T/search.html"
grep -q '<form[^>]*method="get"' "$T/search.html" || { echo "    the empty search page has no GET form; it needs a script to search"; exit 1; }
grep -q 'name="q"' "$T/search.html" || { echo "    the search form has no q field"; exit 1; }
# A capability page link is the discriminator: the sidebar carries `/cockpit/capabilities`
# and `?module=` on every page and never a capability's own page, so one of those in the
# HTML is a result the server rendered and not furniture.
curl -fsS -m 60 "$base/cockpit/search?q=worktree" > "$T/results.html"
[ "$(grep -c 'href="/cockpit/capabilities/' "$T/search.html" || true)" = 0 ] || {
  echo "    the empty search page already carries result links; the discriminator is wrong"
  exit 1; }
hits="$(grep -c 'href="/cockpit/capabilities/' "$T/results.html" || true)"
[ "$hits" -gt 0 ] || {
  echo "    a query rendered no results in the HTML; the page needs a script to search"
  exit 1; }

kill "$SRV" 2>/dev/null || true; SRV=""
echo "    $worktrees work tree(s): every area offered, served and under budget; one render ran $statuses status call(s) and $absgit absolute-git-dir call(s); /docs is a page; search rendered $hits result link(s) with no script"
