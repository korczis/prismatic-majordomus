# majordomus-covers: plan
# A pull request does not land a merge whose delivery differs from the committed projection
# (I4634). On 2026-11-17 #3077's title named issue ids its head did not prove, its merge
# ae13be1bd13 counted two issues more than site/data/generated/delivery.json, and the trunk's
# site refused its own delivery until #3084 repaired master.
#
# `majordomus plan delivery --at <commit> --merge-title <t> --merge-branch <b>` reads the
# commit as the merge the forge will make; scripts/ci/merge-delivery-check, the gate
# merge-delivery, compares that answer's tallies with the committed projection and names each
# issue that would move, with its cause and its remedy. This drives both over a fixture:
#
#   a test merge whose title names nothing new             OK
#   the same merge titled with an issue its head lacks     REFUSED, the title, retitle
#   a trunk that moved after the projection was observed   REFUSED, moved, scripts/derive
#   the runner's event: a pull request into the trunk     judged from the event
#   an issue only the projection has                       REFUSED, named
#   a pull request into another branch, no event, no file  NOTE, nothing to judge
#
# Each MUTATION names a change to the code that this case must fail on.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
fail() { printf '    %s\n' "$*"; exit 1; }
GATE="$ROOT/scripts/ci/merge-delivery-check"
[ -x "$GATE" ] || fail "scripts/ci/merge-delivery-check is not executable"
# the runner's own event must not decide what this fixture judges
unset GITHUB_EVENT_NAME GITHUB_EVENT_PATH

"$MJ" init >/dev/null
pj_init; pj_milestone M1
for i in I0001 I0002 I0003; do pj_issue "$i" M1; done
git add -A >/dev/null && git commit -qm "plan: three issues"
TRUNK="$(git rev-parse --abbrev-ref HEAD)"

# the pull request's branch: a change, then its projection observed at that change
git checkout -q -b feature/a-piece
printf 'echo piece\n' > piece.sh; git add piece.sh; git commit -qm "feat: a piece"
mkdir -p site/data/generated
"$MJ" --json plan delivery --at HEAD > site/data/generated/delivery.json \
  || fail "plan delivery --at HEAD exited non-zero"
git add -A >/dev/null && git commit -qm "chore: derive"
git checkout -q "$TRUNK"
BASE="$(git rev-parse HEAD)"
test_merge() { # the forge's test merge of the branch into the trunk as it stands
  git checkout -q --detach "$1"
  git merge -q --no-ff feature/a-piece -m "Merge 1111111 into 2222222"
}
gate() { # env assignments...: the gate's output in out.txt, its exit code in $rc
  rc=0; env "$@" MJ_ROOT="$PWD" "$GATE" > out.txt 2>&1 || rc=$?
}

# ---------------------------------------------------------------- the anticipated merge
test_merge "$BASE"
"$MJ" --json plan delivery --at HEAD --merge-title "feat: a piece (I0001)" --merge-branch feature/a-piece > m.json \
  || fail "plan delivery --merge-title exited non-zero"
# MUTATION: the anticipated merge left out of the attribution (`ahead.iter().chain` ->
# `all_merges.iter()`) -> I0001 reads none.
[ "$(jq -r '.issues[] | select(.issue == "I0001") | .stage' m.json)" = merged ] \
  || fail "the anticipated title does not move I0001 to merged: $(jq -c '.issues[0]' m.json)"
jq -e '.sources[] | select(.name == "anticipated_merge") | .state == "read"' m.json >/dev/null \
  || fail "the answer does not say a merge was anticipated"
"$MJ" --json plan delivery --merge-title "feat: a piece (I0001)" > live.json 2> live.err \
  && fail "an anticipated merge was answered live"
grep -q "anticipated merge" live.err || fail "the live refusal does not say why: $(cat live.err)"

# ---------------------------------------------------------------- the gate
gate MJ_MERGE_TITLE="feat: a piece" MJ_MERGE_BRANCH=feature/a-piece
[ "$rc" = 0 ] || fail "a merge that names nothing new was refused ($rc): $(cat out.txt)"
grep -q "^OK merge-delivery" out.txt || fail "no OK line: $(cat out.txt)"

