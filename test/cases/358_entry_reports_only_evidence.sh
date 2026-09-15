# majordomus-covers: none
#
# Entering the repository reports only what evidence proves (project.entry-reports-only-evidence,
# ADR 0066).
#
# On 2026-09-15 the entry banner drew a check mark beside the Cockpit and MCP while the server
# behind them was two versions old: the mark was a TCP connection. This case enters a fixture
# the way a shell does and holds what is drawn to the preflight's own verdicts:
#
#   1. no lease: the server and its surfaces are unavailable, not merely unmarked
#   2. a lease naming an address nothing answers: the server failed, the surfaces unavailable
#   3. a foreign process that accepts the connection: still not this checkout's server, and
#      the banner draws no success mark beside a service
#   4. rules discovered are not rules enforced; a tally counted at another commit is stale
#   5. the deployment ref names its source commit: verified at HEAD, stale after a commit
#   6. the repository's own .envrc, sourced in the fixture, renders the preflight through the
#      one bootstrap call and nothing else
#
# The Rust half — every verdict over hand-built observations, and the command line, API, MCP
# and Cockpit agreeing over one served fixture — is apps/majordomus-cli/tests/preflight.rs.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# ASCII glyphs, so the marks compared below do not depend on the machine's locale
LC_ALL=C; export LC_ALL
NO_COLOR=1; export NO_COLOR
lease=.ai/local/state/mcp/server.json

verdicts() { "$RB" env preflight --repo "$T" --format json "$@"; }
verdict_of() {   # <id>, reading the JSON on stdin
  python3 -c 'import json,sys
d=json.load(sys.stdin)
print({c["id"]:c["verdict"] for s in d["sections"] for c in s["checks"]}[sys.argv[1]])' "$1"
}
expect_verdict() {   # <id> <verdict> <why>
  local got
  got="$(verdicts | verdict_of "$1")" || { echo "    no preflight for $1"; exit 1; }
  [ "$got" = "$2" ] || { echo "    $1 is '$got', expected '$2': $3"; verdicts | python3 -m json.tool | head -80; exit 1; }
}
free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'; }
write_lease() {   # <url>
  mkdir -p "$(dirname "$lease")"
  printf '{"schema":"majordomus-mcp-lease/v1","pid":%s,"token":"t","root":"%s","url":"%s","started_at":"2026-09-15T00:00:00Z","version":"0.0.1"}\n' \
    "$$" "$T" "$1" > "$lease"
}

# --- 1. nothing serves this checkout
[ ! -f "$lease" ] || { echo "    a fresh fixture has a lease"; exit 1; }
expect_verdict integration.server unavailable "no lease is no server"
expect_verdict integration.cockpit unavailable "no server serves no Cockpit"
expect_verdict integration.mcp unavailable "no server serves no MCP"

# --- 2. configured, and stopped: the lease names an address nothing listens on
write_lease "http://127.0.0.1:$(free_port)"
expect_verdict integration.server failed "a lease naming a silent address is not a healthy server"
expect_verdict integration.cockpit unavailable "a Cockpit nothing serves is unavailable"

# --- 3. something accepts the connection, and it is not this checkout's server
port="$(free_port)"
python3 -m http.server --bind 127.0.0.1 "$port" >/dev/null 2>&1 &
foreign=$!
trap 'kill "$foreign" 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do
  python3 -c "import socket,sys; s=socket.socket(); sys.exit(s.connect_ex(('127.0.0.1',$port)))" && break
  sleep 0.1
done
write_lease "http://127.0.0.1:$port"
expect_verdict integration.server failed "an answer from a process that is not majordomus proves nothing"
"$RB" env enter --repo "$T" --shell direnv --no-runtime --no-bridge --mode full >/dev/null 2>"$T/full.err" \
  || { echo "    entry failed"; cat "$T/full.err"; exit 1; }
