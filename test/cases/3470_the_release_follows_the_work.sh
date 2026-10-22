# majordomus-covers: none
# Whether the trunk's landed work is released (release.currency, I3070), and that the brief and
# doctor say so. This is clause 9 of project.land-and-publish and the owner's order of
# 2026-10-19: a measured version after every session and milestone, enforced.
#
# A disposable repository with a bare origin, a recorded release and a real history:
#
#   only a release commit past the release   current: a release commit is no landed work
#   a fix landed an hour ago                 owed, with the count and the wait, in the brief too,
#                                            and the same answer to a tools/call over MCP
#   a fix landed seven hours ago             overdue, and doctor's warning says so
#   a top made 7 h ago that landed 1 h ago   owed for an hour: the wait runs from when this
#                                            clone saw it land (its reflog), not from its commits
#   the same, its reflog gone                owed, the commit time bounding the wait from above,
#                                            and a bound alone is never overdue, in doctor either
#   a tag pushed past the record             current: releasing is never reported as owed
#   a fix past an unrecorded tag             the version is not decided, and the answer says why
#   a clone with no remote                   unknown, and doctor says what it cannot measure
#
# A landing is dated by the reflog of the remote-tracking trunk, so each push here records its
# time there (GIT_COMMITTER_DATE), as a fetch of a real landing would.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3470.XXXXXX")"
trap 'rm -rf "$S"' EXIT

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

# record VERSION TAG COMMIT
record() {
  cat > ".ai/repo/releases/$2.yaml" <<Y
schema: release/v1
version: "$1"
tag: $2
channel: stable
commit: $3
published_at: "2026-10-01T00:00:00Z"
artifacts:
  - target: macos-aarch64
    name: fixture-$2-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/$2/fixture-$2.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
}

# land SUBJECT MADE-AGO [LANDED-AGO]: a commit made on the trunk MADE-AGO seconds ago, pushed to
# the origin LANDED-AGO seconds ago (by default when it was made)
land() {
  at="$(( $(date +%s) - $2 ))"
  GIT_AUTHOR_DATE="@$at +0000" GIT_COMMITTER_DATE="@$at +0000" git commit -q --allow-empty -m "$1"
  push "$(( $(date +%s) - ${3:-$2} ))"
}
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
record 0.1.0 v0.1.0 "$released"
git add -A >/dev/null && git commit -qm "chore(release): record v0.1.0"
push
recorded="$(git rev-parse HEAD)"

currency() { # [input]
  "$RB" run release.currency --input "${1:-"{\"next\":false}"}" --quiet --format json 2>/dev/null | jq -c '.output'
}
field() { printf '%s' "$1" | jq -r "$2"; }
# The same question over MCP, as a client asks it: one session against this fixture, alone,
# binding no port and writing no lease, and the typed answer of the tools/call or nothing.
mcp_currency() { # arguments -> the structured content majordomus_release_currency answers
  { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3470","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_release_currency","arguments":%s}}\n' "$1"
  } | "$RB" mcp --standalone --repo "$PWD" 2>"$S/mcp.err" | sed -n 2p | jq -c '.result.structuredContent // empty'
}

# ---------------------------------------------------------------- 1. only a release commit past it
c="$(currency)"
[ "$(field "$c" .stance)" = current ] || { echo "    a release commit past the release is not current: $c"; exit 1; }
field "$c" .summary | grep -qF 'origin/master holds no landed work past v0.1.0' \
  || { echo "    the current answer does not say so: $c"; exit 1; }
echo "    a release commit past the release is no landed work: current"

# ---------------------------------------------------------------- 2. a fix landed an hour ago
land "fix: the second" 3600
c="$(currency)"
[ "$(field "$c" .stance)" = owed ] || { echo "    a fix landed an hour ago is not owed: $c"; exit 1; }
[ "$(field "$c" .landed)" = 1 ] || { echo "    not one landed commit: $c"; exit 1; }
w="$(field "$c" .waited_seconds)"
[ "$w" -ge 3500 ] && [ "$w" -lt 4000 ] || { echo "    the wait is not an hour: $c"; exit 1; }
b="$("$RB" brief --format json 2>/dev/null)"
[ "$(field "$b" .release.stance)" = owed ] || { echo "    the brief's release section is not owed: $(field "$b" .release)"; exit 1; }
field "$b" '.lines[]' | grep -q '^release      1 landed commit(s) in 1 landing(s) wait past v0.1.0 on origin/master' \
  || { echo "    the brief has no release line:"; field "$b" '.lines[]' | sed 's/^/    | /'; exit 1; }
