# majordomus-covers: capability:instances.list
# claims: mcp-instances-from-registries
# Live MCP instances that announce themselves in an instance registry: one owner-only JSON
# record per instance (mcp-instance/v1), written and removed by the instance. Majordomus lists
# them from every registry the configuration names, says which owners are gone, refuses a
# record that is malformed or carries a token, deletes nothing, and probes a loopback endpoint
# only when asked, with the token read from the record's token file and never echoed (I4621).
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git add -A >/dev/null; git commit -qm base
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
same() { [ "$2" = "$3" ] || { printf '    %s: expected [%s], got [%s]\n' "$1" "$3" "$2"; return 1; }; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

REG="$PWD/registry"; mkdir -p "$REG"
SECRET="s3cr3t-token-never-echoed-4621"
# a process that existed and is gone: its pid cannot belong to a live owner
sh -c 'exit 0' & GONE=$!; wait "$GONE"

cat > "$REG/live.json" <<JSON
{"schema": "mcp-instance/v1", "name": "live", "kind": "server", "owner_pid": $$, "registered": 1,
 "endpoint": "http://127.0.0.1:9/mcp", "token_file": "$PWD/live-token.json", "producer": "case 4621"}
JSON
cat > "$REG/stale.json" <<JSON
{"schema": "mcp-instance/v1", "name": "stale", "kind": "gui", "owner_pid": $GONE, "registered": 1,
 "endpoint": "http://127.0.0.1:9/mcp"}
JSON
printf '{"name": "broken", ' > "$REG/broken.json"
cat > "$REG/leaky.json" <<JSON
{"schema": "mcp-instance/v1", "name": "leaky", "kind": "server", "owner_pid": $$, "registered": 1,
 "endpoint": "http://127.0.0.1:9/mcp", "agent_token": "$SECRET"}
JSON
chmod 600 "$REG"/*.json

list() { "$RB" run instances.list --repo . --input "$1" --quiet --format json 2>err.txt | jq -c '.output'; }

# ---------------------------------------------------------------- from the input's registries
OUT="$(list "{\"registries\": [\"$REG\", \"$PWD/absent\"]}")"
printf '%s\n' "$OUT" > out.json
same "the live instance" "$(jq -r '[.instances[] | select(.state == "live") | .name] | join(",")' out.json)" "live"
same "the stale instance" "$(jq -r '[.instances[] | select(.state == "stale") | .name] | join(",")' out.json)" "stale"
same "refused files" "$(jq -r '[.refused[].file] | join(",")' out.json)" "broken.json,leaky.json"
expect_grep '"reason":"carries a token' out.json
expect_no_grep "$SECRET" out.json
same "an absent registry is reported, not an error" "$(jq -r '.registries[] | select(.path | endswith("/absent")) | .exists' out.json)" "false"
same "nothing probed unless asked" "$(jq -r '[.instances[] | select(has("reachable"))] | length' out.json)" "0"
# Majordomus does not own the records: the stale one stays
expect_file "$REG/stale.json"

# ---------------------------------------------------------------- from the environment
MAJORDOMUS_INSTANCE_REGISTRIES="$PWD/absent:$REG" "$RB" run instances.list --repo . --input '{}' --quiet --format json 2>/dev/null \
  | jq -c '.output' > env.json
same "the environment names the registries" "$(jq -r '[.registries[].source] | unique | join(",")' env.json)" "environment"
same "both instances from the environment" "$(jq -r '.instances | length' env.json)" "2"

# ---------------------------------------------------------------- a probe never leaves the machine
cat > "$REG/remote.json" <<JSON
{"schema": "mcp-instance/v1", "name": "remote", "kind": "server", "owner_pid": $$, "registered": 1,
 "endpoint": "http://192.0.2.1:9/mcp", "token_file": "$PWD/remote-token.json"}
JSON
printf '{"agent_token": "%s", "endpoint": "http://192.0.2.1:9/mcp"}\n' "$SECRET" > remote-token.json
printf '{"agent_token": "%s", "endpoint": "http://127.0.0.1:9/mcp"}\n' "$SECRET" > live-token.json
chmod 600 "$REG/remote.json" remote-token.json live-token.json
OUT="$(list "{\"registries\": [\"$REG\"], \"probe\": true}")"
printf '%s\n' "$OUT" > probe.json
same "a remote endpoint is not probed" "$(jq -r '.instances[] | select(.name == "remote") | .probe_error' probe.json)" "not a loopback endpoint: a probe never sends a token off this machine"
# nothing listens on 127.0.0.1:9 (discard): probed, and unreachable
same "a loopback endpoint is probed" "$(jq -r '.instances[] | select(.name == "live") | .reachable' probe.json)" "false"
expect_no_grep "$SECRET" probe.json
# ---------------------------------------------------------------- the same answer over MCP
# One MCP session against this repository: the tool answers the typed list, token-free.
FRAME="$( { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4621","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_instances","arguments":{"registries":["%s"]}}}\n' "$REG"
  } | "$RB" mcp --standalone --repo . 2>mcp-err.txt | sed -n 2p)"
printf '%s\n' "$FRAME" > mcp.json
same "majordomus_instances answers the live instances over MCP" "$(jq -r '[.result.structuredContent.instances[] | select(.state == "live") | .name] | sort | join(",")' mcp.json)" "live,remote"
expect_no_grep "$SECRET" mcp.json

# majordomus-negative: capability:instances.list
# a registries entry that is not a string is refused by the input contract, not guessed
if "$RB" run instances.list --repo . --input '{"registries": [1]}' --quiet --format json >bad.json 2>&1; then
  printf '    a registries entry that is not a string was accepted\n'; exit 1
fi
