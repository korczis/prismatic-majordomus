# majordomus-covers: doctor
# claims: models-local-runtime-observed, models-observed-evidence-routes, models-observation-stays-local
# A local inference runtime, observed end to end (ADR 0174): a fake Ollama answering on a
# loopback port stands in for the runtime, scripts/inference-observe reaches it through
# the endpoint the catalogue declares, `models observe` admits what it found, `models
# observed` reads it back, and `models route --observed` chooses from it with the evidence
# in the reason. No model and no network beyond the loopback interface.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v node >/dev/null 2>&1 || { echo "    node is not installed"; exit 1; }

# ---------------------------------------------------------------- 0. every adapter against fakes
# Ollama, LM Studio and llama-server, each by the responses its runtime documents: identity,
# inventory, timeouts, probes, a weights path that is not an id, and the plan order.
node --test "$ROOT/scripts/lib/inference/inference.test.mjs" > "$T/unit.txt" 2>&1 \
  || { echo "    the transport's unit tests fail"; grep -E '^not ok|error:' "$T/unit.txt" | head -20; exit 1; }
grep -q '^# fail 0' "$T/unit.txt" || { echo "    the transport's unit tests did not report"; exit 1; }

R="$T/repo"
fixture_repo "$R" >/dev/null
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" commit -qm fixture >/dev/null

