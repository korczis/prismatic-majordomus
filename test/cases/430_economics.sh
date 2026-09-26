# majordomus-covers: none
# claims: context-selection-counted
# majordomus-timeout: 600
# Token economics end to end, without a provider. The live runner is driven through a
# stand-in harness that answers the way Claude Code's stream-json does: it reports usage,
# makes the task's reference change in the workspace, and says what it did — so the whole
# pipeline runs for real (workspace from the fixture, Majordomus installed for the treatment,
# sessions, hidden acceptance tests, gate verdicts, the record) and only the model is absent.
# The stand-in also writes down what it saw from inside its workspace, because the workspace
# itself is gone by the time the run is recorded.
#
# Then: the workspaces named nothing, held nothing but the repository, and were removed; the
# sessions saw the caller's credentials but not its session; the records carry no prompt
# text; the summary pairs the runs and derives the reduction from the reported usage; every
# task's hidden tests fail on its start and pass on its reference; the context suite counts
# with the named tokenizer; and the JSON the command line prints is the JSON the HTTP API
# serves.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v python3 >/dev/null || { echo "    skip: no python3, which the fixture's tests need"; exit 0; }

R="$T/repo"
fixture_repo "$R" .ai/repo/benchmarks/economics test/fixtures/economics >/dev/null
# the declarations without the evidence this repository recorded under them: every record
# below is written by this case, so "no evidence yet" and "a dry run wrote nothing" hold
# whatever the repository's own runs/ holds
rm -rf "$R/.ai/repo/benchmarks/economics/runs"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" commit -qm fixture >/dev/null
mj() { ( cd "$R" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }

# ---------------------------------------------------------------- no evidence yet
expect_exit 0 mj economics summary
expect_grep 'No verified total-token-savings claim is available'
expect_no_grep 'reduced by|increased by'

# ---------------------------------------------------------------- the references
expect_exit 0 mj economics references --work-dir "$T/refs"
expect_grep 'ok   csv-credit-notes: fails on the starting state, passes on the reference'
[ "$(grep -c '^ok ' <<<"$LAST_OUT")" = 7 ] || { echo "    expected 7 tasks proven: $LAST_OUT"; exit 1; }
[ -z "$(find "$T/refs" -name 'test_accept_*' 2>/dev/null)" ] || { echo "    a reference check left hidden tests behind"; exit 1; }

# ---------------------------------------------------------------- a stand-in harness
H="$T/fake-claude"
cat > "$H" <<'SH'
#!/bin/sh
# A harness that behaves like `claude -p ... --output-format stream-json`: it answers
# --version, makes the reference change of the task named in FAKE_PATCH, and prints usage.
if [ "$1" = "--version" ]; then echo "9.9.9 (Stand-in)"; exit 0; fi
# what this session can see, one line per session, for the case to judge afterwards
ai=no; [ -d .ai ] && ai=yes
region=no; grep -q 'majordomus:begin' CLAUDE.md && region=yes
team=no; grep -q 'Run the tests with' CLAUDE.md && team=yes
ensure=$(sed -n 's/^  ensure_server_on_start: *\([a-z]*\).*/\1/p' .ai/repo/policy.yaml 2>/dev/null)
printf 'pwd=%s ai=%s region=%s team=%s ensure=%s autoupdater=%s session=%s routing=%s target=%s beside=%s\n' \
  "$(pwd -P)" "$ai" "$region" "$team" "${ensure:-none}" "${DISABLE_AUTOUPDATER:-unset}" \
  "${CLAUDE_CODE_SESSION_ID:-unset}" "${CLAUDE_CODE_USE_BEDROCK:-unset}" "${CARGO_TARGET_DIR:-unset}" \
  "$(ls -A .. | tr '\n' ',')" >> "$FAKE_LOG"
git apply "$FAKE_PATCH" || exit 1
if [ -d .ai ]; then input=72000 first=9000; else input=90000 first=6000; fi
cat <<JSON
{"type":"system","subtype":"init","tools":["Bash","Edit","Read"],"mcp_servers":[]}
{"type":"assistant","message":{"id":"m1","model":"claude-sonnet-5","content":[{"type":"tool_use","name":"Bash","input":{"command":"cat ledgerlite/csv_export.py"}}],"usage":{"input_tokens":2,"cache_creation_input_tokens":$first,"cache_read_input_tokens":0,"output_tokens":100}}}
{"type":"assistant","message":{"id":"m2","model":"claude-sonnet-5","content":[{"type":"tool_use","name":"Bash","input":{"command":"sed -i '' 's/a/b/' ledgerlite/csv_export.py"}}],"usage":{"input_tokens":2,"cache_creation_input_tokens":0,"cache_read_input_tokens":$((input - first - 4)),"output_tokens":900}}}
{"type":"result","subtype":"success","is_error":false,"num_turns":2,"total_cost_usd":0.05,"modelUsage":{"claude-sonnet-5":{"inputTokens":4,"outputTokens":1000,"cacheReadInputTokens":$((input - first - 4)),"cacheCreationInputTokens":$first,"costUSD":0.05}}}
JSON
SH
chmod +x "$H"

# The caller is itself a Claude Code session with a build of its own: the runner must hand the
# sessions its credentials and provider routing, and never its session or its build.
mkdir -p "$T/tmp"
: > "$T/fake.log"
expect_exit 0 env FAKE_PATCH="$R/test/fixtures/economics/reference/csv-credit-notes.patch" \
  FAKE_LOG="$T/fake.log" TMPDIR="$T/tmp" CLAUDE_CODE_SESSION_ID=caller-session \
  CLAUDE_CODE_USE_BEDROCK=1 CARGO_TARGET_DIR="$T/operator-target" \
  sh -c "cd '$R' && MAJORDOMUS_SHARE='$R/share' '$RB' economics run --suite pilot --task csv-credit-notes --repetition 1 --harness '$H' --work-dir '$T/work' --parallel 2"
expect_grep '2 run\(s\) recorded, 0 failed'
REC="$R/.ai/repo/benchmarks/economics/runs/pilot"
expect_file "$REC/csv-credit-notes--baseline--r1.json"
expect_file "$REC/csv-credit-notes--majordomus--r1.json"

# one session per arm; each ran in a workspace whose path names neither task, arm nor
# benchmark, with nothing beside it but its empty MCP configuration
[ "$(wc -l < "$T/fake.log" | tr -d ' ')" = 2 ] || { echo "    expected two sessions:"; cat "$T/fake.log"; exit 1; }
while read -r line; do
  grep -qE '^pwd=[^ ]*/tmp/mj-bench-[0-9a-f]{12}/repo ' <<<"$line" || { echo "    a workspace is not neutral: $line"; exit 1; }
  grep -q ' beside=mcp.json,repo,$' <<<"$line" || { echo "    a workspace sat beside something: $line"; exit 1; }
  # credentials and routing reach the session; the caller's session and build do not
  grep -q ' autoupdater=1 session=unset routing=1 target=unset ' <<<"$line" || { echo "    the session environment is wrong: $line"; exit 1; }
done < "$T/fake.log"
# the treatment really had Majordomus installed, its server switched off and the team's own
# CLAUDE.md kept; the control really did not
grep -q ' ai=yes region=yes team=yes ensure=false ' "$T/fake.log" || { echo "    the treatment was not installed as declared:"; cat "$T/fake.log"; exit 1; }
grep -q ' ai=no region=no team=yes ensure=none ' "$T/fake.log" || { echo "    the control was not the bare fixture:"; cat "$T/fake.log"; exit 1; }

# no workspace outlived its run, and no hidden test is anywhere a later session could find it
[ -z "$(find "$T/tmp" -maxdepth 1 -name 'mj-bench-*')" ] || { echo "    a workspace was left behind: $(ls "$T/tmp")"; exit 1; }
[ -z "$(find "$T/work" -name 'test_accept_*' -o -name .git)" ] || { echo "    the work directory holds a workspace: $(find "$T/work")"; exit 1; }
for arm in baseline majordomus; do
  grep -q '"type":"result"' "$T/work/pilot/csv-credit-notes--$arm--r1/session-1.jsonl" \
    || { echo "    the $arm transcript is not in the work directory"; exit 1; }
done

# every gate passed, the session gate first, the usage is the harness's, and the edit made
# through the shell ended orientation
for arm in baseline majordomus; do
  jq -e '.outcome.completed and .outcome.checks[0].id == "sessions_completed" and ([.outcome.checks[] | select(.passed | not)] | length == 0)' "$REC/csv-credit-notes--$arm--r1.json" >/dev/null \
    || { echo "    the $arm run did not pass its gates: $(jq -c .outcome "$REC/csv-credit-notes--$arm--r1.json")"; exit 1; }
done
jq -e '.sessions[0].models[0].cache_read_input_tokens == 62996 and .sessions[0].orientation.edited and .harness.version == "9.9.9"' \
  "$REC/csv-credit-notes--majordomus--r1.json" >/dev/null || { echo "    the treatment record lost what the harness reported"; exit 1; }
jq -e '(.outcome.changed_files | index("tests/test_accept_csv_credit_notes.py")) == null' "$REC/csv-credit-notes--baseline--r1.json" >/dev/null \
  || { echo "    the hidden tests were counted as the agent's change"; exit 1; }

# nothing the task said reaches a record
expect_no_grep 'credit notes|10.50 EUR|-11.50|cat ledgerlite' "$REC/csv-credit-notes--baseline--r1.json"

# the pair: 91000 against 73000 total tokens, both completed
S="$(mj economics summary --format json --task csv-credit-notes 2>/dev/null)"
jq -e '.pairs[0].status == "valid" and .pairs[0].token_reduction == 0.197802197802' <<<"$S" >/dev/null \
  || { echo "    unexpected pair: $(jq -c '.pairs[0] | {status, token_reduction, reasons}' <<<"$S")"; exit 1; }
jq -e '[.metrics[] | select(.id == "effective_token_reduction")][0] | .status == "preliminary" and .class == "derived" and .inputs == "observed" and .n == 1' <<<"$S" >/dev/null \
  || { echo "    the primary metric is mislabelled: $(jq -c '.metrics[0]' <<<"$S")"; exit 1; }
jq -e '.verdict.publishable == false and (.verdict.statement | startswith("No verified total-token-savings claim is available"))' <<<"$S" >/dev/null \
  || { echo "    one pair must not publish a claim: $(jq -c .verdict <<<"$S")"; exit 1; }
expect_exit 0 mj economics explain effective_token_reduction
expect_grep 'derived from observed'
expect_grep 'reproduce +majordomus economics run --suite pilot'

# ---------------------------------------------------------------- the context suite, counted
expect_exit 0 mj economics measure --dry-run
expect_grep '^context: [0-9]+ seed\(s\) at [0-9a-f]{12}, [0-9]+ candidate token\(s\), [0-9]+ selected \(o200k_base tiktoken-rs [0-9]+\.[0-9]+\.[0-9]+\)$'
expect_grep 'dry run: nothing written'
[ ! -e "$R/.ai/repo/benchmarks/economics/runs/context" ] || { echo "    a dry run wrote a record"; exit 1; }
expect_exit 0 mj economics measure
expect_grep '^recorded \.ai/repo/benchmarks/economics/runs/context/[0-9a-f]{12}\.json'
C=$(ls "$R"/.ai/repo/benchmarks/economics/runs/context/*.json)
jq -e '.tokenizer.encoding == "o200k_base" and (.seeds | length) > 0 and all(.seeds[]; .selected_tokens <= .candidate_tokens and .candidate_tokens <= .considered_tokens)' "$C" >/dev/null \
  || { echo "    the context record is malformed: $(jq -c '{tokenizer, n: (.seeds|length)}' "$C")"; exit 1; }
mj economics summary --format json 2>/dev/null > "$T/after-measure.json"
jq -e '[.metrics[] | select(.id == "context_reduction_ratio")][0] | .status == "measured" and .inputs == "counted" and (.not | contains("not total token savings"))' "$T/after-measure.json" >/dev/null \
  || { echo "    the context metric is mislabelled"; exit 1; }

# ---------------------------------------------------------------- one answer, every transport
( cd "$R" && MAJORDOMUS_SHARE="$R/share" exec "$RB" serve --port 0 --idle 60 ) > "$T/serve.log" 2>&1 &
serve_pid=$!
url=""
for _ in $(seq 1 100); do
  url="$(sed -n 's/.*listening on \(http:[^ ]*\).*/\1/p' "$T/serve.log" | head -1)"
  [ -n "$url" ] && break
  sleep 0.1
done
[ -n "$url" ] || { echo "    the server never logged its address:"; cat "$T/serve.log"; kill $serve_pid 2>/dev/null; exit 1; }
curl -fsS "$url/api/v1/economics" > "$T/http.json" || { kill $serve_pid 2>/dev/null; echo "    GET /api/v1/economics failed"; exit 1; }
curl -fsS "$url/cockpit/economics" > "$T/cockpit.html" || { kill $serve_pid 2>/dev/null; echo "    GET /cockpit/economics failed"; exit 1; }
kill $serve_pid 2>/dev/null; wait $serve_pid 2>/dev/null || true
grep -q 'No verified total-token-savings claim is available' "$T/cockpit.html" || { echo "    the Cockpit does not show the verdict"; exit 1; }
mj economics summary --format json > "$T/cli.json" 2>/dev/null
cmp -s <(jq -S . "$T/http.json") <(jq -S . "$T/cli.json") || { echo "    HTTP and the command line disagree"; diff <(jq -S . "$T/http.json") <(jq -S . "$T/cli.json") | head -20; exit 1; }