gate MJ_MERGE_TITLE="feat: a piece (I0001)" MJ_MERGE_BRANCH=feature/a-piece
# MUTATION: the gate compares the plain answer instead of the merge's (`ask merge.json`
# without the merge arguments) -> exit 0.
[ "$rc" = 10 ] || fail "a title naming an issue its head lacks was not refused ($rc): $(cat out.txt)"
grep -q "I0001: none -> merged, because the pull request's title or branch names it" out.txt \
  || fail "the refusal does not name I0001 and the title: $(cat out.txt)"
grep -q "remedy: retitle" out.txt || fail "the refusal names no retitle: $(cat out.txt)"
grep -q "scripts/derive" out.txt && fail "a title's refusal asks for a derive: $(cat out.txt)"

# the trunk moves after the projection was observed: another pull request merges I0002
git checkout -q "$TRUNK"
git checkout -q -b feature/other
printf 'echo other\n' > other.sh; git add other.sh; git commit -qm "feat: other"
git checkout -q "$TRUNK"
git merge -q --no-ff feature/other -m "Merge pull request #4 from someone/feature/other" -m "feat: other (I0002)"
test_merge "$TRUNK"
gate MJ_MERGE_TITLE="feat: a piece" MJ_MERGE_BRANCH=feature/a-piece
[ "$rc" = 10 ] || fail "a moved trunk was not refused ($rc): $(cat out.txt)"
# MUTATION: every cause called the title (`then "title" else "moved"` -> `"title"`)
# -> no 'moved' line and no derive remedy.
grep -q "I0002: none -> merged, because the trunk or the head moved" out.txt \
  || fail "the refusal does not name I0002 as moved: $(cat out.txt)"
grep -q "remedy: merge the trunk into the branch and run scripts/derive" out.txt \
  || fail "the moved refusal names no derive: $(cat out.txt)"

# an issue only the projection has (dropped from the plan) is named too, from either side
test_merge "$BASE"
git show feature/a-piece:site/data/generated/delivery.json \
  | jq '.issues += [{"issue": "I0099", "stage": "merged"}] | .tallies.merged += 1' > site/data/generated/delivery.json
gate MJ_MERGE_TITLE="feat: a piece" MJ_MERGE_BRANCH=feature/a-piece
# MUTATION: the issues iterated from the merge's answer only (`$m | keys[]`) -> no I0099 line.
[ "$rc" = 10 ] && grep -q "I0099: merged -> none" out.txt \
  || fail "an issue only the projection has was not named ($rc): $(cat out.txt)"
git checkout -q -- site/data/generated/delivery.json

# ---------------------------------------------------------------- the runner's event
test_merge "$BASE"
event() { # base title
  printf '{"pull_request":{"title":"%s","head":{"ref":"feature/a-piece"},"base":{"ref":"%s"}}}\n' "$2" "$1" > event.json
}
event master "feat: a piece (I0001)"
gate GITHUB_EVENT_NAME=pull_request GITHUB_EVENT_PATH="$PWD/event.json"
[ "$rc" = 10 ] || fail "the event's title was not judged ($rc): $(cat out.txt)"
event release/x "feat: a piece (I0001)"
gate GITHUB_EVENT_NAME=pull_request GITHUB_EVENT_PATH="$PWD/event.json"
[ "$rc" = 0 ] && grep -q "not the trunk" out.txt || fail "a pull request into another branch was judged ($rc): $(cat out.txt)"
gate GITHUB_EVENT_NAME=push
[ "$rc" = 0 ] && grep -q "not a pull request" out.txt || fail "a push was judged ($rc): $(cat out.txt)"
git rm -q site/data/generated/delivery.json
gate MJ_MERGE_TITLE="feat: a piece (I0001)"
[ "$rc" = 0 ] && grep -q "commits no site/data/generated/delivery.json" out.txt \
  || fail "a tree with no projection was judged ($rc): $(cat out.txt)"
MJ_ROOT="$PWD" "$GATE" extra > out.txt 2>&1 && fail "an argument was accepted"

# ---------------------------------------------------------------- this repository
grep -q 'runs: scripts/ci/merge-delivery-check' "$ROOT/.ai/repo/ci/gates.yaml" || fail "the gate is not registered"
