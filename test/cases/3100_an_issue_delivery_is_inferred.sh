# majordomus-covers: plan
# An issue's delivery is inferred from branches, pull requests, merges, tags and the
# published release pointer (I2740, ADR 0166).
#
# `majordomus plan delivery` answers, for every issue, the furthest of none, branch, pr_open,
# merged, released and deployed, each with the object that proved it. This drives the shell
# launcher (which hands `plan delivery` to the executable) over a fixture repository with one
# issue merged by a pull request, one on a branch named by its slug, one named only by an
# open pull request in the recorded forge observation, and one named by nothing. Then it
# takes each fact away again and the stage it proved must fall with it: an assertion that
# still held with its fact removed would be proving nothing.
#
# Every assertion names the mutation of the inference that breaks it. Each marked RUN was
# applied to apps/majordomus-cli/src/plan/delivery.rs on 2026-10-16, one at a time, and
# each failed this case where it says.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
fail() { printf '    %s\n' "$*"; exit 1; }

"$MJ" init >/dev/null
pj_init; pj_milestone M1
pj_issue I0001 M1; pj_issue I0002 M1; pj_issue I0003 M1; pj_issue I0004 M1
# I0002 is named by its slug, the way `majordomus worktree create` names work here
sed -i.bak 's/^slug: issue-I0002$/slug: a-slugged-piece/' .ai/repo/project/issues/I0002.yaml
rm -f .ai/repo/project/issues/I0002.yaml.bak
git add -A >/dev/null && git commit -qm "plan: four issues"
TRUNK="$(git rev-parse --abbrev-ref HEAD)"

git checkout -q -b feature/the-work
printf 'echo work\n' > work.sh; git add work.sh; git commit -qm "feat: the work (I0001)"
git checkout -q "$TRUNK"
git merge -q --no-ff feature/the-work -m "Merge pull request #5 from someone/feature/the-work" -m "feat: the work (I0001)"
MERGE="$(git rev-parse HEAD)"
git branch -D -q feature/the-work
git branch fix/a-slugged-piece
git checkout -q fix/a-slugged-piece
printf 'echo slug\n' > slug.sh; git add slug.sh; git commit -qm "fix: a slugged piece"
git checkout -q "$TRUNK"

d() { "$MJ" --json plan delivery "$@" > d.json 2> d.err || { cat d.err; fail "plan delivery $* exited non-zero"; }; }
stage() { jq -r --arg i "$1" '.issues[] | select(.issue == $i) | .stage' d.json; }
field() { jq -r --arg i "$1" ".issues[] | select(.issue == \$i) | $2" d.json; }

# ---------------------------------------------------------------- live, the forge never observed
d
# MUTATION, RUN: drop the pull-request title from the merge (`title = None` for every merge in
# parse_merges) -> I0001 read none before commit subjects were read; now the proof falls to
# the branch's own commit and the merge-commit assertion below fails.
[ "$(stage I0001)" = merged ] || fail "I0001 is not merged: $(field I0001 .)"
[ "$(field I0001 .merged.commit)" = "$MERGE" ] || fail "I0001's merge is not the merge commit"
[ "$(field I0001 .merged.pull_request)" = 5 ] || fail "I0001's merge does not name pull request #5"
# MUTATION, RUN: drop the slug half of branch_names -> I0002 reads none.
[ "$(stage I0002)" = branch ] || fail "I0002 is not on its slugged branch: $(field I0002 .)"
[ "$(field I0002 .branch.name)" = fix/a-slugged-piece ] || fail "I0002's branch is not named"
# MUTATION, RUN: never name the forge as unread (`if false && e.open.is_none() ...` in assess)
# -> I0003 is none and settled, which is the lie this forbids.
[ "$(stage I0003)" = none ] || fail "I0003 has a stage with no forge read"
[ "$(field I0003 .settled)" = false ] || fail "an unread forge left I0003 settled"
[ "$(field I0003 '.unread | join(",")')" = forge ] || fail "I0003 does not say the forge is unread"
jq -e '.sources[] | select(.name == "forge") | .state == "unread"' d.json >/dev/null \
  || fail "the forge source is not reported unread"
