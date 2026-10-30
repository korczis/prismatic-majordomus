# majordomus-covers: none
# The UI roadmap reads delivery and the releases themselves, and its now is where work is
# (I2744, ADR 0166 decision 4), through a real server over a fixture plan of four milestones:
#
#   now        the milestones with work in flight (an open pull request, an ACTIVE issue),
#              never the first open one in the plan's order: M1 comes first and holds no
#              work, the "Now M002" the 07:15Z snapshot of 2026-10-16 showed
#   next       the first open milestone the graph allows once that work lands
#   delivery   an issue's declared status beside its inferred stage, linked to the pull
#              request, the merge commit and the release tag that prove it
#   release    the highest version the releases name: the executable's own version is
#              tagged and its record has not merged, and the screen still shows it, never
#              the older record (the "Latest release 0.26.0 under a 0.27.0 header")
#   unread     a source that was not read (the publication, the mesh) is said
#
# Every assertion names the mutation that breaks it. Each marked RUN was applied to
# apps/majordomus-cli/src/plan/flight.rs or apps/majordomus-cli/src/ui/roadmap.rs on
# 2026-10-16, one at a time, and failed this case where it says.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST
fail() { printf '    %s\n' "$*"; exit 1; }

VER="$("$RB" --version | awk '{print $NF}')"
[ -n "$VER" ] || fail "the executable names no version"

"$MJ" init >/dev/null
pj_init
pj_milestone M1 1
pj_milestone M2 2
pj_milestone M3 3
printf 'depends_on:\n  - M2\n' >> .ai/repo/project/milestones/M3.yaml
pj_milestone M4 4
pj_issue I0001 M1
pj_issue I0002 M2
pj_issue I0003 M3
pj_issue I0004 M4
git add -A >/dev/null && git commit -qm "a plan of four milestones"
TRUNK="$(git rev-parse --abbrev-ref HEAD)"

# I0001 is merged by pull request #5 and released in v0.1.0: delivered, and not in flight
git checkout -q -b feature/the-work
printf 'echo work\n' > work.sh; git add work.sh; git commit -qm "feat: the work (I0001)"
git checkout -q "$TRUNK"
git merge -q --no-ff feature/the-work -m "Merge pull request #5 from someone/feature/the-work" -m "feat: the work (I0001)"
MERGE="$(git rev-parse HEAD)"
git branch -D -q feature/the-work
mkdir -p .ai/repo/releases
cat > .ai/repo/releases/v0.1.0.yaml <<Y
schema: release/v1
version: "0.1.0"
tag: v0.1.0
channel: stable
commit: $MERGE
published_at: "2026-10-16T07:00:00Z"
artifacts:
  - target: linux-x86_64-gnu
    name: m-v0.1.0.tar.gz
    url: https://example.invalid/m-v0.1.0.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000000"
    size: 1
Y
git add -A >/dev/null; git commit -qm "release: record v0.1.0"
# the executable's own version is tagged; its record has not merged yet
git tag "v$VER"

# I0002 has an open pull request, in the forge observation `majordomus prs refresh` records
mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<'J'
{"schema":4,"repository":"example/fixture","base":"master","base_sha":"0000000000000000000000000000000000000000",
 "observed_at":"2026-10-16T08:00:00Z","required_checks":null,"review_policy":null,"merge_methods":["merge"],
 "pull_requests":[{"number":9,"title":"feat: the busy piece (I0002)","author":"someone","head_ref":"feature/busy",
  "head_sha":"1111111111111111111111111111111111111111","base_ref":"master","draft":false,
  "created_at":"2026-10-16T07:00:00Z","updated_at":"2026-10-16T07:30:00Z"}]}
J

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 90 "$U$1"; }

# ---------------------------------------------------------------- the answer
get /api/v1/plan/flight > f.json
jq -e . f.json >/dev/null 2>&1 || { head -c 400 f.json; fail "plan.flight answered no JSON"; }
# MUTATION, RUN: keep every open issue in flight (drop the `in_flight` filter in derive) ->
# now is M1, M2, M3.
[ "$(jq -c .now f.json)" = '["M2"]' ] || fail "now is not the milestone with work in flight: $(jq -c .now f.json)"
[ "$(jq -r .next f.json)" = M1 ] || fail "next is not the first milestone the graph allows: $(jq -r .next f.json)"
[ "$(jq -r '.milestones[0].issues[0] | "\(.issue) \(.stage) \(.in_flight | join(","))"' f.json)" = "I0002 pr_open pr_open" ] \
  || fail "M2's issue in flight is not I0002 by its open pull request: $(jq -c '.milestones[0]' f.json)"
