# majordomus-covers: usecase
# majordomus-negative: usecase
# majordomus-timeout: 300
# A scenario declares where it runs (ADR 38). A live scenario asks its questions of the
# repository the command was invoked in, so the tool holds it to what a question may do:
# no setup, only commands share/commands.yaml calls read-only, obligations asserted only
# where something owes them. And because CI cannot reproduce one machine's tree at one
# minute, a live scenario is never selected by a bare run, its evidence is never committed,
# and it never raises a command to covered. Every refusal here is decided from a
# declaration that already exists; none of them is the author's care.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
UC=.ai/repo/use-cases
mkdir -p docs
printf 'version: 1\nstatuses:\n  - id: guaranteed\n    meaning: Deterministic.\nclaims:\n  - id: exit-code-contract\n    claim: Exit codes mean one thing\n    source: docs/CLI.md\n    implementation: bin/majordomus\n    test: test/cases/00.sh\n    status: guaranteed\n    responsibility: none\n' > docs/CLAIMS.yaml

write_live() { # body of the scenario block
  cat > "$UC/ask-this-repository.md" <<MD
---
id: ask-this-repository
kind: use-case
title: 'Ask this repository a question'
summary: 'Read what this checkout says about itself, without changing it.'
category: adoption
status: active
target: advisory
actors: [operator]
difficulty: basic
commands: [version, start]
doctrines: []
claims: []
responsibilities: []
applications: []
---

# Situation

What does this checkout say about itself?

# Scenario

\`\`\`yaml
$1
\`\`\`

# Outcome

An answer about this repository, which is unchanged by the asking.
MD
}

# --- a live scenario needs no setup and runs a read-only command: accepted
write_live "mode: live
steps:
  - id: which-version
    run: ['version']
    note: 'the version of the tool standing here'
    expect:
      exit: 0
then:
  - 'the repository is unchanged'"
git add -A >/dev/null && git commit -qm "a live use case"
expect_exit 0 "$MJ" usecase validate
expect_grep 'usecase validate: 0 failure'

# Proves `live-scenario-is-read-only`: every refusal below is decided from a declaration
# that already exists — the mode, share/commands.yaml's class, share/obligations.yaml's
# vocabulary — and none of them from the author's care.

# --- a live scenario that names a setup is refused: it prepares nothing
write_live "mode: live
setup: bare
steps:
  - id: which-version
    run: ['version']
    expect:
      exit: 0"
expect_exit 10 "$MJ" usecase validate
expect_grep "names setup 'bare'"
expect_grep 'prepares nothing'

# --- a live step whose command is not read-only is refused, naming the command and the
#     class share/commands.yaml declares for it. `start` is state-mutating there, and the
#     use case lists it, so nothing but the class decides this.
write_live "mode: live
steps:
  - id: begin
    run: ['start', 'a task']
    expect:
      exit: 0"
expect_exit 10 "$MJ" usecase validate
expect_grep "runs 'start', which is state-mutating, in a live scenario"
expect_grep 'share/commands.yaml'

# --- an obligation step is a live step: a disposable fixture owes nothing
write_live "setup: bare
steps:
  - id: committed
    obligation: commit"
expect_exit 10 "$MJ" usecase validate
expect_grep "asserts obligation 'commit' in a fixture scenario"

# --- an obligation the vocabulary does not declare is refused, live or not
write_live "mode: live
steps:
  - id: invented
    obligation: no-such-obligation"
expect_exit 10 "$MJ" usecase validate
expect_grep "asserts obligation 'no-such-obligation', which share/obligations.yaml does not declare"

# --- a step is one thing or the other, never both
write_live "mode: live
steps:
  - id: both
    run: ['version']
    obligation: commit
    expect:
      exit: 0"
expect_exit 10 "$MJ" usecase validate
expect_grep "both runs 'version' and asserts obligation 'commit'"

# Proves `live-scenario-evidence-is-local`: what CI reproduces, where the evidence lands,
# and which verdict an undischarged obligation gets.

# --- a bare run does not select it, so what CI means by `usecase run` does not change
write_live "mode: live
steps:
  - id: which-version
    run: ['version']
    expect:
      exit: 0"
git add -A >/dev/null && git commit -qm "the live use case, restored"
expect_exit 0 "$MJ" usecase validate
expect_exit 0 "$MJ" usecase run
expect_grep '^ask-this-repository +live \(run it with --live\)'
LIVE=.ai/local/evidence/live/ask-this-repository.json
[ -f "$LIVE" ] && { echo "    a bare run produced live evidence"; exit 1; }

# --- --live runs it, against this repository, and writes its evidence to the local half
expect_exit 0 "$MJ" usecase run --live
expect_grep '^ask-this-repository +pass'
[ -f "$LIVE" ] || { echo "    no live evidence under .ai/local/evidence/live/"; exit 1; }
grep -q '"mode":"live"' "$LIVE" || { echo "    the evidence does not say it is live"; cat "$LIVE"; exit 1; }
[ -f .ai/local/evidence/use-cases/ask-this-repository.json ] && { echo "    live evidence was written where fixture evidence is committed"; exit 1; }
# and it is never committed: the tracked tree is untouched and git does not see the file
[ -z "$(git status --porcelain)" ] || { echo "    a live run changed the tracked tree"; git status --porcelain; exit 1; }
git check-ignore -q "$LIVE" || { echo "    live evidence is not ignored; it would be committed"; exit 1; }

# --- an obligation nothing owes is unmet, not failed: work not done is not a defect
write_live "mode: live
steps:
  - id: committed
    obligation: commit"
expect_exit 10 "$MJ" usecase run --live
expect_grep '^ask-this-repository +UNMET'
expect_grep 'no task is active here'
expect_grep '1 unmet'
"$MJ" usecase run --live --json > run.json 2>/dev/null || true
jq -e '.unmet == 1 and .failed == 0 and .ran == 1' run.json >/dev/null \
  || { echo "    the JSON run does not report the unmet as unmet"; cat run.json; exit 1; }
jq -e '.results[0].mode == "live" and .results[0].result == "unmet"' run.json >/dev/null \
  || { echo "    the JSON result does not carry the live verdict"; cat run.json; exit 1; }

# --- a live scenario names its commands and never covers them: CI cannot re-run it
write_live "mode: live
steps:
  - id: which-version
    run: ['version']
    expect:
      exit: 0"
expect_exit 0 "$MJ" usecase coverage
expect_grep '^command +version +1 +0 +0 +partial'
expect_exit 0 "$MJ" usecase coverage --json
expect_grep '"id":"version","use_cases":1,"executable":0,"evidence":0,"status":"partial"'
