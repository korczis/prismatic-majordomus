# majordomus-covers: none
# The site shows what moved since its previously deployed commit, computed when the site is
# built and never committed.
#
# scripts/site-build composes, for the commit being built and the commit the deployment it
# replaces was built from, the movements of three kinds — capabilities (`release analyze
# --since`), documentation (the changelog composition's docs changes) and claims (the claims
# projection committed at each end) — each labelled with the release record whose tree first
# carries it, or not yet released. It writes them to site/data/since-deploy.json, which is
# ignored like build.json, and /changelog/ renders them (site/templates/partials/since-deploy.html).
#
# The fixture is a real repository with a real history, measured by the real executable:
#
#   C0  deployed       the registry lacks one capability; claims kept, promoted (planned), old
#   C1  docs commit    the capability arrives, claim fresh is added; the release v0.2.0 is cut here
#   C2  the record     .ai/repo/releases/v0.2.0.yaml names C1
#   C3  docs commit    promoted becomes guaranteed, old is removed, future is added as planned
#
#   1. from C0 to C3, every movement is found on the rendered page under its group: the
#      capability and claim fresh and the first document released in v0.2.0, the rest not yet
#      released, and the planned claim not at all;
#   2. with no previous commit, with one that is not an ancestor of the built commit, and with
#      no record at all, the page says the delta is unknown and lists nothing;
#   3. the deploy branch names the previous commit: the newest `source:` that is not the commit
#      being built;
#   4. built twice from one commit with two different previous commits, no tracked file
#      changes; scripts/site-check refuses the record when it is tracked;
#   5. the full build writes the record beside build.json, the deploy and the Pages workflow
#      hand it the previous deployment, and the homepage's trust strip links the section.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
command -v zola >/dev/null 2>&1 || skip "zola absent"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
# site-build reaches the executable through bin/majordomus-cli, which uses this one and never builds
MAJORDOMUS_BIN="$RB"; MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_BIN MAJORDOMUS_SHARE
BUILD="$ROOT/scripts/site-build"
PARTIAL="$ROOT/site/templates/partials/since-deploy.html"
[ -x "$BUILD" ] && [ -f "$PARTIAL" ] || { echo "    a unit of the mechanism is missing"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj-1060.XXXXXX")"; trap 'rm -rf "$S"' EXIT
fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- a repository that publishes
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm "chore: install"
awk '/^  deployments: repo\/deployments$/{print; print "  releases: repo/releases"; next} {print}' \
  .ai/manifest.yaml > "$S/manifest.yaml" && mv "$S/manifest.yaml" .ai/manifest.yaml
grep -q '^  releases: repo/releases$' .ai/manifest.yaml || fail "the manifest did not gain a releases section; the skeleton's shape changed"
cat >> .ai/repo/knowledge/sources.yaml <<'Y'

  - id: release
    kind: release-record
    discovery: vcs
    pathspec: ':(glob).ai/repo/releases/*.yaml'
    required: false
Y
mkdir -p .ai/repo/releases apps/majordomus-cli share docs/generated site/data/generated
cat > .ai/repo/releases/README.md <<'Y'
---
schema: context/v1
id: ai.repo.releases
kind: context
title: Published releases
description: One record per published release, read by every projection of the release metadata.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 75
---

# Published releases
Y
printf '[package]\nname = "fixture"\nversion = "0.2.0"\nedition = "2021"\n' > apps/majordomus-cli/Cargo.toml
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\nversion=0.2.0\n' > share/version.txt
# the ignore rule is the repository's own line, so what is ignored here is what is ignored there
grep -qxF 'site/data/since-deploy.json' "$ROOT/.gitignore" || fail "the repository does not ignore site/data/since-deploy.json"
printf 'site/data/since-deploy.json\n' >> .gitignore
git add -A >/dev/null && git commit -qm "chore(fixture): the releases section and the version"

"$RB" generate registry >/dev/null 2>&1 || fail "generate registry failed in the fixture"
cp docs/generated/registry.json "$S/exact.json"
gained="$(jq -r '[.capabilities[] | select((.visibility // "public") == "public") | .id] | sort | .[0]' "$S/exact.json")"
[ -n "$gained" ] && [ "$gained" != null ] || fail "the executable declares no public capability"

# claims <id:status>...: the committed claims projection, as generate-site-data writes its claims
claims() {
  local spec
  for spec in "$@"; do
    jq -n --arg id "${spec%%:*}" --arg status "${spec#*:}" \
      '{id: $id, claim: ("The fixture claims " + $id), status: $status}'
  done | jq -s '{schema: 1, claims: .}' > site/data/generated/capabilities.json
}

# C0: what the previous deployment was built from
jq --arg a "$gained" '.capabilities |= map(select(.id != $a))' "$S/exact.json" > docs/generated/registry.json
claims kept:guaranteed promoted:planned old:guaranteed
echo "# Guide" > docs/GUIDE.md
git add -A >/dev/null && git commit -qm "chore(fixture): the tree the previous deployment was built from"
C0="$(git rev-parse HEAD)"
# C1: the capability arrives and claim fresh is added, under a documentation commit
cp "$S/exact.json" docs/generated/registry.json
claims kept:guaranteed promoted:planned old:guaranteed fresh:guaranteed
echo "The first guide." >> docs/GUIDE.md
git add -A >/dev/null && git commit -qm "docs(guide): write the first guide"
C1="$(git rev-parse HEAD)"
# C2: the record of v0.2.0, cut at C1
cat > .ai/repo/releases/v0.2.0.yaml <<Y
schema: release/v1
version: "0.2.0"
tag: v0.2.0
channel: stable
commit: $C1
published_at: "2026-10-01T00:00:00Z"
artifacts:
  - target: macos-aarch64
    name: fixture-v0.2.0-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/v0.2.0/fixture-v0.2.0.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
git add -A >/dev/null && git commit -qm "chore(release): record v0.2.0"
C2="$(git rev-parse HEAD)"
# C3: after the record — a claim promoted, one removed, one planned, and a second document
claims kept:guaranteed promoted:guaranteed fresh:guaranteed future:planned
echo "What the second deployment adds." >> docs/GUIDE.md
git add -A >/dev/null && git commit -qm "docs(guide): describe the second deployment"
C3="$(git rev-parse HEAD)"

# render <record or ""> -> the HTML of a fixture page that includes the partial over that record
render() {
  local site; site="$(mktemp -d "$S/site.XXXXXX")"
  mkdir -p "$site/templates/partials" "$site/content" "$site/data"
  printf 'base_url = "https://fixture.test"\n' > "$site/config.toml"
  cp "$PARTIAL" "$site/templates/partials/since-deploy.html"
  printf '{%% include "partials/since-deploy.html" %%}\n' > "$site/templates/page.html"
  printf '+++\ntitle = "changelog"\n+++\n' > "$site/content/changelog.md"
  [ -z "$1" ] || cp "$1" "$site/data/since-deploy.json"
  ( cd "$site" && zola build >/dev/null 2>&1 ) || { echo "    zola could not build the fixture page" >&2; return 1; }
  # one movement per line, so that a movement and its label are read together
  awk '{ gsub(/<\/li>/, "</li>\n"); printf "%s ", $0 } END { print "" }' "$site/public/changelog/index.html"
}
# movement <html> <group> <id>: the one line of that movement
movement() { grep -F "data-movement-group=\"$2\" data-movement=\"$3\"" "$1"; }
# labelled <html> <group> <id> <label>: the movement is on the page with that release label
labelled() {
  local line; line="$(movement "$1" "$2" "$3")" || fail "the page lists no $2 movement $3"
  [ "$(printf '%s\n' "$line" | wc -l | tr -d ' ')" = 1 ] || fail "the $2 movement $3 is listed more than once"
  printf '%s' "$line" | grep -qF "data-release-label=\"$4\"" \
    || fail "the $2 movement $3 is not labelled $4: $line"
}

# ---------------------------------------------------------------- 1. the movements, grouped and labelled
expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/known.json" --previous-commit "$C0" || exit 1
[ "$(jq -r .state "$S/known.json")" = known ] || fail "the delta from C0 is not known: $(jq -c . "$S/known.json")"
[ "$(jq -r .previous "$S/known.json")" = "$C0" ] && [ "$(jq -r .built "$S/known.json")" = "$C3" ] \
  || fail "the record does not name C0 and C3: $(jq -c '{previous, built}' "$S/known.json")"
[ "$(jq -r '[.releases[].tag] | join(" ")' "$S/known.json")" = v0.2.0 ] || fail "the range does not carry exactly v0.2.0"
render "$S/known.json" > "$S/known.html" || exit 1
grep -qF 'data-since-deploy="known"' "$S/known.html" || fail "the page does not say the delta is known"
labelled "$S/known.html" capabilities "$gained" v0.2.0
labelled "$S/known.html" claims fresh v0.2.0
labelled "$S/known.html" claims promoted unreleased
labelled "$S/known.html" claims old unreleased
movement "$S/known.html" claims promoted | grep -qF 'planned &rarr; guaranteed' || fail "the promotion is not shown as planned to guaranteed"
movement "$S/known.html" claims old | grep -qF 'removed' || fail "the removed claim is not shown as removed"
! movement "$S/known.html" claims future >/dev/null || fail "a planned claim is listed as a movement"
! movement "$S/known.html" claims kept >/dev/null || fail "a claim that did not move is listed"
labelled "$S/known.html" documentation "$(jq -r '.groups[2].items[] | select(.subject == "write the first guide") | .commit' "$S/known.json")" v0.2.0
labelled "$S/known.html" documentation "$(jq -r '.groups[2].items[] | select(.subject == "describe the second deployment") | .commit' "$S/known.json")" unreleased
[ "$(jq '.groups[2].items | length' "$S/known.json")" = 2 ] || fail "the documentation group is not the two docs commits"
[ "$(jq '.other_changes' "$S/known.json")" = 1 ] || fail "the record commit is not counted as the one other change"

# from C2, after the record: nothing is released any more, and the capability did not move
expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/after.json" --previous-commit "$C2" || exit 1
render "$S/after.json" > "$S/after.html" || exit 1
! grep -qF 'data-movement-group="capabilities"' "$S/after.html" || fail "a capability that did not move after C2 is listed"
! grep -qF 'data-release-label="v0.2.0"' "$S/after.html" || fail "a movement after the record is labelled released"
labelled "$S/after.html" claims promoted unreleased

# ---------------------------------------------------------------- 2. unknown, and nothing listed
# unknown_page <record or ""> <reason pattern>
unknown_page() {
  render "$1" > "$S/unknown.html" || exit 1
  if [ -n "$1" ]; then
    grep -qF 'data-since-deploy="unknown"' "$S/unknown.html" || fail "the page does not say the delta is unknown"
    grep -qE "$2" "$S/unknown.html" || fail "the page does not say why: $(jq -r .reason "$1")"
  else
    grep -qF 'data-since-deploy="absent"' "$S/unknown.html" || fail "a build without a record does not say so"
  fi
  ! grep -qF 'data-movement=' "$S/unknown.html" || fail "an unknown delta lists movements"
}
expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/none.json" || exit 1
[ "$(jq -r .state "$S/none.json")" = unknown ] || fail "no previous commit is not unknown"
unknown_page "$S/none.json" 'no previously deployed commit'
side="$(git commit-tree "$C0^{tree}" -p "$C0" -m "a deployment from a branch that never merged")"
expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/side.json" --previous-commit "$side" || exit 1
[ "$(jq -r .state "$S/side.json")" = unknown ] || fail "a previous commit that is not an ancestor is not unknown"
unknown_page "$S/side.json" 'not an ancestor'
expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/gone.json" --previous-commit 0123456789abcdef0123456789abcdef01234567 || exit 1
unknown_page "$S/gone.json" 'not in this clone'
unknown_page "" ''
# an executable that does not describe the built commit cannot measure it
jq --arg a "$gained" '.capabilities |= map(select(.id != $a))' "$S/exact.json" > docs/generated/registry.json
git add -A >/dev/null && git commit -qm "chore(fixture): a registry the executable does not describe"
expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/foreign.json" --previous-commit "$C0" || exit 1
unknown_page "$S/foreign.json" 'not the built commit'
git reset -q --hard "$C3"

# ---------------------------------------------------------------- 3. the deploy branch names the previous commit
deploy() { git commit-tree "$(git mktree </dev/null)" ${2:+-p "$2"} -m "deploy: site from ${1:0:7}" -m "source: $1"; }
d1="$(deploy "$C0")"; d2="$(deploy "$C3" "$d1")"
git update-ref refs/remotes/origin/gh-pages "$d2"
expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/branch.json" --previous-deploy origin/gh-pages || exit 1
[ "$(jq -r .previous "$S/branch.json")" = "$C0" ] \
  || fail "the previous deployment is not the newest source other than the built commit: $(jq -r .previous "$S/branch.json")"
MJ_SITE_PREVIOUS_DEPLOY=origin/gh-pages expect_exit 0 "$BUILD" --since-deploy "$PWD" "$S/env.json" || exit 1
[ "$(jq -r .previous "$S/env.json")" = "$C0" ] || fail "MJ_SITE_PREVIOUS_DEPLOY is not read"

# ---------------------------------------------------------------- 4. never committed
expect_exit 0 "$BUILD" --since-deploy "$PWD" site/data/since-deploy.json --previous-commit "$C0" || exit 1
cp site/data/since-deploy.json "$S/first.json"
[ -z "$(git status --porcelain)" ] || fail "a build changed the tree: $(git status --porcelain)"
expect_exit 0 "$BUILD" --since-deploy "$PWD" site/data/since-deploy.json --previous-commit "$C2" || exit 1
cmp -s site/data/since-deploy.json "$S/first.json" && fail "two previous commits composed the same record"
[ -z "$(git status --porcelain)" ] || fail "a second build changed the tree: $(git status --porcelain)"
expect_exit 0 "$ROOT/scripts/site-check" --since-deploy "$PWD" || exit 1
expect_grep 'OK   since-deploy'
git add -f site/data/since-deploy.json
expect_exit 10 "$ROOT/scripts/site-check" --since-deploy "$PWD" || exit 1
expect_grep 'FAIL since-deploy .*is tracked'
git rm -q --cached site/data/since-deploy.json
git -C "$ROOT" check-ignore -q site/data/since-deploy.json || fail "the repository does not ignore the record"
! git -C "$ROOT" ls-files --error-unmatch site/data/since-deploy.json >/dev/null 2>&1 || fail "the repository tracks the record"

# ---------------------------------------------------------------- 5. wired where a deployment is built
grep -qE '^  since_deploy "\$ROOT" "\$PREVIOUS" "\$SITE/data/since-deploy\.json"' "$BUILD" \
  || fail "the full build does not compose the record beside build.json"
grep -qF 'cp "$SINCE_RECORD" "$SITE/data/since-deploy.json"' "$BUILD" || fail "the full build cannot adopt a composed record"
grep -qF 'scripts/site-build --previous-deploy "$REMOTE/$BRANCH"' "$ROOT/scripts/site-deploy" \
  || fail "scripts/site-deploy does not name the deployment it replaces"
grep -qF 'MJ_SITE_SINCE_DEPLOY_RECORD: ${{ steps.since.outputs.record }}' "$ROOT/.github/workflows/pages.yml" \
  || fail "the Pages workflow does not hand the build the record it composed"
grep -qF '{% include "partials/since-deploy.html" %}' "$ROOT/site/templates/changelog.html" \
  || fail "/changelog/ does not render the section"
grep -qE 'href="\{\{ get_url\(path="/changelog/", trailing_slash=true\) \}\}#since-deploy"[^>]*data-since-deploy-link' \
  "$ROOT/site/templates/index.html" || fail "the homepage's trust strip does not link the section"
exit 0