# MUTATION, RUN: take the first read source rather than the highest version (latest_of keeps
# its first candidate) -> 0.1.0 from records.
[ "$(jq -r '.release | "\(.latest) \(.source) \(.executable_tagged)"' f.json)" = "$VER tags true" ] \
  || fail "the latest release is not the executable's tagged version: $(jq -c '.release' f.json)"
[ "$(jq -r '.release.readings[] | select(.source == "records") | .version' f.json)" = 0.1.0 ] \
  || fail "the records' own answer is not reported beside it"
[ "$(jq -r '.sources[] | select(.name == "mesh") | .state' f.json)" = unread ] \
  || fail "the mesh, which runs in no server here, is not reported unread"
[ "$(jq -r '.sources[] | select(.name == "publication") | .state' f.json)" = unread ] \
  || fail "the publication, never observed here, is not reported unread"

# ---------------------------------------------------------------- the screen
page="$(get /ui/roadmap)"
printf '%s' "$page" | grep -qF 'Now M2 · next M1' || fail "the screen does not say now M2, next M1"
printf '%s' "$page" | grep -qF 'Now M1' && fail "the screen names M1, which holds no work, as now"
printf '%s' "$page" | grep -qF '/cockpit/capabilities/plan.flight' || fail "the screen does not cite plan.flight"
# MUTATION, RUN: build no forge link for a pull request (href None in proofs) -> no link.
printf '%s' "$page" | grep -qF 'href="https://github.com/example/fixture/pull/9"' \
  || fail "I0002's open pull request is not linked"
# MUTATION, RUN: show the declared status in the Inferred column (`value(i, "status")` in
# issue_table) -> no pr_open stage on the screen.
printf '%s' "$page" | grep -qF '<span class="mj-tag">pr_open</span>' || fail "I0002's inferred stage is not shown"
printf '%s' "$page" | grep -qF "Latest release $VER (from tags)" || fail "the screen shows an older release than its own executable"
printf '%s' "$page" | grep -qF "href=\"https://github.com/example/fixture/releases/tag/v$VER\"" \
  || fail "the latest release is not linked to its tag"
printf '%s' "$page" | grep -qF '<code class="mj-mono">mesh</code>' || fail "the unread mesh is not said on the screen"

# ---------------------------------------------------------------- an inspected milestone
page="$(get '/ui/roadmap?milestone=M1')"
printf '%s' "$page" | grep -qF 'Delivery of M1' || fail "M1's delivery is not shown"
printf '%s' "$page" | grep -qF '<span class="mj-tag">released</span>' || fail "I0001's inferred stage is not released"
for want in \
  'href="https://github.com/example/fixture/pull/5"' \
  "href=\"https://github.com/example/fixture/commit/$MERGE\"" \
  'href="https://github.com/example/fixture/releases/tag/v0.1.0"'; do
  printf '%s' "$page" | grep -qF "$want" || fail "I0001's proof has no link $want"
done
get '/ui/roadmap?milestone=M9' | grep -qF 'M9 is not a milestone of this plan.' \
  || fail "an id the plan does not hold is not said to be unknown"

# ---------------------------------------------------------------- an ACTIVE issue is work in flight
status="$(curl -s -o t.json -w '%{http_code}' --max-time 60 -X POST -H 'content-type: application/json' \
  -d '{"issue":"I0004","transition":"start"}' "$U/api/v1/plan/transition")"
[ "$status" = 200 ] || { cat t.json; fail "plan.transition start I0004 answered $status"; }
get /api/v1/plan/flight > f.json
[ "$(jq -c .now f.json)" = '["M2","M4"]' ] || fail "an ACTIVE issue did not put M4 in flight: $(jq -c .now f.json)"
[ "$(jq -r '.milestones[1].issues[0].in_flight | join(",")' f.json)" = active ] || fail "I0004 is not in flight by its ACTIVE status"

# ---------------------------------------------------------------- the dashboard no longer says "Now"
page="$(get /ui)"
printf '%s' "$page" | grep -qF 'Now M1' && fail "the dashboard still names the plan's first open milestone as now"
printf '%s' "$page" | grep -qF 'First open in the plan&#39;s order M1' || fail "the dashboard's plan line is not what it reads"

echo "    now is where work is, next is the graph's, each issue shows its stage with its proof, and the release is the releases'"