field "$b" .line | grep -qF 'release owed 1' || { echo "    the brief's one line does not name it: $(field "$b" .line)"; exit 1; }
echo "    a fix landed an hour ago is owed, with its count and its wait, in the brief too"
m="$(mcp_currency '{"next":false}')"
[ -n "$m" ] || { echo "    majordomus_release_currency gave no typed answer over MCP:"; sed 's/^/    | /' "$S/mcp.err" | head -5; exit 1; }
[ "$(field "$m" .stance)" = owed ] && [ "$(field "$m" .landed)" = 1 ] \
  && [ "$(field "$m" .release.reference)" = "$(field "$c" .release.reference)" ] \
  && [ "$(field "$m" .waited_seconds)" -ge 3500 ] && [ "$(field "$m" .waited_seconds)" -lt 4000 ] \
  || { echo "    over MCP the tool does not answer what the command does: $m (the command: $c)"; exit 1; }
echo "    asked over MCP, majordomus_release_currency gives the command's answer: owed, one landed commit"

# ---------------------------------------------------------------- 3. seven hours ago: overdue
git reset -q --hard "$recorded"
land "fix: the second, long ago" 25200
c="$(currency)"
[ "$(field "$c" .stance)" = overdue ] || { echo "    a fix landed seven hours ago is not overdue: $c"; exit 1; }
"$MJ" doctor > "$S/doctor.out" 2>&1 || true
grep -E 'WARN.*release.*OVERDUE: 1 landed commit' "$S/doctor.out" >/dev/null \
  || { echo "    doctor does not warn that the release is overdue:"; grep -i release "$S/doctor.out" | sed 's/^/    | /'; exit 1; }
echo "    a fix landed seven hours ago is overdue, and doctor warns"

# ---------------------------------------------------------------- 3b. a top composed before it landed
git reset -q --hard "$recorded"
land "fix: a top composed long before it landed" 25200 3600
c="$(currency)"
[ "$(field "$c" .stance)" = owed ] && [ "$(field "$c" .landed_at)" = reflog ] \
  || { echo "    a top made 7 h ago that landed 1 h ago is not owed by its reflog: $c"; exit 1; }
w="$(field "$c" .waited_seconds)"
[ "$w" -ge 3500 ] && [ "$w" -lt 4000 ] || { echo "    the wait does not run from the landing: $c"; exit 1; }
echo "    a top made seven hours ago that landed an hour ago waits for an hour, as the reflog saw it land"

# ---------------------------------------------------------------- 3c. no reflog: a bound, never overdue
rm -f .git/logs/refs/remotes/origin/master
c="$(currency)"
[ "$(field "$c" .stance)" = owed ] && [ "$(field "$c" .landed_at)" = commit ] \
  || { echo "    with no reflog the commit time did not bound the wait as owed: $c"; exit 1; }
field "$c" .summary | grep -qF 'which alone makes nothing overdue' \
  || { echo "    the answer does not say the bound is only a bound: $c"; exit 1; }
"$MJ" doctor > "$S/doctor.out" 2>&1 || true
if grep -E 'release.*OVERDUE' "$S/doctor.out" >/dev/null; then
  echo "    doctor calls a commit-time bound overdue:"; grep -i release "$S/doctor.out" | sed 's/^/    | /'; exit 1
fi
grep -E 'WARN.*release.*at most' "$S/doctor.out" >/dev/null \
  || { echo "    doctor does not warn with the bound:"; grep -i release "$S/doctor.out" | sed 's/^/    | /'; exit 1; }
echo "    without a reflog the commit time bounds the wait, and the bound is owed, never overdue"

# ---------------------------------------------------------------- 4. a tag pushed past the record
git tag v0.1.1
push
c="$(currency)"
[ "$(field "$c" .stance)" = current ] || { echo "    a pushed tag did not make the trunk current: $c"; exit 1; }
[ "$(field "$c" .release.reference)" = v0.1.1 ] && [ "$(field "$c" .release.recorded)" = false ] \
  || { echo "    the release is not the unrecorded tag: $c"; exit 1; }
echo "    a tag pushed before its record lands is the release: current"

# ---------------------------------------------------------------- 5. the version, where it cannot be decided
land "fix: after the tag" 60
c="$(currency '{}')"
field "$c" .next_unmeasured | grep -qF 'v0.1.1 has no release record yet' \
  || { echo "    the version was not refused for an unrecorded release: $c"; exit 1; }
[ "$(field "$c" '.next // "none"')" = none ] || { echo "    a version was decided from an unrecorded release: $c"; exit 1; }
echo "    the version is not decided from an unrecorded release, and the answer says why"

# ---------------------------------------------------------------- 6. no remote
git remote remove origin
c="$(currency)"
[ "$(field "$c" .stance)" = unknown ] || { echo "    a clone with no remote is not unknown: $c"; exit 1; }
"$MJ" doctor > "$S/doctor.out" 2>&1 || true
grep -E 'release.*no remote-tracking branch' "$S/doctor.out" >/dev/null \
  || { echo "    doctor does not say what it cannot measure:"; grep -i release "$S/doctor.out" | sed 's/^/    | /'; exit 1; }
echo "    a clone with no remote is unknown, and doctor says what it cannot measure"
echo "    release.currency, the brief and doctor say whether the trunk's landed work is released"
