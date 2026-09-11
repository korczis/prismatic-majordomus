# majordomus-covers: none
# Drift, planted on purpose, and the answer that has to catch it.
#
# Every state below is written into a fixture by hand rather than waited for: a lease of one
# checkout naming the server of another — which is what a copied checkout, a moved worktree
# or a lease restored from a backup produces — and a lease whose server answers from a
# version this executable is not. Both are the states `server.status` exists to name, and
# neither can be reached by running the tool correctly.
#
# The second half is the agreement itself: the same question asked of the same server
# through two surfaces — `GET /api/v1/server` and `majordomus serve status --format json` —
# has to come back as one answer. Two surfaces that disagree about the server's own state
# are how a stale runtime went unnoticed for a day; this is the check that they cannot.
#
# `--port 0` throughout: the default port belongs to whatever already serves this machine.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
command -v curl >/dev/null 2>&1 || { echo "    skip: curl not installed"; exit 0; }

mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
mkdir -p "$R"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
( cd "$R" && "$MJ" init >/dev/null )
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null
trunk="$(git -C "$R" symbolic-ref --short HEAD)"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the executable's own progress is not evidence here, and it would land in the output
# expect_grep reads: only what a command decides is worth saying should be there
MAJORDOMUS_LOG=warn; export MAJORDOMUS_LOG

A="$R-wt/feature/alpha"
trap '"$RB" serve stop --repo "$A" >/dev/null 2>&1 || true; "$RB" serve stop --repo "$R" >/dev/null 2>&1 || true' EXIT
mj() { local cwd="$1"; shift; ( cd "$cwd" && "$RB" "$@" ); }

expect_exit 0 mj "$R" worktree create feature/alpha --base "$trunk"
mj "$A" serve ensure --port 0 --idle 60 --wait 40 --format json > "$T/ensure.json" 2> "$T/ensure.err" \
  || { echo "    the worktree's server did not come up:"; cat "$T/ensure.json" "$T/ensure.err"; exit 1; }
url="$(jq -r '.url' "$T/ensure.json")"
[ -n "$url" ] && [ "$url" != null ] || { echo "    no address"; cat "$T/ensure.json"; exit 1; }
la="$A/.ai/local/state/mcp/server.json"; lr="$R/.ai/local/state/mcp/server.json"

# ---------------------------------------------------------------- 1. the two surfaces agree
agree() {   # agree WHEN — the same question through HTTP and through the command line
  curl -fsS --max-time 30 "$url/api/v1/server" | jq -S . > "$T/http.json" \
    || { echo "    GET /api/v1/server failed ($1)"; exit 1; }
  mj "$A" serve status --format json | jq -S . > "$T/cli.json" \
    || { echo "    serve status --format json failed ($1)"; exit 1; }
  diff -u "$T/http.json" "$T/cli.json" > "$T/agree.diff" \
    || { echo "    the two surfaces disagree about the server's state ($1):"; cat "$T/agree.diff"; exit 1; }
}
agree "with nothing planted"
jq -e '.standing == "ready"' "$T/cli.json" >/dev/null || { echo "    the server is not ready to begin with"; cat "$T/cli.json"; exit 1; }

# ---------------------------------------------------------------- 2. a lease naming another checkout's server
# the primary's lease is made to name the worktree's server: an address that answers, for a
# root that is not its own. Nothing but a hand-written file can produce this.
mkdir -p "$(dirname "$lr")"
jq --arg root "$R" '.root = $root | .token = "planted"' "$la" > "$lr"
jq -e --arg url "$url" '.url == $url' "$lr" >/dev/null || { echo "    the planted lease does not name the worktree's server"; cat "$lr"; exit 1; }

expect_exit 0 mj "$R" serve status
expect_grep '^standing   stale'
expect_grep 'does not answer for this checkout'
mj "$R" serve status --format json > "$T/r.json"
jq -e --arg url "$url" '.standing == "stale"
  and (.this_process | not)
  and (.servers[] | select(.this_checkout) | .standing == "stale" and .lease.url == $url and (.reason | contains($url)))' "$T/r.json" >/dev/null \
  || { echo "    the planted lease was not refused by name:"; cat "$T/r.json"; exit 1; }

# and the worktree's own server, asked from its side, calls the primary's lease stale too
mj "$A" serve status --format json > "$T/a.json"
jq -e '.servers[] | select(.primary) | .standing == "stale"' "$T/a.json" >/dev/null \
  || { echo "    the worktree's server does not see the primary's drift:"; cat "$T/a.json"; exit 1; }

# the refusal is the whole point: a stop in the drifted checkout may not end a server that
# is not its own
expect_exit 0 mj "$R" serve stop
expect_grep 'does not answer for this checkout'
curl -fsS -m 3 "$url/" >/dev/null || { echo "    the primary's stop ended the worktree's server"; exit 1; }
agree "with a foreign lease planted in the primary"

# ---------------------------------------------------------------- 3. a server of another version
# the address still answers; the lease now claims a version this executable is not, which is
# exactly what an executable rebuilt under a running server leaves behind
cp "$la" "$T/lease.real"
jq '.version = "0.0.0-planted"' "$T/lease.real" > "$la"
expect_exit 0 mj "$A" serve status
expect_grep '^standing   outdated'
expect_grep '0\.0\.0-planted'
# `ensure` reports it with the remedy and starts nothing: a server that answers is somebody's
pid_before="$(jq -r '.pid' "$la")"
expect_exit 10 mj "$A" serve ensure --port 0 --wait 2 --format json
printf '%s' "$LAST_OUT" | jq -e '.standing == "outdated" and .started == false and (.reason | contains("serve stop"))' >/dev/null \
  || { echo "    ensure did not refuse an outdated server with the remedy:"; printf '%s\n' "$LAST_OUT"; exit 1; }
[ "$(jq -r '.pid' "$la")" = "$pid_before" ] || { echo "    ensure started a second server over one that answers"; exit 1; }
agree "with a version planted in the lease"

# ---------------------------------------------------------------- 4. undone, the drift is gone
cp "$T/lease.real" "$la"
expect_exit 0 mj "$A" serve status
expect_grep '^standing   ready'
expect_exit 0 mj "$A" serve stop
expect_grep '^stopped '

echo "    a foreign lease and a foreign version are both named and refused, and both surfaces say the same thing"