"$MJ" plan delivery I0003 > d.txt
grep -q 'I0003 .* none .*(unread: forge)' d.txt || { cat d.txt; fail "the text calls an unread forge none"; }

# ---------------------------------------------------------------- the forge observed
mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<'J'
{"schema":4,"repository":"someone/fixture","base":"master","base_sha":"0000000000000000000000000000000000000000",
 "observed_at":"2026-10-16T08:00:00Z","required_checks":null,"review_policy":null,"merge_methods":["merge"],
 "pull_requests":[{"number":9,"title":"feat: an open piece (I0003)","author":"someone","head_ref":"feature/x",
  "head_sha":"1111111111111111111111111111111111111111","base_ref":"master","draft":false,
  "created_at":"2026-10-16T07:00:00Z","updated_at":"2026-10-16T07:30:00Z"}]}
J
d
# MUTATION, RUN: read no open pull request (`open_pulls` answers an empty map) -> I0003 none.
[ "$(stage I0003)" = pr_open ] || fail "I0003 is not pr_open: $(field I0003 .)"
[ "$(field I0003 .pr_open.number)" = 9 ] || fail "I0003's pull request number is not 9"
[ "$(field I0004 .stage):$(field I0004 .settled)" = none:true ] || fail "I0004 is not none and settled"

# ---------------------------------------------------------------- a subject names it, a body does not
# A pull request merged inside a combined top reaches the trunk through an octopus merge that
# names it only in prose; its commits carry the issue in their subjects.
printf 'echo direct\n' > direct.sh; git add direct.sh; git commit -qm "fix: the untouched piece (I0004)"
DIRECT="$(git rev-parse HEAD)"
printf 'echo body\n' > body.sh; git add body.sh; git commit -qm "chore: unrelated" -m "I0002 builds on this."
d
[ "$(stage I0004):$(field I0004 .merged.via)" = merged:subject ] || fail "a subject naming I0004 did not merge it: $(field I0004 .)"
[ "$(field I0004 .merged.commit)" = "$DIRECT" ] || fail "I0004's proof is not the commit whose subject names it"
[ "$(stage I0002)" = branch ] || fail "a commit body named I0002: $(field I0002 .)"

# ---------------------------------------------------------------- release records, not tags
# A tag is a ref that can appear on any day; a release record names the commit it released
# and is a file of the tree. Releases are read from the records.
git tag v0.1.0 "$MERGE"
d
[ "$(stage I0001)" = merged ] || fail "a tag with no release record released I0001"
record() { # version commit
  cat > ".ai/repo/releases/v$1.yaml" <<Y
schema: release/v1
version: "$1"
tag: v$1
channel: stable
commit: $2
published_at: "2026-10-16T07:00:00Z"
artifacts:
  - target: linux-x86_64-gnu
    name: m-v$1.tar.gz
    url: https://example.invalid/m-v$1.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000000"
    size: 1
Y
}
mkdir -p .ai/repo/releases site/static/releases
record 0.1.0 "$MERGE"
printf 'echo later\n' > later.sh; git add -A >/dev/null; git commit -qm "release: record v0.1.0"
record 0.2.0 "$(git rev-parse HEAD)"
git add -A >/dev/null; git commit -qm "release: record v0.2.0"
d
# MUTATION: take the last release containing the merge instead of the first (`.and_modify`
# overwriting the entry in first_releases) -> v0.2.0, not v0.1.0.
[ "$(stage I0001)" = released ] || fail "I0001 is not released: $(field I0001 .)"
[ "$(field I0001 .released.tag)" = v0.1.0 ] || fail "I0001's release is not the first record containing it"
[ "$(field I0001 '.unread | join(",")')" = publication ] || fail "no deployment was observed and I0001 does not say so"

