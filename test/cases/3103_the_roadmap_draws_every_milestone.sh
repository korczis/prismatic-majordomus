# majordomus-covers: none
# majordomus-skip-runs-in: site
# /roadmap/ draws every milestone of the plan, in the milestone graph's order, each issue under
# its milestone grouped by declared status, and names the commit its data was observed at
# (I2743, ADR 0166 decision 4). It used to keep the eight milestones whose hand-typed version
# was set and drop the thirty where the work happened.
#
# The real site/ is rendered by Zola in a copy (the committed data, a build.json naming a
# commit), and `scripts/site-check --roadmap` — check 13h of the full site-check, run alone —
# is held to its verdict both ways: green over the page as built, red over a page that drops a
# milestone, one that reorders them, and one that names another commit than the build's.
# The inferred delivery (plan.delivery, I2740) is drawn only when delivery.json is there: absent,
# the page says so and draws no stage; present, a merged issue shows `merged`, names its pull
# request and links its merge commit, and a released one names its tag. The pull request is named,
# not linked: the published site's link check decides every link offline, and a pull link cannot
# be decided offline, while the merge commit is in the published history.
. "$ROOT/test/lib.sh"
mj_zola_ready "$ROOT" || skip "$(mj_zola_why "$ROOT")"
command -v jq >/dev/null || skip "jq absent"

SITECHECK="$ROOT/scripts/site-check"
COMMIT=0123456789abcdef0123456789abcdef01234567
REPO="$(jq -r .repository_url "$ROOT/site/data/generated/project.json")"