# the box still lists the service, with its address; the mark beside it is not a success
row="$(grep -E '^\| +cockpit ' "$T/full.err" || true)"
[ -n "$row" ] || { echo "    the banner lists no cockpit row:"; cat "$T/full.err"; exit 1; }
case "$row" in
  *" ok "*|*" ok|"*) echo "    the banner marks the Cockpit answering while the server is not verified: $row"; exit 1 ;;
esac
grep -qE 'server x' "$T/full.err" || { echo "    the entry did not render the server's failed verdict:"; cat "$T/full.err"; exit 1; }
kill "$foreign" 2>/dev/null || true
rm -f "$lease"

# --- 4. rules discovered are not rules enforced, and a count goes stale with its commit
expect_verdict governance.rules unknown "entry never builds the index, and nothing cached a count"
"$RB" env preflight --repo "$T" --full --format json >/dev/null || { echo "    preflight --full failed"; exit 1; }
expect_verdict governance.rules active "the tally a full run cached is at this commit"
got="$(verdicts | verdict_of verification.enforcement)"
[ "$got" != verified ] || { echo "    rules with no recorded run were reported enforced"; exit 1; }
echo b > lib/b && git add . && git commit -qm later
expect_verdict governance.rules stale "the tally was counted at the previous commit"

# --- 5. the deployment ref, as scripts/pages writes it
head="$(git rev-parse HEAD)"
tree="$(git mktree </dev/null)"
deploy="$(git commit-tree "$tree" -m "deploy: site from ${head%"${head#???????}"}" -m "source: $head")"
git update-ref refs/remotes/origin/gh-pages "$deploy"
expect_verdict verification.deployment verified "the site was built from HEAD"
echo c > lib/c && git add . && git commit -qm after-deploy
expect_verdict verification.deployment stale "HEAD moved past the deployed source"

# --- 6. the repository's own entry file, through the one bootstrap call
cp "$ROOT/.envrc" .envrc
mkdir -p bin && cp "$ROOT/bin/majordomus-env" bin/ && mkdir -p lib && cp "$ROOT/lib/rust_bin.sh" lib/
out="$(MAJORDOMUS_BIN="$RB" MAJORDOMUS_BANNER=compact MAJORDOMUS_RUNTIME=off bash -c '
  PATH_add() { :; }; watch_file() { :; }; source_env_if_exists() { :; }
  . ./.envrc
  printf "ROOT=%s\n" "$MAJORDOMUS_ROOT"
' 2>"$T/envrc.err")" || { echo "    sourcing .envrc failed"; cat "$T/envrc.err"; exit 1; }
# compared as the executable spells it: the canonical path, which on macOS puts /private
# in front of a temporary directory the shell names without it
printf '%s\n' "$out" | grep -q "^ROOT=$(cd "$T" && pwd -P)" || { echo "    .envrc exported no MAJORDOMUS_ROOT:"; printf '%s\n' "$out"; cat "$T/envrc.err"; exit 1; }
grep -qE '^> ' "$T/envrc.err" || { echo "    entering drew no preflight:"; cat "$T/envrc.err"; exit 1; }
server="$(verdicts | verdict_of integration.server)"
case "$server" in
  unavailable) mark=o ;; failed) mark=x ;; *) echo "    unexpected server verdict $server"; exit 1 ;;
esac
grep -qE "server $mark( |$)" "$T/envrc.err" || { echo "    the entry line does not say what the preflight says (server $server):"; cat "$T/envrc.err"; exit 1; }
# the entry file makes the bootstrap call and names no subsystem it could judge itself
grep -cE 'majordomus-env enter' .envrc | grep -qx 1 || { echo "    .envrc does not make exactly one bootstrap call"; exit 1; }
# (the lease it watches is a path direnv needs to re-evaluate on, not a judgement about it)
if grep -vE '^[[:space:]]*(#|watch_file )' .envrc | grep -qiE 'mcp|cockpit|ledger|gh-pages|rules|preflight|coverage'; then
  echo "    .envrc names a subsystem the bootstrap command decides:"; grep -vE '^[[:space:]]*#' .envrc; exit 1
fi
