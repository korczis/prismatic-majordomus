# majordomus-covers: evidence finish
# Gate state is something the runtime can answer (I1512).
#
# `gates.completion` judges one task's change set; with no task open it attributes nothing.
# `gates.state` answers about the gates themselves: every gate the CI model declares and the
# standing of its last recorded run, whichever task recorded it. This case gives a
# disposable repository a model of three gates and walks what a reader must be able to tell
# apart:
#
#   never reported   queued, and named under never_reported — not pass
#   pass / fail      the last run's exit, over the files as they stand now
#   stale            a passing run whose inputs changed since
#   after the task   the record survives `finish`: the state is the gates', not the task's
#   offline          the forge is named as not consulted, on every answer
#
# and that the HTTP projection is the same capability (its route is declared once).
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3059.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

mkdir -p .ai/repo/ci lib docs
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: unit
    job: suite
    runs: "true"
    summary: the unit tests pass
  - id: docs-check
    job: structure
    runs: "true"
    summary: every reference resolves
  - id: probe
    job: site
    runs: "true"
    summary: every route answers
classes:
  - id: src
    paths: [lib/**]
    gates: [unit]
  - id: docs
    paths: [docs/**]
    gates: [docs-check]
  - id: site
    paths: [site/**]
    gates: [probe]
YAML
echo 'a() { :; }' > lib/a.sh; echo '# doc' > docs/guide.md
git add -A >/dev/null && git commit -qm base

export MAJORDOMUS_SHARE="$ROOT/share"
state() { "$BIN" run gates.state --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }
status_of() { state | jq -r --arg g "$1" '.gates[] | select(.id == $g) | .status'; }
expect_status() {   # expect_status GATE STATUS WHEN
  local got; got="$(status_of "$1")"
  [ "$got" = "$2" ] || { echo "    $1 is '$got', not $2 ($3): $(state | jq -c --arg g "$1" '.gates[] | select(.id == $g)')"; exit 1; }
}

# ---------------------------------------------------------------- nothing recorded
[ "$(state | jq -r .count)" = 3 ] || { echo "    the declared set is $(state | jq -r .count) gates, not 3"; exit 1; }
for g in unit docs-check probe; do expect_status "$g" queued "before any run"; done
[ "$(state | jq -c .never_reported)" = '["unit","docs-check","probe"]' ] \
  || { echo "    never_reported is $(state | jq -c .never_reported)"; exit 1; }
state | jq -e '.unreached | any(test("forge"))' >/dev/null \
  || { echo "    the answer does not say the forge was not consulted: $(state | jq -c .unreached)"; exit 1; }
state | jq -e '.findings | any(test("no gate run has ever been recorded"))' >/dev/null \
  || { echo "    a checkout with no record does not say so: $(state | jq -c .findings)"; exit 1; }
HEAD_NOW="$(git rev-parse HEAD)"
[ "$(state | jq -r .head)" = "$HEAD_NOW" ] || { echo "    head is $(state | jq -r .head), not $HEAD_NOW"; exit 1; }

# ---------------------------------------------------------------- runs recorded by a task
expect_exit 0 "$MJ" start "gated work" --scope lib/ --scope docs/
expect_exit 0 "$MJ" evidence --gate unit --exit 0 --command 'true'
expect_exit 0 "$MJ" evidence --gate docs-check --exit 2 --command 'false'
expect_status unit pass "a passing run over this tree"
expect_status docs-check fail "a failing run"
expect_status probe queued "nobody ran it"
[ "$(state | jq -c .never_reported)" = '["probe"]' ] \
  || { echo "    never_reported is $(state | jq -c .never_reported), not [probe]"; exit 1; }
state | jq -e '.gates[] | select(.id == "unit") | .on_head == true and (.last_run.head | length) > 0' >/dev/null \
  || { echo "    the run does not say it was taken at this head"; exit 1; }
[ "$(state | jq -r '.tallies.pass, .tallies.fail, .tallies.queued' | tr '\n' ' ')" = '1 1 1 ' ] \
  || { echo "    tallies are $(state | jq -c .tallies)"; exit 1; }

# ---------------------------------------------------------------- the tree moves: stale
echo 'b() { :; }' >> lib/a.sh
expect_status unit stale "lib/ changed under a passing run"
expect_status docs-check fail "docs/ did not change"

# ---------------------------------------------------------------- the state outlives the task
git checkout -q -- lib/a.sh
expect_exit 0 "$MJ" finish --outcome partial --verify-command "test -d .ai" --note "$S/note.md"
expect_status unit pass "the task is closed and the record still stands"
expect_status docs-check fail "a failure is not forgotten when the task closes"
expect_status probe queued "still never reported"

# ---------------------------------------------------------------- one capability, every surface
# the HTTP route is the registry's projection of the same declaration, not a second reader
"$BIN" capabilities describe gates.state --format json --repo . 2>/dev/null \
  | jq -e '[.. | strings] | any(. == "/api/v1/gates/state") and any(. == "majordomus_gate_state")' >/dev/null \
  || { echo "    gates.state is not projected to /api/v1/gates/state and majordomus_gate_state"; exit 1; }
