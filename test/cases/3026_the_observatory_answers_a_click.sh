# majordomus-covers: none
# claims: observatory-draws-nothing-it-did-not-read
# The Observatory answers a reader who points at a mark (ADR 0158, case 3026), through a real
# server over a fixture `majordomus init` installs:
#
#   freshness       every source says how its reading stands: current, stale or pending
#   a mark          every painter of the director, drawing the server's own model, registers
#                   what it drew: an identity that is in the scene's model, a kind, a state, a
#                   truth class and a place on the stage; the detail a click opens is made of
#                   those words
#   read-only       the scripts ask the server for nothing but the page and the journal's
#                   events, and send nothing that would change anything
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, or no node.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "jq absent"
command -v node >/dev/null 2>&1 || skip_case "node is absent, so no mark was drawn and read back"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# projection <page.html> <out.json>: the value of data-observatory, unescaped
projection() {
  grep -oE 'data-observatory="[^"]*"' "$1" | head -1 \
    | sed -e 's/^data-observatory="//' -e 's/"$//' \
          -e 's/&quot;/"/g' -e 's/&#34;/"/g' -e "s/&#39;/'/g" -e "s/&#x27;/'/g" \
          -e 's/&lt;/</g' -e 's/&gt;/>/g' -e 's/&amp;/\&/g' > "$2"
  [ -s "$2" ] || { echo "    $1 carries no data-observatory"; return 1; }
  jq -e . "$2" >/dev/null 2>&1 || { echo "    data-observatory of $1 is not JSON"; return 1; }
}

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
code() { curl -s --max-time 120 -o "$2" -w '%{http_code}' "$U$1"; }

# ---------------------------------------------------------------- freshness
[ "$(code /ui/observatory obs.html)" = 200 ] || { echo "    /ui/observatory did not answer 200"; exit 1; }
projection obs.html obs.json || exit 1
# a reading's standing is stated, never guessed from its age
jq -e 'all(.sources[]; .freshness == "current" or .freshness == "stale" or .freshness == "pending")' obs.json >/dev/null \
  || { echo "    a source does not say how its reading stands:"; jq -c '[.sources[] | {capability, freshness}]' obs.json; exit 1; }
serve_down; SRV=""

# ---------------------------------------------------------------- a mark
# Every painter of the director draws the server's own model onto a context that records
# nothing, and the marks it registered are read back: each names a record of that model.
MARKS="$(mktemp "${TMPDIR:-/tmp}/mj3026-marks.XXXXXX")"
node --input-type=module - "$ROOT/share/ui/observatory.js" obs.json > "$MARKS" 2>&1 <<'NODE' \
  || { echo "    a painter does not register what it drew:"; sed 's/^/    | /' "$MARKS"; rm -f "$MARKS"; exit 1; }
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
const source = readFileSync(process.argv[2], 'utf8');
const projection = JSON.parse(readFileSync(process.argv[3], 'utf8'));
// a 2D context that accepts every call and draws nothing
const ctx = new Proxy({}, { get: (t, k) => (k in t ? t[k] : () => {}), set: (t, k, v) => { t[k] = v; return true; } });
ctx.getImageData = () => ({ data: [0, 0, 0, 0] });
const sandbox = {
  document: { createElement: () => ({ getContext: () => ctx }), body: { dataset: {} } },
  getComputedStyle: () => ({ getPropertyValue: () => '' }),
  performance: { now: () => 0 },
};
sandbox.window = sandbox;
vm.runInNewContext(source, sandbox);
const registry = sandbox.mjObservatory;
if (!registry || !registry.painters) throw new Error('the script registers no painters');
let drawn = 0;
for (const scene of projection.scenes) {
  const paint = registry.painters[scene.id];
  if (!scene.available || !paint) continue;
  const marks = [];
  const box = { x: 0, y: 0, w: 1600, h: 900, cx: 800, cy: 450, u: 900 / 44, font: 'monospace', mark: (m) => marks.push(m) };
  paint(ctx, box, scene.model, 0, false, new Map());
  const model = JSON.stringify(scene.model);
  for (const m of marks) {
    const where = typeof m.w === 'number' ? [m.x, m.y, m.w, m.h] : [m.x, m.y, m.r];
    if (!where.every(Number.isFinite)) throw new Error(`${scene.id}: mark ${m.id} has no place on the stage`);
    if (!m.kind || m.state === undefined) throw new Error(`${scene.id}: mark ${m.id} names no kind or state`);
    if (m.truth !== 'observed' && m.truth !== 'derived') throw new Error(`${scene.id}: mark ${m.id} has truth ${m.truth}`);
    // a mark that is a part of a record is named `record · part`
    const named = String(m.id).split(' · ')[0];
    if (m.kind !== 'recorded run' && !model.includes(JSON.stringify(named).slice(1, -1))) {
      throw new Error(`${scene.id}: mark ${m.id} names nothing in the scene's model`);
    }
  }
  if (scene.id !== 'ambient' && !marks.length) throw new Error(`${scene.id}: drew a model and registered no mark`);
  drawn += 1;
  console.log(`${scene.id}: ${marks.length} mark(s)`);
}
if (!drawn) throw new Error('no available scene had a painter to draw it');
NODE
rm -f "$MARKS"

# ---------------------------------------------------------------- read-only
# The display asks for its page and the journal's events, and nothing else; it never sends a
# method that would change anything.
for f in observatory.js observatory-kiosk.js; do
  grep -nE "method:|'POST'|\"POST\"|'PUT'|'DELETE'|'PATCH'|sendBeacon|new WebSocket" "$ROOT/share/ui/$f" \
    && { echo "    share/ui/$f sends something that could change state"; exit 1; }
done
n="$(grep -c 'fetch(' "$ROOT/share/ui/observatory.js")"
[ "$n" = 2 ] || { echo "    share/ui/observatory.js fetches in $n places, expected 2 (the page and the journal)"; exit 1; }
grep -q 'fetch(' "$ROOT/share/ui/observatory-kiosk.js" && { echo "    share/ui/observatory-kiosk.js fetches"; exit 1; }
exit 0
