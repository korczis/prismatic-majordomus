# majordomus-covers: none
# /ui/delivery shows what release.currency answers (I3071): whether the work landed since the
# last release is released, how much of it waits, since when and how that was dated, and the
# release it is measured from, and the panel's source line leads to the capability.
#
# Case 3470 holds the answer itself. This case holds the screen to it. It uses a disposable
# repository with a bare origin, a recorded release and a fix landed an hour ago, served by
# this checkout's executable:
#
#   panel     "Is the trunk released?" shows the answer's stance, its sentence, its count of
#             landed commits and landings, its wait dated by the reflog, and the release
#   kpi       the hero's "Since then" tile is the same stance
#   source    the panel's source line names release.currency and links its API route and its
#             technical view in the Cockpit
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3471.XXXXXX")"
PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done; rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# ---------------------------------------------------------------- a repository that publishes
awk '/^  deployments: repo\/deployments$/{print; print "  releases: repo/releases"; next} {print}' \
  .ai/manifest.yaml > "$S/manifest.yaml" && mv "$S/manifest.yaml" .ai/manifest.yaml
grep -q '^  releases: repo/releases$' .ai/manifest.yaml || {
  echo "    the manifest did not gain a releases section; the skeleton's shape changed"; exit 1; }
cat >> .ai/repo/knowledge/sources.yaml <<'Y'

  - id: release
    kind: release-record
    discovery: vcs
    pathspec: ':(glob).ai/repo/releases/*.yaml'
    required: false
Y
mkdir -p .ai/repo/releases

# push [AT]: push to the origin and fetch it back, the trunk's reflog recording AT (default now)
push() {
  when="@${1:-$(date +%s)} +0000"
  GIT_COMMITTER_DATE="$when" git push -q -f origin HEAD:master --tags 2>/dev/null \
    || { echo "    the push to the fixture's origin failed"; exit 1; }
  GIT_COMMITTER_DATE="$when" git fetch -q origin
}

git add -A >/dev/null && git commit -qm "publishes"
git init -q --bare --initial-branch=master "$S/origin.git"
git remote add origin "$S/origin.git"
push
git remote set-head origin master >/dev/null 2>&1 || true
released="$(git rev-parse HEAD)"
git tag v0.1.0 "$released"
cat > .ai/repo/releases/v0.1.0.yaml <<Y
schema: release/v1
version: "0.1.0"
tag: v0.1.0
channel: stable
commit: $released
published_at: "2026-10-01T00:00:00Z"
artifacts:
  - target: macos-aarch64
    name: fixture-v0.1.0-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/v0.1.0/fixture-v0.1.0.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
git add -A >/dev/null && git commit -qm "chore(release): record v0.1.0"
push
# a fix made and landed an hour ago
at="$(( $(date +%s) - 3600 ))"
GIT_AUTHOR_DATE="@$at +0000" GIT_COMMITTER_DATE="@$at +0000" git commit -q --allow-empty -m "fix: the second"
push "$at"

answer="$("$RB" run release.currency --input '{"next":false}' --quiet --format json 2>/dev/null | jq -c '.output')"
[ "$(printf '%s' "$answer" | jq -r .stance)" = owed ] || { echo "    the fixture is not owed: $answer"; exit 1; }

# ---------------------------------------------------------------- the screen, served
mkdir -p "$S/state"
( XDG_STATE_HOME="$S/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$S/serve.log"; exit 1; }; sleep 0.2; done
url="$(jq -r '.url' .ai/local/state/mcp/server.json)"
curl -fsS "$url/ui/delivery" > "$S/delivery.html" || { echo "    /ui/delivery did not answer"; exit 1; }
page() { grep -qF -- "$1" "$S/delivery.html"; }

# ---------------------------------------------------------------- 1. the panel is the answer
page 'Is the trunk released?' || { echo "    /ui/delivery has no release panel"; exit 1; }
summary="$(printf '%s' "$answer" | jq -r .summary)"
page "$summary" || { echo "    the panel does not carry the answer's sentence: $summary"; exit 1; }
counts="$(printf '%s' "$answer" | jq -r '"\(.landed) commit(s) in \(.landings) landing(s)"')"
page "$counts" || { echo "    the panel does not show the answer's counts: $counts"; exit 1; }
[ "$(printf '%s' "$answer" | jq -r .landed_at)" = reflog ] || { echo "    the fixture's landing is not dated by the reflog: $answer"; exit 1; }
page 'since this clone saw it land' || { echo "    the panel does not say how the wait was dated"; exit 1; }
page 'v0.1.0' || { echo "    the panel does not name the release it measures from"; exit 1; }
echo "    the panel shows the answer: owed, $counts, dated by the reflog, from v0.1.0"

# ---------------------------------------------------------------- 2. the KPI is the same stance
grep -E 'Since then' "$S/delivery.html" >/dev/null || { echo "    the hero has no Since then tile"; exit 1; }
grep -E '>owed<' "$S/delivery.html" >/dev/null || { echo "    the stance is not drawn as owed"; exit 1; }
echo "    the hero's Since then tile says owed"

# ---------------------------------------------------------------- 3. the source leads to the capability
page 'release.currency' || { echo "    the panel's source line does not name release.currency"; exit 1; }
page 'href="/api/v1/release/currency' || { echo "    the source line does not link the capability's API route"; exit 1; }
page 'href="/cockpit/capabilities/release.currency"' || { echo "    the source line does not link the technical view"; exit 1; }
echo "    the source line leads to release.currency's API route and its technical view"
echo "    /ui/delivery shows what release.currency answers"
