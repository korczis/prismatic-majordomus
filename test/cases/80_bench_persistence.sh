# majordomus-covers: bench
# majordomus-negative: bench
#
# A benchmark run is local evidence: every run is one versioned document under
# .ai/local/benchmarks/runs/, latest.json is a byte-identical projection of the newest run,
# history.jsonl carries one complete line per run in order, nothing temporary survives a
# save, an interrupted run leaves no partial document behind, and none of it is tracked.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq is required to read the run"; exit 1; }
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
D=.ai/local/benchmarks

# --- two runs: two documents, the projection follows the newest, the history keeps both
expect_exit 0 "$MJ" bench version --samples 2 --warmup 0
first="$(jq -r .run_id "$D/latest.json")"
expect_exit 0 "$MJ" bench version --samples 2 --warmup 0
second="$(jq -r .run_id "$D/latest.json")"
[ "$first" != "$second" ] || { echo "    two runs share one run id"; exit 1; }
[ "$(ls "$D/runs" | wc -l | tr -d ' ')" = 2 ] || { echo "    expected one document per run under runs/"; ls "$D/runs"; exit 1; }
cmp -s "$D/runs/$second.json" "$D/latest.json" || { echo "    latest.json is not the newest run, byte for byte"; exit 1; }
[ "$(wc -l < "$D/history.jsonl" | tr -d ' ')" = 2 ] || { echo "    history.jsonl does not carry one line per run"; exit 1; }
[ "$(jq -r .run_id "$D/history.jsonl" | tr '\n' ' ')" = "$first $second " ] || { echo "    history.jsonl is not in run order or its lines do not parse"; exit 1; }
for f in "$D"/runs/*.json; do
  jq -e '.schema == "majordomus/benchmark-result/v1" and (.repository.commit | length == 40) and (.repository.dirty | type == "boolean") and .environment.os != "" and .profile.samples == 2' "$f" >/dev/null \
    || { echo "    $f is not a complete run document"; cat "$f"; exit 1; }
done
[ -z "$(find "$D" -name '.*' -type f)" ] || { echo "    a temporary file survived the save"; find "$D" -name '.*' -type f; exit 1; }

# --- an interrupted run writes nothing: no partial run, no partial projection, no half line
rm -rf "$D"
"$MJ" bench version --samples 200 --warmup 0 >/dev/null 2>&1 &
pid=$!
sleep 1
kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null || true
pkill -f "bench version --samples 200" 2>/dev/null || true
[ ! -e "$D/latest.json" ] || { echo "    an interrupted run left a latest.json"; exit 1; }
[ -z "$(ls "$D/runs" 2>/dev/null)" ] || { echo "    an interrupted run left a run document"; exit 1; }
[ ! -e "$D/history.jsonl" ] || { echo "    an interrupted run wrote history"; exit 1; }

# --- local evidence is not tracked, and --no-save writes nothing
expect_exit 0 "$MJ" bench version --samples 1 --warmup 0
[ -z "$(git status --porcelain)" ] || { echo "    a run changed the tracked tree"; git status --porcelain; exit 1; }
rm -rf "$D"
expect_exit 0 "$MJ" bench version --samples 1 --warmup 0 --no-save
[ ! -e "$D" ] || { echo "    --no-save still wrote under $D"; exit 1; }
echo "    one document per run, latest.json a projection of the newest, history in order, nothing partial, nothing tracked"