# A fake Ollama: the native signature, two models, one loaded, a chat that answers.
cat > "$T/fake-ollama.mjs" <<'JS'
import http from 'node:http';
import { writeFileSync } from 'node:fs';
const routes = {
  'GET /api/version': { version: '0.0.0-case' },
  'GET /api/tags': { models: [
    { name: 'tiny:chat', digest: 'aa11', size: 1000, details: { family: 'llama', quantization_level: 'Q4_0', format: 'gguf' } },
    { name: 'tiny:embed', digest: 'bb22', size: 500, details: { family: 'bert' } },
  ] },
  'GET /api/ps': { models: [{ name: 'tiny:chat', context_length: 4096 }] },
  'POST /api/chat': { message: { role: 'assistant', content: 'ok' } },
};
const server = http.createServer((req, res) => {
  let body = '';
  req.on('data', (c) => { body += c; });
  req.on('end', () => {
    let answer = routes[`${req.method} ${req.url}`];
    if (req.url === '/api/show') {
      const { model } = JSON.parse(body);
      answer = model === 'tiny:chat'
        ? { capabilities: ['completion'], model_info: { 'general.architecture': 'llama', 'llama.context_length': 8192 } }
        : { capabilities: ['embedding'] };
    }
    res.writeHead(answer ? 200 : 404, { 'content-type': 'application/json' });
    res.end(JSON.stringify(answer ?? {}));
  });
});
server.listen(0, '127.0.0.1', () => writeFileSync(process.argv[2], String(server.address().port)));
JS
node "$T/fake-ollama.mjs" "$T/port" &
FAKE=$!
trap 'kill "$FAKE" 2>/dev/null' EXIT
i=0; while [ ! -s "$T/port" ] && [ "$i" -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
[ -s "$T/port" ] || { echo "    the fake runtime did not start"; exit 1; }
PORT="$(cat "$T/port")"

{
  printf 'version: 1\nvendors:\n  - id: acme\n    title: Acme\n'
  printf '  - id: ollama\n    title: Ollama\n    inference: local\n    runtime:\n'
  printf '      adapter: ollama\n      endpoint: http://127.0.0.1:%s\n      endpoint_env: CASE_OLLAMA_HOST\n' "$PORT"
  printf 'models:\n  - id: acme-large\n    vendor: acme\n    native_id: acme-large-1\n    capabilities: [text]\n'
} > "$R/share/models.yaml"

# The observer runs the executable this case was given, against the fixture's share.
printf '#!/bin/sh\nMAJORDOMUS_SHARE="%s" exec "%s" "$@"\n' "$R/share" "$RB" > "$T/mj-cli"
chmod +x "$T/mj-cli"
mj() { local cwd="$1"; shift; ( cd "$cwd" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }
observe() { ( cd "$R" && env -u CASE_OLLAMA_HOST MAJORDOMUS_CLI="$T/mj-cli" node "$ROOT/scripts/inference-observe" "$@" ); }

# ---------------------------------------------------------------- 1. nothing observed is an answer
expect_exit 0 mj "$R" models observed
expect_grep "none recorded"
expect_exit 0 mj "$R" models route --observed --local-only
expect_grep "nothing qualifies"
expect_grep "none recorded"

# ---------------------------------------------------------------- 2. the runtime is observed
# Proves `models-local-runtime-observed`: the endpoint comes from the catalogue, the
# runtime is identified by its own signature, and its inventory is recorded typed.
out="$(observe 2>&1)" || { echo "    inference-observe failed: $out"; exit 1; }
printf '%s' "$out" | grep -q "ollama .*identified 0.0.0-case — 2 model(s), 1 loaded" \
  || { echo "    the runtime was not identified: $out"; exit 1; }
json="$(mj "$R" models observed --format json 2>/dev/null)" \
  || { echo "    models observed --format json failed"; exit 1; }
printf '%s' "$json" | jq -e '
  .stale == false and .models == 2 and
  (.observation.runtimes[0] | .state == "identified" and .locality == "loopback" and (.id | startswith("rt-"))) and
  (.observation.runtimes[0].models | map(.native_id) == ["tiny:chat", "tiny:embed"]) and
  (.observation.runtimes[0].models[0] | .digest == "sha256:aa11" and .loaded == true
     and .context_window.configured == 4096 and .context_window.advertised == 8192
     and (.standing | map("\(.word):\(.evidence)") == ["text:advertised"]))' >/dev/null \
  || { echo "    the observation is not what the runtime reported"; printf '%s\n' "$json" | head -60; exit 1; }

# ---------------------------------------------------------------- 3. routing reads the evidence
# Proves `models-observed-evidence-routes`: an observed model is chosen only on the
# evidence its required words rest on, and the reason says what that is.
expect_exit 0 mj "$R" models route --observed --local-only --require text
expect_grep "selected   ollama/tiny:chat"
expect_grep "text advertised"
expect_grep "excluded   ollama/tiny:embed  — missing capability: text"
expect_exit 0 mj "$R" models route --observed --local-only --require text --min-evidence probed
expect_grep "nothing qualifies"
expect_grep "text is advertised, under the required probed"
# a probe of the loaded model raises its word to probed
out="$(observe --probe 2>&1)" || { echo "    inference-observe --probe failed: $out"; exit 1; }
expect_exit 0 mj "$R" models route --observed --local-only --require text --min-evidence probed
expect_grep "selected   ollama/tiny:chat"
expect_grep "text probed"
# a stale observation chooses nothing and says to observe again
sleep 1
expect_exit 0 mj "$R" models route --observed --local-only --require text --max-age 0
expect_grep "observe again with scripts/inference-observe"
# without --observed, routing is the catalogue alone, as it was
expect_exit 0 mj "$R" models route --local-only
expect_grep "nothing qualifies"
if printf '%s' "$LAST_OUT" | grep -q "tiny:"; then
  echo "    an observed model was considered without --observed"; exit 1
fi

# ---------------------------------------------------------------- 4. the observation stays local
# Proves `models-observation-stays-local`: an endpoint off the loopback interface is not
# reached unless admitted, the crate refuses one the observer did not admit, and the
# observation lives under .ai/local, which is never tracked.
out="$(observe --endpoint ollama=http://192.0.2.1:11434 --dry-run 2>&1)"
printf '%s' "$out" | jq -e '.refused[0].reason | test("loopback")' >/dev/null \
  || { echo "    a remote endpoint was not refused: $out"; exit 1; }
cat > "$T/remote.json" <<'JSON'
{"version":1,"observed_at":"2026-10-17T00:00:00Z","runtimes":[{"vendor":"ollama","endpoint":"http://192.0.2.1:11434","state":"unreachable"}]}
JSON
expect_exit 13 mj "$R" models observe --file "$T/remote.json"
expect_grep "off the loopback interface"
[ -f "$R/.ai/local/state/inference/observation.json" ] \
  || { echo "    the observation is not under .ai/local/state/inference"; exit 1; }
# this repository's own ignore rules keep it out of every commit, and so off the site
git -C "$ROOT" check-ignore -q .ai/local/state/inference/observation.json \
  || { echo "    this repository does not ignore the observation"; exit 1; }