# ---------------------------------------------------------------- deployed: live, from the Pages deployment observed
printf '{"schema":"release/v1","version":"0.2.0","tag":"v0.2.0"}\n' > site/static/releases/latest.json
git add -A >/dev/null; git commit -qm "release: publish the pointer"
DEPLOYED="$(git rev-parse HEAD)"
observe_deployment() { # sha state
  printf '{"schema":1,"repository":"someone/fixture","repository_url":"https://example.invalid/f","observed_at":"2026-10-16T09:00:00Z","deployments":[{"id":"1","environment":"github-pages","sha":"%s","created_at":"2026-10-16T08:30:00Z","state":"%s"}],"releases":[]}\n' \
    "$1" "$2" > .ai/local/state/integration/deployments.json
}
observe_deployment "$DEPLOYED" success
d
# MUTATION: compare the published version the wrong way (`<=` -> `>`) -> I0001 released.
[ "$(stage I0001)" = deployed ] || fail "I0001 is not deployed: $(field I0001 .)"
[ "$(field I0001 .deployed.version)" = 0.2.0 ] || fail "I0001's deployment does not name 0.2.0"
[ "$(field I0001 .deployed.latest_json)" = "$DEPLOYED:site/static/releases/latest.json" ] \
  || fail "the deployment does not name the latest.json it read"
# the fact taken away again: a failed deployment publishes nothing
observe_deployment "$DEPLOYED" failure
d
[ "$(stage I0001)" = released ] || fail "a failed deployment deployed I0001"
observe_deployment "$DEPLOYED" success

# ---------------------------------------------------------------- at a commit: the history, nothing live
HEAD_SHA="$(git rev-parse HEAD)"
d --at "$HEAD_SHA"
[ "$(stage I0001)" = released ] || fail "an answer at a commit read the live publication: $(field I0001 .)"
[ "$(jq -r .observed.commit d.json)" = "$HEAD_SHA" ] || fail "the answer does not name the commit it observed"
# at a commit no branch and no forge is read: the slugged branch and the open pull request are
# unread, never none
for i in I0002 I0003; do
  [ "$(stage "$i"):$(field "$i" '.unread | join(",")')" = none:branches,forge ] \
    || fail "$i does not say branches and the forge were not read: $(field "$i" .)"
done
cp d.json at1.json
git tag v9.9.9
d --at "$HEAD_SHA"
cmp -s at1.json d.json || fail "an answer at one commit moved when a tag appeared"

# ---------------------------------------------------------------- refusals
rc=0; "$MJ" plan delivery I9999 >/dev/null 2>d.err || rc=$?
[ "$rc" != 0 ] && grep -q I9999 d.err || fail "an undeclared issue was answered (exit $rc)"
rc=0; "$MJ" plan delivery --at no-such-rev >/dev/null 2>d.err || rc=$?
[ "$rc" != 0 ] && grep -q no-such-rev d.err || fail "an unresolvable revision was answered (exit $rc)"

# ---------------------------------------------------------------- the site data is the executable's answer
# site/data/generated/delivery.json is `plan delivery --at <the commit it names>` of this
# repository, pretty-printed and nothing else; scripts/generate-site-data --check is the gate,
# and this is the same comparison made directly. A hand edit must be visible to it.
SITE="$ROOT/site/data/generated/delivery.json"
if [ -f "$SITE" ]; then
  at="$(jq -r .observed.commit "$SITE")"
  if git -C "$ROOT" cat-file -e "$at^{commit}" 2>/dev/null; then
    "$MJ" --repo "$ROOT" --json plan delivery --at "$at" | jq . > site.json
    cmp -s site.json "$SITE" || { diff "$SITE" site.json | head -20; fail "delivery.json is not the executable's answer at $at"; }
    jq '.issues[0].stage = "deployed" | .issues[0].settled = true' "$SITE" > edited.json
    if cmp -s site.json edited.json; then fail "a hand edit of delivery.json reads as the executable's answer"; fi
  else
    printf '    note: %s names %s, which this clone does not hold; the comparison was not made\n' "${SITE#"$ROOT"/}" "$at"
  fi
else
  fail "site/data/generated/delivery.json is not generated"
fi