# a copy of the site's sources: templates, content, the committed data, and the few assets
# scripts/site-build writes that Zola resolves with cachebust (empty here; nothing is styled)
mkdir -p S
( cd "$ROOT/site" && tar cf - --exclude ./public . ) | ( cd S && tar xf - )
mkdir -p S/static/js S/static/graphs
: > S/static/app.css; : > S/static/kit.css
for js in "$ROOT"/site/*.js; do cp "$js" S/static/js/; done
rm -f S/data/generated/delivery.json S/static/build.json
build_json() { # commit
  printf '{"schema":1,"commit":"%s","commit_time":"2026-10-16T12:00:00+00:00","dirty":false,"surfaces":[],"snapshots":[],"release":null}\n' "$1" > S/data/build.json
}
render() { # out-dir
  ( cd S && zola build --force --output-dir "../$1" ) > zola.log 2>&1 \
    || { tail -20 zola.log; echo "    zola could not render the site copy"; exit 1; }
  [ -f "$1/roadmap/index.html" ] || { echo "    no /roadmap/ was rendered into $1"; exit 1; }
}
DATA="$ROOT/site/data/generated"

# --- the page as built: every milestone, in order, every issue, the build's commit
build_json "$COMMIT"
render out
P="out/roadmap/index.html"
# MUTATION, RUN: restore the old filter (`{%- for m in roadmap.milestones %}{% if m.released_in
# != "unreleased" %}`) -> only the milestones a release carried remain, and 13h refuses the page
# naming every milestone and issue it lost and the broken order.
expect_exit 0 "$SITECHECK" --roadmap "$P" "$DATA" S/data/build.json
expect_grep "^OK   roadmap +/roadmap/ draws all $(jq '.milestones | length' "$DATA/plan.json") milestone"
[ "$(grep -o 'data-milestone="' "$P" | wc -l | tr -d ' ')" = "$(jq '.milestones | length' "$DATA/plan.json")" ] \
  || { echo "    /roadmap/ does not carry one card per milestone of plan.json"; exit 1; }
# the observed commit links to its commit, with the commit's own time
expect_grep "href=\"$REPO/commit/$COMMIT\"" "$P"
expect_grep 'data-observed-time="2026-10-16T12:00:00\+00:00"' "$P"
# every milestone links to its page, and the graph's nodes do too
for id in $(jq -r '.milestones[].id' "$DATA/plan.json"); do
  low="$(printf '%s' "$id" | tr 'A-Z' 'a-z')"
  grep -qF "/plan/$low/\"" "$P" || { echo "    milestone $id does not link to its page /plan/$low/"; exit 1; }
  [ -f "out/plan/$low/index.html" ] || { echo "    milestone $id has no page to link to"; exit 1; }
done
# issues are grouped by declared status, in the engine's order of statuses
first_ms="$(jq -r '[.issues[] | .milestone] | first' "$DATA/plan.json")"
first_st="$(jq -r --arg m "$first_ms" '[.issues[] | select(.milestone == $m) | .status] | first' "$DATA/plan.json")"
expect_grep "data-issue-group=\"$first_st\"" "$P"
# no delivery.json: no stage is drawn, and the page says why
expect_no_grep 'data-delivery=' "$P"
expect_grep 'not shown: the inferred delivery projection' "$P"

# --- /plan/ draws the same plan: it names the commit it observed and, on every milestone's
#     card, the release that carried it (or unreleased), read from roadmap.json
PL="out/plan/index.html"
expect_grep "data-observed-commit=\"$COMMIT\"" "$PL"
[ "$(grep -o 'data-released-in="' "$PL" | wc -l | tr -d ' ')" = "$(jq '.milestones | length' "$DATA/plan.json")" ] \
  || { echo "    /plan/ does not show a release on every milestone card"; exit 1; }
for row in $(jq -r '.milestones[] | "\(.released_in)"' "$DATA/roadmap.json" | sort | uniq -c | awk '{print $2"="$1}'); do
  [ "$(grep -o "data-released-in=\"${row%=*}\"" "$PL" | wc -l | tr -d ' ')" = "${row#*=}" ] \
    || { echo "    /plan/ shows ${row%=*} on a different number of milestones than roadmap.json"; exit 1; }
done

# --- 13h refuses a page that drops a milestone
drop="$(jq -r '.milestones[-1].id' "$DATA/roadmap.json")"
jq --arg d "$drop" '.milestones |= map(select(.id != $d))' "$DATA/roadmap.json" > S/data/generated/roadmap.json
render out-drop
expect_exit 10 "$SITECHECK" --roadmap out-drop/roadmap/index.html "$DATA" S/data/build.json
expect_grep "milestone $drop is in plan.json but not on /roadmap/"

# --- 13h refuses a page whose order is not the graph's
jq '.milestones |= reverse' "$DATA/roadmap.json" > S/data/generated/roadmap.json
render out-order
expect_exit 10 "$SITECHECK" --roadmap out-order/roadmap/index.html "$DATA" S/data/build.json
expect_grep 'does not list the milestones once each in the order roadmap.json derives'
cp "$DATA/roadmap.json" S/data/generated/roadmap.json

# --- 13h refuses a page that names another commit than the build's
build_json fedcba9876543210fedcba9876543210fedcba98
expect_exit 10 "$SITECHECK" --roadmap "$P" "$DATA" S/data/build.json
expect_grep "names $COMMIT as observed, the build records fedcba9876543210fedcba9876543210fedcba98"
build_json "$COMMIT"

# --- delivery.json present: a merged issue names its pull request and links its merge, a released one
#     its tag. The shape is plan.delivery's (schema 1); the issues are the plan's own.
mi="$(jq -r '.issues[0].id' "$DATA/plan.json")"; ri="$(jq -r '.issues[1].id' "$DATA/plan.json")"
MERGE=abcdef0123456789abcdef0123456789abcdef01
jq -n --arg mi "$mi" --arg ri "$ri" --arg c "$COMMIT" --arg m "$MERGE" '{schema: 1,
  observed: {mode: "history", trunk: "master", commit: $c, observed_at: "2026-10-16T12:00:00Z"},
  stages: ["none", "branch", "pr_open", "merged", "released", "deployed"], sources: [], tallies: {}, unsettled: 0,
  issues: [
    {issue: $mi, title: "t", milestone: "m", status: "READY", stage: "merged", settled: true, unread: [],
     merged: {commit: $m, pull_request: 4242, via: "title", merged_at: "2026-10-16T11:00:00Z", merges: 1, observed_at: "2026-10-16T12:00:00Z"}},
    {issue: $ri, title: "t", milestone: "m", status: "DONE", stage: "released", settled: true, unread: [],
     merged: {commit: $m, via: "issue", merged_at: "2026-10-16T11:00:00Z", merges: 1, observed_at: "2026-10-16T12:00:00Z"},
     released: {tag: "v0.27.0", commit: $c, observed_at: "2026-10-16T12:00:00Z"}}]}' > S/data/generated/delivery.json
render out-delivery
D="out-delivery/roadmap/index.html"
expect_exit 0 "$SITECHECK" --roadmap "$D" "$DATA" S/data/build.json
# MUTATION, RUN: load delivery.json from a path that is not there (`data/generated/delivery.json`
# renamed in the template) -> no stage is drawn, and the first assertion below fails.
expect_grep 'data-delivery="merged"' "$D"
expect_grep '>#4242</span>' "$D"
# MUTATION, RUN: link the pull request again -> the published site's link check refuses /roadmap/
grep -q "href=\"$REPO/pull/" "$D" && { echo "    the page links a pull request, which the link check cannot decide offline"; exit 1; }
expect_grep "href=\"$REPO/commit/$MERGE\"" "$D"
expect_grep 'data-delivery="released"' "$D"
expect_grep "href=\"$REPO/releases/tag/v0.27.0\"" "$D"
expect_grep "inferred from the history at <a href=\"$REPO/commit/$COMMIT\"" "$D"
# the homepage's delivery card links /roadmap/#delivery; the anchor is the page's own
expect_grep '<dt id="delivery"' "$D"
[ "$(grep -o 'data-delivery="' "$D" | wc -l | tr -d ' ')" = 2 ] \
  || { echo "    the page drew a stage for an issue delivery.json does not name"; exit 1; }

# --- the page's front matter names the plan's real home, not the path it moved out of
expect_no_grep '\.majordomus/project' "$ROOT/site/content/roadmap.md"
expect_grep '^source = "\.ai/repo/project/milestones/"' "$ROOT/site/content/roadmap.md"
