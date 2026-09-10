# majordomus-covers: none
# Two linked worktrees of one repository, made the way the topology makes them, each with a
# server of its own. ADR 0035 decided that a server serves the checkout it started in and
# that the checkouts of one git repository can be told from the checkouts of two; this is
# that decision seen from a shell, in a repository built by `init` and worktrees built by
# `worktree create`, rather than by a `git worktree add` a test wrote itself.
#
# What it proves: the three checkouts report one `git.id` and three distinct `checkout_id`s;
# each server lists every checkout of the repository, the primary first, with the branch and
# the standing of each — including the primary, which has no server at all; a checkout's own
# entry is the one marked `this_checkout`; and `serve stop` in one worktree leaves the other
# worktree's server running and turns only its own entry `absent`.
#
# Every server is started with `--port 0`: the default port belongs to whatever is already
# serving this machine, and a case may never take it.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }

# The repository is nested one level down: its worktree container is by definition its
# sibling, so a repository at $T would leave a container beside $T. Paths are resolved,
# because git holds the resolved path and macOS's $TMPDIR is a symlink.
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

A="$R-wt/feature/alpha"; B="$R-wt/feature/beta"
# nothing is stopped by pattern: each server is ended through the lease of its own checkout
trap '"$RB" serve stop --repo "$A" >/dev/null 2>&1 || true; "$RB" serve stop --repo "$B" >/dev/null 2>&1 || true; "$RB" serve stop --repo "$R" >/dev/null 2>&1 || true' EXIT

mj() { local cwd="$1"; shift; ( cd "$cwd" && "$RB" "$@" ); }

# ---------------------------------------------------------------- 1. two worktrees, by the topology
expect_exit 0 mj "$R" worktree create feature/alpha --base "$trunk"
expect_grep "$A"
expect_exit 0 mj "$R" worktree create feature/beta --base "$trunk"
[ -d "$A" ] && [ -d "$B" ] || { echo "    the worktrees are not at the derived paths"; exit 1; }
[ -f "$A/.ai/manifest.yaml" ] || { echo "    the linked worktree carries no layer"; exit 1; }

# ---------------------------------------------------------------- 2. a server in each, and none in the primary
for w in "$A" "$B"; do
  mj "$w" serve ensure --port 0 --idle 60 --wait 40 --format json > "$T/ensure.json" 2> "$T/ensure.err" \
    || { echo "    serve ensure in $w did not converge:"; cat "$T/ensure.json" "$T/ensure.err"; exit 1; }
  jq -e '.standing == "ready" and .started == true and (.url | startswith("http://127.0.0.1:"))' "$T/ensure.json" >/dev/null \
    || { echo "    $w has no server of its own:"; cat "$T/ensure.json"; exit 1; }
done
url_a="$(mj "$A" serve status --format json | jq -r '.this_process.url')"
url_b="$(mj "$B" serve status --format json | jq -r '.this_process.url')"
[ -n "$url_a" ] && [ "$url_a" != "$url_b" ] || { echo "    the two worktrees share one address ($url_a / $url_b)"; exit 1; }
[ ! -f "$R/.ai/local/state/mcp/server.json" ] || { echo "    a server appeared in the primary, which nothing asked for"; exit 1; }

# ---------------------------------------------------------------- 3. one repository, three checkouts
mj "$A" serve status --format json > "$T/a.json"
mj "$B" serve status --format json > "$T/b.json"
git_a="$(jq -r '.git.id' "$T/a.json")"; git_b="$(jq -r '.git.id' "$T/b.json")"
[ -n "$git_a" ] && [ "$git_a" != null ] || { echo "    no git identity in the status"; cat "$T/a.json"; exit 1; }
[ "$git_a" = "$git_b" ] || { echo "    two worktrees of one repository report different git identities: $git_a / $git_b"; exit 1; }
[ "$(jq -r '.checkout_id' "$T/a.json")" != "$(jq -r '.checkout_id' "$T/b.json")" ] \
  || { echo "    two checkouts report one identity"; exit 1; }
for f in "$T/a.json" "$T/b.json"; do
  jq -e '.git.linked == true' "$f" >/dev/null || { echo "    a linked worktree does not say it is one: $f"; cat "$f"; exit 1; }
  jq -e '(.servers | length) == 3' "$f" >/dev/null || { echo "    the repository has three checkouts and the status lists $(jq '.servers|length' "$f"): $f"; cat "$f"; exit 1; }
  jq -e '.servers[0].primary == true and .servers[0].this_checkout == false and .servers[0].standing == "absent"' "$f" >/dev/null \
    || { echo "    the primary is not listed first, or is not the serverless checkout it is: $f"; cat "$f"; exit 1; }
  jq -e '[.servers[] | select(.this_checkout)] | length == 1' "$f" >/dev/null \
    || { echo "    exactly one entry is this checkout: $f"; cat "$f"; exit 1; }
done
# each names the other, with its branch, its standing and the address its lease holds
jq -e --arg url "$url_b" '.servers[] | select(.branch == "feature/beta")
  | .this_checkout == false and .standing == "ready" and .lease.url == $url and .peers == 0' "$T/a.json" >/dev/null \
  || { echo "    alpha's server does not see beta's:"; cat "$T/a.json"; exit 1; }
jq -e --arg url "$url_a" '.servers[] | select(.branch == "feature/alpha")
  | .this_checkout == false and .standing == "ready" and .lease.url == $url' "$T/b.json" >/dev/null \
  || { echo "    beta's server does not see alpha's:"; cat "$T/b.json"; exit 1; }
jq -e --arg b "$trunk" '.servers[] | select(.primary) | .branch == $b' "$T/a.json" >/dev/null \
  || { echo "    the primary's branch is not named"; cat "$T/a.json"; exit 1; }

# ---------------------------------------------------------------- 4. stopping one leaves the other alone
expect_exit 0 mj "$A" serve stop
expect_grep '^stopped '
[ ! -f "$A/.ai/local/state/mcp/server.json" ] || { echo "    alpha's lease survived its stop"; exit 1; }
mj "$B" serve status --format json > "$T/b2.json"
jq -e --arg url "$url_b" '.standing == "ready" and .this_process.url == $url' "$T/b2.json" >/dev/null \
  || { echo "    stopping alpha's server took beta's with it:"; cat "$T/b2.json"; exit 1; }
jq -e '.servers[] | select(.branch == "feature/alpha") | .standing == "absent" and (.lease | not)' "$T/b2.json" >/dev/null \
  || { echo "    beta still believes alpha is serving:"; cat "$T/b2.json"; exit 1; }
expect_exit 0 mj "$B" serve stop
expect_grep '^stopped '

echo "    two worktrees of one repository each carry a server, name the other, and stop apart"
