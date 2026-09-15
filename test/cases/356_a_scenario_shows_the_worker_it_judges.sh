# majordomus-covers: usecase
# majordomus-negative: usecase
# A scenario can show the change it judges. A `worker` step is one shell line run in the
# scenario's disposable repository between two invocations of the tool, recorded with
# `actor: worker`, so one scenario can tell a whole story: a worker strays, the tool refuses,
# the worker fixes it, the tool accepts. The challenge on the website is exactly that scenario,
# and this case holds each link of the chain it rests on.
#
# What is proved:
#   - a worker step runs, changes the repository, and the next tool step answers the change;
#     its evidence is `actor: worker`, a tool step's is `actor: tool`
#   - a recorded tool command is quoted the way a reader would type it, so it pastes back
#   - a worker step is refused in a live scenario, beside `run`, and without an exit code
#   - `finish` names each blocking doctrine on its own line (the list once lost its newlines)
#   - the challenge this repository publishes passes against this tree's own tool, refuses at
#     least once, and ends with the work accepted
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null && "$MJ" update >/dev/null
mkdir -p lib docs && echo a > lib/a && echo d > docs/d
git add -A >/dev/null && git commit -qm install
UC=.ai/repo/use-cases

# --- a worker strays, check refuses, the worker takes it back, check accepts
cat > "$UC/judge-a-stray-change.md" <<'MD'
---
id: judge-a-stray-change
kind: use-case
title: Judge a stray change
summary: A worker edits outside the scope and the tool says so.
category: workers
status: active
target: advisory
commands: [start, check]
---

# Situation

s

# Scenario

```yaml
setup: installed
steps:
  - id: claim
    run: ['start', 'a task with spaces', '--scope', 'lib']
    expect:
      exit: 0
  - id: stray
    worker: 'echo stray >> docs/d'
    expect:
      exit: 0
  - id: refused
    run: ['check']
    expect:
      exit: 10
      stdout_contains: ['FAIL scope', 'docs/d']
  - id: take-it-back
    worker: 'git checkout -- docs/d'
    expect:
      exit: 0
  - id: accepted
    run: ['check']
    expect:
      exit: 0
```

# Outcome

o
MD
expect_exit 0 "$MJ" usecase validate
rc=0; "$MJ" usecase run judge-a-stray-change --json > run.json 2> run.err || rc=$?
[ "$rc" = 0 ] || { echo "    usecase run exited $rc"; cat run.err run.json; exit 1; }
jq -e '.results[0].result == "pass"' run.json >/dev/null || { echo "    the worker scenario did not pass"; cat run.json; exit 1; }
jq -e '[.results[0].steps[].actor] == ["tool","worker","tool","worker","tool"]' run.json >/dev/null \
  || { echo "    steps are not recorded with their actor"; jq -c '[.results[0].steps[] | {id, actor}]' run.json; exit 1; }
jq -e '.results[0].steps[1].command == "echo stray >> docs/d"' run.json >/dev/null \
  || { echo "    a worker step is not shown as the line the worker ran"; exit 1; }
jq -e '.results[0].steps[0].command == "majordomus start '"'"'a task with spaces'"'"' --scope lib"' run.json >/dev/null \
  || { echo "    a recorded command does not quote an argument with spaces: $(jq -r '.results[0].steps[0].command' run.json)"; exit 1; }
# the tree the case runs in is untouched: the worker changed the disposable repository only
expect_exit 0 git diff --quiet -- docs/d

# a mutation must fail before a green case means anything: the stray line never happens
sed -i.bak "s/worker: 'echo stray >> docs\/d'/worker: 'true'/" "$UC/judge-a-stray-change.md" && rm -f "$UC/judge-a-stray-change.md.bak"
expect_exit 10 "$MJ" usecase run judge-a-stray-change
expect_grep 'expected exit 10, got 0'
git checkout -q -- "$UC" 2>/dev/null || true
rm -f "$UC/judge-a-stray-change.md"

# --- the refusals: live mode, beside run, without an exit code
cat > "$UC/refuse-a-live-worker.md" <<'MD'
---
id: refuse-a-live-worker
kind: use-case
title: Refuse a live worker
summary: A live scenario may not change the repository.
category: workers
status: active
target: advisory
commands: [check]
---

# Situation

s

# Scenario

```yaml
mode: live
steps:
  - id: change-this-repository
    worker: 'echo x >> docs/d'
    expect:
      exit: 0
  - id: both
    run: ['check']
    worker: 'true'
    expect:
      exit: 0
  - id: no-exit
    worker: 'true'
```

# Outcome

o
MD
expect_exit 10 "$MJ" usecase validate
expect_grep "step 'change-this-repository' does worker 'echo x >> docs/d' in a live scenario"
expect_grep "step 'both' both runs 'check' and does worker 'true'"
expect_grep "step 'no-exit' expects no exit code"
rm -f "$UC/refuse-a-live-worker.md"

# --- finish names each blocking doctrine on its own line
expect_exit 0 "$MJ" start "t" --scope lib
echo stray >> docs/d
expect_exit 10 "$MJ" finish --outcome completed
expect_grep '^blocking doctrines:$'
expect_grep '^- majordomus\.scope-integrity$'
expect_no_grep '^- majordomus\.[a-z-]*- '
git checkout -q -- docs/d

# --- the published challenge holds against this tree's own tool
CH="$(sed -n 's/^use_case = "\(.*\)"$/\1/p' "$ROOT/site/data/challenge.toml")"
[ -n "$CH" ] || { echo "    site/data/challenge.toml names no use case"; exit 1; }
rc=0; "$MJ" --repo "$ROOT" usecase run "$CH" --json > challenge.json 2> challenge.err || rc=$?
[ "$rc" = 0 ] || { echo "    the challenge run exited $rc"; cat challenge.err challenge.json; exit 1; }
jq -e '.results[0].result == "pass"' challenge.json >/dev/null || { echo "    the challenge scenario $CH does not pass"; exit 1; }
jq -e '[.results[0].steps[] | select(.actor == "tool" and .exit != 0)] | length > 0' challenge.json >/dev/null \
  || { echo "    the challenge scenario refuses nothing"; exit 1; }
jq -e '.results[0].steps | last | .actor == "tool" and .exit == 0' challenge.json >/dev/null \
  || { echo "    the challenge scenario does not end with the work accepted"; exit 1; }
jq -e '[.results[0].steps[] | select(.actor == "worker")] | length > 0' challenge.json >/dev/null \
  || { echo "    the challenge scenario judges no change a worker made"; exit 1; }
